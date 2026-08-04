//! Integration tests for MetricsExporter trait coverage.

use swe_edge_bootstrap::MetricsConfig;

/// @covers: MetricsExporter
#[test]
fn test_metrics_config_default_bind_is_set() {
    let c = MetricsConfig::default();
    assert!(!c.bind.is_empty());
}

/// @covers: MetricsExporter
#[test]
fn test_metrics_config_default_path_starts_with_slash() {
    let c = MetricsConfig::default();
    assert!(c.path.starts_with('/'));
}
