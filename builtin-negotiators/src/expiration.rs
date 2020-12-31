use anyhow::Result;
use chrono::{DateTime, Duration, TimeZone, Utc};
use serde::{Deserialize, Serialize};

use ya_agreement_utils::OfferDefinition;
use ya_client_model::market::Reason;
use ya_negotiator_component::component::{
    AgreementResult, NegotiationResult, NegotiatorComponent, ProposalView,
};

/// Negotiator that can limit number of running agreements.
pub struct LimitExpiration {
    min_expiration: Duration,
    max_expiration: Duration,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Config {
    #[serde(with = "humantime_serde")]
    pub min_agreement_expiration: std::time::Duration,
    #[serde(with = "humantime_serde")]
    pub max_agreement_expiration: std::time::Duration,
}

impl LimitExpiration {
    pub fn new(config: serde_yaml::Value) -> anyhow::Result<LimitExpiration> {
        let config: Config = serde_yaml::from_value(config)?;
        Ok(LimitExpiration {
            min_expiration: chrono::Duration::from_std(config.min_agreement_expiration)?,
            max_expiration: chrono::Duration::from_std(config.max_agreement_expiration)?,
        })
    }
}

fn proposal_expiration_from(proposal: &ProposalView) -> Result<DateTime<Utc>> {
    let expiration_key_str = "/golem/srv/comp/expiration";
    let value = proposal
        .pointer(expiration_key_str)
        .ok_or_else(|| anyhow::anyhow!("Missing expiration key in Proposal"))?
        .clone();
    let timestamp: i64 = serde_json::from_value(value)?;
    Ok(Utc.timestamp_millis(timestamp))
}

impl NegotiatorComponent for LimitExpiration {
    fn negotiate_step(&mut self, demand: &ProposalView, offer: ProposalView) -> NegotiationResult {
        let min_expiration = Utc::now() + self.min_expiration;
        let max_expiration = Utc::now() + self.max_expiration;

        let expiration = match proposal_expiration_from(&demand) {
            Ok(expiration) => expiration,
            Err(e) => {
                return NegotiationResult::Reject {
                    reason: Some(Reason::new(e)),
                }
            }
        };

        if expiration > max_expiration || expiration < min_expiration {
            log::info!(
                "Negotiator: Reject proposal [{}] due to expiration limits.",
                demand.id
            );
            NegotiationResult::Reject {
                reason: Some(Reason::new(format!(
                    "Proposal expires at: {} which is less than 5 min or more than 30 min from now",
                    expiration
                ))),
            }
        } else {
            NegotiationResult::Ready { offer }
        }
    }

    fn fill_template(
        &mut self,
        offer_template: OfferDefinition,
    ) -> anyhow::Result<OfferDefinition> {
        Ok(offer_template)
    }

    fn on_agreement_terminated(
        &mut self,
        _agreement_id: &str,
        _result: &AgreementResult,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_agreement_approved(&mut self, _agreement_id: &str) -> anyhow::Result<()> {
        Ok(())
    }
}
