//! Integration tests for RuntimeError.

use swe_edge_bootstrap::RuntimeError;

/// @covers: RuntimeError
#[test]
fn test_runtime_error_start_failed_displays_reason() {
    let e = RuntimeError::StartFailed("port in use".into());
    assert_eq!(e.to_string(), "start failed: port in use");
}

/// @covers: RuntimeError
#[test]
fn test_runtime_error_shutdown_timeout_includes_seconds() {
    let e = RuntimeError::ShutdownTimeout(30);
    assert_eq!(e.to_string(), "shutdown timed out after 30s");
}

/// @covers: RuntimeError
#[test]
fn test_runtime_error_bind_failed_displays_address() {
    let e = RuntimeError::BindFailed("addr in use".into());
    assert!(e.to_string().contains("bind failed"));
}
