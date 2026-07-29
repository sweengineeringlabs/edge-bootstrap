//! `CompositeIngress` — composite ingress router contract.

use std::sync::Arc;
use swe_edge_bootstrap_ingress::Ingress;
use swe_edge_ingress_grpc::GrpcIngress;

/// Routes requests between a primary handler and a secondary (e.g. reflection).
///
/// A composite router is itself an [`Ingress`] supplier: its `grpc()` accessor
/// exposes the router's own dispatch surface, while `primary()` exposes the
/// underlying handler non-reflection traffic delegates to.
pub trait CompositeIngress: Ingress {
    /// Returns the primary (non-reflection) gRPC handler.
    fn primary(&self) -> Arc<dyn GrpcIngress>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::future::BoxFuture;
    use swe_edge_ingress_grpc::{
        GrpcHealthCheck, GrpcIngressError, GrpcResponse, HealthCheckRequest, HealthCheckResponse,
        StreamRequest, StreamResponse, UnaryRequest,
    };
    use swe_edge_ingress_http::HttpIngress;

    struct GrpcIngressDouble;
    impl GrpcIngress for GrpcIngressDouble {
        fn handle_unary(
            &self,
            _: UnaryRequest,
        ) -> BoxFuture<'_, Result<GrpcResponse, GrpcIngressError>> {
            Box::pin(async { Err(GrpcIngressError::Unimplemented("double".into())) })
        }
        fn handle_stream(
            &self,
            _: StreamRequest,
        ) -> BoxFuture<'_, Result<StreamResponse, GrpcIngressError>> {
            Box::pin(async { Err(GrpcIngressError::Unimplemented("double".into())) })
        }
        fn health_check(
            &self,
            _: HealthCheckRequest,
        ) -> BoxFuture<'_, Result<HealthCheckResponse, GrpcIngressError>> {
            Box::pin(async {
                Ok(HealthCheckResponse {
                    check: Box::new(GrpcHealthCheck::healthy()),
                })
            })
        }
    }

    struct CompositeIngressDouble {
        primary: Arc<dyn GrpcIngress>,
    }

    impl Ingress for CompositeIngressDouble {
        fn http(&self) -> Option<Arc<dyn HttpIngress>> {
            None
        }
        fn grpc(&self) -> Option<Arc<dyn GrpcIngress>> {
            Some(Arc::clone(&self.primary))
        }
    }

    impl CompositeIngress for CompositeIngressDouble {
        fn primary(&self) -> Arc<dyn GrpcIngress> {
            Arc::clone(&self.primary)
        }
    }

    #[test]
    fn test_composite_ingress_double_satisfies_ingress_supertrait() {
        let d = CompositeIngressDouble {
            primary: Arc::new(GrpcIngressDouble),
        };
        assert!(d.grpc().is_some());
        assert!(d.http().is_none());
        assert!(d.has_any());
    }

    #[test]
    fn test_composite_ingress_double_is_object_safe_as_dyn() {
        let d: Arc<dyn CompositeIngress> = Arc::new(CompositeIngressDouble {
            primary: Arc::new(GrpcIngressDouble),
        });
        let _: Arc<dyn GrpcIngress> = d.primary();
    }
}
