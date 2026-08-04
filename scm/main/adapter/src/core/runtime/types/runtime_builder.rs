//! `RuntimeBuilder` — fluent builder for assembling an edge runtime.

use std::sync::Arc;

use edge_application::Handler;
use edge_proxy::LifecycleMonitor;
use edge_security_runtime_tls::PemTlsConfig;
use swe_edge_egress_grpc::GrpcEgress;
use swe_edge_egress_http::HttpEgress;
use swe_edge_ingress_grpc::{
    GrpcDecodeFn, GrpcEncodeFn, GrpcIngress, GrpcIngressInterceptor, GrpcIngressInterceptorChain,
};
use swe_edge_ingress_http::{HttpDecodeFn, HttpEncodeFn, HttpIngress, HttpStream};
use swe_edge_ingress_verifier::TokenVerifier;

use swe_edge_bootstrap_runtime::RuntimeConfig;
use swe_edge_bootstrap_runtime::ServiceRegistry;

use crate::core::dispatch::{DefaultGrpcJob, DefaultHttpJob};
use crate::core::egress::LoadBalancedHttpEgress;

/// Builder for assembling and starting an edge runtime.
pub struct RuntimeBuilder {
    pub(crate) config: Option<RuntimeConfig>,
    pub(crate) app_name: Option<String>,
    pub(crate) http_handler: Option<Arc<dyn HttpIngress>>,
    pub(crate) grpc_handler: Option<Arc<dyn GrpcIngress>>,
    pub(crate) http_job: Option<Arc<DefaultHttpJob>>,
    pub(crate) grpc_job: Option<Arc<DefaultGrpcJob>>,
    pub(crate) http_tls: Option<PemTlsConfig>,
    pub(crate) grpc_tls: Option<PemTlsConfig>,
    pub(crate) http_bearer_verifier: Option<Arc<dyn TokenVerifier>>,
    pub(crate) grpc_interceptors: GrpcIngressInterceptorChain,
    pub(crate) grpc_allow_unauthenticated: bool,
    pub(crate) egress_http: Option<Arc<dyn HttpEgress>>,
    pub(crate) egress_grpc: Option<Arc<dyn GrpcEgress>>,
    pub(crate) lifecycle: Option<Arc<dyn LifecycleMonitor>>,
    pub(crate) tracing_config: Option<swe_edge_observ_config::TracingConfig>,
    pub(crate) stream_handler: Option<Arc<dyn HttpStream>>,
    pub(crate) metrics_provider: Option<Arc<dyn swe_observ_metrics::MetricsProvider>>,
    #[cfg(feature = "observability")]
    pub(crate) tracer_provider: Option<Arc<dyn swe_observ_tracing::TracerProvider>>,
    #[cfg(feature = "observability")]
    pub(crate) log_drain_backend: Option<Arc<dyn swe_observ_logging::LoggerProvider>>,
    #[cfg(feature = "observability")]
    pub(crate) observer_context_override:
        Option<Arc<dyn edge_application_observer::ObserverContext>>,
    #[cfg(feature = "message-broker")]
    pub(crate) message_broker: Option<Arc<dyn swe_edge_runtime_message_broker::MessageBroker>>,
    #[cfg(feature = "intrusion")]
    pub(crate) intrusion: Option<edge_intrusion::config::Wired>,
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
    pub fn http_route<Req, Resp>(
        self,
        handler: Arc<dyn Handler<Request = Req, Response = Resp>>,
    ) -> Self
    where
        Req: serde::de::DeserializeOwned + Send + 'static + edge_application::Request,
        Resp: serde::Serialize + Send + 'static + edge_application::Response,
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
        handler: Arc<dyn Handler<Request = Req, Response = Resp>>,
        decode: HttpDecodeFn<Req>,
        encode: HttpEncodeFn<Resp>,
    ) -> Self
    where
        Req: Send + 'static + edge_application::Request,
        Resp: Send + 'static + edge_application::Response,
    {
        let job = self
            .http_job
            .get_or_insert_with(|| Arc::new(DefaultHttpJob::new()));
        job.register_route(handler, decode, encode)
            .expect("duplicate HTTP route");
        self
    }

    /// Register a gRPC handler using JSON encode/decode.
    pub fn grpc_route<Req, Resp>(
        self,
        handler: Arc<dyn Handler<Request = Req, Response = Resp>>,
    ) -> Self
    where
        Req: serde::de::DeserializeOwned + Send + 'static + edge_application::Request,
        Resp: serde::Serialize + Send + 'static + edge_application::Response,
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
        handler: Arc<dyn Handler<Request = Req, Response = Resp>>,
        decode: GrpcDecodeFn<Req>,
        encode: GrpcEncodeFn<Resp>,
    ) -> Self
    where
        Req: Send + 'static + edge_application::Request,
        Resp: Send + 'static + edge_application::Response,
    {
        let job = self
            .grpc_job
            .get_or_insert_with(|| Arc::new(DefaultGrpcJob::new()));
        job.register_route(handler, decode, encode)
            .expect("duplicate gRPC route");
        self
    }

    /// Install a tracing subscriber before `serve()` starts.
    ///
    /// Takes precedence over `[observability.tracing]` in TOML config.
    /// Idempotent — safe to call in tests where a subscriber may already be installed.
    #[cfg(feature = "observability")]
    pub fn with_tracing(mut self, config: swe_edge_observ_config::TracingConfig) -> Self {
        self.tracing_config = Some(config);
        self
    }

    /// Attach a TLS configuration to the HTTP server.
    pub fn http_tls(mut self, config: PemTlsConfig) -> Self {
        self.http_tls = Some(config);
        self
    }
    /// Attach a TLS configuration to the gRPC server.
    pub fn grpc_tls(mut self, config: PemTlsConfig) -> Self {
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
    /// [`RuntimeHealth`]: crate::RuntimeHealth
    #[cfg(feature = "message-broker")]
    pub fn with_message_broker(
        mut self,
        broker: impl swe_edge_runtime_message_broker::MessageBroker + 'static,
    ) -> Self {
        self.message_broker = Some(Arc::new(broker));
        self
    }

    /// Attach a pre-built `edge-intrusion` IDS/IPS wiring, wrapping both the
    /// HTTP and gRPC ingress handlers.
    ///
    /// Takes precedence over `[intrusion]` in TOML config. Construct `Wired`
    /// via `edge_intrusion::config::Config::build()`.
    #[cfg(feature = "intrusion")]
    pub fn with_intrusion(mut self, wired: edge_intrusion::config::Wired) -> Self {
        self.intrusion = Some(wired);
        self
    }

    /// Attach a `MetricsProvider` backend for load-monitor/autoscale counters.
    ///
    /// Takes precedence over `[metrics_backend]` in TOML config. Absent
    /// (here and in config) means the in-memory default backend, as before —
    /// this replaces `create_local_metrics_backend()`'s hardcoded, unconfigurable
    /// choice with a real seam a consumer can plug Prometheus/OTel/file/SQLite
    /// (or their own implementation) into.
    pub fn with_metrics_provider(
        mut self,
        provider: Arc<dyn swe_observ_metrics::MetricsProvider>,
    ) -> Self {
        self.metrics_provider = Some(provider);
        self
    }

    /// Attach a `TracerProvider` backend for the real `ObserverContext`
    /// bridge's spans.
    ///
    /// Takes precedence over `[tracer_backend]` in TOML config. Absent
    /// (here and in config) means the in-memory default backend.
    #[cfg(feature = "observability")]
    pub fn with_tracer_provider(
        mut self,
        provider: Arc<dyn swe_observ_tracing::TracerProvider>,
    ) -> Self {
        self.tracer_provider = Some(provider);
        self
    }

    /// Attach a `LoggerProvider` backend for the real `ObserverContext`
    /// bridge's structured log entries.
    ///
    /// Takes precedence over `[log_backend]` in TOML config. Absent (here
    /// and in config) means the in-memory default backend.
    #[cfg(feature = "observability")]
    pub fn with_log_drain_backend(
        mut self,
        backend: Arc<dyn swe_observ_logging::LoggerProvider>,
    ) -> Self {
        self.log_drain_backend = Some(backend);
        self
    }

    /// Supply a complete `ObserverContext` implementation, replacing the
    /// internally-composed one outright.
    ///
    /// This is a full override, not a per-primitive merge: when set, none of
    /// `with_tracer_provider`/`with_log_drain_backend`/`with_metrics_provider`
    /// (or their TOML-config equivalents) have any effect on
    /// `HandlerContext.observer` — the supplied `ObserverContext` is used
    /// as-is. Use those other methods instead if you only want to swap one
    /// primitive's backend while keeping the other two as this crate
    /// composes them.
    #[cfg(feature = "observability")]
    pub fn with_observer_context(
        mut self,
        observer: Arc<dyn edge_application_observer::ObserverContext>,
    ) -> Self {
        self.observer_context_override = Some(observer);
        self
    }

    /// Build a [`ServiceRegistry`] from the configured egress clients, if any.
    ///
    /// Returns `None` unless [`RuntimeBuilder::egress_http`] was called —
    /// `ServiceRegistry` always needs a default HTTP client, and this method
    /// does no config-driven or XDG fallback construction of its own (that
    /// resolution happens in `serve()`, which this method runs independent
    /// of — call it before `.serve()` if you need the registry for handler
    /// construction).
    ///
    /// When [`RuntimeBuilder::config`] was also called and its
    /// [`RuntimeConfig::services`] map has entries, each named service with
    /// a non-empty `[services.<name>.loadbalancer]` backend list is
    /// registered as its own [`LoadBalancedHttpEgress`], wrapping the same
    /// default HTTP client resolved above — see ADR-004. A service whose
    /// backend pool fails to build (e.g. a backend with an empty URL or
    /// zero weight) is skipped with a `tracing::warn!`, not a hard error —
    /// one misconfigured service shouldn't prevent the rest of the registry
    /// from being usable.
    pub fn build_registry(&self) -> Option<Arc<ServiceRegistry>> {
        let default_http = self.egress_http.as_ref()?;
        let mut registry = ServiceRegistry::new(Arc::clone(default_http), self.egress_grpc.clone());

        if let Some(config) = self.config.as_ref() {
            for (name, service_config) in &config.services {
                if service_config.loadbalancer.backends.is_empty() {
                    continue;
                }
                match LoadBalancedHttpEgress::new(
                    Arc::clone(default_http),
                    service_config.loadbalancer.clone(),
                ) {
                    Ok(client) => {
                        registry = registry.with_service(name.clone(), Arc::new(client));
                    }
                    Err(error) => {
                        tracing::warn!(
                            service = %name,
                            %error,
                            "skipping service: invalid [services.<name>.loadbalancer] config"
                        );
                    }
                }
            }
        }

        Some(Arc::new(registry))
    }
}
