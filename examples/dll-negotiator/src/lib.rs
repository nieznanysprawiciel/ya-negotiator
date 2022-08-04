use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::ya_negotiator_component::reason::RejectReason;
use ya_negotiator_shared_lib_interface::plugin::{
    NegotiationResult, NegotiatorComponentMut, NegotiatorConstructor, ProposalView, Score,
};
use ya_negotiator_shared_lib_interface::*;

pub struct FilterNodes {
    names: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct FilterNodesConfig {
    pub names: Vec<String>,
}

impl NegotiatorConstructor<FilterNodes> for FilterNodes {
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
        demand: &ProposalView,
        offer: ProposalView,
        score: Score,
    ) -> anyhow::Result<NegotiationResult> {
        Ok(match demand.pointer_typed("/golem/node/id/name") {
            Ok(node_name) => {
                if self.names.contains(&node_name) {
                    NegotiationResult::Reject {
                        reason: RejectReason::new("Node on rejection list."),
                        is_final: true,
                    }
                } else {
                    NegotiationResult::Ready {
                        proposal: offer,
                        score,
                    }
                }
            }
            Err(_) => NegotiationResult::Reject {
                reason: RejectReason::new("Unnamed Node"),
                is_final: true,
            },
        })
    }
}

register_negotiators!(FilterNodes);
