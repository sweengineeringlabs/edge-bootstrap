//! Integration tests for RuntimeStatus.

use swe_edge_bootstrap::RuntimeStatus;

/// @covers: runtime_status
#[test]
fn test_runtime_status_running_is_healthy() {
    assert!(RuntimeStatus::Running.is_healthy());
}

/// @covers: runtime_status
#[test]
fn test_runtime_status_stopped_is_terminal() {
    assert!(RuntimeStatus::Stopped.is_terminal());
}

/// @covers: runtime_status
#[test]
fn test_runtime_status_display_returns_lowercase() {
    assert_eq!(RuntimeStatus::Running.to_string(), "running");
    assert_eq!(RuntimeStatus::Stopped.to_string(), "stopped");
}
