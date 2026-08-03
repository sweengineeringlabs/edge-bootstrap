//! `HttpIngressJob` — bridges [`DefaultHttpJob`] into the [`HttpIngress`]
//! contract the wire server actually calls.

use std::sync::Arc;

use edge_application_command::NoopCommandBus;
use edge_application_handler::{ExecutionRequest, HandlerContext};
use edge_application_observer::ObserverContext;
use edge_proxy::{Job, JobError};
use swe_edge_ingress_http::{
    HealthCheckRequest, HealthCheckResponse, HttpFuture, HttpHealthCheck, HttpIngress,
    HttpIngressError, HttpResponse, InboundRequest,
};

use super::DefaultHttpJob;

/// Adapts [`DefaultHttpJob`] (an `edge-proxy::Job`) to the `HttpIngress`
/// contract the wire server holds — the only place `Job` and `HttpIngress`
/// meet, per ADR-021.
pub(crate) struct HttpIngressJob {
    job: Arc<DefaultHttpJob>,
    observer: Arc<dyn ObserverContext>,
}

impl HttpIngressJob {
    /// Construct an ingress bridging `job`, using `observer` for every
    /// dispatched request's `HandlerContext.observer`.
    pub(crate) fn new(job: Arc<DefaultHttpJob>, observer: Arc<dyn ObserverContext>) -> Self {
        Self { job, observer }
    }

    fn map_job_error(err: JobError) -> HttpIngressError {
        match err {
            JobError::HandlerUnavailable(m) => HttpIngressError::NotFound(m),
            JobError::Routing(e) => HttpIngressError::NotFound(e.to_string()),
            JobError::Handler(m) => HttpIngressError::Internal(m),
            JobError::Cancelled => HttpIngressError::Unavailable("job cancelled".to_string()),
        }
    }
}

impl HttpIngress for HttpIngressJob {
    fn handle(
        &self,
        req: InboundRequest,
    ) -> HttpFuture<'_, Result<HttpResponse, HttpIngressError>> {
        let InboundRequest { request, ctx, .. } = req;
        let security = ctx.into_security_context();
        let job = Arc::clone(&self.job);
        let observer = Arc::clone(&self.observer);
        HttpFuture::new(async move {
            let commands = NoopCommandBus;
            let hctx = HandlerContext {
                security: &security,
                commands: &commands,
                observer: observer.as_ref(),
            };
            let resp = job
                .run(ExecutionRequest {
                    req: request,
                    ctx: &hctx,
                })
                .await
                .map_err(Self::map_job_error)?;
            Ok(resp.payload)
        })
    }

    fn health_check(
        &self,
        _req: HealthCheckRequest,
    ) -> HttpFuture<'_, Result<HealthCheckResponse, HttpIngressError>> {
        let job = Arc::clone(&self.job);
        HttpFuture::new(async move {
            let health = if job.health_check().await {
                HttpHealthCheck::healthy()
            } else {
                HttpHealthCheck::unhealthy("one or more registered handlers reported unhealthy")
            };
            Ok(HealthCheckResponse { health })
        })
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::sync::Arc;

    use async_trait::async_trait;
    use edge_application::{Handler, HandlerError, Request, Response, SecurityContext};
    use edge_application_handler::{
        ExecutionRequest as HandlerExecutionRequest,
        HealthCheckRequest as HandlerHealthCheckRequest,
        HealthCheckResponse as HandlerHealthCheckResponse, IdRequest, IdResponse, PatternRequest,
        PatternResponse,
    };
    use edge_application_observer::StdObserveFactory;
    use swe_edge_ingress_http::{
        HttpDecodeFn, HttpEncodeFn, HttpIngressError, HttpRequest, RequestContext,
    };

    use super::*;

    #[derive(Clone)]
    struct PingReq;
    impl Request for PingReq {}
    struct PingResp;
    impl Response for PingResp {}

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
            _req: HandlerExecutionRequest<'_, PingReq>,
        ) -> Result<PingResp, HandlerError> {
            Ok(PingResp)
        }
        async fn health_check(
            &self,
            _req: HandlerHealthCheckRequest,
        ) -> Result<HandlerHealthCheckResponse, HandlerError> {
            Ok(HandlerHealthCheckResponse { healthy: true })
        }
    }

    fn decode(_req: &HttpRequest) -> Result<PingReq, HttpIngressError> {
        Ok(PingReq)
    }
    fn encode(_resp: PingResp) -> HttpResponse {
        HttpResponse::new(200, b"pong".to_vec())
    }

    fn ctx() -> RequestContext {
        RequestContext::new(SecurityContext::unauthenticated())
    }

    fn peer_addr() -> SocketAddr {
        "127.0.0.1:0".parse().expect("valid socket addr")
    }

    fn ingress_with_ping() -> HttpIngressJob {
        let job = DefaultHttpJob::new();
        job.register_route(Arc::new(PingHandler), decode, encode)
            .expect("register_route must succeed");
        HttpIngressJob::new(Arc::new(job), StdObserveFactory::noop_arc_observe_context())
    }

    /// @covers: HttpIngressJob::handle
    #[tokio::test]
    async fn test_handle_dispatches_registered_route_happy() {
        let ingress = ingress_with_ping();
        let resp = ingress
            .handle(InboundRequest::new(
                HttpRequest::get("/ping"),
                ctx(),
                peer_addr(),
            ))
            .await
            .expect("handle must succeed");
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, b"pong".to_vec());
    }

    /// @covers: HttpIngressJob::handle
    #[tokio::test]
    async fn test_handle_unregistered_path_returns_not_found_error() {
        let ingress = ingress_with_ping();
        let err = ingress
            .handle(InboundRequest::new(
                HttpRequest::get("/nope"),
                ctx(),
                peer_addr(),
            ))
            .await
            .unwrap_err();
        assert!(matches!(err, HttpIngressError::NotFound(_)));
    }

    /// @covers: HttpIngressJob::health_check
    #[tokio::test]
    async fn test_health_check_with_healthy_handler_reports_healthy_happy() {
        let ingress = ingress_with_ping();
        let resp = ingress
            .health_check(HealthCheckRequest::new(peer_addr()))
            .await
            .expect("health_check must succeed");
        assert!(resp.health.healthy);
    }
}
