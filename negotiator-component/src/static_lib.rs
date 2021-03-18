use anyhow::anyhow;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::component::NegotiatorComponent;

pub type ConstructorFunction = Box<
    dyn Fn(serde_yaml::Value, PathBuf) -> anyhow::Result<Box<dyn NegotiatorComponent>>
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
        Some(constructor) => constructor(config, working_dir),
        None => Err(anyhow!("Negotiator '{}' not found.", name_path)),
    }
}
