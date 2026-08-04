//! Integration tests for the Prometheus metrics endpoint.

use std::sync::Arc;
use swe_edge_bootstrap::{SharedCounters, TrafficCounters};
use swe_observ_metrics::create_local_metrics_backend;

fn make_counters() -> SharedCounters {
    Arc::new(TrafficCounters::new(Arc::new(
        create_local_metrics_backend(),
    )))
}

/// @covers: on_end
#[test]
fn test_traffic_counters_integrates_with_metrics_backend() {
    let c = make_counters();
    c.on_start();
    c.on_end(1_000, false);
    let snaps = c.export();
    assert!(snaps
        .iter()
        .any(|s| s.name == "edge_requests_total" && s.value == 1.0));
}

/// @covers: on_end
#[test]
fn test_traffic_counters_error_path_records_error_total() {
    let c = make_counters();
    c.on_start();
    c.on_end(500, true);
    let snaps = c.export();
    assert!(snaps.iter().any(|s| s.name == "edge_errors_total"));
}
