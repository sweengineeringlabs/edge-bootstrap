//! `Sampler` — background metric sampling loop interface.

/// Marker trait for types that run a background metric-sampling loop.
pub trait Sampler: Send + Sync {}
