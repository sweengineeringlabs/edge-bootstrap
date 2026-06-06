//! `CompositeGrpcIngress` — implementation.

use std::sync::Arc;

use edge_domain::RequestContext;
use futures::future::BoxFuture;
use swe_edge_ingress_grpc::{
    GrpcHealthCheck, GrpcIngress, GrpcIngressResult, GrpcMessageStream, GrpcMetadata, GrpcRequest,
    GrpcResponse,
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
        request: GrpcRequest,
        ctx: RequestContext,
    ) -> BoxFuture<'_, GrpcIngressResult<GrpcResponse>> {
        let handler = self.route(&request.method);
        Box::pin(async move { handler.handle_unary(request, ctx).await })
    }

    fn handle_stream(
        &self,
        method: String,
        metadata: GrpcMetadata,
        messages: GrpcMessageStream,
        ctx: RequestContext,
    ) -> BoxFuture<'_, GrpcIngressResult<(GrpcMessageStream, GrpcMetadata)>> {
        let handler = self.route(&method);
        Box::pin(async move { handler.handle_stream(method, metadata, messages, ctx).await })
    }

    fn health_check(&self) -> BoxFuture<'_, GrpcIngressResult<GrpcHealthCheck>> {
        self.primary.health_check()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::future::BoxFuture;
    use parking_lot::Mutex;
    use std::sync::Arc;
    use swe_edge_ingress_grpc::{GrpcIngressError, GrpcMetadata, GrpcRequest};

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
            _req: GrpcRequest,
            _ctx: RequestContext,
        ) -> BoxFuture<'_, GrpcIngressResult<GrpcResponse>> {
            *self.called.lock() = true;
            Box::pin(async { Err(GrpcIngressError::Unimplemented("stub".into())) })
        }
        fn handle_stream(
            &self,
            _m: String,
            _md: GrpcMetadata,
            _ms: GrpcMessageStream,
            _ctx: RequestContext,
        ) -> BoxFuture<'_, GrpcIngressResult<(GrpcMessageStream, GrpcMetadata)>> {
            *self.called.lock() = true;
            Box::pin(async { Err(GrpcIngressError::Unimplemented("stub".into())) })
        }
        fn health_check(&self) -> BoxFuture<'_, GrpcIngressResult<GrpcHealthCheck>> {
            Box::pin(async { Ok(GrpcHealthCheck::healthy()) })
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
            .handle_unary(
                req("/grpc.reflection.v1alpha.ServerReflection/ServerReflectionInfo"),
                RequestContext::unauthenticated(),
            )
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
            .handle_unary(req("/my.Service/Method"), RequestContext::unauthenticated())
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
