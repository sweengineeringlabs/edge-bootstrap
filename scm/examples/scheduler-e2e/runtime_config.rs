//! Runtime configuration assembly — the one thing this module owns is
//! "what bind addresses does this demo run with."

use swe_edge_bootstrap::RuntimeConfig;

pub(crate) const HTTP_BIND: &str = "127.0.0.1:18490";
pub(crate) const GRPC_BIND: &str = "127.0.0.1:19490";

/// Build the `RuntimeConfig`: HTTP + gRPC bind addresses for the
/// `/scheduler/status` route. No metrics/tracing sections — this demo is
/// scoped to one thing: proving `swe-edge-runtime-scheduler` drives a real
/// interval-based job, not exercising the rest of the `RuntimeBuilder`
/// surface (see `bootstrap-e2e` for that).
pub(crate) fn build_runtime_config() -> RuntimeConfig {
    RuntimeConfig::default()
        .with_service_name("scheduler-e2e-demo")
        .with_http_bind(HTTP_BIND)
        .with_grpc_bind(GRPC_BIND)
        .with_systemd_notify(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_runtime_config_binds_http_to_expected_port() {
        let config = build_runtime_config();
        assert_eq!(config.http_bind, HTTP_BIND);
    }

    #[test]
    fn test_build_runtime_config_binds_grpc_to_expected_port() {
        let config = build_runtime_config();
        assert_eq!(config.grpc_bind, GRPC_BIND);
    }

    #[test]
    fn test_build_runtime_config_disables_systemd_notify() {
        let config = build_runtime_config();
        assert!(!config.systemd_notify);
    }
}
