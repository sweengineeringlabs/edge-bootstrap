//! Testable daemon runner — start, await signal, shut down within timeout.

use std::time::Duration;

use crate::api::runtime::traits::runtime_manager::RuntimeManager;
use crate::api::runtime::{RuntimeError, RuntimeResult};

/// Zero-size orchestrator for the start → await-signal → shutdown cycle.
///
/// All methods are associated functions so they can be used as function
/// pointers where needed. Use `DaemonRunner::run_until_signal` to start.
pub(crate) struct DaemonRunner;

impl DaemonRunner {
    /// Start `manager`, await `signal`, then shut down within `shutdown_timeout_secs`.
    ///
    /// Consumers should use [`crate::saf::daemon::run`] or
    /// [`RuntimeBuilder::serve`](crate::api::runtime::RuntimeBuilder).
    pub(crate) async fn run_until_signal<F>(
        manager: impl RuntimeManager,
        shutdown_timeout_secs: u64,
        signal: F,
    ) -> RuntimeResult<()>
    where
        F: std::future::Future<Output = ()>,
    {
        manager.start().await?;
        tracing::info!("daemon ready — awaiting shutdown signal");
        signal.await;
        tracing::info!(
            timeout_secs = shutdown_timeout_secs,
            "shutdown signal received — draining"
        );
        tokio::time::timeout(
            Duration::from_secs(shutdown_timeout_secs),
            manager.shutdown(),
        )
        .await
        .map_err(|_| RuntimeError::ShutdownTimeout(shutdown_timeout_secs))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::runtime::{RuntimeHealth, RuntimeStatus};
    use futures::future::BoxFuture;

    struct DaemonRunnerOkManager;
    impl RuntimeManager for DaemonRunnerOkManager {
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

    struct DaemonRunnerFailManager;
    impl RuntimeManager for DaemonRunnerFailManager {
        fn start(&self) -> BoxFuture<'_, RuntimeResult<()>> {
            Box::pin(async { Err(RuntimeError::StartFailed("injected".into())) })
        }
        fn shutdown(&self) -> BoxFuture<'_, RuntimeResult<()>> {
            Box::pin(async { Ok(()) })
        }
        fn health(&self) -> BoxFuture<'_, RuntimeHealth> {
            Box::pin(async {
                RuntimeHealth {
                    status: RuntimeStatus::Stopped,
                    components: vec![],
                    uptime_secs: 0,
                }
            })
        }
    }

    struct DaemonRunnerHangManager;
    impl RuntimeManager for DaemonRunnerHangManager {
        fn start(&self) -> BoxFuture<'_, RuntimeResult<()>> {
            Box::pin(async { Ok(()) })
        }
        fn shutdown(&self) -> BoxFuture<'_, RuntimeResult<()>> {
            Box::pin(std::future::pending())
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
    fn test_run_until_signal_returns_send_future() {
        fn _assert_send<T: Send>(_: T) {}
        let fut = DaemonRunner::run_until_signal(DaemonRunnerOkManager, 30, std::future::ready(()));
        _assert_send(fut);
    }

    #[tokio::test]
    async fn test_run_until_signal_starts_and_shuts_down_cleanly() {
        let result =
            DaemonRunner::run_until_signal(DaemonRunnerOkManager, 30, std::future::ready(())).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_run_until_signal_propagates_start_failure() {
        let err =
            DaemonRunner::run_until_signal(DaemonRunnerFailManager, 30, std::future::ready(()))
                .await
                .unwrap_err();
        assert!(matches!(err, RuntimeError::StartFailed(_)));
    }

    #[tokio::test(start_paused = true)]
    async fn test_run_until_signal_returns_shutdown_timeout_when_drain_exceeds_limit() {
        use std::time::Duration;
        let timeout_secs = 5_u64;
        let fut = DaemonRunner::run_until_signal(
            DaemonRunnerHangManager,
            timeout_secs,
            std::future::ready(()),
        );
        tokio::pin!(fut);
        tokio::time::advance(Duration::from_secs(timeout_secs + 1)).await;
        let err = fut.await.unwrap_err();
        assert!(matches!(err, RuntimeError::ShutdownTimeout(5)));
    }
}
