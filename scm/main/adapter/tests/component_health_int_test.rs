//! Integration tests for ComponentHealth.

use swe_edge_bootstrap::ComponentHealth;

/// @covers: component_health
#[test]
fn test_component_health_healthy_has_no_detail() {
    let c = ComponentHealth::healthy("db");
    assert!(c.healthy && c.detail.is_none());
}

/// @covers: component_health
#[test]
fn test_component_health_unhealthy_has_detail() {
    let c = ComponentHealth::unhealthy("db", "connection refused");
    assert!(!c.healthy && c.detail.is_some());
}
