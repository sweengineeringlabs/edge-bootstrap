//! RuntimeConfig — process-level configuration for the daemon.

use edge_security_runtime_tls::PemTlsConfig;
use serde::{Deserialize, Serialize};
use swe_edge_egress_grpc::GrpcChannelConfig;
use swe_edge_egress_http::HttpConfig;
use swe_edge_ingress_verifier::JwtConfig;

pub use swe_edge_bootstrap_monitor::{AutoscalePolicy, MetricsConfig};
pub use swe_edge_observ_config::ObservabilityConfig;
/// Which `LoggerProvider` backend the real `ObserverContext` bridge ships
/// structured log entries to.
pub use swe_observ_logging::LoggingConfig as LogBackendConfig;
pub use swe_observ_logging::LoggingProvider as LogBackendKind;
pub use swe_observ_logging::{
    ElkSettings as LogElkSettings, FileSettings as LogFileSettings,
    OtelSettings as LogOtelSettings, SqliteSettings as LogSqliteSettings,
};
/// Which `MetricsProvider` backend stores load-monitor/autoscale counters —
/// distinct from [`MetricsConfig`], which is the Prometheus *scrape endpoint*
/// bind address/path, not the backend that actually stores the data.
pub use swe_observ_metrics::MetricsConfig as MetricsBackendConfig;
pub use swe_observ_metrics::{
    FileSettings as MetricsFileSettings, MetricsBackendKind, OtelSettings as MetricsOtelSettings,
    PrometheusSettings as MetricsPrometheusSettings, SqliteSettings as MetricsSqliteSettings,
};
pub use swe_observ_tracing::TracingBackendKind as TracerBackendKind;
/// Which `TracerProvider` backend the real `ObserverContext` bridge exports
/// spans to. Named distinctly from `swe_edge_observ_config::TracingConfig`
/// (the bare-`tracing`-crate console subscriber's own settings, a different
/// concern entirely — see `RuntimeBuilder::with_tracing`).
pub use swe_observ_tracing::TracingConfig as TracerBackendConfig;

#[cfg(feature = "intrusion")]
pub use edge_intrusion::config::Config as IntrusionConfig;

/// Egress configuration for one named target service.
///
/// Currently just a backend pool for HTTP load balancing, but kept as its
/// own struct (rather than aliasing `LoadbalancerConfig` directly) so
/// per-service settings unrelated to load balancing (e.g. a dedicated
/// `HttpConfig` override) can be added here later without changing the
/// `[services.<name>]` TOML shape.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ServiceEgressConfig {
    /// Backend pool for this service — strategy and weighted backend URLs
    /// (`[services.<name>.loadbalancer]`). Absent or empty backends means
    /// this service resolves to no client — see `RuntimeBuilder::build_registry`.
    pub loadbalancer: swe_edge_loadbalancer::LoadbalancerConfig,
}

/// Configuration for the runtime manager.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RuntimeConfig {
    /// Service name reported to observability and systemd.
    pub service_name: String,
    /// Address to bind the primary HTTP ingress server.
    pub http_bind: String,
    /// Address to bind the gRPC ingress server.
    pub grpc_bind: String,
    /// Seconds to wait for in-flight requests to drain on shutdown.
    pub shutdown_timeout_secs: u64,
    /// Emit systemd sd_notify signals (READY=1, STOPPING=1).
    pub systemd_notify: bool,
    /// Tenant identifier — `None` for single-tenant deployments.
    pub tenant_id: Option<String>,

    // ── TLS ───────────────────────────────────────────────────────────────────
    /// TLS/mTLS for the HTTP server.  Absent = plain HTTP.
    /// Set `client_ca_pem_path` to enable mTLS.
    pub http_tls: Option<PemTlsConfig>,
    /// TLS/mTLS for the gRPC server.  Absent = plain gRPC.
    pub grpc_tls: Option<PemTlsConfig>,

    // ── Auth ──────────────────────────────────────────────────────────────────
    /// JWT bearer auth for the HTTP server.  Absent = no token enforcement.
    pub http_auth: Option<JwtConfig>,
    /// Skip gRPC auth interceptor enforcement.  Default `false` = fail-closed.
    pub grpc_allow_unauthenticated: bool,

    // ── Egress ────────────────────────────────────────────────────────────────
    /// HTTP egress client config.  When set, `serve()` auto-builds the full
    /// middleware stack (auth, retry, rate, breaker, cache, TLS) using SWE
    /// defaults.  When absent, a plain default client is used.
    pub egress_http: Option<HttpConfig>,
    /// gRPC egress channel config.  When set, `serve()` auto-dials the
    /// channel.  When absent, no gRPC egress client is wired.
    pub egress_grpc: Option<GrpcChannelConfig>,
    /// Named target-service egress configs (`[services.<name>]`), each with
    /// its own independently load-balanced backend pool — e.g.
    /// `[services.user-service.loadbalancer]`. Absent or a name with no
    /// entry here means that service has no registered
    /// [`ServiceRegistry`](crate::ServiceRegistry) client; see
    /// `RuntimeBuilder::build_registry`. Backend-pool ownership moved here
    /// from `edge-transport-http-egress`'s `transport` crate — see ADR-004.
    #[serde(default)]
    pub services: std::collections::BTreeMap<String, ServiceEgressConfig>,

    // ── gRPC extras ───────────────────────────────────────────────────────────
    /// Auto-register the gRPC reflection service (`grpc.reflection.v1alpha`).
    /// Requires at least one `.grpc_route()` call so the service registry is
    /// populated.  Default `false`.
    pub grpc_reflection: bool,

    // ── Observability / auto-scaling ──────────────────────────────────────────
    /// Prometheus metrics endpoint.  Absent = metrics server not started.
    pub metrics: Option<MetricsConfig>,
    /// Which `MetricsProvider` backend stores load-monitor/autoscale counters
    /// (`[metrics_backend]` section).  Absent = the in-memory default.
    /// Overridden by `RuntimeBuilder::with_metrics_provider` when both are set.
    /// Has no effect if `metrics` is absent — no backend is constructed at
    /// all unless the scrape endpoint itself is configured.
    pub metrics_backend: Option<MetricsBackendConfig>,
    /// Which `TracerProvider` backend the real `ObserverContext` bridge
    /// exports spans to (`[tracer_backend]` section). Absent = the in-memory
    /// default. Overridden by `RuntimeBuilder::with_tracer_provider` when
    /// both are set. Note: `swe-observability-tracing` does not publicly
    /// re-export its `JaegerSettings`/`FileSettings`/`OtelSettings`/
    /// `SqliteSettings` types (unlike metrics/logging), so this section can
    /// only be populated via TOML/JSON deserialization — not constructed
    /// programmatically in Rust with a settings literal.
    pub tracer_backend: Option<TracerBackendConfig>,
    /// Which `LoggerProvider` backend the real `ObserverContext` bridge
    /// ships structured log entries to (`[log_backend]` section). Absent =
    /// the in-memory default. Overridden by
    /// `RuntimeBuilder::with_log_drain_backend` when both are set.
    pub log_backend: Option<LogBackendConfig>,
    /// Auto-scale threshold policy.  Checked every second by the sampler.
    /// Has no effect if `metrics` is absent.
    pub autoscale: Option<AutoscalePolicy>,

    // ── Observability ─────────────────────────────────────────────────────────
    /// Tracing subscriber and observability settings (`[observability]` section).
    /// Absent = use `TracingConfig` defaults when `with_tracing()` is called.
    pub observability: Option<ObservabilityConfig>,

    // ── Deployment ────────────────────────────────────────────────────────────
    /// Directory containing deployment artifacts (systemd units, manifests, etc.).
    /// Absent = use the bundled defaults shipped with the runtime.
    /// Consumers set this to their own directory to supply environment-specific
    /// deployment configurations without modifying the upstream repo.
    pub deploy_dir: Option<String>,

    // ── Intrusion detection / prevention ─────────────────────────────────────
    /// `edge-intrusion` IDS/IPS rules (`[intrusion]` section).  Absent = no
    /// intrusion guard wrapping the ingress handlers.  Overridden by
    /// `RuntimeBuilder::with_intrusion` when both are set.
    #[cfg(feature = "intrusion")]
    pub intrusion: Option<IntrusionConfig>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            service_name: "swe-edge".into(),
            http_bind: "0.0.0.0:8080".into(),
            grpc_bind: "0.0.0.0:50051".into(),
            shutdown_timeout_secs: 30,
            systemd_notify: false,
            tenant_id: None,
            http_tls: None,
            grpc_tls: None,
            http_auth: None,
            grpc_allow_unauthenticated: false,
            egress_http: None,
            egress_grpc: None,
            services: std::collections::BTreeMap::new(),
            grpc_reflection: false,
            metrics: None,
            metrics_backend: None,
            tracer_backend: None,
            log_backend: None,
            autoscale: None,
            observability: None,
            deploy_dir: None,
            #[cfg(feature = "intrusion")]
            intrusion: None,
        }
    }
}

impl RuntimeConfig {
    /// Override the service name reported to observability and systemd.
    pub fn with_service_name(mut self, name: impl Into<String>) -> Self {
        self.service_name = name.into();
        self
    }

    /// Override the bind address for the primary HTTP ingress server.
    pub fn with_http_bind(mut self, addr: impl Into<String>) -> Self {
        self.http_bind = addr.into();
        self
    }

    /// Override the bind address for the gRPC ingress server.
    pub fn with_grpc_bind(mut self, addr: impl Into<String>) -> Self {
        self.grpc_bind = addr.into();
        self
    }

    /// Override the graceful-shutdown drain timeout in seconds.
    pub fn with_shutdown_timeout(mut self, secs: u64) -> Self {
        self.shutdown_timeout_secs = secs;
        self
    }

    /// Enable or disable systemd sd_notify signals (READY=1, STOPPING=1).
    pub fn with_systemd_notify(mut self, enabled: bool) -> Self {
        self.systemd_notify = enabled;
        self
    }

    /// Set the tenant identifier for multi-tenant deployments.
    pub fn with_tenant_id(mut self, id: impl Into<String>) -> Self {
        self.tenant_id = Some(id.into());
        self
    }
}

/// Fluent builder for [`RuntimeConfig`].
struct RuntimeConfigBuilder {
    inner: RuntimeConfig,
}

impl RuntimeConfigBuilder {
    fn new() -> Self {
        Self {
            inner: RuntimeConfig::default(),
        }
    }
    fn service_name(mut self, v: impl Into<String>) -> Self {
        self.inner = self.inner.with_service_name(v);
        self
    }
    fn http_bind(mut self, v: impl Into<String>) -> Self {
        self.inner = self.inner.with_http_bind(v);
        self
    }
    fn grpc_bind(mut self, v: impl Into<String>) -> Self {
        self.inner = self.inner.with_grpc_bind(v);
        self
    }
    fn shutdown_timeout_secs(mut self, v: u64) -> Self {
        self.inner = self.inner.with_shutdown_timeout(v);
        self
    }
    fn systemd_notify(mut self, v: bool) -> Self {
        self.inner = self.inner.with_systemd_notify(v);
        self
    }
    fn tenant_id(mut self, v: impl Into<String>) -> Self {
        self.inner = self.inner.with_tenant_id(v);
        self
    }
    fn build(self) -> RuntimeConfig {
        self.inner
    }
}
