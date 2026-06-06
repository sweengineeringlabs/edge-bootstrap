//! `RuntimeHealth` — aggregate health of the running daemon.

use crate::api::runtime::types::health::component_health::ComponentHealth;
use crate::api::runtime::RuntimeStatus;

/// Aggregate health report for the runtime.
#[derive(Debug, Clone)]
pub struct RuntimeHealth {
    /// Overall runtime status (Starting, Running, Stopping, Stopped).
    pub status: RuntimeStatus,
    /// Per-subsystem health snapshots.
    pub components: Vec<ComponentHealth>,
    /// Seconds elapsed since the runtime started.
    pub uptime_secs: u64,
}

impl RuntimeHealth {
    /// Returns `true` when status is `Running` and every component is healthy.
    pub fn is_healthy(&self) -> bool {
        self.status.is_healthy() && self.components.iter().all(|c| c.healthy)
    }
}
