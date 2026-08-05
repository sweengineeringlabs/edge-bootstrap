//! Plain HTTP route — the one thing this module owns is satisfying
//! `RuntimeBuilder::serve()`'s requirement that at least one of
//! `.http_route()`/`.grpc_route()` be registered (`input.has_any()`); the
//! WebSocket upgrade itself never reaches this handler at all — see
//! [`crate::stream_handler`], which the server routes `Upgrade: websocket`
//! requests to instead.

use std::sync::Arc;

use async_trait::async_trait;
use edge_application::{Handler, HandlerError};
use edge_application_handler::{
    ExecutionRequest, IdRequest, IdResponse, PatternRequest, PatternResponse,
};

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
        Ok(IdResponse {
            id: "/echo".to_string(),
        })
    }

    fn pattern(&self, _req: PatternRequest) -> Result<PatternResponse, HandlerError> {
        // `Handler::pattern` defaults to an empty string — `DefaultHttpJob`
        // routes on this, not on `id`, so leaving the default here would
        // make `/echo` unroutable despite `id()` claiming it.
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

/// Build the plain HTTP echo `Handler` registered via `.http_route()`.
pub(crate) fn build_handler() -> Arc<dyn Handler<Request = EchoPayload, Response = EchoPayload>> {
    Arc::new(EchoHandler)
}

#[cfg(test)]
mod tests {
    use super::*;
    use edge_application::SecurityContext;
    use edge_application_command::NoopCommandBus;
    use edge_application_handler::HandlerContext;
    use edge_application_observer::StdObserveFactory;

    #[tokio::test]
    async fn test_execute_returns_payload_unchanged_happy() {
        let handler = EchoHandler;
        let payload = EchoPayload("round-trip".to_string());
        let commands = NoopCommandBus;
        let security = SecurityContext::unauthenticated();
        let observer = StdObserveFactory::noop_arc_observe_context();
        let ctx = HandlerContext {
            security: &security,
            commands: &commands,
            observer: observer.as_ref(),
        };
        let response = handler
            .execute(ExecutionRequest {
                req: payload.clone(),
                ctx: &ctx,
            })
            .await
            .expect("execute must succeed for a well-formed payload");
        assert_eq!(
            response.0, payload.0,
            "the HTTP echo handler must return exactly what it received"
        );
    }

    #[test]
    fn test_id_reports_leading_slash_path_happy() {
        let handler = build_handler();
        let id = handler.id(IdRequest).expect("id must succeed");
        assert_eq!(id.id, "/echo");
    }
}
