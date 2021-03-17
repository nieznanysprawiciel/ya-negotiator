use anyhow::anyhow;
use serde_json::Value;
use std::collections::HashMap;

use ya_agreement_utils::{AgreementView, OfferTemplate, ProposalView};

use crate::component::{
    AgreementResult, NegotiationResult, NegotiatorComponent, PostTerminateEvent, Score,
};

pub struct NegotiatorsPack {
    components: HashMap<String, Box<dyn NegotiatorComponent>>,
}

impl NegotiatorsPack {
    pub fn new() -> NegotiatorsPack {
        NegotiatorsPack {
            components: HashMap::new(),
        }
    }

    pub fn add_component(
        mut self,
        name: &str,
        component: Box<dyn NegotiatorComponent>,
    ) -> NegotiatorsPack {
        self.components.insert(name.to_string(), component);
        self
    }
}

impl NegotiatorComponent for NegotiatorsPack {
    fn negotiate_step(
        &mut self,
        incoming_proposal: &ProposalView,
        mut template: ProposalView,
        mut score: Score,
    ) -> anyhow::Result<NegotiationResult> {
        let mut all_ready = true;
        for (name, component) in &mut self.components {
            let result = component.negotiate_step(incoming_proposal, template, score)?;
            match result {
                NegotiationResult::Ready {
                    proposal: offer,
                    score: new_score,
                } => {
                    template = offer;
                    score = new_score;
                }
                NegotiationResult::Negotiating {
                    proposal: offer,
                    score: new_score,
                } => {
                    log::info!(
                        "Negotiator component '{}' is still negotiating Proposal [{}].",
                        name,
                        incoming_proposal.id
                    );

                    all_ready = false;
                    template = offer;
                    score = new_score;
                }
                NegotiationResult::Reject { reason } => {
                    return Ok(NegotiationResult::Reject { reason })
                }
            }
        }

        // Full negotiations is ready only, if all `NegotiatorComponent` returned
        // ready state. Otherwise we must still continue negotiations.
        Ok(match all_ready {
            true => NegotiationResult::Ready {
                proposal: template,
                score,
            },
            false => NegotiationResult::Negotiating {
                proposal: template,
                score,
            },
        })
    }

    fn fill_template(
        &mut self,
        mut offer_template: OfferTemplate,
    ) -> anyhow::Result<OfferTemplate> {
        for (name, component) in &mut self.components {
            offer_template = component.fill_template(offer_template).map_err(|e| {
                anyhow!(
                    "Negotiator component '{}' failed filling Offer template. {}",
                    name,
                    e
                )
            })?;
        }
        Ok(offer_template)
    }

    fn on_agreement_terminated(
        &mut self,
        agreement_id: &str,
        result: &AgreementResult,
    ) -> anyhow::Result<()> {
        for (name, component) in &mut self.components {
            component
                .on_agreement_terminated(agreement_id, result)
                .map_err(|e| {
                    log::warn!(
                        "Negotiator component '{}' failed handling Agreement [{}] termination. {}",
                        name,
                        agreement_id,
                        e
                    )
                })
                .ok();
        }
        Ok(())
    }

    fn on_agreement_approved(&mut self, agreement: &AgreementView) -> anyhow::Result<()> {
        for (name, component) in &mut self.components {
            component
                .on_agreement_approved(agreement)
                .map_err(|e| {
                    log::warn!(
                        "Negotiator component '{}' failed handling Agreement [{}] approval. {}",
                        name,
                        agreement.id,
                        e
                    )
                })
                .ok();
        }
        Ok(())
    }

    fn on_proposal_rejected(&mut self, proposal_id: &str) -> anyhow::Result<()> {
        for (name, component) in &mut self.components {
            component
                .on_proposal_rejected(proposal_id)
                .map_err(|e| {
                    log::warn!(
                        "Negotiator component '{}' failed handling Proposal [{}] rejection. {}",
                        name,
                        proposal_id,
                        e
                    )
                })
                .ok();
        }
        Ok(())
    }

    fn on_post_terminate_event(
        &mut self,
        agreement_id: &str,
        event: &PostTerminateEvent,
    ) -> anyhow::Result<()> {
        for (name, component) in &mut self.components {
            component
                .on_post_terminate_event(agreement_id, event)
                .map_err(|e| {
                    log::warn!(
                        "Negotiator component '{}' failed handling post Terminate event [{}]. {}",
                        name,
                        agreement_id,
                        e
                    )
                })
                .ok();
        }
        Ok(())
    }

    fn control_event(
        &mut self,
        component: &str,
        params: Value,
    ) -> anyhow::Result<serde_json::Value> {
        match self.components.get_mut(component) {
            None => Ok(serde_json::Value::Null),
            Some(negotiator) => negotiator.control_event(component, params),
        }
    }
}
