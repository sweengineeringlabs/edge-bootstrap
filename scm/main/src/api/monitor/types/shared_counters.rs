//! `SharedCounters` — `Arc`-wrapped `TrafficCounters` shared across request handlers.

use crate::api::monitor::types::traffic_counters::TrafficCounters;
use std::sync::Arc;

/// Shared handle passed between the monitor wrappers and the metrics server.
pub type SharedCounters = Arc<TrafficCounters>;
