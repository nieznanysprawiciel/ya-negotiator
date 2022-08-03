use anyhow::bail;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::negotiators::NegotiatorAddr;
use crate::Negotiator;

use ya_negotiator_shared_lib_interface::SharedLibNegotiator;

use ya_negotiator_component::component::NegotiatorComponent;
use ya_negotiator_component::{static_lib::create_static_negotiator, NegotiatorsPack};

use crate::builtin::AcceptAll;
use crate::builtin::LimitExpiration;
use crate::builtin::MaxAgreements;
pub use crate::composite::CompositeNegotiatorConfig;
use crate::composite::NegotiatorCallbacks;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum LoadMode {
    BuiltIn,
    SharedLibrary { path: PathBuf },
    StaticLib { library: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct NegotiatorConfig {
    pub name: String,
    pub load_mode: LoadMode,
    pub params: serde_yaml::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NegotiatorsConfig {
    pub negotiators: Vec<NegotiatorConfig>,
    pub composite: CompositeNegotiatorConfig,
}

pub fn create_negotiator(
    config: NegotiatorsConfig,
    working_dir: PathBuf,
    plugins_dir: PathBuf,
) -> anyhow::Result<(Arc<NegotiatorAddr>, NegotiatorCallbacks)> {
    let mut components = NegotiatorsPack::new();
    for config in config.negotiators.into_iter() {
        let name = config.name;
        let working_dir = working_dir.join(&name);

        log::info!("Creating negotiator: {}", name);

        fs::create_dir_all(&working_dir)?;

        let negotiator = match config.load_mode {
            LoadMode::BuiltIn => create_builtin(&name, config.params, working_dir)?,
            LoadMode::SharedLibrary { path } => {
                let plugin_path = match path.is_relative() {
                    true => plugins_dir.join(path),
                    false => path,
                };
                create_shared_lib(&plugin_path, &name, config.params, working_dir)?
            }
            LoadMode::StaticLib { library } => create_static_negotiator(
                &format!("{}::{}", &library, &name),
                config.params,
                working_dir,
            )?,
        };

        components = components.add_component(&name, negotiator);
    }

    let (negotiator, callbacks) = Negotiator::new(components, config.composite);
    Ok((Arc::new(NegotiatorAddr::from(negotiator)), callbacks))
}

pub fn create_builtin(
    name: &str,
    config: serde_yaml::Value,
    _working_dir: PathBuf,
) -> anyhow::Result<Box<dyn NegotiatorComponent>> {
    let negotiator = match &name[..] {
        "LimitAgreements" => Box::new(MaxAgreements::new(config)?) as Box<dyn NegotiatorComponent>,
        "LimitExpiration" => {
            Box::new(LimitExpiration::new(config)?) as Box<dyn NegotiatorComponent>
        }
        "AcceptAll" => Box::new(AcceptAll::new(config)?) as Box<dyn NegotiatorComponent>,
        _ => bail!("BuiltIn negotiator {} doesn't exists.", &name),
    };
    Ok(negotiator)
}

pub fn create_shared_lib(
    path: &Path,
    name: &str,
    config: serde_yaml::Value,
    working_dir: PathBuf,
) -> anyhow::Result<Box<dyn NegotiatorComponent>> {
    SharedLibNegotiator::new(path, name, config, working_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ya_builtin_negotiators::*;

    fn test_data_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("tests")
            .join("test-workdir")
    }

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
            composite: CompositeNegotiatorConfig::default_provider(),
        };

        let serialized = serde_yaml::to_string(&config).unwrap();
        println!("{}", serialized);

        let test_dir = test_data_dir();
        create_negotiator(
            serde_yaml::from_str(&serialized).unwrap(),
            test_dir.clone(),
            test_dir,
        )
        .unwrap();
    }
}

impl Default for NegotiatorsConfig {
    fn default() -> Self {
        NegotiatorsConfig {
            negotiators: vec![],
            composite: CompositeNegotiatorConfig::default_provider(),
        }
    }
}
