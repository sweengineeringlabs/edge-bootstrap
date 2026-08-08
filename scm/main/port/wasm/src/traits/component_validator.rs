//! `ComponentValidator` trait — gates a component out of registration
//! before it ever reaches a [`crate::ComponentEngine`].

use crate::error::ComponentResult;
use crate::types::ValidateComponentRequest;

/// Validates a component's manifest and declared imports/exports against
/// the versioned `swe:edge-handler` WIT contract, before its route is ever
/// registered.
///
/// Synchronous by design: manifest/import-export-surface validation is
/// parsing and comparison against already-known data, not I/O — keeping it
/// synchronous also keeps it trivially testable without an async runtime,
/// matching ADR-007's decision driver that this contract stay testable
/// independent of any concrete engine.
pub trait ComponentValidator: Send + Sync {
    /// Validate a component, returning `Ok(())` if it may proceed to
    /// [`crate::ComponentEngine::load`], or a [`crate::ComponentError`]
    /// variant explaining why registration must be refused.
    fn validate(&self, req: ValidateComponentRequest) -> ComponentResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ComponentError;
    use crate::types::{ArtifactProvenance, ComponentManifest, ResourceLimits};

    const SUPPORTED_CONTRACT_VERSION: &str = "swe:edge-handler@0.1.0";

    // Deliberately not `wasmtime`-backed — proves `ComponentValidator` is
    // object-safe and satisfiable using *only* this crate's own dependency
    // graph. Checks exactly the two things ADR-007 calls out by name:
    // contract-version compatibility and non-empty artifact bytes.
    struct ContractVersionValidatorDouble;

    impl ComponentValidator for ContractVersionValidatorDouble {
        fn validate(&self, req: ValidateComponentRequest) -> ComponentResult<()> {
            if req.manifest.contract_version != SUPPORTED_CONTRACT_VERSION {
                return Err(ComponentError::UnsupportedContractVersion(
                    req.manifest.contract_version,
                ));
            }
            if req.component_bytes.is_empty() {
                return Err(ComponentError::InvalidManifest(
                    "component_bytes must not be empty".to_string(),
                ));
            }
            Ok(())
        }
    }

    fn sample_manifest(contract_version: &str) -> ComponentManifest {
        ComponentManifest {
            component_id: "echo-handler".to_string(),
            route_id: "/echo".to_string(),
            contract_version: contract_version.to_string(),
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

    #[test]
    fn test_component_validator_double_is_object_safe_and_accepts_matching_contract() {
        let validator: Box<dyn ComponentValidator> = Box::new(ContractVersionValidatorDouble);
        let result = validator.validate(ValidateComponentRequest {
            manifest: sample_manifest(SUPPORTED_CONTRACT_VERSION),
            component_bytes: vec![0, 1, 2, 3],
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_component_validator_rejects_unsupported_contract_version_error() {
        let validator: Box<dyn ComponentValidator> = Box::new(ContractVersionValidatorDouble);
        let result = validator.validate(ValidateComponentRequest {
            manifest: sample_manifest("swe:edge-handler@99.0.0"),
            component_bytes: vec![0, 1, 2, 3],
        });
        let err = match result {
            Err(e) => e,
            Ok(()) => {
                panic!("an unsupported contract version must be rejected, not silently accepted")
            }
        };
        assert_eq!(
            err,
            ComponentError::UnsupportedContractVersion("swe:edge-handler@99.0.0".to_string())
        );
    }

    #[test]
    fn test_component_validator_rejects_empty_component_bytes_error() {
        let validator: Box<dyn ComponentValidator> = Box::new(ContractVersionValidatorDouble);
        let result = validator.validate(ValidateComponentRequest {
            manifest: sample_manifest(SUPPORTED_CONTRACT_VERSION),
            component_bytes: vec![],
        });
        let err = match result {
            Err(e) => e,
            Ok(()) => {
                panic!("empty component bytes must be rejected before it ever reaches an engine")
            }
        };
        assert!(matches!(err, ComponentError::InvalidManifest(_)));
    }
}
