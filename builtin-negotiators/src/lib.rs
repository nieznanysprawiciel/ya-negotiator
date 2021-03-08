pub mod accept_all;
pub mod expiration;
pub mod max_agreements;

pub use accept_all::AcceptAll;
pub use expiration::LimitExpiration;
pub use max_agreements::MaxAgreements;

use ya_negotiator_component::static_lib::register_negotiator;
use ya_negotiator_component::NegotiatorComponent;

pub fn register_negotiators() {
    register_negotiator(
        "golem-negotiators",
        "AcceptAll",
        Box::new(|config| Ok(Box::new(AcceptAll::new(config)?) as Box<dyn NegotiatorComponent>)),
    );
    register_negotiator(
        "golem-negotiators",
        "LimitExpiration",
        Box::new(|config| {
            Ok(Box::new(LimitExpiration::new(config)?) as Box<dyn NegotiatorComponent>)
        }),
    );
    register_negotiator(
        "golem-negotiators",
        "LimitAgreements",
        Box::new(
            |config| Ok(Box::new(MaxAgreements::new(config)?) as Box<dyn NegotiatorComponent>),
        ),
    );
}
