//! Full landscape — one `Handler` served over HTTP *and* gRPC, with metrics,
//! egress, IDS/IPS, and tracing all wired through the real `RuntimeBuilder`
//! surface.
//!
//! Run:
//!     cargo run -p swe-edge-bootstrap --features "intrusion,observability" --example bootstrap-e2e
//!
//! Unlike `hello_edge` (which hand-implements `HttpIngress` and dispatches
//! against a `HandlerRegistry` directly, bypassing most of `RuntimeBuilder`),
//! this example uses the layer `RuntimeBuilder` is actually built to cover:
//!
//!   1. A real, hand-written `edge_domain::Handler` ([`handler`]),
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
//!   6. Tracing ([`tracing_setup`]): every node in the pipeline that touches
//!      a request — `http_load_monitor`, `http_intrusion_guard`, and our
//!      own `echo_handler` — announces itself via `tracing::debug!`/`info!`,
//!      so a request's path through the system is actually observable, not
//!      just inferable from side effects afterward.
//!
//! Split by responsibility rather than one `main()` doing everything:
//! [`handler`] owns the domain payload/`Handler`, [`intrusion`] owns the
//! IDS/IPS config, [`runtime_config`] owns bind addresses and optional
//! sections, [`tracing_setup`] owns log verbosity — `main` only wires the
//! four together and serves.

mod handler;
mod intrusion;
mod runtime_config;
mod tracing_setup;

use swe_edge_bootstrap::Runtime;

#[tokio::main]
async fn main() {
    let handler = handler::build_handler();
    let wired = intrusion::build_intrusion_guard();
    let config = runtime_config::build_runtime_config();
    let tracing_config = tracing_setup::build_tracing_config();

    println!("http:    http://{}/echo", runtime_config::HTTP_BIND);
    println!("grpc:    {} (reflection on)", runtime_config::GRPC_BIND);
    println!(
        "metrics: http://{}{}",
        runtime_config::METRICS_BIND,
        runtime_config::METRICS_PATH
    );

    let builder = Runtime::builder()
        .config(config)
        .with_intrusion(wired)
        .http_route(handler.clone())
        .grpc_route(handler)
        .grpc_allow_unauthenticated();

    // `.with_tracing()` only exists behind the `observability` feature —
    // without it, the pipeline's `tracing::debug!`/`info!` calls are still
    // real, they just have no subscriber installed to print them anywhere.
    #[cfg(feature = "observability")]
    let builder = builder.with_tracing(tracing_config);
    #[cfg(not(feature = "observability"))]
    let _ = tracing_config;

    if let Err(e) = builder.serve().await {
        panic!("serve failed: {e}");
    }
}
