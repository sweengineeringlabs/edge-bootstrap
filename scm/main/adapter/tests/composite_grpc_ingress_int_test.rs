//! Integration tests for CompositeGrpcIngress.

use swe_edge_bootstrap::GrpcIngress;

/// @covers: CompositeGrpcIngress
#[test]
fn test_composite_grpc_ingress_trait_is_object_safe() {
    fn _accept(_: &dyn GrpcIngress) {}
}
