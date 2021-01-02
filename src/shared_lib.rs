use abi_stable::std_types::RStr;
use std::path::Path;

use ya_agreement_utils::OfferTemplate;
use ya_client_model::market::Reason;
use ya_negotiator_component::component::{
    AgreementResult, NegotiationResult, NegotiatorComponent, ProposalView,
};
use ya_negotiator_shared_lib_interface::{load_library, BoxedSharedNegotiatorAPI};

#[derive(thiserror::Error, Debug)]
pub enum SharedLibError {
    #[error("[Negotiator Error] Failed to serialize/deserialize params on DLL boundary. {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("{0}")]
    Negotiation(String),
    #[error("Failed to serialize negotiator config. {0}")]
    InvalidConfig(#[from] serde_yaml::Error),
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
    ) -> anyhow::Result<Box<dyn NegotiatorComponent>> {
        let config = serde_yaml::to_string(&config).map_err(SharedLibError::from)?;

        let library = load_library(path)?;
        let negotiator =
            library.create_negotiator()(RStr::from_str(negotiator_name), RStr::from_str(&config));

        Ok(Box::new(SharedLibNegotiator { negotiator }))
    }
}

impl NegotiatorComponent for SharedLibNegotiator {
    fn negotiate_step(&mut self, demand: &ProposalView, offer: ProposalView) -> NegotiationResult {
        match (|| {
            let demand = serde_json::to_string(&demand).map_err(SharedLibError::from)?;
            let offer = serde_json::to_string(&offer).map_err(SharedLibError::from)?;

            let result = self
                .negotiator
                .negotiate_step(&RStr::from_str(&demand), &RStr::from_str(&offer));

            Result::<NegotiationResult, SharedLibError>::Ok(
                serde_json::from_str(&result).map_err(SharedLibError::from)?,
            )
        })() {
            Ok(result) => result,
            // If we can't negotiate, we must reject proposal.
            Err(e) => NegotiationResult::Reject {
                reason: Some(Reason::new(e.to_string())),
            },
        }
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

    fn on_agreement_approved(&mut self, agreement_id: &str) -> anyhow::Result<()> {
        Ok(self
            .negotiator
            .on_agreement_approved(&RStr::from_str(agreement_id))
            .into_result()
            .map_err(|e| SharedLibError::Negotiation(e.into_string()))?)
    }
}
