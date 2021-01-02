use abi_stable::std_types::{RResult, RResult::ROk, RStr, RString};

use crate::interface::SharedNegotiatorAPI;

use crate::SharedLibError;

pub use ya_agreement_utils::OfferTemplate;
pub use ya_client_model::market::Reason;
pub use ya_negotiator_component::component::{
    AgreementResult, NegotiationResult, NegotiatorComponent, ProposalView,
};

pub struct NegotiatorWrapper<T: NegotiatorComponent + Sized> {
    component: T,
}

impl<T> SharedNegotiatorAPI for NegotiatorWrapper<T>
where
    T: NegotiatorComponent + Sized,
{
    fn negotiate_step(&mut self, demand: &RStr, offer: &RStr) -> RResult<RString, RString> {
        match (|| {
            let demand = serde_json::from_str(demand.as_str()).map_err(SharedLibError::from)?;
            let offer = serde_json::from_str(offer.as_str()).map_err(SharedLibError::from)?;

            let result = self
                .component
                .negotiate_step(&demand, offer)
                .map_err(|e| SharedLibError::Negotiation(e.to_string()))?;

            Result::<String, SharedLibError>::Ok(
                serde_json::to_string(&result).map_err(SharedLibError::from)?,
            )
        })() {
            Ok(result) => ROk(RString::from(result)),
            Err(e) => RResult::RErr(RString::from(e.to_string())),
        }
    }

    fn fill_template(
        &mut self,
        template_props: &RStr,
        template_constraints: &RStr,
    ) -> RResult<RString, RString> {
        match (|| {
            let properties =
                serde_json::from_str(template_props.as_str()).map_err(SharedLibError::from)?;
            let constraints = template_constraints.to_string();

            let template = OfferTemplate {
                constraints,
                properties,
            };

            let result = self
                .component
                .fill_template(template)
                .map_err(|e| SharedLibError::Negotiation(e.to_string()))?;

            Result::<String, SharedLibError>::Ok(
                serde_json::to_string(&result).map_err(SharedLibError::from)?,
            )
        })() {
            Ok(result) => ROk(RString::from(result)),
            Err(e) => RResult::RErr(RString::from(e.to_string())),
        }
    }

    fn on_agreement_terminated(
        &mut self,
        agreement_id: &RStr,
        result: &RStr,
    ) -> RResult<(), RString> {
        match (|| {
            let result = serde_json::from_str(result.as_str()).map_err(SharedLibError::from)?;
            self.component
                .on_agreement_terminated(agreement_id.as_str(), &result)
                .map_err(|e| SharedLibError::Negotiation(e.to_string()))?;
            Result::<(), SharedLibError>::Ok(())
        })() {
            Ok(_) => ROk(()),
            Err(e) => RResult::RErr(RString::from(e.to_string())),
        }
    }

    fn on_agreement_approved(&mut self, agreement_id: &RStr) -> RResult<(), RString> {
        match self
            .component
            .on_agreement_approved(agreement_id.as_str())
            .map_err(|e| SharedLibError::Negotiation(e.to_string()))
        {
            Ok(()) => ROk(()),
            Err(e) => RResult::RErr(RString::from(e.to_string())),
        }
    }
}
