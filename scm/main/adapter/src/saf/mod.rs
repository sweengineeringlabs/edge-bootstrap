//! SAF layer — daemon public facade.

mod bootstrap_svc;

pub use crate::core::{Runtime, RuntimeBuilder, ServerConfigLoader, ServerMonitor};
pub use swe_edge_bootstrap_config::ConfigError;
pub use swe_edge_bootstrap_config::ConfigLoader;
pub use swe_edge_bootstrap_egress::Egress;
pub use swe_edge_bootstrap_health::HealthHandler;
pub use swe_edge_bootstrap_ingress::Ingress;
pub use swe_edge_bootstrap_runtime::ComponentHealth;
pub use swe_edge_bootstrap_runtime::RuntimeManager;
pub use swe_edge_bootstrap_runtime::ServiceRegistry;
pub use swe_edge_bootstrap_runtime::{RuntimeConfig, RuntimeHealth, RuntimeStatus};
pub use swe_edge_bootstrap_runtime::{RuntimeError, RuntimeResult};

// ── Auth / TLS ────────────────────────────────────────────────────────────────
pub use edge_security_runtime_tls::PemTlsConfig;
pub use swe_edge_ingress_grpc::{
    AuthorizationInterceptor, GrpcIngressInterceptor, GrpcIngressInterceptorChain,
};
pub use swe_edge_ingress_verifier::{
    Claims, JwtConfig, JwtKey, JwtVerifier, TokenVerifier, VerifierError,
};

// ── Handler decorators + config-driven assembly ───────────────────────────────
pub use edge_dispatch::{
    Cache, CacheAsideHandler, CacheAsideResponse, EventEmittingHandler, FallbackHandler,
    FallbackPolicy, MemoryCache, OptionalHandler, TimeoutHandler, TimeoutPolicy,
};
pub use swe_edge_bootstrap_config::{FeatureHandlerBridge, HandlerFactory};
pub use swe_edge_configbuilder::{FeatureRegistry, FeatureState, OptionalSection};

// ── Ingress surface (handlers + request/response types) ───────────────────────
pub use edge_application::{Handler, HandlerError};
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
pub use swe_edge_egress_grpc::{GrpcEgress, GrpcEgressError, GrpcEgressResult};
pub use swe_edge_egress_http::{HttpEgress, HttpEgressError, HttpEgressResult, HttpStreamResponse};

// ── Backend-pool load balancing (per-target-service egress) ──────────────────
// Ownership moved here from edge-transport-http-egress's `transport` crate —
// see ADR-004.
pub use crate::core::egress::{LoadBalancedHttpEgress, LoadBalancedHttpEgressError};
pub use swe_edge_bootstrap_runtime::ServiceEgressConfig;
pub use swe_edge_loadbalancer::{
    BackendConfig, BackendHealth, BackendId, BackendPoolInstance, LoadbalancerConfig,
    LoadbalancerError, LoadbalancerSvc, Outcome, Strategy,
};

// ── Lifecycle / health ────────────────────────────────────────────────────────
pub use edge_proxy::{HealthResponse, LifecycleMonitor, ProxySvc};

// ── Load monitoring / auto-scaling ────────────────────────────────────────────
pub use swe_edge_bootstrap_monitor::RingBuffer;
pub use swe_edge_bootstrap_monitor::{
    AutoscalePolicy, MetricsConfig, SharedCounters, TrafficCounters,
};
pub use swe_edge_bootstrap_runtime::{
    LogBackendConfig, LogBackendKind, LogElkSettings, LogFileSettings, LogOtelSettings,
    LogSqliteSettings, MetricsBackendConfig, MetricsBackendKind, MetricsFileSettings,
    MetricsOtelSettings, MetricsPrometheusSettings, MetricsSqliteSettings, TracerBackendConfig,
    TracerBackendKind,
};
pub use swe_observ_metrics::{MetricSnapshot, MetricType, MetricsProvider};

// ── Observability ─────────────────────────────────────────────────────────────
#[cfg(feature = "observability")]
pub use swe_edge_bootstrap_runtime::TracingInitializer;
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
