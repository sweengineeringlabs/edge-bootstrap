//! [`ServerMonitor`] — factory for constructing observed lifecycle monitors.

/// Factory for wrapping lifecycle monitors with metrics observation.
///
/// Methods on this type provide the SAF surface for constructing observed
/// lifecycle monitors (Rule 191 — no free-standing fns in SAF).
pub struct ServerMonitor;
