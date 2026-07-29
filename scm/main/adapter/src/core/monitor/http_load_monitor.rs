use std::sync::Arc;
use std::time::Instant;

use swe_edge_ingress_http::{
    HealthCheckRequest, HealthCheckResponse, HttpFuture, HttpIngress, HttpIngressError,
    HttpResponse, InboundRequest,
};

use swe_edge_bootstrap_monitor::SharedCounters;

/// Wraps an `HttpIngress` handler; records load metrics on every request.
pub(crate) struct HttpLoadMonitor {
    inner: Arc<dyn HttpIngress>,
    counters: SharedCounters,
}

impl HttpLoadMonitor {
    pub(crate) fn new(inner: Arc<dyn HttpIngress>, counters: SharedCounters) -> Self {
        Self { inner, counters }
    }
}

impl swe_edge_bootstrap_monitor::HttpLoadMonitor for HttpLoadMonitor {}

impl HttpIngress for HttpLoadMonitor {
    fn handle(
        &self,
        req: InboundRequest,
    ) -> HttpFuture<'_, Result<HttpResponse, HttpIngressError>> {
        self.counters.on_start();
        let counters = Arc::clone(&self.counters);
        let fut = self.inner.handle(req);
        HttpFuture::new(async move {
            let start = Instant::now();
            let result = fut.await;
            counters.on_end(start.elapsed().as_micros() as u64, result.is_err());
            result
        })
    }

    fn health_check(
        &self,
        req: HealthCheckRequest,
    ) -> HttpFuture<'_, Result<HealthCheckResponse, HttpIngressError>> {
        self.inner.health_check(req)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swe_edge_bootstrap_monitor::TrafficCounters;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;
    use swe_edge_ingress_http::{HttpHealthCheck, HttpRequest, RequestContext};
    use swe_observ_metrics::create_local_metrics_backend;

    fn counters() -> SharedCounters {
        Arc::new(TrafficCounters::new(Arc::new(
            create_local_metrics_backend(),
        )))
    }

    #[test]
    fn test_http_load_monitor_new_does_not_panic() {
        struct HttpLoadMonitorStub;
        impl HttpIngress for HttpLoadMonitorStub {
            fn handle(
                &self,
                _req: InboundRequest,
            ) -> HttpFuture<'_, Result<HttpResponse, HttpIngressError>> {
                HttpFuture::new(async { Ok(HttpResponse::new(200, vec![])) })
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
        let _m = HttpLoadMonitor::new(Arc::new(HttpLoadMonitorStub), counters());
    }

    #[tokio::test]
    async fn test_http_monitor_handle_records_request_via_provider() {
        struct HttpLoadMonitorOk;
        impl HttpIngress for HttpLoadMonitorOk {
            fn handle(
                &self,
                _req: InboundRequest,
            ) -> HttpFuture<'_, Result<HttpResponse, HttpIngressError>> {
                HttpFuture::new(async { Ok(HttpResponse::new(200, vec![])) })
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
        let c = counters();
        let m = HttpLoadMonitor::new(Arc::new(HttpLoadMonitorOk), Arc::clone(&c));
        m.handle(InboundRequest::new(
            HttpRequest::get("/"),
            RequestContext::new(edge_domain::SecurityContext::unauthenticated()),
            std::net::SocketAddr::from(([127, 0, 0, 1], 0)),
        ))
        .await
        .unwrap();
        assert_eq!(c.requests_in_flight.load(Ordering::Relaxed), 0);
        let snaps = c.provider.export();
        assert!(snaps
            .iter()
            .any(|s| s.name == "edge_requests_total" && s.value == 1.0));
    }
}
