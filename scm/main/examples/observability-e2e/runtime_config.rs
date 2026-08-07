//! Runtime configuration assembly — the one thing this module owns is
//! "what bind addresses and optional observability backends this demo
//! serves with."

use std::path::PathBuf;

use swe_edge_bootstrap::{
    LogBackendConfig, LogBackendKind, LogFileSettings, MetricsConfig, RuntimeConfig,
};
#[cfg(feature = "observability")]
use swe_edge_bootstrap::{MetricsBackendConfig, MetricsBackendKind, MetricsPrometheusSettings};

pub(crate) const HTTP_BIND: &str = "127.0.0.1:18190";
pub(crate) const GRPC_BIND: &str = "127.0.0.1:19190";
pub(crate) const METRICS_BIND: &str = "127.0.0.1:18191";
pub(crate) const METRICS_PATH: &str = "/metrics";

/// Where the durable, file-backed log drain writes structured JSONL entries.
///
/// `LogDrainBridge::emit()` (`swe-edge-bootstrap`'s
/// `core::observability::log_drain_bridge`) flushes its backend on every
/// call, so every entry this demo emits is on disk before the handler
/// returns — killing the process mid-run loses nothing sitting in the
/// `FileLogger`'s `BufWriter`.
pub(crate) fn log_file_path() -> PathBuf {
    std::env::temp_dir()
        .join("swe-edge-bootstrap-observability-e2e")
        .join("handler.log.jsonl")
}

/// Build the `RuntimeConfig`: HTTP + gRPC bind addresses, a separate
/// Prometheus scrape endpoint, and a durable file-backed log drain.
///
/// `config.metrics` activates `HttpLoadMonitor`/`GrpcLoadMonitor`
/// automatically and starts the `/metrics` server regardless of which
/// features are compiled in. `config.metrics_backend` additionally swaps
/// the counters' storage from the framework's in-memory default to a real
/// `PrometheusMetrics` provider — the same instance backs both the load
/// monitor and `ObserverContext.metrics()`, one metrics stream, not two —
/// but only takes effect when the `observability` feature is compiled in;
/// see the field's own doc comment below for why it must stay unset
/// otherwise.
pub(crate) fn build_runtime_config() -> RuntimeConfig {
    let mut config = RuntimeConfig::default()
        .with_service_name("observability-e2e-demo")
        .with_http_bind(HTTP_BIND)
        .with_grpc_bind(GRPC_BIND)
        .with_systemd_notify(false);

    config.metrics = Some(MetricsConfig {
        bind: METRICS_BIND.into(),
        path: METRICS_PATH.into(),
    });

    // `RuntimeBuilderServe::build_metrics_provider` fails `serve()` outright
    // when `active` is a non-`Memory` kind and the `observability` feature
    // isn't compiled in, so this section must only be set in builds that
    // actually have it — the zero-feature build falls back to the
    // in-memory metrics provider instead, exactly like `metrics_backend`
    // being absent altogether.
    #[cfg(feature = "observability")]
    {
        config.metrics_backend = Some(MetricsBackendConfig {
            active: MetricsBackendKind::Prometheus,
            prometheus: Some(MetricsPrometheusSettings {
                endpoint: METRICS_BIND.to_string(),
                path: METRICS_PATH.to_string(),
            }),
            ..Default::default()
        });
    }

    // Real, durable log backend. Has no effect at all unless `observability`
    // is compiled in — `RuntimeBuilderServe::build_log_backend` is entirely
    // `#[cfg(feature = "observability")]`-gated — so setting it
    // unconditionally here is safe: the zero-feature build just ignores the
    // field and keeps the noop in-memory default.
    config.log_backend = Some(LogBackendConfig {
        active: LogBackendKind::File,
        file: Some(LogFileSettings {
            path: log_file_path().to_string_lossy().into_owned(),
            max_size_mb: 10,
            rotation: Default::default(),
        }),
        ..Default::default()
    });

    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_runtime_config_binds_http_grpc_and_metrics_to_distinct_ports() {
        let config = build_runtime_config();
        assert_eq!(config.http_bind, HTTP_BIND);
        assert_eq!(config.grpc_bind, GRPC_BIND);
        let metrics = config.metrics.expect("metrics section must be set");
        assert_eq!(metrics.bind, METRICS_BIND);
        assert_ne!(config.http_bind, metrics.bind);
        assert_ne!(config.grpc_bind, metrics.bind);
    }

    #[test]
    fn test_build_runtime_config_enables_file_log_backend_with_rotation() {
        let config = build_runtime_config();
        let backend = config.log_backend.expect("log backend must be set");
        assert_eq!(backend.active, LogBackendKind::File);
        let file = backend.file.expect("file settings must be set");
        assert!(
            file.path.ends_with("handler.log.jsonl"),
            "unexpected log path: {}",
            file.path
        );
        assert_eq!(file.max_size_mb, 10);
    }

    #[test]
    fn test_build_runtime_config_enables_metrics_scrape_endpoint() {
        let config = build_runtime_config();
        let metrics = config.metrics.expect("metrics section must be set");
        assert_eq!(metrics.bind, METRICS_BIND);
        assert_eq!(metrics.path, METRICS_PATH);
    }

    #[cfg(feature = "observability")]
    #[test]
    fn test_build_runtime_config_enables_prometheus_metrics_backend_with_observability() {
        let config = build_runtime_config();
        let backend = config
            .metrics_backend
            .expect("metrics backend must be set when the observability feature is compiled in");
        assert_eq!(backend.active, MetricsBackendKind::Prometheus);
        let prometheus = backend.prometheus.expect("prometheus settings must be set");
        assert_eq!(prometheus.endpoint, METRICS_BIND);
    }

    #[cfg(not(feature = "observability"))]
    #[test]
    fn test_build_runtime_config_leaves_metrics_backend_unset_without_observability() {
        let config = build_runtime_config();
        assert!(
            config.metrics_backend.is_none(),
            "metrics_backend must stay None without the observability feature — a non-Memory \
             active kind would fail serve() outright"
        );
    }

    #[test]
    fn test_log_file_path_is_absolute_and_scoped_to_this_demo() {
        let path = log_file_path();
        assert!(path.is_absolute(), "log path should be absolute: {path:?}");
        assert!(
            path.to_string_lossy()
                .contains("swe-edge-bootstrap-observability-e2e"),
            "log path should be scoped to this demo, got: {path:?}"
        );
    }
}
