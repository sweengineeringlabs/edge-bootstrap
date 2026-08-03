//! Metric naming constants for lifecycle-monitor health gauges.

/// Prometheus-style metric name emitted by `MetricsLifecycleMonitor` for
/// each component health poll.
pub const LIFECYCLE_HEALTH_GAUGE: &str = "edge_component_health";
