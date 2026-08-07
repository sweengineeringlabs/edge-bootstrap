# TLS Certificate Workflow

**Audience**: Developers, architects
**WHAT**: How a real-world TLS certificate is issued and used, and how that maps onto
`PemTlsConfig`/`RuntimeBuilder::http_tls()`/`.grpc_tls()` and the `tls-e2e` example
**WHY**: `tls-e2e` proves the handshake mechanics with a self-signed certificate, which
deliberately skips most of the real-world issuance process — this records what that process
actually looks like, so the gap between "a cert that makes TLS work" and "a cert a real client
will trust" is explicit rather than assumed

---

## The real-world process

1. **Key generation** — the applicant (e.g. a server operator) generates a public/private key
   pair locally. The private key never leaves their machine and is never sent to a CA — this is
   the core security property the whole system depends on.

2. **CSR (Certificate Signing Request)** — using that private key, the applicant builds a signed
   request containing their public key and identifying info (Common Name — the domain, e.g.
   `api.example.com`; Subject Alternative Names for additional domains/IPs; org details for
   OV/EV certs).

3. **Submit to a CA** — via ACME (Let's Encrypt/Certbot, fully automated), an enterprise CA's
   portal/API, or a commercial CA's web form.

4. **Validation** — the CA won't sign anything until it verifies the applicant actually controls
   what they're requesting a cert for:
   - **DV (Domain Validation)** — DNS-01 (a specific TXT record) or HTTP-01 (a specific file at a
     well-known URL) are the common ACME challenges.
   - **OV** — DV plus verifying the legal organization exists.
   - **EV** — much heavier identity/legal vetting (largely deprecated in browsers today).

5. **CA signs the certificate** — an X.509 certificate is built (subject, issuer, the applicant's
   public key, validity window, serial number, extensions) and signed with the **CA's own private
   key**. Almost no CA signs directly with its root key (kept offline); an **intermediate CA
   cert** (itself signed by the root) does the signing, so issuance actually returns a **chain**:
   leaf cert → intermediate(s) → (implicitly) root.

6. **Deployment** — the applicant installs the private key (secret) + the issued leaf cert + the
   intermediate chain on their server.

7. **The handshake** — the server presents its cert + chain. The client checks the signature
   chain resolves to a root it already trusts (pre-installed in the OS/browser trust store), that
   the cert hasn't expired, and that the hostname matches the cert's CN/SANs. Session keys
   themselves come from ephemeral key exchange (X25519/ECDHE in TLS 1.3) — the certificate's job
   is proving identity and signing the handshake transcript, not directly encrypting data.

8. **Revocation checking** — CRL (a CA-published list of revoked serial numbers), OCSP (a live
   "is this cert still valid?" query), or OCSP stapling (the server proactively attaches a signed
   OCSP response so the client skips the extra round trip).

9. **Renewal** — certs expire (Let's Encrypt caps at 90 days; the industry is trending toward
   ~47 days by CA/Browser Forum ballots). Steps 2–6 repeat before expiry, fully automated for
   ACME/DV certs.

## How this maps onto `PemTlsConfig`

```rust
pub struct PemTlsConfig {
    cert_pem_path: String,       // step 6: the issued leaf cert (+ chain, in a real deployment)
    key_pem_path: String,        // step 1: the private key, generated locally
    ca_pem_path: Option<String>, // mTLS only — the CA used to verify the *client's* certificate
}
```

`DefaultAcceptorBuilder::build_tls_acceptor` (`edge-security-runtime-tls`) only performs step 7's
mechanics — loading the cert/key off disk and running the handshake. It has no involvement in,
and no opinion on, steps 2–5 (the actual issuance process) or 8–9 (revocation/renewal) — those are
entirely the deploying application's responsibility, typically handled by infrastructure tooling
(e.g. cert-manager, an ACME client) outside this codebase.

## What `tls-e2e` skips, deliberately

`examples/tls-e2e/tls_setup.rs` generates a certificate with `openssl req -x509` — meaning the
cert signs **itself**, with no CA involved at all. This skips steps 2–5 entirely:

- No CSR, no submission, no domain/identity validation.
- No chain — just one leaf cert with no issuer behind it.
- No client will ever trust it by default, which is exactly why every client used to verify it in
  this session's testing had to explicitly disable validation (`curl -k`, `reqwest`'s
  invalid-cert-acceptance, `openssl s_client` reporting a self-signed verify code rather than `0`).

This is the right tradeoff for a demo that needs to run with zero external dependencies (no real
domain, no ACME account, no CA to talk to) — but it proves the handshake *mechanics* work, not
that a real client would ever trust the result unmodified.

## Related

- `scm/examples/tls-e2e/` — the live example this document explains.
- `edge-security#57` — a real bug found while building `tls-e2e`: `build_tls_acceptor` never
  installs a rustls `CryptoProvider` itself.
- `edge-security#59` — mTLS (the `ca_pem_path` path above) is proven at config-construction only
  anywhere in `edge-security` — no real client-certificate handshake has ever been tested.
