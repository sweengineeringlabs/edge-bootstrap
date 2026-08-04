//! Integration tests for ConfigValidator.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use swe_edge_bootstrap::ServerConfigLoader;

/// @covers: config_validator
#[test]
fn test_config_validator_accepts_default_runtime_config() {
    let cfg = ServerConfigLoader::load_config().expect("load_config");
    assert!(ServerConfigLoader::validate_config(&cfg).is_ok());
}

/// @covers: config_validator
#[test]
fn test_config_validator_accepts_explicit_valid_config() {
    use swe_edge_bootstrap::RuntimeConfig;
    let cfg = RuntimeConfig::default().with_http_bind("0.0.0.0:8080");
    assert!(ServerConfigLoader::validate_config(&cfg).is_ok());
}
