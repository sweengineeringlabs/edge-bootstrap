//! Domain payload and `Handler` construction — the one thing this module
//! owns is "what does the shared HTTP+gRPC handler look like."

use std::sync::Arc;

use edge_domain::{Domain, Handler};

/// Payload shared by both the HTTP and gRPC routes.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct EchoPayload(pub(crate) String);
impl edge_domain::Request for EchoPayload {}
impl edge_domain::Response for EchoPayload {}

/// Build the echo `Handler`, shared verbatim across `.http_route()` and
/// `.grpc_route()` — proving one handler implementation serves both
/// protocols with no duplicated logic.
pub(crate) fn build_handler() -> Arc<dyn Handler<Request = EchoPayload, Response = EchoPayload>> {
    Domain.echo_handler::<EchoPayload>("echo", "/echo")
}
