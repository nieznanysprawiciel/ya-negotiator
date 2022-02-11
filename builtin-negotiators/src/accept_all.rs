use ya_negotiator_component::component::NegotiatorComponent;

/// Negotiator that accepts every incoming Proposal.
pub struct AcceptAll {}

impl AcceptAll {
    pub fn new(_config: serde_yaml::Value) -> anyhow::Result<AcceptAll> {
        Ok(AcceptAll {})
    }
}

impl NegotiatorComponent for AcceptAll {}
