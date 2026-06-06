//! Public-API integration tests for saf lifecycle_monitor methods on ServerMonitor.

use edge_proxy::ProxySvc;
use std::sync::Arc;
use swe_edge_bootstrap::{MetricsProvider, ServerMonitor};
use swe_observ_metrics::create_local_metrics_backend;

/// @covers: observe
#[tokio::test]
async fn test_observe_lifecycle_monitor() {
    let provider: Arc<dyn MetricsProvider> = Arc::new(create_local_metrics_backend());
    let inner = ProxySvc::new_null_lifecycle_monitor();
    let observed = ServerMonitor::observe(inner, Arc::clone(&provider));
    observed.health().await;
    let snaps = provider.export();
    assert!(
        snaps.iter().any(|s| s.name == "edge_component_health"),
        "expected edge_component_health gauge after health poll, got {snaps:?}"
    );
}
