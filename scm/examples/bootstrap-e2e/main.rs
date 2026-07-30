//! Full landscape — one `Handler` served over HTTP *and* gRPC, with metrics,
//! egress, and IDS/IPS all wired through the real `RuntimeBuilder` surface.
//!
//! Run:
//!     cargo run -p swe-edge-bootstrap --features "intrusion" --example bootstrap-e2e
//!
//! Unlike `hello_edge` (which hand-implements `HttpIngress` and dispatches
//! against a `HandlerRegistry` directly, bypassing most of `RuntimeBuilder`),
//! this example uses the layer `RuntimeBuilder` is actually built to cover:
//!
//!   1. A real `edge_domain::Handler` (`Domain::echo_handler`, [`handler`]),
//!      registered once and served over both `.http_route()` and
//!      `.grpc_route()` — the same handler, two protocols, no duplicated
//!      logic.
//!   2. `RuntimeBuilder::serve()` — real sockets bound for HTTP, gRPC, and a
//!      separate Prometheus metrics endpoint ([`runtime_config`]), not a
//!      hand-rolled listener.
//!   3. Metrics: `HttpLoadMonitor`/`GrpcLoadMonitor` wrap every request
//!      automatically once `RuntimeConfig.metrics` is set.
//!   4. Egress: `serve()` always builds a default `HttpEgress` client (the
//!      full auth/retry/rate/breaker/cache middleware stack) even though
//!      this handler doesn't call out anywhere — proves the wiring exists,
//!      not that a downstream call happens.
//!   5. IDS/IPS ([`intrusion`]): `edge-intrusion` wired in via
//!      `.with_intrusion(...)`, guarding *both* HTTP and gRPC automatically
//!      — a request matching a baseline signature rule is rejected before
//!      it reaches this handler at all.
//!
//! Split by responsibility rather than one `main()` doing everything:
//! [`handler`] owns the domain payload/`Handler`, [`intrusion`] owns the
//! IDS/IPS config, [`runtime_config`] owns bind addresses and optional
//! sections — `main` only wires the three together and serves.

mod handler;
mod intrusion;
mod runtime_config;

use swe_edge_bootstrap::Runtime;

#[tokio::main]
async fn main() {
    let handler = handler::build_handler();
    let wired = intrusion::build_intrusion_guard();
    let config = runtime_config::build_runtime_config();

    println!("http:    http://{}/echo", runtime_config::HTTP_BIND);
    println!("grpc:    {} (reflection on)", runtime_config::GRPC_BIND);
    println!(
        "metrics: http://{}{}",
        runtime_config::METRICS_BIND,
        runtime_config::METRICS_PATH
    );

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
