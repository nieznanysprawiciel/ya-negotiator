use anyhow::bail;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use ya_client_model::market::Reason;

use ya_agreement_utils::{OfferTemplate, ProposalView};
use ya_negotiator_component::component::{AgreementResult, NegotiationResult, NegotiatorComponent};

/// Negotiator that can limit number of running agreements.
pub struct MaxAgreements {
    active_agreements: HashSet<String>,
    max_agreements: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub max_agreements: u32,
}

impl MaxAgreements {
    pub fn new(config: serde_yaml::Value) -> anyhow::Result<MaxAgreements> {
        let config: Config = serde_yaml::from_value(config)?;
        Ok(MaxAgreements {
            max_agreements: config.max_agreements,
            active_agreements: HashSet::new(),
        })
    }

    pub fn has_free_slot(&self) -> bool {
        self.active_agreements.len() < self.max_agreements as usize
    }
}

impl NegotiatorComponent for MaxAgreements {
    fn negotiate_step(
        &mut self,
        demand: &ProposalView,
        offer: ProposalView,
    ) -> anyhow::Result<NegotiationResult> {
        let result = if self.has_free_slot() {
            NegotiationResult::Ready { offer }
        } else {
            log::info!(
                "'MaxAgreements' negotiator: Reject proposal [{}] due to limit.",
                demand.id, // TODO: Should be just `id`, but I reuse AgreementView struct.
            );
            NegotiationResult::Reject {
                reason: Some(Reason::new(format!(
                    "No capacity available. Reached Agreements limit: {}",
                    self.max_agreements
                ))),
            }
        };
        Ok(result)
    }

    fn fill_template(&mut self, offer_template: OfferTemplate) -> anyhow::Result<OfferTemplate> {
        Ok(offer_template)
    }

    fn on_agreement_terminated(
        &mut self,
        agreement_id: &str,
        _result: &AgreementResult,
    ) -> anyhow::Result<()> {
        self.active_agreements.remove(agreement_id);

        let free_slots = self.max_agreements as usize - self.active_agreements.len();
        log::info!("Negotiator: {} free slot(s) for agreements.", free_slots);
        Ok(())
    }

    fn on_agreement_approved(&mut self, agreement_id: &str) -> anyhow::Result<()> {
        if self.has_free_slot() {
            self.active_agreements.insert(agreement_id.to_string());
            Ok(())
        } else {
            self.active_agreements.insert(agreement_id.to_string());
            bail!(
                "Agreement [{}] approved despite not available capacity.",
                agreement_id
            )
        }
    }
}
