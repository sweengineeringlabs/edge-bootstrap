//! `ComponentError` — the structured error taxonomy for `ComponentEngine`/
//! `ComponentValidator`, per ADR-007's decision that manifest/capability
//! validation must be testable independent of any concrete Wasm engine.

use thiserror::Error;

/// Errors that can occur while validating, loading, or invoking a Wasm
/// component handler.
///
/// Variants split into two phases: validation/load-time (rejected before a
/// route is ever registered — [`InvalidManifest`](ComponentError::InvalidManifest),
/// [`UnsupportedContractVersion`](ComponentError::UnsupportedContractVersion),
/// [`RouteCollision`](ComponentError::RouteCollision),
/// [`UntrustedArtifact`](ComponentError::UntrustedArtifact)) and
/// invocation-time (surfaced from a live call —
/// [`Trap`](ComponentError::Trap), [`InvalidOutput`](ComponentError::InvalidOutput),
/// [`ResourceExhausted`](ComponentError::ResourceExhausted),
/// [`Cancelled`](ComponentError::Cancelled), [`Timeout`](ComponentError::Timeout),
/// [`CapabilityUnavailable`](ComponentError::CapabilityUnavailable)) — matching
/// ADR-006's acceptance criteria that both classes map to deterministic
/// framework errors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ComponentError {
    /// The manifest itself is structurally invalid or missing a required
    /// field.
    #[error("invalid manifest: {0}")]
    InvalidManifest(String),
    /// The manifest declares a `swe:edge-handler` contract version this
    /// engine does not support.
    #[error("unsupported contract version: {0}")]
    UnsupportedContractVersion(String),
    /// The manifest's declared route collides with an already-registered
    /// route.
    #[error("route collision: {0}")]
    RouteCollision(String),
    /// The component artifact failed provenance/integrity verification.
    #[error("untrusted artifact: {0}")]
    UntrustedArtifact(String),
    /// The component trapped (panicked/aborted) during execution.
    #[error("component trap: {0}")]
    Trap(String),
    /// The component produced output that does not match its declared
    /// export shape.
    #[error("invalid output: {0}")]
    InvalidOutput(String),
    /// A configured memory, CPU, concurrency, or queue limit was exceeded.
    #[error("resource exhausted: {0}")]
    ResourceExhausted(String),
    /// The invocation was cancelled before it completed.
    #[error("invocation cancelled")]
    Cancelled,
    /// The invocation exceeded its configured deadline.
    #[error("invocation timed out")]
    Timeout,
    /// The component attempted to use a capability it was not granted.
    #[error("capability unavailable: {0}")]
    CapabilityUnavailable(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_error_display_invalid_manifest_includes_reason() {
        let err = ComponentError::InvalidManifest("missing route_id".to_string());
        assert_eq!(err.to_string(), "invalid manifest: missing route_id");
    }

    #[test]
    fn test_component_error_display_cancelled_has_fixed_message() {
        let err = ComponentError::Cancelled;
        assert_eq!(err.to_string(), "invocation cancelled");
    }

    #[test]
    fn test_component_error_eq_distinguishes_variants_with_same_payload_shape() {
        let trap = ComponentError::Trap("division by zero".to_string());
        let invalid_output = ComponentError::InvalidOutput("division by zero".to_string());
        assert_ne!(
            trap, invalid_output,
            "different variants must not compare equal even with identical string payloads"
        );
    }
}
