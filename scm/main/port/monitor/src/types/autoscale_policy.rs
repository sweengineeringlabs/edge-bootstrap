//! `AutoscalePolicy` — thresholds that trigger a scale-out signal.

use serde::{Deserialize, Serialize};

/// Thresholds that trigger a scale-out signal when exceeded.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoscalePolicy {
    /// Scale out when in-flight requests exceed this value.
    pub requests_active_max: u64,
    /// Scale out when requests-per-second exceeds this value.
    pub requests_per_sec_max: u64,
    /// Scale out when p99 latency (ms) exceeds this value.
    pub latency_p99_ms_max: f64,
}

impl Default for AutoscalePolicy {
    fn default() -> Self {
        Self {
            requests_active_max: 500,
            requests_per_sec_max: 1_000,
            latency_p99_ms_max: 200.0,
        }
    }
}
