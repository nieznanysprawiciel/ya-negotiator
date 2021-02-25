use ya_agreement_utils::AgreementView;
use ya_negotiators::{AgreementAction, ProposalAction};

use ya_client_model::market::proposal::State;
use ya_client_model::market::{NewProposal, Reason};
use ya_client_model::NodeId;

use crate::negotiation_record::NegotiationRecordSync;
use crate::node::Node;

use futures::stream::select_all;
use futures::StreamExt;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::Arc;
use tokio_stream::wrappers::BroadcastStream;

pub struct RequestorReactions {
    pub providers: HashMap<NodeId, Arc<Node>>,
    pub requestors: HashMap<NodeId, Arc<Node>>,
    pub record: NegotiationRecordSync,
}

impl RequestorReactions {
    /// On Requestor side accepting Proposal means proposing Agreement.
    pub async fn accept_proposal(&self, node_id: NodeId, proposal_id: String) {
        let record = self.record.clone();
        let requestor = self.requestors.get(&node_id).unwrap();

        let prov_proposal = record.get_proposal(&proposal_id).unwrap();

        let req_proposal = record
            .get_proposal(&prov_proposal.prev_proposal_id.clone().unwrap())
            .unwrap();

        // Register event.
        record.accept(req_proposal.clone(), prov_proposal.issuer_id);

        let agreement = requestor.create_agreement(&req_proposal, &prov_proposal);
        let view = AgreementView::try_from(&agreement).unwrap();

        // Requestor will asynchronously send message, that he wants too send this Agreement to Provider.
        if let Err(e) = requestor.react_to_agreement(&view).await {
            record.error(req_proposal.issuer_id, prov_proposal.issuer_id, e.into());
            return;
        }
    }

    pub async fn reject_proposal(
        &self,
        node_id: NodeId,
        proposal_id: String,
        reason: Option<Reason>,
    ) {
        let record = self.record.clone();
        let prov_proposal = record.get_proposal(&proposal_id).unwrap();

        record.reject(node_id, prov_proposal, reason);

        // We could notify Requestor, if Component API would allow it.
    }

    pub async fn counter_proposal(
        &self,
        node_id: NodeId,
        proposal_id: String,
        proposal: NewProposal,
    ) {
        let record = self.record.clone();
        let requestor = self.requestors.get(&node_id).unwrap();

        let prov_proposal = record.get_proposal(&proposal_id).unwrap();
        let provider = self.requestors.get(&prov_proposal.issuer_id).unwrap();

        let proposal = requestor.into_proposal(proposal, State::Draft);

        // Register event.
        record.counter(proposal.clone(), prov_proposal.issuer_id);

        if let Err(e) = provider.react_to_proposal(&proposal, &prov_proposal).await {
            record.error(prov_proposal.issuer_id, proposal.issuer_id, e.into());
        }
    }

    /// Approve Agreement on Requestor side means, that Agreement will be confirmed
    /// (and sent to Provider).
    pub async fn approve_agreement(&self, node_id: NodeId, agreement_id: String) {
        let record = self.record.clone();
        let agreement = record.get_agreement(&agreement_id).unwrap();
        let provider = self.providers.get(agreement.provider_id()).unwrap();
        let provider_id = agreement.provider_id().clone();

        let view = AgreementView::try_from(&agreement).unwrap();

        record.approve(agreement);

        if let Err(e) = provider.react_to_agreement(&view).await {
            record.error(provider_id, node_id, e.into());
        }
    }

    pub async fn reject_agreement(
        &self,
        _node_id: NodeId,
        agreement_id: String,
        reason: Option<Reason>,
    ) {
        let record = self.record.clone();
        let agreement = record.get_agreement(&agreement_id).unwrap();

        record.reject_agreement(agreement, reason);
    }
}

pub async fn requestor_proposals_processor(
    providers: HashMap<NodeId, Arc<Node>>,
    requestors: HashMap<NodeId, Arc<Node>>,
    record: NegotiationRecordSync,
) {
    let mut r_receivers = select_all(
        requestors
            .iter()
            .map(|(_, node)| BroadcastStream::new(node.proposal_channel()))
            .collect::<Vec<BroadcastStream<_>>>(),
    );

    let reactions = RequestorReactions {
        record: record.clone(),
        requestors,
        providers,
    };

    while let Some(Ok((node_id, action))) = r_receivers.next().await {
        match action {
            ProposalAction::AcceptProposal { id } => reactions.accept_proposal(node_id, id).await,
            ProposalAction::CounterProposal { id, proposal } => {
                reactions.counter_proposal(node_id, id, proposal).await
            }
            ProposalAction::RejectProposal { id, reason } => {
                reactions.reject_proposal(node_id, id, reason).await
            }
        };

        if record.is_finished() {
            break;
        }
    }
}

pub async fn requestor_agreements_processor(
    providers: HashMap<NodeId, Arc<Node>>,
    requestors: HashMap<NodeId, Arc<Node>>,
    record: NegotiationRecordSync,
) {
    let mut r_receivers = select_all(
        requestors
            .iter()
            .map(|(_, node)| BroadcastStream::new(node.agreement_channel()))
            .collect::<Vec<BroadcastStream<_>>>(),
    );

    let reactions = RequestorReactions {
        record: record.clone(),
        requestors,
        providers,
    };

    while let Some(Ok((node_id, action))) = r_receivers.next().await {
        match action {
            AgreementAction::ApproveAgreement { id } => {
                reactions.approve_agreement(node_id, id).await
            }
            AgreementAction::RejectAgreement { id, reason } => {
                reactions.reject_agreement(node_id, id, reason).await
            }
        };

        if record.is_finished() {
            break;
        }
    }
}
