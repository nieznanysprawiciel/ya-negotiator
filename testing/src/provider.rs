use ya_agreement_utils::{AgreementView, OfferTemplate};
use ya_negotiators::factory::*;
use ya_negotiators::{AgreementAction, AgreementResult, ProposalAction};

use ya_client_model::market::proposal::State;
use ya_client_model::market::{DemandOfferBase, NewProposal, Proposal, Reason};
use ya_client_model::NodeId;

use crate::negotiation_record::{NegotiationRecord, NegotiationResult, NegotiationStage};
use crate::node::{Node, NodeType};

use anyhow::{anyhow, Error};
use futures::future::join_all;
use futures::stream::select_all;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;
use std::sync::Arc;
use tokio::sync::broadcast::Receiver;
use tokio::sync::Mutex;
use tokio_stream::wrappers::BroadcastStream;

pub struct ProviderReactions {
    pub providers: HashMap<NodeId, Arc<Node>>,
    pub requestors: HashMap<NodeId, Arc<Node>>,
    pub record: NegotiationRecord,
}

impl ProviderReactions {
    pub async fn accept_proposal(&self, node_id: NodeId, proposal_id: String) {
        let record = self.record.clone();
        let provider = self.providers.get(&node_id).cloned().unwrap();

        let req_proposal = record.get_proposal(&proposal_id).unwrap();
        let requestor = self.requestors.get(&req_proposal.issuer_id).unwrap();

        let prov_proposal = record
            .get_proposal(&req_proposal.prev_proposal_id.clone().unwrap())
            .unwrap();

        // Register event.
        record.accept(prov_proposal.clone(), req_proposal.issuer_id);

        let new_proposal = provider.into_proposal(
            NewProposal {
                properties: prov_proposal.properties,
                constraints: prov_proposal.constraints,
            },
            State::Draft,
        );

        if let Err(e) = requestor
            .react_to_proposal(&new_proposal, &req_proposal)
            .await
        {
            record.error(req_proposal.issuer_id, prov_proposal.issuer_id, e.into());
        }
    }

    pub async fn reject_proposal(
        &self,
        node_id: NodeId,
        proposal_id: String,
        reason: Option<Reason>,
    ) {
        let record = self.record.clone();
        let req_proposal = record.get_proposal(&proposal_id).unwrap();

        record.reject(node_id, req_proposal, reason);

        // We could notify Requestor, if Component API would allow it.
    }

    pub async fn counter_proposal(
        &self,
        node_id: NodeId,
        proposal_id: String,
        proposal: NewProposal,
    ) {
        let record = self.record.clone();
        let provider = self.providers.get(&node_id).cloned().unwrap();
        let req_proposal = record.get_proposal(&proposal_id).unwrap();
        let requestor = self.requestors.get(&req_proposal.issuer_id).unwrap();

        let proposal = provider.into_proposal(proposal, State::Draft);

        // Register event.
        record.counter(proposal.clone(), req_proposal.issuer_id);

        if let Err(e) = requestor.react_to_proposal(&proposal, &req_proposal).await {
            record.error(req_proposal.issuer_id, proposal.issuer_id, e.into())
        }
    }
}

async fn provider_proposals_processor(
    providers: HashMap<NodeId, Arc<Node>>,
    requestors: HashMap<NodeId, Arc<Node>>,
    record: NegotiationRecord,
) {
    let mut p_receivers = select_all(
        providers
            .iter()
            .map(|(_, node)| BroadcastStream::new(node.proposal_channel()))
            .collect::<Vec<BroadcastStream<_>>>(),
    );

    let reactions = ProviderReactions {
        record: record.clone(),
        requestors,
        providers,
    };

    while let Some(Ok((node_id, action))) = p_receivers.next().await {
        match action {
            ProposalAction::AcceptProposal { id } => reactions.accept_proposal(node_id, id).await,
            ProposalAction::CounterProposal { id, proposal } => {
                reactions.counter_proposal(node_id, id, proposal).await
            }
            ProposalAction::RejectProposal { id, reason } => {
                reactions.reject_proposal(node_id, id, reason).await
            }
        }
    }
}
