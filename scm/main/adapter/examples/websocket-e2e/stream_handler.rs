//! WebSocket streaming handler — the one thing this module owns is "what
//! happens to a connection once its WebSocket upgrade handshake completes."
//!
//! Registered via `RuntimeBuilder::stream_handler()`, which `serve()` passes
//! straight through to `AxumHttpServer::with_stream_handler()` (see
//! `src/core/runtime/runtime_builder_serve.rs`). The server itself decides,
//! per request, whether `Upgrade: websocket` routes here or plain requests
//! fall through to [`crate::handler`] — this module never sees an HTTP
//! route or a URL path, only the full-duplex `WsChannel` left after a real
//! handshake already succeeded.
//!
//! The loop below reads every frame off `channel.receiver` and writes it
//! back out unchanged through `channel.sender` — a genuine echo, not a
//! canned reply. A client that sends "ping" and gets back anything other
//! than "ping" proves this loop broke; a client that never gets a reply at
//! all proves the upgrade itself never reached real application code.

use futures::StreamExt;

use swe_edge_ingress_http::{
    HttpFuture, HttpIngressError, HttpStream, SseUpgradeRequest, SseUpgradeResponse,
    WebsocketUpgradeRequest,
};

pub(crate) struct EchoStream;

impl HttpStream for EchoStream {
    fn handle_sse(
        &self,
        _req: SseUpgradeRequest,
    ) -> HttpFuture<'_, Result<SseUpgradeResponse, HttpIngressError>> {
        HttpFuture::new(async {
            Err(HttpIngressError::MethodNotAllowed(
                "websocket-e2e: SSE is not wired in this demo".to_string(),
            ))
        })
    }

    fn handle_websocket(
        &self,
        req: WebsocketUpgradeRequest,
    ) -> HttpFuture<'_, Result<(), HttpIngressError>> {
        HttpFuture::new(async move {
            let WebsocketUpgradeRequest {
                mut channel,
                peer_addr,
                ..
            } = req;
            tracing::info!(peer = %peer_addr, "websocket-e2e: upgrade complete, echoing frames");

            while let Some(frame) = channel.receiver.next().await {
                let msg = frame?;
                channel
                    .sender
                    .send(msg)
                    .await
                    .map_err(|e| HttpIngressError::Internal(e.to_string()))?;
            }

            tracing::info!(peer = %peer_addr, "websocket-e2e: peer closed the connection");
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use edge_application::SecurityContext;
    use swe_edge_ingress_http::{
        HttpRequest, RequestContext, WsChannel, WsMessage, WsReceiver, WsSender,
    };
    use tokio::sync::mpsc;

    fn upgrade_request(channel: WsChannel) -> WebsocketUpgradeRequest {
        WebsocketUpgradeRequest::new(
            HttpRequest::get("/ws"),
            RequestContext::new(SecurityContext::unauthenticated()),
            channel,
            "127.0.0.1:0".parse().expect("valid socket addr"),
        )
    }

    /// Drives the echo loop directly against an in-process frame stream —
    /// no socket, no Axum, no `RuntimeBuilder`. This proves the loop's own
    /// logic (read frame, write frame back, stop at stream end); the
    /// integration test in `tests/websocket_e2e_int_test.rs` proves the
    /// wiring that gets a real socket's frames into this loop at all.
    #[tokio::test]
    async fn test_handle_websocket_echoes_text_frame_happy() {
        let (out_tx, mut out_rx) = mpsc::channel::<WsMessage>(4);
        let incoming = WsReceiver::new(futures::stream::iter(vec![Ok(WsMessage::text("ping"))]));
        let channel = WsChannel {
            sender: WsSender::new(out_tx),
            receiver: incoming,
        };

        EchoStream
            .handle_websocket(upgrade_request(channel))
            .await
            .expect("handle_websocket must return Ok once the peer stream ends");

        let echoed = out_rx.recv().await.expect("must receive an echoed frame");
        assert_eq!(
            echoed.data, b"ping",
            "the echo loop must send back exactly what it received"
        );
        assert!(!echoed.binary, "a text frame in must stay a text frame out");
    }

    #[tokio::test]
    async fn test_handle_websocket_empty_stream_returns_ok_edge() {
        let (out_tx, _out_rx) = mpsc::channel::<WsMessage>(4);
        let incoming = WsReceiver::new(futures::stream::empty());
        let channel = WsChannel {
            sender: WsSender::new(out_tx),
            receiver: incoming,
        };

        let result = EchoStream.handle_websocket(upgrade_request(channel)).await;
        assert!(
            result.is_ok(),
            "a peer that closes without sending anything must not error"
        );
    }

    #[tokio::test]
    async fn test_handle_sse_returns_method_not_allowed_error() {
        let result = EchoStream
            .handle_sse(SseUpgradeRequest::new(
                HttpRequest::get("/sse"),
                RequestContext::new(SecurityContext::unauthenticated()),
                "127.0.0.1:0".parse().expect("valid socket addr"),
            ))
            .await;
        assert!(matches!(result, Err(HttpIngressError::MethodNotAllowed(_))));
    }
}
