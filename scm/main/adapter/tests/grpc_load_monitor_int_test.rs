//! Integration tests for GrpcLoadMonitor trait coverage.

use swe_edge_bootstrap::GrpcIngress;

/// @covers: GrpcLoadMonitor
#[test]
fn test_grpc_ingress_is_object_safe_for_load_monitor() {
    fn _accept(_: &dyn GrpcIngress) {}
}
