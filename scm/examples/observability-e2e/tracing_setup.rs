//! Tracing subscriber configuration — the one thing this module owns is
//! "how verbose is the trace output for this demo."

use swe_edge_bootstrap::{TracingConfig, TracingFormat, TracingLevel};

/// Debug-level, pretty-printed tracing — verbose enough to see the
/// handler's own span (`observed_handler`'s `execute` span, opened through
/// `HandlerContext.observer`) and every load-monitor node a request passes
/// through, on top of the structured entries landing in the file log drain
/// and the counters landing in the metrics backend.
pub(crate) fn build_tracing_config() -> TracingConfig {
    TracingConfig {
        level: TracingLevel::Debug,
        format: TracingFormat::Pretty,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_tracing_config_uses_debug_level() {
        let config = build_tracing_config();
        assert_eq!(config.level, TracingLevel::Debug);
    }

    #[test]
    fn test_build_tracing_config_is_enabled_and_pretty_printed() {
        let config = build_tracing_config();
        assert!(config.enabled, "tracing subscriber must be enabled");
        assert_eq!(config.format, TracingFormat::Pretty);
    }
}
