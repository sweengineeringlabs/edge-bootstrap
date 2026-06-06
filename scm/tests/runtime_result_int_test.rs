//! Integration tests for RuntimeResult type alias.

use swe_edge_bootstrap::{RuntimeError, RuntimeResult};

/// @covers: RuntimeResult
#[test]
fn test_runtime_result_ok_holds_value() {
    let r: RuntimeResult<u32> = Ok(42);
    let Ok(v) = r else { panic!("expected Ok") };
    assert_eq!(v, 42);
}

/// @covers: RuntimeResult
#[test]
fn test_runtime_result_err_holds_error() {
    let r: RuntimeResult<u32> = Err(RuntimeError::StartFailed("test".into()));
    assert!(r.is_err());
}
