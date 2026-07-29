//! `swe-edge-bootstrap-health` — runtime health endpoint port (trait
//! contract), zero implementation.
//!
//! The concrete implementation (`DefaultHealthHandler`) lives in this
//! repo's `core/health/` layer.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod traits;

pub use traits::HealthHandler;
