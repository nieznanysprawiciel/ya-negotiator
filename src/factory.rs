use anyhow::bail;
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::negotiators::NegotiatorAddr;
use crate::Negotiator;

use ya_grpc_negotiator_api::create_grpc_negotiator;
use ya_negotiator_component::component::NegotiatorComponent;
use ya_negotiator_component::static_lib::{create_static_negotiator, factory};
use ya_negotiator_component::NegotiatorsChain;

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
    Grpc { path: PathBuf },
    RemoteGrpc { address: SocketAddr },
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

pub async fn create_negotiator_actor(
    config: NegotiatorsConfig,
    working_dir: PathBuf,
    plugins_dir: PathBuf,
) -> anyhow::Result<(Arc<NegotiatorAddr>, NegotiatorCallbacks)> {
    let components = create_negotiators(config.clone(), working_dir, plugins_dir).await?;

    let (negotiator, callbacks) =
        Negotiator::new(NegotiatorsChain::with(components), config.composite);
    Ok((Arc::new(NegotiatorAddr::from(negotiator)), callbacks))
}

pub async fn create_negotiator(
    config: NegotiatorConfig,
    working_dir: PathBuf,
    plugins_dir: PathBuf,
) -> anyhow::Result<Box<dyn NegotiatorComponent>> {
    let name = config.name;
    let working_dir = working_dir.join(&name);

    log::info!("Creating negotiator: {}", name);

    fs::create_dir_all(&working_dir)?;

    Ok(match config.load_mode {
        LoadMode::BuiltIn => create_builtin(&name, config.params, working_dir)?,
        LoadMode::SharedLibrary { path } => {
            let plugin_path = match path.is_relative() {
                true => plugins_dir.join(path),
                false => path,
            };
            create_shared_lib(&plugin_path, &name, config.params, working_dir)?
        }
        LoadMode::StaticLib { library } => {
            create_static_negotiator(&format!("{library}::{name}"), config.params, working_dir)?
        }
        LoadMode::Grpc { path } => {
            let plugin_path = match path.is_relative() {
                true => plugins_dir.join(path),
                false => path,
            };
            create_grpc_negotiator(plugin_path, &name, config.params, working_dir).await?
        }
        LoadMode::RemoteGrpc { address: _ } => {
            bail!("Not implemented")
        }
    })
}

pub async fn create_negotiators(
    config: NegotiatorsConfig,
    working_dir: PathBuf,
    plugins_dir: PathBuf,
) -> anyhow::Result<Vec<(String, Box<dyn NegotiatorComponent>)>> {
    let mut components = Vec::<(String, Box<dyn NegotiatorComponent>)>::new();
    for config in config.negotiators.into_iter() {
        components.push((
            config.name.clone(),
            create_negotiator(config, working_dir.clone(), plugins_dir.clone()).await?,
        ));
    }
    Ok(components)
}

pub fn create_builtin(
    name: &str,
    config: serde_yaml::Value,
    working_dir: PathBuf,
) -> anyhow::Result<Box<dyn NegotiatorComponent>> {
    let negotiator = match &name[..] {
        "LimitAgreements" => factory::<MaxAgreements>()(name, config, working_dir)?,
        "LimitExpiration" => factory::<LimitExpiration>()(name, config, working_dir)?,
        "AcceptAll" => factory::<AcceptAll>()(name, config, working_dir)?,
        _ => bail!("BuiltIn negotiator {} doesn't exists.", &name),
    };
    Ok(negotiator)
}

pub fn create_shared_lib(
    _path: &Path,
    _name: &str,
    _config: serde_yaml::Value,
    _working_dir: PathBuf,
) -> anyhow::Result<Box<dyn NegotiatorComponent>> {
    bail!("Not supported")
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
        create_negotiator_actor(
            serde_yaml::from_str(&serialized).unwrap(),
            test_dir.clone(),
            test_dir,
        )
        .await
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
