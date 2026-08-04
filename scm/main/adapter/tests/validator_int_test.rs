//! Integration tests for the public `validate_config` function.

use swe_edge_bootstrap::{RuntimeConfig, ServerConfigLoader};

/// @covers: validate_config — valid default config passes
#[test]
fn test_validate_config_accepts_default_config() {
    let config = RuntimeConfig::default();
    assert!(ServerConfigLoader::validate_config(&config).is_ok());
}

/// @covers: validate_config — empty http_bind is rejected
#[test]
fn test_validate_config_rejects_empty_http_bind() {
    let config = RuntimeConfig {
        http_bind: "".into(),
        ..RuntimeConfig::default()
    };
    assert!(ServerConfigLoader::validate_config(&config).is_err());
}

/// @covers: validate_config — zero shutdown timeout is rejected
#[test]
fn test_validate_config_rejects_zero_shutdown_timeout() {
    let config = RuntimeConfig {
        shutdown_timeout_secs: 0,
        ..RuntimeConfig::default()
    };
    assert!(ServerConfigLoader::validate_config(&config).is_err());
}
