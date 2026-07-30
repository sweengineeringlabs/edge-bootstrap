//! Observability bridge — wires a real `ObserverContext` (backed by
//! `swe-observability-tracing`'s `TracerProvider`) into the `Handler`
//! execution seam, in place of the noop default every consumer otherwise
//! gets from `HandlerContext.observer`. See `docs/3-design/adr/` for the
//! decision record.

mod log_drain_bridge;
mod metrics_bridge_registry;
mod tracing_bridge_handler_tracer;
mod tracing_bridge_observer_context;
mod tracing_bridge_span;

use std::sync::Arc;

pub(crate) use tracing_bridge_observer_context::TracingBridgeObserverContext;

/// Build a `TracingBridgeObserverContext` over the in-memory default
/// `TracerProvider`/`LoggerProvider` backends, plus a real `MetricRegistry`
/// bridge when `metrics_provider` is set (reusing the exact same
/// `MetricsProvider` instance `TrafficCounters` records into — one metrics
/// stream, not two).
pub(crate) fn observer_context(
    metrics_provider: Option<Arc<dyn swe_observ_metrics::MetricsProvider>>,
) -> Arc<dyn edge_application_observer::ObserverContext> {
    let tracer_backend = swe_observ_tracing::create_default_tracer_arc();
    let log_backend = Box::new(swe_observ_logging::create_local_logging_backend());
    Arc::new(TracingBridgeObserverContext::new(
        tracer_backend,
        log_backend,
        metrics_provider,
    ))
}
