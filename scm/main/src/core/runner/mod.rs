//! Testable daemon runner — start, await signal, shut down within timeout.

#[allow(clippy::module_inception)]
mod runner;

pub(crate) use runner::DaemonRunner;
