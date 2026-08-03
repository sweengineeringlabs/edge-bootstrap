//! IDS/IPS wiring — the one thing this module owns is "how does
//! `edge-intrusion` get configured and built for this demo."

use edge_intrusion::config::{Config as IntrusionConfig, Wired};

/// Build the `edge-intrusion` wiring: baseline OWASP signature rules only
/// (empty TOML means no threshold/reputation/anomaly rules, but
/// `signature_rules.baseline_enabled` defaults to `true`), rejecting on
/// any `Severity::High`-or-above match — the default `EnforcerConfig`.
pub(crate) fn build_intrusion_guard() -> Wired {
    let config = match IntrusionConfig::from_toml_str("") {
        Ok(cfg) => cfg,
        Err(e) => panic!("empty config must parse to baseline defaults: {e}"),
    };
    match config.build() {
        Ok(wired) => wired,
        Err(e) => panic!("baseline rules must build: {e}"),
    }
}
