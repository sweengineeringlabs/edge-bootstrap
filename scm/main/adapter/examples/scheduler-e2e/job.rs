//! The scheduled workload — the one thing this module owns is "what job
//! runs, how often, how many times, and what side effect it produces."
//! Compiled only behind the `scheduler` feature; see `scheduler_setup` for
//! how it gets driven by the real `swe-edge-runtime-scheduler` executor.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

// `SchedulerError` isn't re-exported from `swe_edge_bootstrap` (only
// `Scheduler`/`SchedulerSvc`/`TokioSchedulerConfig` are), so it comes
// straight from the real upstream crate, which is already a normal
// (optional, feature-gated) dependency of this package.
use swe_edge_runtime_scheduler::SchedulerError;

/// Runs on whatever tokio runtime `SchedulerSvc::tokio_scheduler` builds
/// (see `scheduler_setup::spawn_scheduled_job`): ticks on a genuine
/// `tokio::time::interval` — not an instantly-resolving future — and
/// increments `ticks` exactly once per tick, `max_ticks` times, then
/// returns. Bounded so the dedicated scheduler thread this runs on always
/// terminates rather than looping forever.
///
/// `interval_at` (rather than plain `interval`) is used deliberately: a bare
/// `tokio::time::interval` resolves its *first* tick immediately, which
/// would let a broken/instant scheduler pass this proof by accident. Seeding
/// the first deadline at `now + interval` means every single tick — including
/// the first — requires real wall-clock time to actually elapse.
pub(crate) async fn run_scheduled_job(
    ticks: Arc<AtomicUsize>,
    interval: Duration,
    max_ticks: usize,
) -> Result<(), SchedulerError> {
    let first_deadline = tokio::time::Instant::now() + interval;
    let mut ticker = tokio::time::interval_at(first_deadline, interval);
    for tick_number in 1..=max_ticks {
        ticker.tick().await;
        let total = ticks.fetch_add(1, Ordering::SeqCst) + 1;
        println!("scheduler-e2e: job tick {tick_number}/{max_ticks} fired (total={total})");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real clock test, not a mock: `start_paused = true` gives tokio's
    /// *simulated* time source control of `tokio::time::interval`'s ticks
    /// directly (auto-advancing whenever every task is idle waiting on a
    /// timer) rather than faking the counter increments themselves — the
    /// job's actual tick-then-increment logic still runs, unmodified.
    #[tokio::test(start_paused = true)]
    async fn test_run_scheduled_job_increments_exactly_max_ticks_times() {
        let ticks = Arc::new(AtomicUsize::new(0));
        run_scheduled_job(Arc::clone(&ticks), Duration::from_millis(10), 5)
            .await
            .expect("job must complete without error");
        assert_eq!(ticks.load(Ordering::SeqCst), 5);
    }

    #[tokio::test(start_paused = true)]
    async fn test_run_scheduled_job_does_not_increment_before_first_interval_elapses() {
        let ticks = Arc::new(AtomicUsize::new(0));
        let job = tokio::spawn(run_scheduled_job(
            Arc::clone(&ticks),
            Duration::from_millis(100),
            1,
        ));
        // Yield once without advancing the paused clock — the job must not
        // have incremented yet, proving the first tick genuinely waits for
        // `interval` rather than resolving immediately.
        tokio::task::yield_now().await;
        assert_eq!(
            ticks.load(Ordering::SeqCst),
            0,
            "job must not tick before its first interval has elapsed"
        );
        job.await
            .expect("job task must not panic")
            .expect("job must not error");
        assert_eq!(ticks.load(Ordering::SeqCst), 1);
    }
}
