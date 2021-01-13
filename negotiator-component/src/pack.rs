use anyhow::anyhow;
use std::collections::HashMap;

use ya_agreement_utils::{AgreementView, OfferTemplate, ProposalView};

use crate::component::{AgreementResult, NegotiationResult, NegotiatorComponent};

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
    ) -> anyhow::Result<NegotiationResult> {
        let mut all_ready = true;
        for (name, component) in &mut self.components {
            let result = component.negotiate_step(incoming_proposal, template)?;
            template = match result {
                NegotiationResult::Ready { proposal: offer } => offer,
                NegotiationResult::Negotiating { proposal: offer } => {
                    log::info!(
                        "Negotiator component '{}' is still negotiating Proposal [{}].",
                        name,
                        incoming_proposal.id
                    );
                    all_ready = false;
                    offer
                }
                NegotiationResult::Reject { reason } => {
                    return Ok(NegotiationResult::Reject { reason })
                }
            }
        }

        // Full negotiations is ready only, if all `NegotiatorComponent` returned
        // ready state. Otherwise we must still continue negotiations.
        Ok(match all_ready {
            true => NegotiationResult::Ready { proposal: template },
            false => NegotiationResult::Negotiating { proposal: template },
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
}
