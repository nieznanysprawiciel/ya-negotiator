use serde::{Deserialize, Serialize};

use ya_negotiator_shared_lib_interface::plugin::{
    transparent_impl, NegotiationResult, NegotiatorComponent, NegotiatorConstructor, ProposalView,
    Reason, Score,
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
    fn new(_name: &str, config: serde_yaml::Value) -> anyhow::Result<FilterNodes> {
        let config: FilterNodesConfig = serde_yaml::from_value(config)?;
        Ok(FilterNodes {
            names: config.names,
        })
    }
}

impl NegotiatorComponent for FilterNodes {
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
                        reason: Some(Reason::new("Node on rejection list.")),
                    }
                } else {
                    NegotiationResult::Ready {
                        proposal: offer,
                        score,
                    }
                }
            }
            Err(_) => NegotiationResult::Reject {
                reason: Some(Reason::new("Unnamed Node")),
            },
        })
    }

    transparent_impl!(on_proposal_rejected);
    transparent_impl!(on_post_terminate_event);
    transparent_impl!(fill_template);
    transparent_impl!(on_agreement_terminated);
    transparent_impl!(on_agreement_approved);
}

register_negotiators!(FilterNodes);
