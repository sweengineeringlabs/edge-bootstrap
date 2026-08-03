//! Integration tests for `ServiceRegistry`'s per-target-service lookup.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use async_trait::async_trait;
use swe_edge_bootstrap_runtime::ServiceRegistry;
use swe_edge_egress_http::{
    ConfigRequest, ConfigResponse, HealthCheckRequest, HttpConfig, HttpEgress, HttpEgressError,
    HttpRequest, HttpResponse, HttpStreamResponse,
};

/// A named stub `HttpEgress` — returns its `name` in the response body so
/// tests can prove *which* registered client actually handled a call.
struct NamedHttpEgressDouble {
    name: &'static str,
}

#[async_trait]
impl HttpEgress for NamedHttpEgressDouble {
    async fn send(&self, _: HttpRequest) -> Result<HttpResponse, HttpEgressError> {
        Ok(HttpResponse::new(200, self.name.as_bytes().to_vec()))
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

fn registry_with_two_services() -> ServiceRegistry {
    ServiceRegistry::new(Arc::new(NamedHttpEgressDouble { name: "default" }), None)
        .with_service(
            "user-service",
            Arc::new(NamedHttpEgressDouble {
                name: "user-service",
            }),
        )
        .with_service(
            "billing-service",
            Arc::new(NamedHttpEgressDouble {
                name: "billing-service",
            }),
        )
}

/// @covers: service — a registered service name resolves to its own client, distinct from the default
#[tokio::test]
async fn test_service_registered_name_returns_its_own_client_happy() {
    let registry = registry_with_two_services();

    let user_client = registry
        .service("user-service")
        .expect("must be registered");
    let resp = user_client
        .send(HttpRequest::get("https://irrelevant.test"))
        .await
        .expect("stub send is infallible");
    assert_eq!(resp.body, b"user-service");

    let billing_client = registry
        .service("billing-service")
        .expect("must be registered");
    let resp = billing_client
        .send(HttpRequest::get("https://irrelevant.test"))
        .await
        .expect("stub send is infallible");
    assert_eq!(resp.body, b"billing-service");
}

/// @covers: service — an unregistered name returns None, not the default client
#[tokio::test]
async fn test_service_unregistered_name_returns_none_error() {
    let registry = registry_with_two_services();
    assert!(registry.service("nonexistent-service").is_none());
}

/// @covers: http — the default client is unaffected by per-service registrations
#[tokio::test]
async fn test_http_default_client_unaffected_by_service_registrations_edge() {
    let registry = registry_with_two_services();
    let resp = registry
        .http()
        .send(HttpRequest::get("https://irrelevant.test"))
        .await
        .expect("stub send is infallible");
    assert_eq!(resp.body, b"default");
}

/// @covers: service_names — reports every registered name, and only those
#[test]
fn test_service_names_reports_all_registered_names_happy() {
    let registry = registry_with_two_services();
    let mut names: Vec<&str> = registry.service_names().collect();
    names.sort_unstable();
    assert_eq!(names, vec!["billing-service", "user-service"]);
}

/// @covers: with_service — re-registering the same name overwrites, not accumulates
#[tokio::test]
async fn test_with_service_same_name_twice_overwrites_edge() {
    let registry = ServiceRegistry::new(Arc::new(NamedHttpEgressDouble { name: "default" }), None)
        .with_service(
            "user-service",
            Arc::new(NamedHttpEgressDouble { name: "first" }),
        )
        .with_service(
            "user-service",
            Arc::new(NamedHttpEgressDouble { name: "second" }),
        );

    assert_eq!(registry.service_names().count(), 1);
    let resp = registry
        .service("user-service")
        .expect("must be registered")
        .send(HttpRequest::get("https://irrelevant.test"))
        .await
        .expect("stub send is infallible");
    assert_eq!(resp.body, b"second");
}
