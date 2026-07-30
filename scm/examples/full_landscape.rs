//! Full landscape — one `Handler` served over HTTP *and* gRPC, with metrics,
//! egress, and IDS/IPS all wired through the real `RuntimeBuilder` surface.
//!
//! Run:
//!     cargo run -p swe-edge-bootstrap --features "intrusion" --example full_landscape
//!
//! Unlike `hello_edge` (which hand-implements `HttpIngress` and dispatches
//! against a `HandlerRegistry` directly, bypassing most of `RuntimeBuilder`),
//! this example uses the layer `RuntimeBuilder` is actually built to cover:
//!
//!   1. A real `edge_domain::Handler` (`Domain::echo_handler`), registered
//!      once and served over both `.http_route()` and `.grpc_route()` — the
//!      same handler, two protocols, no duplicated logic.
//!   2. `RuntimeBuilder::serve()` — real sockets bound for HTTP, gRPC, and a
//!      separate Prometheus metrics endpoint (`[metrics]` config), not a
//!      hand-rolled listener.
//!   3. Metrics: `HttpLoadMonitor`/`GrpcLoadMonitor` wrap every request
//!      automatically once `RuntimeConfig.metrics` is set.
//!   4. Egress: `serve()` always builds a default `HttpEgress` client (the
//!      full auth/retry/rate/breaker/cache middleware stack) even though
//!      this handler doesn't call out anywhere — proves the wiring exists,
//!      not that a downstream call happens.
//!   5. IDS/IPS: `edge-intrusion` wired in via `.with_intrusion(...)`, guarding
//!      *both* HTTP and gRPC automatically — a request matching a baseline
//!      signature rule is rejected before it reaches this handler at all.

use edge_domain::Domain;
use edge_intrusion::config::Config as IntrusionConfig;
use swe_edge_bootstrap::{MetricsConfig, Runtime, RuntimeConfig};

/// Payload shared by both the HTTP and gRPC routes.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct EchoPayload(String);
impl edge_domain::Request for EchoPayload {}
impl edge_domain::Response for EchoPayload {}

#[tokio::main]
async fn main() {
    // One Handler, shared across protocols.
    let handler = Domain.echo_handler::<EchoPayload>("echo", "/echo");

    // IDS/IPS: baseline OWASP signature rules, high-severity reject.
    let intrusion_config = match IntrusionConfig::from_toml_str("") {
        Ok(cfg) => cfg,
        Err(e) => panic!("empty config must parse to baseline defaults: {e}"),
    };
    let wired = match intrusion_config.build() {
        Ok(w) => w,
        Err(e) => panic!("baseline rules must build: {e}"),
    };

    let mut config = RuntimeConfig::default()
        .with_service_name("full-landscape-demo")
        .with_http_bind("127.0.0.1:18090")
        .with_grpc_bind("127.0.0.1:19090")
        .with_systemd_notify(false);
    config.metrics = Some(MetricsConfig {
        bind: "127.0.0.1:18091".into(),
        path: "/metrics".into(),
    });
    config.grpc_reflection = true;

    println!("http:    http://127.0.0.1:18090/echo");
    println!("grpc:    127.0.0.1:19090 (reflection on)");
    println!("metrics: http://127.0.0.1:18091/metrics");

    let result = Runtime::builder()
        .config(config)
        .with_intrusion(wired)
        .http_route(handler.clone())
        .grpc_route(handler)
        .grpc_allow_unauthenticated()
        .serve()
        .await;
    if let Err(e) = result {
        panic!("serve failed: {e}");
    }
}
