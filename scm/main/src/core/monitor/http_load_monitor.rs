use std::sync::Arc;
use std::time::Instant;

use edge_domain::RequestContext;
use futures::future::BoxFuture;
use swe_edge_ingress_http::{
    HttpHealthCheck, HttpIngress, HttpIngressResult, HttpRequest, HttpResponse,
};

use crate::api::monitor::SharedCounters;

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

impl crate::api::monitor::HttpLoadMonitor for HttpLoadMonitor {}

impl HttpIngress for HttpLoadMonitor {
    fn handle(
        &self,
        request: HttpRequest,
        ctx: RequestContext,
    ) -> BoxFuture<'_, HttpIngressResult<HttpResponse>> {
        self.counters.on_start();
        let counters = Arc::clone(&self.counters);
        let fut = self.inner.handle(request, ctx);
        Box::pin(async move {
            let start = Instant::now();
            let result = fut.await;
            counters.on_end(start.elapsed().as_micros() as u64, result.is_err());
            result
        })
    }

    fn health_check(&self) -> BoxFuture<'_, HttpIngressResult<HttpHealthCheck>> {
        self.inner.health_check()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::monitor::TrafficCounters;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;
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
                _: HttpRequest,
                _: RequestContext,
            ) -> BoxFuture<'_, HttpIngressResult<HttpResponse>> {
                Box::pin(async { Ok(HttpResponse::new(200, vec![])) })
            }
            fn health_check(&self) -> BoxFuture<'_, HttpIngressResult<HttpHealthCheck>> {
                Box::pin(async { Ok(HttpHealthCheck::healthy()) })
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
                _: HttpRequest,
                _: RequestContext,
            ) -> BoxFuture<'_, HttpIngressResult<HttpResponse>> {
                Box::pin(async { Ok(HttpResponse::new(200, vec![])) })
            }
            fn health_check(&self) -> BoxFuture<'_, HttpIngressResult<HttpHealthCheck>> {
                Box::pin(async { Ok(HttpHealthCheck::healthy()) })
            }
        }
        let c = counters();
        let m = HttpLoadMonitor::new(Arc::new(HttpLoadMonitorOk), Arc::clone(&c));
        m.handle(HttpRequest::get("/"), RequestContext::unauthenticated())
            .await
            .unwrap();
        assert_eq!(c.requests_in_flight.load(Ordering::Relaxed), 0);
        let snaps = c.provider.export();
        assert!(snaps
            .iter()
            .any(|s| s.name == "edge_requests_total" && s.value == 1.0));
    }
}
