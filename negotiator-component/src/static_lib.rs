use anyhow::anyhow;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::component::NegotiatorComponent;
use crate::component_mut::ComponentMutWrapper;
use crate::NegotiatorComponentMut;

pub type ConstructorFunction = Box<
    dyn Fn(&str, serde_yaml::Value, PathBuf) -> anyhow::Result<Box<dyn NegotiatorComponent>>
        + Send
        + Sync,
>;

lazy_static! {
    /// Contains functions that can create negotiators by name.
    static ref CONSTRUCTORS: Arc<Mutex<HashMap<String, ConstructorFunction>>> = Arc::new(Mutex::new(HashMap::new()));
}

pub fn register_negotiator(library: &str, name: &str, constructor: ConstructorFunction) {
    let negotiator = format!("{}::{}", library, name);
    println!("Registering: {}", negotiator);
    (*CONSTRUCTORS)
        .lock()
        .unwrap()
        .insert(negotiator, constructor);
}

pub fn create_static_negotiator(
    name_path: &str,
    config: serde_yaml::Value,
    working_dir: PathBuf,
) -> anyhow::Result<Box<dyn NegotiatorComponent>> {
    let map = (*CONSTRUCTORS)
        .lock()
        .map_err(|e| anyhow!("Failed to acquire static Negotiator creation lock: {}", e))?;

    match map.get(name_path) {
        Some(constructor) => constructor(name_path, config, working_dir),
        None => Err(anyhow!("Negotiator '{}' not found.", name_path)),
    }
}

/// Helper factory trait that allows to convert different types to `Box<dyn NegotiatorComponent>`
/// automatically through generic implementations.
pub trait ToBoxed {
    fn cast(self) -> Box<dyn NegotiatorComponent>;
}

pub trait NegotiatorInterfaceType {}

/// Use `NegotiatorAsync` if you implement `NegotiatorComponent` trait directly.
pub struct NegotiatorAsync;
/// Use `NegotiatorMut` if you implement `NegotiatorComponentMut`.
pub struct NegotiatorMut;

impl NegotiatorInterfaceType for NegotiatorAsync {}
impl NegotiatorInterfaceType for NegotiatorMut {}

/// Defines common `Negotiators` creation interface.
pub trait NegotiatorFactory<T: Sized> {
    /// `NegotiatorAsync` if you implement `NegotiatorComponent` trait directly.
    /// `NegotiatorMut` if you implement `NegotiatorComponentMut`.
    ///
    /// This type helps keep `factory` function implementation generic independent of interface used.
    /// Otherwise we encounter conflicting trait implementation issues.
    type Type: NegotiatorInterfaceType;

    /// Negotiator is allowed to save data only inside `working_dir`. It should be the
    /// same directory across many executions of Provider/Requestor.
    fn new(name: &str, config: serde_yaml::Value, working_dir: PathBuf) -> anyhow::Result<T>;
}

/// Returns factory function for creating Negotiators.
///
/// Example usage:
///
/// ```
/// use ya_negotiator_component::static_lib::{factory, register_negotiator};
///
/// pub fn register_negotiators() {
///     register_negotiator(
///         "golem-negotiators",
///         "LimitExpiration",
///         factory::<LimitExpiration>(),
///     );
/// }
/// ```
pub fn factory<N>() -> ConstructorFunction
where
    N: ToBoxed + NegotiatorFactory<N> + 'static,
{
    Box::new(|name, config, working_dir| Ok(N::new(name, config, working_dir)?.cast()))
}

/// This is overcomplicated, but is necessary for compiler, to stop complaining.
/// You can check, what is the problem here: https://geo-ant.github.io/blog/2021/mutually-exclusive-traits-rust/
trait CastWrapper<T: NegotiatorInterfaceType, F> {
    fn cast(neg: F) -> Box<dyn NegotiatorComponent>;
}

impl<F> CastWrapper<NegotiatorAsync, F> for F
where
    F: NegotiatorFactory<F, Type = NegotiatorAsync> + NegotiatorComponent + 'static,
{
    fn cast(negotiator: F) -> Box<dyn NegotiatorComponent> {
        Box::new(negotiator) as Box<dyn NegotiatorComponent>
    }
}

impl<F> CastWrapper<NegotiatorMut, F> for F
where
    F: NegotiatorFactory<F, Type = NegotiatorMut> + NegotiatorComponentMut + 'static,
{
    fn cast(negotiator: F) -> Box<dyn NegotiatorComponent> {
        Box::new(ComponentMutWrapper::new(negotiator)) as Box<dyn NegotiatorComponent>
    }
}

impl<F, T> ToBoxed for F
where
    F: CastWrapper<T, F> + NegotiatorFactory<F, Type = T> + 'static,
    T: NegotiatorInterfaceType,
{
    fn cast(self) -> Box<dyn NegotiatorComponent> {
        F::cast(self)
    }
}
