//! Integration tests for DefaultRuntimeManager.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use edge_proxy::ProxySvc;
use std::sync::Arc;
use swe_edge_bootstrap::{Runtime, RuntimeConfig};
use swe_edge_egress_http::HttpTransportSvc;

/// @covers: DefaultRuntimeManager
#[test]
fn test_default_runtime_manager_constructs_from_factory() {
    let http = Arc::from(HttpTransportSvc::default_http_egress().expect("http outbound"));
    let ingress = Arc::new(Runtime::empty_ingress());
    let egress = Arc::new(Runtime::http_egress(http));
    let lc = ProxySvc::new_null_lifecycle_monitor();
    let _mgr = Runtime::runtime_manager(RuntimeConfig::default(), ingress, egress, lc);
}
