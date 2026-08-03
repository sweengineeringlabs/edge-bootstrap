//! Tracing subscriber configuration — the one thing this module owns is
//! "how verbose is the trace output for this demo."

use swe_edge_bootstrap::{TracingConfig, TracingLevel};

/// Debug-level, pretty-printed tracing — verbose enough to see every node
/// in the request pipeline (`http_load_monitor`, `http_intrusion_guard`,
/// `echo_handler`) announce itself as a request passes through.
pub(crate) fn build_tracing_config() -> TracingConfig {
    TracingConfig {
        level: TracingLevel::Debug,
        ..Default::default()
    }
}
