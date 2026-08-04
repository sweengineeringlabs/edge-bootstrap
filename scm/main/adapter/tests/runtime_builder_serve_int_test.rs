//! Integration tests for RuntimeBuilderServe.

use swe_edge_bootstrap::Runtime;

/// @covers: RuntimeBuilderServe
#[test]
fn test_runtime_builder_starts_with_all_fields_none() {
    let b = Runtime::builder();
    assert!(b.build_registry().is_none());
}
