/////////////////////////////////////////////////////////////////////////////////
//
//                      Interface crate
//
//////////////////////////////////////////////////////////////////////////////////

use std::path::Path;

use abi_stable::{
    library::{LibraryError, RootModule},
    package_version_strings, sabi_trait,
    sabi_types::VersionStrings,
    std_types::{RBox, RResult, RStr, RString},
    StableAbi,
};

#[repr(C)]
#[derive(StableAbi)]
#[sabi(kind(Prefix(prefix_ref = "NegotiatorLib_Ref")))]
#[sabi(missing_field(panic))]
pub struct NegotiatorLib {
    /**
    The `#[sabi(last_prefix_field)]` attribute here means that this is the last field in this struct
    that was defined in the first compatible version of the library
    (0.1.0, 0.2.0, 0.3.0, 1.0.0, 2.0.0 ,etc),
    requiring new fields to always be added below preexisting ones.

    The `#[sabi(last_prefix_field)]` attribute would stay on this field until the library
    bumps its "major" version,
    at which point it would be moved to the last field at the time.

    */
    #[sabi(last_prefix_field)]
    /// Create negotiator. First parameter is name. Second parameter is negotiator config.
    pub create_negotiator: extern "C" fn(RStr, RStr) -> BoxedSharedNegotiatorAPI,
}

/// The RootModule trait defines how to load the root module of a library.
impl RootModule for NegotiatorLib_Ref {
    abi_stable::declare_root_module_statics! {NegotiatorLib_Ref}

    const BASE_NAME: &'static str = "negotiator";
    const NAME: &'static str = "negotiator";
    const VERSION_STRINGS: VersionStrings = package_version_strings!();
}

/// This loads the root module
///
/// This is for the case where this example is copied into a single crate
pub fn load_library(path: &Path) -> Result<NegotiatorLib_Ref, LibraryError> {
    NegotiatorLib_Ref::load_from_file(path)
}

//////////////////////////////////////////////////////////

#[sabi_trait]
pub trait SharedNegotiatorAPI {
    /// Push forward negotiations as far as you can.
    /// `NegotiatorComponent` should modify only properties in his responsibility
    /// and return remaining part of Proposal unchanged.
    fn negotiate_step(&mut self, demand: &RStr, offer: &RStr) -> RString;

    /// Called during Offer creation. `NegotiatorComponent` should add properties
    /// and constraints for which it is responsible during future negotiations.
    /// TODO: Make API generic enough to work with Requestor.
    fn fill_template(
        &mut self,
        template_props: &RStr,
        template_constraints: &RStr,
    ) -> RResult<RString, RString>;

    /// Called when Agreement was finished. `NegotiatorComponent` can use termination
    /// result to adjust his future negotiation strategy.
    fn on_agreement_terminated(
        &mut self,
        agreement_id: &RStr,
        result: &RStr,
    ) -> RResult<(), RString>;

    /// Called when Negotiator decided to approve/propose Agreement. It's only notification,
    /// `NegotiatorComponent` can't reject Agreement anymore.
    fn on_agreement_approved(&mut self, agreement_id: &RStr) -> RResult<(), RString>;
}

pub type BoxedSharedNegotiatorAPI = SharedNegotiatorAPI_TO<'static, RBox<()>>;
