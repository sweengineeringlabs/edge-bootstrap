//! `ResourceLimits` — the bounds a `ComponentEngine` must enforce per
//! ADR-006's acceptance criteria ("Memory, CPU, concurrency, queue, and
//! payload limits are enforced and tested").

use serde::{Deserialize, Serialize};

/// Resource bounds a loaded component must be enforced against.
///
/// Every field is a hard ceiling, not a hint — exceeding one must surface as
/// [`crate::ComponentError::ResourceExhausted`], not a silent clamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum linear memory the component's instance may grow to, in
    /// bytes.
    pub max_memory_bytes: u64,
    /// Maximum wall-clock time a single invocation may run before it is
    /// cancelled and [`crate::ComponentError::Timeout`] is returned, in
    /// milliseconds.
    pub invoke_timeout_ms: u64,
    /// Maximum number of concurrent in-flight invocations of this
    /// component.
    pub max_concurrency: u32,
    /// Maximum size of the invocation's input payload, in bytes.
    pub max_payload_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_limits_serde_round_trip_preserves_all_fields() {
        let limits = ResourceLimits {
            max_memory_bytes: 16 * 1024 * 1024,
            invoke_timeout_ms: 5_000,
            max_concurrency: 8,
            max_payload_bytes: 256 * 1024,
        };
        let json = match serde_json::to_string(&limits) {
            Ok(json) => json,
            Err(e) => panic!("serialize must succeed: {e}"),
        };
        let round_tripped: ResourceLimits = match serde_json::from_str(&json) {
            Ok(v) => v,
            Err(e) => panic!("deserialize must succeed: {e}"),
        };
        assert_eq!(round_tripped, limits);
    }
}
