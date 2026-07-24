//! `swe_edge_runtime` — process-level runtime manager.

#![allow(
    dead_code,
    unused_imports,
    clippy::let_and_return,
    clippy::module_inception,
    clippy::expect_used,
    clippy::unwrap_used,
    missing_docs
)]

mod api;
mod core;
mod saf;
mod spi;

pub use crate::saf::*;
