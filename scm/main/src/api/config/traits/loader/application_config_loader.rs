//! `ApplicationConfigLoader` — filesystem-backed layered config loader interface.

use crate::api::config::traits::loader::ConfigLoader;

/// Marker supertrait for filesystem-backed, layered config loaders.
pub trait ApplicationConfigLoader: ConfigLoader + Send + Sync {}
