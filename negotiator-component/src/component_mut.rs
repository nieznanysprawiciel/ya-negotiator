use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use ya_agreement_utils::{AgreementView, OfferTemplate, ProposalView};

use crate::{AgreementEvent, AgreementResult, NegotiationResult, NegotiatorComponent, Score};

/// Adapter implementing `NegotiatorComponent` for `NegotiatorComponentMut`.
pub struct ComponentMutWrapper<N: NegotiatorComponentMut + Sized> {
    inner: Arc<Mutex<N>>,
}

/// Mutable version of negotiator component. It simplifies implementation in case someone
/// doesn't need asynchronous execution, but requires access to `&mut self`.
/// By using this trait you can avoid necessary synchronization, which is handled externally.
///
/// Remember that negotiators are ran in asynchronous environment, so you are not allowed
/// to do any heavy computational work here, that could block executor.
pub trait NegotiatorComponentMut {
    /// Check documentation for `NegotiatorComponent::negotiate_step`.
    fn negotiate_step(
        &mut self,
        _their: &ProposalView,
        template: ProposalView,
        score: Score,
    ) -> anyhow::Result<NegotiationResult> {
        Ok(NegotiationResult::Ready {
            proposal: template,
            score,
        })
    }

    /// Check documentation for `NegotiatorComponent::fill_template`.
    fn fill_template(&mut self, template: OfferTemplate) -> anyhow::Result<OfferTemplate> {
        Ok(template)
    }

    /// Check documentation for `NegotiatorComponent::on_agreement_terminated`.
    fn on_agreement_terminated(
        &mut self,
        _agreement_id: &str,
        _result: &AgreementResult,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Check documentation for `NegotiatorComponent::on_agreement_approved`.
    fn on_agreement_approved(&mut self, _agreement: &AgreementView) -> anyhow::Result<()> {
        Ok(())
    }

    /// Check documentation for `NegotiatorComponent::on_proposal_rejected`.
    fn on_proposal_rejected(&mut self, _proposal_id: &str) -> anyhow::Result<()> {
        Ok(())
    }

    /// Check documentation for `NegotiatorComponent::on_agreement_event`.
    fn on_agreement_event(
        &mut self,
        _agreement_id: &str,
        _event: &AgreementEvent,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Check documentation for `NegotiatorComponent::control_event`.
    fn control_event(
        &mut self,
        _component: &str,
        _params: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        Ok(serde_json::Value::Null)
    }

    /// Check documentation for `NegotiatorComponent::shutdown`.
    fn shutdown(&mut self, _timeout: Duration) -> anyhow::Result<()> {
        Ok(())
    }
}

#[async_trait(?Send)]
impl<N> NegotiatorComponent for ComponentMutWrapper<N>
where
    N: NegotiatorComponentMut + Sized,
{
    async fn negotiate_step(
        &self,
        their: &ProposalView,
        template: ProposalView,
        score: Score,
    ) -> anyhow::Result<NegotiationResult> {
        self.inner
            .lock()
            .await
            .negotiate_step(their, template, score)
    }

    async fn fill_template(&self, template: OfferTemplate) -> anyhow::Result<OfferTemplate> {
        self.inner.lock().await.fill_template(template)
    }

    async fn on_agreement_terminated(
        &self,
        agreement_id: &str,
        result: &AgreementResult,
    ) -> anyhow::Result<()> {
        self.inner
            .lock()
            .await
            .on_agreement_terminated(agreement_id, result)
    }

    async fn on_agreement_approved(&self, agreement: &AgreementView) -> anyhow::Result<()> {
        self.inner.lock().await.on_agreement_approved(agreement)
    }

    async fn on_proposal_rejected(&self, proposal_id: &str) -> anyhow::Result<()> {
        self.inner.lock().await.on_proposal_rejected(proposal_id)
    }

    async fn on_agreement_event(
        &self,
        agreement_id: &str,
        event: &AgreementEvent,
    ) -> anyhow::Result<()> {
        self.inner
            .lock()
            .await
            .on_agreement_event(agreement_id, event)
    }

    async fn control_event(
        &self,
        component: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        self.inner.lock().await.control_event(component, params)
    }

    async fn shutdown(&self, timeout: Duration) -> anyhow::Result<()> {
        self.inner.lock().await.shutdown(timeout)
    }
}

impl<N> ComponentMutWrapper<N>
where
    N: NegotiatorComponentMut + Sized,
{
    pub fn new(negotiator: N) -> Self {
        ComponentMutWrapper {
            inner: Arc::new(Mutex::new(negotiator)),
        }
    }
}
