use serde::{Deserialize, Serialize};

use ya_negotiator_shared_lib_interface::plugin::{
    AgreementResult, AgreementView, NegotiationResult, NegotiatorComponent, NegotiatorConstructor,
    OfferTemplate, ProposalView, Reason, Score,
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

    fn fill_template(&mut self, offer_template: OfferTemplate) -> anyhow::Result<OfferTemplate> {
        Ok(offer_template)
    }

    fn on_agreement_terminated(
        &mut self,
        _agreement_id: &str,
        _result: &AgreementResult,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_agreement_approved(&mut self, _agreement: &AgreementView) -> anyhow::Result<()> {
        Ok(())
    }
}

register_negotiators!(FilterNodes);
