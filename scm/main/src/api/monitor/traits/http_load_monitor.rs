//! `HttpLoadMonitor` — HTTP inbound load-monitoring wrapper interface.

use swe_edge_ingress_http::HttpIngress;

/// Marker supertrait for HTTP inbound handlers that record load metrics.
pub trait HttpLoadMonitor: HttpIngress {}
