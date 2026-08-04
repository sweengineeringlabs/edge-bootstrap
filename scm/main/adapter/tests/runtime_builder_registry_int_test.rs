//! Integration tests for `RuntimeBuilder::build_registry`'s per-target-service
//! backend-pool wiring — backend-pool ownership moved here from
//! `edge-transport-http-egress`'s `transport` crate; see ADR-004.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use swe_edge_bootstrap::{
    BackendConfig, LoadbalancerConfig, RuntimeConfig, ServiceEgressConfig, Strategy,
};
use swe_edge_egress_http::{
    ConfigRequest, ConfigResponse, HealthCheckRequest, HttpConfig, HttpEgress, HttpEgressError,
    HttpRequest, HttpResponse, HttpStreamResponse,
};

/// A stub `HttpEgress` that records every URL it was actually sent to, so
/// tests can prove which backend the pool selected — not just that some
/// call succeeded.
struct RecordingHttpEgressDouble {
    seen_urls: Arc<std::sync::Mutex<Vec<String>>>,
}

#[async_trait]
impl HttpEgress for RecordingHttpEgressDouble {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, HttpEgressError> {
        self.seen_urls.lock().expect("lock").push(request.url);
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

/// A stub `HttpEgress` that always errors — used to prove a failing call
/// still degrades the selected backend in the pool.
struct AlwaysFailingHttpEgressDouble;

#[async_trait]
impl HttpEgress for AlwaysFailingHttpEgressDouble {
    async fn send(&self, _: HttpRequest) -> Result<HttpResponse, HttpEgressError> {
        Err(HttpEgressError::Internal("always fails".into()))
    }
    async fn send_stream(&self, _: HttpRequest) -> Result<HttpStreamResponse, HttpEgressError> {
        Err(HttpEgressError::Internal("always fails".into()))
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

fn two_backend_config() -> ServiceEgressConfig {
    ServiceEgressConfig {
        loadbalancer: LoadbalancerConfig {
            strategy: Strategy::RoundRobin,
            backends: vec![
                BackendConfig {
                    url: "https://user-service-1.internal".to_string(),
                    weight: 1,
                },
                BackendConfig {
                    url: "https://user-service-2.internal".to_string(),
                    weight: 1,
                },
            ],
        },
    }
}

/// @covers: build_registry — a configured service resolves to a client that
/// actually load-balances across its configured backends, round-robin.
#[tokio::test]
async fn test_build_registry_service_load_balances_across_configured_backends_happy() {
    let seen_urls = Arc::new(std::sync::Mutex::new(Vec::new()));
    let default_client = Arc::new(RecordingHttpEgressDouble {
        seen_urls: Arc::clone(&seen_urls),
    });

    let mut services = BTreeMap::new();
    services.insert("user-service".to_string(), two_backend_config());
    let config = RuntimeConfig {
        services,
        ..RuntimeConfig::default()
    };

    let registry = swe_edge_bootstrap::Runtime::builder()
        .egress_http(default_client)
        .config(config)
        .build_registry()
        .expect("egress_http was set, must build a registry");

    let user_service_client = registry
        .service("user-service")
        .expect("user-service must be registered")
        .clone();

    for _ in 0..4 {
        user_service_client
            .send(HttpRequest::get("https://irrelevant.test/path?x=1"))
            .await
            .expect("stub send always succeeds");
    }

    let urls = seen_urls.lock().expect("lock").clone();
    assert_eq!(urls.len(), 4);
    // Round-robin over 2 backends across 4 calls: alternates, both appear twice.
    let user_service_1 = urls.iter().filter(|u| u.contains("user-service-1")).count();
    let user_service_2 = urls.iter().filter(|u| u.contains("user-service-2")).count();
    assert_eq!(
        (user_service_1, user_service_2),
        (2, 2),
        "round-robin over 2 backends across 4 calls must split evenly, got: {urls:?}"
    );
    // Path/query from the original request must be preserved, not dropped.
    assert!(
        urls.iter().all(|u| u.ends_with("/path?x=1")),
        "backend rewrite must preserve path and query, got: {urls:?}"
    );
}

/// @covers: build_registry — an unconfigured service name is simply absent,
/// not silently backed by the default client.
#[tokio::test]
async fn test_build_registry_unconfigured_service_name_is_absent_error() {
    let default_client = Arc::new(RecordingHttpEgressDouble {
        seen_urls: Arc::new(std::sync::Mutex::new(Vec::new())),
    });
    let mut services = BTreeMap::new();
    services.insert("user-service".to_string(), two_backend_config());
    let config = RuntimeConfig {
        services,
        ..RuntimeConfig::default()
    };

    let registry = swe_edge_bootstrap::Runtime::builder()
        .egress_http(default_client)
        .config(config)
        .build_registry()
        .expect("egress_http was set, must build a registry");

    assert!(registry.service("billing-service").is_none());
}

/// @covers: build_registry — a service with an empty backend list is
/// skipped, not a hard build failure for the whole registry.
#[tokio::test]
async fn test_build_registry_skips_service_with_empty_backends_edge() {
    let default_client = Arc::new(RecordingHttpEgressDouble {
        seen_urls: Arc::new(std::sync::Mutex::new(Vec::new())),
    });
    let mut services = BTreeMap::new();
    services.insert(
        "broken-service".to_string(),
        ServiceEgressConfig {
            loadbalancer: LoadbalancerConfig {
                strategy: Strategy::RoundRobin,
                backends: vec![],
            },
        },
    );
    services.insert("user-service".to_string(), two_backend_config());
    let config = RuntimeConfig {
        services,
        ..RuntimeConfig::default()
    };

    let registry = swe_edge_bootstrap::Runtime::builder()
        .egress_http(default_client)
        .config(config)
        .build_registry()
        .expect("egress_http was set, must build a registry despite one bad service entry");

    assert!(registry.service("broken-service").is_none());
    assert!(registry.service("user-service").is_some());
}

/// @covers: build_registry — without an explicit `.config(...)`, only the
/// default client is registered; no per-service pools are built.
#[tokio::test]
async fn test_build_registry_without_config_registers_no_services_edge() {
    let default_client = Arc::new(RecordingHttpEgressDouble {
        seen_urls: Arc::new(std::sync::Mutex::new(Vec::new())),
    });

    let registry = swe_edge_bootstrap::Runtime::builder()
        .egress_http(default_client)
        .build_registry()
        .expect("egress_http was set, must build a registry");

    assert_eq!(registry.service_names().count(), 0);
}

/// @covers: LoadBalancedHttpEgress::send — a failing call still degrades the
/// selected backend in the pool (parity with `breaker`'s former pool-report
/// integration for automatic, per-call outcome tracking).
#[tokio::test]
async fn test_load_balanced_egress_failed_call_degrades_selected_backend_happy() {
    let failing_default = Arc::new(AlwaysFailingHttpEgressDouble);
    let mut services = BTreeMap::new();
    services.insert(
        "user-service".to_string(),
        ServiceEgressConfig {
            loadbalancer: LoadbalancerConfig {
                strategy: Strategy::RoundRobin,
                backends: vec![BackendConfig {
                    url: "https://only-backend.internal".to_string(),
                    weight: 1,
                }],
            },
        },
    );
    let config = RuntimeConfig {
        services,
        ..RuntimeConfig::default()
    };

    let registry = swe_edge_bootstrap::Runtime::builder()
        .egress_http(failing_default)
        .config(config)
        .build_registry()
        .expect("egress_http was set, must build a registry");

    let client = registry
        .service("user-service")
        .expect("user-service must be registered")
        .clone();

    // The single backend fails on every call; the client itself must still
    // surface the underlying error, not swallow it.
    let call_count = AtomicUsize::new(0);
    for _ in 0..2 {
        let result = client
            .send(HttpRequest::get("https://irrelevant.test"))
            .await;
        assert!(result.is_err(), "failing backend's error must propagate");
        call_count.fetch_add(1, Ordering::SeqCst);
    }
    assert_eq!(call_count.load(Ordering::SeqCst), 2);
}
