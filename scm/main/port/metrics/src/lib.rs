//! `swe-edge-bootstrap-metrics` — Prometheus exposition ports (trait
//! contracts), zero implementation.
//!
//! The concrete implementation (`MetricsHandler`) lives in this repo's
//! `core/metrics/` layer.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod traits;

pub use traits::{MetricsExporter, MetricsHandler};
