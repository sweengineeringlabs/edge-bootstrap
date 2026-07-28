/// Marker trait for [`LifecycleMonitor`](edge_proxy::LifecycleMonitor) wrappers
/// that emit observability signals (metrics, traces) on health transitions.
pub trait LifecycleObserver: edge_proxy::LifecycleMonitor {}
