//! `HttpLoadMonitor` — HTTP inbound load-monitoring wrapper interface.

use swe_edge_ingress_http::HttpIngress;

/// Marker supertrait for HTTP inbound handlers that record load metrics.
pub trait HttpLoadMonitor: HttpIngress {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use swe_edge_ingress_http::{
        HealthCheckRequest, HealthCheckResponse, HttpFuture, HttpHealthCheck, HttpIngressError,
        HttpResponse, InboundRequest,
    };

    struct HttpLoadMonitorDouble;
    impl HttpIngress for HttpLoadMonitorDouble {
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
    impl HttpLoadMonitor for HttpLoadMonitorDouble {}

    #[test]
    fn test_http_load_monitor_double_is_object_safe_as_dyn() {
        let _: Arc<dyn HttpLoadMonitor> = Arc::new(HttpLoadMonitorDouble);
    }
}
