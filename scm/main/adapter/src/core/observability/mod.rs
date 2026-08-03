//! Observability bridge — wires a real `ObserverContext` (backed by
//! `swe-observability-tracing`/`swe-observability-logging`/
//! `swe-observability-metrics`) into the `Handler` execution seam, in place
//! of the noop default every consumer otherwise gets from
//! `HandlerContext.observer`. See `docs/3-design/adr/` for the decision
//! record.

mod log_drain_bridge;
mod metrics_bridge_registry;
mod tracing_bridge_handler_tracer;
mod tracing_bridge_observer_context;
mod tracing_bridge_span;

use std::sync::Arc;

pub(crate) use tracing_bridge_observer_context::TracingBridgeObserverContext;

/// Build a `TracingBridgeObserverContext` over the given tracer/log
/// backends, plus a real `MetricRegistry` bridge when `metrics_provider` is
/// set (reusing the exact same `MetricsProvider` instance `TrafficCounters`
/// records into — one metrics stream, not two).
///
/// `tracer_backend`/`log_backend` are `Arc`-shared (not owned outright) so
/// the same instances can back more than one dispatcher's `ObserverContext`
/// (e.g. HTTP and gRPC) without opening a second, disconnected backend.
pub(crate) fn observer_context(
    tracer_backend: Arc<dyn swe_observ_tracing::TracerProvider>,
    log_backend: Arc<dyn swe_observ_logging::LoggerProvider>,
    metrics_provider: Option<Arc<dyn swe_observ_metrics::MetricsProvider>>,
) -> Arc<dyn edge_application_observer::ObserverContext> {
    Arc::new(TracingBridgeObserverContext::new(
        tracer_backend,
        log_backend,
        metrics_provider,
    ))
}
