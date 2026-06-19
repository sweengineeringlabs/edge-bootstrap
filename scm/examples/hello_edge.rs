//! Hello Edge — end-to-end provider handler running inside the runtime.
//!
//! Run:
//!     cargo run -p swe-edge-bootstrap --example hello_edge
//!
//! Demonstrates:
//!   1. Registering edge-llm-provider's provider_handler in a HandlerRegistry
//!   2. Wiring a DispatchJob (Router → Registry → Handler) into an HttpIngress
//!   3. Booting the RuntimeManager and verifying the lifecycle

use std::sync::Arc;

use edge_domain::{Domain, HandlerContext, HandlerRegistry, NoopCommandBus};
use edge_llm_provider::{ExecutionConfig, ExecutionMode, ExecutionStepResult, StdProviderFactory};
use edge_proxy::{Job, JobError, ProxySvc, Router, RoutingError};
use futures::future::BoxFuture;
use swe_edge_bootstrap::{
    HttpBody, HttpEgress, HttpEgressResult, HttpIngress, HttpIngressError, HttpIngressResult,
    HttpHealthCheck, HttpRequest, HttpResponse, Runtime, RuntimeConfig, RuntimeManager,
    RuntimeStatus, SecurityContext,
};
use swe_edge_egress_http::{HttpRequest as EgressReq, HttpResponse as EgressResp, HttpStreamResponse};

// ── Router: every request goes to the provider handler ───────────────────────

struct ProviderRouter;

impl Router<String> for ProviderRouter {
    fn route<'a>(&'a self, _input: &'a str) -> BoxFuture<'a, Result<String, RoutingError>> {
        Box::pin(async { Ok("provider.execute_step".to_string()) })
    }
}

// ── DispatchJob ───────────────────────────────────────────────────────────────

struct ProviderJob {
    router:   Arc<dyn Router<String>>,
    registry: Arc<dyn HandlerRegistry<Request = String, Response = ExecutionStepResult>>,
}

impl Job<String, ExecutionStepResult> for ProviderJob {
    fn run<'a>(
        &'a self,
        req: String,
        ctx: HandlerContext<'a>,
    ) -> BoxFuture<'a, Result<ExecutionStepResult, JobError>> {
        let router   = self.router.clone();
        let registry = self.registry.clone();
        Box::pin(async move {
            let id  = router.route(&req).await?;
            let err = JobError::HandlerUnavailable(id.clone());
            let h   = registry.get(&id).ok_or(err)?;
            Ok(h.execute(req, ctx).await?)
        })
    }
}

// ── HttpIngress ───────────────────────────────────────────────────────────────

struct ProviderIngress {
    job: Arc<dyn Job<String, ExecutionStepResult>>,
}

impl HttpIngress for ProviderIngress {
    fn handle(
        &self,
        req: HttpRequest,
        ctx: SecurityContext,
    ) -> BoxFuture<'_, HttpIngressResult<HttpResponse>> {
        let job = self.job.clone();
        Box::pin(async move {
            let payload = match req.body {
                Some(HttpBody::Raw(bytes)) => String::from_utf8_lossy(&bytes).to_string(),
                Some(HttpBody::Json(v))    => v.to_string(),
                _                          => "hello edge".to_string(),
            };
            let bus  = NoopCommandBus;
            let hctx = HandlerContext::new(&ctx, &bus);
            let result = job.run(payload, hctx).await
                .map_err(|e| HttpIngressError::Internal(e.to_string()))?;
            let json = serde_json::to_vec(&result)
                .map_err(|e| HttpIngressError::Internal(e.to_string()))?;
            Ok(HttpResponse::new(200, json))
        })
    }

    fn health_check(&self) -> BoxFuture<'_, HttpIngressResult<HttpHealthCheck>> {
        Box::pin(async { Ok(HttpHealthCheck::healthy()) })
    }
}

// ── NoopEgress ────────────────────────────────────────────────────────────────

struct NoopEgress;

impl HttpEgress for NoopEgress {
    fn send(&self, _: EgressReq) -> BoxFuture<'_, HttpEgressResult<EgressResp>> {
        Box::pin(async { Ok(EgressResp::new(200, vec![])) })
    }

    fn send_stream(&self, _: EgressReq) -> BoxFuture<'_, HttpEgressResult<HttpStreamResponse>> {
        Box::pin(async {
            Ok(HttpStreamResponse {
                status:  200,
                headers: Default::default(),
                body:    Box::pin(futures::stream::empty()),
            })
        })
    }

    fn health_check(&self) -> BoxFuture<'_, HttpEgressResult<()>> {
        Box::pin(async { Ok(()) })
    }
}

// ── main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Domain: build and register the provider handler.
    let exec_config = ExecutionConfig::new(4096, 30_000, true, false, ExecutionMode::Async);
    let handler     = StdProviderFactory::default_provider_handler(exec_config);
    let registry    = Domain::new_handler_registry::<String, ExecutionStepResult>();
    registry.register(Arc::new(handler));

    // 2. Smoke-test dispatch before booting the runtime.
    let h        = registry.get("provider.execute_step").expect("provider handler registered");
    let security = SecurityContext::unauthenticated();
    let bus      = NoopCommandBus;
    let hctx     = HandlerContext::new(&security, &bus);
    let result   = h.execute("hello edge".to_string(), hctx).await?;
    println!("dispatch smoke-test → reasoning: {}", result.reasoning);

    // 3. Proxy: wire router + registry into a dispatch job.
    let job: Arc<dyn Job<String, ExecutionStepResult>> = Arc::new(ProviderJob {
        router:   Arc::new(ProviderRouter),
        registry: registry.clone(),
    });

    // 4. Assemble ingress / egress / lifecycle.
    let ingress   = Arc::new(Runtime::http_ingress(Arc::new(ProviderIngress { job })));
    let egress    = Arc::new(Runtime::http_egress(Arc::new(NoopEgress)));
    let lifecycle = ProxySvc::new_null_lifecycle_monitor();

    // 5. Boot the RuntimeManager: start → health → shutdown.
    let config = RuntimeConfig::default()
        .with_service_name("hello-edge")
        .with_http_bind("127.0.0.1:0")
        .with_shutdown_timeout(5);

    let mgr = Runtime::runtime_manager(config, ingress, egress, lifecycle);
    mgr.start().await?;

    let health = mgr.health().await;
    println!("runtime status: {:?}", health.status);
    assert_eq!(health.status, RuntimeStatus::Running);

    mgr.shutdown().await?;
    println!("hello edge stopped cleanly");

    Ok(())
}
