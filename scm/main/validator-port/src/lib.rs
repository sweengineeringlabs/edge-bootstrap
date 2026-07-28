//! `swe-edge-bootstrap-validator-port` — domain validation ports (trait
//! contracts), zero implementation.
//!
//! The concrete implementation (`ConfigValidator`) lives in this repo's
//! `core/validator/` layer.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod traits;

pub use traits::{ConfigValidator, Validator};
