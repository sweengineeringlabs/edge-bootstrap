//! Integration test — `intrusion` feature: `RuntimeBuilder::with_intrusion` wiring.
//!
//! Exercises the *real* chain end-to-end through a live TCP connection:
//! `RuntimeBuilder::serve()` → `HttpIntrusionGuard` → `edge-intrusion`'s real
//! `DefaultRulesEngine`/`FailOpenEnforcer` → back out as an HTTP 403. Not a
//! unit test of the guard in isolation (those live alongside the adapter in
//! `main/adapter/src/core/intrusion/`) — this proves the wiring itself
//! composes correctly, which a unit test of the guard alone cannot.
#![cfg(feature = "intrusion")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use edge_intrusion::config::Config as IntrusionConfig;
use swe_edge_bootstrap::{Runtime, RuntimeConfig};
use swe_edge_ingress_http::{
    HealthCheckRequest, HealthCheckResponse, HttpFuture, HttpHealthCheck, HttpIngress,
    HttpIngressError, HttpResponse, InboundRequest,
};

struct EchoHandler;
impl HttpIngress for EchoHandler {
    fn handle(
        &self,
        req: InboundRequest,
    ) -> HttpFuture<'_, Result<HttpResponse, HttpIngressError>> {
        HttpFuture::new(async move { Ok(HttpResponse::new(200, req.request.url.into_bytes())) })
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

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    listener.local_addr().expect("local_addr").port()
}

#[tokio::test]
async fn test_serve_with_intrusion_rejects_signature_match_but_allows_clean_request() {
    let addr = format!("127.0.0.1:{}", free_port());

    let wired = IntrusionConfig::from_toml_str("")
        .expect("empty config parses to baseline defaults")
        .build()
        .expect("baseline signature rules build without error");

    let config = RuntimeConfig::default()
        .with_http_bind(addr.clone())
        .with_systemd_notify(false);

    let handle = tokio::spawn(async move {
        Runtime::builder()
            .config(config)
            .with_intrusion(wired)
            .http_handler(Arc::new(EchoHandler))
            .grpc_allow_unauthenticated()
            .serve()
            .await
    });

    // Give the server a moment to bind before connecting.
    tokio::time::sleep(Duration::from_millis(200)).await;

    let blocked = reqwest::get(format!("http://{addr}/download?file=/etc/passwd"))
        .await
        .expect("request must reach the server");
    assert_eq!(
        blocked.status(),
        403,
        "a request matching the lfi-etc-passwd baseline signature rule must be rejected"
    );

    let clean = reqwest::get(format!("http://{addr}/hello"))
        .await
        .expect("request must reach the server");
    assert_eq!(
        clean.status(),
        200,
        "a request matching no rule must reach the real handler"
    );
    assert_eq!(clean.text().await.unwrap(), "/hello");

    handle.abort();
}
