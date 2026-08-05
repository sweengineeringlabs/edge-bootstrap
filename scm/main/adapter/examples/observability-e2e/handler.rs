//! Domain payload and `Handler` construction — the one thing this module
//! owns is "what does the observed HTTP+gRPC handler look like."
//!
//! Hand-written rather than a canned library utility specifically so
//! `execute()` can reach into `HandlerContext.observer` and drive all three
//! observability pillars on every request: a real span through the tracer
//! backend, a real counter through the metrics backend, and a real
//! structured log entry through the (durable, file-backed) log drain —
//! proving a *consumer's own* handler reaches the same seam `RuntimeBuilder`
//! wires a real `ObserverContext` into, with no framework-internal import
//! beyond the `Handler` contract every consumer already needs. See
//! `docs/3-design/adr/`.

use std::sync::Arc;

use async_trait::async_trait;
use edge_application::{Handler, HandlerError};
use edge_application_handler::{
    CounterLookupRequest, DrainRequest, ExecutionRequest, IdRequest, IdResponse, IncrementRequest,
    LogEmitRequest, MetricsRequest, PatternRequest, PatternResponse, SpanAnnotationRequest,
    SpanFinishRequest, SpanStartRequest, TracerRequest,
};

/// Payload shared by both the HTTP and gRPC routes.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct ObservedPayload(pub(crate) String);
impl edge_application::Request for ObservedPayload {}
impl edge_application::Response for ObservedPayload {}

struct ObservedHandler;

#[async_trait]
impl Handler for ObservedHandler {
    type Request = ObservedPayload;
    type Response = ObservedPayload;

    fn id(&self, _req: IdRequest) -> Result<IdResponse, HandlerError> {
        // Must carry the leading `/` — gRPC's real wire method is the HTTP/2
        // request path (e.g. `/observe`), which `DefaultGrpcJob` keys routes
        // on directly; a bare `observe` here would never match an incoming
        // call.
        Ok(IdResponse {
            id: "/observe".to_string(),
        })
    }

    fn pattern(&self, _req: PatternRequest) -> Result<PatternResponse, HandlerError> {
        Ok(PatternResponse {
            pattern: "/observe".to_string(),
        })
    }

    async fn execute(
        &self,
        req: ExecutionRequest<'_, ObservedPayload>,
    ) -> Result<ObservedPayload, HandlerError> {
        // Pillar 1 — tracing: opens a real span through the same seam
        // edge-bootstrap's own infra uses (HandlerContext.observer), not a
        // framework-specific import, just the Handler contract every
        // consumer already has.
        let span = req.ctx.observer.tracer(TracerRequest).ok().and_then(|t| {
            t.tracer
                .start_span(SpanStartRequest {
                    handler_id: "observe".to_string(),
                    operation: "execute".to_string(),
                })
                .ok()
        });
        if let Some(ref s) = span {
            let _ = s.span.record(SpanAnnotationRequest {
                key: "payload".to_string(),
                value: req.req.0.clone(),
            });
        }
        let response = req.req;
        if let Some(s) = span {
            let _ = s.span.finish(SpanFinishRequest);
        }

        // Pillar 2 — metrics: lands in the same MetricsProvider.export()
        // stream HttpLoadMonitor's own counters use (and, whenever the
        // `observability` feature is compiled in, the same Prometheus
        // backend `[metrics_backend]` selects — see `runtime_config`).
        if let Ok(m) = req.ctx.observer.metrics(MetricsRequest) {
            if let Ok(c) = m.metrics.counter(CounterLookupRequest {
                name: "observability_handler_invocations_total".to_string(),
            }) {
                let _ = c.counter.increment(IncrementRequest { delta: 1 });
            }
        }

        // Pillar 3 — durable logging: `LogDrainBridge::emit()` flushes its
        // backend on every call, so this entry is on disk before this
        // function returns, not sitting in a `BufWriter` waiting to be lost
        // to a hard kill.
        if let Ok(d) = req.ctx.observer.drain(DrainRequest) {
            let _ = d.drain.emit(LogEmitRequest {
                level: "info".to_string(),
                handler_id: "observe".to_string(),
                message: format!("observed payload: {}", response.0),
            });
        }

        Ok(response)
    }
}

/// Build the observed `Handler`, shared verbatim across `.http_route()` and
/// `.grpc_route()` — proving one handler implementation serves both
/// protocols with no duplicated logic.
pub(crate) fn build_handler(
) -> Arc<dyn Handler<Request = ObservedPayload, Response = ObservedPayload>> {
    Arc::new(ObservedHandler)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id_returns_leading_slash_wire_path() {
        let handler = ObservedHandler;
        let id = handler.id(IdRequest).expect("id() should not fail");
        assert_eq!(id.id, "/observe");
    }

    #[test]
    fn test_pattern_matches_id() {
        let handler = ObservedHandler;
        let id = handler.id(IdRequest).expect("id() should not fail");
        let pattern = handler
            .pattern(PatternRequest)
            .expect("pattern() should not fail");
        assert_eq!(
            pattern.pattern, id.id,
            "route pattern must match the id gRPC routes on"
        );
    }

    #[test]
    fn test_build_handler_reports_the_same_id_as_the_underlying_handler() {
        let built = build_handler();
        let id = built.id(IdRequest).expect("id() should not fail");
        assert_eq!(id.id, "/observe");
    }
}
