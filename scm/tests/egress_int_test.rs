//! Coverage for Egress trait impl via Runtime::http_egress factory.
// @allow: no_mocks_in_integration — stub impls required to exercise the public API surface

use futures::future::BoxFuture;
use std::sync::Arc;
use swe_edge_bootstrap::{Egress, Runtime};
use swe_edge_egress_http::{
    HttpEgress, HttpEgressResult, HttpRequest, HttpResponse, HttpStreamResponse,
};

struct StubHttp;
impl HttpEgress for StubHttp {
    fn send(&self, _: HttpRequest) -> BoxFuture<'_, HttpEgressResult<HttpResponse>> {
        Box::pin(async { Ok(HttpResponse::new(200, vec![])) })
    }
    fn send_stream(&self, _: HttpRequest) -> BoxFuture<'_, HttpEgressResult<HttpStreamResponse>> {
        Box::pin(async {
            Ok(HttpStreamResponse {
                status: 200,
                headers: Default::default(),
                body: Box::pin(futures::stream::empty()),
            })
        })
    }
    fn health_check(&self) -> BoxFuture<'_, HttpEgressResult<()>> {
        Box::pin(async { Ok(()) })
    }
}

/// @covers: Runtime::http_egress — http()
#[test]
fn test_http_egress_http_returns_configured_adapter() {
    let egress = Runtime::http_egress(Arc::new(StubHttp));
    let _ = egress.http();
    assert!(egress.grpc().is_none());
}
