//! API declarations for the Prometheus metrics HTTP handler.

use crate::api::monitor::SharedCounters;

/// Contract for serving the Prometheus text exposition endpoint.
pub trait MetricsExporter: Send + Sync {
    /// Bind address and path are determined by [`crate::api::monitor::MetricsConfig`].
    fn counters(&self) -> &SharedCounters;
    fn path(&self) -> &str;
}
