//! `swe-edge-bootstrap-config-port` — layered config loading ports (traits
//! + errors), zero implementation.
//!
//! The concrete implementation (`ApplicationConfigLoader`, plus its
//! `ConfigOverride` TOML-merge helper) lives in this repo's `core/config/`
//! layer.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod traits;

pub use error::ConfigError;
pub use traits::{ApplicationConfigLoader, ConfigLoader, FeatureHandlerBridge, HandlerFactory};
