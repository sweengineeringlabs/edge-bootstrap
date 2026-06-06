//! `ConfigValidator` — runtime configuration validator interface.

use crate::api::runtime::RuntimeConfig;
use crate::api::runtime::RuntimeError;
use crate::api::validator::Validator;

/// Marker supertrait for `RuntimeConfig` validators.
pub trait ConfigValidator: Validator<Target = RuntimeConfig, Error = RuntimeError> {}
