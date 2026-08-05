//! Integration test — proves a real WebSocket upgrade and message exchange
//! works through `RuntimeBuilder`'s `.stream_handler(...)`: a genuine TCP
//! socket, a genuine WebSocket handshake (`tokio-tungstenite`, a real
//! client, not a mock), and a genuine echoed frame reaching back across
//! that socket.
//!
//! Not a duplicate of the lower-level `edge-runtime-http-adapter` coverage:
//! that crate's own `axum_http_server_int_test.rs::test_with_stream_handler_does_not_panic_edge`
//! only asserts construction doesn't panic — it never opens a socket, never
//! performs a handshake, and never exchanges a frame. This test proves the
//! wiring end-to-end through the real `RuntimeBuilder` path: `.stream_handler()`
//! → `serve()` → `AxumHttpServer::with_stream_handler()` → a real upgrade →
//! application-level `HttpStream::handle_websocket` → a real echoed frame
//! back across the wire, which a construction-only test cannot show.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use edge_application::{Handler, HandlerError};
use edge_application_handler::{
    ExecutionRequest, IdRequest, IdResponse, PatternRequest, PatternResponse,
};
use futures::{SinkExt, StreamExt};
use swe_edge_bootstrap::{Runtime, RuntimeConfig};
use swe_edge_ingress_http::{
    HttpFuture, HttpIngressError, HttpStream, SseUpgradeRequest, SseUpgradeResponse,
    WebsocketUpgradeRequest,
};
use tokio_tungstenite::tungstenite::Message;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct WsE2ePayload {
    text: String,
}
impl edge_application::Request for WsE2ePayload {}
impl edge_application::Response for WsE2ePayload {}

struct PlainHttpHandler;

#[async_trait]
impl Handler for PlainHttpHandler {
    type Request = WsE2ePayload;
    type Response = WsE2ePayload;

    fn id(&self, _req: IdRequest) -> Result<IdResponse, HandlerError> {
        Ok(IdResponse {
            id: "/echo".to_string(),
        })
    }

    fn pattern(&self, _req: PatternRequest) -> Result<PatternResponse, HandlerError> {
        // `Handler::pattern` defaults to an empty string — `DefaultHttpJob`
        // routes on this, not on `id`, so this must be set explicitly.
        Ok(PatternResponse {
            pattern: "/echo".to_string(),
        })
    }

    async fn execute(
        &self,
        req: ExecutionRequest<'_, WsE2ePayload>,
    ) -> Result<WsE2ePayload, HandlerError> {
        Ok(req.req)
    }
}

/// Echoes every WebSocket frame it receives back to the same peer,
/// unchanged. The only `HttpStream` implementation under test here.
struct EchoStream;

impl HttpStream for EchoStream {
    fn handle_sse(
        &self,
        _req: SseUpgradeRequest,
    ) -> HttpFuture<'_, Result<SseUpgradeResponse, HttpIngressError>> {
        HttpFuture::new(async {
            Err(HttpIngressError::MethodNotAllowed(
                "SSE not wired in this test".to_string(),
            ))
        })
    }

    fn handle_websocket(
        &self,
        req: WebsocketUpgradeRequest,
    ) -> HttpFuture<'_, Result<(), HttpIngressError>> {
        HttpFuture::new(async move {
            let mut channel = req.channel;
            while let Some(frame) = channel.receiver.next().await {
                let msg = frame?;
                channel
                    .sender
                    .send(msg)
                    .await
                    .map_err(|e| HttpIngressError::Internal(e.to_string()))?;
            }
            Ok(())
        })
    }
}

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    listener.local_addr().expect("local_addr").port()
}

#[tokio::test]
async fn test_serve_stream_handler_echoes_real_websocket_message_happy() {
    let addr = format!("127.0.0.1:{}", free_port());

    let config = RuntimeConfig::default().with_http_bind(addr.clone());

    let handle = tokio::spawn(async move {
        Runtime::builder()
            .config(config)
            .http_route(Arc::new(PlainHttpHandler))
            .stream_handler(Arc::new(EchoStream))
            .serve()
            .await
    });

    // Give the server a moment to bind before connecting.
    tokio::time::sleep(Duration::from_millis(200)).await;

    let (ws_stream, response) = tokio_tungstenite::connect_async(format!("ws://{addr}/ws"))
        .await
        .expect("WebSocket handshake must succeed against the real RuntimeBuilder-served endpoint");
    assert_eq!(
        response.status(),
        101,
        "a successful upgrade must answer with HTTP 101 Switching Protocols"
    );

    let (mut write, mut read) = ws_stream.split();

    write
        .send(Message::Text("hello-websocket".into()))
        .await
        .expect("sending a real text frame over the upgraded socket must succeed");

    let echoed = tokio::time::timeout(Duration::from_secs(5), read.next())
        .await
        .expect("must receive an echoed frame before the timeout")
        .expect("the connection must not close before echoing")
        .expect("the received frame must not be a protocol error");

    match echoed {
        Message::Text(text) => assert_eq!(
            text.as_str(),
            "hello-websocket",
            "the server must echo back exactly the text frame it received, proving the \
             real RuntimeBuilder -> AxumHttpServer -> HttpStream::handle_websocket chain \
             ran end-to-end over a real socket"
        ),
        other => panic!("expected a text frame echoed back, got: {other:?}"),
    }

    write
        .send(Message::Close(None))
        .await
        .expect("closing the socket cleanly must succeed");

    handle.abort();
}

#[tokio::test]
async fn test_serve_stream_handler_echoes_binary_frame_happy() {
    let addr = format!("127.0.0.1:{}", free_port());

    let config = RuntimeConfig::default().with_http_bind(addr.clone());

    let handle = tokio::spawn(async move {
        Runtime::builder()
            .config(config)
            .http_route(Arc::new(PlainHttpHandler))
            .stream_handler(Arc::new(EchoStream))
            .serve()
            .await
    });

    tokio::time::sleep(Duration::from_millis(200)).await;

    let (ws_stream, _response) = tokio_tungstenite::connect_async(format!("ws://{addr}/stream"))
        .await
        .expect("WebSocket handshake must succeed on any path — dispatch is by header, not route");

    let (mut write, mut read) = ws_stream.split();

    let payload: Vec<u8> = vec![1, 2, 3, 4, 250, 251, 252];
    write
        .send(Message::Binary(payload.clone().into()))
        .await
        .expect("sending a real binary frame must succeed");

    let echoed = tokio::time::timeout(Duration::from_secs(5), read.next())
        .await
        .expect("must receive an echoed binary frame before the timeout")
        .expect("the connection must not close before echoing")
        .expect("the received frame must not be a protocol error");

    match echoed {
        Message::Binary(bytes) => assert_eq!(
            bytes.as_ref(),
            payload.as_slice(),
            "a binary frame in must come back as the same bytes, proving frames aren't \
             silently coerced to text by the real dispatch path"
        ),
        other => panic!("expected a binary frame echoed back, got: {other:?}"),
    }

    handle.abort();
}

#[tokio::test]
async fn test_connect_without_upgrade_header_falls_through_to_http_route_edge() {
    let addr = format!("127.0.0.1:{}", free_port());

    let config = RuntimeConfig::default().with_http_bind(addr.clone());

    let handle = tokio::spawn(async move {
        Runtime::builder()
            .config(config)
            .http_route(Arc::new(PlainHttpHandler))
            .stream_handler(Arc::new(EchoStream))
            .serve()
            .await
    });

    tokio::time::sleep(Duration::from_millis(200)).await;

    // A plain POST with no `Upgrade` header must reach `PlainHttpHandler`,
    // not `EchoStream` — proving the stream handler doesn't swallow every
    // request once registered, only ones that actually ask to upgrade.
    let response = reqwest::Client::new()
        .post(format!("http://{addr}/echo"))
        .json(&serde_json::json!({ "text": "not-a-websocket" }))
        .send()
        .await
        .expect("plain HTTP request must succeed");
    assert_eq!(
        response.status(),
        200,
        "a request without an Upgrade header must be served by the plain HTTP route"
    );
    let body: WsE2ePayload = response.json().await.expect("deserialize JSON body");
    assert_eq!(body.text, "not-a-websocket");

    handle.abort();
}
