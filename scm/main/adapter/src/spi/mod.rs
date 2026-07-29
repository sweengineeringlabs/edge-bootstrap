//! SPI — service-provider interface extension hooks for downstream consumers.
//!
//! The presence of this directory signals that `saf/` factory functions
//! may return `impl Trait` for downstream polymorphism (SEA Rule 195).
//!
//! Downstream crates implementing custom runtime managers, lifecycle monitors,
//! or config loaders should implement the traits defined in `api/` and register
//! them via the factory functions in `saf/`.

pub(crate) mod runtime_extension;
pub(crate) use runtime_extension::RuntimeExtension;
