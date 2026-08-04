//! Integration tests for `LoadBalancedHttpEgress`'s explicit outcome-reporting
//! hook — the equivalent of `edge-transport-http-egress`'s (removed)
//! `breaker` pool-report integration, relocated here. See ADR-004 and
//! edge-transport-http-egress#25 / edge-bootstrap#24.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use async_trait::async_trait;
use swe_edge_bootstrap::{
    BackendConfig, BackendId, LoadBalancedHttpEgress, LoadbalancerConfig, Outcome, Strategy,
};
use swe_edge_egress_http::{
    ConfigRequest, ConfigResponse, HealthCheckRequest, HttpConfig, HttpEgress, HttpEgressError,
    HttpRequest, HttpResponse, HttpStreamResponse,
};

struct NoopHttpEgressDouble;

#[async_trait]
impl HttpEgress for NoopHttpEgressDouble {
    async fn send(&self, _: HttpRequest) -> Result<HttpResponse, HttpEgressError> {
        Ok(HttpResponse::new(200, vec![]))
    }
    async fn send_stream(&self, _: HttpRequest) -> Result<HttpStreamResponse, HttpEgressError> {
        Err(HttpEgressError::Internal("double".into()))
    }
    async fn health_check(&self, _: HealthCheckRequest) -> Result<(), HttpEgressError> {
        Ok(())
    }
    fn config(&self, _: ConfigRequest) -> Result<ConfigResponse, HttpEgressError> {
        Ok(ConfigResponse {
            config: HttpConfig::default(),
        })
    }
}

fn single_backend_client() -> (LoadBalancedHttpEgress, BackendId) {
    let url = "https://only-backend.internal";
    let client = LoadBalancedHttpEgress::new(
        Arc::new(NoopHttpEgressDouble),
        LoadbalancerConfig {
            strategy: Strategy::RoundRobin,
            backends: vec![BackendConfig {
                url: url.to_string(),
                weight: 1,
            }],
        },
    )
    .expect("valid single-backend config must build");
    (client, BackendId::new(url))
}

/// @covers: LoadBalancedHttpEgress::report_outcome — an explicitly reported
/// failure (e.g. an external circuit breaker's trip signal) degrades the
/// backend, and a subsequent `send` on the now-unhealthy sole backend fails —
/// proving eviction actually affects routing, not just internal bookkeeping.
#[tokio::test]
async fn test_report_outcome_explicit_failure_degrades_sole_backend_happy() {
    let (client, backend_id) = single_backend_client();

    // Healthy before any report — the sole backend is selectable.
    client
        .send(HttpRequest::get("https://irrelevant.test"))
        .await
        .expect("sole backend must be selectable while healthy");

    // Simulate an external breaker's trip signal — not derived from a call
    // made through this client at all.
    client.report_outcome(
        &backend_id,
        Outcome::Failure {
            reason: "circuit open".to_string(),
        },
    );

    let result = client
        .send(HttpRequest::get("https://irrelevant.test"))
        .await;
    assert!(
        result.is_err(),
        "sole backend must be unselectable immediately after an explicit failure report"
    );
}

/// @covers: LoadBalancedHttpEgress::report_outcome — an explicitly reported
/// recovery (e.g. an external circuit breaker's half-open probe success)
/// restores a previously-degraded sole backend to selectable.
#[tokio::test]
async fn test_report_outcome_explicit_success_restores_degraded_backend_happy() {
    let (client, backend_id) = single_backend_client();

    client.report_outcome(
        &backend_id,
        Outcome::Failure {
            reason: "circuit open".to_string(),
        },
    );
    assert!(
        client
            .send(HttpRequest::get("https://irrelevant.test"))
            .await
            .is_err(),
        "sanity check: backend must be degraded before the recovery report"
    );

    // Simulate an external breaker's half-open-probe-succeeded signal.
    client.report_outcome(&backend_id, Outcome::Success);

    client
        .send(HttpRequest::get("https://irrelevant.test"))
        .await
        .expect("backend must be selectable again after an explicit recovery report");
}

/// @covers: LoadBalancedHttpEgress::backend_count — reports the configured
/// backend count, independent of health state.
#[test]
fn test_backend_count_reports_configured_backend_count_happy() {
    let (client, _) = single_backend_client();
    assert_eq!(client.backend_count(), 1);
}
