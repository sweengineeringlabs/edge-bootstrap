//! `ServiceRegistry` — stable egress client registry handed to handlers at construction time.

use std::sync::Arc;

use swe_edge_egress_grpc::GrpcEgress;
use swe_edge_egress_http::HttpEgress;

/// Holds egress clients that handlers may use to make outbound calls.
///
/// Constructed by [`RuntimeBuilder::build_registry`] and passed to
/// handler constructors at startup — not per-request.  Share it via
/// `Arc<ServiceRegistry>`.
pub struct ServiceRegistry {
    http: Arc<dyn HttpEgress>,
    grpc: Option<Arc<dyn GrpcEgress>>,
}

impl ServiceRegistry {
    /// Construct a registry from an HTTP egress client and an optional gRPC client.
    pub fn new(http: Arc<dyn HttpEgress>, grpc: Option<Arc<dyn GrpcEgress>>) -> Self {
        Self { http, grpc }
    }

    /// Return the HTTP egress client.
    pub fn http(&self) -> &Arc<dyn HttpEgress> {
        &self.http
    }

    /// Return the gRPC egress client, if one was registered.
    pub fn grpc(&self) -> Option<&Arc<dyn GrpcEgress>> {
        self.grpc.as_ref()
    }
}
