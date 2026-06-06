//! Integration tests for ConfigError.

use swe_edge_bootstrap::ConfigError;

/// @covers: ConfigError
#[test]
fn test_config_error_parse_variant_displays_correctly() {
    let e = ConfigError::Parse("bad toml".into());
    assert!(e.to_string().contains("parse error"));
}

/// @covers: ConfigError
#[test]
fn test_config_error_unknown_tenant_displays_tenant_id() {
    let e = ConfigError::UnknownTenant("ghost".into());
    assert!(e.to_string().contains("ghost"));
}

/// @covers: ConfigError
#[test]
fn test_config_error_io_variant_displays_correctly() {
    let e = ConfigError::Io("no such file".into());
    assert!(e.to_string().contains("io error"));
}
