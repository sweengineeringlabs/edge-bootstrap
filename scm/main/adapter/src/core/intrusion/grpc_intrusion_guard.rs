use std::sync::Arc;
use std::time::Instant;

use edge_intrusion::config::Wired;
use edge_intrusion_enforcer::{Decision, Enforcer};
use edge_intrusion_rule::{Identity, RequestEvent};
use futures::future::BoxFuture;
use swe_edge_ingress_grpc::{
    GrpcIngress, GrpcIngressError, GrpcResponse, HealthCheckRequest, HealthCheckResponse,
    StreamRequest, StreamResponse, UnaryRequest,
};

/// Wraps a `GrpcIngress` handler; rejects calls `edge-intrusion`'s
/// configured rules engine flags, before delegating to the wrapped handler.
///
/// See [`super::HttpIntrusionGuard`] for the shared design notes (detection
/// and enforcement stay in `edge-intrusion`; this wrapper only builds a
/// `RequestEvent` and honors the resulting `Decision`).
///
/// Neither unary bodies nor stream messages are inspected — only method,
/// peer address, and metadata (headers) feed the rules engine. `handle_stream`
/// builds its event from the request's metadata/method/peer_addr without
/// reading the message stream, so body-based signature matching does not
/// see streamed payloads — a real, documented gap, not an oversight.
pub(crate) struct GrpcIntrusionGuard {
    inner: Arc<dyn GrpcIngress>,
    wired: Arc<Wired>,
}

impl GrpcIntrusionGuard {
    pub(crate) fn new(inner: Arc<dyn GrpcIngress>, wired: Arc<Wired>) -> Self {
        Self { inner, wired }
    }

    fn build_event(
        &self,
        method: &str,
        peer_addr: std::net::SocketAddr,
        headers: &std::collections::HashMap<String, String>,
    ) -> RequestEvent {
        let forwarded_for = headers.get("x-forwarded-for").cloned();
        let ip = self
            .wired
            .identity_extractor
            .extract_ip(peer_addr.ip(), forwarded_for.as_deref());

        let mut event = RequestEvent::new(Identity::Ip(ip), method, method, Instant::now());
        for (name, value) in headers {
            event = event.with_header(name.clone(), value.clone());
        }
        event
    }
}

impl swe_edge_bootstrap_intrusion::GrpcIntrusionGuard for GrpcIntrusionGuard {}

impl GrpcIngress for GrpcIntrusionGuard {
    fn handle_unary(
        &self,
        req: UnaryRequest,
    ) -> BoxFuture<'_, Result<GrpcResponse, GrpcIngressError>> {
        let event = self.build_event(&req.request.method, req.peer_addr, &req.request.metadata.headers);
        match self.wired.enforcer.guard(&event) {
            Decision::Reject(verdict) => {
                tracing::info!(
                    node = "grpc_intrusion_guard",
                    decision = "reject",
                    rule_id = %verdict.rule_id,
                    reason = %verdict.reason,
                    "unary call rejected before reaching the handler"
                );
                Box::pin(async move {
                    Err(GrpcIngressError::PermissionDenied(format!(
                        "blocked by edge-intrusion rule '{}': {}",
                        verdict.rule_id, verdict.reason
                    )))
                })
            }
            Decision::Allow => {
                tracing::debug!(node = "grpc_intrusion_guard", decision = "allow", "unary call allowed through");
                self.inner.handle_unary(req)
            }
        }
    }

    fn handle_stream(
        &self,
        req: StreamRequest,
    ) -> BoxFuture<'_, Result<StreamResponse, GrpcIngressError>> {
        let event = self.build_event(&req.method, req.peer_addr, &req.metadata.headers);
        match self.wired.enforcer.guard(&event) {
            Decision::Reject(verdict) => Box::pin(async move {
                Err(GrpcIngressError::PermissionDenied(format!(
                    "blocked by edge-intrusion rule '{}': {}",
                    verdict.rule_id, verdict.reason
                )))
            }),
            Decision::Allow => self.inner.handle_stream(req),
        }
    }

    fn health_check(
        &self,
        req: HealthCheckRequest,
    ) -> BoxFuture<'_, Result<HealthCheckResponse, GrpcIngressError>> {
        self.inner.health_check(req)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use edge_intrusion::config::Config;
    use edge_domain::SecurityContext;
    use swe_edge_ingress_grpc::{GrpcHealthCheck, GrpcMetadataInner, GrpcRequest};

    fn wired_with_toml(toml: &str) -> Arc<Wired> {
        let cfg = match Config::from_toml_str(toml) {
            Ok(cfg) => cfg,
            Err(e) => panic!("test fixture TOML must parse: {e}"),
        };
        match cfg.build() {
            Ok(w) => Arc::new(w),
            Err(e) => panic!("test fixture config must build: {e}"),
        }
    }

    fn unary(method: &str, peer_ip: [u8; 4]) -> UnaryRequest {
        UnaryRequest::new(
            GrpcRequest::new(method, vec![], std::time::Duration::from_secs(5)),
            SecurityContext::unauthenticated(),
            std::net::SocketAddr::from((peer_ip, 0)),
        )
    }

    struct AlwaysOkInner;
    impl GrpcIngress for AlwaysOkInner {
        fn handle_unary(
            &self,
            _req: UnaryRequest,
        ) -> BoxFuture<'_, Result<GrpcResponse, GrpcIngressError>> {
            Box::pin(async {
                Ok(GrpcResponse {
                    body: vec![],
                    metadata: Arc::new(GrpcMetadataInner::default()),
                })
            })
        }
        fn health_check(
            &self,
            _req: HealthCheckRequest,
        ) -> BoxFuture<'_, Result<HealthCheckResponse, GrpcIngressError>> {
            Box::pin(async {
                Ok(HealthCheckResponse {
                    check: Box::new(GrpcHealthCheck::healthy()),
                })
            })
        }
    }

    #[tokio::test]
    async fn test_handle_unary_allows_clean_call_through_to_inner() {
        let guard = GrpcIntrusionGuard::new(Arc::new(AlwaysOkInner), wired_with_toml(""));
        let resp = guard
            .handle_unary(unary("/my.Service/Method", [203, 0, 113, 5]))
            .await;
        assert!(resp.is_ok());
    }

    #[tokio::test]
    async fn test_handle_unary_rejects_reputation_denylisted_peer_without_calling_inner() {
        let wired = wired_with_toml(
            r#"
                signature_rules = { baseline_enabled = false }

                [[reputation_rules]]
                id = "deny-bad-actor"
                blocks = ["203.0.113.0/24"]
                verdict_on = "denylisted"
                severity = "high"
            "#,
        );
        let guard = GrpcIntrusionGuard::new(Arc::new(AlwaysOkInner), wired);
        let resp = guard
            .handle_unary(unary("/my.Service/Method", [203, 0, 113, 5]))
            .await;
        match resp {
            Err(GrpcIngressError::PermissionDenied(msg)) => {
                assert!(msg.contains("deny-bad-actor"), "unexpected message: {msg}");
            }
            other => panic!("expected PermissionDenied, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_handle_unary_allows_non_denylisted_peer() {
        let wired = wired_with_toml(
            r#"
                signature_rules = { baseline_enabled = false }

                [[reputation_rules]]
                id = "deny-bad-actor"
                blocks = ["203.0.113.0/24"]
                verdict_on = "denylisted"
                severity = "high"
            "#,
        );
        let guard = GrpcIntrusionGuard::new(Arc::new(AlwaysOkInner), wired);
        let resp = guard
            .handle_unary(unary("/my.Service/Method", [198, 51, 100, 9]))
            .await;
        assert!(resp.is_ok());
    }

    #[tokio::test]
    async fn test_health_check_delegates_to_inner_without_enforcement() {
        let guard = GrpcIntrusionGuard::new(Arc::new(AlwaysOkInner), wired_with_toml(""));
        let hc = guard
            .health_check(HealthCheckRequest::new(std::net::SocketAddr::from((
                [127, 0, 0, 1],
                0,
            ))))
            .await;
        assert!(hc.unwrap().check.healthy);
    }
}
