//! JWT bearer-auth demo — a real HS256-signed token verified by
//! `swe-edge-bootstrap`'s own `RuntimeBuilder::serve()` wiring, gating a
//! live HTTP route.
//!
//! Run:
//!     cargo run -p swe-edge-bootstrap --features security --example security-e2e
//!
//! Then, from another shell (mint a matching token with the
//! `#[cfg(test)]` helper in this file — see `tests::test_minted_token_...`
//! below, run with `cargo test --example security-e2e -- --nocapture`):
//!
//!     curl -i http://127.0.0.1:18290/secure-echo \
//!         -H 'content-type: application/json' -d '{"message":"hi"}'
//!         # -> 401 Unauthorized (no Authorization header)
//!
//!     curl -i http://127.0.0.1:18290/secure-echo \
//!         -H 'content-type: application/json' \
//!         -H 'authorization: Bearer garbage.not.a.jwt' -d '{"message":"hi"}'
//!         # -> 401 Unauthorized (signature verification fails)
//!
//!     curl -i http://127.0.0.1:18290/secure-echo \
//!         -H 'content-type: application/json' \
//!         -H "authorization: Bearer $DEMO_JWT" -d '{"message":"hi"}'
//!         # -> 200 OK, echoes {"message":"hi"} back
//!
//! ## What `RuntimeConfig.http_auth` actually wires (traced, not assumed)
//!
//! `RuntimeConfig.http_auth: Option<JwtConfig>` (`runtime_config`) is an
//! unconditional field — no `security` feature gate guards it. When set,
//! `RuntimeBuilder::serve()` (`src/core/runtime/runtime_builder_serve.rs`)
//! builds a `swe_edge_ingress_verifier::JwtVerifier` from it and calls
//! `AxumHttpServer::with_bearer_auth(verifier)` on the HTTP listener — gRPC
//! gets no equivalent wiring in this version, so `.grpc_allow_unauthenticated()`
//! is set below and only the HTTP route demonstrates the auth boundary.
//!
//! Traced end to end in `edge-runtime` (`http-adapter/v0.2.0`):
//!   1. `AxumHttpServerHelper::verify_auth` runs first in the Axum service
//!      stack, ahead of ingress dispatch. A missing/malformed
//!      `Authorization` header, or a token that fails
//!      `TokenVerifier::verify`, both short-circuit with a real `401`
//!      *before* this crate's `HttpIngressJob` — and therefore
//!      [`handler::SecureEchoHandler::execute`] — ever runs.
//!   2. On success, `verify_auth` builds a
//!      `SecurityContext::authenticated_with(..)` from the verified
//!      `Claims` (sub/iss/tenant/custom) and inserts it into the Axum
//!      request's extensions.
//!   3. `AxumHttpServerHelper::extract_request` pulls that `SecurityContext`
//!      back out; `HttpIngressJob::handle` (this crate,
//!      `src/core/dispatch/http_ingress_job.rs`) turns it into
//!      `HandlerContext.security`, which is what actually reaches
//!      `Handler::execute`.
//!
//! Step 3 is where this demo stops short of reading claims *inside* the
//! handler, and says so rather than fake it: `HandlerContext.security` is
//! typed `&dyn SecurityPrincipal` (`edge-application` v0.20.0,
//! `handler/api/handler/traits/security_principal.rs`), and
//! `SecurityPrincipal` is declared with **zero methods**, by design —
//! "opaque handle... callers hold and pass the reference, they don't call
//! anything on it directly." There is no safe way to recover the concrete
//! `SecurityContext` (sub, tenant, custom claims) from inside
//! `Handler::execute` in this version of the stack. So this demo proves the
//! auth boundary the way an external client actually observes it — real
//! `401`s for a missing/invalid token and a real `200` for a valid one —
//! not by reading claims out of the handler. See [`handler`].
//!
//! Split by responsibility: [`handler`] owns the domain payload/`Handler`,
//! [`runtime_config`] owns bind addresses and the JWT auth policy — `main`
//! only wires the two together and serves.

mod handler;
mod runtime_config;

use swe_edge_bootstrap::Runtime;

#[tokio::main]
async fn main() {
    let handler = handler::build_handler();
    let config = runtime_config::build_runtime_config();

    println!(
        "http: http://{}/secure-echo (JWT HS256 bearer auth required)",
        runtime_config::HTTP_BIND
    );
    println!(
        "grpc: {} (no bearer auth wired for gRPC in this version — served open)",
        runtime_config::GRPC_BIND
    );

    #[cfg(feature = "security")]
    report_security_feature_status();
    #[cfg(not(feature = "security"))]
    println!(
        "security feature disabled: SecurityContext/TenantId/authz extras not linked in \
         (the JWT bearer auth above is unaffected by this — http_auth needs no feature)"
    );

    let builder = Runtime::builder()
        .config(config)
        .http_route(handler.clone())
        .grpc_route(handler)
        .grpc_allow_unauthenticated();

    if let Err(e) = builder.serve().await {
        panic!("serve failed: {e}");
    }
}

/// Exercise the `security`-feature-only types (`SecurityContext`,
/// `TenantId`, `NoopAuthzPolicy`) end to end, independent of the live HTTP
/// request path above — these are not wired into `Handler::execute` in this
/// version of the stack (see the module doc comment), so this is a
/// standalone, client-side proof that the types compile, link, and behave
/// as documented when a consumer opts into the feature.
#[cfg(feature = "security")]
fn report_security_feature_status() {
    use edge_security_runtime_authz::{
        AuthzPolicy, AuthzPolicyCheckRequest, NoopAuthzPolicy, RuntimeAuthzContext,
    };
    use swe_edge_bootstrap::{SecurityContext, TenantId};

    const DEMO_TENANT: &str = "security-e2e-demo-tenant";

    let ctx = SecurityContext::authenticated_with(Box::new(TenantId::new(DEMO_TENANT)))
        .with_tenant(DEMO_TENANT);
    let authz_ctx = RuntimeAuthzContext::new(ctx);
    let allowed = NoopAuthzPolicy
        .check(AuthzPolicyCheckRequest {
            context: &authz_ctx,
            resource: Some("/secure-echo"),
            resource_tenant: None,
        })
        .is_ok();
    println!(
        "security feature enabled: SecurityContext/TenantId/NoopAuthzPolicy linked in, \
         sample authz check for tenant '{DEMO_TENANT}' allowed={allowed}"
    );
}

#[cfg(test)]
mod tests {
    use jsonwebtoken::{encode, EncodingKey, Header};
    use serde::Serialize;
    use swe_edge_ingress_verifier::{JwtVerifier, TokenVerifier};

    use crate::runtime_config::{self, DEMO_JWT_SECRET};

    #[derive(Serialize)]
    struct DemoClaims {
        sub: String,
        exp: u64,
    }

    /// Mint a real HS256-signed JWT with `secret`, expiring `ttl_secs` from
    /// now (negative for an already-expired token).
    #[allow(clippy::expect_used)]
    fn mint_token(secret: &[u8], sub: &str, ttl_secs: i64) -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock must be after the Unix epoch")
            .as_secs() as i64;
        let exp = (now + ttl_secs).max(0) as u64;
        let claims = DemoClaims {
            sub: sub.to_string(),
            exp,
        };
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret),
        )
        .expect("HS256 signing with a non-empty secret must succeed")
    }

    #[test]
    fn test_mint_token_produces_three_segment_jwt() {
        let token = mint_token(DEMO_JWT_SECRET, "demo-user", 3600);
        assert_eq!(
            token.split('.').count(),
            3,
            "a JWT must have header.payload.signature"
        );
    }

    /// This is the real integration point: mint a token with the same
    /// secret `runtime_config::build_runtime_config()` wires into
    /// `RuntimeConfig.http_auth`, then verify it with the exact
    /// `JwtVerifier` type `RuntimeBuilder::serve()` constructs from that
    /// config. If the demo secret or `JwtConfig` fields drift out of sync,
    /// this test fails.
    #[allow(clippy::expect_used)]
    #[test]
    fn test_minted_token_verifies_against_runtime_jwt_config() {
        let auth_cfg = runtime_config::build_runtime_config()
            .http_auth
            .expect("build_runtime_config must set http_auth");
        let verifier =
            JwtVerifier::from_config(&auth_cfg).expect("JwtConfig must build a valid verifier");
        let token = mint_token(DEMO_JWT_SECRET, "demo-user", 3600);
        let claims = verifier
            .verify(&token)
            .expect("token signed with the matching demo secret must verify");
        assert_eq!(claims.sub.as_deref(), Some("demo-user"));
        // Printed for manual `curl` verification of the live server
        // (`cargo test --example security-e2e -- --nocapture`).
        println!("DEMO_JWT={token}");
    }

    #[allow(clippy::expect_used)]
    #[test]
    fn test_token_signed_with_wrong_secret_is_rejected() {
        let auth_cfg = runtime_config::build_runtime_config()
            .http_auth
            .expect("build_runtime_config must set http_auth");
        let verifier =
            JwtVerifier::from_config(&auth_cfg).expect("JwtConfig must build a valid verifier");
        let token = mint_token(
            b"a-completely-different-wrong-secret-value",
            "demo-user",
            3600,
        );
        assert!(
            verifier.verify(&token).is_err(),
            "a token signed with the wrong secret must be rejected"
        );
    }

    #[allow(clippy::expect_used)]
    #[test]
    fn test_expired_token_is_rejected() {
        let auth_cfg = runtime_config::build_runtime_config()
            .http_auth
            .expect("build_runtime_config must set http_auth");
        let verifier =
            JwtVerifier::from_config(&auth_cfg).expect("JwtConfig must build a valid verifier");
        let token = mint_token(DEMO_JWT_SECRET, "demo-user", -3600);
        assert!(
            verifier.verify(&token).is_err(),
            "a token that expired an hour ago must be rejected"
        );
    }

    #[allow(clippy::expect_used)]
    #[test]
    fn test_malformed_token_is_rejected() {
        let auth_cfg = runtime_config::build_runtime_config()
            .http_auth
            .expect("build_runtime_config must set http_auth");
        let verifier =
            JwtVerifier::from_config(&auth_cfg).expect("JwtConfig must build a valid verifier");
        assert!(
            verifier.verify("garbage.not.a.jwt").is_err(),
            "a malformed, non-JWT string must be rejected"
        );
    }

    #[cfg(feature = "security")]
    #[test]
    fn test_report_security_feature_status_does_not_panic() {
        super::report_security_feature_status();
    }
}
