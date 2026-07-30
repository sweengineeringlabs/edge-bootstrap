use std::sync::Arc;
use std::time::Instant;

use edge_intrusion::config::Wired;
use edge_intrusion_enforcer::{Decision, Enforcer};
use edge_intrusion_rule::{Identity, RequestEvent};
use swe_edge_ingress_http::{
    HealthCheckRequest, HealthCheckResponse, HttpFuture, HttpIngress, HttpIngressError,
    HttpResponse, InboundRequest,
};

/// Wraps an `HttpIngress` handler; rejects requests `edge-intrusion`'s
/// configured rules engine flags, before delegating to the wrapped handler.
///
/// Detection and enforcement themselves are entirely `edge-intrusion`'s
/// responsibility (including the fail-open guarantee — see that repo's
/// ADR-002); this wrapper only builds the `RequestEvent` from what's
/// available on an `InboundRequest` and honors the resulting `Decision`.
///
/// Request bodies are not inspected — only path, query, method, and
/// headers feed the rules engine. Body-based signature matching is a real,
/// documented gap (not wired here), consistent with `edge-intrusion`'s own
/// `body_excerpt` field being optional.
pub(crate) struct HttpIntrusionGuard {
    inner: Arc<dyn HttpIngress>,
    wired: Arc<Wired>,
}

impl HttpIntrusionGuard {
    pub(crate) fn new(inner: Arc<dyn HttpIngress>, wired: Arc<Wired>) -> Self {
        Self { inner, wired }
    }

    fn build_event(&self, req: &InboundRequest) -> RequestEvent {
        let forwarded_for = req.request.header("x-forwarded-for").map(str::to_owned);
        let ip = self
            .wired
            .identity_extractor
            .extract_ip(req.peer_addr.ip(), forwarded_for.as_deref());

        // Deliberately NOT split off the query string here: `path`-targeted
        // signature rules (e.g. `lfi-etc-passwd`) only check the `path`
        // field, and path-traversal/LFI payloads commonly arrive in a query
        // parameter (`?file=/etc/passwd`) — splitting them apart would blind
        // those rules to that attack shape. `edge-intrusion`'s own tests
        // feed the combined request-target the same way. `.with_query` is
        // still set from the raw suffix, additively, for any rule that
        // wants it specifically.
        let query = req.request.url.split_once('?').map(|(_, q)| q.to_string());

        let mut event = RequestEvent::new(
            Identity::Ip(ip),
            req.request.method.to_string(),
            req.request.url.clone(),
            Instant::now(),
        );
        if let Some(q) = query {
            event = event.with_query(q);
        }
        for (name, value) in &req.request.headers {
            event = event.with_header(name.clone(), value.clone());
        }
        event
    }
}

impl swe_edge_bootstrap_intrusion::HttpIntrusionGuard for HttpIntrusionGuard {}

impl HttpIngress for HttpIntrusionGuard {
    fn handle(
        &self,
        req: InboundRequest,
    ) -> HttpFuture<'_, Result<HttpResponse, HttpIngressError>> {
        let event = self.build_event(&req);
        match self.wired.enforcer.guard(&event) {
            Decision::Reject(verdict) => {
                tracing::info!(
                    node = "http_intrusion_guard",
                    decision = "reject",
                    rule_id = %verdict.rule_id,
                    reason = %verdict.reason,
                    "request rejected before reaching the handler"
                );
                HttpFuture::new(async move {
                    Err(HttpIngressError::PermissionDenied(format!(
                        "blocked by edge-intrusion rule '{}': {}",
                        verdict.rule_id, verdict.reason
                    )))
                })
            }
            Decision::Allow => {
                tracing::debug!(node = "http_intrusion_guard", decision = "allow", "request allowed through");
                self.inner.handle(req)
            }
        }
    }

    fn health_check(
        &self,
        req: HealthCheckRequest,
    ) -> HttpFuture<'_, Result<HealthCheckResponse, HttpIngressError>> {
        self.inner.health_check(req)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use edge_intrusion::config::Config;
    use swe_edge_ingress_http::{HttpHealthCheck, HttpRequest, RequestContext};

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

    fn inbound(url: &str, peer_ip: [u8; 4]) -> InboundRequest {
        InboundRequest::new(
            HttpRequest::get(url),
            RequestContext::new(edge_domain::SecurityContext::unauthenticated()),
            std::net::SocketAddr::from((peer_ip, 0)),
        )
    }

    struct AlwaysOkInner;
    impl HttpIngress for AlwaysOkInner {
        fn handle(
            &self,
            _req: InboundRequest,
        ) -> HttpFuture<'_, Result<HttpResponse, HttpIngressError>> {
            HttpFuture::new(async { Ok(HttpResponse::new(200, vec![])) })
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

    #[tokio::test]
    async fn test_handle_allows_clean_request_through_to_inner() {
        // Empty config = baseline signature rules only; a clean GET / never matches.
        let guard = HttpIntrusionGuard::new(Arc::new(AlwaysOkInner), wired_with_toml(""));
        let resp = guard.handle(inbound("/", [203, 0, 113, 5])).await;
        assert_eq!(resp.unwrap().status, 200);
    }

    #[tokio::test]
    async fn test_handle_rejects_request_matching_a_signature_rule_without_calling_inner() {
        let guard = HttpIntrusionGuard::new(Arc::new(AlwaysOkInner), wired_with_toml(""));
        let resp = guard
            .handle(inbound("/download?file=/etc/passwd", [203, 0, 113, 5]))
            .await;
        match resp {
            Err(HttpIngressError::PermissionDenied(msg)) => {
                assert!(msg.contains("lfi-etc-passwd"), "unexpected message: {msg}");
            }
            other => panic!("expected PermissionDenied, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_handle_uses_peer_addr_as_identity_for_threshold_rules() {
        let wired = wired_with_toml(
            r#"
                signature_rules = { baseline_enabled = false }

                [[threshold_rules]]
                id = "login-threshold"
                predicate = { kind = "ResponseStatusIn", codes = [401] }
                max_count = 1
                window_secs = 60
                severity = "high"
            "#,
        );
        // ThresholdRule keys off Identity — reachable only via peer_addr here.
        // We can't easily produce a 401 through AlwaysOkInner's 200 response,
        // so this test proves identity plumbing indirectly: two distinct peer
        // IPs must not share a single ThresholdRule counter. Evaluate the
        // engine directly through the same event-building path the guard uses.
        let guard = HttpIntrusionGuard::new(Arc::new(AlwaysOkInner), Arc::clone(&wired));
        let event_a = guard.build_event(&inbound("/login", [203, 0, 113, 5]));
        let event_b = guard.build_event(&inbound("/login", [203, 0, 113, 9]));
        assert_ne!(event_a.identity, event_b.identity);
        assert_eq!(
            event_a.identity,
            Identity::Ip("203.0.113.5".parse().unwrap())
        );
    }

    #[tokio::test]
    async fn test_health_check_delegates_to_inner_without_enforcement() {
        let guard = HttpIntrusionGuard::new(Arc::new(AlwaysOkInner), wired_with_toml(""));
        let hc = guard
            .health_check(HealthCheckRequest::new(std::net::SocketAddr::from((
                [127, 0, 0, 1],
                0,
            ))))
            .await;
        assert!(hc.unwrap().health.healthy);
    }
}
