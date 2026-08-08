//! `ArtifactProvenance` — identifies where a component artifact came from
//! and whether it can be trusted, per ADR-007's manifest field list
//! ("artifact provenance").

use serde::{Deserialize, Serialize};

/// Provenance metadata a `ComponentValidator` checks before a component is
/// trusted enough to load.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactProvenance {
    /// Identifier of the build pipeline or process that produced this
    /// artifact (e.g. a CI run URL or build system identifier).
    pub source: String,
    /// SHA-256 checksum of the component binary, hex-encoded.
    pub checksum_sha256: String,
    /// RFC 3339 timestamp of when the artifact was built.
    pub built_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_artifact_provenance_serde_round_trip_preserves_all_fields() {
        let provenance = ArtifactProvenance {
            source: "ci-run-12345".to_string(),
            checksum_sha256: "a".repeat(64),
            built_at: "2026-08-08T00:00:00Z".to_string(),
        };
        let json = match serde_json::to_string(&provenance) {
            Ok(json) => json,
            Err(e) => panic!("serialize must succeed: {e}"),
        };
        let round_tripped: ArtifactProvenance = match serde_json::from_str(&json) {
            Ok(v) => v,
            Err(e) => panic!("deserialize must succeed: {e}"),
        };
        assert_eq!(round_tripped, provenance);
    }
}
