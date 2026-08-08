//! `ComponentManifest` — the metadata a `ComponentValidator` checks before
//! a Wasm component's route is ever registered, per ADR-007's Considered
//! Options (identity, route intent, resource limits, capabilities,
//! contract version, artifact provenance).

use serde::{Deserialize, Serialize};

use crate::types::artifact_provenance::ArtifactProvenance;
use crate::types::resource_limits::ResourceLimits;

/// Declares what a Wasm component handler is, what route it wants, what it
/// is allowed to do, and where it came from.
///
/// Validated by [`crate::ComponentValidator::validate`] before
/// [`crate::ComponentEngine::load`] is ever called for the corresponding
/// artifact — a component whose manifest fails validation must never reach
/// the engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentManifest {
    /// Stable identifier for this component (independent of route or
    /// version — used for caching/pooling keys).
    pub component_id: String,
    /// The route this component wants to serve, in the same
    /// leading-`/`-prefixed shape `edge_application_handler::IdResponse`
    /// uses (e.g. `"/echo"`).
    pub route_id: String,
    /// The `swe:edge-handler` WIT contract version this component was
    /// compiled against (e.g. `"swe:edge-handler@0.1.0"`).
    pub contract_version: String,
    /// Resource bounds the engine must enforce for this component.
    pub resource_limits: ResourceLimits,
    /// Host capabilities this component declares it needs (e.g.
    /// `"http-egress"`, `"clock"`). Anything not listed here must be
    /// denied at invocation time, per ADR-006's capability-denial-by-default
    /// posture.
    pub capabilities: Vec<String>,
    /// Where this artifact came from and how to verify it.
    pub artifact_provenance: ArtifactProvenance,
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn round_trip(manifest: &ComponentManifest) -> ComponentManifest {
        let json = match serde_json::to_string(manifest) {
            Ok(json) => json,
            Err(e) => panic!("serialize must succeed: {e}"),
        };
        match serde_json::from_str(&json) {
            Ok(v) => v,
            Err(e) => panic!("deserialize must succeed: {e}"),
        }
    }

    #[test]
    fn test_component_manifest_serde_round_trip_preserves_nested_fields() {
        let manifest = sample_manifest();
        let round_tripped = round_trip(&manifest);
        assert_eq!(round_tripped, manifest);
    }

    #[test]
    fn test_component_manifest_route_id_survives_round_trip_with_leading_slash() {
        let manifest = sample_manifest();
        let round_tripped = round_trip(&manifest);
        assert!(
            round_tripped.route_id.starts_with('/'),
            "route_id must keep its leading '/' — DefaultGrpcJob/DefaultHttpJob key routes on it directly"
        );
    }
}
