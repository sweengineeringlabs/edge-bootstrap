//! `ApplicationConfigLoader` — filesystem-backed layered config loader interface.

use crate::traits::loader::ConfigLoader;

/// Marker supertrait for filesystem-backed, layered config loaders.
pub trait ApplicationConfigLoader: ConfigLoader + Send + Sync {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ConfigError;
    use swe_edge_bootstrap_runtime::RuntimeConfig;

    struct MinimalAppConfigLoader;
    impl ConfigLoader for MinimalAppConfigLoader {
        fn load(&self) -> Result<RuntimeConfig, ConfigError> {
            Ok(RuntimeConfig::default())
        }
        fn load_for_tenant(&self, _tenant_id: &str) -> Result<RuntimeConfig, ConfigError> {
            Ok(RuntimeConfig::default())
        }
        fn load_section<T>(&self, _key: &str) -> Result<T, ConfigError>
        where
            T: serde::de::DeserializeOwned + Default,
        {
            Ok(T::default())
        }
    }
    impl ApplicationConfigLoader for MinimalAppConfigLoader {}

    // `ConfigLoader::load_section` is generic, so the supertrait bound can't be
    // proven through `dyn ConfigLoader` (not dyn-compatible) — prove it via a
    // static bound instead: anything satisfying `ApplicationConfigLoader` must
    // also satisfy the full `ConfigLoader` contract.
    fn assert_satisfies_config_loader<T: ConfigLoader>(loader: &T) -> Result<(), ConfigError> {
        loader.load().map(|_| ())
    }

    #[test]
    fn test_application_config_loader_double_satisfies_config_loader_supertrait() {
        assert!(assert_satisfies_config_loader(&MinimalAppConfigLoader).is_ok());
    }
}
