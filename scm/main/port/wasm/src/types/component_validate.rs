//! Request DTO for [`crate::ComponentValidator::validate`].

use crate::types::component_manifest::ComponentManifest;

/// Request to validate a component's manifest and declared imports/exports
/// against the versioned `swe:edge-handler` WIT contract, before the
/// corresponding route is ever registered.
#[derive(Debug, Clone)]
pub struct ValidateComponentRequest {
    /// The manifest to validate.
    pub manifest: ComponentManifest,
    /// The raw Wasm component binary — needed to check its actual
    /// import/export surface against `manifest.contract_version`, not just
    /// the manifest's self-reported claims.
    pub component_bytes: Vec<u8>,
}
