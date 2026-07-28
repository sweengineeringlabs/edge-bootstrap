//! `swe-edge-bootstrap-ingress-port` — ingress adapter port (trait
//! contract), zero implementation.
//!
//! The concrete implementation (`DefaultIngress`) lives in this repo's
//! `core/ingress/` layer.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod traits;

pub use traits::Ingress;
