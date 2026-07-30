//! [`TracingBridgeObserverContext`] — real `ObserverContext` backed by
//! `swe-observability-tracing`'s `TracerProvider`.
//!
//! Plugs into the `Handler`/`HandlerContext` seam in place of the
//! framework's noop default. Any `Handler::execute()` call — infra's own
//! handlers or a consumer's — opens real, exportable spans through it via
//! `HandlerContext.observer`, with no per-handler wiring required.
//!
//! `drain()`/`metrics()` intentionally pass through to `StdObserveFactory`'s
//! noop primitives: this bridge is scoped to invocation tracing (spans)
//! only — log/metric backends are a separate, not-yet-requested concern.

use std::sync::Arc;

use edge_application_observer::{
    DrainRequest, DrainResponse, LogDrain, MetricRegistry, MetricsRequest, MetricsResponse,
    ObserveError, ObserverContext, StdObserveFactory, TracerRequest, TracerResponse,
};
use swe_observ_tracing::TracerProvider;

use super::tracing_bridge_handler_tracer::TracingBridgeHandlerTracer;

pub(crate) struct TracingBridgeObserverContext {
    tracer: TracingBridgeHandlerTracer,
    log_drain: Box<dyn LogDrain>,
    metric_registry: Box<dyn MetricRegistry>,
}

impl TracingBridgeObserverContext {
    pub(crate) fn new(backend: Arc<dyn TracerProvider>) -> Self {
        Self {
            tracer: TracingBridgeHandlerTracer::new(backend),
            log_drain: StdObserveFactory::noop_log_drain(),
            metric_registry: StdObserveFactory::noop_metric_registry(),
        }
    }
}

impl ObserverContext for TracingBridgeObserverContext {
    fn tracer(&self, _req: TracerRequest) -> Result<TracerResponse<'_>, ObserveError> {
        Ok(TracerResponse {
            tracer: &self.tracer,
        })
    }

    fn drain(&self, _req: DrainRequest) -> Result<DrainResponse<'_>, ObserveError> {
        Ok(DrainResponse {
            drain: self.log_drain.as_ref(),
        })
    }

    fn metrics(&self, _req: MetricsRequest) -> Result<MetricsResponse<'_>, ObserveError> {
        Ok(MetricsResponse {
            metrics: self.metric_registry.as_ref(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use edge_application_observer::{SpanFinishRequest, SpanStartRequest};
    use parking_lot::Mutex;

    #[derive(Debug, Default)]
    struct RecordingProvider {
        exported: Mutex<Vec<serde_json::Value>>,
    }

    impl TracerProvider for RecordingProvider {
        fn export_span(&self, span: &serde_json::Value) {
            self.exported.lock().push(span.clone());
        }
        fn flush(&self) {}
        fn recent_spans(&self, limit: usize) -> Vec<serde_json::Value> {
            self.exported.lock().iter().take(limit).cloned().collect()
        }
    }

    #[test]
    fn test_tracer_start_span_reaches_the_configured_backend_happy() {
        let backend = Arc::new(RecordingProvider::default());
        let observer =
            TracingBridgeObserverContext::new(Arc::clone(&backend) as Arc<dyn TracerProvider>);
        let span = observer
            .tracer(TracerRequest)
            .unwrap()
            .tracer
            .start_span(SpanStartRequest {
                handler_id: "echo".to_string(),
                operation: "execute".to_string(),
            })
            .unwrap()
            .span;
        span.finish(SpanFinishRequest).unwrap();
        assert_eq!(backend.recent_spans(10).len(), 1);
    }

    #[test]
    fn test_drain_returns_a_working_noop_drain_edge() {
        let backend = Arc::new(RecordingProvider::default());
        let observer =
            TracingBridgeObserverContext::new(Arc::clone(&backend) as Arc<dyn TracerProvider>);
        let drain = observer.drain(DrainRequest).unwrap().drain;
        let result = drain.emit(edge_application_observer::LogEmitRequest {
            level: "info".to_string(),
            handler_id: "echo".to_string(),
            message: "test".to_string(),
        });
        assert!(result.is_ok(), "noop drain must not error: {result:?}");
    }

    #[test]
    fn test_metrics_returns_a_working_noop_registry_edge() {
        let backend = Arc::new(RecordingProvider::default());
        let observer =
            TracingBridgeObserverContext::new(Arc::clone(&backend) as Arc<dyn TracerProvider>);
        let metrics = observer.metrics(MetricsRequest).unwrap().metrics;
        let counter = metrics
            .counter(edge_application_observer::CounterLookupRequest {
                name: "test".to_string(),
            })
            .unwrap()
            .counter;
        assert!(
            counter
                .increment(edge_application_observer::IncrementRequest { delta: 1 })
                .is_ok(),
            "noop counter must not error"
        );
    }
}
