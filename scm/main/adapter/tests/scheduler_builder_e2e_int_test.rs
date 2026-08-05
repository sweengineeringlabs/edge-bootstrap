//! Integration test — proves `RuntimeBuilder::with_scheduler()` actually
//! drives a scheduled job on a real interval through a served runtime, not
//! just that it constructs without panicking. Mirrors `grpc_e2e_int_test.rs`'s
//! pattern: spawn `Runtime::builder()...serve()`, let real wall-clock time
//! pass, assert on a real observable side effect, `handle.abort()`.
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
use swe_edge_bootstrap::{Runtime, RuntimeConfig, TokioSchedulerConfig};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct SchedulerE2ePayload(String);
impl edge_application::Request for SchedulerE2ePayload {}
impl edge_application::Response for SchedulerE2ePayload {}

struct PingHandler;

#[async_trait]
impl Handler for PingHandler {
    type Request = SchedulerE2ePayload;
    type Response = SchedulerE2ePayload;

    fn id(&self, _req: IdRequest) -> Result<IdResponse, HandlerError> {
        Ok(IdResponse {
            id: "/ping".to_string(),
        })
    }

    fn pattern(&self, _req: PatternRequest) -> Result<PatternResponse, HandlerError> {
        Ok(PatternResponse {
            pattern: "/ping".to_string(),
        })
    }

    async fn execute(
        &self,
        req: ExecutionRequest<'_, SchedulerE2ePayload>,
    ) -> Result<SchedulerE2ePayload, HandlerError> {
        Ok(req.req)
    }
}

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    listener.local_addr().expect("local_addr").port()
}

/// Regression test for `#34`: `with_scheduler` used to not exist at all —
/// enabling the `scheduler` feature added an error variant and nothing else.
/// This fails if the dedicated-thread wiring in `serve()` is ever broken or
/// removed, since it asserts on real tick counts advancing over real
/// wall-clock time, not a mocked/instant callback.
#[tokio::test]
async fn test_with_scheduler_drives_real_interval_job_happy() {
    let addr = format!("127.0.0.1:{}", free_port());
    let ticks = Arc::new(AtomicUsize::new(0));
    let ticks_job = Arc::clone(&ticks);

    let config = RuntimeConfig::default().with_http_bind(addr.clone());

    let handle = tokio::spawn(async move {
        Runtime::builder()
            .config(config)
            .http_route(Arc::new(PingHandler))
            .with_scheduler(TokioSchedulerConfig::default(), move |mut shutdown_rx| {
                Box::pin(async move {
                    let mut interval = tokio::time::interval(Duration::from_millis(100));
                    loop {
                        tokio::select! {
                            _ = interval.tick() => {
                                ticks_job.fetch_add(1, Ordering::SeqCst);
                            }
                            _ = &mut shutdown_rx => {
                                return Ok(());
                            }
                        }
                    }
                })
            })
            .serve()
            .await
    });

    // Give the server a moment to bind, then let real wall-clock time pass
    // across several 100ms ticks.
    tokio::time::sleep(Duration::from_millis(700)).await;

    let count = ticks.load(Ordering::SeqCst);
    assert!(
        count >= 4,
        "expected the scheduler job to have ticked at least 4 times over ~700ms at a 100ms \
         interval (proving it runs on a real, live interval), got {count} instead — the \
         scheduler thread may not be running"
    );

    handle.abort();
}

// A test proving the job's shutdown receiver fires during `serve()`'s own
// teardown was deliberately left out: `serve()` only reaches that path on a
// real SIGTERM/SIGINT (via `DaemonRunner::run_until_signal`), and there is no
// programmatic way to trigger it from outside — `handle.abort()` bypasses the
// shutdown path entirely rather than exercising it. Sending a real OS signal
// from within a test is possible but fragile and platform-specific (risks
// affecting the test process itself), so that specific path stays untested
// here rather than covered by a test that doesn't actually exercise it.
