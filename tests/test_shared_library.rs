use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use ya_agreement_utils::{InfNodeInfo, NodeInfo, OfferDefinition, OfferTemplate, ServiceInfo};
use ya_negotiators::factory::*;
use ya_negotiators::ProposalResponse;

use ya_client_model::market::proposal::State;
use ya_client_model::market::{NewDemand, Proposal};

#[derive(Serialize, Deserialize)]
pub struct FilterNodesConfig {
    pub names: Vec<String>,
}

#[cfg(debug_assertions)]
fn debug_or_release() -> String {
    "debug".to_string()
}

#[cfg(not(debug_assertions))]
fn debug_or_release() -> String {
    "release".to_string()
}

fn example_config() -> NegotiatorsConfig {
    let filter_conf = NegotiatorConfig {
        name: "FilterNodes".to_string(),
        load_mode: LoadMode::SharedLibrary {
            path: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("target")
                .join(debug_or_release())
                .join("libdll_negotiator.so"),
        },
        params: serde_yaml::to_value(FilterNodesConfig {
            names: vec!["dany".to_string()],
        })
        .unwrap(),
    };

    NegotiatorsConfig {
        negotiators: vec![filter_conf],
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
async fn test_shared_library() {
    let config = example_config();
    let negotiator = create_negotiator(config).unwrap();

    let offer = negotiator
        .create_offer(&example_offer_definition())
        .await
        .unwrap();
    let offer = proposal_from_demand(&offer);

    let demand = example_demand(Utc::now() + chrono::Duration::seconds(50), "dany");
    let proposal = proposal_from_demand(&demand);

    let result = negotiator
        .react_to_proposal(&proposal, &offer)
        .await
        .unwrap();

    match result {
        ProposalResponse::RejectProposal { .. } => {}
        _ => panic!("Expected reject proposal"),
    }

    // Check variant with to long expiration time. We expect, that proposal will be rejected.
    let demand = example_demand(Utc::now() + chrono::Duration::seconds(900), "node-1");
    let proposal = proposal_from_demand(&demand);

    let result = negotiator
        .react_to_proposal(&proposal, &offer)
        .await
        .unwrap();

    match result {
        ProposalResponse::AcceptProposal { .. } => {}
        _ => panic!("Expected AcceptProposal"),
    }
}
