//! Integration tests for the swe_edge_runtime SAF runtime_manager surface.
#![allow(clippy::unwrap_used, clippy::expect_used)]
// @allow: no_mocks_in_integration — stub impls required to exercise the full daemon stack

use std::sync::Arc;

use futures::future::BoxFuture;
use futures::FutureExt;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use edge_proxy::{
    ComponentRequest, ComponentResponse, HealthRequest, HealthResponse, HealthStatus,
    LifecycleError, LifecycleMonitor, ShutdownRequest, StartBackgroundTasksRequest, StatusRequest,
    StatusResponse,
};
use swe_edge_bootstrap::{Runtime, RuntimeConfig, RuntimeManager, RuntimeStatus};
use swe_edge_egress_http::{
    ConfigRequest, ConfigResponse, HealthCheckRequest as EgressHealthCheckRequest, HttpByteStream,
    HttpConfig, HttpEgress, HttpEgressError, HttpRequest as EgressReq, HttpResponse as EgressResp,
    HttpStreamResponse,
};
use swe_edge_ingress_http::{
    HealthCheckRequest, HealthCheckResponse, HttpFuture, HttpHealthCheck, HttpIngress,
    HttpIngressError, HttpResponse, InboundRequest,
};
use swe_edge_runtime_http::{AxumHttpServer, HttpServer, ServeWithListenerRequest};

struct StubLifecycle;
impl LifecycleMonitor for StubLifecycle {
    fn health(&self, _req: HealthRequest) -> BoxFuture<'_, Result<HealthResponse, LifecycleError>> {
        async move { Ok(HealthResponse::from_components(vec![])) }.boxed()
    }
    fn start_background_tasks(
        &self,
        _req: StartBackgroundTasksRequest,
    ) -> BoxFuture<'_, Result<(), LifecycleError>> {
        async move { Ok(()) }.boxed()
    }
    fn shutdown(&self, _req: ShutdownRequest) -> BoxFuture<'_, Result<(), LifecycleError>> {
        async move { Ok(()) }.boxed()
    }
    fn status(&self, _req: StatusRequest) -> BoxFuture<'_, Result<StatusResponse, LifecycleError>> {
        async move {
            Ok(StatusResponse {
                status: HealthStatus::Healthy,
            })
        }
        .boxed()
    }
    fn component<'a>(
        &'a self,
        _req: ComponentRequest<'_>,
    ) -> BoxFuture<'a, Result<ComponentResponse, LifecycleError>> {
        async move { Ok(ComponentResponse { health: None }) }.boxed()
    }
}

struct StubHttpEgress;
#[async_trait::async_trait]
impl HttpEgress for StubHttpEgress {
    async fn send(&self, _: EgressReq) -> Result<EgressResp, HttpEgressError> {
        Ok(EgressResp::new(200, vec![]))
    }
    async fn send_stream(&self, _: EgressReq) -> Result<HttpStreamResponse, HttpEgressError> {
        Ok(HttpStreamResponse {
            status: 200,
            headers: Default::default(),
            body: HttpByteStream::new(futures::stream::empty()),
        })
    }
    async fn health_check(&self, _: EgressHealthCheckRequest) -> Result<(), HttpEgressError> {
        Ok(())
    }
    fn config(&self, _: ConfigRequest) -> Result<ConfigResponse, HttpEgressError> {
        Ok(ConfigResponse {
            config: HttpConfig::default(),
        })
    }
}

struct EchoHandler;
impl HttpIngress for EchoHandler {
    fn handle(
        &self,
        req: InboundRequest,
    ) -> HttpFuture<'_, Result<HttpResponse, HttpIngressError>> {
        HttpFuture::new(async move {
            let req = req.request;
            Ok(HttpResponse::new(
                200,
                format!("{} {}", req.method, req.url).into_bytes(),
            ))
        })
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

struct NotFoundHandler;
impl HttpIngress for NotFoundHandler {
    fn handle(&self, _: InboundRequest) -> HttpFuture<'_, Result<HttpResponse, HttpIngressError>> {
        HttpFuture::new(async { Err(HttpIngressError::NotFound("resource gone".into())) })
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

async fn start_daemon_stack(
    handler: Arc<dyn HttpIngress>,
) -> (String, impl RuntimeManager, oneshot::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{addr}");
    let config = RuntimeConfig::default().with_systemd_notify(false);
    let ingress = Arc::new(Runtime::http_ingress(handler.clone()));
    let egress = Arc::new(Runtime::http_egress(Arc::new(StubHttpEgress)));
    let mgr = Runtime::runtime_manager(config, ingress, egress, Arc::new(StubLifecycle));
    mgr.start().await.expect("RuntimeManager::start failed");
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let server = AxumHttpServer::new(addr.to_string(), handler);
    tokio::spawn(async move {
        let signal = async move {
            let _ = shutdown_rx.await;
        };
        let _ = server
            .serve_with_listener(ServeWithListenerRequest {
                listener,
                shutdown: Box::pin(signal),
            })
            .await;
    });
    (base_url, mgr, shutdown_tx)
}

/// @covers: runtime_manager — start transitions to Running, health reports components
#[tokio::test]
async fn test_runtime_manager_start_and_shutdown_round_trip() {
    let handler: Arc<dyn HttpIngress> = Arc::new(EchoHandler);
    let config = RuntimeConfig::default().with_systemd_notify(false);
    let ingress = Arc::new(Runtime::http_ingress(handler));
    let egress = Arc::new(Runtime::http_egress(Arc::new(StubHttpEgress)));
    let mgr = Runtime::runtime_manager(config, ingress, egress, Arc::new(StubLifecycle));
    mgr.start().await.expect("start ok");
    assert_eq!(mgr.health().await.status, RuntimeStatus::Running);
    mgr.shutdown().await.expect("shutdown ok");
    assert_eq!(mgr.health().await.status, RuntimeStatus::Stopped);
}

/// @covers: runtime_manager — health reports ingress and egress component names
#[tokio::test]
async fn test_runtime_manager_health_reports_ingress_and_egress() {
    let handler: Arc<dyn HttpIngress> = Arc::new(EchoHandler);
    let config = RuntimeConfig::default().with_systemd_notify(false);
    let ingress = Arc::new(Runtime::http_ingress(handler));
    let egress = Arc::new(Runtime::http_egress(Arc::new(StubHttpEgress)));
    let mgr = Runtime::runtime_manager(config, ingress, egress, Arc::new(StubLifecycle));
    mgr.start().await.expect("start ok");
    let health = mgr.health().await;
    let names: Vec<&str> = health.components.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"ingress.http"));
    assert!(names.contains(&"egress.http"));
}

#[tokio::test]
async fn test_http_get_request_flows_end_to_end_through_daemon_stack() {
    let (base, mgr, shutdown_tx) = start_daemon_stack(Arc::new(EchoHandler)).await;
    let resp = reqwest::get(format!("{base}/ping"))
        .await
        .expect("HTTP request failed");
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("GET"),
        "expected GET in echo body, got: {body}"
    );
    let _ = shutdown_tx.send(());
    mgr.shutdown().await.expect("shutdown ok");
}

#[tokio::test]
async fn test_http_post_with_json_body_reaches_handler() {
    let (base, mgr, shutdown_tx) = start_daemon_stack(Arc::new(EchoHandler)).await;
    let resp = reqwest::Client::new()
        .post(format!("{base}/submit"))
        .json(&serde_json::json!({"key": "value"}))
        .send()
        .await
        .expect("HTTP request failed");
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("POST"),
        "expected POST in echo body, got: {body}"
    );
    let _ = shutdown_tx.send(());
    mgr.shutdown().await.expect("shutdown ok");
}

#[tokio::test]
async fn test_handler_not_found_error_surfaces_as_404_at_wire_level() {
    let (base, mgr, shutdown_tx) = start_daemon_stack(Arc::new(NotFoundHandler)).await;
    let resp = reqwest::get(format!("{base}/missing"))
        .await
        .expect("HTTP request failed");
    assert_eq!(resp.status(), 404);
    let _ = shutdown_tx.send(());
    mgr.shutdown().await.expect("shutdown ok");
}

#[tokio::test]
async fn test_server_refuses_connections_after_daemon_shutdown() {
    let (base, mgr, shutdown_tx) = start_daemon_stack(Arc::new(EchoHandler)).await;
    reqwest::get(format!("{base}/check"))
        .await
        .expect("should be up");
    let _ = shutdown_tx.send(());
    mgr.shutdown().await.expect("shutdown ok");
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let result = reqwest::get(format!("{base}/check")).await;
    assert!(
        result.is_err(),
        "expected connection refused after shutdown"
    );
}
