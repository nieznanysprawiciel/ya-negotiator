use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use ya_grpc_negotiator_api::plugin::{
    NegotiationResult, NegotiatorComponentMut, NegotiatorFactory, NegotiatorMut, ProposalView,
    RejectReason, Score,
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
        Ok(match their.pointer_typed("/golem/node/id/name") {
            Ok(node_name) => {
                if self.names.contains(&node_name) {
                    NegotiationResult::Reject {
                        reason: RejectReason::new("Node on rejection list."),
                        is_final: true,
                    }
                } else {
                    NegotiationResult::Ready {
                        proposal: template,
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
