//! Request/response DTOs for [`crate::ComponentEngine::invoke`] —
//! byte-oriented unary invocation, per ADR-006's WIT contract scope.

use std::time::Duration;

use crate::types::component_handle::ComponentHandle;

/// Request to invoke a loaded component's handler export once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentInvokeRequest {
    /// Handle to the component instance to invoke, from a prior
    /// [`crate::ComponentEngine::load`] call.
    pub handle: ComponentHandle,
    /// Raw request payload bytes, passed to the component's export
    /// unmodified — encoding/decoding into an application-level type
    /// happens above this port, matching how `DefaultHttpJob`/
    /// `DefaultGrpcJob` already operate on raw bytes.
    pub payload: Vec<u8>,
    /// Maximum time to wait for this invocation before it is cancelled and
    /// [`crate::ComponentError::Timeout`] is returned. Must never exceed
    /// the component's own [`crate::types::ResourceLimits::invoke_timeout_ms`].
    pub deadline: Duration,
}

/// Response from a successful [`crate::ComponentEngine::invoke`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentInvokeResponse {
    /// Raw response payload bytes produced by the component's export.
    pub payload: Vec<u8>,
}
