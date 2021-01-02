use actix::{Actor, Context, Handler};
use anyhow::anyhow;
use serde_json::Value;

use ya_client_model::market::{NewOffer, Reason};

use crate::component::{NegotiationResult, NegotiatorComponent, ProposalView};
use crate::negotiators::{AgreementFinalized, CreateOffer, ReactToAgreement, ReactToProposal};
use crate::negotiators::{AgreementResponse, Negotiator, ProposalResponse};
use crate::NegotiatorsPack;

use ya_agreement_utils::agreement::expand;
use ya_agreement_utils::AgreementView;

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
        let offer_template = self
            .components
            .fill_template(msg.offer_definition.into_template())?;
        Ok(NewOffer::new(
            offer_template.properties,
            offer_template.constraints,
        ))
    }
}

impl Handler<ReactToProposal> for CompositeNegotiator {
    type Result = anyhow::Result<ProposalResponse>;

    fn handle(&mut self, msg: ReactToProposal, _: &mut Context<Self>) -> Self::Result {
        // In current implementation we don't allow to change constraints, so we take
        // them from initial Offer.
        let constraints = msg.offer.constraints;
        let proposal = ProposalView {
            id: msg.demand.proposal_id,
            json: expand(msg.demand.properties),
        };

        let offer = ProposalView {
            json: expand(msg.offer.properties),
            id: msg.offer_id,
        };

        let result = self.components.negotiate_step(&proposal, offer);
        match result {
            NegotiationResult::Reject { reason } => Ok(ProposalResponse::RejectProposal { reason }),
            NegotiationResult::Ready { offer } | NegotiationResult::Negotiating { offer } => {
                let offer = NewOffer {
                    properties: offer.json,
                    constraints,
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
        json: offer_proposal,
        id: offer_id,
    };

    let demand_proposal = ProposalView {
        json: demand_proposal,
        id: demand_id,
    };
    Ok((demand_proposal, offer_proposal))
}

impl Handler<ReactToAgreement> for CompositeNegotiator {
    type Result = anyhow::Result<AgreementResponse>;

    fn handle(&mut self, msg: ReactToAgreement, _: &mut Context<Self>) -> Self::Result {
        let (demand_proposal, offer_proposal) = to_proposal_views(msg.agreement).map_err(|e| {
            anyhow!(
                "Negotiator failed to extract Proposals from Agreement. {}",
                e
            )
        })?;

        // We expect that all `NegotiatorComponents` should return ready state.
        // Otherwise we must reject Agreement proposals, because negotiations didn't end.
        match self
            .components
            .negotiate_step(&demand_proposal, offer_proposal)
        {
            NegotiationResult::Ready { .. } => Ok(AgreementResponse::ApproveAgreement),
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
