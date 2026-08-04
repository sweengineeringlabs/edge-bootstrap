//! Integration tests for Validator trait.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use swe_edge_bootstrap::ServerConfigLoader;

/// @covers: traits
#[test]
fn test_validator_trait_validate_config_returns_ok_for_defaults() {
    let cfg = ServerConfigLoader::load_config().expect("load_config");
    assert!(ServerConfigLoader::validate_config(&cfg).is_ok());
}
