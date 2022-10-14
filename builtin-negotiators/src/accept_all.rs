use std::path::PathBuf;
use ya_negotiator_component::static_lib::{NegotiatorAsync, NegotiatorFactory};
use ya_negotiator_component::NegotiatorComponent;

/// Negotiator that accepts every incoming Proposal.
pub struct AcceptAll {}

impl NegotiatorFactory<AcceptAll> for AcceptAll {
    type Type = NegotiatorAsync;

    fn new(
        _name: &str,
        _config: serde_yaml::Value,
        _working_dir: PathBuf,
    ) -> anyhow::Result<AcceptAll> {
        Ok(AcceptAll {})
    }
}

impl NegotiatorComponent for AcceptAll {}
