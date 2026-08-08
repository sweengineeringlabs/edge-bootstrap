//! Request/response DTOs for [`crate::ComponentEngine::load`].

use crate::types::component_handle::ComponentHandle;
use crate::types::component_manifest::ComponentManifest;

/// Request to load a validated component artifact into the engine.
///
/// Callers must run [`crate::ComponentValidator::validate`] against
/// `manifest`/`component_bytes` first — `load` itself is not required to
/// re-validate the manifest, only to fail if the bytes cannot be
/// instantiated per the (already-validated) manifest's declared contract.
#[derive(Debug, Clone)]
pub struct ComponentLoadRequest {
    /// The component's manifest, already validated by
    /// [`crate::ComponentValidator`].
    pub manifest: ComponentManifest,
    /// The raw Wasm component binary.
    pub component_bytes: Vec<u8>,
}

/// Response from a successful [`crate::ComponentEngine::load`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentLoadResponse {
    /// Opaque handle to the now-loaded component instance, to be passed to
    /// [`crate::ComponentEngine::invoke`].
    pub handle: ComponentHandle,
}
