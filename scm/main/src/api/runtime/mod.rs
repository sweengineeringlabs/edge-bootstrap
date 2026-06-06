//! Runtime theme — runtime entry-point, builder, manager, config, health, errors.

pub(crate) mod error;
pub(crate) mod traits;
pub(crate) mod types;

pub use error::{RuntimeError, RuntimeResult};
pub use traits::RuntimeManager;
pub use types::health::{ComponentHealth, RuntimeHealth};
pub use types::{
    Runtime, RuntimeBuilder, RuntimeBuilderServe, RuntimeConfig, RuntimeStatus, ServerConfigLoader,
    ServerMonitor, ServiceRegistry, TracingInitializer,
};
