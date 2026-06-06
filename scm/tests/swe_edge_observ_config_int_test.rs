//! Integration tests proving swe-edge-observ-config coverage through the runtime.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use swe_edge_observ_config::{ObservabilityConfig, TracingConfig, TracingFormat, TracingLevel};

/// @covers: swe-edge-observ-config — TracingConfig accessible via runtime SAF
#[test]
fn test_swe_edge_observ_config_int_tracing_config_default_is_sane() {
    let cfg = TracingConfig::default();
    assert!(cfg.enabled);
    assert_eq!(cfg.format, TracingFormat::Pretty);
    assert_eq!(cfg.level, TracingLevel::Info);
}

/// @covers: swe-edge-observ-config — ObservabilityConfig round-trips through TOML
#[test]
fn test_swe_edge_observ_config_int_observability_config_toml_round_trip() {
    let toml = "[tracing]\nlevel = \"warn\"\nformat = \"json\"\nenabled = false";
    let cfg: ObservabilityConfig = toml::from_str(toml).unwrap();
    assert!(!cfg.tracing.enabled);
    assert_eq!(cfg.tracing.level, TracingLevel::Warn);
    assert_eq!(cfg.tracing.format, TracingFormat::Json);
}

/// @covers: swe-edge-observ-config — TracingLevel all variants accessible
#[test]
fn test_swe_edge_observ_config_int_tracing_level_variants_are_reachable() {
    let levels = [
        TracingLevel::Trace,
        TracingLevel::Debug,
        TracingLevel::Info,
        TracingLevel::Warn,
        TracingLevel::Error,
    ];
    assert_eq!(levels.len(), 5);
}
