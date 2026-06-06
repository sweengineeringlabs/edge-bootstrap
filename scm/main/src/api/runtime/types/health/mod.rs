//! Runtime health types.

pub(crate) mod component_health;
#[allow(clippy::module_inception)]
pub(crate) mod runtime_health;

pub use component_health::ComponentHealth;
pub use runtime_health::RuntimeHealth;
