//! `DefaultHealthHandler` — serves `GET /health` as a JSON `RuntimeHealth` snapshot.

use std::sync::Arc;

use futures::future::BoxFuture;
use swe_edge_ingress_http::{
    HealthCheckRequest, HealthCheckResponse, HttpFuture, HttpHealthCheck, HttpIngress,
    HttpIngressError, HttpMethod, HttpRequest, HttpResponse, InboundRequest,
};

use swe_edge_bootstrap_health::HealthHandler;
use swe_edge_bootstrap_runtime::RuntimeManager;

/// HTTP handler that aggregates health from all runtime components.
///
/// Responds to `GET <path>` (default `/health`) with a JSON body derived from
/// [`RuntimeHealth`](crate::RuntimeHealth). Returns HTTP 200 when the runtime
/// is healthy and 503 when any component is degraded.
pub(crate) struct DefaultHealthHandler {
    manager: Arc<dyn RuntimeManager>,
    path: String,
}

impl DefaultHealthHandler {
    pub(crate) fn new(manager: Arc<dyn RuntimeManager>, path: impl Into<String>) -> Self {
        Self {
            manager,
            path: path.into(),
        }
    }
}

impl HealthHandler for DefaultHealthHandler {
    fn health(&self) -> BoxFuture<'_, swe_edge_bootstrap_runtime::RuntimeHealth> {
        self.manager.health()
    }
}

impl HttpIngress for DefaultHealthHandler {
    fn handle(
        &self,
        req: InboundRequest,
    ) -> HttpFuture<'_, Result<HttpResponse, HttpIngressError>> {
        let manager = Arc::clone(&self.manager);
        let path = self.path.clone();
        let request = req.request;
        HttpFuture::new(async move {
            if request.method != HttpMethod::Get {
                return Err(HttpIngressError::InvalidInput(
                    "health endpoint only accepts GET".into(),
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
            let health = manager.health().await;
            let status_code: u16 = if health.is_healthy() { 200 } else { 503 };
            let body = serde_json::to_vec(&health).map_err(|e| {
                HttpIngressError::InvalidInput(format!("health serialization failed: {e}"))
            })?;
            let mut resp = HttpResponse::new(status_code, body);
            resp.headers
                .insert("content-type".into(), "application/json".into());
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
    use swe_edge_bootstrap_runtime::{ComponentHealth, RuntimeHealth, RuntimeStatus};

    struct AlwaysHealthyManager;
    impl RuntimeManager for AlwaysHealthyManager {
        fn start(&self) -> BoxFuture<'_, swe_edge_bootstrap_runtime::RuntimeResult<()>> {
            Box::pin(async { Ok(()) })
        }
        fn shutdown(&self) -> BoxFuture<'_, swe_edge_bootstrap_runtime::RuntimeResult<()>> {
            Box::pin(async { Ok(()) })
        }
        fn health(&self) -> BoxFuture<'_, RuntimeHealth> {
            Box::pin(async {
                RuntimeHealth {
                    status: RuntimeStatus::Running,
                    components: vec![ComponentHealth::healthy("http")],
                    uptime_secs: 42,
                }
            })
        }
    }

    struct DegradedManager;
    impl RuntimeManager for DegradedManager {
        fn start(&self) -> BoxFuture<'_, swe_edge_bootstrap_runtime::RuntimeResult<()>> {
            Box::pin(async { Ok(()) })
        }
        fn shutdown(&self) -> BoxFuture<'_, swe_edge_bootstrap_runtime::RuntimeResult<()>> {
            Box::pin(async { Ok(()) })
        }
        fn health(&self) -> BoxFuture<'_, RuntimeHealth> {
            Box::pin(async {
                RuntimeHealth {
                    status: RuntimeStatus::Degraded,
                    components: vec![ComponentHealth::unhealthy("db", "connection refused")],
                    uptime_secs: 10,
                }
            })
        }
    }

    fn healthy_handler() -> DefaultHealthHandler {
        DefaultHealthHandler::new(Arc::new(AlwaysHealthyManager), "/health")
    }

    fn degraded_handler() -> DefaultHealthHandler {
        DefaultHealthHandler::new(Arc::new(DegradedManager), "/health")
    }

    fn inbound(request: HttpRequest) -> InboundRequest {
        InboundRequest::new(
            request,
            swe_edge_ingress_http::RequestContext::new(
                edge_application::SecurityContext::unauthenticated(),
            ),
            std::net::SocketAddr::from(([127, 0, 0, 1], 0)),
        )
    }

    #[tokio::test]
    async fn test_handle_get_health_returns_200_when_healthy() {
        let h = healthy_handler();
        let resp = h
            .handle(inbound(HttpRequest::get("/health")))
            .await
            .unwrap();
        assert_eq!(resp.status, 200);
    }

    #[tokio::test]
    async fn test_handle_get_health_returns_503_when_degraded() {
        let h = degraded_handler();
        let resp = h
            .handle(inbound(HttpRequest::get("/health")))
            .await
            .unwrap();
        assert_eq!(resp.status, 503);
    }

    #[tokio::test]
    async fn test_handle_get_health_body_is_json_with_status_field() {
        let h = healthy_handler();
        let resp = h
            .handle(inbound(HttpRequest::get("/health")))
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&resp.body).unwrap();
        assert_eq!(body["status"], "running");
        assert_eq!(body["uptime_secs"], 42);
    }

    #[tokio::test]
    async fn test_handle_get_health_content_type_is_json() {
        let h = healthy_handler();
        let resp = h
            .handle(inbound(HttpRequest::get("/health")))
            .await
            .unwrap();
        assert_eq!(resp.header("content-type"), Some("application/json"));
    }

    #[tokio::test]
    async fn test_handle_non_get_returns_invalid_input_error() {
        let h = healthy_handler();
        let err = h
            .handle(inbound(HttpRequest::post("/health")))
            .await
            .unwrap_err();
        assert!(matches!(err, HttpIngressError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn test_handle_wrong_path_returns_not_found() {
        let h = healthy_handler();
        let err = h
            .handle(inbound(HttpRequest::get("/metrics")))
            .await
            .unwrap_err();
        assert!(matches!(err, HttpIngressError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_handle_trailing_slash_matches_configured_path() {
        let h = healthy_handler();
        let resp = h
            .handle(inbound(HttpRequest::get("/health/")))
            .await
            .unwrap();
        assert_eq!(resp.status, 200);
    }

    #[tokio::test]
    async fn test_health_check_returns_healthy() {
        let h = healthy_handler();
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
    fn test_new_stores_custom_path() {
        let h = DefaultHealthHandler::new(Arc::new(AlwaysHealthyManager), "/livez");
        assert_eq!(h.path, "/livez");
    }

    #[tokio::test]
    async fn test_health_delegates_to_manager() {
        let h = degraded_handler();
        let health = h.health().await;
        assert_eq!(health.status, RuntimeStatus::Degraded);
        assert_eq!(health.uptime_secs, 10);
    }
}
