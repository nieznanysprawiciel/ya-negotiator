use actix::{Actor, Context, Handler, StreamHandler};
use anyhow::anyhow;
use serde_json::Value;
use std::convert::TryFrom;
use std::time::Duration;
use tokio::sync::mpsc;

use ya_client_model::market::{NewOffer, NewProposal, Reason};

use crate::component::{NegotiationResult, NegotiatorComponent, ProposalView, Score};
use crate::negotiators::{
    Action, AgreementAction, AgreementSigned, ControlEvent, PostAgreementEvent, ProposalRejected,
};
use crate::negotiators::{AgreementFinalized, CreateOffer, ReactToAgreement, ReactToProposal};
use crate::{NegotiatorsPack, ProposalsCollection};

use crate::collection::{Feedback, ProposalScore};
use ya_agreement_utils::agreement::{expand, flatten};
use ya_agreement_utils::{AgreementView, OfferTemplate};

struct NegotiatorConfig {
    /// Time period before making decision, which Proposals to choose.
    pub collect_proposals_period: Duration,
    /// Time period before making decision, which Agreements to choose.
    pub collect_agreements_period: Duration,
}

/// Actor implementing Negotiation logic.
///
/// Direction:
/// - Negotiator should asynchronously generate negotiation decisions instead
///   of returning them as direct response to incoming events. This would allow use
///   to implement time dependent logic like: Collect Proposals during `n` seconds
///   and choose the best from them.
/// - Extensibility: we expect, that developers will implement different market strategies.
///   In best case they should be able to do this without modifying `ya-provider` code.
///   This mean we should implement plugin-like system to communicate with external applications/code.
/// - Multiple negotiating plugins cooperating with each other. Note that introducing new features to
///   Agreement specification requires implementing separate negotiation logic. In this case we
///   can end up with explosion of combinations to implement. What worse, we will force external
///   developers to adjust their logic to new Agreement features each time, when they appear.
///   To avoid this we should design internal interfaces, which will allow to combine multiple logics
///   as plugable components.
pub struct Negotiator {
    components: NegotiatorsPack,

    proposal_channel: mpsc::UnboundedSender<Action>,
    agreement_channel: mpsc::UnboundedSender<AgreementAction>,

    proposals: ProposalsCollection,
}

pub struct NegotiatorCallbacks {
    pub proposal_channel: mpsc::UnboundedReceiver<Action>,
    pub agreement_channel: mpsc::UnboundedReceiver<AgreementAction>,
}

impl Negotiator {
    pub fn new(components: NegotiatorsPack) -> (Negotiator, NegotiatorCallbacks) {
        let (proposal_sender, proposal_receiver) = mpsc::unbounded_channel();
        let (agreement_sender, agreement_receiver) = mpsc::unbounded_channel();

        let negotiator = Negotiator {
            components,
            proposal_channel: proposal_sender.clone(),
            agreement_channel: agreement_sender,
            proposals: ProposalsCollection::new(),
        };

        let callbacks = NegotiatorCallbacks {
            proposal_channel: proposal_receiver,
            agreement_channel: agreement_receiver,
        };

        return (negotiator, callbacks);
    }
}

impl Handler<CreateOffer> for Negotiator {
    type Result = anyhow::Result<NewOffer>;

    fn handle(&mut self, msg: CreateOffer, _: &mut Context<Self>) -> Self::Result {
        let offer_template = self.components.fill_template(msg.offer_template)?;
        Ok(NewOffer::new(
            offer_template.properties,
            offer_template.constraints,
        ))
    }
}

impl Handler<ReactToProposal> for Negotiator {
    type Result = anyhow::Result<()>;

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

        let result = self
            .components
            .negotiate_step(&proposal, template, Score::default())?;

        match result {
            NegotiationResult::Reject { reason } => {
                self.proposal_channel.send(Action::RejectProposal {
                    id: proposal.id.clone(),
                    reason,
                })?;
            }
            NegotiationResult::Ready {
                proposal: our_proposal,
                score,
            } => {
                self.proposals.new_scored(ProposalScore {
                    proposal: our_proposal,
                    prev: proposal,
                    score: score.pointer_typed("/final-score").unwrap_or(0.0),
                })?;
            }
            NegotiationResult::Negotiating {
                proposal: template, ..
            } => {
                let offer = NewProposal {
                    properties: serde_json::Value::Object(flatten(template.content.properties)),
                    constraints: template.content.constraints,
                };
                self.proposal_channel.send(Action::CounterProposal {
                    id: proposal.id.clone(),
                    proposal: offer,
                })?;
            }
        }
        Ok(())
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

impl Handler<ReactToAgreement> for Negotiator {
    type Result = anyhow::Result<()>;

    fn handle(&mut self, msg: ReactToAgreement, _: &mut Context<Self>) -> Self::Result {
        let agreement_id = msg.agreement.id.clone();
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
            .negotiate_step(&demand_proposal, offer_proposal, Score::default())?
        {
            NegotiationResult::Ready { .. } => {
                self.agreement_channel
                    .send(AgreementAction::ApproveAgreement { id: agreement_id })?;
            }
            NegotiationResult::Reject { reason } => {
                self.agreement_channel
                    .send(AgreementAction::RejectAgreement {
                        id: agreement_id,
                        reason,
                    })?;
            }
            NegotiationResult::Negotiating { .. } => {
                self.agreement_channel
                    .send(AgreementAction::RejectAgreement {
                        id: agreement_id,
                        reason: Some(Reason::new("Negotiations aren't finished.")),
                    })?;
            }
        }
        Ok(())
    }
}

impl Handler<AgreementSigned> for Negotiator {
    type Result = anyhow::Result<()>;

    fn handle(&mut self, msg: AgreementSigned, _: &mut Context<Self>) -> Self::Result {
        self.components.on_agreement_approved(&msg.agreement)
    }
}

impl Handler<AgreementFinalized> for Negotiator {
    type Result = anyhow::Result<()>;

    fn handle(&mut self, msg: AgreementFinalized, _: &mut Context<Self>) -> Self::Result {
        self.components
            .on_agreement_terminated(&msg.agreement_id, &msg.result)
    }
}

impl Handler<ProposalRejected> for Negotiator {
    type Result = anyhow::Result<()>;

    fn handle(&mut self, msg: ProposalRejected, _: &mut Context<Self>) -> Self::Result {
        // TODO: Pass reason to components.
        self.components.on_proposal_rejected(&msg.proposal_id)
    }
}

impl Handler<PostAgreementEvent> for Negotiator {
    type Result = anyhow::Result<()>;

    fn handle(&mut self, msg: PostAgreementEvent, _: &mut Context<Self>) -> Self::Result {
        self.components
            .on_post_terminate_event(&msg.agreement_id, &msg.event)
    }
}

impl Handler<ControlEvent> for Negotiator {
    type Result = anyhow::Result<serde_json::Value>;

    fn handle(&mut self, msg: ControlEvent, _: &mut Context<Self>) -> Self::Result {
        self.components.control_event(&msg.component, msg.params)
    }
}

impl StreamHandler<Feedback> for Negotiator {
    fn handle(&mut self, item: Feedback, _ctx: &mut Context<Self>) {
        match item {
            Feedback::Decide => self.proposals.decide(),
            Feedback::Accept { id } => self
                .proposal_channel
                .send(Action::AcceptProposal { id: id.clone() })
                .map_err(|_| anyhow!("Failed to send AcceptProposal for {}", id)),
            Feedback::Reject { id, reason, .. } => self
                .proposal_channel
                .send(Action::RejectProposal {
                    id: id.clone(),
                    reason,
                })
                .map_err(|_| anyhow!("Failed to send RejectProposal for {}", id)),
        };
    }
}

impl Actor for Negotiator {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        let channel = self
            .proposals
            .feedback_receiver
            .take()
            .expect("ProposalsCollection has stolen Receiver on initialization.");
        Self::add_stream(channel, ctx);
    }
}
