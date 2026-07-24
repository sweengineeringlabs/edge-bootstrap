//! Integration tests for ServerMonitor.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use edge_proxy::ProxySvc;
use std::sync::Arc;
use swe_edge_bootstrap::{MetricsProvider, ServerMonitor};
use swe_observ_metrics::create_local_metrics_backend;

/// @covers: ServerMonitor
#[test]
fn test_server_monitor_observe_returns_arc_lifecycle_monitor() {
    let inner = ProxySvc::new_null_lifecycle_monitor();
    let provider: Arc<dyn MetricsProvider> = Arc::new(create_local_metrics_backend());
    let _observed = ServerMonitor::observe(inner, provider);
}

/// @covers: ServerMonitor
#[tokio::test]
async fn test_server_monitor_observe_delegates_health_to_inner() {
    let inner = ProxySvc::new_null_lifecycle_monitor();
    let provider: Arc<dyn MetricsProvider> = Arc::new(create_local_metrics_backend());
    let observed = ServerMonitor::observe(inner, provider);
    let report = observed.health(edge_proxy::HealthRequest).await.unwrap();
    assert!(matches!(report.overall, edge_proxy::HealthStatus::Healthy));
}
