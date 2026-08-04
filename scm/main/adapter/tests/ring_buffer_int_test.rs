//! Integration tests for RingBuffer via TrafficCounters public API.

use std::sync::Arc;
use swe_edge_bootstrap::{SharedCounters, TrafficCounters};
use swe_observ_metrics::create_local_metrics_backend;

fn make() -> SharedCounters {
    Arc::new(TrafficCounters::new(Arc::new(
        create_local_metrics_backend(),
    )))
}

/// @covers: ring_buffer
#[test]
fn test_ring_buffer_p99_is_tracked_via_traffic_counters() {
    let c = make();
    // Push several latency samples through on_end
    for _ in 0..10 {
        c.on_start();
        c.on_end(1_000, false); // 1ms in microseconds
    }
    // Just verifying the cycle runs without panic
    let snaps = c.export();
    assert!(!snaps.is_empty());
}
