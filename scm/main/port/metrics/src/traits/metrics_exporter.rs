//! API declarations for the Prometheus metrics HTTP handler.

use swe_edge_bootstrap_monitor::SharedCounters;

/// Contract for serving the Prometheus text exposition endpoint.
pub trait MetricsExporter: Send + Sync {
    /// Bind address and path are determined by `MetricsConfig`.
    fn counters(&self) -> &SharedCounters;
    /// URL path this exporter is served under, e.g. `/metrics`.
    fn path(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use swe_edge_bootstrap_monitor::TrafficCounters;
    use swe_observ_metrics::create_local_metrics_backend;

    struct MetricsExporterDouble {
        counters: SharedCounters,
        path: String,
    }

    impl MetricsExporter for MetricsExporterDouble {
        fn counters(&self) -> &SharedCounters {
            &self.counters
        }
        fn path(&self) -> &str {
            &self.path
        }
    }

    fn counters() -> SharedCounters {
        Arc::new(TrafficCounters::new(Arc::new(
            create_local_metrics_backend(),
        )))
    }

    #[test]
    fn test_metrics_exporter_double_path_matches_configured_value() {
        let d = MetricsExporterDouble {
            counters: counters(),
            path: "/metrics".into(),
        };
        assert_eq!(d.path(), "/metrics");
    }

    #[test]
    fn test_metrics_exporter_double_counters_is_same_instance() {
        let c = counters();
        let d = MetricsExporterDouble {
            counters: Arc::clone(&c),
            path: "/metrics".into(),
        };
        assert!(Arc::ptr_eq(d.counters(), &c));
    }
}
