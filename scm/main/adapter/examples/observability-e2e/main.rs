//! Observability, end to end — one `Handler` served over HTTP, with a
//! durable file-backed log drain, a real Prometheus-backed metrics stream,
//! and a live tracing subscriber, all wired through the real
//! `RuntimeBuilder` surface.
//!
//! Run:
//!     cargo run -p swe-edge-bootstrap --features observability --example observability-e2e
//!
//! Three pillars, each proven live rather than merely configured:
//!
//!   1. Logging ([`runtime_config`]): `RuntimeConfig.log_backend` selects the
//!      file backend (`swe_observ_logging::FileLogger`), a JSONL file under
//!      the OS temp dir. `FileLogger` writes through a `BufWriter`, which
//!      only reaches disk when its internal buffer fills or `.flush()` is
//!      called explicitly — `LogDrainBridge::emit()` (this crate's
//!      `core::observability::log_drain_bridge`) calls `self.backend.flush()`
//!      after every `emit()`, so every entry [`handler`] logs is durable on
//!      disk before the handler returns, not sitting in memory waiting to be
//!      lost to a hard kill.
//!   2. Metrics ([`runtime_config`]): `RuntimeConfig.metrics` starts the
//!      `/metrics` Prometheus scrape endpoint and activates
//!      `HttpLoadMonitor` automatically; whenever the `observability`
//!      feature is compiled in, `RuntimeConfig.metrics_backend` additionally
//!      swaps the counters' storage to a real `PrometheusMetrics` provider —
//!      the same instance backs both the load monitor and
//!      `ObserverContext.metrics()`, so [`handler`]'s own
//!      `observability_handler_invocations_total` counter shows up on the
//!      same `/metrics` response as the framework's `edge_requests_total`.
//!   3. Tracing ([`tracing_setup`]): every node in the pipeline that touches
//!      a request — `http_load_monitor`, [`handler`]'s own `execute` span —
//!      announces itself via `tracing::debug!`/`info!`, so a request's path
//!      through the system is actually observable, not just inferable from
//!      side effects afterward.
//!
//! Split by responsibility rather than one `main()` doing everything:
//! [`handler`] owns the domain payload/`Handler`, [`runtime_config`] owns
//! bind addresses and the log/metrics backend sections, [`tracing_setup`]
//! owns log verbosity — `main` only wires the three together and serves.

mod handler;
mod runtime_config;
mod tracing_setup;

use swe_edge_bootstrap::Runtime;

#[tokio::main]
async fn main() {
    let handler = handler::build_handler();
    let config = runtime_config::build_runtime_config();
    let tracing_config = tracing_setup::build_tracing_config();

    println!("http:    http://{}/observe", runtime_config::HTTP_BIND);
    println!("grpc:    {}", runtime_config::GRPC_BIND);
    println!(
        "metrics: http://{}{}",
        runtime_config::METRICS_BIND,
        runtime_config::METRICS_PATH
    );
    println!("logs:    {}", runtime_config::log_file_path().display());

    let builder = Runtime::builder()
        .config(config)
        .http_route(handler.clone())
        .grpc_route(handler)
        .grpc_allow_unauthenticated();

    // `.with_tracing()` only exists behind the `observability` feature —
    // without it, the pipeline's `tracing::debug!`/`info!` calls are still
    // real, they just have no subscriber installed to print them anywhere,
    // and `RuntimeConfig.log_backend`/`metrics_backend` fall back to their
    // in-memory defaults (see `runtime_config`'s doc comments for why that's
    // safe rather than a silent correctness gap).
    #[cfg(feature = "observability")]
    let builder = builder.with_tracing(tracing_config);
    #[cfg(not(feature = "observability"))]
    let _ = tracing_config;

    if let Err(e) = builder.serve().await {
        panic!("serve failed: {e}");
    }
}
