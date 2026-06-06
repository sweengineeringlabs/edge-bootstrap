//! Integration tests for ApplicationConfigLoader.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use swe_edge_bootstrap::ServerConfigLoader;

/// @covers: ApplicationConfigLoader
#[test]
fn test_application_config_loader_loads_default_config() {
    let cfg = ServerConfigLoader::load_config().expect("load_config");
    assert!(!cfg.http_bind.is_empty());
    assert!(!cfg.grpc_bind.is_empty());
}
