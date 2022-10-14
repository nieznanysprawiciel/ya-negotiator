use actix::prelude::*;
use derive_more::From;
use serde::{Deserialize, Serialize};

use ya_agreement_utils::{AgreementView, OfferTemplate, ProposalView};
use ya_negotiator_component::{
    AgreementEvent, AgreementResult, NegotiationResult, RejectReason, Score,
};

/// `NegotiatorComponent` api expressed as enum.
/// Interchangeable format to pass between binaries.
#[derive(Message, Serialize, Deserialize, Clone, Debug)]
#[rtype(result = "anyhow::Result<NegotiationResponse>")]
#[non_exhaustive]
pub enum NegotiationMessage {
    FillTemplate {
        template: OfferTemplate,
    },
    NegotiateStep {
        their: ProposalView,
        template: ProposalView,
        score: Score,
    },
    AgreementSigned {
        agreement: AgreementView,
    },
    AgreementTerminated {
        agreement_id: String,
        result: AgreementResult,
    },
    ProposalRejected {
        proposal_id: String,
        reason: RejectReason,
    },
    AgreementEvent {
        agreement_id: String,
        event: AgreementEvent,
    },
    ControlEvent {
        component: String,
        params: serde_json::Value,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, From)]
#[non_exhaustive]
pub enum NegotiationResponse {
    #[from]
    OfferTemplate(OfferTemplate),
    #[from]
    NegotiationResult(NegotiationResult),
    #[from]
    Generic(serde_json::Value),
    #[from(types(()))]
    Empty,
}
