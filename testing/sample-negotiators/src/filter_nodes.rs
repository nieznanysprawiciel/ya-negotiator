use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use ya_negotiator_component::static_lib::{NegotiatorFactory, NegotiatorMut};
use ya_negotiator_component::{
    NegotiationResult, NegotiatorComponentMut, ProposalView, RejectReason, Score,
};

pub struct FilterNodes {
    names: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct FilterNodesConfig {
    pub names: Vec<String>,
}

impl NegotiatorFactory<FilterNodes> for FilterNodes {
    type Type = NegotiatorMut;

    fn new(
        _name: &str,
        config: serde_yaml::Value,
        _working_dir: PathBuf,
    ) -> anyhow::Result<FilterNodes> {
        let config: FilterNodesConfig = serde_yaml::from_value(config)?;
        Ok(FilterNodes {
            names: config.names,
        })
    }
}

impl NegotiatorComponentMut for FilterNodes {
    fn negotiate_step(
        &mut self,
        their: &ProposalView,
        template: ProposalView,
        score: Score,
    ) -> anyhow::Result<NegotiationResult> {
        log::debug!("FilterNodes: `negotiate_step` for proposal: {}", their.id);

        Ok(match their.pointer_typed("/golem/node/id/name") {
            Ok(node_name) => {
                if self.names.contains(&node_name) {
                    log::debug!("FilterNodes: decided to reject proposal: {}", their.id);
                    NegotiationResult::Reject {
                        reason: RejectReason::new("Node on rejection list."),
                        is_final: true,
                    }
                } else {
                    log::debug!(
                        "FilterNodes: decided to pass through proposal: {}",
                        their.id
                    );
                    NegotiationResult::Ready {
                        proposal: template,
                        score,
                    }
                }
            }
            Err(_) => {
                log::debug!("FilterNodes: rejecting incorrect proposal: {}", their.id);

                NegotiationResult::Reject {
                    reason: RejectReason::new("Unnamed Node"),
                    is_final: true,
                }
            }
        })
    }
}
