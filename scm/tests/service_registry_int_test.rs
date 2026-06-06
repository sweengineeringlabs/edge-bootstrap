//! Integration tests for ServiceRegistry.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use swe_edge_bootstrap::{Runtime, ServiceRegistry};
use swe_edge_egress_http::HttpTransportSvc;

/// @covers: ServiceRegistry
#[test]
fn test_service_registry_build_registry_returns_none_without_egress() {
    let b = Runtime::builder();
    assert!(b.build_registry().is_none());
}

/// @covers: ServiceRegistry
#[test]
fn test_service_registry_build_registry_returns_some_with_egress_http() {
    let http = Arc::from(HttpTransportSvc::default_http_egress().expect("http egress"));
    let b = Runtime::builder().egress_http(http);
    let reg = b.build_registry();
    assert!(reg.is_some());
}

/// @covers: ServiceRegistry
#[test]
fn test_service_registry_http_returns_stored_client() {
    let http = Arc::from(HttpTransportSvc::default_http_egress().expect("http egress"));
    let reg = ServiceRegistry::new(http, None);
    let _ = reg.http();
}

/// @covers: ServiceRegistry
#[test]
fn test_service_registry_grpc_returns_none_when_not_set() {
    let http = Arc::from(HttpTransportSvc::default_http_egress().expect("http egress"));
    let reg = ServiceRegistry::new(http, None);
    assert!(reg.grpc().is_none());
}
