//! Prometheus metrics HTTP handler.

#[allow(clippy::module_inception)]
mod metrics_handler;

pub(crate) use metrics_handler::MetricsHandler;
