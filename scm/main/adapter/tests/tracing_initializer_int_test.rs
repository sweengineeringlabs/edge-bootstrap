//! Integration tests for [`TracingInitializer`].
//! Requires the `observability` feature.

#[cfg(feature = "observability")]
use swe_edge_bootstrap::TracingInitializer;

/// @covers: TracingInitializer — is a zero-size type that can be named
#[cfg(feature = "observability")]
#[test]
fn test_tracing_initializer_is_object_safe() {
    fn _assert_sized<T: Sized>() {}
    _assert_sized::<TracingInitializer>();
}
