//! Hello Edge — end-to-end echo handler running inside the runtime.
//!
//! Run:
//!     cargo run -p swe-edge-bootstrap --example hello_edge
//!
//! Demonstrates:
//!   1. Registering a `Handler` in a `HandlerRegistry` (via `Domain::echo_handler`)
//!   2. Looking the handler up by id and dispatching to it from an `HttpIngress`
//!   3. Booting the `RuntimeManager` (with `edge-proxy`'s lifecycle monitor) and
//!      verifying the lifecycle
//!
//! This previously routed through `edge-proxy`'s `Job`/`Router` traits and wired up
//! `edge-llm-provider`'s provider handler directly. Both no longer fit: `edge-proxy`'s
//! `Job`/`Router` still key off a separate, unmigrated `edge-domain` package (not
//! `edge-application`), so its `HandlerContext`/`ObserverContext` types don't unify with
//! the ones used here; and `edge-llm-provider`'s handler-construction API
//! (`StdProviderFactory::provider_handler`, requiring a full `ExecutionModel` impl)
//! evolved independently of the wiring shape this example is meant to teach. Dispatch is
//! inlined directly against the `HandlerRegistry` instead, and a self-contained echo
//! handler stands in for the provider handler. `edge-proxy`'s `ProxySvc` lifecycle monitor
//! is still used as-is, since that part has no dependency on `Handler`/`HandlerContext`.

use std::sync::Arc;

use edge_application::{
    DirectCommandBus, Domain, HandlerContext, HandlerRegistry as HandlerRegistryTrait,
};
use edge_application_handler::{
    HandlerLookupRequest, ObserverContextAdapter, RegisterHandlerRequest,
};
use edge_application_observer::StdObserveFactory;
use edge_proxy::ProxySvc;
use swe_edge_bootstrap::{
    HealthCheckRequest, HealthCheckResponse, HttpBody, HttpFuture, HttpHealthCheck, HttpIngress,
    HttpIngressError, HttpResponse, InboundRequest, Runtime, RuntimeConfig, RuntimeManager,
    RuntimeStatus, SecurityContext,
};
use swe_edge_egress_http::{
    ConfigRequest, ConfigResponse, HealthCheckRequest as EgressHealthCheckRequest, HttpByteStream,
    HttpConfig, HttpEgress, HttpEgressError, HttpRequest as EgressReq, HttpResponse as EgressResp,
    HttpStreamResponse,
};

const HANDLER_ID: &str = "echo";

/// Payload carried through the registry → handler chain.
///
/// A local type is required because `String` doesn't implement the domain
/// `Request`/`Response` marker traits (orphan rule).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct EchoPayload(String);
impl edge_application::Request for EchoPayload {}
impl edge_application::Response for EchoPayload {}

// ── HttpIngress ───────────────────────────────────────────────────────────────

struct EchoIngress {
    registry: Arc<dyn HandlerRegistryTrait<Request = EchoPayload, Response = EchoPayload>>,
}

impl HttpIngress for EchoIngress {
    fn handle(
        &self,
        req: InboundRequest,
    ) -> HttpFuture<'_, Result<HttpResponse, HttpIngressError>> {
        let registry = self.registry.clone();
        HttpFuture::new(async move {
            let payload = match req.request.body {
                Some(HttpBody::Raw(bytes)) => String::from_utf8_lossy(&bytes).to_string(),
                Some(HttpBody::Json(v)) => v.to_string(),
                _ => "hello edge".to_string(),
            };
            let handler = registry
                .get(HandlerLookupRequest {
                    id: HANDLER_ID.to_string(),
                })
                .map_err(|e| HttpIngressError::Internal(e.to_string()))?
                .handler
                .ok_or_else(|| HttpIngressError::Internal("echo handler not registered".into()))?;
            let security = SecurityContext::unauthenticated();
            let bus = DirectCommandBus;
            let observer = StdObserveFactory::noop_observer_context();
            let observer_adapter = ObserverContextAdapter(observer.as_ref());
            let hctx = HandlerContext {
                security: &security,
                commands: &bus,
                observer: &observer_adapter,
            };
            let result = handler
                .execute(edge_application_handler::ExecutionRequest {
                    req: EchoPayload(payload),
                    ctx: &hctx,
                })
                .await
                .map_err(|e| HttpIngressError::Internal(e.to_string()))?;
            let json = serde_json::to_vec(&result)
                .map_err(|e| HttpIngressError::Internal(e.to_string()))?;
            Ok(HttpResponse::new(200, json))
        })
    }

    fn health_check(
        &self,
        _req: HealthCheckRequest,
    ) -> HttpFuture<'_, Result<HealthCheckResponse, HttpIngressError>> {
        HttpFuture::new(async {
            Ok(HealthCheckResponse {
                health: HttpHealthCheck::healthy(),
            })
        })
    }
}

// ── NoopEgress ────────────────────────────────────────────────────────────────

struct NoopEgress;

#[async_trait::async_trait]
impl HttpEgress for NoopEgress {
    async fn send(&self, _: EgressReq) -> Result<EgressResp, HttpEgressError> {
        Ok(EgressResp::new(200, vec![]))
    }

    async fn send_stream(&self, _: EgressReq) -> Result<HttpStreamResponse, HttpEgressError> {
        Ok(HttpStreamResponse {
            status: 200,
            headers: Default::default(),
            body: HttpByteStream::new(futures::stream::empty()),
        })
    }

    async fn health_check(&self, _: EgressHealthCheckRequest) -> Result<(), HttpEgressError> {
        Ok(())
    }

    fn config(&self, _: ConfigRequest) -> Result<ConfigResponse, HttpEgressError> {
        Ok(ConfigResponse {
            config: HttpConfig::default(),
        })
    }
}

// ── main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Domain: build and register the echo handler.
    let handler = Domain.echo_handler::<EchoPayload>(HANDLER_ID, "/echo");
    let registry = Domain.new_handler_registry::<EchoPayload, EchoPayload>();
    registry.register(RegisterHandlerRequest::new(handler))?;

    // 2. Smoke-test dispatch before booting the runtime.
    let looked_up = registry
        .get(HandlerLookupRequest {
            id: HANDLER_ID.to_string(),
        })?
        .handler
        .ok_or("echo handler not registered")?;
    let security = SecurityContext::unauthenticated();
    let bus = DirectCommandBus;
    let observer = StdObserveFactory::noop_observer_context();
    let observer_adapter = ObserverContextAdapter(observer.as_ref());
    let hctx = HandlerContext {
        security: &security,
        commands: &bus,
        observer: &observer_adapter,
    };
    let result = looked_up
        .execute(edge_application_handler::ExecutionRequest {
            req: EchoPayload("hello edge".to_string()),
            ctx: &hctx,
        })
        .await?;
    println!("dispatch smoke-test → echoed: {}", result.0);

    // 3. Assemble ingress / egress / lifecycle.
    let ingress = Arc::new(Runtime::http_ingress(Arc::new(EchoIngress { registry })));
    let egress = Arc::new(Runtime::http_egress(Arc::new(NoopEgress)));
    let lifecycle = ProxySvc::new_null_lifecycle_monitor();

    // 4. Boot the RuntimeManager: start → health → shutdown.
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
