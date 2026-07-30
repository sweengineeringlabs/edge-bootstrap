//! `RuntimeBuilder::serve()` implementation.

/// Primary type for this module (matches filename for Rule 89).
pub(crate) struct RuntimeBuilderServe;

use std::sync::Arc;
use std::time::Duration;

use edge_proxy::ProxySvc;
use swe_edge_egress_grpc::TransportSvc as GrpcTransportSvc;
use swe_edge_egress_http::HttpTransportSvc;
use swe_edge_ingress_grpc_reflection::ReflectionService;
use swe_edge_ingress_verifier::{JwtVerifier, TokenVerifier};
use swe_edge_runtime_grpc::{
    AllowUnauthenticatedFlagRequest, GrpcServerManage, TonicGrpcServer, WithInterceptorsRequest,
    WithTlsRequest,
};
use swe_edge_runtime_http::{HttpServer, ServeWithShutdownRequest};
use swe_edge_runtime_http_adapter::AxumHttpServer;
use tokio::sync::oneshot;

use crate::core::config::loader::ApplicationConfigLoader;
use crate::core::egress::DefaultEgress;
use crate::core::ingress::DefaultIngress;
#[cfg(feature = "intrusion")]
use crate::core::intrusion::{GrpcIntrusionGuard, HttpIntrusionGuard};
use crate::core::metrics::handler::MetricsHandler;
use crate::core::monitor::{BackgroundSampler, GrpcLoadMonitor, HttpLoadMonitor};
use crate::core::runner::DaemonRunner;
use crate::core::runtime::manager::DefaultRuntimeManager;
use crate::core::RuntimeBuilder;
use swe_edge_bootstrap_config::ConfigLoader;
use swe_edge_bootstrap_ingress::Ingress;
use swe_edge_bootstrap_monitor::{SharedCounters, TrafficCounters};
use swe_edge_bootstrap_runtime::{
    LogBackendConfig, LogBackendKind, MetricsBackendConfig, MetricsBackendKind, RuntimeError,
    RuntimeResult, TracerBackendConfig, TracerBackendKind,
};
#[cfg(feature = "observability")]
use swe_observ_logging::LoggerProvider;
use swe_observ_metrics::{create_local_metrics_backend, MetricsProvider};
#[cfg(feature = "observability")]
use swe_observ_tracing::TracerProvider;

const DEFAULT_APP_NAME: &str = "swe-edge";

impl RuntimeBuilder {
    /// Assemble all registered components and start the runtime.
    ///
    /// Blocks until SIGTERM / SIGINT or an error.
    pub async fn serve(self) -> RuntimeResult<()> {
        let config = match self.config {
            Some(c) => c,
            None => {
                let name = self.app_name.as_deref().unwrap_or(DEFAULT_APP_NAME);
                ApplicationConfigLoader::xdg(name)
                    .load()
                    .map_err(|e| RuntimeError::StartFailed(e.to_string()))?
            }
        };

        // Builder explicit wins; fall back to [observability.tracing] from TOML.
        #[cfg(feature = "observability")]
        {
            let tracing_cfg = self
                .tracing_config
                .as_ref()
                .or_else(|| config.observability.as_ref().map(|o| &o.tracing));
            if let Some(cfg) = tracing_cfg {
                swe_edge_bootstrap_runtime::TracingInitializer::init(cfg);
            }
        }

        // ── Resolve TLS / auth: builder explicit wins, else fall back to config ─
        let http_tls = self.http_tls.or_else(|| config.http_tls.clone());
        let grpc_tls = self.grpc_tls.or_else(|| config.grpc_tls.clone());

        let http_bearer_verifier: Option<Arc<dyn TokenVerifier>> =
            if let Some(v) = self.http_bearer_verifier {
                Some(v)
            } else if let Some(ref auth_cfg) = config.http_auth {
                Some(Arc::new(
                    JwtVerifier::from_config(auth_cfg)
                        .map_err(|e| RuntimeError::StartFailed(e.to_string()))?,
                ))
            } else {
                None
            };

        let grpc_allow_unauthenticated =
            self.grpc_allow_unauthenticated || config.grpc_allow_unauthenticated;

        // ── Load monitor — shared counters + background sampler ───────────────
        // Backend: builder explicit > [metrics_backend] TOML config > in-memory default.
        // Resolved before Ingress so the same provider instance can also back
        // ObserverContext.metrics() below — one metrics stream, not two.
        let metrics_provider: Option<Arc<dyn MetricsProvider>> = match config.metrics.as_ref() {
            Some(_) => Some(match self.metrics_provider {
                Some(p) => p,
                None => Self::build_metrics_provider(config.metrics_backend.as_ref())?,
            }),
            None => None,
        };
        let counters: Option<SharedCounters> = metrics_provider.as_ref().map(|provider| {
            let c = Arc::new(TrafficCounters::new(Arc::clone(provider)));
            let sampler = BackgroundSampler::new(Arc::clone(&c), config.autoscale.clone());
            tokio::spawn(async move { sampler.run().await });
            c
        });

        // ── ObserverContext tracer/log backends ─────────────────────────────────
        // Backend: builder explicit > [tracer_backend]/[log_backend] TOML config >
        // in-memory default. Always resolved (unlike metrics, which only builds a
        // backend when the scrape endpoint is configured) — tracer/log drain are
        // always active for the ObserverContext bridge.
        #[cfg(feature = "observability")]
        let tracer_provider: Arc<dyn TracerProvider> = match self.tracer_provider {
            Some(p) => p,
            None => Self::build_tracer_provider(config.tracer_backend.as_ref())?,
        };
        #[cfg(feature = "observability")]
        let log_drain_backend: Arc<dyn LoggerProvider> = match self.log_drain_backend {
            Some(b) => b,
            None => Self::build_log_backend(config.log_backend.as_ref())?,
        };
        #[cfg(feature = "observability")]
        let observer_ctx = Self::resolve_observer_context(
            self.observer_context_override,
            &tracer_provider,
            &log_drain_backend,
            &metrics_provider,
        );

        // ── Ingress ───────────────────────────────────────────────────────────
        // Capture gRPC registry before the dispatcher is consumed into input.
        let reflection_registry = if config.grpc_reflection {
            self.grpc_dispatcher
                .as_ref()
                .map(|d| Arc::clone(d.registry()))
        } else {
            None
        };

        let mut input = DefaultIngress::empty();
        if let Some(d) = self.http_dispatcher {
            // Wire a real ObserverContext so every Handler::execute() call —
            // infra's own and any consumer's — opens real, exportable spans
            // via HandlerContext.observer instead of the framework's noop
            // default. See docs/3-design/adr/ for the decision record.
            #[cfg(feature = "observability")]
            let d = d.with_observer_context(Arc::clone(&observer_ctx));
            input = input.with_http(Arc::new(d));
        } else if let Some(h) = self.http_handler {
            input = input.with_http(h);
        }

        if let Some(d) = self.grpc_dispatcher {
            input = input.with_grpc(Arc::new(d));
        } else if let Some(h) = self.grpc_handler {
            input = input.with_grpc(h);
        }

        if !input.has_any() {
            return Err(RuntimeError::StartFailed(
                "Runtime: no handler registered — call .http_route() or .grpc_route()".into(),
            ));
        }

        // ── Egress: builder explicit > TOML config > default ─────────────────
        let egress_http: Arc<dyn swe_edge_egress_http::HttpEgress> =
            if let Some(h) = self.egress_http {
                h
            } else if let Some(http_cfg) = config.egress_http.clone() {
                Arc::from(
                    HttpTransportSvc::default_http_egress_with_config(http_cfg)
                        .map_err(|e| RuntimeError::StartFailed(e.to_string()))?,
                )
            } else {
                Arc::from(
                    HttpTransportSvc::default_http_egress()
                        .map_err(|e| RuntimeError::StartFailed(e.to_string()))?,
                )
            };

        let egress_grpc: Option<Arc<dyn swe_edge_egress_grpc::GrpcEgress>> =
            if let Some(g) = self.egress_grpc {
                Some(g)
            } else if let Some(ref grpc_cfg) = config.egress_grpc {
                Some(
                    GrpcTransportSvc::create_transport_from_config(grpc_cfg)
                        .map_err(|e| RuntimeError::StartFailed(e.to_string()))?,
                )
            } else {
                None
            };

        let mut output = DefaultEgress::new_http(egress_http);
        if let Some(g) = egress_grpc {
            output = output.with_grpc(g);
        }

        let lifecycle = self
            .lifecycle
            .unwrap_or_else(|| ProxySvc::new_null_lifecycle_monitor());

        // ── Intrusion guard — builder explicit wins, else [intrusion] TOML config ─
        #[cfg(feature = "intrusion")]
        let intrusion_wired: Option<Arc<edge_intrusion::config::Wired>> = match self.intrusion {
            Some(w) => Some(Arc::new(w)),
            None => match config.intrusion.as_ref() {
                Some(cfg) => Some(Arc::new(
                    cfg.build()
                        .map_err(|e| RuntimeError::StartFailed(e.to_string()))?,
                )),
                None => None,
            },
        };

        // ── Servers ───────────────────────────────────────────────────────────
        let timeout_secs = config.shutdown_timeout_secs;
        let http_bind = config.http_bind.clone();
        let grpc_bind = config.grpc_bind.clone();
        let stream_handler = self.stream_handler;
        let metrics_bind = config.metrics.as_ref().map(|m| m.bind.clone());
        let metrics_path = config
            .metrics
            .as_ref()
            .map(|m| m.path.clone())
            .unwrap_or_else(|| "/metrics".into());

        let (http_tx, http_rx) = oneshot::channel::<()>();
        let http_task = input.http().map(|handler| {
            // Wrap with load monitor if metrics are enabled.
            let handler: Arc<dyn swe_edge_ingress_http::HttpIngress> = if let Some(ref c) = counters
            {
                Arc::new(HttpLoadMonitor::new(handler, Arc::clone(c)))
            } else {
                handler
            };
            // Wrap with intrusion guard if edge-intrusion is configured — runs
            // before load-monitor recording, so rejected requests aren't counted.
            #[cfg(feature = "intrusion")]
            let handler: Arc<dyn swe_edge_ingress_http::HttpIngress> =
                if let Some(ref w) = intrusion_wired {
                    Arc::new(HttpIntrusionGuard::new(handler, Arc::clone(w)))
                } else {
                    handler
                };
            let mut server = AxumHttpServer::new(http_bind, handler);
            if let Some(tls) = http_tls {
                server = server.with_tls(tls);
            }
            if let Some(verifier) = http_bearer_verifier {
                server = server.with_bearer_auth(verifier);
            }
            if let Some(sh) = stream_handler {
                server = server.with_stream_handler(sh);
            }
            tokio::spawn(async move {
                let signal = async move {
                    let _ = http_rx.await;
                };
                if let Err(e) = server
                    .serve_with_shutdown(ServeWithShutdownRequest {
                        shutdown: Box::pin(signal),
                    })
                    .await
                {
                    tracing::error!("HTTP server error: {e}");
                }
            })
        });

        let (grpc_tx, grpc_rx) = oneshot::channel::<()>();
        let grpc_task = input
            .grpc()
            .map(|handler| {
                // Wrap with load monitor if metrics are enabled.
                let handler: Arc<dyn swe_edge_ingress_grpc::GrpcIngress> =
                    if let Some(ref c) = counters {
                        Arc::new(GrpcLoadMonitor::new(handler, Arc::clone(c)))
                    } else {
                        handler
                    };
                // Wrap with reflection if enabled and a dispatcher registry was captured.
                let handler: Arc<dyn swe_edge_ingress_grpc::GrpcIngress> =
                    if let Some(registry) = reflection_registry {
                        Arc::new(crate::core::composite::CompositeGrpcIngress::new(
                            handler,
                            Arc::new(ReflectionService::new(registry)),
                        ))
                    } else {
                        handler
                    };
                // Wrap with intrusion guard if edge-intrusion is configured — applies
                // to reflection calls too, since it wraps the outermost handler.
                #[cfg(feature = "intrusion")]
                let handler: Arc<dyn swe_edge_ingress_grpc::GrpcIngress> =
                    if let Some(ref w) = intrusion_wired {
                        Arc::new(GrpcIntrusionGuard::new(handler, Arc::clone(w)))
                    } else {
                        handler
                    };
                let mut server = TonicGrpcServer::new(grpc_bind, handler);
                if let Some(tls) = grpc_tls {
                    server = server
                        .with_tls(WithTlsRequest { tls })
                        .map_err(|e| RuntimeError::StartFailed(e.to_string()))?;
                }
                if !self.grpc_interceptors.is_empty() {
                    server = server
                        .with_interceptors(WithInterceptorsRequest {
                            chain: self.grpc_interceptors,
                        })
                        .map_err(|e| RuntimeError::StartFailed(e.to_string()))?;
                }
                if grpc_allow_unauthenticated {
                    server = server
                        .allow_unauthenticated(AllowUnauthenticatedFlagRequest { allow: true })
                        .map_err(|e| RuntimeError::StartFailed(e.to_string()))?;
                }
                Ok::<_, RuntimeError>(tokio::spawn(async move {
                    let signal = async move {
                        let _ = grpc_rx.await;
                    };
                    if let Err(e) = server.serve(signal).await {
                        tracing::error!("gRPC server error: {e}");
                    }
                }))
            })
            .transpose()?;

        // ── Metrics server ────────────────────────────────────────────────────
        let (metrics_tx, metrics_task) =
            if let (Some(bind), Some(ref c)) = (metrics_bind, &counters) {
                let (tx, rx) = oneshot::channel::<()>();
                let server = AxumHttpServer::new(
                    bind,
                    Arc::new(MetricsHandler::new(Arc::clone(c), &metrics_path)),
                );
                let task = tokio::spawn(async move {
                    let signal = async move {
                        let _ = rx.await;
                    };
                    if let Err(e) = server
                        .serve_with_shutdown(ServeWithShutdownRequest {
                            shutdown: Box::pin(signal),
                        })
                        .await
                    {
                        tracing::error!("metrics server error: {e}");
                    }
                });
                (Some(tx), Some(task))
            } else {
                (None, None)
            };

        #[cfg(not(feature = "message-broker"))]
        let mgr = DefaultRuntimeManager::new(config, Arc::new(input), Arc::new(output), lifecycle);
        #[cfg(feature = "message-broker")]
        let mgr = {
            let mut m =
                DefaultRuntimeManager::new(config, Arc::new(input), Arc::new(output), lifecycle);
            if let Some(broker) = self.message_broker {
                m = m.with_message_broker(broker);
            }
            m
        };
        let result = DaemonRunner::run_until_signal(
            mgr,
            timeout_secs,
            RuntimeBuilderServe::wait_for_signal(),
        )
        .await;

        let _ = http_tx.send(());
        let _ = grpc_tx.send(());
        if let Some(tx) = metrics_tx {
            let _ = tx.send(());
        }
        if let Some(t) = http_task {
            let _ = tokio::time::timeout(Duration::from_secs(5), t).await;
        }
        if let Some(t) = grpc_task {
            let _ = tokio::time::timeout(Duration::from_secs(5), t).await;
        }
        if let Some(t) = metrics_task {
            let _ = tokio::time::timeout(Duration::from_secs(5), t).await;
        }

        result
    }

    /// Resolve the `MetricsProvider` backend from `[metrics_backend]` TOML
    /// config. Falls back to the in-memory default when `cfg` is absent or
    /// its `active` kind is `Memory`.
    fn build_metrics_provider(
        cfg: Option<&MetricsBackendConfig>,
    ) -> RuntimeResult<Arc<dyn MetricsProvider>> {
        let Some(cfg) = cfg else {
            return Ok(Arc::new(create_local_metrics_backend()));
        };
        match cfg.active {
            MetricsBackendKind::Memory => Ok(Arc::new(create_local_metrics_backend())),
            MetricsBackendKind::File => {
                #[cfg(feature = "observability")]
                {
                    let settings = cfg.file.as_ref().ok_or_else(|| {
                        RuntimeError::StartFailed(
                            "[metrics_backend.file] path is required when active = \"file\"".into(),
                        )
                    })?;
                    Ok(Arc::new(swe_observ_metrics::create_file_metrics_backend(
                        settings.path.clone(),
                    )))
                }
                #[cfg(not(feature = "observability"))]
                {
                    Err(RuntimeError::StartFailed(
                        "metrics_backend = \"file\" requires the \"observability\" feature".into(),
                    ))
                }
            }
            MetricsBackendKind::Prometheus => {
                #[cfg(feature = "observability")]
                {
                    if cfg.prometheus.is_none() {
                        return Err(RuntimeError::StartFailed(
                            "[metrics_backend.prometheus] is required when active = \"prometheus\""
                                .into(),
                        ));
                    }
                    Ok(Arc::new(
                        swe_observ_metrics::create_prometheus_metrics_backend(),
                    ))
                }
                #[cfg(not(feature = "observability"))]
                {
                    Err(RuntimeError::StartFailed(
                        "metrics_backend = \"prometheus\" requires the \"observability\" feature"
                            .into(),
                    ))
                }
            }
            MetricsBackendKind::Otel => {
                #[cfg(feature = "observability")]
                {
                    let settings = cfg.otel.as_ref().ok_or_else(|| {
                        RuntimeError::StartFailed(
                            "[metrics_backend.otel] service_name is required when active = \"otel\""
                                .into(),
                        )
                    })?;
                    Ok(Arc::new(swe_observ_metrics::create_otel_metrics_backend(
                        &settings.service_name,
                    )))
                }
                #[cfg(not(feature = "observability"))]
                {
                    Err(RuntimeError::StartFailed(
                        "metrics_backend = \"otel\" requires the \"observability\" feature".into(),
                    ))
                }
            }
            MetricsBackendKind::Sqlite => Err(RuntimeError::StartFailed(
                "metrics_backend = \"sqlite\" is not supported yet — requires async pool \
                 initialisation this composition root doesn't perform. Use \
                 RuntimeBuilder::with_metrics_provider() to supply a pre-initialised \
                 backend instead."
                    .into(),
            )),
        }
    }

    /// Resolve the `TracerProvider` backend from `[tracer_backend]` TOML
    /// config. Falls back to the in-memory default when `cfg` is absent or
    /// its `active` kind is `Memory`.
    #[cfg(feature = "observability")]
    fn build_tracer_provider(
        cfg: Option<&TracerBackendConfig>,
    ) -> RuntimeResult<Arc<dyn TracerProvider>> {
        let Some(cfg) = cfg else {
            return Ok(swe_observ_tracing::create_default_tracer_arc());
        };
        match cfg.active {
            TracerBackendKind::Memory => Ok(swe_observ_tracing::create_default_tracer_arc()),
            TracerBackendKind::File => {
                let settings = cfg.file.as_ref().ok_or_else(|| {
                    RuntimeError::StartFailed(
                        "[tracer_backend.file] path is required when active = \"file\"".into(),
                    )
                })?;
                Ok(Arc::new(swe_observ_tracing::create_file_tracer(
                    settings.path.clone(),
                )))
            }
            TracerBackendKind::Jaeger => {
                let settings = cfg.jaeger.as_ref().ok_or_else(|| {
                    RuntimeError::StartFailed(
                        "[tracer_backend.jaeger] endpoint is required when active = \"jaeger\""
                            .into(),
                    )
                })?;
                Ok(Arc::new(swe_observ_tracing::create_jaeger_tracer(
                    &settings.service_name,
                    &settings.endpoint,
                )))
            }
            TracerBackendKind::Otel => {
                let settings = cfg.otel.as_ref().ok_or_else(|| {
                    RuntimeError::StartFailed(
                        "[tracer_backend.otel] service_name is required when active = \"otel\""
                            .into(),
                    )
                })?;
                Ok(Arc::new(swe_observ_tracing::create_otel_tracer(
                    &settings.service_name,
                )))
            }
            TracerBackendKind::Sqlite => Err(RuntimeError::StartFailed(
                "tracer_backend = \"sqlite\" is not supported yet — requires async pool \
                 initialisation this composition root doesn't perform. Use \
                 RuntimeBuilder::with_tracer_provider() to supply a pre-initialised \
                 backend instead."
                    .into(),
            )),
        }
    }

    /// Resolve the `LoggerProvider` backend from `[log_backend]` TOML
    /// config. Falls back to the in-memory default when `cfg` is absent or
    /// its `active` kind is `Memory`.
    #[cfg(feature = "observability")]
    fn build_log_backend(cfg: Option<&LogBackendConfig>) -> RuntimeResult<Arc<dyn LoggerProvider>> {
        let Some(cfg) = cfg else {
            return Ok(Arc::new(swe_observ_logging::create_local_logging_backend()));
        };
        match cfg.active {
            LogBackendKind::Memory => {
                Ok(Arc::new(swe_observ_logging::create_local_logging_backend()))
            }
            LogBackendKind::File => {
                let settings = cfg.file.as_ref().ok_or_else(|| {
                    RuntimeError::StartFailed(
                        "[log_backend.file] path is required when active = \"file\"".into(),
                    )
                })?;
                Ok(Arc::new(swe_observ_logging::create_file_logger(
                    settings.path.clone(),
                    settings.max_size_mb,
                )))
            }
            LogBackendKind::Elk => {
                let settings = cfg.elk.as_ref().ok_or_else(|| {
                    RuntimeError::StartFailed(
                        "[log_backend.elk] endpoint/index is required when active = \"elk\"".into(),
                    )
                })?;
                Ok(Arc::new(swe_observ_logging::create_elk_logger(
                    &settings.index,
                )))
            }
            LogBackendKind::Otel => {
                let settings = cfg.otel.as_ref().ok_or_else(|| {
                    RuntimeError::StartFailed(
                        "[log_backend.otel] service_name is required when active = \"otel\"".into(),
                    )
                })?;
                Ok(Arc::new(swe_observ_logging::create_otel_logger(
                    &settings.service_name,
                )))
            }
            LogBackendKind::Sqlite => Err(RuntimeError::StartFailed(
                "log_backend = \"sqlite\" is not supported yet — requires async pool \
                 initialisation this composition root doesn't perform. Use \
                 RuntimeBuilder::with_log_drain_backend() to supply a pre-initialised \
                 backend instead."
                    .into(),
            )),
        }
    }

    /// Resolve the `ObserverContext` a dispatcher's `Handler::execute()`
    /// calls receive. A builder-supplied override wins outright — not a
    /// per-primitive merge — over the composed tracer/log/metrics bridge.
    #[cfg(feature = "observability")]
    fn resolve_observer_context(
        override_ctx: Option<Arc<dyn edge_application_observer::ObserverContext>>,
        tracer_provider: &Arc<dyn TracerProvider>,
        log_drain_backend: &Arc<dyn LoggerProvider>,
        metrics_provider: &Option<Arc<dyn MetricsProvider>>,
    ) -> Arc<dyn edge_application_observer::ObserverContext> {
        match override_ctx {
            Some(o) => o,
            None => crate::core::observability::observer_context(
                Arc::clone(tracer_provider),
                Arc::clone(log_drain_backend),
                metrics_provider.clone(),
            ),
        }
    }
}

impl RuntimeBuilderServe {
    /// Wait for SIGTERM or SIGINT, whichever arrives first.
    async fn wait_for_signal() {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigterm = match signal(SignalKind::terminate()) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("could not register SIGTERM handler: {e}");
                    let _ = tokio::signal::ctrl_c().await;
                    return;
                }
            };
            tokio::select! {
                _ = sigterm.recv()           => {}
                _ = tokio::signal::ctrl_c() => {}
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Runtime;
    use swe_edge_bootstrap_runtime::RuntimeError;

    /// @covers: serve
    #[test]
    fn test_serve_returns_start_failed_when_no_handler_registered() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(Runtime::builder().grpc_allow_unauthenticated().serve());
        assert!(
            matches!(result, Err(RuntimeError::StartFailed(_))),
            "expected StartFailed, got: {result:?}",
        );
    }

    /// @covers: build_metrics_provider
    #[test]
    fn test_build_metrics_provider_defaults_to_memory_when_config_absent_happy() {
        let provider = RuntimeBuilder::build_metrics_provider(None).unwrap();
        provider.record_counter("test_counter", 3.0, &[]);
        let snap = provider.export();
        assert!(
            snap.iter()
                .any(|s| s.name == "test_counter" && s.value == 3.0),
            "expected the default in-memory backend to record real data, got: {snap:?}"
        );
    }

    /// @covers: build_metrics_provider
    #[cfg(feature = "observability")]
    #[test]
    fn test_build_metrics_provider_file_backend_writes_real_data_to_disk_happy() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("metrics.json");
        let cfg = MetricsBackendConfig {
            active: MetricsBackendKind::File,
            file: Some(swe_edge_bootstrap_runtime::MetricsFileSettings {
                path: path.to_string_lossy().to_string(),
            }),
            ..Default::default()
        };
        let provider = RuntimeBuilder::build_metrics_provider(Some(&cfg)).unwrap();
        provider.record_counter("selected_file_backend", 42.0, &[]);
        provider.flush();

        let contents = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!("expected the File backend to actually write {path:?}: {e}")
        });
        assert!(
            contents.contains("selected_file_backend") && contents.contains("42"),
            "expected real recorded data in the file, got: {contents}"
        );
    }

    /// @covers: build_tracer_provider
    #[cfg(feature = "observability")]
    #[test]
    fn test_build_tracer_provider_defaults_to_memory_when_config_absent_happy() {
        let provider = RuntimeBuilder::build_tracer_provider(None).unwrap();
        provider.export_span(&serde_json::json!({"handler_id": "test_span"}));
        assert_eq!(provider.recent_spans(10).len(), 1);
    }

    /// @covers: build_tracer_provider
    ///
    /// `swe-observability-tracing` doesn't publicly re-export its settings
    /// types (unlike metrics/logging), so the config has to be built via
    /// TOML deserialization here rather than a Rust struct literal — see the
    /// doc comment on `RuntimeConfig.tracer_backend`.
    #[cfg(feature = "observability")]
    #[test]
    fn test_build_tracer_provider_file_backend_writes_real_data_to_disk_happy() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("spans.jsonl");
        let toml_str = format!(
            "active = \"file\"\n[file]\npath = {:?}\n",
            path.to_string_lossy()
        );
        let cfg: TracerBackendConfig = toml::from_str(&toml_str).unwrap();
        let provider = RuntimeBuilder::build_tracer_provider(Some(&cfg)).unwrap();
        provider.export_span(&serde_json::json!({"handler_id": "selected_file_tracer_backend"}));
        provider.flush();

        let contents = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!("expected the File backend to actually write {path:?}: {e}")
        });
        assert!(
            contents.contains("selected_file_tracer_backend"),
            "expected real recorded data in the file, got: {contents}"
        );
    }

    /// @covers: build_tracer_provider
    #[cfg(feature = "observability")]
    #[test]
    fn test_build_tracer_provider_jaeger_missing_settings_returns_error_edge() {
        let cfg: TracerBackendConfig = toml::from_str("active = \"jaeger\"\n").unwrap();
        let result = RuntimeBuilder::build_tracer_provider(Some(&cfg));
        assert!(
            matches!(result, Err(RuntimeError::StartFailed(_))),
            "expected StartFailed when [tracer_backend.jaeger] is missing, got: {result:?}"
        );
    }

    /// @covers: build_tracer_provider
    #[cfg(feature = "observability")]
    #[test]
    fn test_build_tracer_provider_sqlite_returns_not_supported_error_edge() {
        let cfg: TracerBackendConfig = toml::from_str("active = \"sqlite\"\n").unwrap();
        let result = RuntimeBuilder::build_tracer_provider(Some(&cfg));
        assert!(
            matches!(result, Err(RuntimeError::StartFailed(_))),
            "expected StartFailed for the not-yet-supported sqlite backend, got: {result:?}"
        );
    }

    /// @covers: build_log_backend
    #[cfg(feature = "observability")]
    #[test]
    fn test_build_log_backend_defaults_to_memory_when_config_absent_happy() {
        let backend = RuntimeBuilder::build_log_backend(None).unwrap();
        backend.emit(&swe_observ_logging::LogEntry::new(
            swe_observ_logging::LogLevel::Info,
            "test",
            "hello",
        ));
        assert_eq!(backend.recent_entries(10).len(), 1);
    }

    /// @covers: build_log_backend
    #[cfg(feature = "observability")]
    #[test]
    fn test_build_log_backend_file_backend_writes_real_data_to_disk_happy() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("logs.jsonl");
        let cfg = LogBackendConfig {
            active: LogBackendKind::File,
            file: Some(swe_edge_bootstrap_runtime::LogFileSettings {
                path: path.to_string_lossy().to_string(),
                max_size_mb: 10,
                rotation: Default::default(),
            }),
            ..Default::default()
        };
        let backend = RuntimeBuilder::build_log_backend(Some(&cfg)).unwrap();
        backend.emit(&swe_observ_logging::LogEntry::new(
            swe_observ_logging::LogLevel::Error,
            "test",
            "selected_file_log_backend",
        ));
        backend.flush();

        let contents = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!("expected the File backend to actually write {path:?}: {e}")
        });
        assert!(
            contents.contains("selected_file_log_backend"),
            "expected real recorded data in the file, got: {contents}"
        );
    }

    /// @covers: build_log_backend
    #[cfg(feature = "observability")]
    #[test]
    fn test_build_log_backend_elk_missing_settings_returns_error_edge() {
        let cfg = LogBackendConfig {
            active: LogBackendKind::Elk,
            elk: None,
            ..Default::default()
        };
        let result = RuntimeBuilder::build_log_backend(Some(&cfg));
        assert!(
            matches!(result, Err(RuntimeError::StartFailed(_))),
            "expected StartFailed when [log_backend.elk] is missing, got: {result:?}"
        );
    }

    /// @covers: build_log_backend
    #[cfg(feature = "observability")]
    #[test]
    fn test_build_log_backend_sqlite_returns_not_supported_error_edge() {
        let cfg = LogBackendConfig {
            active: LogBackendKind::Sqlite,
            ..Default::default()
        };
        let result = RuntimeBuilder::build_log_backend(Some(&cfg));
        assert!(
            matches!(result, Err(RuntimeError::StartFailed(_))),
            "expected StartFailed for the not-yet-supported sqlite backend, got: {result:?}"
        );
    }

    /// @covers: resolve_observer_context
    #[cfg(feature = "observability")]
    #[test]
    fn test_resolve_observer_context_returns_the_exact_override_instance_happy() {
        let override_ctx: Arc<dyn edge_application_observer::ObserverContext> =
            edge_application_observer::StdObserveFactory::noop_arc_observe_context();
        let tracer_provider: Arc<dyn TracerProvider> =
            swe_observ_tracing::create_default_tracer_arc();
        let log_backend: Arc<dyn LoggerProvider> =
            Arc::new(swe_observ_logging::create_local_logging_backend());
        let resolved = RuntimeBuilder::resolve_observer_context(
            Some(Arc::clone(&override_ctx)),
            &tracer_provider,
            &log_backend,
            &None,
        );
        assert!(
            Arc::ptr_eq(&override_ctx, &resolved),
            "expected resolve_observer_context to return the exact override instance, not a \
             freshly-composed one"
        );
    }

    /// @covers: resolve_observer_context
    #[cfg(feature = "observability")]
    #[test]
    fn test_resolve_observer_context_uses_the_composed_bridge_when_no_override_supplied_edge() {
        let tracer_provider: Arc<dyn TracerProvider> =
            swe_observ_tracing::create_default_tracer_arc();
        let log_backend: Arc<dyn LoggerProvider> =
            Arc::new(swe_observ_logging::create_local_logging_backend());
        let resolved =
            RuntimeBuilder::resolve_observer_context(None, &tracer_provider, &log_backend, &None);

        // Prove it's genuinely wired to the *given* tracer_provider, not a
        // detached noop — open a span through the resolved context and
        // confirm it lands in the exact backend instance passed in.
        resolved
            .tracer(edge_application_observer::TracerRequest)
            .unwrap()
            .tracer
            .start_span(edge_application_observer::SpanStartRequest {
                handler_id: "test".to_string(),
                operation: "resolve_no_override".to_string(),
            })
            .unwrap()
            .span
            .finish(edge_application_observer::SpanFinishRequest)
            .unwrap();
        let recent = tracer_provider.recent_spans(10);
        assert!(
            recent
                .iter()
                .any(|s| s["operation"] == "resolve_no_override"),
            "expected the composed bridge to be wired to the given tracer_provider, got: {recent:?}"
        );
    }

    /// @covers: build_metrics_provider
    #[cfg(feature = "observability")]
    #[test]
    fn test_build_metrics_provider_file_backend_missing_settings_returns_error_edge() {
        let cfg = MetricsBackendConfig {
            active: MetricsBackendKind::File,
            file: None,
            ..Default::default()
        };
        let result = RuntimeBuilder::build_metrics_provider(Some(&cfg));
        assert!(
            matches!(result, Err(RuntimeError::StartFailed(_))),
            "expected StartFailed when [metrics_backend.file] is missing, got: {result:?}"
        );
    }

    /// @covers: build_metrics_provider
    #[cfg(feature = "observability")]
    #[test]
    fn test_build_metrics_provider_prometheus_missing_settings_returns_error_edge() {
        let cfg = MetricsBackendConfig {
            active: MetricsBackendKind::Prometheus,
            prometheus: None,
            ..Default::default()
        };
        let result = RuntimeBuilder::build_metrics_provider(Some(&cfg));
        assert!(
            matches!(result, Err(RuntimeError::StartFailed(_))),
            "expected StartFailed when [metrics_backend.prometheus] is missing, got: {result:?}"
        );
    }

    /// @covers: build_metrics_provider
    #[test]
    fn test_build_metrics_provider_sqlite_returns_not_supported_error_edge() {
        let cfg = MetricsBackendConfig {
            active: MetricsBackendKind::Sqlite,
            ..Default::default()
        };
        let result = RuntimeBuilder::build_metrics_provider(Some(&cfg));
        assert!(
            matches!(result, Err(RuntimeError::StartFailed(_))),
            "expected StartFailed for the not-yet-supported sqlite backend, got: {result:?}"
        );
    }
}
