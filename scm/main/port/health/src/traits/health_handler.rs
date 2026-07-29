//! `HealthHandler` — HTTP health endpoint port contract.

use futures::future::BoxFuture;
use swe_edge_bootstrap_runtime::RuntimeHealth;
use swe_edge_ingress_http::HttpIngress;

/// Supertrait for runtime health HTTP endpoint handlers.
///
/// Implementations respond to `GET /health` with a JSON `RuntimeHealth`
/// body and HTTP 200 when healthy, 503 when degraded.
pub trait HealthHandler: HttpIngress {
    /// Aggregate health this handler reports over `GET /health`.
    fn health(&self) -> BoxFuture<'_, RuntimeHealth>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use swe_edge_bootstrap_runtime::RuntimeStatus;
    use swe_edge_ingress_http::{
        HealthCheckRequest, HealthCheckResponse, HttpFuture, HttpHealthCheck, HttpIngressError,
        HttpResponse, InboundRequest,
    };

    struct HealthHandlerDouble;

    impl HttpIngress for HealthHandlerDouble {
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

    impl HealthHandler for HealthHandlerDouble {
        fn health(&self) -> BoxFuture<'_, RuntimeHealth> {
            Box::pin(async {
                RuntimeHealth {
                    status: RuntimeStatus::Degraded,
                    components: vec![],
                    uptime_secs: 5,
                }
            })
        }
    }

    #[tokio::test]
    async fn test_health_handler_double_health_returns_configured_status() {
        let d = HealthHandlerDouble;
        let health = d.health().await;
        assert_eq!(health.status, RuntimeStatus::Degraded);
        assert_eq!(health.uptime_secs, 5);
    }
}
