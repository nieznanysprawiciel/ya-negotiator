use anyhow::bail;
use serde::{Deserialize, Serialize};

use ya_negotiator_component::static_lib::{NegotiatorFactoryDefault, NegotiatorMut};
use ya_negotiator_component::{
    AgreementEvent, AgreementResult, AgreementView, NegotiationResult, NegotiatorComponentMut,
    OfferTemplate, ProposalView, Score,
};

#[derive(Default)]
pub struct EmitErrors {
    next_error: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct AddError(pub String);

impl NegotiatorFactoryDefault<EmitErrors> for EmitErrors {
    type Type = NegotiatorMut;
}

impl NegotiatorComponentMut for EmitErrors {
    fn negotiate_step(
        &mut self,
        _their: &ProposalView,
        template: ProposalView,
        score: Score,
    ) -> anyhow::Result<NegotiationResult> {
        if self.next_error.is_empty() {
            log::info!("EmitErrors: Returning Ok, since no errors in queue.");

            Ok(NegotiationResult::Ready {
                proposal: template,
                score,
            })
        } else {
            bail!(self.next_error.pop().unwrap())
        }
    }

    /// Check documentation for `NegotiatorComponent::fill_template`.
    fn fill_template(&mut self, template: OfferTemplate) -> anyhow::Result<OfferTemplate> {
        if self.next_error.is_empty() {
            log::info!("EmitErrors: Returning Ok, since no errors in queue.");
            Ok(template)
        } else {
            bail!(self.next_error.pop().unwrap())
        }
    }

    /// Check documentation for `NegotiatorComponent::on_agreement_terminated`.
    fn on_agreement_terminated(
        &mut self,
        _agreement_id: &str,
        _result: &AgreementResult,
    ) -> anyhow::Result<()> {
        if self.next_error.is_empty() {
            log::info!("EmitErrors: Returning Ok, since no errors in queue.");
            Ok(())
        } else {
            bail!(self.next_error.pop().unwrap())
        }
    }

    /// Check documentation for `NegotiatorComponent::on_agreement_approved`.
    fn on_agreement_approved(&mut self, _agreement: &AgreementView) -> anyhow::Result<()> {
        if self.next_error.is_empty() {
            log::info!("EmitErrors: Returning Ok, since no errors in queue.");
            Ok(())
        } else {
            bail!(self.next_error.pop().unwrap())
        }
    }

    /// Check documentation for `NegotiatorComponent::on_proposal_rejected`.
    fn on_proposal_rejected(&mut self, _proposal_id: &str) -> anyhow::Result<()> {
        if self.next_error.is_empty() {
            log::info!("EmitErrors: Returning Ok, since no errors in queue.");
            Ok(())
        } else {
            bail!(self.next_error.pop().unwrap())
        }
    }

    /// Check documentation for `NegotiatorComponent::on_agreement_event`.
    fn on_agreement_event(
        &mut self,
        _agreement_id: &str,
        _event: &AgreementEvent,
    ) -> anyhow::Result<()> {
        if self.next_error.is_empty() {
            log::info!("EmitErrors: Returning Ok, since no errors in queue.");
            Ok(())
        } else {
            bail!(self.next_error.pop().unwrap())
        }
    }

    /// Check documentation for `NegotiatorComponent::control_event`.
    fn control_event(
        &mut self,
        _component: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        let add: AddError = serde_json::from_value(params)?;

        log::info!("EmitErrors: Adding error to queue: {}", add.0);
        self.next_error.insert(0, add.0);
        log::info!("EmitErrors: Num events in queue: {}", self.next_error.len());

        Ok(serde_json::Value::Null)
    }
}
