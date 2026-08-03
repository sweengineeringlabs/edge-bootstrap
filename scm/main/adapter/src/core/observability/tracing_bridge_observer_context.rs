//! [`TracingBridgeObserverContext`] — real `ObserverContext` backed by
//! `swe-observability-tracing`'s `TracerProvider` (spans),
//! `swe-observability-logging`'s `LoggerProvider` (structured logs), and,
//! when a `MetricsProvider` is supplied, `MetricsBridgeRegistry` (counters/
//! gauges/histograms).
//!
//! Plugs into the `Handler`/`HandlerContext` seam in place of the
//! framework's noop default. Any `Handler::execute()` call — infra's own
//! handlers or a consumer's — opens real, exportable spans, emits real log
//! entries, and (when wired) records real metrics through it via
//! `HandlerContext.observer`, with no per-handler wiring required.

use std::sync::Arc;

use edge_application_observer::{
    DrainRequest, DrainResponse, LogDrain, MetricRegistry, MetricsRequest, MetricsResponse,
    ObserveError, ObserverContext, StdObserveFactory, TracerRequest, TracerResponse,
};
use swe_observ_logging::LoggerProvider;
use swe_observ_metrics::MetricsProvider as MetricsBackend;
use swe_observ_tracing::TracerProvider;

use super::log_drain_bridge::LogDrainBridge;
use super::metrics_bridge_registry::MetricsBridgeRegistry;
use super::tracing_bridge_handler_tracer::TracingBridgeHandlerTracer;

pub(crate) struct TracingBridgeObserverContext {
    tracer: TracingBridgeHandlerTracer,
    log_drain: Box<dyn LogDrain>,
    metric_registry: Box<dyn MetricRegistry>,
}

impl TracingBridgeObserverContext {
    pub(crate) fn new(
        tracer_backend: Arc<dyn TracerProvider>,
        log_backend: Arc<dyn LoggerProvider>,
        metrics_provider: Option<Arc<dyn MetricsBackend>>,
    ) -> Self {
        let metric_registry: Box<dyn MetricRegistry> = match metrics_provider {
            Some(provider) => Box::new(MetricsBridgeRegistry::new(provider)),
            None => StdObserveFactory::noop_metric_registry(),
        };
        Self {
            tracer: TracingBridgeHandlerTracer::new(tracer_backend),
            log_drain: Box::new(LogDrainBridge::new(log_backend)),
            metric_registry,
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

    fn log_backend() -> Arc<dyn LoggerProvider> {
        Arc::new(swe_observ_logging::create_local_logging_backend())
    }

    #[test]
    fn test_tracer_start_span_reaches_the_configured_backend_happy() {
        let backend = Arc::new(RecordingProvider::default());
        let observer = TracingBridgeObserverContext::new(
            Arc::clone(&backend) as Arc<dyn TracerProvider>,
            log_backend(),
            None,
        );
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
    fn test_drain_reaches_the_real_backend_happy() {
        let backend = Arc::new(RecordingProvider::default());
        let observer = TracingBridgeObserverContext::new(
            Arc::clone(&backend) as Arc<dyn TracerProvider>,
            log_backend(),
            None,
        );
        let drain = observer.drain(DrainRequest).unwrap().drain;
        let result = drain.emit(edge_application_observer::LogEmitRequest {
            level: "info".to_string(),
            handler_id: "echo".to_string(),
            message: "test".to_string(),
        });
        assert!(result.is_ok(), "drain emit must not error: {result:?}");
    }

    #[test]
    fn test_metrics_returns_a_working_noop_registry_when_no_provider_supplied_edge() {
        let backend = Arc::new(RecordingProvider::default());
        let observer = TracingBridgeObserverContext::new(
            Arc::clone(&backend) as Arc<dyn TracerProvider>,
            log_backend(),
            None,
        );
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

    #[test]
    fn test_metrics_reaches_the_real_backend_when_provider_supplied_happy() {
        let tracer_backend = Arc::new(RecordingProvider::default());
        let metrics_backend: Arc<dyn MetricsBackend> =
            Arc::new(swe_observ_metrics::create_local_metrics_backend());
        let observer = TracingBridgeObserverContext::new(
            tracer_backend as Arc<dyn TracerProvider>,
            log_backend(),
            Some(Arc::clone(&metrics_backend)),
        );
        observer
            .metrics(MetricsRequest)
            .unwrap()
            .metrics
            .counter(edge_application_observer::CounterLookupRequest {
                name: "observer_context_metric".to_string(),
            })
            .unwrap()
            .counter
            .increment(edge_application_observer::IncrementRequest { delta: 7 })
            .unwrap();
        let snap = metrics_backend.export();
        assert!(
            snap.iter()
                .any(|s| s.name == "observer_context_metric" && s.value == 7.0),
            "expected the metric to reach the real backend, got: {snap:?}"
        );
    }
}
