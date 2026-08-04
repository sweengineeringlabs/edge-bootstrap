//! Integration tests for RuntimeHealth.

use swe_edge_bootstrap::{ComponentHealth, RuntimeHealth, RuntimeStatus};

/// @covers: runtime_health
#[test]
fn test_runtime_health_is_healthy_when_running_with_healthy_components() {
    let h = RuntimeHealth {
        status: RuntimeStatus::Running,
        components: vec![ComponentHealth::healthy("http")],
        uptime_secs: 5,
    };
    assert!(h.is_healthy());
}

/// @covers: runtime_health
#[test]
fn test_runtime_health_is_unhealthy_when_component_is_down() {
    let h = RuntimeHealth {
        status: RuntimeStatus::Running,
        components: vec![ComponentHealth::unhealthy("http", "timeout")],
        uptime_secs: 5,
    };
    assert!(!h.is_healthy());
}
