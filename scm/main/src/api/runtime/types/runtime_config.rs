//! RuntimeConfig — process-level configuration for the daemon.

use serde::{Deserialize, Serialize};
use swe_edge_egress_grpc::GrpcChannelConfig;
use swe_edge_egress_http::HttpConfig;
use swe_edge_ingress_http::IngressTlsConfig;
use swe_edge_ingress_verifier::JwtConfig;

pub use crate::api::config::ObservabilityConfig;
pub use crate::api::monitor::{AutoscalePolicy, MetricsConfig};

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
    pub http_tls: Option<IngressTlsConfig>,
    /// TLS/mTLS for the gRPC server.  Absent = plain gRPC.
    pub grpc_tls: Option<IngressTlsConfig>,

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

    // ── gRPC extras ───────────────────────────────────────────────────────────
    /// Auto-register the gRPC reflection service (`grpc.reflection.v1alpha`).
    /// Requires at least one `.grpc_route()` call so the service registry is
    /// populated.  Default `false`.
    pub grpc_reflection: bool,

    // ── Observability / auto-scaling ──────────────────────────────────────────
    /// Prometheus metrics endpoint.  Absent = metrics server not started.
    pub metrics: Option<MetricsConfig>,
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
            grpc_reflection: false,
            metrics: None,
            autoscale: None,
            observability: None,
            deploy_dir: None,
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
