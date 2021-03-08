use chrono::{DateTime, Utc};

use ya_agreement_utils::{InfNodeInfo, NodeInfo, OfferDefinition, OfferTemplate, ServiceInfo};
use ya_builtin_negotiators::*;
use ya_negotiators::factory::*;
use ya_negotiators_testing::Framework;

fn example_config() -> NegotiatorsConfig {
    let expiration_conf = NegotiatorConfig {
        name: "LimitExpiration".to_string(),
        load_mode: LoadMode::BuiltIn,
        params: serde_yaml::to_value(expiration::Config {
            min_expiration: std::time::Duration::from_secs(30),
            max_expiration: std::time::Duration::from_secs(300),
        })
        .unwrap(),
    };

    NegotiatorsConfig {
        negotiators: vec![expiration_conf],
    }
}

fn req_example_config() -> NegotiatorsConfig {
    let conf = NegotiatorConfig {
        name: "AcceptAll".to_string(),
        load_mode: LoadMode::BuiltIn,
        params: serde_yaml::Value::Null,
    };

    NegotiatorsConfig {
        negotiators: vec![conf],
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

fn example_demand(deadline: DateTime<Utc>) -> OfferTemplate {
    let ts = deadline.timestamp_millis();
    let properties = serde_json::json!({
        "golem.node.id.name": "example-node",
        "golem.srv.comp.expiration": ts
    });

    OfferTemplate {
        properties,
        constraints: "".to_string(),
    }
}

#[actix_rt::test]
async fn test_requestor_provider_flow() {
    let framework = Framework::new(example_config(), req_example_config()).unwrap();
    let result = framework
        .run_for_templates(
            example_demand(Utc::now() + chrono::Duration::seconds(150)),
            example_offer(),
        )
        .await
        .unwrap();

    println!("{}", result);
    // framework
    //     .run_finalize_agreement(
    //         &result.agreement.unwrap(),
    //         AgreementResult::ClosedByRequestor,
    //     )
    //     .await
    //     .unwrap();
    //assert!(false);
}
