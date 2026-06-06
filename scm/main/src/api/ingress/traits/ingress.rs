//! `Ingress` — ingress adapter contract.

use std::sync::Arc;
use swe_edge_ingress_grpc::GrpcIngress;
use swe_edge_ingress_http::HttpIngress;

/// Supplies the ingress adapters the runtime binds traffic through.
pub trait Ingress: Send + Sync {
    /// Returns the HTTP inbound adapter, if configured.
    fn http(&self) -> Option<Arc<dyn HttpIngress>>;
    /// Returns the gRPC inbound adapter, if configured.
    fn grpc(&self) -> Option<Arc<dyn GrpcIngress>>;
    /// Returns `true` when at least one transport is configured.
    fn has_any(&self) -> bool {
        self.http().is_some() || self.grpc().is_some()
    }
}
