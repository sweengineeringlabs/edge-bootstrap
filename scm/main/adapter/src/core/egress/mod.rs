//! DefaultEgress — egress adapter holder and its Egress impl.

mod default_egress;
mod load_balanced_http_egress;

pub(crate) use default_egress::DefaultEgress;
pub use load_balanced_http_egress::{LoadBalancedHttpEgress, LoadBalancedHttpEgressError};
