//! `ConfigError` — errors from loading daemon configuration.

use thiserror::Error;

/// Errors that can occur when loading daemon configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// TOML or JSON parse error.
    #[error("config parse error: {0}")]
    Parse(String),
    /// File or directory I/O error.
    #[error("config io error: {0}")]
    Io(String),
    /// Requested tenant ID does not exist in the config.
    #[error("unknown tenant: '{0}'")]
    UnknownTenant(String),
    /// Tenant ID contains characters outside `[a-zA-Z0-9_-]`.
    #[error("invalid tenant id: '{0}' — only [a-zA-Z0-9_-] are allowed")]
    InvalidTenantId(String),
    /// Required environment variable is missing or malformed.
    #[error("invalid env var: {0}")]
    BadEnvVar(String),
    /// A [`HandlerFactory::build`](edge_domain::HandlerFactory::build) call failed.
    #[error("handler build failed: {0}")]
    HandlerBuild(String),
}

impl From<swe_edge_configbuilder::ConfigError> for ConfigError {
    fn from(e: swe_edge_configbuilder::ConfigError) -> Self {
        match e {
            swe_edge_configbuilder::ConfigError::Parse(s) => ConfigError::Parse(s),
            swe_edge_configbuilder::ConfigError::Io(s) => ConfigError::Io(s),
            other => ConfigError::Io(other.to_string()),
        }
    }
}
