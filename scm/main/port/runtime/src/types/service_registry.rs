//! `ServiceRegistry` — stable egress client registry handed to handlers at construction time.

use std::collections::HashMap;
use std::sync::Arc;

use swe_edge_egress_grpc::GrpcEgress;
use swe_edge_egress_http::HttpEgress;

/// Holds egress clients that handlers may use to make outbound calls.
///
/// Constructed by `RuntimeBuilder::build_registry` and passed to handler
/// constructors at startup — not per-request.  Share it via
/// `Arc<ServiceRegistry>`.
///
/// Beyond the single default HTTP/gRPC client, a registry can hold
/// independently-resolved clients per named target service — e.g. a
/// load-balanced client for `"user-service"` backed by its own backend
/// pool, distinct from `"billing-service"`'s. Register these via
/// [`ServiceRegistry::with_service`]; look them up via
/// [`ServiceRegistry::service`]. See ADR-004 (backend-pool ownership moves
/// to the composition root, out of `edge-transport-http-egress`).
pub struct ServiceRegistry {
    http: Arc<dyn HttpEgress>,
    grpc: Option<Arc<dyn GrpcEgress>>,
    services: HashMap<String, Arc<dyn HttpEgress>>,
}

impl ServiceRegistry {
    /// Construct a registry from a default HTTP egress client and an optional gRPC client.
    pub fn new(http: Arc<dyn HttpEgress>, grpc: Option<Arc<dyn GrpcEgress>>) -> Self {
        Self {
            http,
            grpc,
            services: HashMap::new(),
        }
    }

    /// Register a named target service's own resolved HTTP client —
    /// typically one wrapping a load-balanced pool of backend URLs.
    /// Overwrites any previous registration under the same name.
    #[must_use]
    pub fn with_service(mut self, name: impl Into<String>, client: Arc<dyn HttpEgress>) -> Self {
        self.services.insert(name.into(), client);
        self
    }

    /// Return the default HTTP egress client.
    pub fn http(&self) -> &Arc<dyn HttpEgress> {
        &self.http
    }

    /// Return the gRPC egress client, if one was registered.
    pub fn grpc(&self) -> Option<&Arc<dyn GrpcEgress>> {
        self.grpc.as_ref()
    }

    /// Return the HTTP client registered for a named target service, if any.
    ///
    /// Returns `None` when `name` was never registered — this does **not**
    /// fall back to [`ServiceRegistry::http`]; callers that want the
    /// default client on a miss should do that explicitly, e.g.
    /// `registry.service(name).unwrap_or_else(|| registry.http())`.
    pub fn service(&self, name: &str) -> Option<&Arc<dyn HttpEgress>> {
        self.services.get(name)
    }

    /// Names of every registered target service, in unspecified order.
    pub fn service_names(&self) -> impl Iterator<Item = &str> {
        self.services.keys().map(String::as_str)
    }
}
