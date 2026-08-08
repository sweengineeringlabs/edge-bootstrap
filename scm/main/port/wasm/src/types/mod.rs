//! DTOs shared by [`crate::ComponentEngine`] and [`crate::ComponentValidator`].

mod artifact_provenance;
mod component_handle;
mod component_invoke;
mod component_load;
mod component_manifest;
mod component_validate;
mod resource_limits;

pub use artifact_provenance::ArtifactProvenance;
pub use component_handle::ComponentHandle;
pub use component_invoke::{ComponentInvokeRequest, ComponentInvokeResponse};
pub use component_load::{ComponentLoadRequest, ComponentLoadResponse};
pub use component_manifest::ComponentManifest;
pub use component_validate::ValidateComponentRequest;
pub use resource_limits::ResourceLimits;
