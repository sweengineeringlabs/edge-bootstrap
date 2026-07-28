//! `GrpcLoadMonitor` — gRPC inbound load-monitoring wrapper interface.

use swe_edge_ingress_grpc::GrpcIngress;

/// Marker supertrait for gRPC inbound handlers that record load metrics.
pub trait GrpcLoadMonitor: GrpcIngress {}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::future::BoxFuture;
    use std::sync::Arc;
    use swe_edge_ingress_grpc::{
        GrpcHealthCheck, GrpcIngressError, GrpcResponse, HealthCheckRequest, HealthCheckResponse,
        StreamRequest, StreamResponse, UnaryRequest,
    };

    struct GrpcLoadMonitorDouble;
    impl GrpcIngress for GrpcLoadMonitorDouble {
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
    impl GrpcLoadMonitor for GrpcLoadMonitorDouble {}

    #[test]
    fn test_grpc_load_monitor_double_is_object_safe_as_dyn() {
        let _: Arc<dyn GrpcLoadMonitor> = Arc::new(GrpcLoadMonitorDouble);
    }
}
