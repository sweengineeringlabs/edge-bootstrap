//! `swe-edge-bootstrap-runner-port` — lifecycle driver port (trait
//! contract), zero implementation.
//!
//! The concrete implementation (`DaemonRunner`) lives in this repo's
//! `core/runner/` layer.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod traits;

pub use traits::Runner;
