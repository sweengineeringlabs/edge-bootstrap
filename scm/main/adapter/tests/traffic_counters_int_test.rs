//! Integration tests for TrafficCounters.

use std::sync::Arc;
use swe_edge_bootstrap::{SharedCounters, TrafficCounters};
use swe_observ_metrics::create_local_metrics_backend;

fn make() -> SharedCounters {
    Arc::new(TrafficCounters::new(Arc::new(
        create_local_metrics_backend(),
    )))
}

/// @covers: TrafficCounters
#[test]
fn test_traffic_counters_on_start_and_on_end_cycle() {
    let c = make();
    c.on_start();
    c.on_end(500, false);
    let snaps = c.export();
    assert!(snaps.iter().any(|s| s.name == "edge_requests_total"));
}

/// @covers: TrafficCounters
#[test]
fn test_traffic_counters_on_end_error_increments_error_counter() {
    let c = make();
    c.on_start();
    c.on_end(100, true);
    let snaps = c.export();
    assert!(snaps.iter().any(|s| s.name == "edge_errors_total"));
}
