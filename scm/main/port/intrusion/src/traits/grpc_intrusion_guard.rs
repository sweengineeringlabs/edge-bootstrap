//! `GrpcIntrusionGuard` — gRPC inbound IDS/IPS wrapper interface.

use swe_edge_ingress_grpc::GrpcIngress;

/// Marker supertrait for gRPC inbound handlers that reject calls flagged by
/// an intrusion-detection rules engine before delegating to the wrapped
/// handler. Per the fail-open contract the concrete implementation relies
/// on (`edge-intrusion`'s ADR-002), a detector fault must never turn into a
/// rejected call — only an explicit `Decision::Reject` blocks.
pub trait GrpcIntrusionGuard: GrpcIngress {}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::future::BoxFuture;
    use std::sync::Arc;
    use swe_edge_ingress_grpc::{
        GrpcHealthCheck, GrpcIngressError, GrpcResponse, HealthCheckRequest, HealthCheckResponse,
        StreamRequest, StreamResponse, UnaryRequest,
    };

    struct GrpcIntrusionGuardDouble;
    impl GrpcIngress for GrpcIntrusionGuardDouble {
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
    impl GrpcIntrusionGuard for GrpcIntrusionGuardDouble {}

    #[test]
    fn test_grpc_intrusion_guard_double_is_object_safe_as_dyn() {
        let _: Arc<dyn GrpcIntrusionGuard> = Arc::new(GrpcIntrusionGuardDouble);
    }
}
