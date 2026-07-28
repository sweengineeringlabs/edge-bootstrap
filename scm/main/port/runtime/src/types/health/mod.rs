//! Runtime health types.

pub mod component_health;
#[allow(clippy::module_inception)]
pub mod runtime_health;

pub use component_health::ComponentHealth;
pub use runtime_health::RuntimeHealth;
