//! `ConfigOverride` — partial TOML overlay applied over a `RuntimeConfig`.

use crate::api::config::error::config_error::ConfigError;
use crate::api::monitor::{AutoscalePolicy, MetricsConfig};
use crate::api::runtime::RuntimeConfig;
use serde::Deserialize;
use swe_edge_egress_grpc::GrpcChannelConfig;
use swe_edge_egress_http::HttpConfig;
use swe_edge_ingress_http::IngressTlsConfig;
use swe_edge_ingress_verifier::JwtConfig;
use swe_edge_observ_config::ObservabilityConfig;

/// A partial `RuntimeConfig` — all fields optional so any
/// subset of keys can be present in a TOML overlay file.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct ConfigOverride {
    pub(crate) service_name: Option<String>,
    pub(crate) http_bind: Option<String>,
    pub(crate) grpc_bind: Option<String>,
    pub(crate) shutdown_timeout_secs: Option<u64>,
    pub(crate) systemd_notify: Option<bool>,
    pub(crate) tenant_id: Option<String>,
    pub(crate) http_tls: Option<IngressTlsConfig>,
    pub(crate) grpc_tls: Option<IngressTlsConfig>,
    pub(crate) http_auth: Option<JwtConfig>,
    pub(crate) grpc_allow_unauthenticated: Option<bool>,
    pub(crate) egress_http: Option<HttpConfig>,
    pub(crate) egress_grpc: Option<GrpcChannelConfig>,
    pub(crate) grpc_reflection: Option<bool>,
    pub(crate) metrics: Option<MetricsConfig>,
    pub(crate) autoscale: Option<AutoscalePolicy>,
    pub(crate) observability: Option<ObservabilityConfig>,
}

impl ConfigOverride {
    pub(crate) fn from_str(s: &str) -> Result<Self, ConfigError> {
        toml::from_str(s).map_err(|e| ConfigError::Parse(e.to_string()))
    }

    pub(crate) fn apply_to(self, mut base: RuntimeConfig) -> RuntimeConfig {
        if let Some(v) = self.service_name {
            base.service_name = v;
        }
        if let Some(v) = self.http_bind {
            base.http_bind = v;
        }
        if let Some(v) = self.grpc_bind {
            base.grpc_bind = v;
        }
        if let Some(v) = self.shutdown_timeout_secs {
            base.shutdown_timeout_secs = v;
        }
        if let Some(v) = self.systemd_notify {
            base.systemd_notify = v;
        }
        if let Some(v) = self.tenant_id {
            if !v.is_empty() {
                base.tenant_id = Some(v);
            }
        }
        if let Some(v) = self.http_tls {
            base.http_tls = Some(v);
        }
        if let Some(v) = self.grpc_tls {
            base.grpc_tls = Some(v);
        }
        if let Some(v) = self.http_auth {
            base.http_auth = Some(v);
        }
        if let Some(v) = self.grpc_allow_unauthenticated {
            base.grpc_allow_unauthenticated = v;
        }
        if let Some(v) = self.egress_http {
            base.egress_http = Some(v);
        }
        if let Some(v) = self.egress_grpc {
            base.egress_grpc = Some(v);
        }
        if let Some(v) = self.grpc_reflection {
            base.grpc_reflection = v;
        }
        if let Some(v) = self.metrics {
            base.metrics = Some(v);
        }
        if let Some(v) = self.autoscale {
            base.autoscale = Some(v);
        }
        if let Some(v) = self.observability {
            base.observability = Some(v);
        }
        base
    }
}

/// Fluent builder for [`ConfigOverride`].
struct ConfigOverrideBuilder {
    inner: ConfigOverride,
}

impl ConfigOverrideBuilder {
    fn new() -> Self {
        Self {
            inner: ConfigOverride::default(),
        }
    }
    fn service_name(mut self, v: impl Into<String>) -> Self {
        self.inner.service_name = Some(v.into());
        self
    }
    fn http_bind(mut self, v: impl Into<String>) -> Self {
        self.inner.http_bind = Some(v.into());
        self
    }
    fn grpc_bind(mut self, v: impl Into<String>) -> Self {
        self.inner.grpc_bind = Some(v.into());
        self
    }
    fn shutdown_timeout_secs(mut self, v: u64) -> Self {
        self.inner.shutdown_timeout_secs = Some(v);
        self
    }
    fn systemd_notify(mut self, v: bool) -> Self {
        self.inner.systemd_notify = Some(v);
        self
    }
    fn tenant_id(mut self, v: impl Into<String>) -> Self {
        self.inner.tenant_id = Some(v.into());
        self
    }
    fn grpc_allow_unauthenticated(mut self, v: bool) -> Self {
        self.inner.grpc_allow_unauthenticated = Some(v);
        self
    }
    fn grpc_reflection(mut self, v: bool) -> Self {
        self.inner.grpc_reflection = Some(v);
        self
    }
    fn build(self) -> ConfigOverride {
        self.inner
    }
}
