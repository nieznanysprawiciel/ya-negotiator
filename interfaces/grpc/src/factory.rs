use anyhow::anyhow;
use lazy_static::lazy_static;
use portpicker::pick_unused_port;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::process::{Child, Command};
use tokio::sync::RwLock;

use crate::grpc::negotiator_service_client::NegotiatorServiceClient;

use crate::component::GRPCComponent;
use ya_negotiator_component::NegotiatorComponent;

lazy_static! {
    // Stores all created services
    static ref SERVICES: Arc<RwLock<HashMap<PathBuf, RemoteServiceHandle>>> = Arc::new(RwLock::new(HashMap::new()));
}

pub type NegotiatorClient = NegotiatorServiceClient<tonic::transport::Channel>;

/// Handle to single grpc binary with negotiators.
/// Each binary can serve multiple negotiators of different types.
///
/// We need to manage process from single place, to avoid spawning more processes
/// than necessary.
#[derive(Clone)]
pub struct RemoteServiceHandle {
    inner: Arc<RwLock<RemoteServiceHandleImpl>>,
}

#[allow(dead_code)]
struct RemoteServiceHandleImpl {
    pub client: NegotiatorClient,
    pub process: Child,
    pub address: SocketAddr,
    pub file: PathBuf,
}

impl RemoteServiceHandle {
    pub async fn create_service(path: PathBuf) -> anyhow::Result<RemoteServiceHandle> {
        let path = path
            .canonicalize()
            .map_err(|e| anyhow!("Can't canonicalize binary path. {e}"))?;

        log::debug!("Looking for existing service: {}", path.display());

        if let Some(service) = existing_service(&path).await {
            log::debug!("Service: {} already running. Reusing..", path.display());
            return Ok(service);
        }

        log::debug!("Service: {} isn't running yet.", path.display());

        let ip = "127.0.0.1";
        let port: u16 = pick_unused_port().ok_or(anyhow!("No ports free"))?;
        let address: SocketAddr = format!("{ip}:{port}").parse()?;

        log::debug!("Spawning service: {}", path.display());

        let process = Command::new(path.clone())
            .args(["--listen", &address.to_string()])
            .spawn()
            .map_err(|e| anyhow!("Can't spawn process. {e}"))?;

        log::debug!("Connecting to service: {} on {address}", path.display());

        // TODO: Find better way to know, that server is ready.
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let client = NegotiatorClient::connect(format!("http://{}", address.to_string()))
            .await
            .map_err(|e| anyhow!("Can't connect to service. {e}"))?;

        let service = RemoteServiceHandle {
            inner: Arc::new(RwLock::new(RemoteServiceHandleImpl {
                client,
                process,
                address,
                file: path.clone(),
            })),
        };

        // TODO: Race conditions between this place and earlier lookup.
        (*SERVICES).write().await.insert(path, service.clone());
        Ok(service)
    }

    pub async fn client(&self) -> NegotiatorServiceClient<tonic::transport::Channel> {
        self.inner.read().await.client.clone()
    }
}

async fn existing_service(path: &PathBuf) -> Option<RemoteServiceHandle> {
    (*SERVICES).read().await.get(path).cloned()
}

pub async fn create_grpc_negotiator(
    path: PathBuf,
    name: &str,
    config: serde_yaml::Value,
    workdir: PathBuf,
) -> anyhow::Result<Box<dyn NegotiatorComponent>> {
    GRPCComponent::new(path, name, config, workdir)
        .await
        .map(|negotiator| Box::new(negotiator) as Box<dyn NegotiatorComponent>)
}
