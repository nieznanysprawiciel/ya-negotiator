use ya_grpc_negotiator_api::entrypoint::{factory, register_negotiator, server_run};

use ya_testing_examples::EmitErrors;
use ya_testing_examples::FilterNodes;

pub fn register_negotiators() {
    register_negotiator("grpc-example", "FilterNodes", factory::<FilterNodes>());
    register_negotiator("grpc-example", "EmitErrors", factory::<EmitErrors>());
}

#[actix_rt::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    register_negotiators();
    server_run().await
}
