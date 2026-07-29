//! Runner contract — start, await signal, drain.

use swe_edge_bootstrap_runtime::{RuntimeManager, RuntimeResult};

/// Drives a [`RuntimeManager`] through start → signal → shutdown.
pub trait Runner: Send + Sync {
    /// The runtime manager this runner drives through its lifecycle.
    type Manager: RuntimeManager;

    /// Drive [`Self::Manager`] through start → signal → shutdown.
    fn run(&self) -> RuntimeResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::future::BoxFuture;
    use swe_edge_bootstrap_runtime::{RuntimeHealth, RuntimeStatus};

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

    struct RunnerDouble {
        manager: RuntimeManagerDouble,
    }
    impl Runner for RunnerDouble {
        type Manager = RuntimeManagerDouble;

        fn run(&self) -> RuntimeResult<()> {
            futures::executor::block_on(async {
                self.manager.start().await?;
                self.manager.shutdown().await
            })
        }
    }

    #[test]
    fn test_runner_double_manager_associated_type_bound_is_satisfiable() {
        let r = RunnerDouble {
            manager: RuntimeManagerDouble,
        };
        assert!(r.run().is_ok());
    }
}
