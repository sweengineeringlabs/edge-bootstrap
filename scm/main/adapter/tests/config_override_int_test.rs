//! Integration tests for config override behavior via ServerConfigLoader::load_config_from.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::Write as _;
use swe_edge_bootstrap::ServerConfigLoader;

/// @covers: config_override
#[test]
fn test_config_override_http_bind_is_applied_from_toml() {
    let dir = tempfile::tempdir().unwrap();
    let mut f = std::fs::File::create(dir.path().join("application.toml")).unwrap();
    writeln!(f, r#"http_bind = "127.0.0.1:9999""#).unwrap();
    let cfg = ServerConfigLoader::load_config_from(dir.path()).unwrap();
    assert_eq!(cfg.http_bind, "127.0.0.1:9999");
}

/// @covers: config_override
#[test]
fn test_config_override_service_name_is_applied_from_toml() {
    let dir = tempfile::tempdir().unwrap();
    let mut f = std::fs::File::create(dir.path().join("application.toml")).unwrap();
    writeln!(f, r#"service_name = "acme""#).unwrap();
    let cfg = ServerConfigLoader::load_config_from(dir.path()).unwrap();
    assert_eq!(cfg.service_name, "acme");
}
