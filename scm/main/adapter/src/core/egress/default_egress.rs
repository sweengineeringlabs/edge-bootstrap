//! `DefaultEgress` — egress adapter holder and its [`Egress`] impl.

use std::sync::Arc;

use swe_edge_egress_grpc::GrpcEgress;
use swe_edge_egress_http::HttpEgress;

use swe_edge_bootstrap_egress::Egress;

pub(crate) struct DefaultEgress {
    http: Arc<dyn HttpEgress>,
    grpc: Option<Arc<dyn GrpcEgress>>,
}

impl DefaultEgress {
    pub(crate) fn new_http(http: Arc<dyn HttpEgress>) -> Self {
        Self { http, grpc: None }
    }
    pub(crate) fn with_grpc(mut self, grpc: Arc<dyn GrpcEgress>) -> Self {
        self.grpc = Some(grpc);
        self
    }
}

impl Egress for DefaultEgress {
    fn http(&self) -> Arc<dyn HttpEgress> {
        self.http.clone()
    }
    fn grpc(&self) -> Option<Arc<dyn GrpcEgress>> {
        self.grpc.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::future::BoxFuture;
    use swe_edge_egress_http::{
        ConfigRequest, ConfigResponse, HealthCheckRequest, HttpConfig, HttpEgressError,
        HttpStreamResponse,
    };

    struct DefaultEgressStubHttp;
    #[async_trait::async_trait]
    impl HttpEgress for DefaultEgressStubHttp {
        async fn send(
            &self,
            _: swe_edge_egress_http::HttpRequest,
        ) -> Result<swe_edge_egress_http::HttpResponse, HttpEgressError> {
            Err(HttpEgressError::Internal("stub".into()))
        }
        async fn send_stream(
            &self,
            _: swe_edge_egress_http::HttpRequest,
        ) -> Result<HttpStreamResponse, HttpEgressError> {
            Err(HttpEgressError::Internal("stub".into()))
        }
        async fn health_check(&self, _: HealthCheckRequest) -> Result<(), HttpEgressError> {
            Ok(())
        }
        fn config(&self, _: ConfigRequest) -> Result<ConfigResponse, HttpEgressError> {
            Ok(ConfigResponse {
                config: HttpConfig::default(),
            })
        }
    }

    #[test]
    fn test_new_http_egress_http_returns_configured_client() {
        let e = DefaultEgress::new_http(Arc::new(DefaultEgressStubHttp));
        let _ = e.http();
    }

    #[test]
    fn test_new_http_egress_grpc_returns_none() {
        let e = DefaultEgress::new_http(Arc::new(DefaultEgressStubHttp));
        assert!(e.grpc().is_none());
    }

    #[test]
    fn test_with_grpc_egress_grpc_returns_some() {
        use swe_edge_egress_grpc::{GrpcEgressResult, GrpcMetadata, GrpcRequest, GrpcResponse};
        struct DefaultEgressStubGrpc;
        impl GrpcEgress for DefaultEgressStubGrpc {
            fn call_unary(&self, _: GrpcRequest) -> BoxFuture<'_, GrpcEgressResult<GrpcResponse>> {
                Box::pin(async {
                    Ok(GrpcResponse {
                        body: vec![],
                        metadata: GrpcMetadata::default(),
                    })
                })
            }
            fn health_check(&self) -> BoxFuture<'_, GrpcEgressResult<()>> {
                Box::pin(async { Ok(()) })
            }
        }
        let e = DefaultEgress::new_http(Arc::new(DefaultEgressStubHttp))
            .with_grpc(Arc::new(DefaultEgressStubGrpc));
        assert!(e.grpc().is_some());
    }
}
