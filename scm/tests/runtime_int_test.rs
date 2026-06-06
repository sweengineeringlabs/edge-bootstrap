//! Integration tests for `RuntimeBuilder`.

use swe_edge_bootstrap::{IngressTlsConfig, Runtime, RuntimeConfig};

/// @covers: builder — builder is constructible with no arguments
#[test]
fn test_builder_creates_without_arguments() {
    let _builder = Runtime::builder();
}

/// @covers: config — accepted without error
#[test]
fn test_builder_accepts_explicit_runtime_config() {
    let cfg = RuntimeConfig::default().with_service_name("test-svc");
    let _builder = Runtime::builder().config(cfg);
}

/// @covers: app_name — accepted without error
#[test]
fn test_builder_accepts_app_name() {
    let _builder = Runtime::builder().app_name("my-service");
}

/// @covers: http_tls — accepted without error
#[test]
fn test_builder_accepts_http_tls_config() {
    let tls = IngressTlsConfig::tls("cert.pem", "key.pem");
    let _builder = Runtime::builder().http_tls(tls);
}

/// @covers: grpc_tls — mTLS variant accepted
#[test]
fn test_builder_accepts_grpc_mtls_config() {
    let tls = IngressTlsConfig::mtls("cert.pem", "key.pem", "ca.pem");
    let _builder = Runtime::builder().grpc_tls(tls);
}

/// @covers: grpc_allow_unauthenticated — chainable
#[test]
fn test_builder_grpc_allow_unauthenticated_is_chainable() {
    let _builder = Runtime::builder().grpc_allow_unauthenticated();
}

/// @covers: build_registry — None when no egress set
#[test]
fn test_build_registry_returns_none_when_no_egress_http_set() {
    assert!(Runtime::builder().build_registry().is_none());
}
