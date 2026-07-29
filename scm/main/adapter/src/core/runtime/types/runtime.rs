//! `Runtime` — zero-size entry-point; use `Runtime::builder()`.

use crate::core::runtime::types::runtime_builder::RuntimeBuilder;
use swe_edge_ingress_grpc::GrpcIngressInterceptorChain;

/// Entry-point for the edge runtime.
pub struct Runtime;

impl Runtime {
    /// Create a new builder for assembling an edge runtime.
    pub fn builder() -> RuntimeBuilder {
        RuntimeBuilder {
            config: None,
            app_name: None,
            http_handler: None,
            grpc_handler: None,
            http_dispatcher: None,
            grpc_dispatcher: None,
            http_tls: None,
            grpc_tls: None,
            http_bearer_verifier: None,
            grpc_interceptors: GrpcIngressInterceptorChain::new(),
            grpc_allow_unauthenticated: false,
            egress_http: None,
            egress_grpc: None,
            lifecycle: None,
            tracing_config: None,
            stream_handler: None,
            #[cfg(feature = "message-broker")]
            message_broker: None,
            #[cfg(feature = "intrusion")]
            intrusion: None,
        }
    }
}
