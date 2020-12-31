use anyhow::bail;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::negotiators::NegotiatorAddr;
use crate::CompositeNegotiator;

use ya_negotiator_component::component::NegotiatorComponent;
use ya_negotiator_component::NegotiatorsPack;

use crate::builtin::LimitExpiration;
use crate::builtin::MaxAgreements;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[non_exhaustive]
pub enum LoadMode {
    BuiltIn,
    SharedLibrary,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct NegotiatorConfig {
    pub name: String,
    pub load_mode: LoadMode,
    pub config: serde_yaml::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NegotiatorsConfig {
    pub negotiators: Vec<NegotiatorConfig>,
}

pub fn create_negotiator(config: NegotiatorsConfig) -> anyhow::Result<Arc<NegotiatorAddr>> {
    let mut components = NegotiatorsPack::new();
    for config in config.negotiators.into_iter() {
        let name = config.name;
        let negotiator = match config.load_mode {
            LoadMode::BuiltIn => create_builtin(&name, config.config)?,
            _ => bail!("Negotiator LoadMode::{:?} not supported."),
        };

        components = components.add_component(&name, negotiator);
    }

    Ok(Arc::new(NegotiatorAddr::from(CompositeNegotiator::new(
        components,
    ))))
}

pub fn create_builtin(
    name: &str,
    config: serde_yaml::Value,
) -> anyhow::Result<Box<dyn NegotiatorComponent>> {
    let negotiator = match &name[..] {
        "LimitAgreements" => Box::new(MaxAgreements::new(config)?) as Box<dyn NegotiatorComponent>,
        "LimitExpiration" => {
            Box::new(LimitExpiration::new(config)?) as Box<dyn NegotiatorComponent>
        }
        _ => bail!("BuiltIn negotiator {} doesn't exists.", &name),
    };
    Ok(negotiator)
}
