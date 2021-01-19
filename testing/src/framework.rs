use ya_agreement_utils::{AgreementView, OfferTemplate};
use ya_negotiators::factory::*;
use ya_negotiators::{AgreementResponse, AgreementResult, ProposalResponse};

use ya_client_model::market::proposal::State;
use ya_client_model::market::Proposal;

use crate::node::{Node, NodeType};

use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NegotiationStage {
    Initial,
    Proposal { last_response: ProposalResponse },
    Agreement { last_response: AgreementResponse },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NegotiationResult {
    pub stage: NegotiationStage,
    pub proposals: Vec<Proposal>,
    pub agreement: Option<AgreementView>,
}

#[derive(thiserror::Error)]
#[error("{error}\nNegotiation traceback:\n\n{negotiation_traceback}")]
pub struct FrameworkError {
    error: anyhow::Error,
    negotiation_traceback: NegotiationResult,
}

/// Emulates running negotiations between Requestor and Provider.
/// TODO: Support for multiple Provider/Requestor Negotiators at the same time.
pub struct Framework {
    pub requestor: Node,
    pub provider: Node,
}

impl Framework {
    pub fn new(
        prov_config: NegotiatorsConfig,
        req_config: NegotiatorsConfig,
    ) -> anyhow::Result<Framework> {
        Ok(Framework {
            requestor: Node::new(req_config, NodeType::Requestor)?,
            provider: Node::new(prov_config, NodeType::Provider)?,
        })
    }

    pub async fn run_for_templates(
        &self,
        demand: OfferTemplate,
        offer: OfferTemplate,
    ) -> Result<NegotiationResult, FrameworkError> {
        let offer = self
            .provider
            .create_offer(&offer)
            .await
            .map_err(|e| FrameworkError::from(e, &NegotiationResult::new()))?;
        let demand = self
            .requestor
            .create_offer(&demand)
            .await
            .map_err(|e| FrameworkError::from(e, &NegotiationResult::new()))?;

        self.run_for_offers(demand, offer).await
    }

    /// Negotiators should have offers already created. This functions emulates
    /// negotiations, when negotiators continue negotiations with new Nodes, without
    /// resubscribing Offers/Demands.
    pub async fn run_for_offers(
        &self,
        demand: Proposal,
        offer: Proposal,
    ) -> Result<NegotiationResult, FrameworkError> {
        let result = self.run_negotiation_phase(demand, offer).await?;
        self.run_agreement_phase(result).await
    }

    pub async fn run_finalize_agreement(
        &self,
        agreement: &AgreementView,
        result: AgreementResult,
    ) -> anyhow::Result<()> {
        // First call both functions and resolve errors later. We don't want
        // to omit any of these calls.
        let prov_result = self
            .requestor
            .agreement_finalized(&agreement.id, result.clone())
            .await;
        let req_result = self
            .provider
            .agreement_finalized(&agreement.id, result)
            .await;

        prov_result?;
        req_result?;
        Ok(())
    }

    pub async fn run_negotiation_phase(
        &self,
        demand: Proposal,
        offer: Proposal,
    ) -> Result<NegotiationResult, FrameworkError> {
        let mut result = NegotiationResult {
            proposals: vec![demand.clone(), offer.clone()],
            stage: NegotiationStage::Initial,
            agreement: None,
        };

        let mut prev_requestor_proposal = demand;
        let mut prev_provider_proposal = offer;

        let max_negotiation_steps = 20;

        for _ in 0..max_negotiation_steps {
            let response = self
                .requestor
                .react_to_proposal(&prev_provider_proposal, &prev_requestor_proposal)
                .await
                .map_err(|e| FrameworkError::from(e, &result))?;
            let req_proposal = match &response {
                // Move to Agreement phase. Accepting on Requestor side doesn't generate new Proposal.
                // Agreement will be proposed for last Provider's Proposal.
                ProposalResponse::AcceptProposal => {
                    result.stage = NegotiationStage::Proposal {
                        last_response: response,
                    };
                    return Ok(result);
                }
                ProposalResponse::CounterProposal { offer } => {
                    let proposal = self.requestor.into_proposal(offer.clone(), State::Draft);

                    result.proposals.push(proposal.clone());
                    result.stage = NegotiationStage::Proposal {
                        last_response: response,
                    };
                    proposal
                }
                ProposalResponse::IgnoreProposal | ProposalResponse::RejectProposal { .. } => {
                    result.stage = NegotiationStage::Proposal {
                        last_response: response,
                    };
                    return Ok(result);
                }
            };

            // After Requestor counters Provider's Proposal for the first time,
            // it is no longer in Initial state.
            prev_provider_proposal.state = State::Draft;

            let response = self
                .provider
                .react_to_proposal(&req_proposal, &prev_provider_proposal)
                .await
                .map_err(|e| FrameworkError::from(e, &result))?;
            let prov_proposal = match &response {
                ProposalResponse::CounterProposal { offer } => {
                    let proposal = self.provider.into_proposal(offer.clone(), State::Draft);

                    result.proposals.push(proposal.clone());
                    result.stage = NegotiationStage::Proposal {
                        last_response: response,
                    };
                    proposal
                }
                ProposalResponse::AcceptProposal => {
                    result.proposals.push(prev_provider_proposal.clone());
                    result.stage = NegotiationStage::Proposal {
                        last_response: response,
                    };
                    prev_provider_proposal.clone()
                }
                ProposalResponse::RejectProposal { .. } | ProposalResponse::IgnoreProposal => {
                    result.stage = NegotiationStage::Proposal {
                        last_response: response,
                    };
                    return Ok(result);
                }
            };

            prev_requestor_proposal = req_proposal;
            prev_provider_proposal = prov_proposal;
        }

        return Err(FrameworkError::from(anyhow!(
            "Exceeded negotiation loops limit ({}). Probably your negotiators have wrong stop conditions.",
            max_negotiation_steps
        ), &result));
    }

    pub async fn run_agreement_phase(
        &self,
        mut result: NegotiationResult,
    ) -> Result<NegotiationResult, FrameworkError> {
        match &result.stage {
            NegotiationStage::Proposal { last_response } => match last_response {
                // This is the only correct state in which we should try to propose Agreement.
                // Otherwise we can just return from this function.
                ProposalResponse::AcceptProposal => (),
                _ => return Ok(result),
            },
            _ => return Ok(result),
        };

        let agreement_view = AgreementView::try_from(&self.requestor.create_agreement(
            &result.proposals[result.proposals.len() - 2],
            &result.proposals[result.proposals.len() - 1],
        ))
        .map_err(|e| FrameworkError::from(e, &result))?;

        result.agreement = Some(agreement_view.clone());

        let response = self
            .requestor
            .react_to_agreement(&agreement_view)
            .await
            .map_err(|e| FrameworkError::from(e, &result))?;
        result.stage = NegotiationStage::Agreement {
            last_response: response.clone(),
        };

        if let AgreementResponse::RejectAgreement { .. } = &response {
            return Ok(result);
        }

        let response = self
            .provider
            .react_to_agreement(&agreement_view)
            .await
            .map_err(|e| FrameworkError::from(e, &result))?;
        result.stage = NegotiationStage::Agreement {
            last_response: response.clone(),
        };

        if let AgreementResponse::RejectAgreement { .. } = &response {
            self.requestor
                .agreement_finalized(&agreement_view.id, AgreementResult::ApprovalFailed)
                .await
                .map_err(|e| FrameworkError::from(e, &result))?;
            return Ok(result);
        }

        // Note: Provider will never get AgreementResult::ApprovalFailed, because it can happen only,
        // if `approve_agreement` call fails.

        Ok(result)
    }
}

impl FrameworkError {
    pub fn from(error: impl Into<anyhow::Error>, result: &NegotiationResult) -> FrameworkError {
        FrameworkError {
            error: error.into(),
            negotiation_traceback: result.clone(),
        }
    }
}

impl NegotiationResult {
    pub fn new() -> NegotiationResult {
        NegotiationResult {
            stage: NegotiationStage::Initial,
            proposals: vec![],
            agreement: None,
        }
    }
}

impl fmt::Display for NegotiationResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", serde_json::to_string_pretty(&self).unwrap())
    }
}

impl fmt::Debug for FrameworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}
