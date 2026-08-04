//! `GrpcIngressJob` — bridges [`DefaultGrpcJob`] into the [`GrpcIngress`]
//! contract the wire server actually calls.

use std::sync::Arc;

use edge_application_command::NoopCommandBus;
use edge_application_handler::{ExecutionRequest, HandlerContext};
use edge_application_observer::ObserverContext;
use edge_proxy::{Job, JobError};
use futures::future::BoxFuture;
use swe_edge_ingress_grpc::{
    GrpcHealthCheck, GrpcIngress, GrpcIngressError, GrpcResponse, HealthCheckRequest,
    HealthCheckResponse, UnaryRequest,
};

use super::DefaultGrpcJob;

/// Adapts [`DefaultGrpcJob`] (an `edge-proxy::Job`) to the `GrpcIngress`
/// contract the wire server holds — the only place `Job` and `GrpcIngress`
/// meet, per ADR-021.
pub(crate) struct GrpcIngressJob {
    job: Arc<DefaultGrpcJob>,
    observer: Arc<dyn ObserverContext>,
}

impl GrpcIngressJob {
    /// Construct an ingress bridging `job`, using `observer` for every
    /// dispatched request's `HandlerContext.observer`.
    pub(crate) fn new(job: Arc<DefaultGrpcJob>, observer: Arc<dyn ObserverContext>) -> Self {
        Self { job, observer }
    }

    fn map_job_error(err: JobError) -> GrpcIngressError {
        match err {
            JobError::HandlerUnavailable(m) => GrpcIngressError::Unimplemented(m),
            JobError::Routing(e) => GrpcIngressError::Unimplemented(e.to_string()),
            JobError::Handler(m) => GrpcIngressError::Internal(m),
            JobError::Cancelled => GrpcIngressError::Unavailable("job cancelled".to_string()),
        }
    }
}

impl GrpcIngress for GrpcIngressJob {
    fn handle_unary(
        &self,
        req: UnaryRequest,
    ) -> BoxFuture<'_, Result<GrpcResponse, GrpcIngressError>> {
        let UnaryRequest { request, ctx, .. } = req;
        let job = Arc::clone(&self.job);
        let observer = Arc::clone(&self.observer);
        Box::pin(async move {
            let commands = NoopCommandBus;
            let hctx = HandlerContext {
                security: &ctx,
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
    ) -> BoxFuture<'_, Result<HealthCheckResponse, GrpcIngressError>> {
        let job = Arc::clone(&self.job);
        Box::pin(async move {
            let check = if job.health_check().await {
                GrpcHealthCheck::healthy()
            } else {
                GrpcHealthCheck::unhealthy(
                    "one or more registered handlers reported unhealthy".to_string(),
                )
            };
            Ok(HealthCheckResponse {
                check: Box::new(check),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;
    use edge_application::{Handler, HandlerError, Request, Response, SecurityContext};
    use edge_application_handler::{
        ExecutionRequest as HandlerExecutionRequest,
        HealthCheckRequest as HandlerHealthCheckRequest,
        HealthCheckResponse as HandlerHealthCheckResponse, IdRequest, IdResponse, PatternRequest,
        PatternResponse,
    };
    use edge_application_observer::StdObserveFactory;
    use swe_edge_ingress_grpc::{GrpcIngressError, GrpcIngressResult, GrpcRequest};

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
                id: "/pkg.Svc/Ping".to_string(),
            })
        }
        fn pattern(&self, _req: PatternRequest) -> Result<PatternResponse, HandlerError> {
            Ok(PatternResponse {
                pattern: "ping".to_string(),
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

    fn decode(_bytes: &[u8]) -> GrpcIngressResult<PingReq> {
        Ok(PingReq)
    }
    fn encode(_resp: &PingResp) -> Vec<u8> {
        b"pong".to_vec()
    }

    fn peer_addr() -> SocketAddr {
        "127.0.0.1:0".parse().expect("valid socket addr")
    }

    fn ingress_with_ping() -> GrpcIngressJob {
        let job = DefaultGrpcJob::new();
        job.register_route(Arc::new(PingHandler), decode, encode)
            .expect("register_route must succeed");
        GrpcIngressJob::new(Arc::new(job), StdObserveFactory::noop_arc_observe_context())
    }

    /// @covers: GrpcIngressJob::handle_unary
    #[tokio::test]
    async fn test_handle_unary_dispatches_registered_route_happy() {
        let ingress = ingress_with_ping();
        let request = GrpcRequest::new("/pkg.Svc/Ping", vec![], Duration::from_secs(1));
        let resp = ingress
            .handle_unary(UnaryRequest::new(
                request,
                SecurityContext::unauthenticated(),
                peer_addr(),
            ))
            .await
            .expect("handle_unary must succeed");
        assert_eq!(resp.body, b"pong".to_vec());
    }

    /// @covers: GrpcIngressJob::handle_unary
    #[tokio::test]
    async fn test_handle_unary_unregistered_method_returns_unimplemented_error() {
        let ingress = ingress_with_ping();
        let request = GrpcRequest::new("/pkg.Svc/Nope", vec![], Duration::from_secs(1));
        let err = ingress
            .handle_unary(UnaryRequest::new(
                request,
                SecurityContext::unauthenticated(),
                peer_addr(),
            ))
            .await
            .unwrap_err();
        assert!(matches!(err, GrpcIngressError::Unimplemented(_)));
    }

    /// @covers: GrpcIngressJob::health_check
    #[tokio::test]
    async fn test_health_check_with_healthy_handler_reports_healthy_happy() {
        let ingress = ingress_with_ping();
        let resp = ingress
            .health_check(HealthCheckRequest::new(peer_addr()))
            .await
            .expect("health_check must succeed");
        assert!(resp.check.healthy);
    }
}
