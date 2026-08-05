//! Domain payload and `Handler` construction — the one thing this module
//! owns is "what does the `/scheduler/status` HTTP+gRPC handler look like."
//!
//! Unlike `bootstrap-e2e`'s `EchoHandler` (which echoes whatever it's given),
//! this handler reports state: the live value of a shared `AtomicUsize`
//! counter that [`crate::scheduler_setup`]'s background job increments once
//! per real interval tick. Reading it through an `Arc` (not a value captured
//! at construction time) is the whole point — see the `#[cfg(test)]` module
//! below for the test that would fail if that weren't true.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use edge_application::{Handler, HandlerError};
use edge_application_handler::{
    ExecutionRequest, IdRequest, IdResponse, PatternRequest, PatternResponse,
};

/// Empty request body — this route reports state, it doesn't consume input.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct StatusQuery {}
impl edge_application::Request for StatusQuery {}

/// Current tick count of the background scheduled job at the moment this
/// route was hit — `0` forever when the `scheduler` feature is compiled out.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct StatusResponse {
    pub(crate) ticks: usize,
}
impl edge_application::Response for StatusResponse {}

struct StatusHandler {
    ticks: Arc<AtomicUsize>,
}

#[async_trait]
impl Handler for StatusHandler {
    type Request = StatusQuery;
    type Response = StatusResponse;

    fn id(&self, _req: IdRequest) -> Result<IdResponse, HandlerError> {
        // Must carry the leading `/` — gRPC's real wire method is the HTTP/2
        // request path, which `DefaultGrpcJob` keys routes on directly.
        Ok(IdResponse {
            id: "/scheduler/status".to_string(),
        })
    }

    fn pattern(&self, _req: PatternRequest) -> Result<PatternResponse, HandlerError> {
        Ok(PatternResponse {
            pattern: "/scheduler/status".to_string(),
        })
    }

    async fn execute(
        &self,
        _req: ExecutionRequest<'_, StatusQuery>,
    ) -> Result<StatusResponse, HandlerError> {
        Ok(StatusResponse {
            ticks: self.ticks.load(Ordering::SeqCst),
        })
    }
}

/// Build the status `Handler`, shared verbatim across `.http_route()` and
/// `.grpc_route()` — proving one handler implementation serves both
/// protocols with no duplicated logic. `ticks` is the same `Arc` the
/// background scheduled job writes into, so every request observes its
/// current, live value.
pub(crate) fn build_handler(
    ticks: Arc<AtomicUsize>,
) -> Arc<dyn Handler<Request = StatusQuery, Response = StatusResponse>> {
    Arc::new(StatusHandler { ticks })
}

#[cfg(test)]
mod tests {
    use super::*;
    use edge_application::SecurityContext;
    use edge_application_command::NoopCommandBus;
    use edge_application_handler::HandlerContext;
    use edge_application_observer::StdObserveFactory;

    #[test]
    fn test_id_returns_leading_slash_wire_path() {
        let ticks = Arc::new(AtomicUsize::new(0));
        let handler = StatusHandler { ticks };
        let id = handler.id(IdRequest).expect("id() should not fail");
        assert_eq!(id.id, "/scheduler/status");
    }

    #[test]
    fn test_pattern_matches_id() {
        let ticks = Arc::new(AtomicUsize::new(0));
        let handler = StatusHandler { ticks };
        let id = handler.id(IdRequest).expect("id() should not fail");
        let pattern = handler
            .pattern(PatternRequest)
            .expect("pattern() should not fail");
        assert_eq!(
            pattern.pattern, id.id,
            "route pattern must match the id gRPC routes on"
        );
    }

    async fn execute_status(handler: &StatusHandler) -> StatusResponse {
        let commands = NoopCommandBus;
        let security = SecurityContext::unauthenticated();
        let observer = StdObserveFactory::noop_arc_observe_context();
        let ctx = HandlerContext {
            security: &security,
            commands: &commands,
            observer: observer.as_ref(),
        };
        handler
            .execute(ExecutionRequest {
                req: StatusQuery {},
                ctx: &ctx,
            })
            .await
            .expect("execute() should not fail")
    }

    #[tokio::test]
    async fn test_execute_reports_current_tick_count() {
        let ticks = Arc::new(AtomicUsize::new(7));
        let handler = StatusHandler {
            ticks: Arc::clone(&ticks),
        };
        let resp = execute_status(&handler).await;
        assert_eq!(resp.ticks, 7);
    }

    /// The real proof this handler reads *live* shared state rather than a
    /// value snapshotted at construction: mutate the counter after building
    /// the handler and confirm the next `execute()` call sees the update.
    #[tokio::test]
    async fn test_execute_reflects_ticks_incremented_after_handler_construction() {
        let ticks = Arc::new(AtomicUsize::new(0));
        let handler = StatusHandler {
            ticks: Arc::clone(&ticks),
        };
        ticks.fetch_add(3, Ordering::SeqCst);
        let resp = execute_status(&handler).await;
        assert_eq!(
            resp.ticks, 3,
            "handler must read the live Arc<AtomicUsize>, not a stale snapshot"
        );
    }

    #[test]
    fn test_build_handler_id_matches_scheduler_status() {
        let built = build_handler(Arc::new(AtomicUsize::new(0)));
        let id = built.id(IdRequest).expect("id() should not fail");
        assert_eq!(id.id, "/scheduler/status");
    }
}
