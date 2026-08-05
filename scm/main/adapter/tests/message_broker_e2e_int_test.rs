//! Integration test — `message-broker-nats` feature: proves a `Handler`
//! wired with a real NATS-backed `MessageBroker` publishes over a real
//! `nats-server` connection when served through `RuntimeBuilder::serve()`,
//! and that an independent subscriber genuinely receives it end to end.
//!
//! Mirrors `grpc_e2e_int_test.rs`'s pattern — spawn `serve()` in a
//! `tokio::spawn`, settle briefly, drive it with a real client, assert on
//! the real response, `handle.abort()` — but adds a second real connection
//! (the `nats-server` process itself) that must also compose correctly. Not
//! a unit test of `OrderHandler`/`MessageBrokerFactory::nats` in isolation
//! (those live in `examples/messaging-e2e/`) — this proves the whole chain
//! (`RuntimeBuilder::serve()` -> `Handler::execute` -> real NATS publish ->
//! real NATS subscriber) composes through real sockets, not mocks.
#![cfg(feature = "message-broker-nats")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use edge_application::{Handler, HandlerError};
use edge_application_handler::{
    ExecutionRequest, IdRequest, IdResponse, PatternRequest, PatternResponse,
};
use futures::StreamExt;
use swe_edge_bootstrap::{Message, MessageBroker, MessageBrokerFactory, Runtime, RuntimeConfig};

/// A dedicated port, distinct from the default `4222`, so this test never
/// collides with a manually-run `nats-server` instance on a developer's
/// machine.
const NATS_PORT: u16 = 24222;
const TOPIC: &str = "orders.created";

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct OrderPayload {
    id: String,
}
impl edge_application::Request for OrderPayload {}
impl edge_application::Response for OrderPayload {}

struct OrderHandler {
    broker: Arc<dyn MessageBroker>,
}

#[async_trait]
impl Handler for OrderHandler {
    type Request = OrderPayload;
    type Response = OrderPayload;

    fn id(&self, _req: IdRequest) -> Result<IdResponse, HandlerError> {
        Ok(IdResponse {
            id: "/orders".to_string(),
        })
    }

    fn pattern(&self, _req: PatternRequest) -> Result<PatternResponse, HandlerError> {
        Ok(PatternResponse {
            pattern: "/orders".to_string(),
        })
    }

    async fn execute(
        &self,
        req: ExecutionRequest<'_, OrderPayload>,
    ) -> Result<OrderPayload, HandlerError> {
        let order = req.req;
        let payload =
            serde_json::to_vec(&order).map_err(|e| HandlerError::ExecutionFailed(e.to_string()))?;
        self.broker
            .publish(TOPIC, Message::new(payload))
            .await
            .map_err(|e| HandlerError::ExecutionFailed(format!("publish failed: {e}")))?;
        Ok(order)
    }
}

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    listener.local_addr().expect("local_addr").port()
}

fn nats_reachable(port: u16) -> bool {
    std::net::TcpStream::connect_timeout(
        &format!("127.0.0.1:{port}")
            .parse()
            .expect("valid socket addr"),
        Duration::from_millis(200),
    )
    .is_ok()
}

/// Kills the `nats-server` child this test spawned, if any, when dropped —
/// so a panicking assertion still cleans it up rather than leaking a
/// process. Holds `None` when an already-running server was reused instead,
/// in which case this test must not kill it.
struct NatsServerGuard(Option<Child>);
impl Drop for NatsServerGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Starts a real `nats-server` on [`NATS_PORT`] if nothing is listening
/// there yet, and blocks until it is actually accepting connections.
/// Reuses an already-running instance on that port instead of spawning a
/// second one.
async fn ensure_nats_server() -> NatsServerGuard {
    if nats_reachable(NATS_PORT) {
        return NatsServerGuard(None);
    }

    let child = Command::new("nats-server")
        .args(["-p", &NATS_PORT.to_string(), "-a", "127.0.0.1"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect(
            "failed to spawn `nats-server` — install it and ensure it's on PATH; \
             this test requires a real NATS server, not a mock",
        );

    for _ in 0..50 {
        if nats_reachable(NATS_PORT) {
            return NatsServerGuard(Some(child));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("nats-server did not start accepting connections on port {NATS_PORT} in time");
}

#[tokio::test]
async fn test_serve_with_nats_broker_publishes_and_delivers_real_message_happy() {
    let _nats_guard = ensure_nats_server().await;
    let nats_url = format!("127.0.0.1:{NATS_PORT}");

    let handler_broker: Arc<dyn MessageBroker> = Arc::new(
        MessageBrokerFactory::nats(&nats_url)
            .await
            .expect("must connect to the real nats-server this test just ensured is running"),
    );

    // A second, independent NATS connection — subscribing here (not on the
    // handler's own broker `Arc`) proves delivery genuinely round-trips
    // through the `nats-server` process, not merely through a shared
    // in-process handle.
    let subscriber_broker = MessageBrokerFactory::nats(&nats_url)
        .await
        .expect("must connect to the real nats-server for the independent subscriber");
    let mut stream = subscriber_broker
        .subscribe(TOPIC)
        .await
        .expect("subscribe must succeed against the real nats-server");

    let addr = format!("127.0.0.1:{}", free_port());
    let config = RuntimeConfig::default().with_http_bind(addr.clone());
    let handler = Arc::new(OrderHandler {
        broker: handler_broker,
    });

    let handle = tokio::spawn(async move {
        Runtime::builder()
            .config(config)
            .http_route(handler)
            .serve()
            .await
    });

    // Give the server a moment to bind before connecting.
    tokio::time::sleep(Duration::from_millis(200)).await;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://{addr}/orders"))
        .json(&serde_json::json!({"id": "order-e2e-1"}))
        .send()
        .await
        .expect("HTTP request must reach the real served handler");
    assert_eq!(response.status(), 200);
    let echoed: OrderPayload = response
        .json()
        .await
        .expect("response body must be valid JSON matching OrderPayload");
    assert_eq!(
        echoed,
        OrderPayload {
            id: "order-e2e-1".to_string()
        },
        "the served handler must echo back exactly what it received"
    );

    let received = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .expect(
            "timed out waiting for the published message to arrive via the real nats-server \
             — the handler's publish never reached the independent subscriber",
        )
        .expect("stream ended without yielding a message")
        .expect("message must decode without a broker error");
    let decoded: OrderPayload =
        serde_json::from_slice(&received.payload).expect("payload must be valid JSON");
    assert_eq!(
        decoded,
        OrderPayload {
            id: "order-e2e-1".to_string()
        },
        "the independent NATS subscriber must receive exactly what the served handler \
         published — this is the real end-to-end proof, not just that publish() didn't error"
    );

    handle.abort();
}
