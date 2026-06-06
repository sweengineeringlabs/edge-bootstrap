//! Gateway layer — inbound and outbound integration boundaries.
pub(crate) mod egress;
pub(crate) mod ingress;

pub use crate::saf::*;
