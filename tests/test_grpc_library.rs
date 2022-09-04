use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use test_binary::build_test_binary;

use ya_agreement_utils::{InfNodeInfo, NodeInfo, OfferDefinition, OfferTemplate, ServiceInfo};
use ya_negotiators::factory::*;
use ya_negotiators::{NegotiatorCallbacks, ProposalAction};

use ya_client_model::market::proposal::State;
use ya_client_model::market::{NewDemand, Proposal};
use ya_negotiators_testing::prepare_test_dir;

#[derive(Serialize, Deserialize)]
pub struct FilterNodesConfig {
    pub names: Vec<String>,
}

fn example_config() -> NegotiatorsConfig {
    let test_bin_path =
        build_test_binary("grpc-example", "examples").expect("error building grpc-example");

    let filter_conf = NegotiatorConfig {
        name: "grpc-example::FilterNodes".to_string(),
        load_mode: LoadMode::Grpc {
            path: PathBuf::from(test_bin_path),
        },
        params: serde_yaml::to_value(FilterNodesConfig {
            names: vec!["dany".to_string()],
        })
        .unwrap(),
    };

    NegotiatorsConfig {
        negotiators: vec![filter_conf],
        composite: CompositeNegotiatorConfig::default_test(),
    }
}

fn example_offer_definition() -> OfferTemplate {
    OfferDefinition {
        node_info: NodeInfo::with_name("blabla"),
        srv_info: ServiceInfo::new(InfNodeInfo::default(), serde_json::Value::Null),
        com_info: Default::default(),
        offer: OfferTemplate::default(),
    }
    .into_template()
}

fn example_demand(deadline: DateTime<Utc>, node_name: &str) -> NewDemand {
    let ts = deadline.timestamp_millis();
    let properties = serde_json::json!({
        "golem.node.id.name": node_name,
        "golem.node.debug.subnet": "net-1",
        "golem.srv.comp.task_package": "package".to_string(),
        "golem.srv.comp.expiration": ts
    });

    // No constraints, since we don't validate them whatsoever
    let constraints = "".to_string();

    NewDemand {
        properties,
        constraints,
    }
}

fn proposal_from_demand(demand: &NewDemand) -> Proposal {
    Proposal {
        properties: demand.properties.clone(),
        constraints: demand.constraints.clone(),
        proposal_id: "".to_string(),
        issuer_id: Default::default(),
        state: State::Draft,
        timestamp: Utc::now(),
        prev_proposal_id: None,
    }
}

#[actix_rt::test]
async fn test_grpc_library() {
    env_logger::init();

    let config = example_config();
    let test_dir = prepare_test_dir("test_grpc_library").unwrap();
    let (
        negotiator,
        NegotiatorCallbacks {
            proposal_channel: mut proposals,
            agreement_channel: _agreements,
        },
    ) = create_negotiator(config, test_dir.clone(), test_dir)
        .await
        .unwrap();

    let offer = negotiator
        .create_offer(&example_offer_definition())
        .await
        .unwrap();
    let offer = proposal_from_demand(&offer);

    let demand = example_demand(Utc::now() + chrono::Duration::seconds(50), "dany");
    let proposal = proposal_from_demand(&demand);

    negotiator
        .react_to_proposal("", &proposal, &offer)
        .await
        .unwrap();

    match proposals.recv().await {
        Some(ProposalAction::RejectProposal { .. }) => {}
        _ => panic!("Expected reject proposal"),
    }

    // Check variant with to long expiration time. We expect, that proposal will be rejected.
    let demand = example_demand(Utc::now() + chrono::Duration::seconds(900), "node-1");
    let proposal = proposal_from_demand(&demand);

    negotiator
        .react_to_proposal("", &proposal, &offer)
        .await
        .unwrap();

    match proposals.recv().await {
        Some(ProposalAction::AcceptProposal { .. }) => {}
        _ => panic!("Expected AcceptProposal"),
    }
}
