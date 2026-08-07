//! TLS material for this demo — the one thing this module owns is "how to
//! get a `PemTlsConfig` (or none, for the plaintext default) before
//! `serve()` starts."
//!
//! `RuntimeBuilder::http_tls`/`grpc_tls` themselves aren't gated behind any
//! Cargo feature (see `runtime_builder.rs`) — there is nothing of the
//! framework's to opt into. Serving TLS by default would, however, make
//! `cargo run --example tls-e2e` fail outright on any machine without
//! `openssl` on `PATH`, unlike every other example in this crate. So this is
//! this demo's own choice: TLS is opt-in via the `TLS_E2E_TLS` environment
//! variable, and the zero-config path serves plaintext, matching the rest
//! of the crate's examples.
//!
//! Run with TLS: `TLS_E2E_TLS=1 cargo run -p swe-edge-bootstrap --example tls-e2e`
//!
//! One more thing this module owns: installing a process-wide rustls
//! `CryptoProvider` before any TLS-configured server starts. This process
//! links both a client-side gRPC transport (pulling in `hyper-rustls`'s
//! aws-lc-rs backend) and `edge-security-runtime-tls`'s server-side
//! acceptor (built on rustls's ring backend) — with both backend features
//! present, rustls refuses to auto-select either and panics on first use.
//! `edge-security-runtime-tls`'s production `build_tls_acceptor` doesn't
//! install a provider itself, so a consumer wiring in `.http_tls()`/
//! `.grpc_tls()` has to (confirmed the hard way: without this, the TLS
//! listener task panics on its first request instead of ever completing a
//! handshake). Only paid for on the TLS-enabled path — the plaintext
//! default never touches rustls at all.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Once;

use swe_edge_bootstrap::PemTlsConfig;

static CRYPTO_PROVIDER_INIT: Once = Once::new();

/// TLS material for the demo: the [`PemTlsConfig`] handed to
/// `RuntimeBuilder`, plus the temp directory the cert/key live in.
///
/// The directory is kept alive for as long as the caller holds this value —
/// `RuntimeBuilder::serve()` reads the cert/key files from disk when it
/// builds the TLS acceptor, so they must still exist at that point.
pub(crate) struct TlsMaterial {
    pub(crate) config: PemTlsConfig,
    _cert_dir: tempfile::TempDir,
}

/// Returns `Some(TlsMaterial)` when `TLS_E2E_TLS` is set to a truthy value
/// (generating a real self-signed certificate via the `openssl` CLI),
/// `None` otherwise — the zero-config default: plaintext.
pub(crate) fn build_tls_material() -> Option<TlsMaterial> {
    if !env_flag_enabled(std::env::var("TLS_E2E_TLS").ok().as_deref()) {
        return None;
    }
    ensure_crypto_provider();
    let cert_dir = match tempfile::tempdir() {
        Ok(dir) => dir,
        Err(e) => panic!("create tempdir for generated TLS material: {e}"),
    };
    let (cert_path, key_path) = generate_self_signed_cert(cert_dir.path());
    Some(TlsMaterial {
        config: PemTlsConfig::tls(
            cert_path.to_string_lossy().into_owned(),
            key_path.to_string_lossy().into_owned(),
        ),
        _cert_dir: cert_dir,
    })
}

fn env_flag_enabled(value: Option<&str>) -> bool {
    matches!(value, Some(v) if v == "1" || v.eq_ignore_ascii_case("true"))
}

/// Installs a process-wide rustls `CryptoProvider` exactly once — see the
/// module doc comment for why this is necessary before any TLS-configured
/// server can accept a connection.
fn ensure_crypto_provider() {
    CRYPTO_PROVIDER_INIT.call_once(|| {
        if rustls::crypto::ring::default_provider()
            .install_default()
            .is_err()
        {
            panic!("install a process-wide rustls CryptoProvider exactly once");
        }
    });
}

/// Shell out to the `openssl` CLI to generate a real self-signed certificate
/// and private key into `dir` — a genuine cert/key pair produced by a real
/// TLS toolchain, not a canned fixture checked into the repo.
fn generate_self_signed_cert(dir: &Path) -> (PathBuf, PathBuf) {
    let cert_path = dir.join("cert.pem");
    let key_path = dir.join("key.pem");
    let status = match Command::new("openssl")
        .arg("req")
        .arg("-x509")
        .arg("-newkey")
        .arg("rsa:2048")
        .arg("-nodes")
        .arg("-keyout")
        .arg(&key_path)
        .arg("-out")
        .arg(&cert_path)
        .arg("-days")
        .arg("1")
        .arg("-subj")
        .arg("/CN=localhost")
        .status()
    {
        Ok(s) => s,
        Err(e) => {
            panic!("spawn `openssl` to generate a self-signed cert (openssl must be on PATH): {e}")
        }
    };
    assert!(
        status.success(),
        "`openssl req` must succeed generating the self-signed cert"
    );
    (cert_path, key_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// @covers: env_flag_enabled
    #[test]
    fn test_env_flag_enabled_true_for_one_happy() {
        assert!(env_flag_enabled(Some("1")));
    }

    /// @covers: ensure_crypto_provider
    #[test]
    fn test_ensure_crypto_provider_is_idempotent_across_repeated_calls_happy() {
        // Must not panic, however many times it's called in this process —
        // `CryptoProvider::install_default()` itself errors on a second
        // install, which is exactly what `Once` exists to paper over here.
        ensure_crypto_provider();
        ensure_crypto_provider();
        ensure_crypto_provider();
    }

    /// @covers: env_flag_enabled
    #[test]
    fn test_env_flag_enabled_true_for_true_case_insensitive_happy() {
        assert!(env_flag_enabled(Some("TRUE")));
        assert!(env_flag_enabled(Some("true")));
    }

    /// @covers: env_flag_enabled
    #[test]
    fn test_env_flag_enabled_false_for_unset_edge() {
        assert!(!env_flag_enabled(None));
    }

    /// @covers: env_flag_enabled
    #[test]
    fn test_env_flag_enabled_false_for_zero_edge() {
        assert!(!env_flag_enabled(Some("0")));
    }

    /// @covers: env_flag_enabled
    #[test]
    fn test_env_flag_enabled_false_for_garbage_value_negative() {
        assert!(!env_flag_enabled(Some("nope")));
    }

    /// @covers: generate_self_signed_cert
    #[test]
    fn test_generate_self_signed_cert_produces_valid_pem_files_happy() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (cert_path, key_path) = generate_self_signed_cert(dir.path());

        let cfg = PemTlsConfig::tls(
            cert_path.to_string_lossy().into_owned(),
            key_path.to_string_lossy().into_owned(),
        );
        cfg.validate()
            .expect("generated cert/key paths must be non-empty and pass PemTlsConfig::validate");

        let cert_bytes = std::fs::read(&cert_path).expect("read generated cert file");
        assert!(
            cert_bytes.starts_with(b"-----BEGIN CERTIFICATE-----"),
            "generated cert file must be a real PEM certificate, not a placeholder"
        );
        let key_bytes = std::fs::read(&key_path).expect("read generated key file");
        assert!(
            key_bytes.starts_with(b"-----BEGIN PRIVATE KEY-----"),
            "generated key file must be a real PEM private key, not a placeholder"
        );
    }
}
