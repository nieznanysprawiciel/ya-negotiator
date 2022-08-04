mod collection;
mod composite;
pub mod factory;
mod negotiators;

pub(crate) use collection::ProposalsCollection;
pub use composite::{Negotiator, NegotiatorCallbacks};

pub use negotiators::{
    AgreementAction, AgreementSigned, ControlEvent, NegotiatorAddr, PostAgreementEvent,
    ProposalAction,
};

pub use ya_negotiator_component::{
    AgreementResult, NegotiationResult, NegotiatorComponent, NegotiatorComponentMut,
    NegotiatorsPack,
};

pub mod builtin {
    pub use ya_builtin_negotiators::{AcceptAll, LimitExpiration, MaxAgreements};
}

pub mod component {
    pub use ya_agreement_utils::ProposalView;
    pub use ya_negotiator_component::static_lib::register_negotiator;
    pub use ya_negotiator_component::{
        AgreementEvent, AgreementResult, NegotiationResult, NegotiatorComponent,
        NegotiatorComponentMut, NegotiatorsPack, RejectReason, Score,
    };
}
