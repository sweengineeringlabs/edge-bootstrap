//! Wiring for the real `swe-edge-runtime-scheduler` executor — the one thing
//! this module owns is "how does `SchedulerSvc::tokio_scheduler` get spun up
//! and drive [`crate::job::run_scheduled_job`] alongside the served HTTP/gRPC
//! routes." Compiled only behind the `scheduler` feature.
//!
//! `swe-edge-runtime-scheduler` is not a cron/interval-registration API —
//! inspecting its source (`edge-scheduler` v0.3.6) shows `Scheduler` is a
//! runtime-agnostic contract for driving a single async future to completion:
//! `fn run<F>(&self, fut: F) -> Result<(), SchedulerError>` *blocks the
//! calling thread* until `fut` resolves, and `TokioScheduler::run` builds and
//! owns its own multi-thread tokio runtime internally
//! (`tokio::runtime::Builder::new_multi_thread()...block_on(fut)`). There is
//! no `RuntimeBuilder::with_scheduler()` hook in this version of the stack
//! (confirmed: `swe-edge-bootstrap-runtime`'s `scheduler` feature only adds a
//! `RuntimeError::Scheduler` variant, nothing else) — this crate's own
//! interval logic ([`crate::job`]) is what turns "drive one future to
//! completion" into "fire repeatedly on a clock."
//!
//! Because `Scheduler::run` is a blocking call that owns its own runtime, it
//! cannot be `.await`ed from inside `RuntimeBuilder::serve()`'s own async
//! runtime (nesting one tokio runtime inside another panics). A dedicated OS
//! thread is the idiomatic way to run it standalone, alongside the served
//! routes, in the same process — the same shape a consumer would use to run
//! any other blocking, self-contained executor next to `serve()`.

use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use swe_edge_bootstrap::{Scheduler, SchedulerSvc, TokioSchedulerConfig};

use crate::job;

/// How often the demo job fires.
pub(crate) const TICK_INTERVAL: Duration = Duration::from_millis(200);
/// How many times it fires before the scheduler thread exits on its own —
/// bounded so the demo never leaves a runaway background thread behind.
pub(crate) const MAX_TICKS: usize = 50;

/// Spin up a real `SchedulerSvc::tokio_scheduler` on its own OS thread and
/// drive [`job::run_scheduled_job`] on it. Returns the shared tick counter
/// [`crate::handler`] reads from, plus the thread's `JoinHandle` (not joined
/// here — the job is bounded and exits on its own; `main` doesn't block
/// startup waiting for a background job to finish, mirroring how
/// `messaging-e2e`'s subscriber task is spawned and left running).
pub(crate) fn spawn_scheduled_job() -> (Arc<AtomicUsize>, JoinHandle<()>) {
    let ticks = Arc::new(AtomicUsize::new(0));
    let ticks_for_job = Arc::clone(&ticks);

    let scheduler = SchedulerSvc::tokio_scheduler(TokioSchedulerConfig::default(), "scheduler-e2e");
    let join_handle = std::thread::Builder::new()
        .name("scheduler-e2e-job".to_string())
        .spawn(move || {
            if let Err(e) = scheduler.run(job::run_scheduled_job(
                ticks_for_job,
                TICK_INTERVAL,
                MAX_TICKS,
            )) {
                eprintln!("scheduler-e2e: scheduled job failed: {e}");
            }
        })
        .expect("spawn scheduler-e2e background thread");

    (ticks, join_handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    /// Real clock, real thread, real scheduler — proves `spawn_scheduled_job`
    /// actually gets the job running on genuine wall-clock time: nothing
    /// fires before the first `TICK_INTERVAL` elapses, and at least one tick
    /// has landed shortly after. If `SchedulerSvc::tokio_scheduler().run()`
    /// silently failed to execute the future, `ticks` would stay `0` forever
    /// and this test would fail.
    #[test]
    fn test_spawn_scheduled_job_increments_after_first_interval_elapses() {
        let (ticks, handle) = spawn_scheduled_job();
        assert_eq!(
            ticks.load(Ordering::SeqCst),
            0,
            "no tick should have fired before any time has passed"
        );

        std::thread::sleep(TICK_INTERVAL * 2);

        assert!(
            ticks.load(Ordering::SeqCst) >= 1,
            "at least one real interval tick should have fired by now"
        );
        drop(handle);
    }
}
