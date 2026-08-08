//! WebSocket upgrade + message exchange, wired entirely through the real
//! `RuntimeBuilder` surface.
//!
//! Run:
//!     cargo run -p swe-edge-bootstrap --example websocket-e2e
//!
//! Then, from another terminal, drive a real handshake and echo round trip
//! with any WebSocket client, e.g. `websocat`:
//!     websocat ws://127.0.0.1:18690/ws
//!     > hello
//!     < hello
//!
//! What this proves that a unit test of `HttpStream` alone cannot:
//!
//!   1. `RuntimeBuilder::stream_handler()` ([`stream_handler`]) reaches all
//!      the way through `serve()` into `AxumHttpServer::with_stream_handler()`
//!      — the exact same builder surface `.http_route()` and `.grpc_route()`
//!      use, not a side channel.
//!   2. A request whose `Upgrade: websocket` header is set is routed to
//!      [`stream_handler::EchoStream::handle_websocket`] instead of falling
//!      through to [`handler`]'s plain HTTP route — both share one HTTP
//!      bind address, dispatched by header inspection alone (see
//!      `edge-runtime-http-adapter`'s `axum_http_server.rs` fallback route).
//!   3. The `WsChannel` handed to that handler is backed by a real, live
//!      TCP socket that already completed a real WebSocket handshake — not
//!      a mock full-duplex pair — so every frame the echo loop writes back
//!      actually reaches the connected peer.
//!
//! Split by responsibility rather than one `main()` doing everything:
//! [`handler`] owns the plain HTTP route required to satisfy
//! `RuntimeBuilder::serve()`'s "at least one route registered" check,
//! [`stream_handler`] owns the WebSocket echo loop under test,
//! [`runtime_config`] owns the bind address — `main` only wires the three
//! together and serves.

mod handler;
mod runtime_config;
mod stream_handler;

use std::sync::Arc;

use swe_edge_bootstrap::Runtime;

#[tokio::main]
async fn main() {
    let handler = handler::build_handler();
    let config = runtime_config::build_runtime_config();

    println!("http:      http://{}/echo", runtime_config::HTTP_BIND);
    println!(
        "websocket: ws://{}/ws (any path — routed by Upgrade header, not path)",
        runtime_config::HTTP_BIND
    );

    let result = Runtime::builder()
        .config(config)
        .http_route(handler)
        .stream_handler(Arc::new(stream_handler::EchoStream))
        .serve()
        .await;

    if let Err(e) = result {
        panic!("serve failed: {e}");
    }
}
