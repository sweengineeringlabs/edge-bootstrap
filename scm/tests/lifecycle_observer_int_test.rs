//! Integration tests for LifecycleObserver trait coverage.

use swe_edge_bootstrap::LifecycleMonitor;

/// @covers: LifecycleObserver
#[test]
fn test_lifecycle_monitor_is_object_safe_for_observer() {
    fn _accept(_: &dyn LifecycleMonitor) {}
}
