//! `LifecycleObserver` — marker trait for observability-emitting lifecycle-monitor wrappers.

/// Marker trait for [`LifecycleMonitor`](edge_proxy::LifecycleMonitor) wrappers
/// that emit observability signals (metrics, traces) on health transitions.
pub trait LifecycleObserver: edge_proxy::LifecycleMonitor {}

#[cfg(test)]
mod tests {
    use super::*;
    use edge_proxy::{
        ComponentHealth, ComponentRequest, EmptyResponse, HealthRequest, HealthResponse,
        HealthStatus, LifecycleError, LifecycleMonitor, ProxySvc, ShutdownRequest,
        StartBackgroundTasksRequest, StatusRequest,
    };
    use std::sync::Arc;

    struct LifecycleObserverDouble {
        inner: Arc<dyn LifecycleMonitor>,
    }

    #[async_trait::async_trait]
    impl LifecycleMonitor for LifecycleObserverDouble {
        async fn health(&self, req: HealthRequest) -> Result<HealthResponse, LifecycleError> {
            self.inner.health(req).await
        }
        async fn start_background_tasks(
            &self,
            req: StartBackgroundTasksRequest,
        ) -> Result<(), LifecycleError> {
            self.inner.start_background_tasks(req).await
        }
        async fn shutdown(&self, req: ShutdownRequest) -> Result<(), LifecycleError> {
            self.inner.shutdown(req).await
        }
        async fn status(
            &self,
            req: StatusRequest,
        ) -> Result<EmptyResponse<HealthStatus>, LifecycleError> {
            self.inner.status(req).await
        }
        async fn component(
            &self,
            req: ComponentRequest<'_>,
        ) -> Result<EmptyResponse<Option<ComponentHealth>>, LifecycleError> {
            self.inner.component(req).await
        }
    }
    impl LifecycleObserver for LifecycleObserverDouble {}

    #[test]
    fn test_lifecycle_observer_double_is_object_safe_as_dyn() {
        let _: Arc<dyn LifecycleObserver> = Arc::new(LifecycleObserverDouble {
            inner: ProxySvc::new_null_lifecycle_monitor(),
        });
    }

    #[tokio::test]
    async fn test_lifecycle_observer_double_delegates_shutdown_to_inner() {
        let d = LifecycleObserverDouble {
            inner: ProxySvc::new_null_lifecycle_monitor(),
        };
        assert!(d.shutdown(ShutdownRequest).await.is_ok());
    }
}
