//! Trait contracts for loading, validating, and invoking Wasm component
//! handlers.

mod component_engine;
mod component_validator;

pub use component_engine::ComponentEngine;
pub use component_validator::ComponentValidator;
