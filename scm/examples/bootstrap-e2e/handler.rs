//! Domain payload and `Handler` construction — the one thing this module
//! owns is "what does the shared HTTP+gRPC handler look like."
//!
//! Hand-written rather than `Domain::echo_handler` (a canned library
//! utility) specifically so it can carry its own `#[tracing::instrument]` —
//! proving a *consumer's own* handler is traceable, not just the
//! infra-level nodes edge-bootstrap itself wraps requests in.

use std::sync::Arc;

use async_trait::async_trait;
use edge_application_handler::{ExecutionRequest, IdRequest, IdResponse, PatternRequest, PatternResponse};
use edge_domain::{Handler, HandlerError};

/// Payload shared by both the HTTP and gRPC routes.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct EchoPayload(pub(crate) String);
impl edge_domain::Request for EchoPayload {}
impl edge_domain::Response for EchoPayload {}

struct EchoHandler;

#[async_trait]
impl Handler for EchoHandler {
    type Request = EchoPayload;
    type Response = EchoPayload;

    fn id(&self, _req: IdRequest) -> Result<IdResponse, HandlerError> {
        Ok(IdResponse {
            id: "echo".to_string(),
        })
    }

    fn pattern(&self, _req: PatternRequest) -> Result<PatternResponse, HandlerError> {
        Ok(PatternResponse {
            pattern: "/echo".to_string(),
        })
    }

    #[tracing::instrument(name = "echo_handler", skip(self, req), fields(node = "echo_handler"))]
    async fn execute(
        &self,
        req: ExecutionRequest<'_, EchoPayload>,
    ) -> Result<EchoPayload, HandlerError> {
        tracing::info!(payload = %req.req.0, "handler executing");
        let response = req.req;
        tracing::debug!(payload = %response.0, "handler returning");
        Ok(response)
    }
}

/// Build the echo `Handler`, shared verbatim across `.http_route()` and
/// `.grpc_route()` — proving one handler implementation serves both
/// protocols with no duplicated logic.
pub(crate) fn build_handler() -> Arc<dyn Handler<Request = EchoPayload, Response = EchoPayload>> {
    Arc::new(EchoHandler)
}
