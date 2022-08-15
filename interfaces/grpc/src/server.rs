use actix::{Actor, Addr};
use clap::Parser;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
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

#[derive(Default)]
pub struct GrpcNegotiatorServer {
    components: Arc<RwLock<HashMap<String, Addr<NegotiatorWrapper>>>>,
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
        let (id, wrapper) = {
            let negotiator = create_static_negotiator(&name, params, PathBuf::from(workdir))
                .map_err(|e| {
                    Status::invalid_argument(format!("Failed to create negotiator {name}. {e}"))
                })?;

            let wrapper = NegotiatorWrapper::new(negotiator);
            (wrapper.id.clone(), wrapper.start())
        };

        {
            self.components.write().await.insert(id.clone(), wrapper);
        }

        Ok(Response::new(CreateNegotiatorResponse { id }))
    }

    async fn shutdown_negotiator(
        &self,
        request: Request<ShutdownRequest>,
    ) -> Result<Response<ShutdownResponse>, Status> {
        let ShutdownRequest { id } = request.into_inner();

        match { self.components.write().await.remove(&id) } {
            None => {
                return Err(Status::not_found(format!(
                    "Can't shutdown. Negotiator: {id} not found."
                )))
            }
            Some(wrapper) => wrapper.send(Shutdown {}).await.ok(),
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
                .map_err(|e| Status::ok(format!("Negotiator error: {e}")))?,
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
/// #[tokio::main]
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

    println!("GrpcNegotiator server listening on {}", addr);

    Server::builder()
        .add_service(NegotiatorServiceServer::new(negotiator))
        .serve(addr)
        .await?;

    Ok(())
}
