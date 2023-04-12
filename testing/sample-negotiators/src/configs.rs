use std::path::PathBuf;
use test_binary::build_test_binary;

use ya_negotiators::factory::*;

use crate::filter_nodes::FilterNodesConfig;

pub fn example_filter_config(bin_path: impl Into<PathBuf>, filter: &str) -> NegotiatorConfig {
    NegotiatorConfig {
        name: "grpc-example::FilterNodes".to_string(),
        load_mode: LoadMode::Grpc {
            path: bin_path.into(),
        },
        params: serde_yaml::to_value(FilterNodesConfig {
            names: vec![filter.to_string()],
        })
        .unwrap(),
    }
}

pub fn example_config() -> NegotiatorsConfig {
    let test_bin_path =
        build_test_binary("grpc-example", "examples").expect("error building grpc-example");

    let filter_conf = example_filter_config(test_bin_path.clone(), "dany");

    let emit_error_config = NegotiatorConfig {
        name: "grpc-example::EmitErrors".to_string(),
        load_mode: LoadMode::Grpc {
            path: PathBuf::from(&test_bin_path),
        },
        params: serde_yaml::to_value(()).unwrap(),
    };

    NegotiatorsConfig {
        negotiators: vec![filter_conf, emit_error_config],
        composite: CompositeNegotiatorConfig::default_test(),
    }
}

pub fn example_config_filter(names: &[&str]) -> NegotiatorsConfig {
    let test_bin_path =
        build_test_binary("grpc-example", "examples").expect("error building grpc-example");

    NegotiatorsConfig {
        negotiators: names
            .iter()
            .map(|name| example_filter_config(test_bin_path.clone(), name))
            .collect(),
        composite: CompositeNegotiatorConfig::default_test(),
    }
}
