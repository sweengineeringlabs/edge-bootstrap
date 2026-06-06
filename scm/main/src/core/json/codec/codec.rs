//! Default JSON codec for HTTP and gRPC routes.
//!
//! `http_route` and `grpc_route` use these automatically when `Req` and `Resp`
//! implement `serde::de::DeserializeOwned` / `serde::Serialize`.

/// Primary implementation type for this module (satisfies Rule 89 filename match).
pub(crate) struct Codec;

impl crate::api::json::traits::codec::Codec for Codec {}

use swe_edge_ingress_grpc::GrpcIngressError;
use swe_edge_ingress_http::{HttpBody, HttpIngressError, HttpRequest, HttpResponse};

impl Codec {
    /// Default HTTP decode: JSON body → `Req`.
    ///
    /// Handles `application/json` (parsed `HttpBody::Json`) and raw bytes
    /// (`HttpBody::Raw`) transparently.  Returns `InvalidInput` on any other body
    /// type or a deserialization failure.
    pub(crate) fn json_decode<Req: serde::de::DeserializeOwned>(
        req: &HttpRequest,
    ) -> Result<Req, HttpIngressError> {
        match &req.body {
            Some(HttpBody::Json(v)) => serde_json::from_value(v.clone())
                .map_err(|e| HttpIngressError::InvalidInput(e.to_string())),
            Some(HttpBody::Raw(b)) => {
                serde_json::from_slice(b).map_err(|e| HttpIngressError::InvalidInput(e.to_string()))
            }
            None => serde_json::from_slice(b"null")
                .map_err(|e| HttpIngressError::InvalidInput(e.to_string())),
            Some(_) => Err(HttpIngressError::InvalidInput(
                "expected JSON body (application/json or raw bytes)".into(),
            )),
        }
    }

    /// Default HTTP encode: `Resp` → 200 JSON response.
    pub(crate) fn json_encode<Resp: serde::Serialize>(resp: Resp) -> HttpResponse {
        match serde_json::to_vec(&resp) {
            Ok(body) => {
                let mut r = HttpResponse::new(200, body);
                r.headers
                    .insert("content-type".into(), "application/json".into());
                r
            }
            Err(e) => HttpResponse::new(500, e.to_string().into_bytes()),
        }
    }

    /// Default gRPC decode: raw bytes → `Req` via JSON.
    pub(crate) fn grpc_json_decode<Req: serde::de::DeserializeOwned>(
        bytes: &[u8],
    ) -> Result<Req, GrpcIngressError> {
        serde_json::from_slice(bytes).map_err(|e| GrpcIngressError::InvalidArgument(e.to_string()))
    }

    /// Default gRPC encode: `Resp` → raw bytes via JSON.
    pub(crate) fn grpc_json_encode<Resp: serde::Serialize>(resp: &Resp) -> Vec<u8> {
        serde_json::to_vec(resp).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use swe_edge_ingress_http::{HttpBody, HttpMethod, HttpRequest};

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct CodecPayload {
        text: String,
    }

    fn post_with_json(v: serde_json::Value) -> HttpRequest {
        HttpRequest {
            method: HttpMethod::Post,
            url: "/".into(),
            headers: Default::default(),
            query: Default::default(),
            body: Some(HttpBody::Json(v)),
            timeout: None,
        }
    }

    fn post_with_raw(b: Vec<u8>) -> HttpRequest {
        HttpRequest {
            method: HttpMethod::Post,
            url: "/".into(),
            headers: Default::default(),
            query: Default::default(),
            body: Some(HttpBody::Raw(b)),
            timeout: None,
        }
    }

    fn post_with_no_body() -> HttpRequest {
        HttpRequest {
            method: HttpMethod::Post,
            url: "/".into(),
            headers: Default::default(),
            query: Default::default(),
            body: None,
            timeout: None,
        }
    }

    #[test]
    fn test_json_decode_json_body_deserializes_correctly() {
        let req = post_with_json(serde_json::json!({"text": "hello"}));
        let msg: CodecPayload = Codec::json_decode(&req).unwrap();
        assert_eq!(
            msg,
            CodecPayload {
                text: "hello".into()
            }
        );
    }

    #[test]
    fn test_json_decode_raw_body_deserializes_correctly() {
        let req = post_with_raw(br#"{"text":"world"}"#.to_vec());
        let msg: CodecPayload = Codec::json_decode(&req).unwrap();
        assert_eq!(
            msg,
            CodecPayload {
                text: "world".into()
            }
        );
    }

    #[test]
    fn test_json_decode_none_body_fails_for_non_nullable_type() {
        let req = post_with_no_body();
        let result = Codec::json_decode::<CodecPayload>(&req);
        assert!(result.is_err());
    }

    #[test]
    fn test_json_decode_form_body_returns_invalid_input_error() {
        let req = HttpRequest {
            body: Some(HttpBody::Form(Default::default())),
            ..post_with_no_body()
        };
        let result = Codec::json_decode::<CodecPayload>(&req);
        assert!(matches!(result, Err(HttpIngressError::InvalidInput(_))));
    }

    #[test]
    fn test_json_encode_produces_200_with_json_content_type() {
        let resp = Codec::json_encode(CodecPayload { text: "ok".into() });
        assert_eq!(resp.status, 200);
        assert_eq!(resp.header("content-type"), Some("application/json"));
        let parsed: CodecPayload = serde_json::from_slice(&resp.body).unwrap();
        assert_eq!(parsed.text, "ok");
    }

    #[test]
    fn test_grpc_json_decode_deserializes_bytes_correctly() {
        let bytes = br#"{"text":"grpc"}"#;
        let msg: CodecPayload = Codec::grpc_json_decode(bytes).unwrap();
        assert_eq!(
            msg,
            CodecPayload {
                text: "grpc".into()
            }
        );
    }

    #[test]
    fn test_grpc_json_decode_invalid_bytes_returns_invalid_argument() {
        let result = Codec::grpc_json_decode::<CodecPayload>(b"not json");
        assert!(matches!(result, Err(GrpcIngressError::InvalidArgument(_))));
    }

    #[test]
    fn test_grpc_json_encode_serializes_to_bytes() {
        let msg = CodecPayload {
            text: "bytes".into(),
        };
        let bytes = Codec::grpc_json_encode(&msg);
        let decoded: CodecPayload = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, msg);
    }
}
