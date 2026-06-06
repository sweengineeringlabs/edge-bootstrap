//! Integration tests for SharedCounters type alias.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use swe_edge_bootstrap::{SharedCounters, TrafficCounters};
use swe_observ_metrics::create_local_metrics_backend;

fn make_shared() -> SharedCounters {
    Arc::new(TrafficCounters::new(Arc::new(
        create_local_metrics_backend(),
    )))
}

/// @covers: SharedCounters
#[test]
fn test_shared_counters_can_be_cloned_across_threads() {
    let c = make_shared();
    let c2 = Arc::clone(&c);
    let handle = std::thread::spawn(move || {
        c2.on_start();
    });
    handle.join().unwrap();
    c.on_end(100, false);
}

/// @covers: SharedCounters
#[test]
fn test_shared_counters_arc_strong_count_tracks_clones() {
    let c = make_shared();
    let c2 = Arc::clone(&c);
    assert_eq!(Arc::strong_count(&c), 2);
    drop(c2);
    assert_eq!(Arc::strong_count(&c), 1);
}
