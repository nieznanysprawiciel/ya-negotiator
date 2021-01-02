use anyhow::bail;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::negotiators::NegotiatorAddr;
use crate::CompositeNegotiator;

use ya_negotiator_shared_lib_interface::SharedLibNegotiator;

use ya_negotiator_component::component::NegotiatorComponent;
use ya_negotiator_component::NegotiatorsPack;

use crate::builtin::LimitExpiration;
use crate::builtin::MaxAgreements;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[non_exhaustive]
pub enum LoadMode {
    BuiltIn,
    SharedLibrary { path: PathBuf },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NegotiatorConfig {
    pub name: String,
    pub load_mode: LoadMode,
    pub params: serde_yaml::Value,
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
            LoadMode::BuiltIn => create_builtin(&name, config.params)?,
            LoadMode::SharedLibrary { path } => create_shared_lib(&path, &name, config.params)?,
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

pub fn create_shared_lib(
    path: &Path,
    name: &str,
    config: serde_yaml::Value,
) -> anyhow::Result<Box<dyn NegotiatorComponent>> {
    SharedLibNegotiator::new(path, name, config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ya_builtin_negotiators::*;

    #[actix_rt::test]
    async fn test_negotiators_config() {
        let expiration_conf = NegotiatorConfig {
            name: "LimitExpiration".to_string(),
            load_mode: LoadMode::BuiltIn,
            params: serde_yaml::to_value(expiration::Config {
                min_expiration: std::time::Duration::from_secs(2),
                max_expiration: std::time::Duration::from_secs(300),
            })
            .unwrap(),
        };

        let limit_conf = NegotiatorConfig {
            name: "LimitAgreements".to_string(),
            load_mode: LoadMode::BuiltIn,
            params: serde_yaml::to_value(max_agreements::Config { max_agreements: 1 }).unwrap(),
        };

        let config = NegotiatorsConfig {
            negotiators: vec![expiration_conf, limit_conf],
        };

        let serialized = serde_yaml::to_string(&config).unwrap();
        println!("{}", serialized);

        create_negotiator(serde_yaml::from_str(&serialized).unwrap()).unwrap();
    }
}
