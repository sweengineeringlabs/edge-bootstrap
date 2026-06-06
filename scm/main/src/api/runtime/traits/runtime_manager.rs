//! RuntimeManager trait — owns the full process lifecycle.

use futures::future::BoxFuture;

use crate::api::runtime::types::health::RuntimeHealth;
use crate::api::runtime::RuntimeResult;

/// Manages the full process lifecycle: start, shutdown, and health.
///
/// Implementations wire ingress servers, the controller, and egress
/// adapters into a single runtime that can be started and stopped
/// cleanly. Designed to integrate with systemd via `sd_notify`.
pub trait RuntimeManager: Send + Sync {
    /// Start all ingress servers and background tasks.
    ///
    /// Resolves when the runtime is fully started and ready to serve
    /// traffic. Implementations should emit `sd_notify READY=1` here.
    fn start(&self) -> BoxFuture<'_, RuntimeResult<()>>;

    /// Gracefully shut down: drain in-flight requests, stop servers,
    /// release resources. Implementations should emit
    /// `sd_notify STOPPING=1` before beginning teardown.
    fn shutdown(&self) -> BoxFuture<'_, RuntimeResult<()>>;

    /// Aggregate health across all wired components.
    fn health(&self) -> BoxFuture<'_, RuntimeHealth>;
}
