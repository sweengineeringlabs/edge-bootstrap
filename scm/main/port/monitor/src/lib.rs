//! `swe-edge-bootstrap-monitor` — load-monitoring ports (traits + DTOs
//! + value objects), zero implementation.
//!
//! Consumed by `swe-edge-bootstrap-runtime` (`AutoscalePolicy`,
//! `MetricsConfig` embedded in `RuntimeConfig`) and
//! `swe-edge-bootstrap-metrics` (`SharedCounters`). The concrete
//! implementation lives in this repo's `core/monitor/` layer.

#![deny(unsafe_code)]
#![warn(missing_docs)]
// `TrafficCountersBuilder` is unused scaffolding carried over verbatim from
// the pre-split `api/monitor/types/traffic_counters.rs` — not introduced here.
#![allow(dead_code)]

pub mod traits;
pub mod types;
pub mod vo;

pub use traits::{GrpcLoadMonitor, HttpLoadMonitor, LifecycleObserver, Sampler};
pub use types::{AutoscalePolicy, MetricsConfig, RingBuffer, SharedCounters, TrafficCounters};
