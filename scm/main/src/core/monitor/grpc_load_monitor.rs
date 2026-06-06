use std::sync::Arc;
use std::time::Instant;

use edge_domain::RequestContext;
use futures::future::BoxFuture;
use swe_edge_ingress_grpc::{
    GrpcHealthCheck, GrpcIngress, GrpcIngressResult, GrpcMessageStream, GrpcMetadata, GrpcRequest,
    GrpcResponse,
};

use crate::api::monitor::SharedCounters;

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

impl crate::api::monitor::GrpcLoadMonitor for GrpcLoadMonitor {}

impl GrpcIngress for GrpcLoadMonitor {
    fn handle_unary(
        &self,
        request: GrpcRequest,
        ctx: RequestContext,
    ) -> BoxFuture<'_, GrpcIngressResult<GrpcResponse>> {
        self.counters.on_start();
        let counters = Arc::clone(&self.counters);
        let fut = self.inner.handle_unary(request, ctx);
        Box::pin(async move {
            let start = Instant::now();
            let result = fut.await;
            counters.on_end(start.elapsed().as_micros() as u64, result.is_err());
            result
        })
    }

    fn handle_stream(
        &self,
        method: String,
        metadata: GrpcMetadata,
        messages: GrpcMessageStream,
        ctx: RequestContext,
    ) -> BoxFuture<'_, GrpcIngressResult<(GrpcMessageStream, GrpcMetadata)>> {
        self.counters.on_start();
        let counters = Arc::clone(&self.counters);
        let fut = self.inner.handle_stream(method, metadata, messages, ctx);
        Box::pin(async move {
            let start = Instant::now();
            let result = fut.await;
            counters.on_end(start.elapsed().as_micros() as u64, result.is_err());
            result
        })
    }

    fn health_check(&self) -> BoxFuture<'_, GrpcIngressResult<GrpcHealthCheck>> {
        self.inner.health_check()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::monitor::TrafficCounters;
    use std::sync::Arc;
    use swe_edge_ingress_grpc::GrpcIngressError;
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
                _: GrpcRequest,
                _: RequestContext,
            ) -> BoxFuture<'_, GrpcIngressResult<GrpcResponse>> {
                Box::pin(async { Err(GrpcIngressError::Unimplemented("stub".into())) })
            }
            fn handle_stream(
                &self,
                _: String,
                _: GrpcMetadata,
                _: GrpcMessageStream,
                _: RequestContext,
            ) -> BoxFuture<'_, GrpcIngressResult<(GrpcMessageStream, GrpcMetadata)>> {
                Box::pin(async { Err(GrpcIngressError::Unimplemented("stub".into())) })
            }
            fn health_check(&self) -> BoxFuture<'_, GrpcIngressResult<GrpcHealthCheck>> {
                Box::pin(async { Ok(GrpcHealthCheck::healthy()) })
            }
        }
        let _m = GrpcLoadMonitor::new(Arc::new(GrpcLoadMonitorStub), counters());
    }
}
