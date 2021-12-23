use actix::{Actor, Context, Handler, StreamHandler};
use anyhow::anyhow;
use futures::stream::select;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::time::Duration;
use tokio::sync::mpsc;

use ya_client_model::market::proposal::State;
use ya_client_model::market::{NewOffer, Reason};

use crate::component::{NegotiationResult, NegotiatorComponent, ProposalView, Score};
use crate::negotiators::{
    AgreementAction, AgreementSigned, ControlEvent, PostAgreementEvent, ProposalAction,
    ProposalRejected, RequestAgreements,
};
use crate::negotiators::{AgreementFinalized, CreateOffer, ReactToAgreement, ReactToProposal};
use crate::{NegotiatorsPack, ProposalsCollection};

use crate::collection::{
    CollectionConfig, CollectionType, DecideGoal, DecideReason, Feedback, FeedbackAction,
    ProposalScore,
};

use ya_agreement_utils::agreement::expand;
use ya_agreement_utils::{AgreementView, OfferTemplate};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositeNegotiatorConfig {
    pub proposals: CollectionConfig,
    pub agreements: CollectionConfig,
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

    proposal_channel: mpsc::UnboundedSender<ProposalAction>,
    agreement_channel: mpsc::UnboundedSender<AgreementAction>,

    proposals: ProposalsCollection,
    agreements: ProposalsCollection,

    /// Mapping between Proposal Ids and Agreements.
    proposal_agreement: HashMap<String, String>,
}

pub struct NegotiatorCallbacks {
    pub proposal_channel: mpsc::UnboundedReceiver<ProposalAction>,
    pub agreement_channel: mpsc::UnboundedReceiver<AgreementAction>,
}

impl Negotiator {
    pub fn new(
        components: NegotiatorsPack,
        config: CompositeNegotiatorConfig,
    ) -> (Negotiator, NegotiatorCallbacks) {
        let (proposal_sender, proposal_receiver) = mpsc::unbounded_channel();
        let (agreement_sender, agreement_receiver) = mpsc::unbounded_channel();

        let negotiator = Negotiator {
            components,
            proposal_channel: proposal_sender.clone(),
            agreement_channel: agreement_sender,
            proposals: ProposalsCollection::new(CollectionType::Proposal, config.proposals),
            agreements: ProposalsCollection::new(CollectionType::Agreement, config.agreements),
            proposal_agreement: Default::default(),
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
        log::debug!(
            "Reacting to Proposal [{}]",
            msg.incoming_proposal.proposal_id
        );

        let their = ProposalView::try_from(&msg.incoming_proposal)?;
        let template = ProposalView {
            content: OfferTemplate {
                properties: expand(msg.our_prev_proposal.properties),
                constraints: msg.our_prev_proposal.constraints,
            },
            id: msg.our_prev_proposal.proposal_id,
            issuer: msg.our_prev_proposal.issuer_id,
            state: msg.our_prev_proposal.state,
            timestamp: msg.our_prev_proposal.timestamp,
        };

        let result = self
            .components
            .negotiate_step(&their, template, Score::default())?;

        match result {
            NegotiationResult::Reject { reason } => {
                self.proposal_channel.send(ProposalAction::RejectProposal {
                    id: their.id.clone(),
                    reason,
                })?;
            }
            NegotiationResult::Ready {
                proposal: our,
                score,
            } => match their.state {
                State::Initial => {
                    // We must counter Initial Proposal even, if it is ready to promote to Agreement.
                    // ProposalsCollection should store only fully negotiated Proposals.
                    self.proposal_channel
                        .send(ProposalAction::CounterProposal {
                            id: their.id.clone(),
                            proposal: our.into(),
                        })?;
                }
                State::Draft => {
                    let id = their.id.clone();
                    self.proposals.new_scored(
                        ProposalScore {
                            their,
                            our,
                            score: score.pointer_typed("/final-score").unwrap_or(0.0),
                        },
                        &id,
                    )?;
                }
                _ => {
                    log::warn!("Invalid Proposal [{}] state {:?}", their.id, their.state)
                }
            },

            NegotiationResult::Negotiating { proposal: our, .. } => {
                self.proposal_channel
                    .send(ProposalAction::CounterProposal {
                        id: their.id.clone(),
                        proposal: our.into(),
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
        state: State::Accepted,
        timestamp: agreement.creation_timestamp()?,
    };

    let demand_proposal = ProposalView {
        content: OfferTemplate {
            properties: demand_proposal,
            constraints: agreement.pointer_typed("/demand/constraints")?,
        },
        id: demand_id,
        issuer: agreement.pointer_typed("/demand/requestorId")?,
        state: State::Accepted,
        timestamp: agreement.creation_timestamp()?,
    };
    Ok((demand_proposal, offer_proposal))
}

impl Handler<ReactToAgreement> for Negotiator {
    type Result = anyhow::Result<()>;

    fn handle(&mut self, msg: ReactToAgreement, _: &mut Context<Self>) -> Self::Result {
        log::debug!("Reacting to Agreement [{}]", msg.agreement.id);

        let agreement_id = msg.agreement.id.clone();
        let (their, our) = to_proposal_views(msg.agreement.clone()).map_err(|e| {
            anyhow!(
                "Negotiator failed to extract Proposals from Agreement. {}",
                e
            )
        })?;

        self.proposal_agreement
            .insert(their.id.clone(), agreement_id.clone());
        self.proposal_agreement
            .insert(our.id.clone(), agreement_id.clone());

        // We expect that all `NegotiatorComponents` should return ready state.
        // Otherwise we must reject Agreement proposals, because negotiations weren't finished.
        match self
            .components
            .negotiate_step(&their, our, Score::default())?
        {
            NegotiationResult::Ready { proposal, score } => {
                self.agreements.new_scored(
                    ProposalScore {
                        their,
                        our: proposal,
                        score: score.pointer_typed("/final-score").unwrap_or(0.0),
                    },
                    &agreement_id,
                )?;
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
            .on_agreement_event(&msg.agreement_id, &msg.event)
    }
}

impl Handler<ControlEvent> for Negotiator {
    type Result = anyhow::Result<serde_json::Value>;

    fn handle(&mut self, msg: ControlEvent, _: &mut Context<Self>) -> Self::Result {
        self.components.control_event(&msg.component, msg.params)
    }
}

impl Handler<RequestAgreements> for Negotiator {
    type Result = ();

    fn handle(&mut self, msg: RequestAgreements, _: &mut Context<Self>) -> Self::Result {
        self.agreements.set_goal(DecideGoal::Limit(msg.0))
    }
}

/// Executes actions proposed by ProposalCollections. ProposalCollection collects
/// Agreements/Proposals and decides, when we should send responses based on scores,
/// number of artifacts collected, timeouts etc.
impl StreamHandler<Feedback> for Negotiator {
    fn handle(&mut self, item: Feedback, _ctx: &mut Context<Self>) {
        match item.collection_type {
            CollectionType::Agreement => match item.action {
                FeedbackAction::Decide(reason) => {
                    match reason {
                        DecideReason::TimeElapsed => {
                            log::info!("Choosing Agreements, because collect period elapsed.")
                        }
                        DecideReason::GoalReached => log::info!(
                            "Choosing Agreements, because collected expected number of them."
                        ),
                    };
                    self.agreements.decide()
                }
                FeedbackAction::Accept { id } => {
                    let id = match self.proposal_agreement.get(&id) {
                        Some(id) => id.clone(),
                        None => {
                            log::warn!("Accepted Proposal [{}] with no matching Agreement.", id);
                            return;
                        }
                    };

                    log::info!("Accepting Agreement [{}]", id);

                    self.agreement_channel
                        .send(AgreementAction::ApproveAgreement { id: id.clone() })
                        .map_err(|_| anyhow!("Failed to send AcceptAgreement for {}", id))
                }
                FeedbackAction::Reject { id, reason, .. } => {
                    let id = match self.proposal_agreement.get(&id) {
                        Some(id) => id.clone(),
                        None => {
                            log::warn!("Rejected Proposal [{}] with no matching Agreement.", id);
                            return;
                        }
                    };

                    log::info!("Rejecting Agreement [{}]", id);

                    self.agreement_channel
                        .send(AgreementAction::RejectAgreement {
                            id: id.clone(),
                            reason,
                        })
                        .map_err(|_| anyhow!("Failed to send RejectAgreement for [{}]", id))
                }
            },
            CollectionType::Proposal => match item.action {
                FeedbackAction::Decide(reason) => {
                    match reason {
                        DecideReason::TimeElapsed => {
                            log::info!("Choosing Proposals, because collect period elapsed.")
                        }
                        DecideReason::GoalReached => log::info!(
                            "Choosing Proposals, because collected expected number of them."
                        ),
                    };
                    self.proposals.decide()
                }
                FeedbackAction::Accept { id } => {
                    log::info!("Accepting Proposal [{}]", id);

                    self.proposal_channel
                        .send(ProposalAction::AcceptProposal { id: id.clone() })
                        .map_err(|_| anyhow!("Failed to send AcceptProposal for [{}]", id))
                }
                FeedbackAction::Reject { id, reason, .. } => {
                    log::info!("Rejecting Proposal {}", id);

                    self.proposal_channel
                        .send(ProposalAction::RejectProposal {
                            id: id.clone(),
                            reason,
                        })
                        .map_err(|_| anyhow!("Failed to send RejectProposal for [{}]", id))
                }
            },
        }
        .map_err(|e| log::warn!("{}", e))
        .ok();
    }
}

impl Actor for Negotiator {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        let p_channel = self
            .proposals
            .feedback_receiver
            .take()
            .expect("Proposals collection receiver already taken on initialization.");

        let a_channel = self
            .agreements
            .feedback_receiver
            .take()
            .expect("Agreements collection receiver already taken on initialization.");
        Self::add_stream(select(p_channel, a_channel), ctx);
    }
}

impl CompositeNegotiatorConfig {
    pub fn default_provider() -> CompositeNegotiatorConfig {
        CompositeNegotiatorConfig {
            proposals: CollectionConfig {
                collect_period: Some(Duration::from_secs(5)),
                collect_amount: Some(5),
                goal: DecideGoal::Batch(10),
            },
            agreements: CollectionConfig {
                collect_period: Some(Duration::from_secs(20)),
                collect_amount: Some(5),
                goal: DecideGoal::Limit(1),
            },
        }
    }

    pub fn default_test() -> CompositeNegotiatorConfig {
        CompositeNegotiatorConfig {
            proposals: CollectionConfig {
                collect_period: Some(Duration::from_secs(5)),
                collect_amount: Some(1),
                goal: DecideGoal::Batch(10),
            },
            agreements: CollectionConfig {
                collect_period: Some(Duration::from_secs(20)),
                collect_amount: Some(1),
                goal: DecideGoal::Limit(1),
            },
        }
    }
}
