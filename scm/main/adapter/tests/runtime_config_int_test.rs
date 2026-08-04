//! Integration tests for RuntimeConfig.

use swe_edge_bootstrap::RuntimeConfig;

/// @covers: runtime_config
#[test]
fn test_runtime_config_default_has_expected_http_bind() {
    assert_eq!(RuntimeConfig::default().http_bind, "0.0.0.0:8080");
}

/// @covers: runtime_config
#[test]
fn test_runtime_config_with_service_name_sets_name() {
    let cfg = RuntimeConfig::default().with_service_name("my-svc");
    assert_eq!(cfg.service_name, "my-svc");
}

/// @covers: runtime_config
#[test]
fn test_runtime_config_with_http_bind_overrides_default() {
    let cfg = RuntimeConfig::default().with_http_bind("127.0.0.1:9000");
    assert_eq!(cfg.http_bind, "127.0.0.1:9000");
}

/// @covers: runtime_config
#[test]
fn test_runtime_config_with_shutdown_timeout_sets_secs() {
    let cfg = RuntimeConfig::default().with_shutdown_timeout(60);
    assert_eq!(cfg.shutdown_timeout_secs, 60);
}
