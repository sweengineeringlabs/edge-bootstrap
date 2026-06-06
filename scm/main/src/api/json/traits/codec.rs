//! `Codec` — JSON codec interface for HTTP and gRPC routes.

/// Marker trait for JSON codec implementations.
pub trait Codec: Send + Sync {}
