//! [`MetricsBridgeRegistry`] — real `MetricRegistry` backed by
//! `swe-observability-metrics`'s `MetricsProvider`. Reuses whichever
//! provider instance the caller passes in — the same one `TrafficCounters`
//! records into — so metrics recorded through `HandlerContext.observer`
//! land in the exact same export stream as the load monitor's own counters.

use std::sync::Arc;

use edge_application_observer::{
    Counter, CounterLookupRequest, CounterLookupResponse, Gauge, GaugeLookupRequest,
    GaugeLookupResponse, GaugeSetRequest, GaugeSetResponse, Histogram, HistogramLookupRequest,
    HistogramLookupResponse, HistogramRecordRequest, HistogramRecordResponse, IncrementRequest,
    IncrementResponse, MetricRegistry, ObserveError,
};
use swe_observ_metrics::MetricsProvider;

pub(crate) struct MetricsBridgeRegistry {
    backend: Arc<dyn MetricsProvider>,
}

impl MetricsBridgeRegistry {
    pub(crate) fn new(backend: Arc<dyn MetricsProvider>) -> Self {
        Self { backend }
    }
}

impl MetricRegistry for MetricsBridgeRegistry {
    fn counter(&self, req: CounterLookupRequest) -> Result<CounterLookupResponse, ObserveError> {
        Ok(CounterLookupResponse {
            counter: Box::new(MetricsBridgeCounter {
                name: req.name,
                backend: Arc::clone(&self.backend),
            }),
        })
    }

    fn histogram(
        &self,
        req: HistogramLookupRequest,
    ) -> Result<HistogramLookupResponse, ObserveError> {
        Ok(HistogramLookupResponse {
            histogram: Box::new(MetricsBridgeHistogram {
                name: req.name,
                backend: Arc::clone(&self.backend),
            }),
        })
    }

    fn gauge(&self, req: GaugeLookupRequest) -> Result<GaugeLookupResponse, ObserveError> {
        Ok(GaugeLookupResponse {
            gauge: Box::new(MetricsBridgeGauge {
                name: req.name,
                backend: Arc::clone(&self.backend),
            }),
        })
    }
}

struct MetricsBridgeCounter {
    name: String,
    backend: Arc<dyn MetricsProvider>,
}

impl Counter for MetricsBridgeCounter {
    fn increment(&self, req: IncrementRequest) -> Result<IncrementResponse, ObserveError> {
        self.backend
            .record_counter(&self.name, req.delta as f64, &[]);
        Ok(IncrementResponse)
    }
}

struct MetricsBridgeGauge {
    name: String,
    backend: Arc<dyn MetricsProvider>,
}

impl Gauge for MetricsBridgeGauge {
    fn set(&self, req: GaugeSetRequest) -> Result<GaugeSetResponse, ObserveError> {
        self.backend.record_gauge(&self.name, req.value, &[]);
        Ok(GaugeSetResponse)
    }
}

struct MetricsBridgeHistogram {
    name: String,
    backend: Arc<dyn MetricsProvider>,
}

impl Histogram for MetricsBridgeHistogram {
    fn record(&self, req: HistogramRecordRequest) -> Result<HistogramRecordResponse, ObserveError> {
        self.backend.record_histogram(&self.name, req.value, &[]);
        Ok(HistogramRecordResponse)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swe_observ_metrics::create_local_metrics_backend;

    #[test]
    fn test_counter_increment_reaches_the_shared_backend_happy() {
        let backend: Arc<dyn MetricsProvider> = Arc::new(create_local_metrics_backend());
        let registry = MetricsBridgeRegistry::new(Arc::clone(&backend));
        let counter = registry
            .counter(CounterLookupRequest {
                name: "bridge_counter".to_string(),
            })
            .unwrap()
            .counter;
        counter.increment(IncrementRequest { delta: 5 }).unwrap();
        let snap = backend.export();
        assert!(
            snap.iter()
                .any(|s| s.name == "bridge_counter" && s.value == 5.0),
            "expected the increment to reach the shared backend, got: {snap:?}"
        );
    }

    #[test]
    fn test_gauge_set_reaches_the_shared_backend_happy() {
        let backend: Arc<dyn MetricsProvider> = Arc::new(create_local_metrics_backend());
        let registry = MetricsBridgeRegistry::new(Arc::clone(&backend));
        let gauge = registry
            .gauge(GaugeLookupRequest {
                name: "bridge_gauge".to_string(),
            })
            .unwrap()
            .gauge;
        gauge.set(GaugeSetRequest { value: 12.5 }).unwrap();
        let snap = backend.export();
        assert!(
            snap.iter()
                .any(|s| s.name == "bridge_gauge" && s.value == 12.5),
            "expected the gauge set to reach the shared backend, got: {snap:?}"
        );
    }

    #[test]
    fn test_histogram_record_reaches_the_shared_backend_happy() {
        let backend: Arc<dyn MetricsProvider> = Arc::new(create_local_metrics_backend());
        let registry = MetricsBridgeRegistry::new(Arc::clone(&backend));
        let histogram = registry
            .histogram(HistogramLookupRequest {
                name: "bridge_histogram".to_string(),
            })
            .unwrap()
            .histogram;
        histogram
            .record(HistogramRecordRequest { value: 99.0 })
            .unwrap();
        let snap = backend.export();
        assert!(
            snap.iter()
                .any(|s| s.name == "bridge_histogram" && s.value == 99.0),
            "expected the histogram record to reach the shared backend, got: {snap:?}"
        );
    }

    #[test]
    fn test_two_counter_lookups_for_the_same_name_share_state_edge() {
        let backend: Arc<dyn MetricsProvider> = Arc::new(create_local_metrics_backend());
        let registry = MetricsBridgeRegistry::new(Arc::clone(&backend));
        for _ in 0..3 {
            registry
                .counter(CounterLookupRequest {
                    name: "repeated".to_string(),
                })
                .unwrap()
                .counter
                .increment(IncrementRequest { delta: 1 })
                .unwrap();
        }
        let snap = backend.export();
        assert!(
            snap.iter().any(|s| s.name == "repeated" && s.value == 3.0),
            "expected repeated lookups of the same name to accumulate in the shared backend, got: {snap:?}"
        );
    }
}
