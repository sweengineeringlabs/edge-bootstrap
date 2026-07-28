//! Runtime SAF-facing factory types — behavior-bearing, not port contracts.
//!
//! `Runtime`, `RuntimeBuilder`, `ServerConfigLoader`, and `ServerMonitor` are
//! defined here (not in `swe-edge-bootstrap-runtime`) because their real
//! methods are implemented across this file, `runtime_builder_serve.rs`, and
//! `saf::bootstrap_svc` — all within this crate. Rust's orphan rules require
//! a type and every inherent `impl` of it to share one crate, so these
//! cannot be pure port/contract types.

#[allow(clippy::module_inception)]
pub(crate) mod runtime;
pub(crate) mod runtime_builder;
pub(crate) mod server;

pub use runtime::Runtime;
pub use runtime_builder::RuntimeBuilder;
pub use server::{ServerConfigLoader, ServerMonitor};
