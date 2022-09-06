use chrono::{DateTime, Utc};

use ya_agreement_utils::{InfNodeInfo, NodeInfo, OfferDefinition, OfferTemplate, ServiceInfo};
use ya_builtin_negotiators::*;
use ya_negotiators::factory::*;
use ya_negotiators::{NegotiatorCallbacks, ProposalAction};

use ya_client_model::market::proposal::State;
use ya_client_model::market::NewDemand;
use ya_client_model::market::Proposal;
use ya_negotiators_testing::prepare_test_dir;

fn example_config() -> NegotiatorsConfig {
    let expiration_conf = NegotiatorConfig {
        name: "LimitExpiration".to_string(),
        load_mode: LoadMode::StaticLib {
            library: "golem-negotiators".to_string(),
        },
        params: serde_yaml::to_value(expiration::Config {
            min_expiration: std::time::Duration::from_secs(30),
            max_expiration: std::time::Duration::from_secs(300),
        })
        .unwrap(),
    };

    let limit_conf = NegotiatorConfig {
        name: "LimitAgreements".to_string(),
        load_mode: LoadMode::StaticLib {
            library: "golem-negotiators".to_string(),
        },
        params: serde_yaml::to_value(max_agreements::Config { max_agreements: 1 }).unwrap(),
    };

    NegotiatorsConfig {
        negotiators: vec![expiration_conf, limit_conf],
        composite: CompositeNegotiatorConfig::default_test(),
    }
}

fn example_offer() -> OfferTemplate {
    OfferDefinition {
        node_info: NodeInfo::with_name("dany"),
        srv_info: ServiceInfo::new(InfNodeInfo::default(), serde_json::Value::Null),
        com_info: Default::default(),
        offer: OfferTemplate::default(),
    }
    .into_template()
}

fn example_demand(deadline: DateTime<Utc>, subnet: &str) -> NewDemand {
    let ts = deadline.timestamp_millis();
    let properties = serde_json::json!({
        "golem.node.id.name": "example-node",
        "golem.node.debug.subnet": subnet,
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
async fn test_static_library() {
    // Register negotiators as static library instead of using them as builtin future.
    ya_builtin_negotiators::register_negotiators();

    let config = example_config();
    let test_dir = prepare_test_dir("test_negotiation").unwrap();
    let (
        negotiator,
        NegotiatorCallbacks {
            proposal_channel: mut proposals,
            agreement_channel: _agreements,
        },
    ) = create_negotiator_actor(config, test_dir.clone(), test_dir)
        .await
        .unwrap();

    let offer = negotiator.create_offer(&example_offer()).await.unwrap();
    let offer = proposal_from_demand(&offer);

    let demand = example_demand(Utc::now() + chrono::Duration::seconds(50), "net-1");
    let proposal = proposal_from_demand(&demand);

    negotiator
        .react_to_proposal("", &proposal, &offer)
        .await
        .unwrap();

    match proposals.recv().await {
        Some(ProposalAction::AcceptProposal { .. }) => {}
        _ => panic!("Expected AcceptProposal"),
    }

    // Check variant with to long expiration time. We expect, that proposal will be rejected.
    let demand = example_demand(Utc::now() + chrono::Duration::seconds(900), "net-1");
    let proposal = proposal_from_demand(&demand);

    negotiator
        .react_to_proposal("", &proposal, &offer)
        .await
        .unwrap();

    match proposals.recv().await {
        Some(ProposalAction::RejectProposal { .. }) => {}
        _ => panic!("Expected reject proposal"),
    }
}
