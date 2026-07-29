//! `swe-edge-bootstrap-intrusion` — intrusion-guard ports (traits), zero
//! implementation.
//!
//! Consumed by `swe-edge-bootstrap-runtime` when the `intrusion` feature is
//! enabled. The concrete implementation lives in this repo's
//! `core/intrusion/` layer and wraps the actual detection/enforcement logic
//! from the external `edge-intrusion` crate — see that repo's ADR-001/ADR-002
//! for why detection and enforcement stay there rather than here.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod traits;

pub use traits::{GrpcIntrusionGuard, HttpIntrusionGuard};
