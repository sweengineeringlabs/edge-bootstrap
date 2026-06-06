//! `ComponentHealth` — health status of a single runtime subsystem.

/// Health status of a single runtime subsystem.
#[derive(Debug, Clone)]
pub struct ComponentHealth {
    /// Name of the runtime subsystem (e.g. "http", "grpc", "db").
    pub name: String,
    /// `true` when the subsystem is operating normally.
    pub healthy: bool,
    /// Optional human-readable explanation when `healthy` is `false`.
    pub detail: Option<String>,
}

impl ComponentHealth {
    /// Construct a healthy component with no detail message.
    pub fn healthy(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            healthy: true,
            detail: None,
        }
    }
    /// Construct an unhealthy component with a detail message.
    pub fn unhealthy(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            healthy: false,
            detail: Some(detail.into()),
        }
    }
}
