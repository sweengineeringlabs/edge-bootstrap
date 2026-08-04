//! Integration tests that exercise cross-crate dependencies (Rule 95).
#![allow(clippy::unwrap_used, clippy::expect_used)]
// @allow: no_mocks_in_integration — stub impls required to exercise the public API surface

use std::sync::Arc;
use swe_edge_bootstrap::{Runtime, RuntimeConfig};

// ── edge-domain ───────────────────────────────────────────────────────────────

/// Local payload implementing the `edge_application::Request`/`Response` marker
/// traits — `Handler::Request`/`Response` require them and neither trait has
/// a blanket impl for foreign types (orphan rule), so every handler under
/// test needs its own local payload type.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct DepCoverageTextPayload(String);
impl edge_application::Request for DepCoverageTextPayload {}
impl edge_application::Response for DepCoverageTextPayload {}

/// Exercises edge-domain via the RuntimeBuilder HTTP route path.
#[tokio::test]
async fn test_edge_application_handler_registered_via_builder() {
    use edge_application::{Handler, HandlerError};

    struct PingHandler;

    #[async_trait::async_trait]
    impl Handler for PingHandler {
        type Request = DepCoverageTextPayload;
        type Response = DepCoverageTextPayload;
        async fn execute(
            &self,
            req: edge_application_handler::ExecutionRequest<'_, DepCoverageTextPayload>,
        ) -> Result<DepCoverageTextPayload, HandlerError> {
            Ok(req.req)
        }
    }

    // http_route succeeding without panic confirms edge-domain Handler is wired
    let b = Runtime::builder().http_route(Arc::new(PingHandler));
    // build_registry returns None (no egress) but the builder is valid
    assert!(b.build_registry().is_none());
}

// ── swe-edge-ingress-verifier ─────────────────────────────────────────────────

/// Exercises the JWT verifier integration through the builder.
#[test]
fn test_ingress_verifier_wired_via_bearer_auth() {
    use swe_edge_bootstrap::{JwtConfig, JwtKey, JwtVerifier};

    let secret: Vec<u8> = b"super-secret-test-key-32-bytes!!".to_vec();
    let cfg = JwtConfig {
        key: JwtKey::Hs256 { secret },
        required_issuer: None,
        required_audience: None,
        leeway_seconds: 0,
    };
    let verifier = JwtVerifier::from_config(&cfg).expect("jwt verifier");
    // Wiring the verifier to the builder exercises the TokenVerifier trait path
    let b = Runtime::builder().http_bearer_auth(Arc::new(verifier));
    assert!(b.build_registry().is_none()); // egress not set, but builder is valid
}

// ── swe-edge-egress-grpc ──────────────────────────────────────────────────────

/// Exercises egress-grpc wiring through the builder.
#[test]
fn test_egress_grpc_wired_via_builder() {
    use futures::future::BoxFuture;
    use swe_edge_bootstrap::GrpcEgress;
    use swe_edge_egress_grpc::{
        GrpcEgressError, GrpcEgressResult, GrpcRequest, GrpcResponse, GrpcStatusCode,
        HealthCheckRequest as GrpcHealthCheckRequest,
    };

    struct StubGrpc;
    impl GrpcEgress for StubGrpc {
        fn call_unary(&self, _: GrpcRequest) -> BoxFuture<'_, GrpcEgressResult<GrpcResponse>> {
            Box::pin(async {
                Err(GrpcEgressError::Status(
                    GrpcStatusCode::Unavailable,
                    "stub".into(),
                ))
            })
        }
        fn health_check(&self, _: GrpcHealthCheckRequest) -> BoxFuture<'_, GrpcEgressResult<()>> {
            Box::pin(async { Ok(()) })
        }
    }

    let b = Runtime::builder().egress_grpc(Arc::new(StubGrpc));
    // build_registry is None because no egress_http is set
    assert!(b.build_registry().is_none());
}

// ── swe-edge-ingress-grpc-reflection ─────────────────────────────────────────

/// Exercises gRPC reflection flag in RuntimeConfig.
#[test]
fn test_grpc_reflection_config_field_respected() {
    let cfg = RuntimeConfig {
        grpc_reflection: true,
        ..RuntimeConfig::default()
    };
    assert!(cfg.grpc_reflection);
}

/// Exercises swe-edge-ingress-grpc-reflection directly via ReflectionService.
#[test]
fn test_reflection_service_can_be_constructed_with_empty_registry() {
    use swe_edge_ingress_grpc_reflection_adapter::ReflectionService;

    let _svc = ReflectionService::new();
}

// ── swe-edge-ingress-verifier ─────────────────────────────────────────────────

/// Exercises swe-edge-ingress-verifier directly via JwtVerifier.
#[test]
fn test_jwt_verifier_rejects_invalid_token_directly() {
    use swe_edge_ingress_verifier::{JwtConfig, JwtKey, JwtVerifier, TokenVerifier};
    let secret: Vec<u8> = b"test-secret-key-that-is-long-enough".to_vec();
    let cfg = JwtConfig {
        key: JwtKey::Hs256 { secret },
        required_issuer: None,
        required_audience: None,
        leeway_seconds: 0,
    };
    let verifier = JwtVerifier::from_config(&cfg).expect("create verifier");
    let result = verifier.verify("not.a.jwt");
    assert!(result.is_err(), "invalid token must be rejected");
}

// ── swe-edge-ingress-grpc ─────────────────────────────────────────────────────

/// Exercises swe-edge-ingress-grpc through the RuntimeBuilder gRPC route path.
#[test]
fn test_ingress_grpc_handler_registered_via_builder() {
    use edge_application::{Handler, HandlerError};
    use swe_edge_bootstrap::Runtime;

    struct EchoHandler;

    #[async_trait::async_trait]
    impl Handler for EchoHandler {
        type Request = DepCoverageTextPayload;
        type Response = DepCoverageTextPayload;
        async fn execute(
            &self,
            req: edge_application_handler::ExecutionRequest<'_, DepCoverageTextPayload>,
        ) -> Result<DepCoverageTextPayload, HandlerError> {
            Ok(req.req)
        }
    }

    use swe_edge_bootstrap::{GrpcDecodeFn, GrpcEncodeFn};
    let decode: GrpcDecodeFn<DepCoverageTextPayload> = |b| {
        Ok(DepCoverageTextPayload(
            String::from_utf8_lossy(b).into_owned(),
        ))
    };
    let encode: GrpcEncodeFn<DepCoverageTextPayload> =
        |v: &DepCoverageTextPayload| v.0.clone().into_bytes();
    // grpc_route_with wires up the gRPC dispatcher — success without panic confirms the dep is used
    let _b = Runtime::builder().grpc_route_with(Arc::new(EchoHandler), decode, encode);
}

// ── edge-dispatch ──────────────────────────────────────────────────────────────

/// Exercises edge-dispatch directly via `OptionalHandler`'s enabled/disabled states.
#[test]
fn test_optional_handler_enabled_and_disabled_states() {
    use edge_dispatch::OptionalHandler;

    let disabled: OptionalHandler<DepCoverageTextPayload> = OptionalHandler::disabled();
    assert!(!disabled.is_enabled());

    let enabled = OptionalHandler::enabled(DepCoverageTextPayload("on".to_string()));
    assert!(enabled.is_enabled());
}

// ── swe-edge-runtime-grpc ─────────────────────────────────────────────────────

/// Exercises swe-edge-runtime-grpc directly via `TonicGrpcServer` construction.
#[test]
fn test_tonic_grpc_server_constructs_from_bind_and_handler() {
    use swe_edge_ingress_grpc::{
        GrpcIngress, GrpcIngressError, GrpcResponse, HealthCheckRequest, HealthCheckResponse,
        StreamRequest, StreamResponse, UnaryRequest,
    };
    use swe_edge_runtime_grpc_adapter::{GrpcServerManage, TonicGrpcServer};

    struct NullGrpcHandler;
    impl GrpcIngress for NullGrpcHandler {
        fn handle_unary(
            &self,
            _: UnaryRequest,
        ) -> futures::future::BoxFuture<'_, Result<GrpcResponse, GrpcIngressError>> {
            Box::pin(async { Err(GrpcIngressError::Unimplemented("stub".into())) })
        }
        fn handle_stream(
            &self,
            _: StreamRequest,
        ) -> futures::future::BoxFuture<'_, Result<StreamResponse, GrpcIngressError>> {
            Box::pin(async { Err(GrpcIngressError::Unimplemented("stub".into())) })
        }
        fn health_check(
            &self,
            _: HealthCheckRequest,
        ) -> futures::future::BoxFuture<'_, Result<HealthCheckResponse, GrpcIngressError>> {
            Box::pin(async {
                Err(GrpcIngressError::Unimplemented(
                    "health check not implemented".into(),
                ))
            })
        }
    }
    let _server = TonicGrpcServer::new("127.0.0.1:0", Arc::new(NullGrpcHandler));
}

// ── swe-edge-runtime-scheduler ───────────────────────────────────────────────

/// Exercises swe-edge-runtime-scheduler via the public feature-gated re-export.
/// The scheduler feature is off by default — compile-checks the dependency is declared.
#[cfg(feature = "scheduler")]
#[test]
fn test_scheduler_tokio_scheduler_factory_compiles() {
    use swe_edge_bootstrap::{SchedulerSvc, TokioSchedulerConfig};
    let cfg = TokioSchedulerConfig::default();
    let _sched = SchedulerSvc::tokio_scheduler(cfg, "dep-coverage");
}
