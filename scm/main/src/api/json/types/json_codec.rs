//! JSON codec type aliases — default HTTP/gRPC decode/encode functions.
//!
//! These are the function-pointer types used by `http_route` and `grpc_route`
//! when no explicit codec is supplied by the caller.

/// Marker trait for types that can be used as JSON codecs.
pub trait JsonCodec: Send + Sync {}

use swe_edge_ingress_grpc::GrpcIngressError;
use swe_edge_ingress_http::{HttpIngressError, HttpRequest, HttpResponse};

/// Default JSON decode: deserialises the HTTP request body into `Req`.
pub(crate) type JsonHttpDecodeFn<Req> = fn(&HttpRequest) -> Result<Req, HttpIngressError>;

/// Default JSON encode: serialises `Resp` into a `200 application/json` response.
pub(crate) type JsonHttpEncodeFn<Resp> = fn(Resp) -> HttpResponse;

/// Default gRPC JSON decode: deserialises raw bytes into `Req`.
pub(crate) type JsonGrpcDecodeFn<Req> = fn(&[u8]) -> Result<Req, GrpcIngressError>;

/// Default gRPC JSON encode: serialises `Resp` to raw bytes.
pub(crate) type JsonGrpcEncodeFn<Resp> = fn(&Resp) -> Vec<u8>;
