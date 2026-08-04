//! Integration tests for CompositeIngress trait.

use swe_edge_bootstrap::GrpcIngress;

/// @covers: CompositeIngress
#[test]
fn test_composite_ingress_grpc_ingress_is_object_safe() {
    fn _accept(_: &dyn GrpcIngress) {}
}
