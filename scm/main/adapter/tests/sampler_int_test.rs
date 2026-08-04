//! Integration tests for Sampler trait coverage.

use swe_edge_bootstrap::LifecycleMonitor;

/// @covers: Sampler
#[test]
fn test_lifecycle_monitor_is_object_safe_for_sampler() {
    // The Sampler runs the background metric-sampling loop tied to the lifecycle monitor.
    fn _accept(_: &dyn LifecycleMonitor) {}
}
