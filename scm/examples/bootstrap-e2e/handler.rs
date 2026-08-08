//! Domain payload and `Handler` construction — the one thing this module
//! owns is "what does the shared HTTP+gRPC handler look like."
//!
//! Hand-written rather than `Domain::echo_handler` (a canned library
//! utility) specifically so `execute()` can reach into `HandlerContext.observer`
//! — proving a *consumer's own* handler reaches the same real tracer/
//! metrics/log backends edge-bootstrap's own infra uses (`RuntimeBuilder`
//! wires a real `ObserverContext` in place of the framework's noop default;
//! see `docs/3-design/adr/`), with no import beyond the `Handler` contract
//! every consumer already needs.

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
pub(crate) struct EchoPayload(pub(crate) String);
impl edge_application::Request for EchoPayload {}
impl edge_application::Response for EchoPayload {}

struct EchoHandler;

#[async_trait]
impl Handler for EchoHandler {
    type Request = EchoPayload;
    type Response = EchoPayload;

    fn id(&self, _req: IdRequest) -> Result<IdResponse, HandlerError> {
        // Must carry the leading `/` — gRPC's real wire method is the HTTP/2
        // request path (e.g. `/echo`), which `DefaultGrpcJob` keys routes on
        // directly; a bare `echo` here would never match an incoming call.
        Ok(IdResponse {
            id: "/echo".to_string(),
        })
    }

    fn pattern(&self, _req: PatternRequest) -> Result<PatternResponse, HandlerError> {
        Ok(PatternResponse {
            pattern: "/echo".to_string(),
        })
    }

    async fn execute(
        &self,
        req: ExecutionRequest<'_, EchoPayload>,
    ) -> Result<EchoPayload, HandlerError> {
        // Opens a real span through the same seam edge-bootstrap's own
        // infra uses (HandlerContext.observer) — not a framework-specific
        // import, just the Handler contract every consumer already has.
        let span = req.ctx.observer.tracer(TracerRequest).ok().and_then(|t| {
            t.tracer
                .start_span(SpanStartRequest {
                    handler_id: "echo".to_string(),
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

        // Real metrics through the same seam — lands in the same
        // MetricsProvider.export() stream HttpLoadMonitor's own counters use.
        if let Ok(m) = req.ctx.observer.metrics(MetricsRequest) {
            if let Ok(c) = m.metrics.counter(CounterLookupRequest {
                name: "echo_handler_invocations_total".to_string(),
            }) {
                let _ = c.counter.increment(IncrementRequest { delta: 1 });
            }
        }

        // Real structured log through the same seam.
        if let Ok(d) = req.ctx.observer.drain(DrainRequest) {
            let _ = d.drain.emit(LogEmitRequest {
                level: "info".to_string(),
                handler_id: "echo".to_string(),
                message: format!("echoed payload: {}", response.0),
            });
        }

        Ok(response)
    }
}

/// Build the echo `Handler`, shared verbatim across `.http_route()` and
/// `.grpc_route()` — proving one handler implementation serves both
/// protocols with no duplicated logic.
pub(crate) fn build_handler() -> Arc<dyn Handler<Request = EchoPayload, Response = EchoPayload>> {
    Arc::new(EchoHandler)
}
