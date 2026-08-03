//! `Ingress` — ingress adapter contract.

use std::sync::Arc;
use swe_edge_ingress_grpc::GrpcIngress;
use swe_edge_ingress_http::HttpIngress;

/// Supplies the ingress adapters the runtime binds traffic through.
pub trait Ingress: Send + Sync {
    /// Returns the HTTP inbound adapter, if configured.
    fn http(&self) -> Option<Arc<dyn HttpIngress>>;
    /// Returns the gRPC inbound adapter, if configured.
    fn grpc(&self) -> Option<Arc<dyn GrpcIngress>>;
    /// Returns `true` when at least one transport is configured.
    fn has_any(&self) -> bool {
        self.http().is_some() || self.grpc().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::future::BoxFuture;
    use swe_edge_ingress_grpc::{
        GrpcHealthCheck, GrpcIngressError, GrpcResponse, HealthCheckRequest as GrpcHealthReq,
        HealthCheckResponse as GrpcHealthResp, StreamRequest, StreamResponse, UnaryRequest,
    };
    use swe_edge_ingress_http::{
        HealthCheckRequest as HttpHealthReq, HealthCheckResponse as HttpHealthResp, HttpFuture,
        HttpHealthCheck, HttpIngressError, HttpResponse, InboundRequest,
    };

    struct HttpIngressDouble;
    impl HttpIngress for HttpIngressDouble {
        fn handle(
            &self,
            _: InboundRequest,
        ) -> HttpFuture<'_, Result<HttpResponse, HttpIngressError>> {
            HttpFuture::new(async { Ok(HttpResponse::new(200, vec![])) })
        }
        fn health_check(
            &self,
            _: HttpHealthReq,
        ) -> HttpFuture<'_, Result<HttpHealthResp, HttpIngressError>> {
            HttpFuture::new(async {
                Ok(HttpHealthResp {
                    health: HttpHealthCheck::healthy(),
                })
            })
        }
    }

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
            _: GrpcHealthReq,
        ) -> BoxFuture<'_, Result<GrpcHealthResp, GrpcIngressError>> {
            Box::pin(async {
                Ok(GrpcHealthResp {
                    check: Box::new(GrpcHealthCheck::healthy()),
                })
            })
        }
    }

    struct IngressDouble {
        http: Option<Arc<dyn HttpIngress>>,
        grpc: Option<Arc<dyn GrpcIngress>>,
    }

    impl Ingress for IngressDouble {
        fn http(&self) -> Option<Arc<dyn HttpIngress>> {
            self.http.clone()
        }
        fn grpc(&self) -> Option<Arc<dyn GrpcIngress>> {
            self.grpc.clone()
        }
    }

    #[test]
    fn test_has_any_default_method_is_false_when_nothing_configured() {
        let i = IngressDouble {
            http: None,
            grpc: None,
        };
        assert!(!i.has_any());
    }

    #[test]
    fn test_has_any_default_method_is_true_when_only_http_configured() {
        let i = IngressDouble {
            http: Some(Arc::new(HttpIngressDouble)),
            grpc: None,
        };
        assert!(i.has_any());
    }

    #[test]
    fn test_has_any_default_method_is_true_when_only_grpc_configured() {
        let i = IngressDouble {
            http: None,
            grpc: Some(Arc::new(GrpcIngressDouble)),
        };
        assert!(i.has_any());
    }
}
