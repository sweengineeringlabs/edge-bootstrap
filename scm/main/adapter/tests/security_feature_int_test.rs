//! Integration tests — `security` feature flag: SAF re-exports + SecurityContext behaviour.

#![cfg(feature = "security")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use swe_edge_bootstrap::{Principal, PrincipalRequest, SecurityContext, TenantId};

#[test]
fn test_security_context_unauthenticated_sets_authenticated_false() {
    let ctx = SecurityContext::unauthenticated();
    assert!(!ctx.authenticated);
}

#[test]
fn test_security_context_unauthenticated_has_no_principal() {
    let ctx = SecurityContext::unauthenticated();
    assert!(ctx.principal.is_none());
}

#[test]
fn test_security_context_authenticated_with_tenant_sets_authenticated_true() {
    let ctx = SecurityContext::authenticated_with(Box::new(TenantId::new("acme")));
    assert!(ctx.authenticated);
}

#[test]
fn test_security_context_authenticated_with_tenant_has_principal() {
    let ctx = SecurityContext::authenticated_with(Box::new(TenantId::new("acme")));
    assert!(ctx.principal.is_some());
}

#[test]
fn test_tenant_id_principal_id_returns_value() {
    let t = TenantId::new("org-42");
    assert_eq!(t.id(PrincipalRequest).unwrap().value, "org-42");
}

#[test]
fn test_tenant_id_principal_kind_returns_tenant() {
    let t = TenantId::new("org-42");
    assert_eq!(t.kind(PrincipalRequest).unwrap().value, "tenant");
}

#[test]
fn test_security_context_with_claim_is_retrievable() {
    let ctx = SecurityContext::unauthenticated().with_claim("sub", "user-99");
    assert_eq!(ctx.claims.get("sub").map(String::as_str), Some("user-99"));
}

#[test]
fn test_security_context_with_tenant_id_is_set() {
    let ctx = SecurityContext::unauthenticated().with_tenant("tenant-001");
    assert_eq!(ctx.tenant_id.as_deref(), Some("tenant-001"));
}

#[test]
fn test_security_context_with_trace_id_is_set() {
    let ctx = SecurityContext::unauthenticated().with_trace_id("trace-abc");
    assert_eq!(ctx.trace_id.as_deref(), Some("trace-abc"));
}

#[test]
fn test_security_context_send_sync_via_arc_thread_boundary() {
    use std::sync::Arc;
    let ctx = Arc::new(SecurityContext::unauthenticated());
    let ctx2 = Arc::clone(&ctx);
    std::thread::spawn(move || {
        assert!(!ctx2.authenticated);
    })
    .join()
    .unwrap();
}

#[tokio::test]
async fn test_security_context_arc_crosses_tokio_spawn_boundary() {
    use std::sync::Arc;
    let ctx = Arc::new(SecurityContext::unauthenticated());
    let ctx2 = Arc::clone(&ctx);
    tokio::spawn(async move {
        assert!(!ctx2.authenticated);
    })
    .await
    .unwrap();
}

// ── edge-security-runtime ─────────────────────────────────────────────────────

/// Exercises edge-security-runtime directly via `SecurityContextBuilder`.
#[test]
fn test_security_context_builder_sets_principal_and_authenticated() {
    use edge_security_runtime::SecurityContextBuilder;

    let ctx = SecurityContextBuilder::new()
        .principal(Box::new(TenantId::new("builder-tenant")))
        .tenant_id("builder-tenant")
        .build();
    assert!(ctx.authenticated);
    assert_eq!(ctx.tenant_id.as_deref(), Some("builder-tenant"));
}

// ── edge-security-runtime-authz ───────────────────────────────────────────────

/// Exercises edge-security-runtime-authz directly via `NoopAuthzPolicy`.
#[test]
fn test_noop_authz_policy_always_allows() {
    use edge_security_runtime_authz::{
        AuthzPolicy, AuthzPolicyCheckRequest, NoopAuthzPolicy, RuntimeAuthzContext,
    };

    let ctx = RuntimeAuthzContext::new(SecurityContext::unauthenticated());
    let result = NoopAuthzPolicy.check(AuthzPolicyCheckRequest {
        context: &ctx,
        resource: Some("/any/resource"),
        resource_tenant: None,
    });
    assert!(result.is_ok());
}

// ── edge-security-runtime-credential ──────────────────────────────────────────

/// Exercises edge-security-runtime-credential directly via `SecretString`'s redaction.
#[test]
fn test_secret_string_exposes_original_value_but_redacts_debug() {
    use edge_security_runtime_credential::SecretString;

    let secret = SecretString::new("super-secret-token");
    assert_eq!(secret.expose(), "super-secret-token");
    assert!(!format!("{secret:?}").contains("super-secret-token"));
}

// ── edge-security-runtime-tls ─────────────────────────────────────────────────

/// Exercises edge-security-runtime-tls directly via `PemTlsConfig`'s mTLS detection.
#[test]
fn test_pem_tls_config_mtls_sets_ca_path() {
    use edge_security_runtime_tls::PemTlsConfig;

    let cfg = PemTlsConfig::mtls("cert.pem", "key.pem", "ca.pem");
    assert!(cfg.is_mtls());
}
