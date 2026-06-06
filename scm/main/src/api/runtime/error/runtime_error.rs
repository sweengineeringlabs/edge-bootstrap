//! `RuntimeError` — errors from runtime manager operations.

use thiserror::Error;

/// Errors that can occur during runtime manager operations.
#[derive(Debug, Error)]
pub enum RuntimeError {
    /// Runtime failed to start one or more components.
    #[error("start failed: {0}")]
    StartFailed(String),
    /// Runtime failed to cleanly stop one or more components.
    #[error("shutdown failed: {0}")]
    ShutdownFailed(String),
    /// Could not bind the server socket to the configured address.
    #[error("bind failed: {0}")]
    BindFailed(String),
    /// Unexpected internal error.
    #[error("internal: {0}")]
    Internal(String),
    /// Graceful shutdown exceeded the configured timeout (seconds).
    #[error("shutdown timed out after {0}s")]
    ShutdownTimeout(u64),
    /// Message broker error.
    #[error("broker error: {0}")]
    Broker(String),
    /// Scheduler error.
    #[error("scheduler error: {0}")]
    Scheduler(String),
}

impl From<crate::api::config::ConfigError> for RuntimeError {
    fn from(e: crate::api::config::ConfigError) -> Self {
        RuntimeError::StartFailed(e.to_string())
    }
}

#[cfg(feature = "message-broker")]
impl From<swe_edge_runtime_message_broker::BrokerError> for RuntimeError {
    fn from(e: swe_edge_runtime_message_broker::BrokerError) -> Self {
        RuntimeError::Broker(e.to_string())
    }
}

#[cfg(feature = "scheduler")]
impl From<swe_edge_runtime_scheduler::SchedulerError> for RuntimeError {
    fn from(e: swe_edge_runtime_scheduler::SchedulerError) -> Self {
        RuntimeError::Scheduler(e.to_string())
    }
}
