//! Runtime configuration assembly — the one thing this module owns is
//! "what bind addresses and optional sections does this demo run with."

use swe_edge_bootstrap::{MetricsConfig, RuntimeConfig};

pub(crate) const HTTP_BIND: &str = "127.0.0.1:18090";
pub(crate) const GRPC_BIND: &str = "127.0.0.1:19090";
pub(crate) const METRICS_BIND: &str = "127.0.0.1:18091";
pub(crate) const METRICS_PATH: &str = "/metrics";

/// Build the `RuntimeConfig`: HTTP + gRPC bind addresses, a separate
/// Prometheus metrics endpoint (activates `HttpLoadMonitor`/
/// `GrpcLoadMonitor` automatically), and gRPC reflection for tooling
/// like `grpcurl`.
pub(crate) fn build_runtime_config() -> RuntimeConfig {
    let mut config = RuntimeConfig::default()
        .with_service_name("full-landscape-demo")
        .with_http_bind(HTTP_BIND)
        .with_grpc_bind(GRPC_BIND)
        .with_systemd_notify(false);
    config.metrics = Some(MetricsConfig {
        bind: METRICS_BIND.into(),
        path: METRICS_PATH.into(),
    });
    config.grpc_reflection = true;
    config
}
