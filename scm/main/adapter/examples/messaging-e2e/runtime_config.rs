//! Runtime configuration assembly — the one thing this module owns is
//! "what bind addresses does this demo run with."

use swe_edge_bootstrap::RuntimeConfig;

pub(crate) const HTTP_BIND: &str = "127.0.0.1:18390";
pub(crate) const GRPC_BIND: &str = "127.0.0.1:19390";

/// Build the `RuntimeConfig`: HTTP + gRPC bind addresses for the
/// order-intake handler.
pub(crate) fn build_runtime_config() -> RuntimeConfig {
    RuntimeConfig::default()
        .with_service_name("messaging-e2e-demo")
        .with_http_bind(HTTP_BIND)
        .with_grpc_bind(GRPC_BIND)
        .with_systemd_notify(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_runtime_config_binds_http_and_grpc_to_distinct_ports() {
        let config = build_runtime_config();
        assert_eq!(config.http_bind, HTTP_BIND);
        assert_eq!(config.grpc_bind, GRPC_BIND);
        assert_ne!(config.http_bind, config.grpc_bind);
    }

    #[test]
    fn test_ports_do_not_collide_with_other_examples() {
        // bootstrap-e2e uses 18090/19090, observability-e2e uses 181xx,
        // security-e2e uses 182xx — this demo must stay clear of all of them.
        assert_eq!(HTTP_BIND, "127.0.0.1:18390");
        assert_eq!(GRPC_BIND, "127.0.0.1:19390");
    }
}
