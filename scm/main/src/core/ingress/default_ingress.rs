//! `DefaultIngress` — ingress adapter holder and its [`Ingress`] impl.

use std::sync::Arc;

use swe_edge_ingress_grpc::GrpcIngress;
use swe_edge_ingress_http::HttpIngress;

use swe_edge_bootstrap_ingress_port::Ingress;

pub(crate) struct DefaultIngress {
    http: Option<Arc<dyn HttpIngress>>,
    grpc: Option<Arc<dyn GrpcIngress>>,
}

impl DefaultIngress {
    pub(crate) fn new_http(http: Arc<dyn HttpIngress>) -> Self {
        Self {
            http: Some(http),
            grpc: None,
        }
    }
    pub(crate) fn new_grpc(grpc: Arc<dyn GrpcIngress>) -> Self {
        Self {
            http: None,
            grpc: Some(grpc),
        }
    }
    pub(crate) fn empty() -> Self {
        Self {
            http: None,
            grpc: None,
        }
    }
    pub(crate) fn with_http(mut self, http: Arc<dyn HttpIngress>) -> Self {
        self.http = Some(http);
        self
    }
    pub(crate) fn with_grpc(mut self, grpc: Arc<dyn GrpcIngress>) -> Self {
        self.grpc = Some(grpc);
        self
    }
}

impl Ingress for DefaultIngress {
    fn http(&self) -> Option<Arc<dyn HttpIngress>> {
        self.http.clone()
    }
    fn grpc(&self) -> Option<Arc<dyn GrpcIngress>> {
        self.grpc.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::future::BoxFuture;
    use std::collections::HashMap;
    use swe_edge_ingress_grpc::{
        GrpcHealthCheck, GrpcIngressResult, GrpcMetadataInner, GrpcResponse,
        HealthCheckRequest as GrpcHealthCheckRequest,
        HealthCheckResponse as GrpcHealthCheckResponse, UnaryRequest,
    };
    use swe_edge_ingress_http::{
        HealthCheckRequest, HealthCheckResponse, HttpFuture, HttpHealthCheck, HttpIngressError,
        HttpResponse, InboundRequest,
    };

    struct DefaultIngressStubHttp;
    impl HttpIngress for DefaultIngressStubHttp {
        fn handle(
            &self,
            _: InboundRequest,
        ) -> HttpFuture<'_, Result<HttpResponse, HttpIngressError>> {
            HttpFuture::new(async { Ok(HttpResponse::new(200, vec![])) })
        }
        fn health_check(
            &self,
            _: HealthCheckRequest,
        ) -> HttpFuture<'_, Result<HealthCheckResponse, HttpIngressError>> {
            HttpFuture::new(async {
                Ok(HealthCheckResponse {
                    health: HttpHealthCheck::healthy(),
                })
            })
        }
    }

    struct DefaultIngressStubGrpc;
    impl GrpcIngress for DefaultIngressStubGrpc {
        fn handle_unary(&self, _: UnaryRequest) -> BoxFuture<'_, GrpcIngressResult<GrpcResponse>> {
            Box::pin(async {
                Ok(GrpcResponse {
                    body: vec![],
                    metadata: std::sync::Arc::new(GrpcMetadataInner {
                        headers: HashMap::new(),
                    }),
                })
            })
        }
        fn health_check(
            &self,
            _: GrpcHealthCheckRequest,
        ) -> BoxFuture<'_, GrpcIngressResult<GrpcHealthCheckResponse>> {
            Box::pin(async {
                Ok(GrpcHealthCheckResponse {
                    check: Box::new(GrpcHealthCheck::healthy()),
                })
            })
        }
    }

    #[test]
    fn test_new_http_sets_http_and_leaves_grpc_none() {
        let i = DefaultIngress::new_http(Arc::new(DefaultIngressStubHttp));
        assert!(i.http().is_some());
        assert!(i.grpc().is_none());
    }

    #[test]
    fn test_new_grpc_sets_grpc_and_leaves_http_none() {
        let i = DefaultIngress::new_grpc(Arc::new(DefaultIngressStubGrpc));
        assert!(i.grpc().is_some());
        assert!(i.http().is_none());
    }

    #[test]
    fn test_empty_has_no_transports() {
        let i = DefaultIngress::empty();
        assert!(i.http().is_none());
        assert!(i.grpc().is_none());
    }

    #[test]
    fn test_with_http_adds_http_transport() {
        let i = DefaultIngress::empty().with_http(Arc::new(DefaultIngressStubHttp));
        assert!(i.http().is_some());
    }

    #[test]
    fn test_with_grpc_adds_grpc_transport() {
        let i = DefaultIngress::new_http(Arc::new(DefaultIngressStubHttp))
            .with_grpc(Arc::new(DefaultIngressStubGrpc));
        assert!(i.http().is_some());
        assert!(i.grpc().is_some());
    }
}
