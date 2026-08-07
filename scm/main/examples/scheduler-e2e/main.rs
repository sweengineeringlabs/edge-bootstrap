//! A `/scheduler/status` `Handler` served over HTTP *and* gRPC, backed by a
//! background job that `swe-edge-runtime-scheduler`'s real tokio-backed
//! executor ticks on a genuine wall-clock interval.
//!
//! Run:
//!     cargo run -p swe-edge-bootstrap --features scheduler --example scheduler-e2e
//!
//! Run with `scheduler` compiled out entirely (the route still serves, it
//! just always reports `ticks: 0` — no background job exists to increment
//! it):
//!     cargo run -p swe-edge-bootstrap --example scheduler-e2e
//!
//! Unlike `bootstrap-e2e` (which wires metrics, egress, IDS/IPS, and tracing
//! through `RuntimeBuilder`), this example is scoped to one thing: proving
//! `swe-edge-runtime-scheduler`'s `SchedulerSvc::tokio_scheduler` is a real,
//! live dependency that actually drives execution off a real clock — not a
//! declared-and-unused one, and not something that merely constructs without
//! panicking.
//!
//!   1. [`job`] owns the scheduled workload: a loop that ticks on a genuine
//!      `tokio::time::interval` and increments a shared `AtomicUsize` once
//!      per tick, bounded to a fixed number of ticks.
//!   2. [`scheduler_setup`] owns spinning up the real
//!      `SchedulerSvc::tokio_scheduler` on its own OS thread — see that
//!      module's doc comment for why a dedicated thread is required (the
//!      real `Scheduler::run` blocks the calling thread and builds its own
//!      tokio runtime, so it can't be `.await`ed inside `serve()`'s own
//!      runtime, and there's no `RuntimeBuilder::with_scheduler()` hook in
//!      this version of the stack).
//!   3. [`handler`] serves `/scheduler/status`, reporting the shared
//!      counter's *live* value on every request — proving the interval job
//!      and the served route are the same running process, not two
//!      independent demos glued together after the fact.
//!   4. [`runtime_config`] owns bind addresses.
//!
//! Split by responsibility rather than one `main()` doing everything, same
//! as `bootstrap-e2e`: each module owns exactly one piece of the wiring,
//! `main` only assembles them and serves.

mod handler;
#[cfg(feature = "scheduler")]
mod job;
mod runtime_config;
#[cfg(feature = "scheduler")]
mod scheduler_setup;

#[cfg(not(feature = "scheduler"))]
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

use swe_edge_bootstrap::Runtime;

#[tokio::main]
async fn main() {
    #[cfg(feature = "scheduler")]
    let (ticks, _scheduler_thread) = scheduler_setup::spawn_scheduled_job();
    #[cfg(not(feature = "scheduler"))]
    let ticks: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));

    let handler = handler::build_handler(Arc::clone(&ticks));
    let config = runtime_config::build_runtime_config();

    println!(
        "http: http://{}/scheduler/status   (POST {{}})",
        runtime_config::HTTP_BIND
    );
    println!("grpc: {} (reflection off)", runtime_config::GRPC_BIND);

    #[cfg(feature = "scheduler")]
    println!(
        "scheduler: real tokio-backed job ticking every {:?}, {} ticks total \
         (SchedulerSvc::tokio_scheduler running on its own thread)",
        scheduler_setup::TICK_INTERVAL,
        scheduler_setup::MAX_TICKS,
    );
    #[cfg(not(feature = "scheduler"))]
    println!(
        "scheduler feature disabled: no background job runs — \
         /scheduler/status will always report ticks=0"
    );

    let builder = Runtime::builder()
        .config(config)
        .http_route(handler.clone())
        .grpc_route(handler)
        .grpc_allow_unauthenticated();

    if let Err(e) = builder.serve().await {
        panic!("serve failed: {e}");
    }
}
