use ya_negotiator_component::component::NegotiatorComponent;
use ya_negotiator_component::transparent_impl;

/// Negotiator that accepts every incoming Proposal.
pub struct AcceptAll {}

impl AcceptAll {
    pub fn new(_config: serde_yaml::Value) -> anyhow::Result<AcceptAll> {
        Ok(AcceptAll {})
    }
}

impl NegotiatorComponent for AcceptAll {
    transparent_impl!(negotiate_step);
    transparent_impl!(fill_template);
    transparent_impl!(on_agreement_terminated);
    transparent_impl!(on_agreement_approved);
    transparent_impl!(on_proposal_rejected);
    transparent_impl!(on_post_terminate_event);
    transparent_impl!(control_event);
}
