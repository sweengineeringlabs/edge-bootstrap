//! Integration tests for swe-edge-runtime-scheduler dependency coverage.

use swe_edge_runtime_scheduler::{SchedulerSvc, TokioSchedulerConfig};

/// @covers: swe-edge-runtime-scheduler
#[test]
fn test_tokio_scheduler_constructs_with_default_config() {
    let cfg = TokioSchedulerConfig::default();
    let _sched = SchedulerSvc::tokio_scheduler(cfg, "test");
}
