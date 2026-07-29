//! Intrusion guard — HTTP/gRPC wrappers around `edge-intrusion`'s IDS/IPS.

mod grpc_intrusion_guard;
mod http_intrusion_guard;

pub(crate) use grpc_intrusion_guard::GrpcIntrusionGuard;
pub(crate) use http_intrusion_guard::HttpIntrusionGuard;
