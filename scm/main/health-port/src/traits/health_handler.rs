//! `HealthHandler` — HTTP health endpoint port contract.

use swe_edge_ingress_http::HttpIngress;

/// Marker supertrait for runtime health HTTP endpoint handlers.
///
/// Implementations respond to `GET /health` with a JSON `RuntimeHealth`
/// body and HTTP 200 when healthy, 503 when degraded.
pub trait HealthHandler: HttpIngress {}
