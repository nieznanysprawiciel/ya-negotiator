use abi_stable::sabi_extern_fn;
use abi_stable::sabi_trait::TU_Opaque;
use abi_stable::std_types::{RResult, RResult::RErr, RResult::ROk, RStr, RString};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::interface::{BoxedSharedNegotiatorAPI, SharedNegotiatorAPI};
use crate::SharedLibError;

use std::str::FromStr;
pub use ya_agreement_utils::{AgreementView, OfferTemplate, ProposalView};
pub use ya_client_model::market::Reason;
pub use ya_negotiator_component::component::{
    AgreementResult, NegotiationResult, NegotiatorComponent, Score,
};

pub trait NegotiatorConstructor<T: NegotiatorComponent + Sync + Send + Sized>: Sync + Send {
    fn new(name: &str, config: serde_yaml::Value, working_dir: PathBuf) -> anyhow::Result<T>;
}

/// Wraps `NegotiatorComponent` inside shared library and translates communication
/// from outside to `NegotiatorComponent` API.
/// This way developer can only implement more convenient API, instead of being force to deal
/// with binary compatibility problems.
pub struct NegotiatorWrapper<T: NegotiatorComponent + NegotiatorConstructor<T> + Sized> {
    component: T,
}

impl<T> NegotiatorWrapper<T>
where
    T: NegotiatorComponent + NegotiatorConstructor<T> + Sized + 'static,
{
    pub fn new(
        name: RStr,
        config: RStr,
        working_dir: RStr,
    ) -> RResult<BoxedSharedNegotiatorAPI, RString> {
        match Self::new_impl(name, config, working_dir) {
            Ok(nagotiator) => ROk(nagotiator),
            Err(e) => RErr(RString::from(e.to_string())),
        }
    }

    fn new_impl(
        name: RStr,
        config: RStr,
        working_dir: RStr,
    ) -> anyhow::Result<BoxedSharedNegotiatorAPI> {
        let working_dir = PathBuf::from_str(working_dir.as_str())?;
        let config = serde_yaml::from_str(config.as_str())?;
        let component = T::new(name.as_str(), config, working_dir)?;

        Ok(BoxedSharedNegotiatorAPI::from_value(
            NegotiatorWrapper { component },
            TU_Opaque,
        ))
    }
}

impl<T> SharedNegotiatorAPI for NegotiatorWrapper<T>
where
    T: NegotiatorComponent + NegotiatorConstructor<T> + Sized,
{
    fn negotiate_step(
        &mut self,
        demand: &RStr,
        offer: &RStr,
        score: &RStr,
    ) -> RResult<RString, RString> {
        match (|| {
            let demand = serde_json::from_str(demand.as_str()).map_err(SharedLibError::from)?;
            let offer = serde_json::from_str(offer.as_str()).map_err(SharedLibError::from)?;
            let score = serde_json::from_str(score.as_str()).map_err(SharedLibError::from)?;

            let result = self
                .component
                .negotiate_step(&demand, offer, score)
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

    fn on_agreement_approved(&mut self, agreement: &RStr) -> RResult<(), RString> {
        match (|| {
            let agreement =
                serde_json::from_str(agreement.as_str()).map_err(SharedLibError::from)?;
            self.component
                .on_agreement_approved(&agreement)
                .map_err(|e| SharedLibError::Negotiation(e.to_string()))
        })() {
            Ok(_) => ROk(()),
            Err(e) => RResult::RErr(RString::from(e.to_string())),
        }
    }

    fn on_proposal_rejected(&mut self, proposal_id: &RStr) -> RResult<(), RString> {
        match (|| {
            self.component
                .on_proposal_rejected(proposal_id.as_str())
                .map_err(|e| SharedLibError::Negotiation(e.to_string()))?;
            Result::<(), SharedLibError>::Ok(())
        })() {
            Ok(_) => ROk(()),
            Err(e) => RResult::RErr(RString::from(e.to_string())),
        }
    }

    fn on_agreement_event(&mut self, agreement_id: &RStr, event: &RStr) -> RResult<(), RString> {
        match (|| {
            let result = serde_json::from_str(event.as_str()).map_err(SharedLibError::from)?;
            self.component
                .on_agreement_event(agreement_id.as_str(), &result)
                .map_err(|e| SharedLibError::Negotiation(e.to_string()))?;
            Result::<(), SharedLibError>::Ok(())
        })() {
            Ok(_) => ROk(()),
            Err(e) => RResult::RErr(RString::from(e.to_string())),
        }
    }

    fn control_event(&mut self, component: &RStr, params: &RStr) -> RResult<RString, RString> {
        match (|| {
            let params = serde_json::from_str(params.as_str()).map_err(SharedLibError::from)?;
            let response = self
                .component
                .control_event(component.as_str(), params)
                .map_err(|e| SharedLibError::Negotiation(e.to_string()))?;

            Result::<String, SharedLibError>::Ok(
                serde_json::to_string(&response).map_err(SharedLibError::from)?,
            )
        })() {
            Ok(response) => ROk(RString::from(response)),
            Err(e) => RResult::RErr(RString::from(e.to_string())),
        }
    }
}

type ConstructorFunction =
    Box<dyn Fn(RStr, RStr, RStr) -> RResult<BoxedSharedNegotiatorAPI, RString> + Send + Sync>;

lazy_static! {
    /// Contains functions that can create negotiators by name.
    static ref CONSTRUCTORS: Arc<Mutex<HashMap<&'static str, ConstructorFunction>>> = Arc::new(Mutex::new(HashMap::new()));
}

pub fn register_negotiator_impl(
    name: &'static str,
    fun: ConstructorFunction,
) -> anyhow::Result<()> {
    (*CONSTRUCTORS).lock().unwrap().insert(name, fun);
    Ok(())
}

#[sabi_extern_fn]
pub fn create_negotiator(
    name: RStr,
    config: RStr,
    working_dir: RStr,
) -> RResult<BoxedSharedNegotiatorAPI, RString> {
    let map = match (*CONSTRUCTORS).lock() {
        Ok(map) => map,
        Err(e) => return RErr(RString::from(e.to_string())),
    };

    match map.get(name.as_str()) {
        Some(constructor) => constructor(name, config, working_dir),
        None => RErr(RString::from(format!("Negotiator '{}' not found.", name))),
    }
}

#[macro_export]
macro_rules! register_negotiators_inner {
    ($NegotiatorType:ty) => {{
        ya_negotiator_shared_lib_interface::plugin::register_negotiator_impl(stringify!($NegotiatorType), Box::new(|name, config, working_dir| {
            ya_negotiator_shared_lib_interface::plugin::NegotiatorWrapper::<$NegotiatorType>::new(name, config, working_dir)
        })).unwrap();
    }};
    ($NegotiatorType:ty, $($Rest:ty),+) => {
        ya_negotiator_shared_lib_interface::register_negotiators_inner!($NegotiatorType)
        ya_negotiator_shared_lib_interface::register_negotiators_inner!($($Rest),+)
    };
}

#[macro_export]
macro_rules! register_negotiators {
    ($($NegotiatorTypes:ty),+) => {
        #[ya_negotiator_shared_lib_interface::export_root_module]
        pub fn get_library() -> ya_negotiator_shared_lib_interface::interface::NegotiatorLib_Ref {
            ya_negotiator_shared_lib_interface::register_negotiators_inner!($($NegotiatorTypes),+);

            ya_negotiator_shared_lib_interface::interface::NegotiatorLib {
                create_negotiator: ya_negotiator_shared_lib_interface::plugin::create_negotiator
            }.leak_into_prefix()
        }
    };
}
