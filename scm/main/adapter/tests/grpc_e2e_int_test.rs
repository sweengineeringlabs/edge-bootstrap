//! Integration test — proves gRPC goes through `RuntimeBuilder` per ADR-021:
//! `GrpcIngressJob` (real `edge_proxy::Job`/`Router`) → `DefaultGrpcJob`'s
//! `HandlerRegistry`/`Pipeline` → `Handler::execute`. Not a unit test of
//! `DefaultGrpcJob`/`GrpcIngressJob` in isolation (those live alongside the
//! adapter in `main/adapter/src/core/dispatch/`) — this proves the wiring
//! itself composes correctly through a real socket and a real client, which a
//! unit test of the job alone cannot.
//!
//! No client-side reflection assertion here: `swe-edge-egress-grpc` has no
//! `ListServices`/reflection client capability (confirmed by inspecting its
//! source) — reflection's server-side wiring (`ReflectionService::with_route_lister`)
//! is proven separately via a manual `grpcurl` call against the live
//! `bootstrap-e2e` example.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use edge_application::{Handler, HandlerError};
use edge_application_handler::{ExecutionRequest, IdRequest, IdResponse};
use swe_edge_bootstrap::{Runtime, RuntimeConfig};
use swe_edge_egress_grpc::{
    GrpcChannelConfig, GrpcRequest, HealthCheckRequest as GrpcEgressHealthCheckRequest,
    TransportConstruction,
};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct GrpcE2ePayload {
    text: String,
}
impl edge_application::Request for GrpcE2ePayload {}
impl edge_application::Response for GrpcE2ePayload {}

struct EchoHandler;

#[async_trait]
impl Handler for EchoHandler {
    type Request = GrpcE2ePayload;
    type Response = GrpcE2ePayload;

    fn id(&self, _req: IdRequest) -> Result<IdResponse, HandlerError> {
        // Must carry the leading `/` — the server keys routes on
        // `req.uri().path()` (see edge-runtime-grpc-adapter's
        // `tonic_server_dispatcher.rs`), which always includes it.
        Ok(IdResponse {
            id: "/echo".to_string(),
        })
    }

    async fn execute(
        &self,
        req: ExecutionRequest<'_, GrpcE2ePayload>,
    ) -> Result<GrpcE2ePayload, HandlerError> {
        Ok(req.req)
    }
}

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    listener.local_addr().expect("local_addr").port()
}

#[tokio::test]
async fn test_serve_grpc_route_dispatches_real_unary_call_happy() {
    let addr = format!("127.0.0.1:{}", free_port());

    let config = RuntimeConfig::default().with_grpc_bind(addr.clone());

    let handle = tokio::spawn(async move {
        Runtime::builder()
            .config(config)
            .grpc_route(Arc::new(EchoHandler))
            .grpc_allow_unauthenticated()
            .serve()
            .await
    });

    // Give the server a moment to bind before connecting.
    tokio::time::sleep(Duration::from_millis(200)).await;

    let client = TransportConstruction::create_transport_from_config(
        &GrpcChannelConfig::new(format!("http://{addr}")).allow_plaintext(),
    )
    .expect("create_transport_from_config must succeed against a plaintext endpoint");

    let payload = GrpcE2ePayload {
        text: "hello-grpc".to_string(),
    };
    let body = serde_json::to_vec(&payload).expect("serialize payload");
    let request = GrpcRequest::new("echo", body, Duration::from_secs(5));

    let response = client
        .call_unary(request)
        .await
        .expect("call_unary must reach the real DefaultGrpcJob-routed handler");
    let echoed: GrpcE2ePayload =
        serde_json::from_slice(&response.body).expect("deserialize echoed payload");
    assert_eq!(
        echoed, payload,
        "the handler must echo back exactly what it received, proving the real \
         GrpcIngressJob -> DefaultGrpcJob -> Handler::execute chain ran end-to-end"
    );

    handle.abort();
}

#[tokio::test]
async fn test_serve_grpc_route_health_check_reports_healthy_happy() {
    let addr = format!("127.0.0.1:{}", free_port());

    let config = RuntimeConfig::default().with_grpc_bind(addr.clone());

    let handle = tokio::spawn(async move {
        Runtime::builder()
            .config(config)
            .grpc_route(Arc::new(EchoHandler))
            .grpc_allow_unauthenticated()
            .serve()
            .await
    });

    tokio::time::sleep(Duration::from_millis(200)).await;

    let client = TransportConstruction::create_transport_from_config(
        &GrpcChannelConfig::new(format!("http://{addr}")).allow_plaintext(),
    )
    .expect("create_transport_from_config must succeed against a plaintext endpoint");

    client
        .health_check(GrpcEgressHealthCheckRequest)
        .await
        .expect(
            "health_check must reach the real DefaultGrpcJob and report healthy \
             (proving GrpcIngressJob::health_check, not a stub, answered)",
        );

    handle.abort();
}
