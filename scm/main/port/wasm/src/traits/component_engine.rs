//! `ComponentEngine` trait — loads and invokes Wasm component handlers.

use futures::future::BoxFuture;

use crate::error::ComponentResult;
use crate::types::{
    ComponentInvokeRequest, ComponentInvokeResponse, ComponentLoadRequest, ComponentLoadResponse,
};

/// Loads validated Wasm components and invokes their handler export.
///
/// Implementations own instantiation, isolation, resource-limit
/// enforcement, and caching/pooling — none of that is expressed in this
/// trait, matching every other port in this crate (the contract is the
/// behavior, not the mechanism). `load` must reject bytes that do not
/// match `manifest.contract_version`'s declared import/export surface, per
/// ADR-006's malformed/incompatible-artifact acceptance criteria — but
/// callers should still run [`crate::ComponentValidator::validate`] first
/// so that class of rejection happens before registration, not at load
/// time.
pub trait ComponentEngine: Send + Sync {
    /// Load a component artifact, returning a handle for subsequent
    /// [`ComponentEngine::invoke`] calls.
    fn load(
        &self,
        req: ComponentLoadRequest,
    ) -> BoxFuture<'_, ComponentResult<ComponentLoadResponse>>;

    /// Invoke a previously loaded component's handler export once.
    fn invoke(
        &self,
        req: ComponentInvokeRequest,
    ) -> BoxFuture<'_, ComponentResult<ComponentInvokeResponse>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ComponentError;
    use crate::types::{ArtifactProvenance, ComponentHandle, ComponentManifest, ResourceLimits};
    use std::sync::Arc;
    use std::time::Duration;

    // Deliberately not `wasmtime`-backed — proves `ComponentEngine` is
    // object-safe and satisfiable using *only* this crate's own dependency
    // graph, matching `RuntimeManager`'s equivalent standalone-double test
    // in `swe-edge-bootstrap-runtime`.
    struct EchoEngineDouble;

    impl ComponentEngine for EchoEngineDouble {
        fn load(
            &self,
            req: ComponentLoadRequest,
        ) -> BoxFuture<'_, ComponentResult<ComponentLoadResponse>> {
            Box::pin(async move {
                Ok(ComponentLoadResponse {
                    handle: ComponentHandle(req.manifest.component_id),
                })
            })
        }

        fn invoke(
            &self,
            req: ComponentInvokeRequest,
        ) -> BoxFuture<'_, ComponentResult<ComponentInvokeResponse>> {
            Box::pin(async move {
                if req.payload.is_empty() {
                    return Err(ComponentError::InvalidOutput(
                        "echo double refuses empty payloads".to_string(),
                    ));
                }
                Ok(ComponentInvokeResponse {
                    payload: req.payload,
                })
            })
        }
    }

    fn sample_manifest() -> ComponentManifest {
        ComponentManifest {
            component_id: "echo-handler".to_string(),
            route_id: "/echo".to_string(),
            contract_version: "swe:edge-handler@0.1.0".to_string(),
            resource_limits: ResourceLimits {
                max_memory_bytes: 16 * 1024 * 1024,
                invoke_timeout_ms: 5_000,
                max_concurrency: 8,
                max_payload_bytes: 256 * 1024,
            },
            capabilities: vec![],
            artifact_provenance: ArtifactProvenance {
                source: "ci-run-12345".to_string(),
                checksum_sha256: "a".repeat(64),
                built_at: "2026-08-08T00:00:00Z".to_string(),
            },
        }
    }

    #[tokio::test]
    async fn test_component_engine_double_is_object_safe_as_dyn_and_runs_standalone() {
        let engine: Arc<dyn ComponentEngine> = Arc::new(EchoEngineDouble);
        let loaded = match engine
            .load(ComponentLoadRequest {
                manifest: sample_manifest(),
                component_bytes: vec![0, 1, 2, 3],
            })
            .await
        {
            Ok(v) => v,
            Err(e) => panic!("load must succeed: {e}"),
        };
        assert_eq!(loaded.handle, ComponentHandle("echo-handler".to_string()));

        let invoked = match engine
            .invoke(ComponentInvokeRequest {
                handle: loaded.handle,
                payload: b"hello".to_vec(),
                deadline: Duration::from_secs(1),
            })
            .await
        {
            Ok(v) => v,
            Err(e) => panic!("invoke must succeed: {e}"),
        };
        assert_eq!(
            invoked.payload,
            b"hello".to_vec(),
            "the double must echo back exactly what it received"
        );
    }

    #[tokio::test]
    async fn test_component_engine_invoke_empty_payload_returns_error() {
        let engine: Arc<dyn ComponentEngine> = Arc::new(EchoEngineDouble);
        let result = engine
            .invoke(ComponentInvokeRequest {
                handle: ComponentHandle("echo-handler".to_string()),
                payload: vec![],
                deadline: Duration::from_secs(1),
            })
            .await;
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("invoke with an empty payload must fail, not silently succeed"),
        };
        assert!(matches!(err, ComponentError::InvalidOutput(_)));
    }
}
