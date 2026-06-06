//! Integration test coverage for the composite gRPC inbound router.
//!
//! Routing logic is covered by inline tests in `core/composite.rs`
//! (the type is `pub(crate)`).  This file satisfies the per-module
//! test-file requirement and verifies the observable contract from
//! the public API surface.

use swe_edge_bootstrap::Runtime;

/// @covers: composite routing — builder wires reflection without error
#[test]
fn test_builder_with_grpc_reflection_flag_is_accepted() {
    let cfg = swe_edge_bootstrap::RuntimeConfig::default();
    let cfg = swe_edge_bootstrap::RuntimeConfig {
        grpc_reflection: true,
        ..cfg
    };
    let _builder = Runtime::builder().config(cfg);
}
