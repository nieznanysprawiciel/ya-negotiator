use anyhow::anyhow;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use ya_agreement_utils::{AgreementView, OfferTemplate, ProposalView};

use crate::component::{
    AgreementEvent, AgreementResult, NegotiationResult, NegotiatorComponent, Score,
};

/// Processes multiple negotiators.
pub struct NegotiatorsPack {
    components: Arc<RwLock<HashMap<String, Box<dyn NegotiatorComponent>>>>,
}

impl NegotiatorsPack {
    pub fn new() -> NegotiatorsPack {
        NegotiatorsPack {
            components: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_component(
        self,
        name: &str,
        component: Box<dyn NegotiatorComponent>,
    ) -> NegotiatorsPack {
        self.components
            .write()
            .await
            .insert(name.to_string(), component);
        self
    }
}

#[async_trait(?Send)]
impl NegotiatorComponent for NegotiatorsPack {
    async fn negotiate_step(
        &self,
        incoming_proposal: &ProposalView,
        mut template: ProposalView,
        mut score: Score,
    ) -> anyhow::Result<NegotiationResult> {
        let mut all_ready = true;
        for (name, component) in self.components.read().await.iter() {
            let result = component
                .negotiate_step(incoming_proposal, template, score)
                .await?;
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
                NegotiationResult::Reject { reason, is_final } => {
                    return Ok(NegotiationResult::Reject { reason, is_final })
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

    async fn fill_template(
        &self,
        mut offer_template: OfferTemplate,
    ) -> anyhow::Result<OfferTemplate> {
        for (name, component) in self.components.read().await.iter() {
            offer_template = component.fill_template(offer_template).await.map_err(|e| {
                anyhow!("Negotiator component '{name}' failed filling Offer template. {e}")
            })?;
        }
        Ok(offer_template)
    }

    async fn on_agreement_terminated(
        &self,
        agreement_id: &str,
        result: &AgreementResult,
    ) -> anyhow::Result<()> {
        for (name, component) in self.components.read().await.iter() {
            component
                .on_agreement_terminated(agreement_id, result).await
                .map_err(|e| {
                    log::warn!(
                        "Negotiator component '{name}' failed handling Agreement [{agreement_id}] termination. {e}"
                    )
                })
                .ok();
        }
        Ok(())
    }

    async fn on_agreement_approved(&self, agreement: &AgreementView) -> anyhow::Result<()> {
        for (name, component) in self.components.read().await.iter() {
            component
                .on_agreement_approved(agreement).await
                .map_err(|e| {
                    log::warn!(
                        "Negotiator component '{name}' failed handling Agreement [{}] approval. {e}",
                        agreement.id,
                    )
                })
                .ok();
        }
        Ok(())
    }

    async fn on_proposal_rejected(&self, proposal_id: &str) -> anyhow::Result<()> {
        for (name, component) in self.components.read().await.iter() {
            component
                .on_proposal_rejected(proposal_id).await
                .map_err(|e| {
                    log::warn!(
                        "Negotiator component '{name}' failed handling Proposal [{proposal_id}] rejection. {e}",
                    )
                })
                .ok();
        }
        Ok(())
    }

    async fn on_agreement_event(
        &self,
        agreement_id: &str,
        event: &AgreementEvent,
    ) -> anyhow::Result<()> {
        for (name, component) in self.components.read().await.iter() {
            component
                .on_agreement_event(agreement_id, event).await
                .map_err(|e| {
                    log::warn!(
                        "Negotiator component '{name}' failed handling post Terminate event [{agreement_id}]. {e}",
                    )
                })
                .ok();
        }
        Ok(())
    }

    async fn control_event(
        &self,
        component: &str,
        params: Value,
    ) -> anyhow::Result<serde_json::Value> {
        match self.components.read().await.get(component) {
            None => Ok(serde_json::Value::Null),
            Some(negotiator) => negotiator.control_event(component, params).await,
        }
    }
}
