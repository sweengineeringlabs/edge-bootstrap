//! Validator theme port contracts.

pub mod config_validator;
#[allow(clippy::module_inception)]
pub mod validator;

pub use config_validator::ConfigValidator;
pub use validator::Validator;
