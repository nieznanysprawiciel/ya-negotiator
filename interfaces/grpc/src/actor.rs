use actix::prelude::*;
use futures::FutureExt;
use std::sync::Arc;

use crate::message::{NegotiationMessage, NegotiationResponse};
use ya_negotiator_component::NegotiatorComponent;

/// Responsible for handling messages to single specific Negotiator.
/// This is adapter between `Send` GrpcNegotiatorServer and `?Send` `NegotiatorComponent`.
pub struct NegotiatorWrapper {
    negotiator: Arc<Box<dyn NegotiatorComponent>>,
}

impl Actor for NegotiatorWrapper {
    type Context = Context<Self>;
}

impl Handler<NegotiationMessage> for NegotiatorWrapper {
    type Result = ResponseFuture<anyhow::Result<NegotiationResponse>>;

    fn handle(&mut self, msg: NegotiationMessage, _ctx: &mut Self::Context) -> Self::Result {
        let negotiator = self.negotiator.clone();
        async move {
            match msg {
                NegotiationMessage::FillTemplate { template } => negotiator
                    .fill_template(template)
                    .await
                    .map(NegotiationResponse::from),
                NegotiationMessage::NegotiateStep {
                    their,
                    template,
                    score,
                } => negotiator
                    .negotiate_step(&their, template, score)
                    .await
                    .map(NegotiationResponse::from),
                NegotiationMessage::AgreementSigned { agreement } => negotiator
                    .on_agreement_approved(&agreement)
                    .await
                    .map(|_| NegotiationResponse::Empty),
                NegotiationMessage::AgreementTerminated {
                    agreement_id,
                    result,
                } => negotiator
                    .on_agreement_terminated(&agreement_id, &result)
                    .await
                    .map(|_| NegotiationResponse::Empty),
                NegotiationMessage::ProposalRejected {
                    proposal_id,
                    reason: _,
                } => negotiator
                    .on_proposal_rejected(&proposal_id)
                    .await
                    .map(|_| NegotiationResponse::Empty),
                NegotiationMessage::AgreementEvent {
                    agreement_id,
                    event,
                } => negotiator
                    .on_agreement_event(&agreement_id, &event)
                    .await
                    .map(|_| NegotiationResponse::Empty),
                NegotiationMessage::ControlEvent { component, params } => negotiator
                    .control_event(&component, params)
                    .await
                    .map(NegotiationResponse::from),
            }
        }
        .boxed_local()
    }
}
