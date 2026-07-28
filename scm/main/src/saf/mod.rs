//! SAF layer — daemon public facade.

mod bootstrap_svc;

pub use crate::core::{Runtime, RuntimeBuilder, ServerConfigLoader, ServerMonitor};
pub use swe_edge_bootstrap_config_port::ConfigError;
pub use swe_edge_bootstrap_config_port::ConfigLoader;
pub use swe_edge_bootstrap_egress_port::Egress;
pub use swe_edge_bootstrap_health_port::HealthHandler;
pub use swe_edge_bootstrap_ingress_port::Ingress;
pub use swe_edge_bootstrap_runtime_port::ComponentHealth;
pub use swe_edge_bootstrap_runtime_port::RuntimeManager;
pub use swe_edge_bootstrap_runtime_port::ServiceRegistry;
pub use swe_edge_bootstrap_runtime_port::{RuntimeConfig, RuntimeHealth, RuntimeStatus};
pub use swe_edge_bootstrap_runtime_port::{RuntimeError, RuntimeResult};

// ── Auth / TLS ────────────────────────────────────────────────────────────────
pub use edge_security_runtime_tls::PemTlsConfig;
pub use swe_edge_ingress_grpc::{
    AuthorizationInterceptor, GrpcIngressInterceptor, GrpcIngressInterceptorChain,
};
pub use swe_edge_ingress_verifier::{
    Claims, JwtConfig, JwtKey, JwtVerifier, TokenVerifier, VerifierError,
};

// ── Handler decorators + config-driven assembly ───────────────────────────────
pub use swe_edge_bootstrap_config_port::{FeatureHandlerBridge, HandlerFactory};
pub use edge_dispatch::{
    Cache, CacheAsideHandler, CacheAsideResponse, EventEmittingHandler, FallbackHandler,
    FallbackPolicy, MemoryCache, OptionalHandler, TimeoutHandler, TimeoutPolicy,
};
pub use swe_edge_configbuilder::{FeatureRegistry, FeatureState, OptionalSection};

// ── Ingress surface (handlers + request/response types) ───────────────────────
pub use edge_domain::{Handler, HandlerError};
pub use swe_edge_ingress_grpc::{
    GrpcDecodeFn, GrpcEncodeFn, GrpcHealthCheck, GrpcIngress, GrpcIngressError, GrpcIngressResult,
    GrpcMessageStream, GrpcMetadata, GrpcRequest, GrpcResponse, GrpcStatusCode,
};
pub use swe_edge_ingress_http::{
    HealthCheckRequest, HealthCheckResponse, HttpAuth, HttpBody, HttpConfig, HttpDecodeFn,
    HttpEncodeFn, HttpFuture, HttpHealthCheck, HttpIngress, HttpIngressError, HttpMethod,
    HttpRequest, HttpResponse, HttpStream, InboundRequest, RequestContext, SecurityContext,
};

// ── Egress surface (outbound clients) ─────────────────────────────────────────
pub use swe_edge_egress_grpc::{GrpcEgress, GrpcEgressError, GrpcEgressResult, TonicGrpcClient};
pub use swe_edge_egress_http::{HttpEgress, HttpEgressError, HttpEgressResult, HttpStreamResponse};

// ── Lifecycle / health ────────────────────────────────────────────────────────
pub use edge_proxy::{HealthResponse, LifecycleMonitor, ProxySvc};

// ── Load monitoring / auto-scaling ────────────────────────────────────────────
pub use swe_edge_bootstrap_monitor_port::RingBuffer;
pub use swe_edge_bootstrap_monitor_port::{AutoscalePolicy, MetricsConfig, SharedCounters, TrafficCounters};
pub use swe_observ_metrics::{MetricSnapshot, MetricType, MetricsProvider};

// ── Observability ─────────────────────────────────────────────────────────────
#[cfg(feature = "observability")]
pub use swe_edge_bootstrap_runtime_port::TracingInitializer;
pub use swe_edge_observ_config::{ObservabilityConfig, TracingConfig, TracingFormat, TracingLevel};

// ── Scheduler ─────────────────────────────────────────────────────────────────
#[cfg(feature = "scheduler")]
pub use swe_edge_runtime_scheduler::{Scheduler, SchedulerSvc, TokioSchedulerConfig};

// ── Message broker ────────────────────────────────────────────────────────────
#[cfg(feature = "message-broker")]
pub use swe_edge_runtime_message_broker::MessageBrokerFactory;
#[cfg(feature = "message-broker")]
pub use swe_edge_runtime_message_broker::{Message, MessageBroker, MessageStream};

// ── Security context propagation ──────────────────────────────────────────────
#[cfg(feature = "security")]
pub use edge_security_runtime::{
    Principal, PrincipalRequest, PrincipalResponse, SecurityContextBuilder, SecurityError,
};
#[cfg(feature = "security")]
pub use edge_security_runtime_authz::{AuthzPolicy, NoopAuthzPolicy};
#[cfg(feature = "security")]
pub use edge_security_runtime_credential::{
    CredentialResolver, CredentialSource, SecretString, TenantId,
};
#[cfg(feature = "security")]
pub use edge_security_runtime_tls::PeerIdentity;
