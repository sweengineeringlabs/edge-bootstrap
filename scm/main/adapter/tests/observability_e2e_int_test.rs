//! Integration test — proves observability goes through `RuntimeBuilder` for
//! real, per the `observability` feature epic: a live HTTP request drives
//! `Handler::execute` -> `HandlerContext.observer.drain()` -> the real
//! `LogDrainBridge` -> the real file-backed `LoggerProvider` -> disk, with
//! the durability fix (`LogDrainBridge::emit` flushing its backend after
//! every `emit()`) proven by reading the log file back off disk in this
//! same test process immediately after the response returns — no manual
//! flush anywhere in this test, and no delay beyond the connect wait below.
//!
//! Mirrors `grpc_e2e_int_test.rs`'s pattern (and this crate's own
//! `intrusion_guard_int_test.rs`, the established HTTP sibling): spawn
//! `RuntimeBuilder::serve()` on an ephemeral port via `free_port()`, sleep
//! 200ms to let it bind, hit it with a real client (`reqwest`, the same
//! crate `intrusion_guard_int_test.rs` already uses against a live HTTP
//! server), assert on the real response, `handle.abort()`.
//!
//! Defines its own payload/handler rather than importing
//! `examples/observability-e2e`'s private `ObservedHandler` — integration
//! tests under `tests/` are a separate compilation target from `examples/`
//! and cannot see anything `pub(crate)` there.
#![cfg(feature = "observability")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use edge_application::{Handler, HandlerError};
use edge_application_handler::{
    DrainRequest, ExecutionRequest, IdRequest, IdResponse, LogEmitRequest, PatternRequest,
    PatternResponse,
};
use swe_edge_bootstrap::{
    LogBackendConfig, LogBackendKind, LogFileSettings, MetricsConfig, Runtime, RuntimeConfig,
};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct ObservabilityE2ePayload {
    text: String,
}
impl edge_application::Request for ObservabilityE2ePayload {}
impl edge_application::Response for ObservabilityE2ePayload {}

struct LoggingEchoHandler;

#[async_trait]
impl Handler for LoggingEchoHandler {
    type Request = ObservabilityE2ePayload;
    type Response = ObservabilityE2ePayload;

    fn id(&self, _req: IdRequest) -> Result<IdResponse, HandlerError> {
        Ok(IdResponse {
            id: "/observe".to_string(),
        })
    }

    // HTTP routing (unlike gRPC) keys `DefaultHttpJob`'s `matchit::Router` on
    // `pattern`, not `id` — see `DefaultHttpJob::register_route`. The trait
    // default (empty string) would fail route registration outright.
    fn pattern(&self, _req: PatternRequest) -> Result<PatternResponse, HandlerError> {
        Ok(PatternResponse {
            pattern: "/observe".to_string(),
        })
    }

    async fn execute(
        &self,
        req: ExecutionRequest<'_, ObservabilityE2ePayload>,
    ) -> Result<ObservabilityE2ePayload, HandlerError> {
        // The real seam under test: `HandlerContext.observer` here is the
        // real `ObserverContext` `RuntimeBuilder` wires from
        // `RuntimeConfig.log_backend` — not a noop, not a test double.
        if let Ok(d) = req.ctx.observer.drain(DrainRequest) {
            let _ = d.drain.emit(LogEmitRequest {
                level: "info".to_string(),
                handler_id: "observe".to_string(),
                message: format!("observed payload: {}", req.req.text),
            });
        }
        Ok(req.req)
    }
}

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    listener.local_addr().expect("local_addr").port()
}

#[tokio::test]
async fn test_serve_http_route_persists_log_entry_to_real_file_backend_happy() {
    let addr = format!("127.0.0.1:{}", free_port());
    let metrics_addr = format!("127.0.0.1:{}", free_port());

    let log_dir = tempfile::tempdir().expect("create tempdir for the durable log backend");
    let log_path = log_dir.path().join("observability_e2e_int_test.jsonl");

    let mut config = RuntimeConfig::default()
        .with_http_bind(addr.clone())
        .with_systemd_notify(false);
    config.metrics = Some(MetricsConfig {
        bind: metrics_addr.clone(),
        path: "/metrics".to_string(),
    });
    config.log_backend = Some(LogBackendConfig {
        active: LogBackendKind::File,
        file: Some(LogFileSettings {
            path: log_path.to_string_lossy().into_owned(),
            max_size_mb: 10,
            rotation: Default::default(),
        }),
        ..Default::default()
    });

    let handle = tokio::spawn(async move {
        Runtime::builder()
            .config(config)
            .http_route(Arc::new(LoggingEchoHandler))
            .grpc_allow_unauthenticated()
            .serve()
            .await
    });

    // Give the server a moment to bind before connecting.
    tokio::time::sleep(Duration::from_millis(200)).await;

    let payload = ObservabilityE2ePayload {
        text: "durable-e2e-payload".to_string(),
    };
    let response = reqwest::Client::new()
        .post(format!("http://{addr}/observe"))
        .json(&payload)
        .send()
        .await
        .expect("request must reach the real DefaultHttpJob-routed handler");
    assert_eq!(
        response.status(),
        200,
        "the handler must accept the request"
    );

    let echoed: ObservabilityE2ePayload =
        response.json().await.expect("deserialize echoed payload");
    assert_eq!(
        echoed, payload,
        "the handler must echo back exactly what it received, proving the real \
         HttpIngressJob -> DefaultHttpJob -> Handler::execute chain ran end-to-end"
    );

    // No manual flush here — `LogDrainBridge::emit()` (the real bridge
    // `RuntimeBuilder` wires from `RuntimeConfig.log_backend`) is the only
    // thing that can have reached disk between the request above and this
    // read. Regression coverage for the data-loss bug: without
    // `LogDrainBridge::emit` calling `self.backend.flush()`, this entry
    // would still be sitting in `FileLogger`'s in-memory `BufWriter`.
    let contents = std::fs::read_to_string(&log_path).unwrap_or_else(|e| {
        panic!("expected the log entry already on disk with no manual flush: {e}")
    });
    assert!(
        contents.contains("durable-e2e-payload"),
        "expected the handler's log entry on disk immediately after the HTTP response, got: {contents}"
    );

    let metrics_response = reqwest::get(format!("http://{metrics_addr}/metrics"))
        .await
        .expect("metrics endpoint must be reachable");
    let metrics_body = metrics_response
        .text()
        .await
        .expect("read metrics response body");
    assert!(
        metrics_body.contains("edge_requests_total 1"),
        "expected the real HttpLoadMonitor counter to reflect the one request sent, got: {metrics_body}"
    );

    handle.abort();
}
