use chrono::{DateTime, Duration, Utc};

use ya_agreement_utils::OfferTemplate;
use ya_builtin_negotiators::*;
use ya_negotiators::factory::*;
use ya_negotiators::AgreementResult;
use ya_negotiators_testing::Framework;
use ya_testing_examples::{InfNodeInfo, NodeInfo, OfferDefinition, ServiceInfo};

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
        composite: CompositeNegotiatorConfig::default_test(),
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
    let framework = Framework::new(
        "test_requestor_provider_flow",
        example_config(),
        req_example_config(),
    )
    .await
    .unwrap();
    let record = framework
        .run_for_templates(
            example_demand(Utc::now() + chrono::Duration::seconds(150)),
            example_offer(),
        )
        .await
        .unwrap();

    assert_eq!(framework.providers.len(), 1);
    assert_eq!(framework.requestors.len(), 1);

    assert!(!record.results.is_empty());
    record
        .results
        .iter()
        .for_each(|(_nodes, result)| result.is_finished_with_agreement().unwrap());

    let results = framework
        .run_finalize_agreements(
            record
                .agreements
                .iter()
                .map(|(_, agreement)| (agreement, AgreementResult::ClosedByThem))
                .collect(),
        )
        .await;

    if results.iter().any(|result| result.is_err()) {
        panic!("{:?}", results);
    }

    // println!("{}", record);
    // assert!(false);
}

/// Provider should be able to negotiate with new Requestor after previous Agreement is finished.
#[actix_rt::test]
async fn test_negotiations_after_agreement_termination() {
    let framework = Framework::new(
        "test_negotiations_after_agreement_termination",
        example_config(),
        req_example_config(),
    )
    .await
    .unwrap();
    let record = framework
        .run_for_templates(
            example_demand(Utc::now() + chrono::Duration::seconds(150)),
            example_offer(),
        )
        .await
        .unwrap();

    // Close all(1) negotiated Agreement.
    framework
        .run_finalize_agreements(
            record
                .agreements
                .iter()
                .map(|(_, agreement)| (agreement, AgreementResult::ClosedByThem))
                .collect(),
        )
        .await;

    for (_, node) in framework.providers.iter() {
        node.request_agreements(1).await.unwrap();
    }

    // Add new Requestor to negotiate with Provider.
    let framework = framework
        .add_named_requestor(req_example_config(), "IncomingReq")
        .await
        .unwrap();
    let record = framework
        .continue_run_for_named_requestor(
            "IncomingReq",
            example_demand(Utc::now() + chrono::Duration::seconds(150)),
            &record,
        )
        .await
        .unwrap();

    assert_eq!(record.agreements.len(), 1);
    assert!(!record.results.is_empty());
    record
        .results
        .iter()
        .for_each(|(_nodes, result)| result.is_finished_with_agreement().unwrap());
}

/// Provider should wait collect period before accepting Proposals.
#[actix_rt::test]
async fn test_negotiations_collect_period() {
    let mut provider_config = example_config();
    provider_config.composite.agreements.collect_amount = None;
    provider_config.composite.agreements.collect_period = Some(std::time::Duration::from_secs(6));

    let framework = Framework::new_empty("test_negotiations_collect_period")
        .unwrap()
        .test_timeout(std::time::Duration::from_secs(10))
        .add_provider(provider_config)
        .await
        .unwrap()
        .add_requestor(req_example_config())
        .await
        .unwrap()
        .add_requestor(req_example_config())
        .await
        .unwrap();

    let before = Utc::now();
    let record = framework
        .run_for_templates(
            example_demand(Utc::now() + chrono::Duration::seconds(150)),
            example_offer(),
        )
        .await
        .unwrap();

    let diff = Utc::now() - before;
    assert!(diff > Duration::seconds(6));

    //println!("{}", record);

    // 2 Agreements will be created, but only one should be signed.
    assert_eq!(record.agreements.len(), 2);
    assert!(!record.results.is_empty());
    assert_eq!(
        record
            .results
            .iter()
            .filter(|(_nodes, result)| result.is_finished_with_agreement().is_ok())
            .count(),
        1
    );
}
