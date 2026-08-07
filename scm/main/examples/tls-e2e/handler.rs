//! Domain payload and `Handler` construction — the one thing this module
//! owns is "what does the shared HTTP+gRPC handler look like."
//!
//! Deliberately trivial (an echo) — this demo's whole point is TLS
//! transport wiring, not handler logic, so the handler itself stays out of
//! the way and is shared verbatim across `.http_route()` and `.grpc_route()`.

use std::sync::Arc;

use async_trait::async_trait;
use edge_application::{Handler, HandlerError};
use edge_application_handler::{
    ExecutionRequest, IdRequest, IdResponse, PatternRequest, PatternResponse,
};

/// Payload shared by both the HTTP and gRPC routes.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct EchoPayload {
    pub(crate) text: String,
}
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

    // HTTP routing (unlike gRPC) keys `DefaultHttpJob`'s `matchit::Router` on
    // `pattern`, not `id` — see `DefaultHttpJob::register_route`.
    fn pattern(&self, _req: PatternRequest) -> Result<PatternResponse, HandlerError> {
        Ok(PatternResponse {
            pattern: "/echo".to_string(),
        })
    }

    async fn execute(
        &self,
        req: ExecutionRequest<'_, EchoPayload>,
    ) -> Result<EchoPayload, HandlerError> {
        Ok(req.req)
    }
}

/// Build the echo `Handler`, shared verbatim across `.http_route()` and
/// `.grpc_route()` — proving the same handler serves both protocols, and
/// (per this example's point) both a TLS-terminated and a plaintext
/// listener, with no protocol- or transport-specific logic of its own.
pub(crate) fn build_handler() -> Arc<dyn Handler<Request = EchoPayload, Response = EchoPayload>> {
    Arc::new(EchoHandler)
}

#[cfg(test)]
mod tests {
    use super::*;
    use edge_application::{HandlerContext, SecurityContext};
    use edge_application_command::NoopCommandBus;
    use edge_application_observer::StdObserveFactory;

    /// @covers: id
    #[test]
    fn test_id_returns_leading_slash_wire_path_happy() {
        assert_eq!(EchoHandler.id(IdRequest).unwrap().id, "/echo");
    }

    /// @covers: pattern
    #[test]
    fn test_pattern_matches_id_happy() {
        let id = EchoHandler.id(IdRequest).unwrap().id;
        let pattern = EchoHandler.pattern(PatternRequest).unwrap().pattern;
        assert_eq!(
            pattern, id,
            "the HTTP route pattern must match the path gRPC routes on"
        );
    }

    /// @covers: execute
    #[tokio::test]
    async fn test_execute_echoes_payload_back_unchanged_happy() {
        let security = SecurityContext::unauthenticated();
        let commands = NoopCommandBus;
        let observer = StdObserveFactory::noop_arc_observe_context();
        let ctx = HandlerContext {
            security: &security,
            commands: &commands,
            observer: observer.as_ref(),
        };
        let payload = EchoPayload {
            text: "tls-e2e-handler-test".to_string(),
        };
        let response = EchoHandler
            .execute(ExecutionRequest {
                req: payload.clone(),
                ctx: &ctx,
            })
            .await
            .expect("execute must succeed");
        assert_eq!(
            response, payload,
            "the handler must echo back exactly what it received"
        );
    }

    /// @covers: build_handler
    #[test]
    fn test_build_handler_reports_the_same_id_as_the_underlying_handler_happy() {
        let built = build_handler();
        assert_eq!(built.id(IdRequest).unwrap().id, "/echo");
    }
}
