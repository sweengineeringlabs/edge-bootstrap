//! Integration test — proves JWT bearer auth goes through `RuntimeBuilder`
//! per the same pattern as `grpc_e2e_int_test.rs`/`observability_e2e_int_test.rs`:
//! `RuntimeConfig.http_auth` -> `JwtVerifier::from_config` ->
//! `AxumHttpServer::with_bearer_auth` -> real `401`/`200` HTTP responses
//! observed by a real `reqwest` client against a real socket. Not a unit
//! test of `JwtVerifier`/`AxumHttpServerHelper::verify_auth` in isolation
//! (those live in `swe-edge-ingress-verifier` and `edge-runtime`
//! respectively, and are already unit-tested there) — this proves the
//! wiring itself composes correctly through `RuntimeBuilder::serve()`,
//! which a unit test of either piece alone cannot show.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use edge_application::{Handler, HandlerError};
use edge_application_handler::{
    ExecutionRequest, IdRequest, IdResponse, PatternRequest, PatternResponse,
};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::Serialize;
use swe_edge_bootstrap::{JwtConfig, JwtKey, Runtime, RuntimeConfig};

/// Demo secret local to this test file — intentionally distinct from
/// `examples/security-e2e/runtime_config.rs`'s `DEMO_JWT_SECRET` so this
/// test's correctness never depends on that example's constant.
const TEST_HS256_SECRET: &[u8] = b"security-e2e-int-test-hs256-secret-value";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct SecurityE2ePayload {
    message: String,
}
impl edge_application::Request for SecurityE2ePayload {}
impl edge_application::Response for SecurityE2ePayload {}

struct EchoHandler;

#[async_trait]
impl Handler for EchoHandler {
    type Request = SecurityE2ePayload;
    type Response = SecurityE2ePayload;

    fn id(&self, _req: IdRequest) -> Result<IdResponse, HandlerError> {
        Ok(IdResponse {
            id: "/secure-echo".to_string(),
        })
    }

    fn pattern(&self, _req: PatternRequest) -> Result<PatternResponse, HandlerError> {
        // `Handler::pattern` defaults to an empty string — `DefaultHttpJob`
        // routes on this, not on `id`, so this must be set explicitly.
        Ok(PatternResponse {
            pattern: "/secure-echo".to_string(),
        })
    }

    async fn execute(
        &self,
        req: ExecutionRequest<'_, SecurityE2ePayload>,
    ) -> Result<SecurityE2ePayload, HandlerError> {
        Ok(req.req)
    }
}

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    listener.local_addr().expect("local_addr").port()
}

#[derive(Serialize)]
struct TestClaims {
    sub: String,
    exp: u64,
}

/// Mint a real HS256-signed JWT with `secret`, expiring `ttl_secs` from now
/// (a negative `ttl_secs` produces an already-expired token).
fn mint_token(secret: &[u8], sub: &str, ttl_secs: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock must be after the Unix epoch")
        .as_secs() as i64;
    let exp = (now + ttl_secs).max(0) as u64;
    let claims = TestClaims {
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

/// Spawn a real `RuntimeBuilder::serve()` with `RuntimeConfig.http_auth` set
/// to an HS256 verifier over `TEST_HS256_SECRET`, on an ephemeral port. gRPC
/// is left unregistered (no `.grpc_route()` call), so `.grpc_allow_unauthenticated()`
/// only exists to satisfy `serve()`'s fail-closed default — no gRPC listener
/// actually binds without a registered route.
async fn spawn_secured_server() -> (String, tokio::task::JoinHandle<()>) {
    let addr = format!("127.0.0.1:{}", free_port());
    let mut config = RuntimeConfig::default()
        .with_http_bind(addr.clone())
        .with_systemd_notify(false);
    config.http_auth = Some(JwtConfig {
        key: JwtKey::Hs256 {
            secret: TEST_HS256_SECRET.to_vec(),
        },
        required_issuer: None,
        required_audience: None,
        leeway_seconds: 5,
    });

    let handle = tokio::spawn(async move {
        let _ = Runtime::builder()
            .config(config)
            .http_route(Arc::new(EchoHandler))
            .grpc_allow_unauthenticated()
            .serve()
            .await;
    });

    // Give the server a moment to bind before connecting.
    tokio::time::sleep(Duration::from_millis(200)).await;
    (addr, handle)
}

#[tokio::test]
async fn test_secure_echo_without_authorization_header_returns_401() {
    let (addr, handle) = spawn_secured_server().await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/secure-echo"))
        .json(&SecurityE2ePayload {
            message: "hi".to_string(),
        })
        .send()
        .await
        .expect("request must reach the real AxumHttpServer");

    assert_eq!(
        response.status(),
        401,
        "a request with no Authorization header must be rejected before it reaches the handler"
    );

    handle.abort();
}

#[tokio::test]
async fn test_secure_echo_with_garbage_bearer_token_returns_401() {
    let (addr, handle) = spawn_secured_server().await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/secure-echo"))
        .header("authorization", "Bearer garbage.not.a.jwt")
        .json(&SecurityE2ePayload {
            message: "hi".to_string(),
        })
        .send()
        .await
        .expect("request must reach the real AxumHttpServer");

    assert_eq!(
        response.status(),
        401,
        "a bearer token that fails signature verification must be rejected"
    );

    handle.abort();
}

#[tokio::test]
async fn test_secure_echo_with_valid_hs256_token_returns_200_and_echoes_payload_happy() {
    let (addr, handle) = spawn_secured_server().await;
    let token = mint_token(TEST_HS256_SECRET, "demo-user", 3600);
    let payload = SecurityE2ePayload {
        message: "hi".to_string(),
    };

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/secure-echo"))
        .header("authorization", format!("Bearer {token}"))
        .json(&payload)
        .send()
        .await
        .expect("request must reach the real HttpIngressJob-routed handler");

    assert_eq!(
        response.status(),
        200,
        "a token signed with the exact secret RuntimeConfig.http_auth was given must be accepted"
    );
    let echoed: SecurityE2ePayload = response.json().await.expect("deserialize echoed payload");
    assert_eq!(
        echoed, payload,
        "the handler must echo back exactly what it received, proving the real \
         HttpIngressJob -> DefaultHttpJob -> Handler::execute chain ran end-to-end \
         after the JWT bearer-auth boundary accepted the request"
    );

    handle.abort();
}

#[tokio::test]
async fn test_secure_echo_with_expired_token_returns_401() {
    let (addr, handle) = spawn_secured_server().await;
    let token = mint_token(TEST_HS256_SECRET, "demo-user", -3600);

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/secure-echo"))
        .header("authorization", format!("Bearer {token}"))
        .json(&SecurityE2ePayload {
            message: "hi".to_string(),
        })
        .send()
        .await
        .expect("request must reach the real AxumHttpServer");

    assert_eq!(
        response.status(),
        401,
        "a token that expired an hour ago must be rejected even though it is well-formed and correctly signed"
    );

    handle.abort();
}
