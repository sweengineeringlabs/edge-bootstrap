//! `swe-edge-bootstrap-wasm` — Wasm component handler port
//! (`ComponentEngine`/`ComponentValidator` traits + DTOs), zero
//! implementation, per ADR-007.
//!
//! The concrete `wasmtime`-backed engine is **not** exposed here — that is
//! real implementation, not a contract, and per ADR-007 lives in
//! `main/adapter`'s `core/wasm/` module behind the `wasm` Cargo feature
//! instead. This crate stays free of any Wasm-runtime dependency so its
//! manifest/validation contract is testable and swappable independent of
//! which engine (if any) is linked in.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod traits;
pub mod types;

pub use error::{ComponentError, ComponentResult};
pub use traits::{ComponentEngine, ComponentValidator};
pub use types::{
    ArtifactProvenance, ComponentHandle, ComponentInvokeRequest, ComponentInvokeResponse,
    ComponentLoadRequest, ComponentLoadResponse, ComponentManifest, ResourceLimits,
    ValidateComponentRequest,
};
