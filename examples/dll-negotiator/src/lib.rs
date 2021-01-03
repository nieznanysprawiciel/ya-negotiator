use serde::{Deserialize, Serialize};
use ya_negotiator_shared_lib_interface::plugin::{
    AgreementResult, NegotiationResult, NegotiatorComponent, NegotiatorConstructor,
    NegotiatorWrapper, OfferTemplate, ProposalView, Reason,
};

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
    ) -> anyhow::Result<NegotiationResult> {
        Ok(match demand.pointer_typed("/golem/node/id/name") {
            Ok(node_name) => {
                if self.names.contains(&node_name) {
                    NegotiationResult::Reject {
                        reason: Some(Reason::new("Node on rejection list.")),
                    }
                } else {
                    NegotiationResult::Ready { offer }
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

    fn on_agreement_approved(&mut self, _agreement_id: &str) -> anyhow::Result<()> {
        Ok(())
    }
}

use abi_stable::std_types::{RResult, RResult::RErr, RStr, RString};
use abi_stable::{export_root_module, prefix_type::PrefixTypeTrait, sabi_extern_fn};
use ya_negotiator_shared_lib_interface::interface::{
    BoxedSharedNegotiatorAPI, NegotiatorLib, NegotiatorLib_Ref,
};

#[sabi_extern_fn]
pub fn create_negotiator(name: RStr, config: RStr) -> RResult<BoxedSharedNegotiatorAPI, RString> {
    match name.as_str() {
        "FilterNodes" => NegotiatorWrapper::<FilterNodes>::new(name, config),
        _ => RErr(RString::from(format!("Negotiator {} not found.", name))),
    }
}

#[export_root_module]
pub fn get_library() -> NegotiatorLib_Ref {
    NegotiatorLib { create_negotiator }.leak_into_prefix()
}
