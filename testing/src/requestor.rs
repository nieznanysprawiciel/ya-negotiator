use ya_agreement_utils::AgreementView;
use ya_negotiators::{AgreementAction, ProposalAction};

use ya_client_model::market::proposal::State;
use ya_client_model::market::{NewProposal, Reason};
use ya_client_model::NodeId;

use crate::error::NegotiatorError;
use crate::negotiation_record::NegotiationRecordSync;
use crate::node::Node;

use backtrace::Backtrace;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::Arc;
use tokio::stream::{StreamExt, StreamMap};

/// Receives Proposal and Agreement reactions from negotiators and processes them.
/// This simulates Requestor Agent expected behavior.
pub struct RequestorReactions {
    pub providers: HashMap<NodeId, Arc<Node>>,
    pub requestors: HashMap<NodeId, Arc<Node>>,
    pub record: NegotiationRecordSync,
}

impl RequestorReactions {
    /// On Requestor side accepting Proposal means proposing Agreement.
    pub async fn accept_proposal(
        &self,
        node_id: NodeId,
        proposal_id: String,
    ) -> anyhow::Result<()> {
        log::info!(
            "Processing Requestor [{}] accept_proposal for Proposal {}",
            node_id,
            proposal_id
        );

        let record = self.record.clone();
        let requestor = self.get_requestor(&node_id)?;

        let prov_proposal = record.get_proposal(&proposal_id)?;
        let provider = self.get_provider(&prov_proposal.issuer_id)?;

        let prev_req_proposal =
            record.get_proposal(&prov_proposal.prev_proposal_id.clone().ok_or(
                NegotiatorError::NoPrevProposal {
                    id: proposal_id.to_string(),
                    trace: format!("{:?}", Backtrace::new()),
                },
            )?)?;

        let req_proposal = requestor.recounter_proposal(&proposal_id, &prev_req_proposal);

        // Register event.
        record.accept(req_proposal.clone(), prov_proposal.issuer_id);

        // It means, we are countering Initial Proposal, so we can't create Agreement
        // without at least one step of negotiations.
        if let None = prev_req_proposal.prev_proposal_id {
            log::info!(
                "Requestor [{}] sends counter Proposal {} to {}",
                node_id,
                req_proposal.proposal_id,
                provider.node_id
            );

            if let Err(e) = provider
                .react_to_proposal(&req_proposal, &prov_proposal)
                .await
            {
                record.error(prov_proposal.issuer_id, req_proposal.issuer_id, e.into());
            }
            return Ok(());
        }

        log::info!("Creating Agreement on Requestor [{}].", node_id,);

        let agreement = requestor.create_agreement(&req_proposal, &prov_proposal);
        let agreement = AgreementView::try_from(&agreement).unwrap();

        record.propose_agreement(agreement.clone());

        log::info!(
            "Requestor [{}] will react to Agreement {}",
            node_id,
            agreement.id,
        );

        // Requestor will asynchronously send message, that he wants too send this Agreement to Provider.
        if let Err(e) = requestor.react_to_agreement(&agreement).await {
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
        log::info!(
            "Processing Requestor [{}] reject_proposal for Proposal {}",
            node_id,
            proposal_id
        );

        let record = self.record.clone();
        let prov_proposal = record.get_proposal(&proposal_id)?;

        record.reject(node_id, prov_proposal, reason);

        // We could notify Requestor, if Component API would allow it.
        Ok(())
    }

    pub async fn counter_proposal(
        &self,
        node_id: NodeId,
        proposal_id: String,
        proposal: NewProposal,
    ) -> anyhow::Result<()> {
        log::info!(
            "Processing Requestor [{}] counter_proposal for Proposal {}",
            node_id,
            proposal_id
        );

        let record = self.record.clone();
        let requestor = self.get_requestor(&node_id)?;

        let prov_proposal = record.get_proposal(&proposal_id)?;
        let provider = self.get_provider(&prov_proposal.issuer_id)?;

        let proposal = requestor.into_proposal(proposal, State::Draft, Some(proposal_id));

        // Register event.
        record.counter(proposal.clone(), prov_proposal.issuer_id);

        if let Err(e) = provider.react_to_proposal(&proposal, &prov_proposal).await {
            record.error(prov_proposal.issuer_id, proposal.issuer_id, e.into());
        }
        Ok(())
    }

    /// Approve Agreement on Requestor side means, that Agreement will be confirmed
    /// (and sent to Provider).
    pub async fn approve_agreement(
        &self,
        node_id: NodeId,
        agreement_id: String,
    ) -> anyhow::Result<()> {
        log::info!(
            "Processing Requestor [{}] approve_agreement for Agreement {}",
            node_id,
            agreement_id
        );

        let record = self.record.clone();
        let agreement = record.get_agreement(&agreement_id)?;
        let provider_id = agreement.provider_id()?.clone();
        let provider = self.get_provider(&provider_id)?;

        record.approve(agreement.clone());

        log::info!(
            "Requestor [{}] will send Agreement {} to {}",
            node_id,
            agreement.id,
            provider_id
        );

        if let Err(e) = provider.react_to_agreement(&agreement).await {
            record.error(provider_id, node_id, e.into());
        }
        Ok(())
    }

    pub async fn reject_agreement(
        &self,
        node_id: NodeId,
        agreement_id: String,
        reason: Option<Reason>,
    ) -> anyhow::Result<()> {
        log::info!(
            "Processing Requestor [{}] reject_agreement for Agreement {}",
            node_id,
            agreement_id
        );

        let record = self.record.clone();
        let agreement = record.get_agreement(&agreement_id).unwrap();

        record.reject_agreement(agreement, reason);
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
            .ok_or(NegotiatorError::RequestorNotFound {
                node_id: id.clone(),
                trace: format!("{:?}", Backtrace::new()),
            })
    }
}

pub async fn requestor_proposals_processor(
    providers: HashMap<NodeId, Arc<Node>>,
    requestors: HashMap<NodeId, Arc<Node>>,
    record: NegotiationRecordSync,
) {
    let mut r_receivers = StreamMap::new();

    requestors.iter().for_each(|(_, node)| {
        r_receivers.insert(
            node.node_id,
            Box::pin(node.proposal_channel().into_stream()),
        );
    });

    let reactions = RequestorReactions {
        record: record.clone(),
        requestors,
        providers,
    };

    while let Some((node_id, Ok(action))) = r_receivers.next().await {
        match action {
            ProposalAction::AcceptProposal { id } => reactions.accept_proposal(node_id, id).await,
            ProposalAction::CounterProposal { id, proposal } => {
                reactions.counter_proposal(node_id, id, proposal).await
            }
            ProposalAction::RejectProposal { id, reason } => {
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

pub async fn requestor_agreements_processor(
    providers: HashMap<NodeId, Arc<Node>>,
    requestors: HashMap<NodeId, Arc<Node>>,
    record: NegotiationRecordSync,
) {
    let mut r_receivers = StreamMap::new();

    requestors.iter().for_each(|(_, node)| {
        r_receivers.insert(
            node.node_id,
            Box::pin(node.agreement_channel().into_stream()),
        );
    });

    let reactions = RequestorReactions {
        record: record.clone(),
        requestors,
        providers,
    };

    while let Some((node_id, Ok(action))) = r_receivers.next().await {
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
