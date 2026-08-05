//! Runtime configuration assembly — the one thing this module owns is
//! "what bind addresses does this demo run on."

use swe_edge_bootstrap::RuntimeConfig;

pub(crate) const HTTP_BIND: &str = "127.0.0.1:18590";
pub(crate) const GRPC_BIND: &str = "127.0.0.1:19590";

/// Build the `RuntimeConfig`: HTTP + gRPC bind addresses, no systemd notify
/// (this is a demo process, not a real daemon under a service manager).
pub(crate) fn build_runtime_config() -> RuntimeConfig {
    RuntimeConfig::default()
        .with_service_name("tls-e2e-demo")
        .with_http_bind(HTTP_BIND)
        .with_grpc_bind(GRPC_BIND)
        .with_systemd_notify(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// @covers: build_runtime_config
    #[test]
    fn test_build_runtime_config_binds_expected_http_address_happy() {
        let config = build_runtime_config();
        assert_eq!(config.http_bind, HTTP_BIND);
    }

    /// @covers: build_runtime_config
    #[test]
    fn test_build_runtime_config_binds_expected_grpc_address_happy() {
        let config = build_runtime_config();
        assert_eq!(config.grpc_bind, GRPC_BIND);
    }

    /// @covers: build_runtime_config
    #[test]
    fn test_build_runtime_config_disables_systemd_notify_happy() {
        let config = build_runtime_config();
        assert!(
            !config.systemd_notify,
            "a demo process must not tell systemd it's a managed service"
        );
    }
}
