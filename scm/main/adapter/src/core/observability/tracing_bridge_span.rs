//! [`TracingBridgeSpan`] — accumulates annotations for one handler
//! invocation and exports them as a single JSON span object on `finish()`.

use std::sync::Arc;
use std::time::Instant;

use edge_application_observer::{
    ObserveError, Span, SpanAnnotationRequest, SpanAnnotationResponse, SpanFinishRequest,
    SpanFinishResponse,
};
use parking_lot::Mutex;
use swe_observ_tracing::TracerProvider;

/// One open invocation span. Annotations recorded via `record()` accumulate
/// until `finish()` flattens `handler_id`/`operation`/`duration_us` plus
/// every annotation into a single JSON object and hands it to the backend.
pub(crate) struct TracingBridgeSpan {
    handler_id: String,
    operation: String,
    started_at: Instant,
    fields: Mutex<serde_json::Map<String, serde_json::Value>>,
    backend: Arc<dyn TracerProvider>,
}

impl TracingBridgeSpan {
    pub(crate) fn new(
        handler_id: String,
        operation: String,
        backend: Arc<dyn TracerProvider>,
    ) -> Self {
        Self {
            handler_id,
            operation,
            started_at: Instant::now(),
            fields: Mutex::new(serde_json::Map::new()),
            backend,
        }
    }
}

impl Span for TracingBridgeSpan {
    fn record(&self, req: SpanAnnotationRequest) -> Result<SpanAnnotationResponse, ObserveError> {
        self.fields
            .lock()
            .insert(req.key, serde_json::Value::String(req.value));
        Ok(SpanAnnotationResponse)
    }

    fn finish(&self, _req: SpanFinishRequest) -> Result<SpanFinishResponse, ObserveError> {
        let mut span = serde_json::Map::new();
        span.insert(
            "handler_id".to_string(),
            serde_json::Value::String(self.handler_id.clone()),
        );
        span.insert(
            "operation".to_string(),
            serde_json::Value::String(self.operation.clone()),
        );
        span.insert(
            "duration_us".to_string(),
            serde_json::Value::from(self.started_at.elapsed().as_micros() as u64),
        );
        for (key, value) in std::mem::take(&mut *self.fields.lock()) {
            span.insert(key, value);
        }
        let span = serde_json::Value::Object(span);
        tracing::debug!(node = "tracing_bridge_span", %span, "invocation span exported");
        self.backend.export_span(&span);
        Ok(SpanFinishResponse)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_finish_exports_handler_id_and_operation_happy() {
        let backend = Arc::new(RecordingProvider::default());
        let span = TracingBridgeSpan::new(
            "echo".to_string(),
            "execute".to_string(),
            Arc::clone(&backend) as Arc<dyn TracerProvider>,
        );
        span.finish(SpanFinishRequest).unwrap();
        let exported = backend.recent_spans(10);
        assert_eq!(exported.len(), 1, "finish must export exactly one span");
        assert_eq!(exported[0]["handler_id"], "echo");
        assert_eq!(exported[0]["operation"], "execute");
    }

    #[test]
    fn test_record_annotation_appears_in_exported_span_happy() {
        let backend = Arc::new(RecordingProvider::default());
        let span = TracingBridgeSpan::new(
            "echo".to_string(),
            "execute".to_string(),
            Arc::clone(&backend) as Arc<dyn TracerProvider>,
        );
        span.record(SpanAnnotationRequest {
            key: "order.id".to_string(),
            value: "ord-42".to_string(),
        })
        .unwrap();
        span.finish(SpanFinishRequest).unwrap();
        let exported = backend.recent_spans(10);
        assert_eq!(exported[0]["order.id"], "ord-42");
    }

    #[test]
    fn test_finish_without_any_annotation_still_exports_edge() {
        let backend = Arc::new(RecordingProvider::default());
        let span = TracingBridgeSpan::new(
            "echo".to_string(),
            "execute".to_string(),
            Arc::clone(&backend) as Arc<dyn TracerProvider>,
        );
        span.finish(SpanFinishRequest).unwrap();
        assert_eq!(backend.recent_spans(10).len(), 1);
    }
}
