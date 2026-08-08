//! Composition-root bridge from `edge-proxy::Job` to `edge-dispatch`'s
//! `HandlerRegistry`/`Pipeline`, per ADR-021 (see
//! `docs/3-design/adr/003-adopt-adr-021-system-request-flow.md`).
mod default_grpc_job;
mod default_http_job;
mod grpc_ingress_job;
mod http_ingress_job;

pub(crate) use default_grpc_job::DefaultGrpcJob;
pub(crate) use default_http_job::DefaultHttpJob;
pub(crate) use grpc_ingress_job::GrpcIngressJob;
pub(crate) use http_ingress_job::HttpIngressJob;

/// Local wrapper satisfying `edge_application::Request`/`Response` for a
/// foreign wire-payload type. `edge_application::Handler`'s associated
/// types require `Request`/`Response`, but the actual wire types
/// (`HttpRequest`/`HttpResponse` from edge-transport-http-ingress, raw
/// `Vec<u8>` for gRPC) are foreign to this crate — the orphan rule blocks
/// implementing the markers directly on them — and, per
/// edge-transport-http-ingress#29/edge-transport-grpc-ingress#33, no longer
/// implement these markers themselves (transport must not know
/// `edge-application` exists). Used by both `default_http_job` and
/// `default_grpc_job` for the identical reason.
#[derive(Clone)]
pub(crate) struct Payload<T>(pub(crate) T);
impl<T: Send + 'static> edge_application::Request for Payload<T> {}
impl<T: Send + 'static> edge_application::Response for Payload<T> {}
