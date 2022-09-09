use actix::{Actor, Addr, Arbiter};
use clap::Parser;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tonic::{transport::Server, Request, Response, Status};

use grpc::negotiator_service_server::{NegotiatorService, NegotiatorServiceServer};
use grpc::{
    CallNegotiatorRequest, CallNegotiatorResponse, CreateNegotiatorRequest,
    CreateNegotiatorResponse, ShutdownRequest, ShutdownResponse,
};

use crate::actor::{NegotiatorWrapper, Shutdown};
use crate::message::NegotiationMessage;
use ya_negotiator_component::static_lib::create_static_negotiator;

pub mod grpc {
    tonic::include_proto!("grpc_negotiator");
}

pub struct GrpcNegotiatorServer {
    components: Arc<RwLock<HashMap<String, Addr<NegotiatorWrapper>>>>,
    arbiter: Arbiter,
}

impl Default for GrpcNegotiatorServer {
    fn default() -> Self {
        GrpcNegotiatorServer {
            components: Arc::new(Default::default()),
            arbiter: Arbiter::new(),
        }
    }
}

#[tonic::async_trait]
impl NegotiatorService for GrpcNegotiatorServer {
    async fn create_negotiator(
        &self,
        request: Request<CreateNegotiatorRequest>,
    ) -> Result<Response<CreateNegotiatorResponse>, Status> {
        let CreateNegotiatorRequest {
            name,
            params,
            workdir,
        } = request.into_inner();

        let params = serde_yaml::from_str(&params).map_err(|e| {
            Status::invalid_argument(format!(
                "Failed to deserialize params for negotiator: {name}. {e}"
            ))
        })?;

        // Creating negotiator in new scope, because it is not sync and we must limit it's lifetime,
        // so it can't outlive any await call.
        let (tx, rx) = tokio::sync::oneshot::channel();
        let name_ = name.clone();
        self.arbiter.spawn_fn(move || {
            let negotiator = match create_static_negotiator(&name_, params, PathBuf::from(workdir))
            {
                Ok(negotiator) => negotiator,
                Err(e) => {
                    tx.send(Err(Status::invalid_argument(format!(
                        "Failed to create negotiator {name_}. {e}"
                    ))))
                    .ok();
                    return;
                }
            };

            let wrapper = NegotiatorWrapper::new(negotiator);

            let id = wrapper.id.clone();
            let addr = wrapper.start();

            tx.send(Ok((id, addr))).ok();
        });

        let (id, wrapper) = rx.await.map_err(|e| {
            Status::internal(format!("Failed to start NegotiatorWrapper {name}. {e}"))
        })??;

        {
            self.components.write().await.insert(id.clone(), wrapper);
        }

        Ok(Response::new(CreateNegotiatorResponse { id }))
    }

    async fn shutdown_negotiator(
        &self,
        request: Request<ShutdownRequest>,
    ) -> Result<Response<ShutdownResponse>, Status> {
        let ShutdownRequest { id, timeout } = request.into_inner();

        match { self.components.write().await.remove(&id) } {
            None => {
                return Err(Status::not_found(format!(
                    "Can't shutdown. Negotiator: {id} not found."
                )))
            }
            Some(wrapper) => wrapper
                .send(Shutdown {
                    timeout: Duration::from_secs_f32(timeout),
                })
                .await
                .map_err(|e| Status::internal(format!("Actor for {id} is stopped. {e}")))?
                .map_err(|e| Status::ok(e.to_string()))?,
        };

        Ok(Response::new(ShutdownResponse {}))
    }

    async fn call_negotiator(
        &self,
        request: Request<CallNegotiatorRequest>,
    ) -> Result<Response<CallNegotiatorResponse>, Status> {
        let CallNegotiatorRequest { id, message } = request.into_inner();
        let message: NegotiationMessage = serde_json::from_str(&message).map_err(|e| {
            Status::invalid_argument(format!(
                "Failed to deserialize request for negotiator: {id}. {e}"
            ))
        })?;

        let response = match { self.components.read().await.get(&id).cloned() } {
            None => return Err(Status::not_found(format!("Negotiator: {id} not found"))),
            Some(wrapper) => wrapper
                .send(message)
                .await
                .map_err(|e| Status::internal(format!("Failed to call negotiator: {e}")))?
                .map_err(|e| {
                    log::info!("Negotiator error: {e}");
                    Status::ok(format!("Negotiator error: {e}"))
                })?,
        };

        Ok(Response::new(CallNegotiatorResponse {
            response: serde_json::to_string(&response).map_err(|e| {
                Status::internal(format!(
                    "Failed to serialize response from negotiator: {id}. {e}"
                ))
            })?,
        }))
    }
}

#[derive(clap::Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(long, env = "GRPC_SERVER_LISTEN", default_value = "[::1]:50051")]
    pub listen: String,
}

/// Runs grpc server. To be used in your negotiator binary.
/// Example:
/// ```no_run
/// use std::path::PathBuf;
///
/// use ya_grpc_negotiator_api::entrypoint::{factory, register_negotiator, server_run};
/// use ya_grpc_negotiator_api::plugin::{NegotiatorComponent, NegotiatorAsync, NegotiatorFactoryDefault};
///
/// #[derive(Default)]
/// pub struct ExampleNegotiator {}
///
/// impl NegotiatorComponent for ExampleNegotiator {}
///
/// impl NegotiatorFactoryDefault<ExampleNegotiator> for ExampleNegotiator {
///     type Type = NegotiatorAsync;
/// }
///
/// pub fn register_negotiators() {
///     register_negotiator("grpc-example", "FilterNodes", factory::<ExampleNegotiator>());
/// }
///
/// #[actix_rt::main]
/// async fn main() -> anyhow::Result<()> {
///     register_negotiators();
///     server_run().await
/// }
///```
#[allow(dead_code)]
pub async fn server_run() -> anyhow::Result<()> {
    let args = Args::parse();
    let addr = args.listen.parse().unwrap();
    let negotiator = GrpcNegotiatorServer::default();

    log::info!("GrpcNegotiator server listening on {}", addr);

    Server::builder()
        .add_service(NegotiatorServiceServer::new(negotiator))
        .serve(addr)
        .await?;

    log::info!("Shutting down server..");

    Ok(())
}
