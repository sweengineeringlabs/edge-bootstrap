//! `ConfigValidator` — runtime configuration validator interface.

use crate::traits::Validator;
use swe_edge_bootstrap_runtime::{RuntimeConfig, RuntimeError};

/// Marker supertrait for `RuntimeConfig` validators.
pub trait ConfigValidator: Validator<Target = RuntimeConfig, Error = RuntimeError> {}

#[cfg(test)]
mod tests {
    use super::*;

    struct RejectEmptyHttpBind;
    impl Validator for RejectEmptyHttpBind {
        type Target = RuntimeConfig;
        type Error = RuntimeError;
        fn validate(&self, value: &RuntimeConfig) -> Result<(), RuntimeError> {
            if value.http_bind.is_empty() {
                Err(RuntimeError::Internal("http_bind must not be empty".into()))
            } else {
                Ok(())
            }
        }
    }
    impl ConfigValidator for RejectEmptyHttpBind {}

    #[test]
    fn test_config_validator_double_accepts_default_config() {
        assert!(RejectEmptyHttpBind
            .validate(&RuntimeConfig::default())
            .is_ok());
    }

    #[test]
    fn test_config_validator_double_rejects_empty_http_bind() {
        let cfg = RuntimeConfig {
            http_bind: String::new(),
            ..RuntimeConfig::default()
        };
        assert!(RejectEmptyHttpBind.validate(&cfg).is_err());
    }
}
