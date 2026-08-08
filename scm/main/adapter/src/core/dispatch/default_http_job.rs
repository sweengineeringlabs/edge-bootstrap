//! `DefaultHttpJob` — the concrete `edge-proxy::Job` implementation
//! `edge-proxy` itself ships none of (only `NullJob`/scaffolding). Bridges
//! `Ingress -> Proxy(Job/Router) -> Dispatch(HandlerRegistry/Pipeline) -> Handler::execute`
//! per ADR-021 (see `docs/3-design/adr/003-adopt-adr-021-system-request-flow.md`).

use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;

use edge_application::Handler;
use edge_application_handler::{
    ExecutionRequest, HandlerError, HandlerLookupRequest,
    HealthCheckRequest as HandlerHealthCheckRequest,
    HealthCheckResponse as HandlerHealthCheckResponse, IdRequest, IdResponse, PatternRequest,
    PatternResponse, RegisterHandlerRequest,
};
use edge_dispatch::{HandlerComposer, HandlerRegistry, Pipeline, StageConfig};
use edge_proxy::{Job, JobError, JobResponse, RouteRequest, RouteResponse, Router, RoutingError};
use swe_edge_ingress_http::{HttpDecodeFn, HttpEncodeFn, HttpRequest, HttpResponse};

use super::Payload;

/// Pure type witnesses satisfying `edge_application::Request`/`Response` —
/// never instantiated, only named, to fix `create_registry::<H>()`'s type
/// parameters.
#[derive(Clone)]
struct WitnessRequest;
impl edge_application::Request for WitnessRequest {}
struct WitnessResponse;
impl edge_application::Response for WitnessResponse {}

/// The registry's actual payload types — deliberately not `HttpRequest`/
/// `HttpResponse` directly. `HttpRequest`/`HttpResponse` stopped implementing
/// `edge_application::Request`/`Response` in edge-transport-http-ingress#29's
/// ADR-021 refactor (transport must not know `edge-application` exists), and
/// both types are foreign to this crate, so the orphan rule blocks adding
/// the impls here. Routed through the shared `Payload` wrapper (see
/// `super::Payload`) instead — mirrors `default_grpc_job.rs`'s identical
/// `GrpcPayload` wrapper for the same reason.
type HttpRequestPayload = Payload<HttpRequest>;
type HttpResponsePayload = Payload<HttpResponse>;

/// Bridges a typed `Handler<Req, Resp>` into the HTTP-typed registry
/// (`Handler<HttpRequestPayload, HttpResponsePayload>`) via a decode/encode
/// pair — the same erasure the transport crate used to own before ADR-021
/// (see edge-transport-http-ingress#29); it now lives here, at the
/// composition root, not in transport.
struct BridgedHttpHandler<Req, Resp>
where
    Req: Send + 'static,
    Resp: Send + 'static,
{
    inner: Arc<dyn Handler<Request = Req, Response = Resp>>,
    decode: HttpDecodeFn<Req>,
    encode: HttpEncodeFn<Resp>,
}

#[async_trait]
impl<Req, Resp> Handler for BridgedHttpHandler<Req, Resp>
where
    Req: Send + 'static + edge_application::Request,
    Resp: Send + 'static + edge_application::Response,
{
    type Request = HttpRequestPayload;
    type Response = HttpResponsePayload;

    fn id(&self, req: IdRequest) -> Result<IdResponse, HandlerError> {
        self.inner.id(req)
    }

    fn pattern(&self, req: PatternRequest) -> Result<PatternResponse, HandlerError> {
        self.inner.pattern(req)
    }

    async fn execute(
        &self,
        req: ExecutionRequest<'_, HttpRequestPayload>,
    ) -> Result<HttpResponsePayload, HandlerError> {
        let typed =
            (self.decode)(&req.req.0).map_err(|e| HandlerError::InvalidRequest(e.to_string()))?;
        let resp = self
            .inner
            .execute(ExecutionRequest {
                req: typed,
                ctx: req.ctx,
            })
            .await?;
        Ok(Payload((self.encode)(resp)))
    }

    async fn health_check(
        &self,
        req: HandlerHealthCheckRequest,
    ) -> Result<HandlerHealthCheckResponse, HandlerError> {
        self.inner.health_check(req).await
    }
}

struct JobHandlerComposer;
impl HandlerComposer for JobHandlerComposer {}

/// Error registering a route on [`DefaultHttpJob`].
#[derive(Debug, thiserror::Error)]
pub(crate) enum JobRegistrationError {
    /// The route pattern conflicts with an already-registered route.
    #[error("failed to register pattern `{pattern}`: {reason}")]
    RegistrationFailed { pattern: String, reason: String },
    /// The handler itself rejected registration (its own `id`/`pattern` lookup failed).
    #[error("handler rejected registration: {0}")]
    HandlerRejected(String),
}

/// The real `edge-proxy::Job` implementation this org's `edge-proxy` crate
/// ships none of — owns path routing (`Router`) and dispatches through
/// `edge-dispatch`'s `HandlerRegistry`/`Pipeline`, reaching `Handler::execute`
/// only at the end of that chain.
pub(crate) struct DefaultHttpJob {
    router: RwLock<matchit::Router<String>>,
    registry:
        Arc<dyn HandlerRegistry<Request = HttpRequestPayload, Response = HttpResponsePayload>>,
    /// Pre-`Pipeline`-wrap handlers, keyed by id, kept purely for
    /// `health_check`: `edge_dispatch::Pipeline`'s own `Handler` impl only
    /// overrides `id`/`pattern`/`execute` — its `health_check` falls back to
    /// the trait's always-healthy default and never reaches the wrapped
    /// stage, so checking `self.registry`'s stored value can't see real
    /// handler health at all.
    health_handlers: RwLock<
        std::collections::HashMap<
            String,
            Arc<dyn Handler<Request = HttpRequestPayload, Response = HttpResponsePayload>>,
        >,
    >,
}

impl Default for DefaultHttpJob {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultHttpJob {
    /// Construct a job with no routes registered.
    pub(crate) fn new() -> Self {
        Self {
            router: RwLock::new(matchit::Router::new()),
            registry: Arc::new(JobHandlerComposer::create_registry::<
                BridgedHttpHandler<WitnessRequest, WitnessResponse>,
            >()),
            health_handlers: RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Register a typed handler under its own `id`/`pattern`, wrapped in a
    /// single-stage [`Pipeline`] so every route is dispatched through
    /// Pipeline mediation (ADR-005), not a raw `Handler::execute()` call.
    pub(crate) fn register_route<Req, Resp>(
        &self,
        handler: Arc<dyn Handler<Request = Req, Response = Resp>>,
        decode: HttpDecodeFn<Req>,
        encode: HttpEncodeFn<Resp>,
    ) -> Result<(), JobRegistrationError>
    where
        Req: Send + 'static + edge_application::Request,
        Resp: Send + 'static + edge_application::Response,
    {
        let id = handler
            .id(IdRequest)
            .map_err(|e| JobRegistrationError::HandlerRejected(e.to_string()))?
            .id;
        if id.is_empty() {
            return Err(JobRegistrationError::HandlerRejected(
                "Handler::id() must not be empty".to_string(),
            ));
        }
        let pattern = handler
            .pattern(PatternRequest)
            .map_err(|e| JobRegistrationError::HandlerRejected(e.to_string()))?
            .pattern;
        if pattern.is_empty() {
            return Err(JobRegistrationError::HandlerRejected(
                "Handler::pattern() must not be empty — HTTP routes on pattern, not id; a \
                 Handler that only overrides id() compiles, works over gRPC, and would \
                 otherwise silently never match any HTTP request"
                    .to_string(),
            ));
        }
        let bridged = BridgedHttpHandler {
            inner: handler,
            decode,
            encode,
        };
        let erased: Arc<dyn Handler<Request = HttpRequestPayload, Response = HttpResponsePayload>> =
            Arc::new(bridged);
        self.health_handlers
            .write()
            .insert(id.clone(), Arc::clone(&erased));
        let pipeline = Pipeline::<HttpRequestPayload, HttpResponsePayload>::builder()
            .id(id.clone())
            .pattern(pattern.clone())
            .stage(id.clone(), erased, StageConfig::passthrough())
            .build();
        self.registry
            .register(RegisterHandlerRequest::new(Arc::new(pipeline)))
            .map_err(|e| JobRegistrationError::HandlerRejected(e.to_string()))?;
        self.router
            .write()
            .insert(pattern.clone(), id)
            .map_err(|e| JobRegistrationError::RegistrationFailed {
                pattern,
                reason: e.to_string(),
            })?;
        Ok(())
    }

    fn path_from_url(url: &str) -> String {
        url.parse::<http::Uri>()
            .map(|u| u.path().to_string())
            .unwrap_or_else(|_| {
                url.split('?')
                    .next()
                    .and_then(|s| s.split('#').next())
                    .unwrap_or("/")
                    .to_string()
            })
    }

    /// Aggregate health across every registered route's handler — mirrors
    /// the transport crate's former per-handler health aggregation (now
    /// this crate's job, since transport no longer knows `Handler` exists).
    ///
    /// Checks `health_handlers` (the pre-`Pipeline`-wrap handler), not
    /// `self.registry` — see the field doc for why the registered `Pipeline`
    /// itself can't answer this.
    ///
    /// Returns `false` on the first unhealthy handler.
    pub(crate) async fn health_check(&self) -> bool {
        let handlers: Vec<_> = self.health_handlers.read().values().cloned().collect();
        for h in handlers {
            match h.health_check(HandlerHealthCheckRequest).await {
                Ok(resp) if resp.healthy => continue,
                _ => return false,
            }
        }
        true
    }
}

#[async_trait]
impl Router<String> for DefaultHttpJob {
    async fn route(&self, req: RouteRequest<'_>) -> Result<RouteResponse<String>, RoutingError> {
        let router = self.router.read();
        let m = router.at(req.input).map_err(|_| RoutingError::NoMatch)?;
        Ok(RouteResponse {
            intent: m.value.clone(),
        })
    }
}

#[async_trait]
impl Job<HttpRequest, HttpResponse> for DefaultHttpJob {
    async fn run(
        &self,
        req: ExecutionRequest<'_, HttpRequest>,
    ) -> Result<JobResponse<HttpResponse>, JobError> {
        let ExecutionRequest { req: http_req, ctx } = req;
        let path = Self::path_from_url(&http_req.url);
        let id = self.route(RouteRequest { input: &path }).await?.intent;
        let handler = self
            .registry
            .get(HandlerLookupRequest { id: id.clone() })
            .map_err(|e| JobError::Handler(e.to_string()))?
            .handler
            .ok_or(JobError::HandlerUnavailable(id))?;
        let resp = handler
            .execute(ExecutionRequest {
                req: Payload(http_req),
                ctx,
            })
            .await
            .map_err(|e| JobError::Handler(e.to_string()))?;
        Ok(JobResponse { payload: resp.0 })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use edge_application::{HandlerContext, SecurityContext};
    use edge_application_command::NoopCommandBus;
    use edge_application_handler::{HealthCheckRequest, HealthCheckResponse};
    use edge_application_observer::StdObserveFactory;

    use super::*;

    struct PingHandler;
    #[async_trait]
    impl Handler for PingHandler {
        type Request = PingReq;
        type Response = PingResp;
        fn id(&self, _req: IdRequest) -> Result<IdResponse, HandlerError> {
            Ok(IdResponse {
                id: "ping".to_string(),
            })
        }
        fn pattern(&self, _req: PatternRequest) -> Result<PatternResponse, HandlerError> {
            Ok(PatternResponse {
                pattern: "/ping".to_string(),
            })
        }
        async fn execute(
            &self,
            _req: ExecutionRequest<'_, PingReq>,
        ) -> Result<PingResp, HandlerError> {
            Ok(PingResp)
        }
        async fn health_check(
            &self,
            _req: HealthCheckRequest,
        ) -> Result<HealthCheckResponse, HandlerError> {
            Ok(HealthCheckResponse { healthy: true })
        }
    }

    #[derive(Clone)]
    struct PingReq;
    impl edge_application::Request for PingReq {}
    struct PingResp;
    impl edge_application::Response for PingResp {}

    fn decode(_req: &HttpRequest) -> Result<PingReq, swe_edge_ingress_http::HttpIngressError> {
        Ok(PingReq)
    }
    fn encode(_resp: PingResp) -> HttpResponse {
        HttpResponse::new(200, b"pong".to_vec())
    }

    fn job_with_ping() -> DefaultHttpJob {
        let job = DefaultHttpJob::new();
        job.register_route(Arc::new(PingHandler), decode, encode)
            .expect("register_route must succeed");
        job
    }

    /// @covers: DefaultHttpJob::new
    #[tokio::test]
    async fn test_new_starts_with_no_routes_registered_edge() {
        let job = DefaultHttpJob::new();
        let err = job.route(RouteRequest { input: "/ping" }).await;
        assert!(matches!(err, Err(RoutingError::NoMatch)));
    }

    /// @covers: DefaultHttpJob::register_route
    #[tokio::test]
    async fn test_register_route_makes_path_routable_happy() {
        let job = job_with_ping();
        let resolved = job
            .route(RouteRequest { input: "/ping" })
            .await
            .expect("route must succeed");
        assert_eq!(resolved.intent, "ping");
    }

    /// @covers: DefaultHttpJob::register_route
    #[test]
    fn test_register_route_duplicate_pattern_returns_error_error() {
        let job = job_with_ping();
        let err = job
            .register_route(Arc::new(PingHandler), decode, encode)
            .unwrap_err();
        assert!(matches!(
            err,
            JobRegistrationError::RegistrationFailed { .. }
        ));
    }

    /// Regression test for `#32`: a `Handler` that only implements `id()`
    /// correctly and leaves `pattern()` at its default empty string used to
    /// register successfully here, then silently never match any real HTTP
    /// request. This now fails loudly at registration time instead.
    #[test]
    fn test_register_route_rejects_empty_pattern_negative() {
        struct EmptyPatternHandler;
        #[async_trait]
        impl Handler for EmptyPatternHandler {
            type Request = PingReq;
            type Response = PingResp;
            fn id(&self, _req: IdRequest) -> Result<IdResponse, HandlerError> {
                Ok(IdResponse {
                    id: "no-pattern".to_string(),
                })
            }
            async fn execute(
                &self,
                _req: ExecutionRequest<'_, PingReq>,
            ) -> Result<PingResp, HandlerError> {
                Ok(PingResp)
            }
        }

        let job = DefaultHttpJob::new();
        let err = job
            .register_route(Arc::new(EmptyPatternHandler), decode, encode)
            .unwrap_err();
        assert!(
            matches!(err, JobRegistrationError::HandlerRejected(ref msg) if msg.contains("pattern")),
            "expected a HandlerRejected error naming `pattern`, got: {err:?}"
        );
    }

    /// Symmetric defense-in-depth check: an empty `id()` is rejected too,
    /// even though HTTP routes on `pattern`, not `id` — `id` still keys the
    /// registry/health_handlers map, so an empty one is never valid here.
    #[test]
    fn test_register_route_rejects_empty_id_negative() {
        struct EmptyIdHandler;
        #[async_trait]
        impl Handler for EmptyIdHandler {
            type Request = PingReq;
            type Response = PingResp;
            fn id(&self, _req: IdRequest) -> Result<IdResponse, HandlerError> {
                Ok(IdResponse { id: String::new() })
            }
            fn pattern(&self, _req: PatternRequest) -> Result<PatternResponse, HandlerError> {
                Ok(PatternResponse {
                    pattern: "/no-id".to_string(),
                })
            }
            async fn execute(
                &self,
                _req: ExecutionRequest<'_, PingReq>,
            ) -> Result<PingResp, HandlerError> {
                Ok(PingResp)
            }
        }

        let job = DefaultHttpJob::new();
        let err = job
            .register_route(Arc::new(EmptyIdHandler), decode, encode)
            .unwrap_err();
        assert!(
            matches!(err, JobRegistrationError::HandlerRejected(ref msg) if msg.contains("id")),
            "expected a HandlerRejected error naming `id`, got: {err:?}"
        );
    }

    /// @covers: DefaultHttpJob::route
    #[tokio::test]
    async fn test_route_unknown_path_returns_no_match_error() {
        let job = job_with_ping();
        let result = job.route(RouteRequest { input: "/nope" }).await;
        assert!(matches!(result, Err(RoutingError::NoMatch)));
    }

    fn security() -> SecurityContext {
        SecurityContext::unauthenticated()
    }

    /// @covers: DefaultHttpJob::run
    /// Proves the full `Job::run -> Router -> HandlerRegistry -> Pipeline ->
    /// Handler::execute` chain actually runs, not just that dispatch
    /// succeeds — a bypassed Pipeline would still return 200 here, so this
    /// alone wouldn't prove Pipeline mediation; paired with the
    /// stage-lifecycle-event test below for that proof.
    #[tokio::test]
    async fn test_run_dispatches_through_full_chain_happy() {
        let job = job_with_ping();
        let commands = NoopCommandBus;
        let observer = StdObserveFactory::noop_arc_observe_context();
        let sec = security();
        let hctx = HandlerContext {
            security: &sec,
            commands: &commands,
            observer: observer.as_ref(),
        };
        let resp = Job::run(
            &job,
            ExecutionRequest {
                req: HttpRequest::get("/ping"),
                ctx: &hctx,
            },
        )
        .await
        .expect("run must succeed");
        assert_eq!(resp.payload.status, 200);
        assert_eq!(resp.payload.body, b"pong".to_vec());
    }

    /// @covers: DefaultHttpJob::run
    /// Proves dispatch actually runs through an `edge_dispatch::Pipeline`
    /// stage, not just accepts the dependency without using it — mirrors the
    /// proof edge-transport-grpc-ingress#33's own test suite uses for the
    /// same claim.
    #[tokio::test]
    async fn test_run_dispatches_through_registered_pipeline_emits_stage_lifecycle_events_happy() {
        struct EventCountingHandler {
            started: AtomicUsize,
        }
        #[async_trait]
        impl Handler for EventCountingHandler {
            type Request = PingReq;
            type Response = PingResp;
            fn id(&self, _req: IdRequest) -> Result<IdResponse, HandlerError> {
                Ok(IdResponse {
                    id: "counted".to_string(),
                })
            }
            fn pattern(&self, _req: PatternRequest) -> Result<PatternResponse, HandlerError> {
                Ok(PatternResponse {
                    pattern: "/counted".to_string(),
                })
            }
            async fn execute(
                &self,
                _req: ExecutionRequest<'_, PingReq>,
            ) -> Result<PingResp, HandlerError> {
                self.started.fetch_add(1, Ordering::SeqCst);
                Ok(PingResp)
            }
        }

        let job = DefaultHttpJob::new();
        let handler = Arc::new(EventCountingHandler {
            started: AtomicUsize::new(0),
        });
        job.register_route(
            Arc::clone(&handler) as Arc<dyn Handler<Request = PingReq, Response = PingResp>>,
            decode,
            encode,
        )
        .expect("register_route must succeed");

        let commands = NoopCommandBus;
        let observer = StdObserveFactory::noop_arc_observe_context();
        let sec = security();
        let hctx = HandlerContext {
            security: &sec,
            commands: &commands,
            observer: observer.as_ref(),
        };
        let resp = Job::run(
            &job,
            ExecutionRequest {
                req: HttpRequest::get("/counted"),
                ctx: &hctx,
            },
        )
        .await
        .expect("run must succeed");
        assert_eq!(resp.payload.status, 200);
        assert_eq!(
            handler.started.load(Ordering::SeqCst),
            1,
            "Job::run must have actually invoked the registered handler through the real \
             Pipeline, not just returned a response without dispatching to it"
        );
    }

    /// @covers: DefaultHttpJob::run
    #[tokio::test]
    async fn test_run_unregistered_path_returns_handler_unavailable_error() {
        let job = job_with_ping();
        let commands = NoopCommandBus;
        let observer = StdObserveFactory::noop_arc_observe_context();
        let sec = security();
        let hctx = HandlerContext {
            security: &sec,
            commands: &commands,
            observer: observer.as_ref(),
        };
        let err = Job::run(
            &job,
            ExecutionRequest {
                req: HttpRequest::get("/nope"),
                ctx: &hctx,
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(err, JobError::Routing(_)));
    }

    #[test]
    fn test_path_from_url_extracts_path_from_full_url() {
        assert_eq!(
            DefaultHttpJob::path_from_url("https://example.com/api/v1"),
            "/api/v1"
        );
    }

    /// @covers: DefaultHttpJob::health_check
    #[tokio::test]
    async fn test_health_check_with_no_routes_reports_healthy_edge() {
        let job = DefaultHttpJob::new();
        assert!(job.health_check().await);
    }

    /// @covers: DefaultHttpJob::health_check
    #[tokio::test]
    async fn test_health_check_all_healthy_handlers_reports_healthy_happy() {
        let job = job_with_ping();
        assert!(job.health_check().await);
    }

    /// @covers: DefaultHttpJob::health_check
    #[tokio::test]
    async fn test_health_check_with_unhealthy_handler_reports_unhealthy_error() {
        struct UnhealthyHandler;
        #[async_trait]
        impl Handler for UnhealthyHandler {
            type Request = PingReq;
            type Response = PingResp;
            fn id(&self, _req: IdRequest) -> Result<IdResponse, HandlerError> {
                Ok(IdResponse {
                    id: "unhealthy".to_string(),
                })
            }
            fn pattern(&self, _req: PatternRequest) -> Result<PatternResponse, HandlerError> {
                Ok(PatternResponse {
                    pattern: "/unhealthy".to_string(),
                })
            }
            async fn execute(
                &self,
                _req: ExecutionRequest<'_, PingReq>,
            ) -> Result<PingResp, HandlerError> {
                Ok(PingResp)
            }
            async fn health_check(
                &self,
                _req: HealthCheckRequest,
            ) -> Result<HealthCheckResponse, HandlerError> {
                Ok(HealthCheckResponse { healthy: false })
            }
        }

        let job = DefaultHttpJob::new();
        job.register_route(Arc::new(UnhealthyHandler), decode, encode)
            .expect("register_route must succeed");
        assert!(!job.health_check().await);
    }
}
