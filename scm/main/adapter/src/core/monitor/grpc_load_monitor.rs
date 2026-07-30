use std::sync::Arc;
use std::time::Instant;

use futures::future::BoxFuture;
use swe_edge_ingress_grpc::{
    GrpcIngress, GrpcIngressError, GrpcResponse, HealthCheckRequest, HealthCheckResponse,
    StreamRequest, StreamResponse, UnaryRequest,
};

use swe_edge_bootstrap_monitor::SharedCounters;

/// Wraps a `GrpcIngress` handler; records load metrics on every call.
pub(crate) struct GrpcLoadMonitor {
    inner: Arc<dyn GrpcIngress>,
    counters: SharedCounters,
}

impl GrpcLoadMonitor {
    pub(crate) fn new(inner: Arc<dyn GrpcIngress>, counters: SharedCounters) -> Self {
        Self { inner, counters }
    }
}

impl swe_edge_bootstrap_monitor::GrpcLoadMonitor for GrpcLoadMonitor {}

impl GrpcIngress for GrpcLoadMonitor {
    fn handle_unary(
        &self,
        req: UnaryRequest,
    ) -> BoxFuture<'_, Result<GrpcResponse, GrpcIngressError>> {
        let method = req.request.method.clone();
        tracing::debug!(node = "grpc_load_monitor", %method, "recording load metrics");
        self.counters.on_start();
        let counters = Arc::clone(&self.counters);
        let fut = self.inner.handle_unary(req);
        Box::pin(async move {
            let start = Instant::now();
            let result = fut.await;
            let elapsed_us = start.elapsed().as_micros() as u64;
            counters.on_end(elapsed_us, result.is_err());
            tracing::debug!(
                node = "grpc_load_monitor",
                %method,
                elapsed_us,
                is_err = result.is_err(),
                "load metrics recorded"
            );
            result
        })
    }

    fn handle_stream(
        &self,
        req: StreamRequest,
    ) -> BoxFuture<'_, Result<StreamResponse, GrpcIngressError>> {
        self.counters.on_start();
        let counters = Arc::clone(&self.counters);
        let fut = self.inner.handle_stream(req);
        Box::pin(async move {
            let start = Instant::now();
            let result = fut.await;
            counters.on_end(start.elapsed().as_micros() as u64, result.is_err());
            result
        })
    }

    fn health_check(
        &self,
        req: HealthCheckRequest,
    ) -> BoxFuture<'_, Result<HealthCheckResponse, GrpcIngressError>> {
        self.inner.health_check(req)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swe_edge_bootstrap_monitor::TrafficCounters;
    use std::sync::Arc;
    use swe_edge_ingress_grpc::GrpcHealthCheck;
    use swe_observ_metrics::create_local_metrics_backend;

    fn counters() -> SharedCounters {
        Arc::new(TrafficCounters::new(Arc::new(
            create_local_metrics_backend(),
        )))
    }

    #[test]
    fn test_grpc_load_monitor_new_does_not_panic() {
        struct GrpcLoadMonitorStub;
        impl GrpcIngress for GrpcLoadMonitorStub {
            fn handle_unary(
                &self,
                _req: UnaryRequest,
            ) -> BoxFuture<'_, Result<GrpcResponse, GrpcIngressError>> {
                Box::pin(async { Err(GrpcIngressError::Unimplemented("stub".into())) })
            }
            fn handle_stream(
                &self,
                _req: StreamRequest,
            ) -> BoxFuture<'_, Result<StreamResponse, GrpcIngressError>> {
                Box::pin(async { Err(GrpcIngressError::Unimplemented("stub".into())) })
            }
            fn health_check(
                &self,
                _req: HealthCheckRequest,
            ) -> BoxFuture<'_, Result<HealthCheckResponse, GrpcIngressError>> {
                Box::pin(async {
                    Ok(HealthCheckResponse {
                        check: Box::new(GrpcHealthCheck::healthy()),
                    })
                })
            }
        }
        let _m = GrpcLoadMonitor::new(Arc::new(GrpcLoadMonitorStub), counters());
    }
}
