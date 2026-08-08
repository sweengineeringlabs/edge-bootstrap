//! `ComponentHandle` — an opaque reference to a component instance a
//! `ComponentEngine` has loaded, returned from `load` and consumed by
//! `invoke`. Deliberately opaque (a newtype over `String`, not exposing any
//! engine-internal state) so the port stays independent of whatever the
//! concrete engine uses to key its cache/pool internally.

use serde::{Deserialize, Serialize};

/// Opaque handle to a loaded component instance.
///
/// Callers must treat this as an unstructured token: construct it only from
/// a [`crate::ComponentEngine::load`] response, never by hand.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ComponentHandle(pub String);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_handle_equality_is_by_value_not_identity() {
        let a = ComponentHandle("echo-handler-v1".to_string());
        let b = ComponentHandle("echo-handler-v1".to_string());
        assert_eq!(a, b);
    }

    #[test]
    fn test_component_handle_inequality_for_distinct_ids() {
        let a = ComponentHandle("echo-handler-v1".to_string());
        let b = ComponentHandle("echo-handler-v2".to_string());
        assert_ne!(a, b);
    }
}
