use anyhow::bail;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

use ya_agreement_utils::{AgreementView, ProposalView};
use ya_negotiator_component::reason::RejectReason;
use ya_negotiator_component::static_lib::{NegotiatorFactory, NegotiatorMut};
use ya_negotiator_component::{AgreementResult, NegotiationResult, NegotiatorComponentMut, Score};

/// Negotiator that can limit number of running agreements.
pub struct MaxAgreements {
    active_agreements: HashSet<String>,
    max_agreements: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub max_agreements: u32,
}

impl NegotiatorFactory<MaxAgreements> for MaxAgreements {
    type Type = NegotiatorMut;

    fn new(
        _name: &str,
        config: serde_yaml::Value,
        _agent_env: serde_yaml::Value,
        _working_dir: PathBuf,
    ) -> anyhow::Result<MaxAgreements> {
        let config: Config = serde_yaml::from_value(config)?;
        Ok(MaxAgreements {
            max_agreements: config.max_agreements,
            active_agreements: HashSet::new(),
        })
    }
}

impl MaxAgreements {
    pub fn has_free_slot(&self) -> bool {
        self.active_agreements.len() < self.max_agreements as usize
    }
}

impl NegotiatorComponentMut for MaxAgreements {
    fn negotiate_step(
        &mut self,
        demand: &ProposalView,
        offer: ProposalView,
        score: Score,
    ) -> anyhow::Result<NegotiationResult> {
        let result = if self.has_free_slot() {
            NegotiationResult::Ready {
                proposal: offer,
                score,
            }
        } else {
            log::info!(
                "'MaxAgreements' negotiator: Reject proposal [{}] due to limit.",
                demand.id, // TODO: Should be just `id`, but I reuse AgreementView struct.
            );
            NegotiationResult::Reject {
                reason: RejectReason::new(format!(
                    "No capacity available. Reached Agreements limit: {}",
                    self.max_agreements
                )),
                is_final: false,
            }
        };
        Ok(result)
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

    fn on_agreement_approved(&mut self, agreement: &AgreementView) -> anyhow::Result<()> {
        if self.has_free_slot() {
            self.active_agreements.insert(agreement.id.clone());
            Ok(())
        } else {
            self.active_agreements.insert(agreement.id.clone());
            bail!(
                "Agreement [{}] approved despite not available capacity.",
                agreement.id
            )
        }
    }
}
