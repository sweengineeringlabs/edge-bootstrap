//! Integration tests for `ServerMonitor::health_handler`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use futures::future::BoxFuture;
use swe_edge_bootstrap::{
    ComponentHealth, HealthHandler, HttpIngress, HttpIngressError, HttpMethod, HttpRequest,
    RequestContext, RuntimeHealth, RuntimeManager, RuntimeResult, RuntimeStatus, ServerMonitor,
};

// ── Test doubles ──────────────────────────────────────────────────────────────

struct HealthyRuntime;
impl RuntimeManager for HealthyRuntime {
    fn start(&self) -> BoxFuture<'_, RuntimeResult<()>> {
        Box::pin(async { Ok(()) })
    }
    fn shutdown(&self) -> BoxFuture<'_, RuntimeResult<()>> {
        Box::pin(async { Ok(()) })
    }
    fn health(&self) -> BoxFuture<'_, RuntimeHealth> {
        Box::pin(async {
            RuntimeHealth {
                status: RuntimeStatus::Running,
                components: vec![ComponentHealth::healthy("http")],
                uptime_secs: 99,
            }
        })
    }
}

struct DegradedRuntime;
impl RuntimeManager for DegradedRuntime {
    fn start(&self) -> BoxFuture<'_, RuntimeResult<()>> {
        Box::pin(async { Ok(()) })
    }
    fn shutdown(&self) -> BoxFuture<'_, RuntimeResult<()>> {
        Box::pin(async { Ok(()) })
    }
    fn health(&self) -> BoxFuture<'_, RuntimeHealth> {
        Box::pin(async {
            RuntimeHealth {
                status: RuntimeStatus::Degraded,
                components: vec![ComponentHealth::unhealthy("db", "connection refused")],
                uptime_secs: 5,
            }
        })
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// @covers: ServerMonitor::health_handler
#[tokio::test]
async fn test_health_handler_get_returns_200_when_runtime_healthy() {
    let h = ServerMonitor::health_handler(Arc::new(HealthyRuntime));
    let resp = h
        .handle(
            HttpRequest::get("/health"),
            RequestContext::unauthenticated(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status, 200);
}

/// @covers: ServerMonitor::health_handler
#[tokio::test]
async fn test_health_handler_get_returns_503_when_runtime_degraded() {
    let h = ServerMonitor::health_handler(Arc::new(DegradedRuntime));
    let resp = h
        .handle(
            HttpRequest::get("/health"),
            RequestContext::unauthenticated(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status, 503);
}

/// @covers: ServerMonitor::health_handler — response body is valid JSON
#[tokio::test]
async fn test_health_handler_body_contains_status_and_uptime() {
    let h = ServerMonitor::health_handler(Arc::new(HealthyRuntime));
    let resp = h
        .handle(
            HttpRequest::get("/health"),
            RequestContext::unauthenticated(),
        )
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&resp.body).unwrap();
    assert_eq!(body["status"], "running", "unexpected status field");
    assert_eq!(body["uptime_secs"], 99, "unexpected uptime_secs");
}

/// @covers: ServerMonitor::health_handler — Content-Type is application/json
#[tokio::test]
async fn test_health_handler_content_type_is_json() {
    let h = ServerMonitor::health_handler(Arc::new(HealthyRuntime));
    let resp = h
        .handle(
            HttpRequest::get("/health"),
            RequestContext::unauthenticated(),
        )
        .await
        .unwrap();
    assert_eq!(resp.header("content-type"), Some("application/json"));
}

/// @covers: ServerMonitor::health_handler — non-GET returns error
#[tokio::test]
async fn test_health_handler_non_get_returns_invalid_input_error() {
    let h = ServerMonitor::health_handler(Arc::new(HealthyRuntime));
    let err = h
        .handle(
            HttpRequest::post("/health"),
            RequestContext::unauthenticated(),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, HttpIngressError::InvalidInput(_)),
        "expected InvalidInput, got: {err:?}"
    );
}

/// @covers: ServerMonitor::health_handler — wrong path returns not-found
#[tokio::test]
async fn test_health_handler_wrong_path_returns_not_found_error() {
    let h = ServerMonitor::health_handler(Arc::new(HealthyRuntime));
    let err = h
        .handle(
            HttpRequest::get("/metrics"),
            RequestContext::unauthenticated(),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, HttpIngressError::NotFound(_)),
        "expected NotFound, got: {err:?}"
    );
}

/// @covers: ServerMonitor::health_handler — implements HealthHandler supertrait
#[test]
fn test_health_handler_implements_health_handler_supertrait() {
    fn assert_health_handler<H: HealthHandler>(_: H) {}
    assert_health_handler(ServerMonitor::health_handler(Arc::new(HealthyRuntime)));
}

/// @covers: ServerMonitor::health_handler — implements HttpIngress
#[test]
fn test_health_handler_implements_http_ingress() {
    fn assert_http_ingress<H: HttpIngress>(_: H) {}
    assert_http_ingress(ServerMonitor::health_handler(Arc::new(HealthyRuntime)));
}
