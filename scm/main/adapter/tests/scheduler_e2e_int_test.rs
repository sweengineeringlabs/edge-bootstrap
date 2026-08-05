//! Integration test — `scheduler` feature: proves `swe-edge-runtime-scheduler`
//! (`SchedulerSvc::tokio_scheduler`) actually drives a job off a real
//! clock/interval when served alongside `RuntimeBuilder::serve()`, not just
//! that it constructs without panicking.
//!
//! Inspecting the real upstream crate's source (`edge-scheduler` v0.3.6)
//! shows `Scheduler` is a runtime-agnostic contract for driving one async
//! future to completion — `fn run<F>(&self, fut: F) -> Result<(),
//! SchedulerError>` *blocks the calling thread* until `fut` resolves, and
//! `TokioScheduler::run` builds and owns its own multi-thread tokio runtime
//! internally. There is no `RuntimeBuilder::with_scheduler()` hook in this
//! version of the stack (`swe-edge-bootstrap-runtime`'s `scheduler` feature
//! only adds a `RuntimeError::Scheduler` variant) — so this test spins the
//! real scheduler up on its own OS thread, the same standalone-alongside-
//! `serve()` shape `examples/scheduler-e2e` uses, since a blocking call that
//! owns its own runtime can't be `.await`ed inside `serve()`'s own runtime.
//!
//! Mirrors `grpc_e2e_int_test.rs`'s pattern — spawn `serve()` in a
//! `tokio::spawn`, settle briefly, drive it with a real client, assert on the
//! real response, `handle.abort()` — but the "real client" assertion here is
//! specific to a scheduler: schedule a job on a short real interval, let real
//! wall-clock time pass (`tokio::time::sleep`), and confirm the *served
//! route* reports an `AtomicUsize` tick counter that has genuinely advanced
//! — proving the scheduler's real clock, not a mocked/instant callback,
//! drives execution. If `Scheduler::run` were a no-op, or returned before
//! the future actually ran, this counter would never move off zero.
#![cfg(feature = "scheduler")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use edge_application::{Handler, HandlerError};
use edge_application_handler::{
    ExecutionRequest, IdRequest, IdResponse, PatternRequest, PatternResponse,
};
use swe_edge_bootstrap::{Runtime, RuntimeConfig, Scheduler, SchedulerSvc, TokioSchedulerConfig};
use swe_edge_runtime_scheduler::SchedulerError;

/// Real wall-clock tick cadence — short enough that several ticks land well
/// within this test's lifetime, long enough that CI scheduling jitter can't
/// plausibly be mistaken for an instant/mocked callback.
const TICK_INTERVAL: Duration = Duration::from_millis(100);
/// Bounded so the dedicated scheduler thread this test spawns always
/// terminates rather than leaking a runaway background thread.
const MAX_TICKS: usize = 30;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct StatusQuery {}
impl edge_application::Request for StatusQuery {}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct StatusResponse {
    ticks: usize,
}
impl edge_application::Response for StatusResponse {}

struct StatusHandler {
    ticks: Arc<AtomicUsize>,
}

#[async_trait]
impl Handler for StatusHandler {
    type Request = StatusQuery;
    type Response = StatusResponse;

    fn id(&self, _req: IdRequest) -> Result<IdResponse, HandlerError> {
        // Must carry the leading `/` — the server keys routes on
        // `req.uri().path()` (see edge-runtime-grpc-adapter's
        // `tonic_server_dispatcher.rs`), which always includes it.
        Ok(IdResponse {
            id: "/scheduler/status".to_string(),
        })
    }

    fn pattern(&self, _req: PatternRequest) -> Result<PatternResponse, HandlerError> {
        // Required for the HTTP route: `HttpIngressJob` matches incoming
        // requests against registered `matchit`-style patterns, not `id()`
        // alone — omitting this (fine for the gRPC-only route in
        // `grpc_e2e_int_test.rs`) makes an HTTP route 404 with "no intent
        // matched" even though the handler itself is registered correctly.
        Ok(PatternResponse {
            pattern: "/scheduler/status".to_string(),
        })
    }

    async fn execute(
        &self,
        _req: ExecutionRequest<'_, StatusQuery>,
    ) -> Result<StatusResponse, HandlerError> {
        Ok(StatusResponse {
            ticks: self.ticks.load(Ordering::SeqCst),
        })
    }
}

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    listener.local_addr().expect("local_addr").port()
}

/// Runs on the real tokio runtime `SchedulerSvc::tokio_scheduler` builds:
/// ticks on a genuine `tokio::time::interval` (seeded so the *first* tick
/// also requires a full `interval` of real time, not an instant resolution)
/// and increments `ticks` once per tick, `MAX_TICKS` times, then returns.
async fn run_scheduled_job(ticks: Arc<AtomicUsize>) -> Result<(), SchedulerError> {
    let first_deadline = tokio::time::Instant::now() + TICK_INTERVAL;
    let mut ticker = tokio::time::interval_at(first_deadline, TICK_INTERVAL);
    for _ in 0..MAX_TICKS {
        ticker.tick().await;
        ticks.fetch_add(1, Ordering::SeqCst);
    }
    Ok(())
}

/// Spawns the real `SchedulerSvc::tokio_scheduler` on its own OS thread and
/// drives [`run_scheduled_job`] on it, exactly as `examples/scheduler-e2e`
/// does — `Scheduler::run` blocks the calling thread, so it cannot run on
/// the test's own tokio runtime.
fn spawn_real_scheduler(ticks: Arc<AtomicUsize>) -> std::thread::JoinHandle<()> {
    let scheduler =
        SchedulerSvc::tokio_scheduler(TokioSchedulerConfig::default(), "scheduler-e2e-test");
    std::thread::Builder::new()
        .name("scheduler-e2e-test".to_string())
        .spawn(move || {
            if let Err(e) = scheduler.run(run_scheduled_job(ticks)) {
                eprintln!("scheduler-e2e-test: scheduled job failed: {e}");
            }
        })
        .expect("spawn scheduler thread")
}

#[tokio::test]
async fn test_serve_with_scheduler_reports_increasing_ticks_from_real_interval_happy() {
    let ticks = Arc::new(AtomicUsize::new(0));
    let _scheduler_thread = spawn_real_scheduler(Arc::clone(&ticks));

    let addr = format!("127.0.0.1:{}", free_port());
    let config = RuntimeConfig::default().with_http_bind(addr.clone());
    let handler = Arc::new(StatusHandler {
        ticks: Arc::clone(&ticks),
    });

    let handle = tokio::spawn(async move {
        Runtime::builder()
            .config(config)
            .http_route(handler)
            .serve()
            .await
    });

    // Give the server a moment to bind before connecting.
    tokio::time::sleep(Duration::from_millis(200)).await;

    let client = reqwest::Client::new();

    let first: StatusResponse = client
        .post(format!("http://{addr}/scheduler/status"))
        .json(&StatusQuery {})
        .send()
        .await
        .expect("first request must reach the real served handler")
        .json()
        .await
        .expect("response body must be valid JSON matching StatusResponse");

    // Poll for further real ticks rather than asserting after one fixed
    // sleep — tolerant of scheduling jitter on a busy machine, while a
    // scheduler that never actually fires still fails this outright once
    // the deadline below elapses (this is a bound, not an early-exit that
    // would let a broken scheduler slip through).
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let second: StatusResponse = loop {
        let resp: StatusResponse = client
            .post(format!("http://{addr}/scheduler/status"))
            .json(&StatusQuery {})
            .send()
            .await
            .expect("polling request must reach the real served handler")
            .json()
            .await
            .expect("response body must be valid JSON matching StatusResponse");
        if resp.ticks >= first.ticks + 2 {
            break resp;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "expected at least 2 additional real interval ticks (at a {TICK_INTERVAL:?} \
             cadence) within 5s of real wall-clock time, but only reached {} starting from {} \
             — if the scheduler's real clock weren't actually driving job execution, this \
             count would never advance",
            resp.ticks,
            first.ticks
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    };

    assert!(
        second.ticks > first.ticks,
        "tick count must have advanced between the two requests (first={}, second={}) — \
         if the scheduler's real clock weren't actually driving job execution, both \
         requests would observe the same (or a fully pre-populated) count",
        first.ticks,
        second.ticks
    );

    handle.abort();
}

#[tokio::test]
async fn test_scheduler_job_does_not_fire_before_first_interval_elapses_happy() {
    let ticks = Arc::new(AtomicUsize::new(0));
    let _scheduler_thread = spawn_real_scheduler(Arc::clone(&ticks));

    // Read immediately — well under one `TICK_INTERVAL` of real time has
    // passed, so a genuine interval-driven job must not have ticked yet.
    // A scheduler that (incorrectly) ran the job's first iteration instantly
    // rather than waiting on the real clock would fail this assertion.
    assert_eq!(
        ticks.load(Ordering::SeqCst),
        0,
        "no tick should fire before the first real interval elapses"
    );

    // Poll up to a generous bound rather than a single fixed sleep —
    // tolerant of scheduling jitter on a busy machine, while a scheduler
    // that never fires still fails this outright once the deadline elapses.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if ticks.load(Ordering::SeqCst) >= 1 {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "at least one real interval tick should have fired within 5s of real \
             wall-clock time at a {TICK_INTERVAL:?} cadence"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
