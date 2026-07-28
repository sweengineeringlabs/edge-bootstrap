//! Runner contract — start, await signal, drain.

use swe_edge_bootstrap_runtime_port::RuntimeResult;

/// Drives a `RuntimeManager` through start → signal → shutdown.
pub trait Runner: Send + Sync {
    /// Drive the runtime through start → signal → shutdown.
    fn run(&self) -> RuntimeResult<()>;
}
