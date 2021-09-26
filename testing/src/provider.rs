use ya_negotiators::{Action, AgreementAction, AgreementResult};

use ya_client_model::market::proposal::State;
use ya_client_model::market::{NewProposal, Reason};
use ya_client_model::NodeId;

use crate::negotiation_record::NegotiationRecordSync;
use crate::node::Node;

use crate::error::NegotiatorError;
use backtrace::Backtrace;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::stream::{StreamExt, StreamMap};

pub struct ProviderReactions {
    pub providers: HashMap<NodeId, Arc<Node>>,
    pub requestors: HashMap<NodeId, Arc<Node>>,
    pub record: NegotiationRecordSync,
}

impl ProviderReactions {
    pub async fn accept_proposal(
        &self,
        node_id: NodeId,
        proposal_id: String,
    ) -> anyhow::Result<()> {
        let record = self.record.clone();
        let provider = self.get_provider(&node_id)?;

        let req_proposal = record.get_proposal(&proposal_id)?;
        let requestor = self.get_requestor(&req_proposal.issuer_id)?;

        let prev_prov_proposal =
            record.get_proposal(&req_proposal.prev_proposal_id.clone().unwrap())?;

        let prov_proposal = provider.recounter_proposal(&proposal_id, &prev_prov_proposal);

        // Register event.
        record.accept(prov_proposal.clone(), req_proposal.issuer_id);

        if let Err(e) = requestor
            .react_to_proposal(&prov_proposal, &req_proposal)
            .await
        {
            record.error(req_proposal.issuer_id, prov_proposal.issuer_id, e.into());
        }
        Ok(())
    }

    pub async fn reject_proposal(
        &self,
        node_id: NodeId,
        proposal_id: String,
        reason: Option<Reason>,
    ) -> anyhow::Result<()> {
        let record = self.record.clone();
        let req_proposal = record.get_proposal(&proposal_id)?;

        record.reject(node_id, req_proposal, reason);

        // We could notify Requestor, if Component API would allow it.
        Ok(())
    }

    pub async fn counter_proposal(
        &self,
        node_id: NodeId,
        proposal_id: String,
        proposal: NewProposal,
    ) -> anyhow::Result<()> {
        let record = self.record.clone();
        let provider = self.get_provider(&node_id)?;
        let req_proposal = record.get_proposal(&proposal_id)?;
        let requestor = self.get_requestor(&req_proposal.issuer_id)?;

        let proposal = provider.into_proposal(proposal, State::Draft);

        // Register event.
        record.counter(proposal.clone(), req_proposal.issuer_id);

        if let Err(e) = requestor.react_to_proposal(&proposal, &req_proposal).await {
            record.error(req_proposal.issuer_id, proposal.issuer_id, e.into())
        }
        Ok(())
    }

    pub async fn approve_agreement(
        &self,
        node_id: NodeId,
        agreement_id: String,
    ) -> anyhow::Result<()> {
        let record = self.record.clone();

        let agreement = record.get_agreement(&agreement_id)?;
        let provider = self.get_provider(&node_id)?;
        let requestor = self.get_requestor(&agreement.requestor_id()?)?;

        record.approve(agreement.clone());

        let r_result = requestor.agreement_signed(&agreement).await;
        let p_result = provider.agreement_signed(&agreement).await;

        if let Err(e) = r_result {
            record.error(requestor.node_id, provider.node_id, e.into())
        }

        if let Err(e) = p_result {
            record.error(provider.node_id, requestor.node_id, e.into())
        }
        Ok(())
    }

    pub async fn reject_agreement(
        &self,
        node_id: NodeId,
        agreement_id: String,
        reason: Option<Reason>,
    ) -> anyhow::Result<()> {
        let record = self.record.clone();
        let agreement = record.get_agreement(&agreement_id)?;
        let requestor = self.get_requestor(&agreement.requestor_id()?)?;
        let agreement_id = agreement.id.clone();

        record.reject_agreement(agreement, reason);

        if let Err(e) = requestor
            .agreement_finalized(&agreement_id, AgreementResult::ApprovalFailed)
            .await
        {
            record.error(requestor.node_id, node_id, e.into())
        }
        Ok(())
    }

    pub fn get_provider(&self, id: &NodeId) -> Result<Arc<Node>, NegotiatorError> {
        self.providers
            .get(id)
            .cloned()
            .ok_or(NegotiatorError::ProviderNotFound {
                node_id: id.clone(),
                trace: format!("{:?}", Backtrace::new()),
            })
    }

    pub fn get_requestor(&self, id: &NodeId) -> Result<Arc<Node>, NegotiatorError> {
        self.requestors
            .get(id)
            .cloned()
            .ok_or(NegotiatorError::ProviderNotFound {
                node_id: id.clone(),
                trace: format!("{:?}", Backtrace::new()),
            })
    }
}

pub async fn provider_proposals_processor(
    providers: HashMap<NodeId, Arc<Node>>,
    requestors: HashMap<NodeId, Arc<Node>>,
    record: NegotiationRecordSync,
) {
    let mut p_receivers = StreamMap::new();

    providers.iter().for_each(|(_, node)| {
        p_receivers.insert(
            node.node_id,
            Box::pin(node.proposal_channel().into_stream()),
        );
    });

    let reactions = ProviderReactions {
        record: record.clone(),
        requestors,
        providers,
    };

    while let Some((node_id, Ok(action))) = p_receivers.next().await {
        match action {
            Action::AcceptProposal { id } => reactions.accept_proposal(node_id, id).await,
            Action::CounterProposal { id, proposal } => {
                reactions.counter_proposal(node_id, id, proposal).await
            }
            Action::RejectProposal { id, reason } => {
                reactions.reject_proposal(node_id, id, reason).await
            }
        }
        .map_err(|e| record.node_error(node_id, e))
        .ok();

        if record.is_finished() {
            break;
        }
    }
}

pub async fn provider_agreements_processor(
    providers: HashMap<NodeId, Arc<Node>>,
    requestors: HashMap<NodeId, Arc<Node>>,
    record: NegotiationRecordSync,
) {
    let mut p_receivers = StreamMap::new();

    providers.iter().for_each(|(_, node)| {
        p_receivers.insert(
            node.node_id,
            Box::pin(node.agreement_channel().into_stream()),
        );
    });

    let reactions = ProviderReactions {
        record: record.clone(),
        requestors,
        providers,
    };

    while let Some((node_id, Ok(action))) = p_receivers.next().await {
        match action {
            AgreementAction::ApproveAgreement { id } => {
                reactions.approve_agreement(node_id, id).await
            }
            AgreementAction::RejectAgreement { id, reason } => {
                reactions.reject_agreement(node_id, id, reason).await
            }
        }
        .map_err(|e| record.node_error(node_id, e))
        .ok();

        if record.is_finished() {
            break;
        }
    }
}
