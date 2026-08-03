//! `Sampler` — background metric sampling loop interface.

use crate::types::SharedCounters;

/// Trait for types that run a background metric-sampling loop.
pub trait Sampler: Send + Sync {
    /// Counters this sampler reads from and derives gauges for.
    fn counters(&self) -> &SharedCounters;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TrafficCounters;
    use std::sync::Arc;
    use swe_observ_metrics::create_local_metrics_backend;

    struct SamplerDouble {
        counters: SharedCounters,
    }
    impl Sampler for SamplerDouble {
        fn counters(&self) -> &SharedCounters {
            &self.counters
        }
    }

    #[test]
    fn test_sampler_double_counters_is_same_instance() {
        let c: SharedCounters = Arc::new(TrafficCounters::new(Arc::new(
            create_local_metrics_backend(),
        )));
        let d = SamplerDouble {
            counters: Arc::clone(&c),
        };
        assert!(Arc::ptr_eq(d.counters(), &c));
    }
}
