//! `MetricsHandler` — Prometheus-compatible metrics HTTP endpoint interface.

use swe_edge_ingress_http::HttpIngress;

/// Marker supertrait for Prometheus metrics HTTP endpoint handlers.
pub trait MetricsHandler: HttpIngress {}
