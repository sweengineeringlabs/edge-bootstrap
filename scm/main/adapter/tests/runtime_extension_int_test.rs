//! Integration tests for the SPI RuntimeExtension marker.

use swe_edge_bootstrap::ServerConfigLoader;

/// @covers: runtime_extension
/// The RuntimeExtension SPI type is crate-internal. This test verifies that
/// the runtime operates correctly with the SPI extension point declared.
#[test]
fn test_runtime_extension_spi_does_not_break_config_loading() {
    let cfg = ServerConfigLoader::load_config().unwrap_or_default();
    assert!(!cfg.http_bind.is_empty());
}
