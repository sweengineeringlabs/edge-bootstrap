//! [`TracingBridgeHandlerTracer`] — opens [`TracingBridgeSpan`]s against a
//! shared `TracerProvider` backend.

use std::sync::Arc;

use edge_application_observer::{HandlerTracer, ObserveError, SpanStartRequest, SpanStartResponse};
use swe_observ_tracing::TracerProvider;

use super::tracing_bridge_span::TracingBridgeSpan;

pub(crate) struct TracingBridgeHandlerTracer {
    backend: Arc<dyn TracerProvider>,
}

impl TracingBridgeHandlerTracer {
    pub(crate) fn new(backend: Arc<dyn TracerProvider>) -> Self {
        Self { backend }
    }
}

impl HandlerTracer for TracingBridgeHandlerTracer {
    fn start_span(&self, req: SpanStartRequest) -> Result<SpanStartResponse, ObserveError> {
        Ok(SpanStartResponse {
            span: Box::new(TracingBridgeSpan::new(
                req.handler_id,
                req.operation,
                Arc::clone(&self.backend),
            )),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn test_start_span_returns_a_span_that_exports_on_finish_happy() {
        let backend = Arc::new(RecordingProvider::default());
        let tracer =
            TracingBridgeHandlerTracer::new(Arc::clone(&backend) as Arc<dyn TracerProvider>);
        let span = tracer
            .start_span(SpanStartRequest {
                handler_id: "echo".to_string(),
                operation: "execute".to_string(),
            })
            .unwrap()
            .span;
        span.finish(edge_application_observer::SpanFinishRequest)
            .unwrap();
        assert_eq!(backend.recent_spans(10).len(), 1);
    }

    #[test]
    fn test_multiple_start_span_calls_share_the_same_backend_edge() {
        let backend = Arc::new(RecordingProvider::default());
        let tracer =
            TracingBridgeHandlerTracer::new(Arc::clone(&backend) as Arc<dyn TracerProvider>);
        for _ in 0..3 {
            let span = tracer
                .start_span(SpanStartRequest {
                    handler_id: "echo".to_string(),
                    operation: "execute".to_string(),
                })
                .unwrap()
                .span;
            span.finish(edge_application_observer::SpanFinishRequest)
                .unwrap();
        }
        assert_eq!(backend.recent_spans(10).len(), 3);
    }
}
