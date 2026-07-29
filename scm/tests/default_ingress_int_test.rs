//! Integration tests for Runtime ingress factory methods.

use futures::future::BoxFuture;
use std::sync::Arc;
use swe_edge_bootstrap::{Ingress, Runtime};
use swe_edge_ingress_grpc::{
    GrpcHealthCheck, GrpcIngressError, GrpcIngressResult, GrpcResponse, HealthCheckRequest,
    HealthCheckResponse, StreamRequest, StreamResponse, UnaryRequest,
};

/// @covers: Runtime::empty_ingress
#[test]
fn test_empty_ingress_has_no_transports() {
    let i = Runtime::empty_ingress();
    assert!(!i.has_any());
}

/// @covers: Runtime::empty_ingress
#[test]
fn test_empty_ingress_http_returns_none() {
    let i = Runtime::empty_ingress();
    assert!(i.http().is_none());
}

/// @covers: Runtime::empty_ingress
#[test]
fn test_empty_ingress_grpc_returns_none() {
    let i = Runtime::empty_ingress();
    assert!(i.grpc().is_none());
}

/// @covers: Runtime::grpc_ingress
#[test]
fn test_grpc_ingress_has_any_returns_true() {
    struct Stub;
    impl swe_edge_bootstrap::GrpcIngress for Stub {
        fn handle_unary(&self, _: UnaryRequest) -> BoxFuture<'_, GrpcIngressResult<GrpcResponse>> {
            Box::pin(async { Err(GrpcIngressError::Unimplemented("stub".into())) })
        }
        fn handle_stream(
            &self,
            _: StreamRequest,
        ) -> BoxFuture<'_, GrpcIngressResult<StreamResponse>> {
            Box::pin(async { Err(GrpcIngressError::Unimplemented("stub".into())) })
        }
        fn health_check(
            &self,
            _: HealthCheckRequest,
        ) -> BoxFuture<'_, GrpcIngressResult<HealthCheckResponse>> {
            Box::pin(async {
                Ok(HealthCheckResponse {
                    check: Box::new(GrpcHealthCheck::healthy()),
                })
            })
        }
    }
    let i = Runtime::grpc_ingress(Arc::new(Stub));
    assert!(i.has_any());
}
