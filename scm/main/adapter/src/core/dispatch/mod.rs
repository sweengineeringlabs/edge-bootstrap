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
