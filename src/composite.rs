use actix::{Actor, Context, Handler};
use anyhow::anyhow;
use serde_json::Value;

use ya_client_model::market::{NewOffer, Reason};

use crate::component::{NegotiationResult, NegotiatorComponent, ProposalView};
use crate::negotiators::{AgreementFinalized, CreateOffer, ReactToAgreement, ReactToProposal};
use crate::negotiators::{AgreementResponse, Negotiator, ProposalResponse};
use crate::NegotiatorsPack;

use std::convert::TryFrom;
use ya_agreement_utils::agreement::{expand, flatten};
use ya_agreement_utils::{AgreementView, OfferTemplate};

/// Negotiator that can limit number of running agreements.
pub struct CompositeNegotiator {
    components: NegotiatorsPack,
}

impl CompositeNegotiator {
    pub fn new(components: NegotiatorsPack) -> CompositeNegotiator {
        CompositeNegotiator { components }
    }
}

impl Handler<CreateOffer> for CompositeNegotiator {
    type Result = anyhow::Result<NewOffer>;

    fn handle(&mut self, msg: CreateOffer, _: &mut Context<Self>) -> Self::Result {
        let offer_template = self.components.fill_template(msg.offer_template)?;
        Ok(NewOffer::new(
            offer_template.properties,
            offer_template.constraints,
        ))
    }
}

impl Handler<ReactToProposal> for CompositeNegotiator {
    type Result = anyhow::Result<ProposalResponse>;

    fn handle(&mut self, msg: ReactToProposal, _: &mut Context<Self>) -> Self::Result {
        let proposal = ProposalView::try_from(&msg.incoming_proposal)?;
        let template = ProposalView {
            content: OfferTemplate {
                properties: expand(msg.our_prev_proposal.properties),
                constraints: msg.our_prev_proposal.constraints,
            },
            id: msg.our_prev_proposal.proposal_id,
            issuer: msg.our_prev_proposal.issuer_id,
        };

        let result = self.components.negotiate_step(&proposal, template)?;
        match result {
            NegotiationResult::Reject { reason } => Ok(ProposalResponse::RejectProposal { reason }),
            NegotiationResult::Ready { .. } => Ok(ProposalResponse::AcceptProposal),
            NegotiationResult::Negotiating { proposal: template } => {
                let offer = NewOffer {
                    properties: serde_json::Value::Object(flatten(template.content.properties)),
                    constraints: template.content.constraints,
                };
                Ok(ProposalResponse::CounterProposal { offer })
            }
        }
    }
}

pub fn to_proposal_views(
    mut agreement: AgreementView,
) -> anyhow::Result<(ProposalView, ProposalView)> {
    // Dispatch Agreement into separate Demand-Offer Proposal pair.
    let offer_id = agreement.pointer_typed("/offer/offerId")?;
    let demand_id = agreement.pointer_typed("/demand/demandId")?;
    let offer_proposal = agreement
        .json
        .pointer_mut("/offer/properties")
        .map(Value::take)
        .unwrap_or(Value::Null);

    let demand_proposal = agreement
        .json
        .pointer_mut("/demand/properties")
        .map(Value::take)
        .unwrap_or(Value::Null);

    let offer_proposal = ProposalView {
        content: OfferTemplate {
            properties: offer_proposal,
            constraints: agreement.pointer_typed("/offer/constraints")?,
        },
        id: offer_id,
        issuer: agreement.pointer_typed("/offer/providerId")?,
    };

    let demand_proposal = ProposalView {
        content: OfferTemplate {
            properties: demand_proposal,
            constraints: agreement.pointer_typed("/demand/constraints")?,
        },
        id: demand_id,
        issuer: agreement.pointer_typed("/demand/requestorId")?,
    };
    Ok((demand_proposal, offer_proposal))
}

impl Handler<ReactToAgreement> for CompositeNegotiator {
    type Result = anyhow::Result<AgreementResponse>;

    fn handle(&mut self, msg: ReactToAgreement, _: &mut Context<Self>) -> Self::Result {
        let (demand_proposal, offer_proposal) =
            to_proposal_views(msg.agreement.clone()).map_err(|e| {
                anyhow!(
                    "Negotiator failed to extract Proposals from Agreement. {}",
                    e
                )
            })?;

        // We expect that all `NegotiatorComponents` should return ready state.
        // Otherwise we must reject Agreement proposals, because negotiations didn't end.
        match self
            .components
            .negotiate_step(&demand_proposal, offer_proposal)?
        {
            NegotiationResult::Ready { .. } => {
                self.components.on_agreement_approved(&msg.agreement).ok();
                Ok(AgreementResponse::ApproveAgreement)
            }
            NegotiationResult::Reject { reason } => {
                Ok(AgreementResponse::RejectAgreement { reason })
            }
            NegotiationResult::Negotiating { .. } => Ok(AgreementResponse::RejectAgreement {
                reason: Some(Reason::new("Negotiations aren't finished.")),
            }),
        }
    }
}

impl Handler<AgreementFinalized> for CompositeNegotiator {
    type Result = anyhow::Result<()>;

    fn handle(&mut self, msg: AgreementFinalized, _: &mut Context<Self>) -> Self::Result {
        self.components
            .on_agreement_terminated(&msg.agreement_id, &msg.result)
    }
}

impl Negotiator for CompositeNegotiator {}
impl Actor for CompositeNegotiator {
    type Context = Context<Self>;
}
