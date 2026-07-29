//! `swe-edge-bootstrap-json` — JSON codec port (trait contract) and
//! default decode/encode function-pointer aliases, zero implementation.
//!
//! The concrete implementation (`Codec`) lives in this repo's
//! `core/json/` layer.

#![deny(unsafe_code)]
#![warn(missing_docs)]
// The `json_codec` function-pointer aliases are unused scaffolding carried
// over verbatim from the pre-split `api/json/types/json_codec.rs` — not
// introduced here.
#![allow(dead_code)]

pub mod traits;
pub mod types;

pub use traits::Codec;
