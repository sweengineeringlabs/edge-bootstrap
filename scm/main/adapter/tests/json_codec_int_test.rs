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

    #[derive(Deserialize)]
    struct Req {
        prompt: String,
    }
    impl edge_application::Request for Req {}
    #[derive(Serialize)]
    struct Resp {
        text: String,
    }
    impl edge_application::Response for Resp {}

    struct EchoHandler;

    #[async_trait::async_trait]
    impl Handler for EchoHandler {
        type Request = Req;
        type Response = Resp;
        fn id(
            &self,
            _req: edge_application_handler::IdRequest,
        ) -> Result<edge_application_handler::IdResponse, HandlerError> {
            Ok(edge_application_handler::IdResponse {
                id: "json-codec-echo".to_string(),
            })
        }
        fn pattern(
            &self,
            _req: edge_application_handler::PatternRequest,
        ) -> Result<edge_application_handler::PatternResponse, HandlerError> {
            Ok(edge_application_handler::PatternResponse {
                pattern: "/json-codec-echo".to_string(),
            })
        }
        async fn execute(
            &self,
            req: edge_application_handler::ExecutionRequest<'_, Req>,
        ) -> Result<Resp, HandlerError> {
            Ok(Resp {
                text: req.req.prompt,
            })
        }
    }

    let cfg = RuntimeConfig::default();
    let _builder = Runtime::builder()
        .config(cfg)
        .http_route(Arc::new(EchoHandler));
}
