mod filter_nodes;

use ya_grpc_negotiator_api::entrypoint::{factory, register_negotiator, server_run};

use crate::filter_nodes::FilterNodes;

pub fn register_negotiators() {
    register_negotiator("grpc-example", "FilterNodes", factory::<FilterNodes>());
}

#[actix_rt::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    register_negotiators();
    server_run().await
}
