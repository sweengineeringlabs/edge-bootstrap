//! Config validation — ensures `RuntimeConfig` fields are in bounds before serve().

use crate::api::runtime::RuntimeConfig;
use crate::api::runtime::RuntimeError;
use crate::api::validator::Validator;

/// Validates a [`RuntimeConfig`] before the runtime starts.
///
/// Checks non-empty bind addresses and positive shutdown timeout.
pub(crate) struct ConfigValidator;

impl crate::api::validator::ConfigValidator for ConfigValidator {}

impl Validator for ConfigValidator {
    type Target = RuntimeConfig;
    type Error = RuntimeError;

    fn validate(&self, value: &RuntimeConfig) -> Result<(), RuntimeError> {
        if value.http_bind.trim().is_empty() {
            return Err(RuntimeError::StartFailed(
                "http_bind must not be empty".into(),
            ));
        }
        if value.grpc_bind.trim().is_empty() {
            return Err(RuntimeError::StartFailed(
                "grpc_bind must not be empty".into(),
            ));
        }
        if value.shutdown_timeout_secs == 0 {
            return Err(RuntimeError::StartFailed(
                "shutdown_timeout_secs must be > 0".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid() -> RuntimeConfig {
        RuntimeConfig::default()
    }

    #[test]
    fn test_validate_accepts_valid_config() {
        assert!(ConfigValidator.validate(&valid()).is_ok());
    }

    #[test]
    fn test_validate_rejects_empty_http_bind() {
        let mut c = valid();
        c.http_bind = "".into();
        assert!(matches!(
            ConfigValidator.validate(&c),
            Err(RuntimeError::StartFailed(_))
        ));
    }

    #[test]
    fn test_validate_rejects_empty_grpc_bind() {
        let mut c = valid();
        c.grpc_bind = "".into();
        assert!(matches!(
            ConfigValidator.validate(&c),
            Err(RuntimeError::StartFailed(_))
        ));
    }

    #[test]
    fn test_validate_rejects_zero_shutdown_timeout() {
        let mut c = valid();
        c.shutdown_timeout_secs = 0;
        assert!(matches!(
            ConfigValidator.validate(&c),
            Err(RuntimeError::StartFailed(_))
        ));
    }
}
