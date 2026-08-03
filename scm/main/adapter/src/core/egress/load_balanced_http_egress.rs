//! `LoadBalancedHttpEgress` — an [`HttpEgress`] backed by a `swe-edge-loadbalancer`
//! backend pool for one target service.
//!
//! Backend-pool ownership lives here, at the process composition root, not
//! inside `edge-transport-http-egress`'s `transport` crate — see ADR-004.

use async_trait::async_trait;
use std::sync::Arc;
use swe_edge_egress_http::{
    ConfigRequest, ConfigResponse, HealthCheckRequest, HttpEgress, HttpEgressError, HttpRequest,
    HttpResponse, HttpStreamResponse,
};
use swe_edge_loadbalancer::{
    BackendId, BackendPoolInstance, LoadbalancerConfig, LoadbalancerSvc, Outcome,
};

/// An [`HttpEgress`] that load-balances across a pool of backend URLs for
/// one target service.
///
/// Delegates the actual HTTP call to `inner` — a plain, single-destination
/// client — after rewriting the request's URL to a backend selected from
/// the pool (round-robin/weighted/least-connections, per the pool's
/// configured [`Strategy`](swe_edge_loadbalancer::Strategy)).
///
/// Reports `Outcome::Success`/`Outcome::Failure` back to the pool after
/// every call automatically (a 5xx response or a transport error degrades
/// the selected backend; anything else keeps it healthy). For an outcome
/// signal that isn't derived from a single call — e.g. an external circuit
/// breaker's trip or recovery — call [`LoadBalancedHttpEgress::report_outcome`]
/// directly.
pub struct LoadBalancedHttpEgress {
    inner: Arc<dyn HttpEgress>,
    pool: Arc<BackendPoolInstance>,
}

/// Failure constructing a [`LoadBalancedHttpEgress`].
#[derive(Debug, thiserror::Error)]
pub enum LoadBalancedHttpEgressError {
    /// The supplied [`LoadbalancerConfig`] failed validation (e.g. an empty
    /// backend list, or a backend with an empty URL or zero weight).
    #[error("invalid load balancer config: {0}")]
    InvalidConfig(String),
}

impl LoadBalancedHttpEgress {
    /// Build a load-balanced client from a backend-pool config, delegating
    /// every actual HTTP call to `inner`.
    ///
    /// # Errors
    ///
    /// Returns [`LoadBalancedHttpEgressError::InvalidConfig`] if `config`
    /// has an empty backend list, or any backend has an empty URL or zero
    /// weight.
    pub fn new(
        inner: Arc<dyn HttpEgress>,
        config: LoadbalancerConfig,
    ) -> Result<Self, LoadBalancedHttpEgressError> {
        let pool = LoadbalancerSvc::build_pool(config)
            .map_err(|e| LoadBalancedHttpEgressError::InvalidConfig(e.to_string()))?;
        Ok(Self {
            inner,
            pool: Arc::new(pool),
        })
    }

    /// Report an outcome for `backend_id` directly, independent of this
    /// client's own per-call reporting in [`HttpEgress::send`].
    ///
    /// Intended for a signal not derived from a single HTTP call — e.g. an
    /// external circuit breaker reporting `Outcome::Failure` on trip (so
    /// the backend is evicted from rotation) and `Outcome::Success` on
    /// recovery (so it's restored).
    pub fn report_outcome(&self, backend_id: &BackendId, outcome: Outcome) {
        LoadbalancerSvc::report_outcome(&self.pool, backend_id, outcome);
    }

    /// Number of backends registered in the pool, healthy or not.
    pub fn backend_count(&self) -> usize {
        LoadbalancerSvc::backend_count(&self.pool)
    }

    /// Rewrite `orig`'s path/query/fragment onto `backend_url`'s origin,
    /// mirroring the URL-rewrite `edge-transport-http-egress`'s own
    /// (now-removed) loadbalancer middleware used to perform.
    fn rewrite_url(orig: &str, backend_url: &str) -> Result<String, HttpEgressError> {
        let mut base = url::Url::parse(backend_url)
            .map_err(|e| HttpEgressError::Internal(format!("invalid backend URL: {e}")))?;
        let orig_url = url::Url::parse(orig)
            .map_err(|e| HttpEgressError::Internal(format!("invalid request URL: {e}")))?;
        base.set_path(orig_url.path());
        base.set_query(orig_url.query());
        base.set_fragment(orig_url.fragment());
        Ok(base.into())
    }
}

#[async_trait]
impl HttpEgress for LoadBalancedHttpEgress {
    async fn send(&self, mut request: HttpRequest) -> Result<HttpResponse, HttpEgressError> {
        let backend = LoadbalancerSvc::select(&self.pool)
            .map_err(|e| HttpEgressError::Internal(e.to_string()))?;
        request.url = Self::rewrite_url(&request.url, &backend.url)?;

        let result = self.inner.send(request).await;
        let outcome = match &result {
            Ok(resp) if resp.is_server_error() => Outcome::Failure {
                reason: format!("status {}", resp.status),
            },
            Ok(_) => Outcome::Success,
            Err(e) => Outcome::Failure {
                reason: e.to_string(),
            },
        };
        self.report_outcome(&backend.id, outcome);
        result
    }

    async fn send_stream(
        &self,
        mut request: HttpRequest,
    ) -> Result<HttpStreamResponse, HttpEgressError> {
        let backend = LoadbalancerSvc::select(&self.pool)
            .map_err(|e| HttpEgressError::Internal(e.to_string()))?;
        request.url = Self::rewrite_url(&request.url, &backend.url)?;

        // A stream's status isn't known synchronously the way `send`'s is —
        // report success on a successful connection; stream-level failures
        // after that are the caller's concern, not this pool's.
        let result = self.inner.send_stream(request).await;
        let outcome = match &result {
            Ok(_) => Outcome::Success,
            Err(e) => Outcome::Failure {
                reason: e.to_string(),
            },
        };
        self.report_outcome(&backend.id, outcome);
        result
    }

    async fn health_check(&self, request: HealthCheckRequest) -> Result<(), HttpEgressError> {
        self.inner.health_check(request).await
    }

    fn config(&self, request: ConfigRequest) -> Result<ConfigResponse, HttpEgressError> {
        self.inner.config(request)
    }
}
