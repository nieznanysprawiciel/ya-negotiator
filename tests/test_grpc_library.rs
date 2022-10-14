use chrono::{DateTime, Utc};
use std::convert::TryFrom;

use ya_agreement_utils::{
    AgreementView, InfNodeInfo, NodeInfo, OfferDefinition, OfferTemplate, ServiceInfo,
};
use ya_negotiators::factory::*;
use ya_negotiators::{AgreementAction, NegotiatorCallbacks, ProposalAction};

use ya_client_model::market::proposal::State;
use ya_client_model::market::{NewDemand, Proposal};
use ya_negotiator_component::{AgreementEvent, AgreementResult};
use ya_negotiators_testing::{generate_id, prepare_test_dir, test_assets_dir};
use ya_testing_examples::configs::{example_config, example_config_filter};

use ya_testing_examples::AddError;

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
        proposal_id: generate_id(),
        issuer_id: Default::default(),
        state: State::Draft,
        timestamp: Utc::now(),
        prev_proposal_id: None,
    }
}

fn sample_agreement() -> anyhow::Result<AgreementView> {
    let agreement_file = test_assets_dir().join("agreement.json");
    Ok(AgreementView::try_from(&agreement_file)?)
}

#[actix_rt::test]
async fn test_grpc_library() {
    let config = example_config();
    let test_dir = prepare_test_dir("test_grpc_library").unwrap();
    let (
        negotiator,
        NegotiatorCallbacks {
            proposal_channel: mut proposals,
            agreement_channel: mut agreements,
        },
    ) = create_negotiator_actor(config, test_dir.clone(), test_dir)
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
        .react_to_proposal("62d6b078d65c2cf8cf11fcdd0784388d31", &proposal, &offer)
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

    negotiator
        .react_to_agreement(
            "62d6b078d65c2cf8cf11fcdd0784388d31",
            &sample_agreement().unwrap(),
        )
        .await
        .unwrap();

    match agreements.recv().await {
        Some(AgreementAction::ApproveAgreement { .. }) => {}
        action => panic!("Expected ApproveAgreement, got: {:?}", action),
    }
}

/// Negotiators will get data that should result in non-error response.
/// This test checks, if all negotiator interface functions are called correctly.  
#[actix_rt::test]
async fn test_grpc_library_positive_calls() {
    let config = example_config();
    let test_dir = prepare_test_dir("test_grpc_library_positive_calls").unwrap();
    let (
        negotiator,
        NegotiatorCallbacks {
            proposal_channel: _proposals,
            agreement_channel: _agreements,
        },
    ) = create_negotiator_actor(config, test_dir.clone(), test_dir)
        .await
        .unwrap();

    negotiator
        .agreement_signed(&sample_agreement().unwrap())
        .await
        .unwrap();

    negotiator
        .agreement_rejected("0d17822518dc3770042d69262d6b078d65c2cf8cf11fcdd0784388d31fd2a7e8")
        .await
        .unwrap();

    negotiator
        .agreement_finalized(
            "0d17822518dc3770042d69262d6b078d65c2cf8cf11fcdd0784388d31fd2a7e8",
            AgreementResult::ClosedByUs,
        )
        .await
        .unwrap();

    negotiator
        .proposal_rejected(
            "0d17822518dc3770042d69262d6b078d65c2cf8cf11fcdd0784388d31fd2a7e8",
            &None,
        )
        .await
        .unwrap();

    negotiator
        .post_agreement_event(
            "0d17822518dc3770042d69262d6b078d65c2cf8cf11fcdd0784388d31fd2a7e8",
            AgreementEvent::InvoiceAccepted,
        )
        .await
        .unwrap();
}

/// Check if grpc correctly receives errors from negotiators.
#[actix_rt::test]
async fn test_grpc_library_negative_calls() {
    let config = example_config();
    let test_dir = prepare_test_dir("test_grpc_library_negative_calls").unwrap();
    let negotiator = create_negotiator(config.negotiators[1].clone(), test_dir.clone(), test_dir)
        .await
        .unwrap();

    let errors = vec![
        "agreement_signed failed",
        "agreement_finalized failed",
        "proposal_rejected failed",
        "post_agreement_event failed",
        "react_to_proposal failed",
        "react_to_agreement failed",
    ];

    for error in errors {
        negotiator
            .control_event(
                "grpc-example::EmitErrors",
                serde_json::to_value(AddError(error.to_string())).unwrap(),
            )
            .await
            .unwrap();
    }

    negotiator
        .on_agreement_approved(&sample_agreement().unwrap())
        .await
        .unwrap_err();

    negotiator
        .on_agreement_terminated(
            "0d17822518dc3770042d69262d6b078d65c2cf8cf11fcdd0784388d31fd2a7e8",
            &AgreementResult::ClosedByUs,
        )
        .await
        .unwrap_err();

    negotiator
        .on_proposal_rejected(
            "0d17822518dc3770042d69262d6b078d65c2cf8cf11fcdd0784388d31fd2a7e8",
            // &None,
        )
        .await
        .unwrap_err();

    negotiator
        .on_agreement_event(
            "0d17822518dc3770042d69262d6b078d65c2cf8cf11fcdd0784388d31fd2a7e8",
            &AgreementEvent::InvoiceAccepted,
        )
        .await
        .unwrap_err();
}

/// Spawn 2 negotiators of the same type to ensure grpc handles this correctly.
#[actix_rt::test]
async fn test_2same_negotiators() {
    env_logger::init();

    let config = example_config_filter(&["dany", "nieznanysprawiciel-laptop-Requestor-1"]);
    let test_dir = prepare_test_dir("test_grpc_library").unwrap();
    let (
        negotiator,
        NegotiatorCallbacks {
            proposal_channel: mut proposals,
            agreement_channel: _agreements,
        },
    ) = create_negotiator_actor(config, test_dir.clone(), test_dir)
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
        .react_to_proposal("62d6b078d65c2cf8cf11fcdd0784388d31", &proposal, &offer)
        .await
        .unwrap();

    match proposals.recv().await {
        Some(ProposalAction::RejectProposal { .. }) => {}
        action => panic!("Expected reject proposal, found: {:?}", action),
    }
}
