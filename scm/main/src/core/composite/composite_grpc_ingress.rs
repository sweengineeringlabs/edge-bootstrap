//! `CompositeGrpcIngress` — implementation.

use std::sync::Arc;

use futures::future::BoxFuture;
use swe_edge_ingress_grpc::{
    GrpcIngress, GrpcIngressError, HealthCheckRequest, HealthCheckResponse, StreamRequest,
    StreamResponse, UnaryRequest,
};

pub(crate) use crate::api::composite::types::composite_grpc_ingress::CompositeGrpcIngress;

const REFLECTION_PREFIX: &str = "/grpc.reflection.";

impl CompositeGrpcIngress {
    pub(crate) fn new(primary: Arc<dyn GrpcIngress>, reflection: Arc<dyn GrpcIngress>) -> Self {
        Self {
            primary,
            reflection,
        }
    }

    fn route(&self, method: &str) -> Arc<dyn GrpcIngress> {
        if method.starts_with(REFLECTION_PREFIX) {
            Arc::clone(&self.reflection)
        } else {
            Arc::clone(&self.primary)
        }
    }
}

impl GrpcIngress for CompositeGrpcIngress {
    fn handle_unary(
        &self,
        req: UnaryRequest,
    ) -> BoxFuture<'_, Result<swe_edge_ingress_grpc::GrpcResponse, GrpcIngressError>> {
        let handler = self.route(&req.request.method);
        Box::pin(async move { handler.handle_unary(req).await })
    }

    fn handle_stream(
        &self,
        req: StreamRequest,
    ) -> BoxFuture<'_, Result<StreamResponse, GrpcIngressError>> {
        let handler = self.route(&req.method);
        Box::pin(async move { handler.handle_stream(req).await })
    }

    fn health_check(
        &self,
        req: HealthCheckRequest,
    ) -> BoxFuture<'_, Result<HealthCheckResponse, GrpcIngressError>> {
        self.primary.health_check(req)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::future::BoxFuture;
    use parking_lot::Mutex;
    use std::sync::Arc;
    use swe_edge_ingress_grpc::{GrpcHealthCheck, GrpcRequest, SecurityContext};

    #[derive(Default)]
    struct CompositeGrpcIngressTracker {
        called: Mutex<bool>,
    }

    impl CompositeGrpcIngressTracker {
        fn was_called(&self) -> bool {
            *self.called.lock()
        }
    }

    impl GrpcIngress for CompositeGrpcIngressTracker {
        fn handle_unary(
            &self,
            _req: UnaryRequest,
        ) -> BoxFuture<'_, Result<swe_edge_ingress_grpc::GrpcResponse, GrpcIngressError>> {
            *self.called.lock() = true;
            Box::pin(async { Err(GrpcIngressError::Unimplemented("stub".into())) })
        }
        fn handle_stream(
            &self,
            _req: StreamRequest,
        ) -> BoxFuture<'_, Result<StreamResponse, GrpcIngressError>> {
            *self.called.lock() = true;
            Box::pin(async { Err(GrpcIngressError::Unimplemented("stub".into())) })
        }
        fn health_check(
            &self,
            _req: HealthCheckRequest,
        ) -> BoxFuture<'_, Result<HealthCheckResponse, GrpcIngressError>> {
            Box::pin(async {
                Ok(HealthCheckResponse {
                    check: Box::new(GrpcHealthCheck::healthy()),
                })
            })
        }
    }

    fn req(method: &str) -> GrpcRequest {
        GrpcRequest::new(method, vec![], std::time::Duration::from_secs(5))
    }

    #[test]
    fn test_new_creates_composite_with_both_handlers() {
        let primary = Arc::new(CompositeGrpcIngressTracker::default());
        let reflection = Arc::new(CompositeGrpcIngressTracker::default());
        let _composite = CompositeGrpcIngress::new(
            Arc::clone(&primary) as Arc<dyn GrpcIngress>,
            Arc::clone(&reflection) as Arc<dyn GrpcIngress>,
        );
        assert!(!primary.was_called());
        assert!(!reflection.was_called());
    }

    #[tokio::test]
    async fn test_handle_unary_routes_reflection_path_to_reflection_handler() {
        let primary = Arc::new(CompositeGrpcIngressTracker::default());
        let reflection = Arc::new(CompositeGrpcIngressTracker::default());
        let composite = CompositeGrpcIngress::new(
            Arc::clone(&primary) as Arc<dyn GrpcIngress>,
            Arc::clone(&reflection) as Arc<dyn GrpcIngress>,
        );
        let _ = composite
            .handle_unary(UnaryRequest {
                request: req("/grpc.reflection.v1alpha.ServerReflection/ServerReflectionInfo"),
                ctx: SecurityContext::unauthenticated(),
            })
            .await;
        assert!(
            !primary.was_called(),
            "primary must not be called for reflection path"
        );
        assert!(
            reflection.was_called(),
            "reflection must be called for reflection path"
        );
    }

    #[tokio::test]
    async fn test_handle_unary_routes_non_reflection_path_to_primary_handler() {
        let primary = Arc::new(CompositeGrpcIngressTracker::default());
        let reflection = Arc::new(CompositeGrpcIngressTracker::default());
        let composite = CompositeGrpcIngress::new(
            Arc::clone(&primary) as Arc<dyn GrpcIngress>,
            Arc::clone(&reflection) as Arc<dyn GrpcIngress>,
        );
        let _ = composite
            .handle_unary(UnaryRequest {
                request: req("/my.Service/Method"),
                ctx: SecurityContext::unauthenticated(),
            })
            .await;
        assert!(
            primary.was_called(),
            "primary must be called for non-reflection path"
        );
        assert!(
            !reflection.was_called(),
            "reflection must not be called for non-reflection path"
        );
    }
}
