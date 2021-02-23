use ya_agreement_utils::{AgreementView, OfferTemplate};
use ya_negotiators::factory::*;
use ya_negotiators::{AgreementAction, AgreementResult, ProposalAction};

use ya_client_model::market::proposal::State;
use ya_client_model::market::{NewProposal, Proposal, Reason};
use ya_client_model::NodeId;

use crate::negotiation_record::{NegotiationRecord, NegotiationResult, NegotiationStage};
use crate::node::{Node, NodeType};

use futures::future::join_all;
use futures::stream::select_all;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;
use std::sync::Arc;
use tokio::sync::broadcast::Receiver;
use tokio::sync::Mutex;
use tokio_stream::wrappers::BroadcastStream;

#[derive(thiserror::Error)]
#[error("{error}\nNegotiation traceback:\n\n{negotiation_traceback}")]
pub struct FrameworkError {
    error: anyhow::Error,
    negotiation_traceback: NegotiationRecord,
}

/// Emulates running negotiations between Requestor and Provider.
/// TODO: Support for multiple Provider/Requestor Negotiators at the same time.
pub struct Framework {
    pub requestors: Vec<Arc<Node>>,
    pub providers: Vec<Arc<Node>>,
}

impl Framework {
    pub fn new(
        prov_config: NegotiatorsConfig,
        req_config: NegotiatorsConfig,
    ) -> anyhow::Result<Framework> {
        Ok(Framework {
            requestors: vec![Node::new(req_config, NodeType::Requestor)?],
            providers: vec![Node::new(prov_config, NodeType::Provider)?],
        })
    }

    pub fn add_provider(mut self, config: NegotiatorsConfig) -> anyhow::Result<Self> {
        let node = Node::new(config, NodeType::Provider)?;
        self.providers.push(node);
        Ok(self)
    }

    pub fn add_requestor(mut self, config: NegotiatorsConfig) -> anyhow::Result<Self> {
        let node = Node::new(config, NodeType::Requestor)?;
        self.requestors.push(node);
        Ok(self)
    }
    //
    //     pub async fn run_for_templates(
    //         &self,
    //         demand: OfferTemplate,
    //         offer: OfferTemplate,
    //     ) -> Result<NegotiationResult, FrameworkError> {
    //         let mut offers = vec![];
    //         for provider in self.providers {
    //             offers.push(
    //                 provider
    //                     .create_offer(&offer)
    //                     .await
    //                     .map_err(|e| FrameworkError::from(e, &NegotiationResult::new()))?,
    //             )
    //         }
    //
    //         let mut demands = vec![];
    //         for requestor in self.requestors {
    //             demands.push(
    //                 requestor
    //                     .create_offer(&demand)
    //                     .await
    //                     .map_err(|e| FrameworkError::from(e, &NegotiationResult::new()))?,
    //             )
    //         }
    //
    //         // let offer = self
    //         //     .provider
    //         //     .create_offer(&offer)
    //         //     .await
    //         //     .map_err(|e| FrameworkError::from(e, &NegotiationResult::new()))?;
    //         // let demand = self
    //         //     .requestor
    //         //     .create_offer(&demand)
    //         //     .await
    //         //     .map_err(|e| FrameworkError::from(e, &NegotiationResult::new()))?;
    //
    //         self.run_for_offers(demand, offer).await
    //     }
    //
    //     /// Negotiators should have offers already created. This functions emulates
    //     /// negotiations, when negotiators continue negotiations with new Nodes, without
    //     /// resubscribing Offers/Demands.
    //     pub async fn run_for_offers(
    //         &self,
    //         demand: Proposal,
    //         offer: Proposal,
    //     ) -> Result<NegotiationResult, FrameworkError> {
    //         let result = self.run_negotiation_phase(demand, offer).await?;
    //         self.run_agreement_phase(result).await
    //     }
    //
    //     pub async fn run_finalize_agreement(
    //         &self,
    //         agreement: &AgreementView,
    //         result: AgreementResult,
    //     ) -> anyhow::Result<()> {
    //         // First call both functions and resolve errors later. We don't want
    //         // to omit any of these calls.
    //         let prov_result = self
    //             .requestor
    //             .agreement_finalized(&agreement.id, result.clone())
    //             .await;
    //         let req_result = self
    //             .provider
    //             .agreement_finalized(&agreement.id, result)
    //             .await;
    //
    //         prov_result?;
    //         req_result?;
    //         Ok(())
    //     }
    //
    //     pub async fn run_negotiation_phase(
    //         &self,
    //         demand: Proposal,
    //         offer: Proposal,
    //     ) -> Result<NegotiationResult, FrameworkError> {
    //         let mut result = NegotiationResult {
    //             proposals: vec![demand.clone(), offer.clone()],
    //             stage: NegotiationStage::Initial,
    //             agreement: None,
    //         };
    //
    //         let mut prev_requestor_proposal = demand;
    //         let mut prev_provider_proposal = offer;
    //
    //         let max_negotiation_steps = 20;
    //
    //         for _ in 0..max_negotiation_steps {
    //             let response = self
    //                 .requestor
    //                 .react_to_proposal(&prev_provider_proposal, &prev_requestor_proposal)
    //                 .await
    //                 .map_err(|e| FrameworkError::from(e, &result))?;
    //             let req_proposal = match &response {
    //                 // Move to Agreement phase. Accepting on Requestor side doesn't generate new Proposal.
    //                 // Agreement will be proposed for last Provider's Proposal.
    //                 ProposalAction::AcceptProposal => {
    //                     result.stage = NegotiationStage::Proposal {
    //                         last_response: response,
    //                     };
    //                     return Ok(result);
    //                 }
    //                 ProposalAction::CounterProposal { proposal: offer } => {
    //                     let proposal = self.requestor.into_proposal(offer.clone(), State::Draft);
    //
    //                     result.proposals.push(proposal.clone());
    //                     result.stage = NegotiationStage::Proposal {
    //                         last_response: response,
    //                     };
    //                     proposal
    //                 }
    //                 ProposalAction::IgnoreProposal | ProposalAction::RejectProposal { .. } => {
    //                     result.stage = NegotiationStage::Proposal {
    //                         last_response: response,
    //                     };
    //                     return Ok(result);
    //                 }
    //             };
    //
    //             // After Requestor counters Provider's Proposal for the first time,
    //             // it is no longer in Initial state.
    //             prev_provider_proposal.state = State::Draft;
    //
    //             let response = self
    //                 .provider
    //                 .react_to_proposal(&req_proposal, &prev_provider_proposal)
    //                 .await
    //                 .map_err(|e| FrameworkError::from(e, &result))?;
    //             let prov_proposal = match &response {
    //                 ProposalAction::CounterProposal { proposal: offer } => {
    //                     let proposal = self.provider.into_proposal(offer.clone(), State::Draft);
    //
    //                     result.proposals.push(proposal.clone());
    //                     result.stage = NegotiationStage::Proposal {
    //                         last_response: response,
    //                     };
    //                     proposal
    //                 }
    //                 ProposalAction::AcceptProposal => {
    //                     result.proposals.push(prev_provider_proposal.clone());
    //                     result.stage = NegotiationStage::Proposal {
    //                         last_response: response,
    //                     };
    //                     prev_provider_proposal.clone()
    //                 }
    //                 ProposalAction::RejectProposal { .. } | ProposalAction::IgnoreProposal => {
    //                     result.stage = NegotiationStage::Proposal {
    //                         last_response: response,
    //                     };
    //                     return Ok(result);
    //                 }
    //             };
    //
    //             prev_requestor_proposal = req_proposal;
    //             prev_provider_proposal = prov_proposal;
    //         }
    //
    //         return Err(FrameworkError::from(anyhow!(
    //             "Exceeded negotiation loops limit ({}). Probably your negotiators have wrong stop conditions.",
    //             max_negotiation_steps
    //         ), &result));
    //     }
    //
    //     pub async fn run_agreement_phase(
    //         &self,
    //         mut result: NegotiationResult,
    //     ) -> Result<NegotiationResult, FrameworkError> {
    //         match &result.stage {
    //             NegotiationStage::Proposal { last_response } => match last_response {
    //                 // This is the only correct state in which we should try to propose Agreement.
    //                 // Otherwise we can just return from this function.
    //                 ProposalAction::AcceptProposal => (),
    //                 _ => return Ok(result),
    //             },
    //             _ => return Ok(result),
    //         };
    //
    //         let agreement_view = AgreementView::try_from(&self.requestor.create_agreement(
    //             &result.proposals[result.proposals.len() - 2],
    //             &result.proposals[result.proposals.len() - 1],
    //         ))
    //         .map_err(|e| FrameworkError::from(e, &result))?;
    //
    //         result.agreement = Some(agreement_view.clone());
    //
    //         let response = self
    //             .requestor
    //             .react_to_agreement(&agreement_view)
    //             .await
    //             .map_err(|e| FrameworkError::from(e, &result))?;
    //         result.stage = NegotiationStage::Agreement {
    //             last_response: response.clone(),
    //         };
    //
    //         if let AgreementAction::RejectAgreement { .. } = &response {
    //             return Ok(result);
    //         }
    //
    //         let response = self
    //             .provider
    //             .react_to_agreement(&agreement_view)
    //             .await
    //             .map_err(|e| FrameworkError::from(e, &result))?;
    //         result.stage = NegotiationStage::Agreement {
    //             last_response: response.clone(),
    //         };
    //
    //         if let AgreementAction::RejectAgreement { .. } = &response {
    //             self.requestor
    //                 .agreement_finalized(&agreement_view.id, AgreementResult::ApprovalFailed)
    //                 .await
    //                 .map_err(|e| FrameworkError::from(e, &result))?;
    //             return Ok(result);
    //         }
    //
    //         // Note: Provider will never get AgreementResult::ApprovalFailed, because it can happen only,
    //         // if `approve_agreement` call fails.
    //
    //         Ok(result)
    //     }
    // }
    //
    // async fn requestor_proposals_processor(
    //     providers: HashMap<NodeId, Arc<Node>>,
    //     requestors: HashMap<NodeId, Arc<Node>>,
    //     record: Arc<Mutex<NegotiationRecord>>,
    // ) {
    //     let r_receivers = requestors
    //         .iter()
    //         .map(|(_id, node)| BroadcastStream::new(node.proposal_channel()))
    //         .collect();
    //
    //     while Some(action) = select_all(r_receivers).await {
    //         match action {
    //             ProposalAction::AcceptProposal { id } => {}
    //             ProposalAction::CounterProposal { id, proposal } => {}
    //             ProposalAction::RejectProposal { id, reason } => {}
    //         }
    //     }
}

impl FrameworkError {
    pub fn from(error: impl Into<anyhow::Error>, result: &NegotiationRecord) -> FrameworkError {
        FrameworkError {
            error: error.into(),
            negotiation_traceback: result.clone(),
        }
    }
}

// impl NegotiationResult {
//     pub fn new() -> NegotiationResult {
//         NegotiationResult {
//             stage: NegotiationStage::Initial,
//             proposals: vec![],
//             agreement: None,
//         }
//     }
// }

impl fmt::Debug for FrameworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}
