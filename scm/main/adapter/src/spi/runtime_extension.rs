//! [`RuntimeExtension`] — SPI hook for downstream runtime extensions.

/// Marker extension point for downstream crates that extend the runtime.
///
/// Implement this on a zero-size struct to register your extension with
/// the `swe-edge-runtime-server` SPI surface.
pub(crate) struct RuntimeExtension;
