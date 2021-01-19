use ya_agreement_utils::{AgreementView, OfferTemplate, ProposalView};
use ya_negotiator_component::component::{AgreementResult, NegotiationResult, NegotiatorComponent};

/// Negotiator that accepts every incoming Proposal.
pub struct AcceptAll {}

impl AcceptAll {
    pub fn new(_config: serde_yaml::Value) -> anyhow::Result<AcceptAll> {
        Ok(AcceptAll {})
    }
}

impl NegotiatorComponent for AcceptAll {
    fn negotiate_step(
        &mut self,
        _demand: &ProposalView,
        offer: ProposalView,
    ) -> anyhow::Result<NegotiationResult> {
        Ok(NegotiationResult::Ready { proposal: offer })
    }

    fn fill_template(&mut self, offer_template: OfferTemplate) -> anyhow::Result<OfferTemplate> {
        Ok(offer_template)
    }

    fn on_agreement_terminated(
        &mut self,
        _agreement_id: &str,
        _result: &AgreementResult,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_agreement_approved(&mut self, _agreement: &AgreementView) -> anyhow::Result<()> {
        Ok(())
    }
}
