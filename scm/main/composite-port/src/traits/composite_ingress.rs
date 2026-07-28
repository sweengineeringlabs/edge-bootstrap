//! `CompositeIngress` — composite ingress router contract.

use std::sync::Arc;
use swe_edge_ingress_grpc::GrpcIngress;

/// Routes requests between a primary handler and a secondary (e.g. reflection).
pub trait CompositeIngress: Send + Sync {
    fn primary(&self) -> Arc<dyn GrpcIngress>;
}
