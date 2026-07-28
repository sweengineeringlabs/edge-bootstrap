use std::sync::atomic::Ordering;

use swe_edge_bootstrap_monitor::{AutoscalePolicy, SharedCounters};

/// Ticks every second: pushes derived gauges into the provider and checks
/// autoscale thresholds.
pub(crate) struct BackgroundSampler {
    counters: SharedCounters,
    policy: Option<AutoscalePolicy>,
}

impl swe_edge_bootstrap_monitor::Sampler for BackgroundSampler {
    fn counters(&self) -> &SharedCounters {
        &self.counters
    }
}

impl BackgroundSampler {
    pub(crate) fn new(counters: SharedCounters, policy: Option<AutoscalePolicy>) -> Self {
        Self { counters, policy }
    }

    pub(crate) async fn run(self) {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            interval.tick().await;

            let active = self.counters.requests_in_flight.load(Ordering::Relaxed) as f64;
            let rps = self.counters.requests_since_tick.swap(0, Ordering::Relaxed) as f64;
            let eps = self.counters.errors_since_tick.swap(0, Ordering::Relaxed) as f64;
            let p99 = self.counters.latency_ring.lock().p99_ms();

            let p = &*self.counters.provider;
            p.record_gauge("edge_requests_active", active, &[]);
            p.record_gauge("edge_requests_per_second", rps, &[]);
            p.record_gauge("edge_errors_per_second", eps, &[]);
            p.record_gauge("edge_request_latency_p99_ms", p99, &[]);

            if let Some(ref policy) = self.policy {
                if active as u64 > policy.requests_active_max {
                    tracing::warn!(
                        active,
                        max = policy.requests_active_max,
                        "scale-out signal: requests_active exceeded threshold"
                    );
                }
                if rps as u64 > policy.requests_per_sec_max {
                    tracing::warn!(
                        rps,
                        max = policy.requests_per_sec_max,
                        "scale-out signal: requests_per_second exceeded threshold"
                    );
                }
                if p99 > policy.latency_p99_ms_max {
                    tracing::warn!(
                        p99_ms = p99,
                        max = policy.latency_p99_ms_max,
                        "scale-out signal: latency_p99_ms exceeded threshold"
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swe_edge_bootstrap_monitor::TrafficCounters;
    use std::sync::Arc;
    use swe_observ_metrics::create_local_metrics_backend;

    fn counters() -> SharedCounters {
        Arc::new(TrafficCounters::new(Arc::new(
            create_local_metrics_backend(),
        )))
    }

    #[test]
    fn test_background_sampler_new_does_not_panic() {
        let _s = BackgroundSampler::new(counters(), None);
    }

    #[test]
    fn test_sampler_counters_is_same_instance() {
        use swe_edge_bootstrap_monitor::Sampler;
        let c = counters();
        let s = BackgroundSampler::new(Arc::clone(&c), None);
        assert!(Arc::ptr_eq(s.counters(), &c));
    }

    #[test]
    fn test_run_returns_send_future() {
        fn _assert_send<T: Send>(_: T) {}
        let s = BackgroundSampler::new(counters(), None);
        _assert_send(s.run());
    }

    #[tokio::test(start_paused = true)]
    async fn test_run_records_gauges_after_one_tick() {
        use std::time::Duration;
        let c = counters();
        c.on_start();
        let sampler = BackgroundSampler::new(Arc::clone(&c), None);
        let handle = tokio::spawn(sampler.run());
        // Yield once to let the task start and reach the first interval.tick().await
        tokio::task::yield_now().await;
        // Advance past the 1-second interval so the tick fires
        tokio::time::advance(Duration::from_secs(2)).await;
        // Yield again to let the task process the tick
        tokio::task::yield_now().await;
        handle.abort();
        let snaps = c.provider.export();
        assert!(
            snaps.iter().any(|s| s.name == "edge_requests_active"),
            "expected edge_requests_active gauge after tick"
        );
    }
}
