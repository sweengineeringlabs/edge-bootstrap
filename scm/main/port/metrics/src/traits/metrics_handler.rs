//! `MetricsHandler` — Prometheus-compatible metrics HTTP endpoint interface.

use crate::traits::metrics_exporter::MetricsExporter;
use swe_edge_ingress_http::HttpIngress;

/// Supertrait for Prometheus metrics HTTP endpoint handlers.
///
/// A metrics handler is also a [`MetricsExporter`]: it serves over HTTP the
/// same counters/path it exposes through the exporter contract.
pub trait MetricsHandler: HttpIngress + MetricsExporter {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use swe_edge_bootstrap_monitor::{SharedCounters, TrafficCounters};
    use swe_edge_ingress_http::{
        HealthCheckRequest, HealthCheckResponse, HttpFuture, HttpHealthCheck, HttpIngressError,
        HttpResponse, InboundRequest,
    };
    use swe_observ_metrics::create_local_metrics_backend;

    struct MetricsHandlerDouble {
        counters: SharedCounters,
        path: String,
    }

    impl HttpIngress for MetricsHandlerDouble {
        fn handle(
            &self,
            _: InboundRequest,
        ) -> HttpFuture<'_, Result<HttpResponse, HttpIngressError>> {
            HttpFuture::new(async { Ok(HttpResponse::new(200, vec![])) })
        }
        fn health_check(
            &self,
            _: HealthCheckRequest,
        ) -> HttpFuture<'_, Result<HealthCheckResponse, HttpIngressError>> {
            HttpFuture::new(async {
                Ok(HealthCheckResponse {
                    health: HttpHealthCheck::healthy(),
                })
            })
        }
    }

    impl MetricsExporter for MetricsHandlerDouble {
        fn counters(&self) -> &SharedCounters {
            &self.counters
        }
        fn path(&self) -> &str {
            &self.path
        }
    }

    impl MetricsHandler for MetricsHandlerDouble {}

    #[test]
    fn test_metrics_handler_double_satisfies_both_supertraits() {
        let d = MetricsHandlerDouble {
            counters: Arc::new(TrafficCounters::new(Arc::new(
                create_local_metrics_backend(),
            ))),
            path: "/metrics".into(),
        };
        let as_exporter: &dyn MetricsExporter = &d;
        assert_eq!(as_exporter.path(), "/metrics");
        let as_ingress: &dyn HttpIngress = &d;
        let _ = as_ingress;
    }
}
