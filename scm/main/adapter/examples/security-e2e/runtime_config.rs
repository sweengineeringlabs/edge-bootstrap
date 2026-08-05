//! Runtime configuration assembly — the one thing this module owns is
//! "what bind addresses, and what JWT bearer-auth policy, this demo runs
//! with."

use swe_edge_bootstrap::{JwtConfig, JwtKey, RuntimeConfig};

pub(crate) const HTTP_BIND: &str = "127.0.0.1:18290";
pub(crate) const GRPC_BIND: &str = "127.0.0.1:19290";

/// HS256 signing secret for this demo only. Hardcoded and printed in plain
/// sight on purpose so the example is runnable and its output is
/// reproducible — a real deployment must load key material from a secret
/// store (Vault, KMS, environment injection, etc.), never a source-code
/// constant. **Never reuse this value outside this local demonstration.**
pub(crate) const DEMO_JWT_SECRET: &[u8] =
    b"security-e2e-demo-hs256-secret-DO-NOT-USE-IN-PRODUCTION";

/// Build the `RuntimeConfig`: HTTP + gRPC bind addresses, plus `http_auth`
/// wired to a symmetric HS256 verifier.
///
/// `RuntimeConfig.http_auth: Option<JwtConfig>` is an unconditional field —
/// no `security` feature gate guards it. When set,
/// `RuntimeBuilder::serve()` (`src/core/runtime/runtime_builder_serve.rs`)
/// builds a `swe_edge_ingress_verifier::JwtVerifier` from it and calls
/// `AxumHttpServer::with_bearer_auth(verifier)` on the HTTP listener before
/// any request reaches ingress dispatch.
///
/// gRPC has no equivalent bearer-auth wiring in this version of
/// `swe-edge-runtime-grpc-adapter` (`http-adapter/v0.2.0`'s
/// `TonicGrpcServer` only exposes TLS/interceptors/allow-unauthenticated,
/// not `with_bearer_auth`) — so `main.rs` calls
/// `.grpc_allow_unauthenticated()` and only the HTTP route below
/// demonstrates the JWT auth boundary.
pub(crate) fn build_runtime_config() -> RuntimeConfig {
    let mut config = RuntimeConfig::default()
        .with_service_name("security-e2e-demo")
        .with_http_bind(HTTP_BIND)
        .with_grpc_bind(GRPC_BIND)
        .with_systemd_notify(false);
    config.http_auth = Some(JwtConfig {
        key: JwtKey::Hs256 {
            secret: DEMO_JWT_SECRET.to_vec(),
        },
        required_issuer: None,
        required_audience: None,
        leeway_seconds: 5,
    });
    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::expect_used)]
    #[test]
    fn test_build_runtime_config_sets_hs256_secret_matching_demo_secret() {
        let config = build_runtime_config();
        let auth = config.http_auth.expect("http_auth must be set");
        match auth.key {
            JwtKey::Hs256 { secret } => assert_eq!(secret, DEMO_JWT_SECRET),
            other => panic!("expected JwtKey::Hs256, got {other:?}"),
        }
    }

    #[test]
    fn test_build_runtime_config_binds_expected_http_addr() {
        assert_eq!(build_runtime_config().http_bind, HTTP_BIND);
    }

    #[test]
    fn test_build_runtime_config_binds_expected_grpc_addr() {
        assert_eq!(build_runtime_config().grpc_bind, GRPC_BIND);
    }

    #[allow(clippy::expect_used)]
    #[test]
    fn test_build_runtime_config_leeway_seconds_is_five() {
        let auth = build_runtime_config()
            .http_auth
            .expect("http_auth must be set");
        assert_eq!(auth.leeway_seconds, 5);
    }

    #[allow(clippy::expect_used)]
    #[test]
    fn test_build_runtime_config_skips_issuer_and_audience_checks() {
        let auth = build_runtime_config()
            .http_auth
            .expect("http_auth must be set");
        assert!(auth.required_issuer.is_none());
        assert!(auth.required_audience.is_none());
    }
}
