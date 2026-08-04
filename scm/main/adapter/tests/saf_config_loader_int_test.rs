//! Public-API integration tests for saf config-loading methods on ServerConfigLoader.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use swe_edge_bootstrap::ServerConfigLoader;

/// @covers: load_config
#[test]
fn test_load_config_returns_valid_runtime_config() {
    let cfg = ServerConfigLoader::load_config().expect("load_config");
    assert!(!cfg.http_bind.is_empty());
}

/// @covers: load_config_from
#[test]
fn test_load_config_from_temp_dir_returns_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = ServerConfigLoader::load_config_from(dir.path()).expect("load_config_from");
    assert!(!cfg.http_bind.is_empty());
}

/// @covers: load_config_xdg
#[test]
fn test_load_config_xdg_nonexistent_app_returns_defaults() {
    let cfg = ServerConfigLoader::load_config_xdg("swe-edge-test-xyz-nonexistent")
        .expect("load_config_xdg");
    assert!(!cfg.grpc_bind.is_empty());
}

/// @covers: load_tenant_config
#[test]
fn test_load_tenant_config_unknown_tenant_returns_error() {
    let result = ServerConfigLoader::load_tenant_config("nonexistent-tenant-xyz");
    assert!(result.is_err(), "expected error for unknown tenant");
}

/// @covers: load_tenant_config_from
#[test]
fn test_load_tenant_config_from_missing_file_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let result = ServerConfigLoader::load_tenant_config_from("no-tenant", dir.path());
    assert!(result.is_err());
}

/// @covers: load_tenant_config_xdg
#[test]
fn test_load_tenant_config_xdg_missing_returns_error() {
    let result =
        ServerConfigLoader::load_tenant_config_xdg("swe-edge-test-xyz-nonexistent", "no-tenant");
    assert!(result.is_err());
}

/// @covers: validate_config
#[test]
fn test_validate_config_accepts_valid_default_config() {
    let cfg = ServerConfigLoader::load_config().unwrap();
    assert!(ServerConfigLoader::validate_config(&cfg).is_ok());
}

/// @covers: create_config_builder
#[test]
fn test_create_config_builder_returns_configured_builder() {
    let builder = ServerConfigLoader::create_config_builder();
    // The builder is pre-seeded with this crate's package name and version.
    assert!(!builder.name().is_empty(), "builder name must not be empty");
    assert!(
        !builder.version().is_empty(),
        "builder version must not be empty"
    );
}
