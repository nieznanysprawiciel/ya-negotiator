use actix::prelude::*;
use actix::Actor;
use anyhow::Result;
use derive_more::Display;
use serde::{Deserialize, Serialize};

use ya_agreement_utils::{AgreementView, OfferTemplate};
use ya_client_model::market::{NewOffer, NewProposal, Proposal, Reason};

use crate::component::AgreementResult;
use crate::Negotiator;
use ya_negotiator_component::component::AgreementEvent;

/// Response for requestor proposals.
#[derive(Debug, Clone, Display, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum ProposalAction {
    #[display(fmt = "CounterProposal")]
    CounterProposal {
        id: String,
        subscription_id: String,
        proposal: NewProposal,
    },
    #[display(fmt = "AcceptProposal")]
    AcceptProposal { id: String, subscription_id: String },
    #[display(
        fmt = "RejectProposal [{}]{}",
        id,
        "reason.as_ref().map(|r| format!(\" (reason: {})\", r)).unwrap_or(\"\".into())"
    )]
    RejectProposal {
        subscription_id: String,
        id: String,
        reason: Option<Reason>,
    },
}

/// Response for requestor agreements.
#[derive(Debug, Clone, Display, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum AgreementAction {
    #[display(fmt = "ApproveAgreement")]
    ApproveAgreement { id: String, subscription_id: String },
    #[display(
        fmt = "RejectAgreement [{}]{}",
        id,
        "reason.as_ref().map(|r| format!(\" (reason: {})\", r)).unwrap_or(\"\".into())"
    )]
    RejectAgreement {
        id: String,
        subscription_id: String,
        reason: Option<Reason>,
    },
}

// =========================================== //
// Negotiator interface
// =========================================== //

/// Negotiator can modify offer, that was generated for him. He can save
/// information about this offer, that are necessary for negotiations.
#[derive(Message)]
#[rtype(result = "Result<NewOffer>")]
pub struct CreateOffer {
    pub offer_template: OfferTemplate,
}

/// Reactions to events from market. These function make market decisions
/// related to incoming Proposals.
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct ReactToProposal {
    pub subscription_id: String,
    /// It is new proposal that we got from other party.
    pub incoming_proposal: Proposal,
    /// It is always our proposal that we sent last time.
    pub our_prev_proposal: Proposal,
}

/// Reactions to events from market. These function make market decisions
/// related to incoming Agreements.
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct ReactToAgreement {
    pub subscription_id: String,
    pub agreement: AgreementView,
}

/// Agreement was successfully signed by both parties.
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct AgreementSigned {
    pub agreement: AgreementView,
}

/// Agreement finished notifications. Negotiator can adjust his strategy based on it.
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct AgreementFinalized {
    pub agreement_id: String,
    pub result: AgreementResult,
}

/// Notification about what happened to Agreement after termination.
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct PostAgreementEvent {
    pub agreement_id: String,
    pub event: AgreementEvent,
}

/// Proposal was rejected by other party.
#[derive(Message)]
#[rtype(result = "Result<()>")]
pub struct ProposalRejected {
    pub proposal_id: String,
    pub reason: Option<Reason>,
}

/// Message for controlling chosen component.
#[derive(Message)]
#[rtype(result = "Result<serde_json::Value>")]
pub struct ControlEvent {
    pub component: String,
    pub params: serde_json::Value,
}

/// Negotiator should provide expected number of Agreements.
#[derive(Message)]
#[rtype(result = "()")]
pub struct RequestAgreements(pub usize);

// TODO: Consider, if this struct is helpful at all and remove if not.
#[derive(Clone)]
pub struct NegotiatorAddr(pub Addr<Negotiator>);

impl NegotiatorAddr {
    pub async fn create_offer(&self, template: &OfferTemplate) -> Result<NewProposal> {
        self.0
            .send(CreateOffer {
                offer_template: template.clone(),
            })
            .await?
    }

    pub async fn react_to_proposal(
        &self,
        subscription_id: &str,
        incoming_proposal: &Proposal,
        our_proposal: &Proposal,
    ) -> Result<()> {
        self.0
            .send(ReactToProposal {
                subscription_id: subscription_id.to_string(),
                incoming_proposal: incoming_proposal.clone(),
                our_prev_proposal: our_proposal.clone(),
            })
            .await?
    }

    pub async fn react_to_agreement(
        &self,
        subscription_id: &str,
        agreement_view: &AgreementView,
    ) -> Result<()> {
        self.0
            .send(ReactToAgreement {
                subscription_id: subscription_id.to_string(),
                agreement: agreement_view.clone(),
            })
            .await?
    }

    pub async fn agreement_signed(&self, agreement_view: &AgreementView) -> Result<()> {
        self.0
            .send(AgreementSigned {
                agreement: agreement_view.clone(),
            })
            .await?
    }

    pub async fn agreement_finalized(
        &self,
        agreement_id: &str,
        result: AgreementResult,
    ) -> Result<()> {
        self.0
            .send(AgreementFinalized {
                agreement_id: agreement_id.to_string(),
                result,
            })
            .await?
    }

    pub async fn proposal_rejected(
        &self,
        proposal_id: &str,
        reason: &Option<Reason>,
    ) -> Result<()> {
        self.0
            .send(ProposalRejected {
                proposal_id: proposal_id.to_string(),
                reason: reason.clone(),
            })
            .await?
    }

    pub async fn post_agreement_event(
        &self,
        agreement_id: &str,
        event: AgreementEvent,
    ) -> Result<()> {
        self.0
            .send(PostAgreementEvent {
                agreement_id: agreement_id.to_string(),
                event,
            })
            .await?
    }

    pub async fn control_event(
        &self,
        component: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.0
            .send(ControlEvent {
                component: component.to_string(),
                params,
            })
            .await?
    }

    pub async fn request_agreements(&self, count: usize) -> Result<()> {
        Ok(self.0.send(RequestAgreements(count)).await?)
    }

    pub fn from(negotiator: Negotiator) -> NegotiatorAddr {
        NegotiatorAddr(negotiator.start())
    }
}

impl ProposalAction {
    pub fn id(&self) -> String {
        match &self {
            ProposalAction::CounterProposal { id, .. } => id.clone(),
            ProposalAction::AcceptProposal { id, .. } => id.clone(),
            ProposalAction::RejectProposal { id, .. } => id.clone(),
        }
    }
}

impl AgreementAction {
    pub fn id(&self) -> String {
        match &self {
            AgreementAction::ApproveAgreement { id, .. } => id.clone(),
            AgreementAction::RejectAgreement { id, .. } => id.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proposal_response_display() {
        let reason = ProposalAction::RejectProposal {
            id: "".to_string(),
            subscription_id: "".to_string(),
            reason: Some("zima".into()),
        };
        let no_reason = ProposalAction::RejectProposal {
            id: "".to_string(),
            subscription_id: "".to_string(),
            reason: None,
        };

        assert_eq!(reason.to_string(), "RejectProposal [] (reason: 'zima')");
        assert_eq!(no_reason.to_string(), "RejectProposal []");
    }

    #[test]
    fn test_agreement_response_display() {
        let reason = AgreementAction::RejectAgreement {
            id: "".to_string(),
            subscription_id: "".to_string(),
            reason: Some("lato".into()),
        };
        let no_reason = AgreementAction::RejectAgreement {
            id: "".to_string(),
            subscription_id: "".to_string(),
            reason: None,
        };

        assert_eq!(reason.to_string(), "RejectAgreement [] (reason: 'lato')");
        assert_eq!(no_reason.to_string(), "RejectAgreement []");
    }
}
