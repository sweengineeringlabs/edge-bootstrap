//! Prometheus metrics HTTP handler — serves `GET /metrics`.
//!
//! Calls `MetricsProvider::export()` and formats the resulting
//! `Vec<MetricSnapshot>` as Prometheus text exposition (version 0.0.4).

use std::sync::Arc;

use swe_edge_ingress_http::{
    HealthCheckRequest, HealthCheckResponse, HttpFuture, HttpHealthCheck, HttpIngress,
    HttpIngressError, HttpMethod, HttpRequest, HttpResponse, InboundRequest,
};
use swe_observ_metrics::{MetricType, MetricsProvider};

use swe_edge_bootstrap_monitor::SharedCounters;

/// Serves the Prometheus text exposition endpoint.
///
/// Bound to `[metrics] bind` (default `0.0.0.0:9090`) — never on
/// the primary `http_bind`.  Responds only to the configured `path`
/// (default `/metrics`); all other paths return `404 Not Found`.
pub(crate) struct MetricsHandler {
    counters: SharedCounters,
    path: String,
}

impl MetricsHandler {
    pub(crate) fn new(counters: SharedCounters, path: impl Into<String>) -> Self {
        Self {
            counters,
            path: path.into(),
        }
    }

    /// Format `MetricSnapshot` values as Prometheus text exposition.
    fn render_prometheus(provider: &dyn MetricsProvider) -> String {
        let mut out = String::new();
        for snap in provider.export() {
            let type_str = match snap.metric_type {
                MetricType::Counter => "counter",
                MetricType::Gauge => "gauge",
                MetricType::Histogram => "gauge", // exported as gauge via export()
            };
            out.push_str(&format!("# TYPE {} {}\n", snap.name, type_str));
            let labels = if snap.labels.is_empty() {
                String::new()
            } else {
                let parts: Vec<String> = snap
                    .labels
                    .iter()
                    .map(|(k, v)| format!("{k}=\"{v}\""))
                    .collect();
                format!("{{{}}}", parts.join(","))
            };
            out.push_str(&format!("{}{} {}\n", snap.name, labels, snap.value));
        }
        out
    }
}

impl swe_edge_bootstrap_metrics::traits::metrics_handler::MetricsHandler for MetricsHandler {}

impl swe_edge_bootstrap_metrics::MetricsExporter for MetricsHandler {
    fn counters(&self) -> &SharedCounters {
        &self.counters
    }

    fn path(&self) -> &str {
        &self.path
    }
}

impl HttpIngress for MetricsHandler {
    fn handle(
        &self,
        req: InboundRequest,
    ) -> HttpFuture<'_, Result<HttpResponse, HttpIngressError>> {
        let provider = Arc::clone(&self.counters.provider);
        let path = self.path.clone();
        let request = req.request;
        HttpFuture::new(async move {
            if request.method != HttpMethod::Get {
                return Err(HttpIngressError::InvalidInput(
                    "metrics endpoint only accepts GET".into(),
                ));
            }
            let req_path = request
                .url
                .split('?')
                .next()
                .unwrap_or(&request.url)
                .trim_end_matches('/');
            let cfg_path = path.trim_end_matches('/');
            if req_path != cfg_path {
                return Err(HttpIngressError::NotFound(format!(
                    "not found: {}",
                    request.url
                )));
            }
            let body = MetricsHandler::render_prometheus(&*provider);
            let mut resp = HttpResponse::new(200, body.into_bytes());
            resp.headers.insert(
                "content-type".into(),
                "text/plain; version=0.0.4; charset=utf-8".into(),
            );
            Ok(resp)
        })
    }

    fn health_check(
        &self,
        _req: HealthCheckRequest,
    ) -> HttpFuture<'_, Result<HealthCheckResponse, HttpIngressError>> {
        HttpFuture::new(async {
            Ok(HealthCheckResponse {
                health: HttpHealthCheck::healthy(),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swe_edge_bootstrap_monitor::TrafficCounters;
    use std::sync::Arc;
    use swe_observ_metrics::create_local_metrics_backend;

    fn handler_with_data() -> MetricsHandler {
        let provider = Arc::new(create_local_metrics_backend());
        provider.record_counter("edge_requests_total", 42.0, &[]);
        provider.record_gauge("edge_requests_active", 7.0, &[]);
        let counters = Arc::new(TrafficCounters::new(provider));
        MetricsHandler::new(counters, "/metrics")
    }

    fn inbound(request: HttpRequest) -> InboundRequest {
        InboundRequest::new(
            request,
            swe_edge_ingress_http::RequestContext::new(
                edge_domain::SecurityContext::unauthenticated(),
            ),
            std::net::SocketAddr::from(([127, 0, 0, 1], 0)),
        )
    }

    #[tokio::test]
    async fn test_handle_get_configured_path_returns_prometheus_text() {
        let h = handler_with_data();
        let resp = h
            .handle(inbound(HttpRequest::get("/metrics")))
            .await
            .unwrap();
        assert_eq!(resp.status, 200);
        let ct = resp.header("content-type").map(str::to_owned);
        let body = String::from_utf8(resp.body).unwrap();
        assert!(
            body.contains("edge_requests_total"),
            "missing counter in: {body}"
        );
        assert!(
            body.contains("edge_requests_active"),
            "missing gauge in: {body}"
        );
        assert_eq!(
            ct.as_deref(),
            Some("text/plain; version=0.0.4; charset=utf-8")
        );
    }

    #[tokio::test]
    async fn test_handle_get_wrong_path_returns_not_found() {
        let h = handler_with_data();
        let err = h
            .handle(inbound(HttpRequest::get("/healthz")))
            .await
            .unwrap_err();
        assert!(matches!(err, HttpIngressError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_handle_get_path_with_trailing_slash_returns_200() {
        let h = handler_with_data();
        let resp = h
            .handle(inbound(HttpRequest::get("/metrics/")))
            .await
            .unwrap();
        assert_eq!(resp.status, 200);
    }

    #[tokio::test]
    async fn test_handle_non_get_returns_invalid_input_error() {
        let h = handler_with_data();
        let err = h
            .handle(inbound(HttpRequest::post("/metrics")))
            .await
            .unwrap_err();
        assert!(matches!(err, HttpIngressError::InvalidInput(_)));
    }

    #[test]
    fn test_render_prometheus_formats_type_line_and_value() {
        let provider = create_local_metrics_backend();
        provider.record_counter("my_counter", 5.0, &[]);
        let out = MetricsHandler::render_prometheus(&provider);
        assert!(out.contains("# TYPE my_counter counter"));
        assert!(out.contains("my_counter 5"));
    }

    #[tokio::test]
    async fn test_health_check_returns_healthy() {
        let h = handler_with_data();
        let hc = h
            .health_check(HealthCheckRequest::new(std::net::SocketAddr::from((
                [127, 0, 0, 1],
                0,
            ))))
            .await
            .unwrap();
        assert!(hc.health.healthy);
    }

    #[test]
    fn test_new_sets_path() {
        let provider = Arc::new(create_local_metrics_backend());
        let counters = Arc::new(TrafficCounters::new(provider));
        let h = MetricsHandler::new(counters, "/custom");
        assert_eq!(h.path, "/custom");
    }

    #[test]
    fn test_metrics_exporter_path_matches_configured_path() {
        use swe_edge_bootstrap_metrics::MetricsExporter;
        let h = handler_with_data();
        assert_eq!(h.path(), "/metrics");
    }

    #[test]
    fn test_metrics_exporter_counters_is_same_instance() {
        use swe_edge_bootstrap_metrics::MetricsExporter;
        let provider = Arc::new(create_local_metrics_backend());
        let counters = Arc::new(TrafficCounters::new(provider));
        let h = MetricsHandler::new(Arc::clone(&counters), "/metrics");
        assert!(std::sync::Arc::ptr_eq(h.counters(), &counters));
    }
}
