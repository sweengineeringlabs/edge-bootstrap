//! `MetricsConfig` — Prometheus endpoint bind address and path.

use serde::{Deserialize, Serialize};

/// Configuration for the Prometheus metrics endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MetricsConfig {
    /// Bind address for the metrics server (separate from `http_bind`).
    pub bind: String,
    /// Path served by the metrics endpoint.
    pub path: String,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:9090".into(),
            path: "/metrics".into(),
        }
    }
}
