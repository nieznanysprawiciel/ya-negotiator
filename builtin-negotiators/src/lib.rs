pub mod accept_all;
pub mod expiration;
pub mod max_agreements;

pub use accept_all::AcceptAll;
pub use expiration::LimitExpiration;
pub use max_agreements::MaxAgreements;

use ya_negotiator_component::static_lib::{factory, register_negotiator};

pub fn register_negotiators() {
    register_negotiator("golem-negotiators", "AcceptAll", factory::<AcceptAll>());
    register_negotiator(
        "golem-negotiators",
        "LimitExpiration",
        factory::<LimitExpiration>(),
    );
    register_negotiator(
        "golem-negotiators",
        "LimitAgreements",
        factory::<MaxAgreements>(),
    );
}
