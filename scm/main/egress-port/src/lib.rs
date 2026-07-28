//! `swe-edge-bootstrap-egress-port` — egress adapter port (trait contract),
//! zero implementation.
//!
//! The concrete implementation (`DefaultEgress`) lives in this repo's
//! `core/egress/` layer.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod traits;

pub use traits::Egress;
