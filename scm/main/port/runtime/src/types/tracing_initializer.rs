//! `TracingInitializer` — factory for installing the tracing subscriber.

/// Factory for initialising the tracing subscriber.
///
/// Use [`TracingInitializer::init`] to install the subscriber driven by
/// `[observability.tracing]` config. Satisfies SEA Rule 191 (no free-standing fns in api/).
pub struct TracingInitializer;

#[cfg(feature = "observability")]
impl TracingInitializer {
    /// Install a `tracing-subscriber` driven by `config`.
    ///
    /// Requires the `observability` feature. Idempotent — safe to call multiple
    /// times. Does nothing when `config.enabled` is `false`. `RUST_LOG` overrides
    /// `config.level` and `config.filter`.
    pub fn init(config: &swe_edge_observ_config::TracingConfig) {
        swe_edge_observ_config::TracingSvc::init(config);
    }
}
