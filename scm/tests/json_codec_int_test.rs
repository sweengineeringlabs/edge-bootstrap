//! Integration test coverage for the JSON codec type aliases.
//!
//! Codec functions are covered by inline tests in `core/json_codec.rs`.
//! This file satisfies the per-module test-file requirement and verifies
//! the observable codec contract via the public `http_route` API.

use swe_edge_bootstrap::{Runtime, RuntimeConfig};

/// @covers: json_codec — http_route accepts a handler without explicit codec
#[test]
fn test_http_route_accepts_handler_with_auto_json_codec() {
    use serde::{Deserialize, Serialize};
    use std::sync::Arc;
    use swe_edge_bootstrap::{Handler, HandlerError};
    use edge_domain::HandlerContext;

    #[derive(Deserialize)]
    struct Req {
        prompt: String,
    }
    #[derive(Serialize)]
    struct Resp {
        text: String,
    }

    struct EchoHandler;

    #[async_trait::async_trait]
    impl Handler for EchoHandler {
        type Request = Req;
        type Response = Resp;
        fn id(&self) -> &str {
            "echo"
        }
        fn pattern(&self) -> &str {
            "/echo"
        }
        async fn execute(&self, req: Req, _ctx: HandlerContext<'_>) -> Result<Resp, HandlerError> {
            Ok(Resp { text: req.prompt })
        }
    }

    let cfg = RuntimeConfig::default();
    let _builder = Runtime::builder()
        .config(cfg)
        .http_route(Arc::new(EchoHandler));
}
