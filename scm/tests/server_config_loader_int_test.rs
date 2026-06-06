//! Integration tests for ServerConfigLoader.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use swe_edge_bootstrap::ServerConfigLoader;

/// @covers: ServerConfigLoader
#[test]
fn test_server_config_loader_create_config_builder_returns_named_builder() {
    let builder = ServerConfigLoader::create_config_builder();
    assert!(!builder.name().is_empty(), "builder name must not be empty");
    assert!(
        !builder.version().is_empty(),
        "builder version must not be empty"
    );
}

/// @covers: ServerConfigLoader
#[test]
fn test_server_config_loader_load_config_returns_ok() {
    let cfg = ServerConfigLoader::load_config().expect("load_config");
    assert!(!cfg.http_bind.is_empty());
}

/// @covers: ServerConfigLoader
#[test]
fn test_server_config_loader_validate_config_accepts_defaults() {
    let cfg = ServerConfigLoader::load_config().unwrap();
    assert!(ServerConfigLoader::validate_config(&cfg).is_ok());
}
