use abi_stable::std_types::RStr;
use anyhow::anyhow;
use std::path::{Path, PathBuf};

use crate::interface::{load_library, BoxedSharedNegotiatorAPI};

use serde_json::Value;
use ya_agreement_utils::{AgreementView, OfferTemplate, ProposalView};
use ya_negotiator_component::component::{
    AgreementResult, NegotiationResult, NegotiatorComponent, PostTerminateEvent, Score,
};

#[derive(thiserror::Error, Debug)]
pub enum SharedLibError {
    #[error("[Negotiator Error] Failed to serialize/deserialize params on DLL boundary. {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("{0}")]
    Negotiation(String),
    #[error("Failed to serialize negotiator config. {0}")]
    InvalidConfig(#[from] serde_yaml::Error),
    #[error("Failed to initialize negotiator '{0}'. {1}")]
    Initialization(String, String),
}

/// Negotiator loaded from shared library.
pub struct SharedLibNegotiator {
    negotiator: BoxedSharedNegotiatorAPI,
}

impl SharedLibNegotiator {
    pub fn new(
        path: &Path,
        negotiator_name: &str,
        config: serde_yaml::Value,
        working_dir: PathBuf,
    ) -> anyhow::Result<Box<dyn NegotiatorComponent>> {
        let config = serde_yaml::to_string(&config).map_err(SharedLibError::from)?;
        let working_dir = working_dir
            .to_path_buf()
            .to_str()
            .ok_or(anyhow!(
                "Failed to convert path: {} to string.",
                working_dir.display()
            ))?
            .to_string();

        let library = load_library(path)?;
        let negotiator = library.create_negotiator()(
            RStr::from_str(negotiator_name),
            RStr::from_str(&config),
            RStr::from_str(&working_dir),
        )
        .into_result()
        .map_err(|e| {
            SharedLibError::Initialization(negotiator_name.to_string(), e.into_string())
        })?;

        Ok(Box::new(SharedLibNegotiator { negotiator }))
    }
}

impl NegotiatorComponent for SharedLibNegotiator {
    fn negotiate_step(
        &mut self,
        demand: &ProposalView,
        offer: ProposalView,
        score: Score,
    ) -> anyhow::Result<NegotiationResult> {
        let demand = serde_json::to_string(&demand).map_err(SharedLibError::from)?;
        let offer = serde_json::to_string(&offer).map_err(SharedLibError::from)?;
        let score = serde_json::to_string(&score).map_err(SharedLibError::from)?;

        let result = self
            .negotiator
            .negotiate_step(
                &RStr::from_str(&demand),
                &RStr::from_str(&offer),
                &RStr::from_str(&score),
            )
            .into_result()
            .map_err(|e| SharedLibError::Negotiation(e.into_string()))?;

        Ok(serde_json::from_str(&result).map_err(SharedLibError::from)?)
    }

    fn fill_template(&mut self, offer_template: OfferTemplate) -> anyhow::Result<OfferTemplate> {
        let constraints = offer_template.constraints;
        let properties =
            serde_json::to_string(&offer_template.properties).map_err(SharedLibError::from)?;

        let result = self
            .negotiator
            .fill_template(&RStr::from_str(&properties), &RStr::from_str(&constraints))
            .into_result()
            .map_err(|e| SharedLibError::Negotiation(e.into_string()))?;
        Ok(serde_json::from_str(result.as_str()).map_err(SharedLibError::from)?)
    }

    fn on_agreement_terminated(
        &mut self,
        agreement_id: &str,
        result: &AgreementResult,
    ) -> anyhow::Result<()> {
        let result = serde_json::to_string(&result).map_err(SharedLibError::from)?;

        Ok(self
            .negotiator
            .on_agreement_terminated(&RStr::from_str(agreement_id), &RStr::from_str(&result))
            .into_result()
            .map_err(|e| SharedLibError::Negotiation(e.into_string()))?)
    }

    fn on_agreement_approved(&mut self, agreement: &AgreementView) -> anyhow::Result<()> {
        let agreement = serde_json::to_string(&agreement).map_err(SharedLibError::from)?;

        Ok(self
            .negotiator
            .on_agreement_approved(&RStr::from_str(&agreement))
            .into_result()
            .map_err(|e| SharedLibError::Negotiation(e.into_string()))?)
    }

    fn on_proposal_rejected(&mut self, proposal_id: &str) -> anyhow::Result<()> {
        Ok(self
            .negotiator
            .on_proposal_rejected(&RStr::from_str(&proposal_id))
            .into_result()
            .map_err(|e| SharedLibError::Negotiation(e.into_string()))?)
    }

    fn on_post_terminate_event(
        &mut self,
        agreement_id: &str,
        event: &PostTerminateEvent,
    ) -> anyhow::Result<()> {
        let event = serde_json::to_string(&event).map_err(SharedLibError::from)?;
        Ok(self
            .negotiator
            .on_post_terminate_event(&RStr::from_str(&agreement_id), &RStr::from_str(&event))
            .into_result()
            .map_err(|e| SharedLibError::Negotiation(e.into_string()))?)
    }

    fn control_event(
        &mut self,
        component: &str,
        params: Value,
    ) -> anyhow::Result<serde_json::Value> {
        let params = serde_json::to_string(&params).map_err(SharedLibError::from)?;
        let result = self
            .negotiator
            .control_event(&RStr::from_str(&component), &RStr::from_str(&params))
            .into_result()
            .map_err(|e| SharedLibError::Negotiation(e.into_string()))?;

        Ok(serde_json::from_str(result.as_str()).map_err(SharedLibError::from)?)
    }
}
