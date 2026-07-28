//! ConfigLoader trait — layered config loading contract.

use crate::error::ConfigError;
use swe_edge_bootstrap_runtime::RuntimeConfig;

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

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedConfigLoader;
    impl ConfigLoader for FixedConfigLoader {
        fn load(&self) -> Result<RuntimeConfig, ConfigError> {
            Ok(RuntimeConfig::default())
        }
        fn load_for_tenant(&self, tenant_id: &str) -> Result<RuntimeConfig, ConfigError> {
            if tenant_id.is_empty() {
                Err(ConfigError::UnknownTenant(tenant_id.to_string()))
            } else {
                Ok(RuntimeConfig::default())
            }
        }
        fn load_section<T>(&self, _key: &str) -> Result<T, ConfigError>
        where
            T: serde::de::DeserializeOwned + Default,
        {
            Ok(T::default())
        }
    }

    #[test]
    fn test_config_loader_double_load_returns_default_config() {
        assert_eq!(FixedConfigLoader.load().unwrap().service_name, "swe-edge");
    }

    #[test]
    fn test_config_loader_double_load_for_tenant_rejects_empty_tenant() {
        assert!(matches!(
            FixedConfigLoader.load_for_tenant(""),
            Err(ConfigError::UnknownTenant(_))
        ));
    }

    #[test]
    fn test_config_loader_double_load_section_returns_type_default() {
        let v: bool = FixedConfigLoader.load_section("whatever").unwrap();
        assert!(!v);
    }
}
