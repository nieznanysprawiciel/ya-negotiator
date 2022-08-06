mod actor;
mod client;
mod component;
mod message;
mod server;

extern crate lazy_static;
pub use lazy_static::lazy_static;

pub extern crate ya_agreement_utils;
pub extern crate ya_negotiator_component;

pub use ya_agreement_utils::{AgreementView, OfferTemplate, ProposalView};
pub use ya_client_model::market::Reason;
pub use ya_negotiator_component::component::{
    AgreementResult, NegotiationResult, NegotiatorComponent, Score,
};

pub mod entrypoint {
    pub use crate::server::server_run;
    pub use ya_negotiator_component::static_lib::{factory, register_negotiator};
}

pub mod plugin {
    pub use ya_agreement_utils::{AgreementView, OfferTemplate, ProposalView};
    pub use ya_client_model::market::Reason;
    pub use ya_negotiator_component::static_lib::{
        NegotiatorAsync, NegotiatorFactory, NegotiatorFactoryDefault, NegotiatorMut,
    };
    pub use ya_negotiator_component::{
        AgreementResult, NegotiationResult, NegotiatorComponent, NegotiatorComponentMut,
        RejectReason, Score,
    };
}
