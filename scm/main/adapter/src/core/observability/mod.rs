//! Observability bridge — wires a real `ObserverContext` (backed by
//! `swe-observability-tracing`'s `TracerProvider`) into the `Handler`
//! execution seam, in place of the noop default every consumer otherwise
//! gets from `HandlerContext.observer`. See `docs/3-design/adr/` for the
//! decision record.

mod tracing_bridge_handler_tracer;
mod tracing_bridge_observer_context;
mod tracing_bridge_span;

use std::sync::Arc;

pub(crate) use tracing_bridge_observer_context::TracingBridgeObserverContext;

/// Build a `TracingBridgeObserverContext` over the in-memory default
/// `TracerProvider` backend. Spans recorded through it are retrievable via
/// the backend's own `recent_spans()` — useful for local verification and as
/// the fallback when no external backend (file/Jaeger/OTel) is configured.
pub(crate) fn default_observer_context() -> Arc<dyn edge_application_observer::ObserverContext> {
    let backend = swe_observ_tracing::create_default_tracer_arc();
    Arc::new(TracingBridgeObserverContext::new(backend))
}
