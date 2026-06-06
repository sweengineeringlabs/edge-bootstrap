//! Runner contract — start, await signal, drain.

use crate::api::runtime::RuntimeResult;

/// Drives a [`RuntimeManager`] through start → signal → shutdown.
pub trait Runner: Send + Sync {
    /// Drive the runtime through start → signal → shutdown.
    fn run(&self) -> RuntimeResult<()>;
}
