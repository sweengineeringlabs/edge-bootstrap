//! `ConfigValidator` — runtime configuration validator interface.

use crate::traits::Validator;
use swe_edge_bootstrap_runtime_port::{RuntimeConfig, RuntimeError};

/// Marker supertrait for `RuntimeConfig` validators.
pub trait ConfigValidator: Validator<Target = RuntimeConfig, Error = RuntimeError> {}
