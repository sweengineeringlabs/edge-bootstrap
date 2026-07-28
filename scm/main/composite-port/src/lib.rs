//! `swe-edge-bootstrap-composite-port` — composite inbound routing port
//! (trait contract), zero implementation.
//!
//! `CompositeGrpcIngress` (the concrete router) is not exposed here — its
//! routing logic is real behavior, not a contract, so it lives entirely in
//! this repo's `core/composite/` layer instead.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod traits;

pub use traits::CompositeIngress;
