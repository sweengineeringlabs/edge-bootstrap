//! SAF — runtime server public factory surface.
//!
//! All public functions are methods on factory types (`ServerConfigLoader`,
//! `ServerMonitor`, `Runtime`) to satisfy SEA Rule 191 (no free-standing fns).

use std::sync::Arc;
use std::time::Duration;

use crate::api::config::traits::loader::ConfigLoader;
use crate::api::config::ConfigError;
use crate::api::egress::Egress;
use crate::api::ingress::Ingress;
use crate::api::runtime::Runtime;
use crate::api::runtime::RuntimeConfig;
use crate::api::runtime::ServerConfigLoader;
use crate::api::runtime::ServerMonitor;
use crate::api::runtime::{RuntimeError, RuntimeResult};
use crate::api::validator::Validator;
use crate::core::egress::DefaultEgress;
use crate::core::ingress::DefaultIngress;
use crate::core::runner::DaemonRunner;
use crate::core::validator::ConfigValidator;
use crate::core::ApplicationConfigLoader;
use edge_proxy::LifecycleMonitor;
use swe_observ_metrics::MetricsProvider;

// ── ServerConfigLoader methods ─────────────────────────────────────────────────

impl ServerConfigLoader {
    /// Return a config builder pre-seeded with this crate's package name and version.
    pub fn create_config_builder() -> swe_edge_configbuilder::ConfigBuilderImpl {
        let mut b = swe_edge_configbuilder::ConfigBuilderImpl::new();
        b = b.with_name(env!("CARGO_PKG_NAME"));
        b = b.with_version(env!("CARGO_PKG_VERSION"));
        b
    }

    /// Load config using the default layered chain
    /// (`default.toml` → `application.toml` → env vars).
    ///
    /// The config directory is resolved from `SWE_EDGE_CONFIG_DIR` or
    /// defaults to `config/` relative to the working directory.
    /// Consumer apps should prefer [`ServerConfigLoader::load_config_from`] to supply their
    /// own path explicitly.
    pub fn load_config() -> Result<RuntimeConfig, ConfigError> {
        let loader = ApplicationConfigLoader::new();
        loader.load()
    }

    /// Load config from an explicit directory.
    ///
    /// Identical layer chain to [`ServerConfigLoader::load_config`] but reads
    /// `<dir>/application.toml` instead of relying on env or cwd.
    pub fn load_config_from(
        dir: impl Into<std::path::PathBuf>,
    ) -> Result<RuntimeConfig, ConfigError> {
        let loader = ApplicationConfigLoader::with_dir(dir);
        loader.load()
    }

    /// Load config scoped to a tenant
    /// (`default.toml` → `application.toml` → `tenants/<id>.toml` → env vars).
    pub fn load_tenant_config(tenant_id: &str) -> Result<RuntimeConfig, ConfigError> {
        let loader = ApplicationConfigLoader::new();
        loader.load_for_tenant(tenant_id)
    }

    /// Load tenant config from an explicit directory.
    ///
    /// Reads `<dir>/application.toml` and `<dir>/tenants/<tenant_id>.toml`.
    pub fn load_tenant_config_from(
        tenant_id: &str,
        dir: impl Into<std::path::PathBuf>,
    ) -> Result<RuntimeConfig, ConfigError> {
        let loader = ApplicationConfigLoader::with_dir(dir);
        loader.load_for_tenant(tenant_id)
    }

    /// Load config following the XDG Base Directory specification.
    pub fn load_config_xdg(app_name: &str) -> Result<RuntimeConfig, ConfigError> {
        let loader = ApplicationConfigLoader::xdg(app_name);
        loader.load()
    }

    /// Load tenant config following the XDG Base Directory specification.
    pub fn load_tenant_config_xdg(
        app_name: &str,
        tenant_id: &str,
    ) -> Result<RuntimeConfig, ConfigError> {
        let loader = ApplicationConfigLoader::xdg(app_name);
        loader.load_for_tenant(tenant_id)
    }

    /// Validate a [`RuntimeConfig`] using the built-in [`ConfigValidator`].
    ///
    /// Returns `Err(RuntimeError::StartFailed)` if any field is out of bounds.
    pub fn validate_config(config: &RuntimeConfig) -> Result<(), RuntimeError> {
        ConfigValidator.validate(config)
    }

    /// Load an arbitrary TOML section from the default config chain.
    ///
    /// `key` is a dotted path, e.g. `"observability.tracing"` or
    /// `"application.completion"`.
    pub fn load_section<T>(key: &str) -> Result<T, ConfigError>
    where
        T: serde::de::DeserializeOwned + Default,
    {
        let loader = ApplicationConfigLoader::new();
        loader.load_section(key)
    }

    /// Load an arbitrary TOML section from an explicit config directory.
    pub fn load_section_from<T>(
        key: &str,
        dir: impl Into<std::path::PathBuf>,
    ) -> Result<T, ConfigError>
    where
        T: serde::de::DeserializeOwned + Default,
    {
        let loader = ApplicationConfigLoader::with_dir(dir);
        loader.load_section(key)
    }

    /// Load an arbitrary TOML section following the XDG Base Directory chain.
    pub fn load_section_xdg<T>(app_name: &str, key: &str) -> Result<T, ConfigError>
    where
        T: serde::de::DeserializeOwned + Default,
    {
        let loader = ApplicationConfigLoader::xdg(app_name);
        loader.load_section(key)
    }
}

// ── ServerMonitor methods ──────────────────────────────────────────────────────

impl ServerMonitor {
    /// Wrap any [`LifecycleMonitor`] with health-state gauge recording.
    ///
    /// After every `health()` call each component's status is emitted as
    /// `edge_component_health` (1.0 = Healthy, 0.5 = Degraded, 0.0 = Unhealthy).
    pub fn observe(
        inner: Arc<dyn LifecycleMonitor>,
        provider: Arc<dyn MetricsProvider>,
    ) -> Arc<dyn LifecycleMonitor> {
        let monitor = crate::core::monitor::MetricsLifecycleMonitor::new(inner, provider);
        let wrapped: Arc<dyn LifecycleMonitor> = Arc::new(monitor);
        wrapped
    }
}

// ── Runtime SAF methods ────────────────────────────────────────────────────────

impl Runtime {
    /// Assemble a runtime manager from the supplied config, ingress, egress,
    /// and lifecycle monitor.
    pub fn runtime_manager(
        config: RuntimeConfig,
        ingress: Arc<dyn Ingress>,
        egress: Arc<dyn Egress>,
        lifecycle: Arc<dyn LifecycleMonitor>,
    ) -> impl crate::api::runtime::traits::runtime_manager::RuntimeManager {
        let mgr = crate::core::DefaultRuntimeManager::new(config, ingress, egress, lifecycle);
        mgr
    }

    /// Start the daemon and block until a shutdown signal is received.
    ///
    /// Calls [`RuntimeManager::start`], waits for `SIGTERM` or `SIGINT`
    /// (Ctrl+C on all platforms), then calls [`RuntimeManager::shutdown`].
    /// If shutdown does not complete within `config.shutdown_timeout_secs`,
    /// returns [`RuntimeError::ShutdownTimeout`].
    pub async fn run(
        config: RuntimeConfig,
        ingress: Arc<dyn Ingress>,
        egress: Arc<dyn Egress>,
        lifecycle: Arc<dyn LifecycleMonitor>,
    ) -> RuntimeResult<()> {
        use swe_edge_ingress_grpc::TonicGrpcServer;
        use swe_edge_ingress_http::AxumHttpServer;
        use swe_edge_ingress_verifier::{JwtVerifier, TokenVerifier};
        use tokio::sync::oneshot;

        let timeout_secs = config.shutdown_timeout_secs;
        let http_bind = config.http_bind.clone();
        let grpc_bind = config.grpc_bind.clone();
        let http_tls = config.http_tls.clone();
        let grpc_tls = config.grpc_tls.clone();
        let grpc_allow_unauthenticated = config.grpc_allow_unauthenticated;

        let http_bearer_verifier: Option<Arc<dyn TokenVerifier>> =
            if let Some(ref auth_cfg) = config.http_auth {
                Some(Arc::new(
                    JwtVerifier::from_config(auth_cfg)
                        .map_err(|e| RuntimeError::StartFailed(e.to_string()))?,
                ))
            } else {
                None
            };

        let (http_shutdown_tx, http_shutdown_rx) = oneshot::channel::<()>();
        let http_task = ingress.http().map(|handler| {
            let mut server = AxumHttpServer::new(http_bind, handler);
            if let Some(tls) = http_tls {
                server = server.with_tls(tls);
            }
            if let Some(verifier) = http_bearer_verifier {
                server = server.with_bearer_auth(verifier);
            }
            tokio::spawn(async move {
                let signal = async move {
                    let _ = http_shutdown_rx.await;
                };
                if let Err(e) = server.serve(signal).await {
                    tracing::error!("HTTP server error: {e}");
                }
            })
        });

        let (grpc_shutdown_tx, grpc_shutdown_rx) = oneshot::channel::<()>();
        let grpc_task = ingress.grpc().map(|handler| {
            let mut server = TonicGrpcServer::new(grpc_bind, handler);
            if let Some(tls) = grpc_tls {
                server = server.with_tls(tls);
            }
            if grpc_allow_unauthenticated {
                server = server.allow_unauthenticated(true);
            }
            tokio::spawn(async move {
                let signal = async move {
                    let _ = grpc_shutdown_rx.await;
                };
                if let Err(e) = server.serve(signal).await {
                    tracing::error!("gRPC server error: {e}");
                }
            })
        });

        let mgr = Runtime::runtime_manager(config, ingress, egress, lifecycle);
        let result =
            DaemonRunner::run_until_signal(mgr, timeout_secs, Runtime::wait_for_signal()).await;

        let _ = http_shutdown_tx.send(());
        let _ = grpc_shutdown_tx.send(());
        if let Some(task) = http_task {
            let _ = tokio::time::timeout(Duration::from_secs(5), task).await;
        }
        if let Some(task) = grpc_task {
            let _ = tokio::time::timeout(Duration::from_secs(5), task).await;
        }

        result
    }

    /// Wait for SIGTERM or SIGINT, whichever arrives first.
    async fn wait_for_signal() {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigterm = match signal(SignalKind::terminate()) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(
                        "could not register SIGTERM handler: {e} — falling back to SIGINT only"
                    );
                    let _ = tokio::signal::ctrl_c().await;
                    return;
                }
            };
            tokio::select! {
                _ = sigterm.recv()           => tracing::info!("SIGTERM received"),
                _ = tokio::signal::ctrl_c() => tracing::info!("SIGINT received"),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("SIGINT received");
        }
    }

    /// Construct an egress adapter holding an HTTP-only outbound client.
    pub fn http_egress(
        http: Arc<dyn swe_edge_egress_http::HttpEgress>,
    ) -> impl crate::api::egress::Egress {
        let egress = DefaultEgress::new_http(http);
        egress
    }

    /// Construct an egress adapter holding both HTTP and gRPC outbound clients.
    pub fn http_grpc_egress(
        http: Arc<dyn swe_edge_egress_http::HttpEgress>,
        grpc: Arc<dyn swe_edge_egress_grpc::GrpcEgress>,
    ) -> impl crate::api::egress::Egress {
        let egress = DefaultEgress::new_http(http).with_grpc(grpc);
        egress
    }

    /// Construct an ingress adapter with no transports (add via `with_http`/`with_grpc` on serve).
    pub fn empty_ingress() -> impl crate::api::ingress::Ingress {
        let ingress = DefaultIngress::empty();
        ingress
    }

    /// Construct an ingress adapter bound to an HTTP transport.
    pub fn http_ingress(
        http: Arc<dyn swe_edge_ingress_http::HttpIngress>,
    ) -> impl crate::api::ingress::Ingress {
        let ingress = DefaultIngress::new_http(http);
        ingress
    }

    /// Construct an ingress adapter bound to a gRPC transport.
    pub fn grpc_ingress(
        grpc: Arc<dyn swe_edge_ingress_grpc::GrpcIngress>,
    ) -> impl crate::api::ingress::Ingress {
        let ingress = DefaultIngress::new_grpc(grpc);
        ingress
    }

    /// Construct an ingress adapter bound to both HTTP and gRPC transports.
    pub fn http_grpc_ingress(
        http: Arc<dyn swe_edge_ingress_http::HttpIngress>,
        grpc: Arc<dyn swe_edge_ingress_grpc::GrpcIngress>,
    ) -> impl crate::api::ingress::Ingress {
        let ingress = DefaultIngress::new_http(http).with_grpc(grpc);
        ingress
    }
}
