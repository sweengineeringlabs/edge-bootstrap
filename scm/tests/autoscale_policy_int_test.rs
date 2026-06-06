//! Integration tests for AutoscalePolicy.

use swe_edge_bootstrap::AutoscalePolicy;

/// @covers: AutoscalePolicy
#[test]
fn test_autoscale_policy_default_thresholds_are_positive() {
    let p = AutoscalePolicy::default();
    assert!(p.requests_active_max > 0);
    assert!(p.requests_per_sec_max > 0);
    assert!(p.latency_p99_ms_max > 0.0);
}
