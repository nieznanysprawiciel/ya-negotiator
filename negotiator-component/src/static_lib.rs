use anyhow::anyhow;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::component::NegotiatorComponent;

pub type ConstructorFunction = Box<
    dyn Fn(&str, serde_yaml::Value) -> anyhow::Result<Box<dyn NegotiatorComponent + Send + Sync>>
        + Send
        + Sync,
>;

lazy_static! {
    /// Contains functions that can create negotiators by name.
    static ref CONSTRUCTORS: Arc<Mutex<HashMap<String, ConstructorFunction>>> = Arc::new(Mutex::new(HashMap::new()));
}

pub fn register_negotiator(library: &str, name: &str, constructor: ConstructorFunction) {
    (*CONSTRUCTORS)
        .lock()
        .unwrap()
        .insert(format!("{}::{}", library, name), constructor);
}

pub fn create_static_negotiator(
    name_path: &str,
    config: serde_yaml::Value,
) -> anyhow::Result<Box<dyn NegotiatorComponent + Send + Sync>> {
    let map = (*CONSTRUCTORS)
        .lock()
        .map_err(|e| anyhow!("Failed to acquire static Negotiator creation lock: {}", e))?;

    match map.get(name_path) {
        Some(constructor) => constructor(name_path, config),
        None => Err(anyhow!("Negotiator '{}' not found.", name_path)),
    }
}
