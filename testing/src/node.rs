use anyhow::*;
use chrono::{Duration, Utc};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use std::sync::Arc;
use tokio::sync::broadcast;

use ya_agreement_utils::{AgreementView, OfferTemplate};
use ya_client_model::market::agreement::State as AgreementState;
use ya_client_model::market::proposal::State;
use ya_client_model::market::{Agreement, Demand, DemandOfferBase, Offer, Proposal};
use ya_client_model::NodeId;
use ya_negotiators::factory::{create_negotiator, NegotiatorsConfig};
use ya_negotiators::{
    AgreementAction, AgreementResult, NegotiatorAddr, NegotiatorCallbacks, ProposalAction,
};

pub enum NodeType {
    Provider,
    Requestor,
}

pub struct Node {
    pub negotiator: Arc<NegotiatorAddr>,
    pub node_id: NodeId,
    pub node_type: NodeType,

    pub agreement_sender: broadcast::Sender<(NodeId, AgreementAction)>,
    pub proposal_sender: broadcast::Sender<(NodeId, ProposalAction)>,
}

impl Node {
    pub fn new(config: NegotiatorsConfig, node_type: NodeType) -> anyhow::Result<Arc<Node>> {
        let (negotiator, callbacks) = create_negotiator(config)?;
        let node_id = generate_identity();

        let (agreement_sender, _) = broadcast::channel(16);
        let (proposal_sender, _) = broadcast::channel(16);

        let node = Node {
            node_id: node_id.clone(),
            negotiator,
            node_type,
            proposal_sender: proposal_sender.clone(),
            agreement_sender: agreement_sender.clone(),
        };

        let NegotiatorCallbacks {
            proposal_channel: mut proposal,
            agreement_channel: mut agreement,
        } = callbacks;

        let id = node_id.clone();
        tokio::task::spawn_local(async move {
            while let Some(action) = proposal.recv().await {
                proposal_sender.send((id, action)).ok();
            }
        });

        let id = node_id.clone();
        tokio::task::spawn_local(async move {
            while let Some(action) = agreement.recv().await {
                agreement_sender.send((id, action)).ok();
            }
        });

        Ok(Arc::new(node))
    }

    pub fn agreement_channel(&self) -> broadcast::Receiver<(NodeId, AgreementAction)> {
        self.agreement_sender.subscribe()
    }

    pub fn proposal_channel(&self) -> broadcast::Receiver<(NodeId, ProposalAction)> {
        self.proposal_sender.subscribe()
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
        incoming_proposal: &Proposal,
        our_prev_proposal: &Proposal,
    ) -> Result<()> {
        self.negotiator
            .react_to_proposal(incoming_proposal, our_prev_proposal)
            .await
    }

    pub async fn react_to_agreement(&self, agreement_view: &AgreementView) -> Result<()> {
        self.negotiator.react_to_agreement(agreement_view).await
    }

    pub async fn agreement_signed(&self, agreement_view: &AgreementView) -> Result<()> {
        self.negotiator.agreement_signed(agreement_view).await
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

    pub fn into_proposal(&self, offer: DemandOfferBase, state: State) -> Proposal {
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

    pub fn create_agreement(
        &self,
        demand_proposal: &Proposal,
        offer_proposal: &Proposal,
    ) -> Agreement {
        let offer = Offer {
            properties: offer_proposal.properties.clone(),
            constraints: offer_proposal.constraints.clone(),
            offer_id: offer_proposal.proposal_id.clone(),
            provider_id: offer_proposal.issuer_id,
            timestamp: offer_proposal.timestamp,
        };

        let demand = Demand {
            properties: demand_proposal.properties.clone(),
            constraints: demand_proposal.constraints.clone(),
            demand_id: demand_proposal.proposal_id.clone(),
            timestamp: demand_proposal.timestamp,
            requestor_id: demand_proposal.issuer_id,
        };

        Agreement {
            agreement_id: generate_id(),
            demand,
            offer,
            valid_to: Utc::now() + Duration::minutes(20),
            approved_date: None,
            state: AgreementState::Proposal,
            timestamp: Utc::now(),
            app_session_id: None,
            proposed_signature: None,
            approved_signature: None,
            committed_signature: None,
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
