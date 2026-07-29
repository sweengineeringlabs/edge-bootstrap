//! `HttpIntrusionGuard` — HTTP inbound IDS/IPS wrapper interface.

use swe_edge_ingress_http::HttpIngress;

/// Marker supertrait for HTTP inbound handlers that reject requests flagged
/// by an intrusion-detection rules engine before delegating to the wrapped
/// handler. Per the fail-open contract the concrete implementation relies
/// on (`edge-intrusion`'s ADR-002), a detector fault must never turn into a
/// rejected request — only an explicit `Decision::Reject` blocks.
pub trait HttpIntrusionGuard: HttpIngress {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use swe_edge_ingress_http::{
        HealthCheckRequest, HealthCheckResponse, HttpFuture, HttpHealthCheck, HttpIngressError,
        HttpResponse, InboundRequest,
    };

    struct HttpIntrusionGuardDouble;
    impl HttpIngress for HttpIntrusionGuardDouble {
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
    impl HttpIntrusionGuard for HttpIntrusionGuardDouble {}

    #[test]
    fn test_http_intrusion_guard_double_is_object_safe_as_dyn() {
        let _: Arc<dyn HttpIntrusionGuard> = Arc::new(HttpIntrusionGuardDouble);
    }
}
