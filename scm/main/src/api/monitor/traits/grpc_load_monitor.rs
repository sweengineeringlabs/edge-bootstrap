//! `GrpcLoadMonitor` — gRPC inbound load-monitoring wrapper interface.

use swe_edge_ingress_grpc::GrpcIngress;

/// Marker supertrait for gRPC inbound handlers that record load metrics.
pub trait GrpcLoadMonitor: GrpcIngress {}
