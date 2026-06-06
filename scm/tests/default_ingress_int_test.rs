//! Integration tests for Runtime ingress factory methods.

use futures::future::BoxFuture;
use std::sync::Arc;
use swe_edge_bootstrap::{Ingress, Runtime};
use swe_edge_ingress_grpc::{
    GrpcHealthCheck, GrpcIngressError, GrpcIngressResult, GrpcMessageStream, GrpcMetadata,
    GrpcRequest, GrpcResponse,
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
    use edge_domain::RequestContext;
    struct Stub;
    impl swe_edge_bootstrap::GrpcIngress for Stub {
        fn handle_unary(
            &self,
            _: GrpcRequest,
            _: RequestContext,
        ) -> BoxFuture<'_, GrpcIngressResult<GrpcResponse>> {
            Box::pin(async { Err(GrpcIngressError::Unimplemented("stub".into())) })
        }
        fn handle_stream(
            &self,
            _: String,
            _: GrpcMetadata,
            _: GrpcMessageStream,
            _: RequestContext,
        ) -> BoxFuture<'_, GrpcIngressResult<(GrpcMessageStream, GrpcMetadata)>> {
            Box::pin(async { Err(GrpcIngressError::Unimplemented("stub".into())) })
        }
        fn health_check(&self) -> BoxFuture<'_, GrpcIngressResult<GrpcHealthCheck>> {
            Box::pin(async { Ok(GrpcHealthCheck::healthy()) })
        }
    }
    let i = Runtime::grpc_ingress(Arc::new(Stub));
    assert!(i.has_any());
}
