//! Integration tests for `TracingInitializer` and `TracingFormat`.

#[cfg(feature = "observability")]
use swe_edge_bootstrap::{TracingConfig, TracingInitializer};

/// @covers: TracingInitializer::init — Json format installs without panic
#[cfg(feature = "observability")]
#[test]
fn test_observability_struct_init_tracing_json_installs_without_panic() {
    use swe_edge_observ_config::TracingFormat;
    let cfg = TracingConfig {
        format: TracingFormat::Json,
        ..TracingConfig::default()
    };
    TracingInitializer::init(&cfg);
}

/// @covers: TracingInitializer::init — Pretty format installs without panic
#[cfg(feature = "observability")]
#[test]
fn test_observability_struct_init_tracing_pretty_installs_without_panic() {
    use swe_edge_observ_config::TracingFormat;
    let cfg = TracingConfig {
        format: TracingFormat::Pretty,
        ..TracingConfig::default()
    };
    TracingInitializer::init(&cfg);
}

/// @covers: TracingInitializer::init — idempotent across multiple calls
#[cfg(feature = "observability")]
#[test]
fn test_observability_struct_init_tracing_called_twice_is_idempotent() {
    use swe_edge_observ_config::TracingFormat;
    let json = TracingConfig {
        format: TracingFormat::Json,
        ..TracingConfig::default()
    };
    let pretty = TracingConfig {
        format: TracingFormat::Pretty,
        ..TracingConfig::default()
    };
    TracingInitializer::init(&json);
    TracingInitializer::init(&pretty);
}
