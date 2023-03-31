use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use ya_agreement_utils::ProposalView;
use ya_negotiator_component::reason::RejectReason;
use ya_negotiator_component::static_lib::{NegotiatorFactory, NegotiatorMut};
use ya_negotiator_component::{NegotiationResult, NegotiatorComponentMut, Score};

/// Negotiator that can limit number of running agreements.
pub struct LimitExpiration {
    min_expiration: Duration,
    max_expiration: Duration,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(with = "humantime_serde")]
    pub min_expiration: std::time::Duration,
    #[serde(with = "humantime_serde")]
    pub max_expiration: std::time::Duration,
}

impl NegotiatorFactory<LimitExpiration> for LimitExpiration {
    type Type = NegotiatorMut;

    fn new(
        _name: &str,
        config: serde_yaml::Value,
        _agent_env: serde_yaml::Value,
        _working_dir: PathBuf,
    ) -> Result<LimitExpiration> {
        let config: Config = serde_yaml::from_value(config)?;
        Ok(LimitExpiration {
            min_expiration: chrono::Duration::from_std(config.min_expiration)?,
            max_expiration: chrono::Duration::from_std(config.max_expiration)?,
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
    match Utc.timestamp_millis_opt(timestamp) {
        chrono::LocalResult::Single(t) => Ok(t),
        _ => Err(anyhow!("Cannot make DateTime from timestamp {timestamp}")),
    }
}

impl NegotiatorComponentMut for LimitExpiration {
    fn negotiate_step(
        &mut self,
        demand: &ProposalView,
        offer: ProposalView,
        score: Score,
    ) -> anyhow::Result<NegotiationResult> {
        let min_expiration = Utc::now() + self.min_expiration;
        let max_expiration = Utc::now() + self.max_expiration;

        let expiration = proposal_expiration_from(&demand)?;

        let result = if expiration > max_expiration || expiration < min_expiration {
            log::info!(
                "Negotiator: Reject proposal [{}] due to expiration limits.",
                demand.id
            );
            NegotiationResult::Reject {
                reason: RejectReason::new(format!(
                    "Proposal expires at: {} which is less than {} or more than {} from now",
                    expiration, self.min_expiration, self.max_expiration
                )),
                is_final: true,
            }
        } else {
            NegotiationResult::Ready {
                proposal: offer,
                score,
            }
        };
        Ok(result)
    }
}
