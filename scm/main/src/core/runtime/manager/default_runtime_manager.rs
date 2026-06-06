use std::sync::Arc;
use std::time::Instant;

use futures::future::BoxFuture;
use parking_lot::Mutex;

use edge_proxy::{HealthStatus, LifecycleMonitor};

use crate::api::egress::Egress;
use crate::api::ingress::Ingress;
use crate::api::runtime::traits::runtime_manager::RuntimeManager;
use crate::api::runtime::types::health::ComponentHealth;
use crate::api::runtime::{RuntimeConfig, RuntimeHealth, RuntimeStatus};
use crate::api::runtime::{RuntimeError, RuntimeResult};

pub(crate) struct DefaultRuntimeManager {
    config: RuntimeConfig,
    ingress: Arc<dyn Ingress>,
    egress: Arc<dyn Egress>,
    lifecycle: Arc<dyn LifecycleMonitor>,
    status: Arc<Mutex<RuntimeStatus>>,
    started_at: Arc<Mutex<Option<Instant>>>,
    #[cfg(feature = "message-broker")]
    message_broker: Option<Arc<dyn swe_edge_runtime_message_broker::MessageBroker>>,
}

impl DefaultRuntimeManager {
    pub(crate) fn new(
        config: RuntimeConfig,
        ingress: Arc<dyn Ingress>,
        egress: Arc<dyn Egress>,
        lifecycle: Arc<dyn LifecycleMonitor>,
    ) -> Self {
        Self {
            config,
            ingress,
            egress,
            lifecycle,
            status: Arc::new(Mutex::new(RuntimeStatus::Stopped)),
            started_at: Arc::new(Mutex::new(None)),
            #[cfg(feature = "message-broker")]
            message_broker: None,
        }
    }

    /// Attach a message broker for health monitoring and lifecycle probing.
    #[cfg(feature = "message-broker")]
    pub(crate) fn with_message_broker(
        mut self,
        broker: Arc<dyn swe_edge_runtime_message_broker::MessageBroker>,
    ) -> Self {
        self.message_broker = Some(broker);
        self
    }
}

/// Fluent builder for [`DefaultRuntimeManager`].
struct DefaultRuntimeManagerBuilder {
    config: Option<RuntimeConfig>,
    ingress: Option<Arc<dyn Ingress>>,
    egress: Option<Arc<dyn Egress>>,
    lifecycle: Option<Arc<dyn edge_proxy::LifecycleMonitor>>,
}

impl DefaultRuntimeManagerBuilder {
    fn new() -> Self {
        Self {
            config: None,
            ingress: None,
            egress: None,
            lifecycle: None,
        }
    }
    fn config(mut self, c: RuntimeConfig) -> Self {
        self.config = Some(c);
        self
    }
    fn ingress(mut self, i: Arc<dyn Ingress>) -> Self {
        self.ingress = Some(i);
        self
    }
    fn egress(mut self, e: Arc<dyn Egress>) -> Self {
        self.egress = Some(e);
        self
    }
    fn lifecycle(mut self, l: Arc<dyn edge_proxy::LifecycleMonitor>) -> Self {
        self.lifecycle = Some(l);
        self
    }
    fn build(self) -> DefaultRuntimeManager {
        DefaultRuntimeManager::new(
            self.config.expect("config required"),
            self.ingress.expect("ingress required"),
            self.egress.expect("egress required"),
            self.lifecycle.expect("lifecycle required"),
        )
    }
}

impl RuntimeManager for DefaultRuntimeManager {
    fn start(&self) -> BoxFuture<'_, RuntimeResult<()>> {
        Box::pin(async move {
            {
                let mut s = self.status.lock();
                if *s == RuntimeStatus::Running {
                    return Ok(());
                }
                *s = RuntimeStatus::Starting;
            }

            if !self.ingress.has_any() {
                return Err(RuntimeError::StartFailed(
                    "no ingress transport configured — add http or grpc".into(),
                ));
            }

            self.lifecycle.start_background_tasks().await;

            // Probe each configured ingress transport to surface misconfigurations early.
            if let Some(h) = self.ingress.http() {
                let _ = h.health_check().await;
            }
            if let Some(g) = self.ingress.grpc() {
                let _ = g.health_check().await;
            }

            // Probe egress.
            let _ = self.egress.http().health_check().await;
            if let Some(g) = self.egress.grpc() {
                let _ = g.health_check().await;
            }

            // Probe message broker if configured.
            #[cfg(feature = "message-broker")]
            if let Some(broker) = &self.message_broker {
                let _ = broker.health_check().await;
            }

            {
                let mut s = self.status.lock();
                *s = RuntimeStatus::Running;
                *self.started_at.lock() = Some(Instant::now());
            }

            if self.config.systemd_notify {
                tracing::info!("READY=1");
            }

            tracing::info!(
                service = %self.config.service_name,
                http    = %self.config.http_bind,
                grpc    = %self.config.grpc_bind,
                "runtime started"
            );

            Ok(())
        })
    }

    fn shutdown(&self) -> BoxFuture<'_, RuntimeResult<()>> {
        Box::pin(async move {
            {
                let mut s = self.status.lock();
                if *s == RuntimeStatus::Stopped {
                    return Ok(());
                }
                *s = RuntimeStatus::Stopping;
            }

            if self.config.systemd_notify {
                tracing::info!("STOPPING=1");
            }

            self.lifecycle
                .shutdown()
                .await
                .map_err(|e| RuntimeError::ShutdownFailed(e.to_string()))?;

            *self.status.lock() = RuntimeStatus::Stopped;

            tracing::info!(service = %self.config.service_name, "runtime stopped");

            Ok(())
        })
    }

    fn health(&self) -> BoxFuture<'_, RuntimeHealth> {
        Box::pin(async move {
            let status = *self.status.lock();
            let report = self.lifecycle.health().await;

            let uptime_secs = self
                .started_at
                .lock()
                .map(|t| t.elapsed().as_secs())
                .unwrap_or(0);

            let mut components: Vec<ComponentHealth> = report
                .components
                .iter()
                .map(|c| match c.status {
                    HealthStatus::Healthy => ComponentHealth::healthy(&c.id),
                    _ => ComponentHealth::unhealthy(
                        &c.id,
                        c.message.as_deref().unwrap_or("degraded"),
                    ),
                })
                .collect();

            // Report health for each configured ingress transport.
            if let Some(h) = self.ingress.http() {
                match h.health_check().await {
                    Ok(_) => components.push(ComponentHealth::healthy("ingress.http")),
                    Err(e) => {
                        components.push(ComponentHealth::unhealthy("ingress.http", e.to_string()))
                    }
                }
            }
            if let Some(g) = self.ingress.grpc() {
                match g.health_check().await {
                    Ok(_) => components.push(ComponentHealth::healthy("ingress.grpc")),
                    Err(e) => {
                        components.push(ComponentHealth::unhealthy("ingress.grpc", e.to_string()))
                    }
                }
            }
            // Report egress transport health.
            match self.egress.http().health_check().await {
                Ok(_) => components.push(ComponentHealth::healthy("egress.http")),
                Err(e) => components.push(ComponentHealth::unhealthy("egress.http", e.to_string())),
            }
            if let Some(g) = self.egress.grpc() {
                match g.health_check().await {
                    Ok(_) => components.push(ComponentHealth::healthy("egress.grpc")),
                    Err(e) => {
                        components.push(ComponentHealth::unhealthy("egress.grpc", e.to_string()))
                    }
                }
            }

            // Report message broker health if configured.
            #[cfg(feature = "message-broker")]
            if let Some(broker) = &self.message_broker {
                match broker.health_check().await {
                    Ok(_) => components.push(ComponentHealth::healthy("message-broker")),
                    Err(e) => {
                        components.push(ComponentHealth::unhealthy("message-broker", e.to_string()))
                    }
                }
            }

            RuntimeHealth {
                status,
                components,
                uptime_secs,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::egress::DefaultEgress;
    use crate::core::ingress::DefaultIngress;
    use edge_proxy::{HealthReport, LifecycleError};
    use futures::future::BoxFuture;
    use futures::FutureExt;
    use std::collections::HashMap;
    use swe_edge_egress_grpc::{
        GrpcEgress, GrpcEgressError, GrpcEgressResult, GrpcMetadata as EgressGrpcMetadata,
        GrpcRequest as EgressGrpcRequest, GrpcResponse as EgressGrpcResponse,
    };
    use swe_edge_egress_http::{
        HttpEgressResult, HttpRequest as EgressReq, HttpResponse as EgressResp, HttpStreamResponse,
    };
    use swe_edge_ingress_grpc::{
        GrpcHealthCheck, GrpcIngress, GrpcIngressResult, GrpcMetadata, GrpcRequest, GrpcResponse,
    };
    use swe_edge_ingress_http::{
        HttpHealthCheck, HttpIngressResult, HttpRequest, HttpResponse, RequestContext,
    };

    struct DefaultRuntimeManagerStubLifecycle;

    impl LifecycleMonitor for DefaultRuntimeManagerStubLifecycle {
        fn health(&self) -> BoxFuture<'_, HealthReport> {
            async move { HealthReport::from_components(vec![]) }.boxed()
        }
        fn start_background_tasks(&self) -> BoxFuture<'_, ()> {
            async move {}.boxed()
        }
        fn shutdown(&self) -> BoxFuture<'_, Result<(), LifecycleError>> {
            async move { Ok(()) }.boxed()
        }
    }

    struct DefaultRuntimeManagerStubHttp;
    impl swe_edge_ingress_http::HttpIngress for DefaultRuntimeManagerStubHttp {
        fn handle(
            &self,
            _: HttpRequest,
            _ctx: RequestContext,
        ) -> BoxFuture<'_, HttpIngressResult<HttpResponse>> {
            Box::pin(async { Ok(HttpResponse::new(200, vec![])) })
        }
        fn health_check(&self) -> BoxFuture<'_, HttpIngressResult<HttpHealthCheck>> {
            Box::pin(async { Ok(HttpHealthCheck::healthy()) })
        }
    }

    struct DefaultRuntimeManagerStubGrpc;
    impl GrpcIngress for DefaultRuntimeManagerStubGrpc {
        fn handle_unary(
            &self,
            _: GrpcRequest,
            _ctx: RequestContext,
        ) -> BoxFuture<'_, GrpcIngressResult<GrpcResponse>> {
            Box::pin(async {
                Ok(GrpcResponse {
                    body: vec![],
                    metadata: GrpcMetadata {
                        headers: HashMap::new(),
                    },
                })
            })
        }
        fn health_check(&self) -> BoxFuture<'_, GrpcIngressResult<GrpcHealthCheck>> {
            Box::pin(async {
                Ok(GrpcHealthCheck {
                    healthy: true,
                    message: None,
                })
            })
        }
    }

    struct DefaultRuntimeManagerStubHttpOut;
    impl swe_edge_egress_http::HttpEgress for DefaultRuntimeManagerStubHttpOut {
        fn send(&self, _: EgressReq) -> BoxFuture<'_, HttpEgressResult<EgressResp>> {
            Box::pin(async { Ok(EgressResp::new(200, vec![])) })
        }
        fn send_stream(&self, _: EgressReq) -> BoxFuture<'_, HttpEgressResult<HttpStreamResponse>> {
            Box::pin(async {
                Ok(HttpStreamResponse {
                    status: 200,
                    headers: Default::default(),
                    body: Box::pin(futures::stream::empty()),
                })
            })
        }
        fn health_check(&self) -> BoxFuture<'_, HttpEgressResult<()>> {
            Box::pin(async { Ok(()) })
        }
    }

    struct DefaultRuntimeManagerStubGrpcOut;
    impl GrpcEgress for DefaultRuntimeManagerStubGrpcOut {
        fn call_unary(
            &self,
            _: EgressGrpcRequest,
        ) -> BoxFuture<'_, GrpcEgressResult<EgressGrpcResponse>> {
            Box::pin(async {
                Ok(EgressGrpcResponse {
                    body: vec![],
                    metadata: EgressGrpcMetadata::default(),
                })
            })
        }
        fn health_check(&self) -> BoxFuture<'_, GrpcEgressResult<()>> {
            Box::pin(async { Ok(()) })
        }
    }

    struct DefaultRuntimeManagerDownGrpcOut;
    impl GrpcEgress for DefaultRuntimeManagerDownGrpcOut {
        fn call_unary(
            &self,
            _: EgressGrpcRequest,
        ) -> BoxFuture<'_, GrpcEgressResult<EgressGrpcResponse>> {
            Box::pin(async { Err(GrpcEgressError::Unavailable("down".into())) })
        }
        fn health_check(&self) -> BoxFuture<'_, GrpcEgressResult<()>> {
            Box::pin(async { Err(GrpcEgressError::Unavailable("unreachable".into())) })
        }
    }

    fn make_manager() -> DefaultRuntimeManager {
        DefaultRuntimeManager::new(
            RuntimeConfig::default().with_systemd_notify(false),
            Arc::new(DefaultIngress::new_http(Arc::new(
                DefaultRuntimeManagerStubHttp,
            ))),
            Arc::new(DefaultEgress::new_http(Arc::new(
                DefaultRuntimeManagerStubHttpOut,
            ))),
            Arc::new(DefaultRuntimeManagerStubLifecycle),
        )
    }

    #[test]
    fn test_new_creates_stopped_status() {
        let m = make_manager();
        assert_eq!(*m.status.lock(), RuntimeStatus::Stopped);
        assert!(m.started_at.lock().is_none());
    }

    #[tokio::test]
    async fn test_start_transitions_status_to_running() {
        let m = make_manager();
        m.start().await.expect("start ok");
        assert_eq!(*m.status.lock(), RuntimeStatus::Running);
    }

    #[tokio::test]
    async fn test_start_is_idempotent() {
        let m = make_manager();
        m.start().await.expect("first start ok");
        m.start().await.expect("second start ok");
        assert_eq!(*m.status.lock(), RuntimeStatus::Running);
    }

    #[tokio::test]
    async fn test_shutdown_transitions_status_to_stopped() {
        let m = make_manager();
        m.start().await.expect("start ok");
        m.shutdown().await.expect("shutdown ok");
        assert_eq!(*m.status.lock(), RuntimeStatus::Stopped);
    }

    #[tokio::test]
    async fn test_shutdown_is_idempotent() {
        let m = make_manager();
        m.shutdown().await.expect("first");
        m.shutdown().await.expect("second");
    }

    #[tokio::test]
    async fn test_start_fails_when_no_ingress_configured() {
        let m = DefaultRuntimeManager::new(
            RuntimeConfig::default(),
            Arc::new(DefaultIngress::empty()),
            Arc::new(DefaultEgress::new_http(Arc::new(
                DefaultRuntimeManagerStubHttpOut,
            ))),
            Arc::new(DefaultRuntimeManagerStubLifecycle),
        );
        let err = m.start().await.unwrap_err();
        assert!(matches!(err, RuntimeError::StartFailed(_)));
        assert!(err.to_string().contains("no ingress transport"));
    }

    #[tokio::test]
    async fn test_health_includes_ingress_http_and_egress_components() {
        let m = make_manager();
        m.start().await.expect("start ok");
        let h = m.health().await;
        let names: Vec<&str> = h.components.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"ingress.http"));
        assert!(names.contains(&"egress.http"));
    }

    #[tokio::test]
    async fn test_health_reports_grpc_when_only_grpc_configured() {
        let m = DefaultRuntimeManager::new(
            RuntimeConfig::default(),
            Arc::new(DefaultIngress::new_grpc(Arc::new(
                DefaultRuntimeManagerStubGrpc,
            ))),
            Arc::new(DefaultEgress::new_http(Arc::new(
                DefaultRuntimeManagerStubHttpOut,
            ))),
            Arc::new(DefaultRuntimeManagerStubLifecycle),
        );
        m.start().await.expect("start ok");
        let h = m.health().await;
        let names: Vec<&str> = h.components.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"ingress.grpc"));
        assert!(!names.contains(&"ingress.http"));
    }

    #[tokio::test]
    async fn test_health_reports_http_and_grpc_when_both_configured() {
        let m = DefaultRuntimeManager::new(
            RuntimeConfig::default(),
            Arc::new(
                DefaultIngress::new_http(Arc::new(DefaultRuntimeManagerStubHttp))
                    .with_grpc(Arc::new(DefaultRuntimeManagerStubGrpc)),
            ),
            Arc::new(DefaultEgress::new_http(Arc::new(
                DefaultRuntimeManagerStubHttpOut,
            ))),
            Arc::new(DefaultRuntimeManagerStubLifecycle),
        );
        m.start().await.expect("start ok");
        let h = m.health().await;
        let names: Vec<&str> = h.components.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"ingress.http"));
        assert!(names.contains(&"ingress.grpc"));
    }

    #[tokio::test]
    async fn test_health_reports_egress_grpc_when_configured() {
        let m = DefaultRuntimeManager::new(
            RuntimeConfig::default(),
            Arc::new(DefaultIngress::new_http(Arc::new(
                DefaultRuntimeManagerStubHttp,
            ))),
            Arc::new(
                DefaultEgress::new_http(Arc::new(DefaultRuntimeManagerStubHttpOut))
                    .with_grpc(Arc::new(DefaultRuntimeManagerStubGrpcOut)),
            ),
            Arc::new(DefaultRuntimeManagerStubLifecycle),
        );
        m.start().await.expect("start ok");
        let h = m.health().await;
        let names: Vec<&str> = h.components.iter().map(|c| c.name.as_str()).collect();
        assert!(
            names.contains(&"egress.grpc"),
            "egress.grpc must appear in health report"
        );
        assert!(names.contains(&"egress.http"));
    }

    #[tokio::test]
    async fn test_health_reports_egress_grpc_unhealthy_when_down() {
        let m = DefaultRuntimeManager::new(
            RuntimeConfig::default(),
            Arc::new(DefaultIngress::new_http(Arc::new(
                DefaultRuntimeManagerStubHttp,
            ))),
            Arc::new(
                DefaultEgress::new_http(Arc::new(DefaultRuntimeManagerStubHttpOut))
                    .with_grpc(Arc::new(DefaultRuntimeManagerDownGrpcOut)),
            ),
            Arc::new(DefaultRuntimeManagerStubLifecycle),
        );
        m.start().await.expect("start ok");
        let h = m.health().await;
        let grpc_comp = h
            .components
            .iter()
            .find(|c| c.name == "egress.grpc")
            .expect("egress.grpc component must be present");
        assert!(
            !grpc_comp.healthy,
            "egress.grpc must be unhealthy when health_check returns Err"
        );
    }

    #[cfg(feature = "message-broker")]
    #[test]
    fn test_with_message_broker_sets_broker_field() {
        use swe_edge_runtime_message_broker::{BrokerError, Message, MessageBroker, MessageStream};
        struct DefaultRuntimeManagerStubBroker;
        impl MessageBroker for DefaultRuntimeManagerStubBroker {
            fn publish<'a>(
                &'a self,
                _: &'a str,
                _: Message,
            ) -> futures::future::BoxFuture<'a, Result<(), BrokerError>> {
                Box::pin(futures::future::ready(Ok(())))
            }
            fn subscribe<'a>(
                &'a self,
                _: &'a str,
            ) -> futures::future::BoxFuture<'a, Result<MessageStream, BrokerError>> {
                Box::pin(futures::future::ready(Ok(
                    Box::pin(futures::stream::empty()) as MessageStream,
                )))
            }
            fn health_check(&self) -> futures::future::BoxFuture<'_, Result<(), BrokerError>> {
                Box::pin(futures::future::ready(Ok(())))
            }
        }
        let m = make_manager().with_message_broker(Arc::new(DefaultRuntimeManagerStubBroker));
        assert!(m.message_broker.is_some());
    }

    #[cfg(feature = "message-broker")]
    #[tokio::test]
    async fn test_with_message_broker_appears_in_health_report() {
        use swe_edge_runtime_message_broker::{BrokerError, Message, MessageBroker, MessageStream};
        struct DefaultRuntimeManagerHealthyBroker;
        impl MessageBroker for DefaultRuntimeManagerHealthyBroker {
            fn publish<'a>(
                &'a self,
                _: &'a str,
                _: Message,
            ) -> futures::future::BoxFuture<'a, Result<(), BrokerError>> {
                Box::pin(futures::future::ready(Ok(())))
            }
            fn subscribe<'a>(
                &'a self,
                _: &'a str,
            ) -> futures::future::BoxFuture<'a, Result<MessageStream, BrokerError>> {
                Box::pin(futures::future::ready(Ok(
                    Box::pin(futures::stream::empty()) as MessageStream,
                )))
            }
            fn health_check(&self) -> futures::future::BoxFuture<'_, Result<(), BrokerError>> {
                Box::pin(futures::future::ready(Ok(())))
            }
        }
        let m = make_manager().with_message_broker(Arc::new(DefaultRuntimeManagerHealthyBroker));
        m.start().await.expect("start ok");
        let h = m.health().await;
        let names: Vec<&str> = h.components.iter().map(|c| c.name.as_str()).collect();
        assert!(
            names.contains(&"message-broker"),
            "message-broker must appear in health report"
        );
    }

    #[cfg(feature = "message-broker")]
    #[tokio::test]
    async fn test_with_message_broker_unhealthy_reports_component_as_unhealthy() {
        use swe_edge_runtime_message_broker::{BrokerError, Message, MessageBroker, MessageStream};
        struct DefaultRuntimeManagerDownBroker;
        impl MessageBroker for DefaultRuntimeManagerDownBroker {
            fn publish<'a>(
                &'a self,
                _: &'a str,
                _: Message,
            ) -> futures::future::BoxFuture<'a, Result<(), BrokerError>> {
                Box::pin(futures::future::ready(Err(BrokerError::Unavailable(
                    "down".into(),
                ))))
            }
            fn subscribe<'a>(
                &'a self,
                _: &'a str,
            ) -> futures::future::BoxFuture<'a, Result<MessageStream, BrokerError>> {
                Box::pin(futures::future::ready(Err(BrokerError::Unavailable(
                    "down".into(),
                ))))
            }
            fn health_check(&self) -> futures::future::BoxFuture<'_, Result<(), BrokerError>> {
                Box::pin(futures::future::ready(Err(BrokerError::Unavailable(
                    "unreachable".into(),
                ))))
            }
        }
        let m = make_manager().with_message_broker(Arc::new(DefaultRuntimeManagerDownBroker));
        m.start().await.expect("start ok");
        let h = m.health().await;
        let comp = h
            .components
            .iter()
            .find(|c| c.name == "message-broker")
            .expect("message-broker component must be present");
        assert!(
            !comp.healthy,
            "message-broker must be unhealthy when health_check returns Err"
        );
    }

    #[test]
    fn test_default_runtime_manager_builder_creates_stopped_manager() {
        let m = DefaultRuntimeManagerBuilder::new()
            .config(RuntimeConfig::default())
            .ingress(Arc::new(DefaultIngress::new_http(Arc::new(
                DefaultRuntimeManagerStubHttp,
            ))))
            .egress(Arc::new(DefaultEgress::new_http(Arc::new(
                DefaultRuntimeManagerStubHttpOut,
            ))))
            .lifecycle(Arc::new(DefaultRuntimeManagerStubLifecycle))
            .build();
        assert_eq!(*m.status.lock(), RuntimeStatus::Stopped);
    }
}
