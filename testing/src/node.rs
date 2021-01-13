use anyhow::*;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

use chrono::Utc;
use std::sync::Arc;
use ya_agreement_utils::{AgreementView, OfferTemplate};
use ya_client_model::market::proposal::State;
use ya_client_model::market::{DemandOfferBase, Proposal};
use ya_client_model::NodeId;
use ya_negotiators::factory::{create_negotiator, NegotiatorsConfig};
use ya_negotiators::{AgreementResponse, AgreementResult, NegotiatorAddr, ProposalResponse};

pub enum NodeType {
    Provider,
    Requestor,
}

pub struct Node {
    pub negotiator: Arc<NegotiatorAddr>,
    pub node_id: NodeId,
    pub node_type: NodeType,
}

impl Node {
    pub fn new(config: NegotiatorsConfig, node_type: NodeType) -> anyhow::Result<Node> {
        let negotiator = create_negotiator(config)?;
        let node_id = generate_identity();

        Ok(Node {
            node_id,
            negotiator,
            node_type,
        })
    }

    pub async fn create_offer(&self, template: &OfferTemplate) -> Result<Proposal> {
        let offer = self.negotiator.create_offer(&template).await?;
        let state = match self.node_type {
            NodeType::Provider => State::Initial,
            NodeType::Requestor => State::Draft,
        };

        Ok(self.into_proposal(offer, state))
    }

    pub async fn react_to_proposal(
        &self,
        offer: &Proposal,
        demand: &Proposal,
    ) -> Result<ProposalResponse> {
        self.negotiator.react_to_proposal(offer, demand).await
    }

    pub async fn react_to_agreement(
        &self,
        agreement_view: &AgreementView,
    ) -> Result<AgreementResponse> {
        self.negotiator.react_to_agreement(agreement_view).await
    }

    pub async fn agreement_finalized(
        &self,
        agreement_id: &str,
        result: AgreementResult,
    ) -> Result<()> {
        self.negotiator
            .agreement_finalized(agreement_id, result)
            .await
    }

    fn into_proposal(&self, offer: DemandOfferBase, state: State) -> Proposal {
        Proposal {
            properties: offer.properties,
            constraints: offer.constraints,
            proposal_id: generate_id(),
            issuer_id: self.node_id,
            state,
            timestamp: Utc::now(),
            prev_proposal_id: None,
        }
    }
}

pub fn generate_identity() -> NodeId {
    let random_node_id: String = thread_rng().sample_iter(&Alphanumeric).take(20).collect();
    NodeId::from(random_node_id.as_bytes())
}

pub fn generate_id() -> String {
    thread_rng().sample_iter(&Alphanumeric).take(64).collect()
}
