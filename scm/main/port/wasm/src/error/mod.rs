//! Error types for Wasm component loading, validation, and invocation.

mod component_error;

pub use component_error::ComponentError;

/// Convenience alias for a `Result` whose error is [`ComponentError`].
pub type ComponentResult<T> = Result<T, ComponentError>;
