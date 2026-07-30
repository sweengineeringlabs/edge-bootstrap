//! `LifecycleObserver` — marker trait for observability-emitting lifecycle-monitor wrappers.

/// Marker trait for [`LifecycleMonitor`](edge_proxy::LifecycleMonitor) wrappers
/// that emit observability signals (metrics, traces) on health transitions.
pub trait LifecycleObserver: edge_proxy::LifecycleMonitor {}

#[cfg(test)]
mod tests {
    use super::*;
    use edge_proxy::{
        ComponentRequest, ComponentResponse, HealthRequest, HealthResponse, LifecycleError,
        LifecycleMonitor, ProxySvc, ShutdownRequest, StartBackgroundTasksRequest, StatusRequest,
        StatusResponse,
    };
    use futures::future::BoxFuture;
    use std::sync::Arc;

    struct LifecycleObserverDouble {
        inner: Arc<dyn LifecycleMonitor>,
    }

    impl LifecycleMonitor for LifecycleObserverDouble {
        fn health(
            &self,
            req: HealthRequest,
        ) -> BoxFuture<'_, Result<HealthResponse, LifecycleError>> {
            self.inner.health(req)
        }
        fn start_background_tasks(
            &self,
            req: StartBackgroundTasksRequest,
        ) -> BoxFuture<'_, Result<(), LifecycleError>> {
            self.inner.start_background_tasks(req)
        }
        fn shutdown(&self, req: ShutdownRequest) -> BoxFuture<'_, Result<(), LifecycleError>> {
            self.inner.shutdown(req)
        }
        fn status(
            &self,
            req: StatusRequest,
        ) -> BoxFuture<'_, Result<StatusResponse, LifecycleError>> {
            self.inner.status(req)
        }
        fn component<'a>(
            &'a self,
            req: ComponentRequest<'_>,
        ) -> BoxFuture<'a, Result<ComponentResponse, LifecycleError>> {
            self.inner.component(req)
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
