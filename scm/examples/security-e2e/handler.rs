//! Domain payload and `Handler` construction — the one thing this module
//! owns is "what does the JWT-protected HTTP+gRPC handler look like."
//!
//! Deliberately minimal compared to `bootstrap-e2e`'s `EchoHandler` (which
//! also opens tracer spans and increments metrics through
//! `HandlerContext.observer`): this demo's subject is the auth boundary in
//! front of the handler, not the handler's own observability wiring. This
//! module still emits one real structured log line through
//! `HandlerContext.observer` per request, both as evidence the seam is live
//! and to make it visible in stdout which requests actually reached the
//! handler versus were rejected at the auth boundary before ever arriving
//! here — see `main.rs` for why `Handler::execute` can't read the verified
//! JWT's claims/principal itself in this version of the stack.

use std::sync::Arc;

use async_trait::async_trait;
use edge_application::{Handler, HandlerError};
use edge_application_handler::{
    DrainRequest, ExecutionRequest, IdRequest, IdResponse, LogEmitRequest, PatternRequest,
    PatternResponse,
};

/// Payload served over the JWT-protected `/secure-echo` HTTP route (and,
/// unauthenticated, the `/secure-echo` gRPC route — see `runtime_config`).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct SecurePayload {
    pub(crate) message: String,
}
impl edge_application::Request for SecurePayload {}
impl edge_application::Response for SecurePayload {}

struct SecureEchoHandler;

#[async_trait]
impl Handler for SecureEchoHandler {
    type Request = SecurePayload;
    type Response = SecurePayload;

    fn id(&self, _req: IdRequest) -> Result<IdResponse, HandlerError> {
        // Must carry the leading `/` — gRPC's real wire method is the HTTP/2
        // request path, which `DefaultGrpcJob` keys routes on directly.
        Ok(IdResponse {
            id: "/secure-echo".to_string(),
        })
    }

    fn pattern(&self, _req: PatternRequest) -> Result<PatternResponse, HandlerError> {
        Ok(PatternResponse {
            pattern: "/secure-echo".to_string(),
        })
    }

    async fn execute(
        &self,
        req: ExecutionRequest<'_, SecurePayload>,
    ) -> Result<SecurePayload, HandlerError> {
        // Reaching this line at all is the actual proof point of this demo:
        // `AxumHttpServerHelper::verify_auth` (edge-runtime, http-adapter
        // v0.2.0) runs ahead of ingress dispatch and returns a real `401`
        // response before `Handler::execute` is ever invoked, whenever the
        // bearer token is missing or fails `TokenVerifier::verify` — see the
        // module doc comment on `main`.
        if let Ok(d) = req.ctx.observer.drain(DrainRequest) {
            let _ = d.drain.emit(LogEmitRequest {
                level: "info".to_string(),
                handler_id: "secure-echo".to_string(),
                message: format!("authenticated request handled: {}", req.req.message),
            });
        }
        Ok(req.req)
    }
}

/// Build the handler shared verbatim by `.http_route()` and `.grpc_route()`.
/// gRPC has no bearer-auth enforcement wired in this version (see
/// `runtime_config`), so only the HTTP route actually demonstrates the JWT
/// auth boundary; the gRPC route is served open via
/// `.grpc_allow_unauthenticated()` in `main.rs`.
pub(crate) fn build_handler() -> Arc<dyn Handler<Request = SecurePayload, Response = SecurePayload>>
{
    Arc::new(SecureEchoHandler)
}

#[cfg(test)]
mod tests {
    use super::*;
    use edge_application::SecurityContext;
    use edge_application_command::NoopCommandBus;
    use edge_application_handler::HandlerContext;
    use edge_application_observer::StdObserveFactory;

    #[allow(clippy::unwrap_used)]
    #[test]
    fn test_id_returns_secure_echo_path() {
        assert_eq!(SecureEchoHandler.id(IdRequest).unwrap().id, "/secure-echo");
    }

    #[allow(clippy::unwrap_used)]
    #[test]
    fn test_pattern_returns_secure_echo_path() {
        assert_eq!(
            SecureEchoHandler.pattern(PatternRequest).unwrap().pattern,
            "/secure-echo"
        );
    }

    #[allow(clippy::unwrap_used)]
    #[test]
    fn test_build_handler_id_matches_secure_echo() {
        let handler = build_handler();
        assert_eq!(handler.id(IdRequest).unwrap().id, "/secure-echo");
    }

    #[allow(clippy::unwrap_used)]
    #[tokio::test]
    async fn test_execute_echoes_payload_back_unchanged() {
        let commands = NoopCommandBus;
        let security = SecurityContext::unauthenticated();
        let observer = StdObserveFactory::noop_arc_observe_context();
        let ctx = HandlerContext {
            security: &security,
            commands: &commands,
            observer: observer.as_ref(),
        };
        let payload = SecurePayload {
            message: "integration-test-payload".to_string(),
        };
        let resp = SecureEchoHandler
            .execute(ExecutionRequest {
                req: payload.clone(),
                ctx: &ctx,
            })
            .await
            .unwrap();
        assert_eq!(resp.message, payload.message);
    }
}
