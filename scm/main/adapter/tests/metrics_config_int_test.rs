//! Integration tests for MetricsConfig.

use swe_edge_bootstrap::MetricsConfig;

/// @covers: MetricsConfig
#[test]
fn test_metrics_config_default_bind_is_0_0_0_0_9090() {
    assert_eq!(MetricsConfig::default().bind, "0.0.0.0:9090");
}

/// @covers: MetricsConfig
#[test]
fn test_metrics_config_default_path_is_metrics() {
    assert_eq!(MetricsConfig::default().path, "/metrics");
}
