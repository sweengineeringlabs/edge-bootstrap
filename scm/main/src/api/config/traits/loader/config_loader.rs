//! ConfigLoader trait — layered config loading contract.

use crate::api::config::ConfigError;
use crate::api::runtime::RuntimeConfig;

/// Loads `RuntimeConfig` from the layered config chain:
/// `default.toml` → `application.toml` → `tenants/<id>.toml` → env vars.
pub trait ConfigLoader: Send + Sync {
    /// Load config for a single-tenant (or unscoped) deployment.
    fn load(&self) -> Result<RuntimeConfig, ConfigError>;

    /// Load config scoped to a specific tenant, layering
    /// `tenants/<tenant_id>.toml` on top of `application.toml`.
    fn load_for_tenant(&self, tenant_id: &str) -> Result<RuntimeConfig, ConfigError>;

    /// Load an arbitrary TOML section from the layered config chain.
    ///
    /// `key` is a dotted path into the config tree, e.g.
    /// `"observability.tracing"` or `"application.completion"`.
    /// Returns `Ok(T::default())` if the key is absent from all sources.
    fn load_section<T>(&self, key: &str) -> Result<T, ConfigError>
    where
        T: serde::de::DeserializeOwned + Default;
}
