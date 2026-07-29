//! Coverage for Egress trait impl via Runtime::http_egress factory.
// @allow: no_mocks_in_integration — stub impls required to exercise the public API surface

use std::sync::Arc;
use swe_edge_bootstrap::{Egress, Runtime};
use swe_edge_egress_http::{
    ConfigRequest, ConfigResponse, HealthCheckRequest, HttpByteStream, HttpConfig, HttpEgress,
    HttpEgressError, HttpRequest, HttpResponse, HttpStreamResponse,
};

struct StubHttp;
#[async_trait::async_trait]
impl HttpEgress for StubHttp {
    async fn send(&self, _: HttpRequest) -> Result<HttpResponse, HttpEgressError> {
        Ok(HttpResponse::new(200, vec![]))
    }
    async fn send_stream(&self, _: HttpRequest) -> Result<HttpStreamResponse, HttpEgressError> {
        Ok(HttpStreamResponse {
            status: 200,
            headers: Default::default(),
            body: HttpByteStream::new(futures::stream::empty()),
        })
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

/// @covers: Runtime::http_egress — http()
#[test]
fn test_http_egress_http_returns_configured_adapter() {
    let egress = Runtime::http_egress(Arc::new(StubHttp));
    let _ = egress.http();
    assert!(egress.grpc().is_none());
}
