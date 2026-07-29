//! `Egress` — egress adapter contract.

use std::sync::Arc;
use swe_edge_egress_grpc::GrpcEgress;
use swe_edge_egress_http::HttpEgress;

/// Supplies the egress adapters the runtime uses for outbound calls.
pub trait Egress: Send + Sync {
    /// Returns the HTTP outbound client.
    fn http(&self) -> Arc<dyn HttpEgress>;
    /// Returns the gRPC outbound client, if configured.
    fn grpc(&self) -> Option<Arc<dyn GrpcEgress>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::future::BoxFuture;
    use swe_edge_egress_grpc::{GrpcEgressResult, GrpcMetadata, GrpcRequest, GrpcResponse};
    use swe_edge_egress_http::{
        ConfigRequest, ConfigResponse, HealthCheckRequest, HttpConfig, HttpEgressError,
        HttpRequest, HttpResponse, HttpStreamResponse,
    };

    struct HttpEgressDouble;
    #[async_trait::async_trait]
    impl HttpEgress for HttpEgressDouble {
        async fn send(&self, _: HttpRequest) -> Result<HttpResponse, HttpEgressError> {
            Err(HttpEgressError::Internal("double".into()))
        }
        async fn send_stream(&self, _: HttpRequest) -> Result<HttpStreamResponse, HttpEgressError> {
            Err(HttpEgressError::Internal("double".into()))
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

    struct GrpcEgressDouble;
    impl GrpcEgress for GrpcEgressDouble {
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

    struct EgressDouble {
        http: Arc<dyn HttpEgress>,
        grpc: Option<Arc<dyn GrpcEgress>>,
    }

    impl Egress for EgressDouble {
        fn http(&self) -> Arc<dyn HttpEgress> {
            Arc::clone(&self.http)
        }
        fn grpc(&self) -> Option<Arc<dyn GrpcEgress>> {
            self.grpc.clone()
        }
    }

    #[test]
    fn test_egress_double_http_is_always_present() {
        let e = EgressDouble {
            http: Arc::new(HttpEgressDouble),
            grpc: None,
        };
        let _: Arc<dyn HttpEgress> = e.http();
        assert!(e.grpc().is_none());
    }

    #[test]
    fn test_egress_double_grpc_present_when_configured() {
        let e = EgressDouble {
            http: Arc::new(HttpEgressDouble),
            grpc: Some(Arc::new(GrpcEgressDouble)),
        };
        assert!(e.grpc().is_some());
    }
}
