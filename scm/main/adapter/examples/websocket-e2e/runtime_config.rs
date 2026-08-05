//! Runtime configuration assembly — the one thing this module owns is
//! "what bind address does this demo run with."
//!
//! HTTP-only: `.with_stream_handler(...)` is wired into the same
//! `AxumHttpServer` that serves plain HTTP, so a WebSocket upgrade needs no
//! separate listener — just an `Upgrade: websocket` header on a request to
//! the one HTTP bind address below.

use swe_edge_bootstrap::RuntimeConfig;

pub(crate) const HTTP_BIND: &str = "127.0.0.1:18690";

/// Build the `RuntimeConfig`: a single HTTP bind address, nothing else —
/// this demo has no gRPC route and no metrics/intrusion sections to keep
/// focus on the one thing under test, the stream handler wiring.
pub(crate) fn build_runtime_config() -> RuntimeConfig {
    RuntimeConfig::default()
        .with_service_name("websocket-e2e-demo")
        .with_http_bind(HTTP_BIND)
        .with_systemd_notify(false)
}
