//! `RuntimeBuilder` — fluent builder for assembling an edge runtime.

use std::sync::Arc;

use edge_domain::{Handler, HandlerRegistry};
use edge_proxy::LifecycleMonitor;
use swe_edge_egress_grpc::GrpcEgress;
use swe_edge_egress_http::HttpEgress;
use swe_edge_ingress_grpc::{
    GrpcDecodeFn, GrpcEncodeFn, GrpcHandlerAdapter, GrpcHandlerRegistryDispatcher, GrpcIngress,
    GrpcIngressInterceptor, GrpcIngressInterceptorChain,
};
use swe_edge_ingress_http::{
    HttpDecodeFn, HttpEncodeFn, HttpHandlerAdapter, HttpHandlerRegistryDispatcher, HttpIngress,
    HttpStream, IngressTlsConfig,
};
use swe_edge_ingress_verifier::TokenVerifier;

use crate::api::runtime::types::runtime_config::RuntimeConfig;
use crate::api::runtime::types::service_registry::ServiceRegistry;

/// Builder for assembling and starting an edge runtime.
pub struct RuntimeBuilder {
    pub(crate) config: Option<RuntimeConfig>,
    pub(crate) app_name: Option<String>,
    pub(crate) http_handler: Option<Arc<dyn HttpIngress>>,
    pub(crate) grpc_handler: Option<Arc<dyn GrpcIngress>>,
    pub(crate) http_dispatcher: Option<HttpHandlerRegistryDispatcher>,
    pub(crate) grpc_dispatcher: Option<GrpcHandlerRegistryDispatcher>,
    pub(crate) http_tls: Option<IngressTlsConfig>,
    pub(crate) grpc_tls: Option<IngressTlsConfig>,
    pub(crate) http_bearer_verifier: Option<Arc<dyn TokenVerifier>>,
    pub(crate) grpc_interceptors: GrpcIngressInterceptorChain,
    pub(crate) grpc_allow_unauthenticated: bool,
    pub(crate) egress_http: Option<Arc<dyn HttpEgress>>,
    pub(crate) egress_grpc: Option<Arc<dyn GrpcEgress>>,
    pub(crate) lifecycle: Option<Arc<dyn LifecycleMonitor>>,
    pub(crate) tracing_config: Option<crate::api::config::TracingConfig>,
    pub(crate) stream_handler: Option<Arc<dyn HttpStream>>,
    #[cfg(feature = "message-broker")]
    pub(crate) message_broker: Option<Arc<dyn swe_edge_runtime_message_broker::MessageBroker>>,
}

impl RuntimeBuilder {
    /// Override the default TOML config with an explicit [`RuntimeConfig`].
    pub fn config(mut self, config: RuntimeConfig) -> Self {
        self.config = Some(config);
        self
    }
    /// Set the application name used for XDG config path resolution.
    pub fn app_name(mut self, name: impl Into<String>) -> Self {
        self.app_name = Some(name.into());
        self
    }

    /// Register an HTTP handler using JSON encode/decode.
    pub fn http_route<Req, Resp>(self, handler: Arc<dyn Handler<Req, Resp>>) -> Self
    where
        Req: serde::de::DeserializeOwned + Send + 'static,
        Resp: serde::Serialize + Send + 'static,
    {
        self.http_route_with(
            handler,
            crate::core::json::codec::Codec::json_decode::<Req>,
            crate::core::json::codec::Codec::json_encode::<Resp>,
        )
    }

    /// Register an HTTP handler with custom decode and encode functions.
    pub fn http_route_with<Req, Resp>(
        mut self,
        handler: Arc<dyn Handler<Req, Resp>>,
        decode: HttpDecodeFn<Req>,
        encode: HttpEncodeFn<Resp>,
    ) -> Self
    where
        Req: Send + 'static,
        Resp: Send + 'static,
    {
        let d = self.http_dispatcher.get_or_insert_with(|| {
            HttpHandlerRegistryDispatcher::new(Arc::new(HandlerRegistry::new()))
        });
        d.register(HttpHandlerAdapter::new(handler, decode, encode))
            .expect("duplicate HTTP route");
        self
    }

    /// Register a gRPC handler using JSON encode/decode.
    pub fn grpc_route<Req, Resp>(self, handler: Arc<dyn Handler<Req, Resp>>) -> Self
    where
        Req: serde::de::DeserializeOwned + Send + 'static,
        Resp: serde::Serialize + Send + 'static,
    {
        self.grpc_route_with(
            handler,
            crate::core::json::codec::Codec::grpc_json_decode::<Req>,
            crate::core::json::codec::Codec::grpc_json_encode::<Resp>,
        )
    }

    /// Register a gRPC handler with custom decode and encode functions.
    pub fn grpc_route_with<Req, Resp>(
        mut self,
        handler: Arc<dyn Handler<Req, Resp>>,
        decode: GrpcDecodeFn<Req>,
        encode: GrpcEncodeFn<Resp>,
    ) -> Self
    where
        Req: Send + 'static,
        Resp: Send + 'static,
    {
        let d = self.grpc_dispatcher.get_or_insert_with(|| {
            GrpcHandlerRegistryDispatcher::new(Arc::new(HandlerRegistry::new()))
        });
        d.register(GrpcHandlerAdapter::new(handler, decode, encode));
        self
    }

    /// Install a tracing subscriber before `serve()` starts.
    ///
    /// Takes precedence over `[observability.tracing]` in TOML config.
    /// Idempotent — safe to call in tests where a subscriber may already be installed.
    #[cfg(feature = "observability")]
    pub fn with_tracing(mut self, config: crate::api::config::TracingConfig) -> Self {
        self.tracing_config = Some(config);
        self
    }

    /// Attach a TLS configuration to the HTTP server.
    pub fn http_tls(mut self, config: IngressTlsConfig) -> Self {
        self.http_tls = Some(config);
        self
    }
    /// Attach a TLS configuration to the gRPC server.
    pub fn grpc_tls(mut self, config: IngressTlsConfig) -> Self {
        self.grpc_tls = Some(config);
        self
    }
    /// Attach a JWT bearer token verifier to the HTTP server.
    pub fn http_bearer_auth(mut self, verifier: Arc<dyn TokenVerifier>) -> Self {
        self.http_bearer_verifier = Some(verifier);
        self
    }
    /// Append a gRPC inbound interceptor (e.g. auth, authz).
    pub fn grpc_auth(mut self, interceptor: Arc<dyn GrpcIngressInterceptor>) -> Self {
        self.grpc_interceptors = self.grpc_interceptors.push(interceptor);
        self
    }
    /// Allow gRPC requests without an `AuthorizationInterceptor` registered.
    pub fn grpc_allow_unauthenticated(mut self) -> Self {
        self.grpc_allow_unauthenticated = true;
        self
    }
    /// Override the default egress HTTP client.
    pub fn egress_http(mut self, client: Arc<dyn HttpEgress>) -> Self {
        self.egress_http = Some(client);
        self
    }
    /// Attach an egress gRPC client.
    pub fn egress_grpc(mut self, client: Arc<dyn GrpcEgress>) -> Self {
        self.egress_grpc = Some(client);
        self
    }
    /// Attach a lifecycle monitor (health, start/stop hooks).
    pub fn lifecycle(mut self, monitor: Arc<dyn LifecycleMonitor>) -> Self {
        self.lifecycle = Some(monitor);
        self
    }
    /// Supply a pre-built HTTP inbound handler instead of using registered routes.
    pub fn http_handler(mut self, handler: Arc<dyn HttpIngress>) -> Self {
        self.http_handler = Some(handler);
        self
    }
    /// Supply a pre-built gRPC inbound handler instead of using registered routes.
    pub fn grpc_handler(mut self, handler: Arc<dyn GrpcIngress>) -> Self {
        self.grpc_handler = Some(handler);
        self
    }
    /// Attach a streaming handler for SSE and WebSocket requests.
    ///
    /// When set, `Accept: text/event-stream` requests are routed to
    /// [`HttpStream::handle_sse`] and `Upgrade: websocket` requests to
    /// [`HttpStream::handle_websocket`] instead of falling through to
    /// [`HttpIngress::handle`].
    pub fn stream_handler(mut self, handler: Arc<dyn HttpStream>) -> Self {
        self.stream_handler = Some(handler);
        self
    }

    /// Attach a message broker for health monitoring during runtime lifecycle.
    ///
    /// The runtime probes [`MessageBroker::health_check`] on startup and
    /// includes `"message-broker"` in every [`RuntimeHealth`] report.
    ///
    /// [`MessageBroker::health_check`]: swe_edge_runtime_message_broker::MessageBroker::health_check
    /// [`RuntimeHealth`]: crate::api::runtime::types::health::RuntimeHealth
    #[cfg(feature = "message-broker")]
    pub fn with_message_broker(
        mut self,
        broker: impl swe_edge_runtime_message_broker::MessageBroker + 'static,
    ) -> Self {
        self.message_broker = Some(Arc::new(broker));
        self
    }

    /// Build a [`ServiceRegistry`] from the configured egress clients, if any.
    pub fn build_registry(&self) -> Option<Arc<ServiceRegistry>> {
        self.egress_http.as_ref().map(|http| {
            Arc::new(ServiceRegistry::new(
                Arc::clone(http),
                self.egress_grpc.clone(),
            ))
        })
    }
}
