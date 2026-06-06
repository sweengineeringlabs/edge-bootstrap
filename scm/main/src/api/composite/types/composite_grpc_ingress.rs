//! `CompositeGrpcIngress` — routes gRPC requests to primary or reflection handler.

use std::sync::Arc;
use swe_edge_ingress_grpc::GrpcIngress;

/// Holds the primary and reflection gRPC handlers for composite routing.
pub struct CompositeGrpcIngress {
    pub(crate) primary: Arc<dyn GrpcIngress>,
    pub(crate) reflection: Arc<dyn GrpcIngress>,
}
