//! `swe-edge-bootstrap-runtime` — runtime config/health/error/manager
//! ports (traits + DTOs), zero implementation.
//!
//! `Runtime`, `RuntimeBuilder`, `ServerConfigLoader`, and `ServerMonitor` are
//! **not** exposed here — their behavior (server hosting, XDG/TOML config
//! loading orchestration, JSON codec wiring) is real implementation, not a
//! contract, so those types live entirely in this repo's `core/runtime/`
//! layer instead and are re-exported by `saf::`.

#![deny(unsafe_code)]
#![warn(missing_docs)]
// `RuntimeConfigBuilder` is unused scaffolding carried over verbatim from
// the pre-split `api/runtime/types/runtime_config.rs` — not introduced here.
#![allow(dead_code)]

pub mod error;
pub mod traits;
pub mod types;

pub use error::{RuntimeError, RuntimeResult};
pub use traits::RuntimeManager;
pub use types::health::{ComponentHealth, RuntimeHealth};
pub use types::{
    LogBackendConfig, LogBackendKind, LogElkSettings, LogFileSettings, LogOtelSettings,
    LogSqliteSettings, MetricsBackendConfig, MetricsBackendKind, MetricsFileSettings,
    MetricsOtelSettings, MetricsPrometheusSettings, MetricsSqliteSettings, RuntimeConfig,
    RuntimeStatus, ServiceRegistry, TracerBackendConfig, TracerBackendKind, TracingInitializer,
};
