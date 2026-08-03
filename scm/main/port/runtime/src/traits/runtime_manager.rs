//! RuntimeManager trait — owns the full process lifecycle.

use futures::future::BoxFuture;

use crate::types::health::RuntimeHealth;
use crate::RuntimeResult;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::runtime_status::RuntimeStatus;
    use std::sync::Arc;

    // Deliberately not a re-derivation of core/'s object-safety doubles
    // (AlwaysHealthyManager, DegradedManager, DaemonRunnerOkManager/FailManager/
    // HangManager) — those already prove RuntimeManager is object-safe with 5
    // independent implementors. This double proves something they can't: that
    // `RuntimeManager` is satisfiable using *only* this crate's own dependency
    // graph (no `edge_application`, no ingress/egress transports), i.e. that the
    // trait genuinely stands on its own outside the assembled `core/` crate.
    struct RuntimeManagerDouble;
    impl RuntimeManager for RuntimeManagerDouble {
        fn start(&self) -> BoxFuture<'_, RuntimeResult<()>> {
            Box::pin(async { Ok(()) })
        }
        fn shutdown(&self) -> BoxFuture<'_, RuntimeResult<()>> {
            Box::pin(async { Ok(()) })
        }
        fn health(&self) -> BoxFuture<'_, RuntimeHealth> {
            Box::pin(async {
                RuntimeHealth {
                    status: RuntimeStatus::Running,
                    components: vec![],
                    uptime_secs: 0,
                }
            })
        }
    }

    #[test]
    fn test_runtime_manager_double_is_object_safe_as_dyn_and_runs_standalone() {
        let d: Arc<dyn RuntimeManager> = Arc::new(RuntimeManagerDouble);
        let health = futures::executor::block_on(d.health());
        assert_eq!(health.status, RuntimeStatus::Running);
    }
}
