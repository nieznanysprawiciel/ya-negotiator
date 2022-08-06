use actix::Addr;
use clap::Parser;
use std::collections::HashMap;
use tonic::{transport::Server, Request, Response, Status};

use grpc::negotiator_service_server::{NegotiatorService, NegotiatorServiceServer};
use grpc::{
    CallNegotiatorRequest, CallNegotiatorResponse, CreateNegotiatorRequest,
    CreateNegotiatorResponse, ShutdownRequest, ShutdownResponse,
};

use crate::actor::NegotiatorWrapper;

pub mod grpc {
    tonic::include_proto!("grpc_negotiator");
}

#[derive(Default)]
pub struct GrpcNegotiatorServer {
    components: HashMap<String, Addr<NegotiatorWrapper>>,
}

#[tonic::async_trait]
impl NegotiatorService for GrpcNegotiatorServer {
    async fn create_negotiator(
        &self,
        _request: Request<CreateNegotiatorRequest>,
    ) -> Result<Response<CreateNegotiatorResponse>, Status> {
        Ok(Response::new(CreateNegotiatorResponse {
            id: "None".to_string(),
        }))
    }

    async fn shutdown_negotiator(
        &self,
        _request: Request<ShutdownRequest>,
    ) -> Result<Response<ShutdownResponse>, Status> {
        Ok(Response::new(ShutdownResponse {
            response: "Unimplemented".to_string(),
        }))
    }

    async fn call_negotiator(
        &self,
        request: Request<CallNegotiatorRequest>,
    ) -> Result<Response<CallNegotiatorResponse>, Status> {
        let CallNegotiatorRequest { id, message } = request.into_inner();
        let message = serde_json::from_str(&message).map_err(|e| {
            Status::invalid_argument(format!(
                "Failed to deserialize request for negotiator: {id}. {e}"
            ))
        })?;

        let response = match self.components.get(&id) {
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

// #[tokio::main]
// async fn main() -> anyhow::Result<()> {
//     server_run().await
// }
