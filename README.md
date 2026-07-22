# swe-edge-bootstrap

> **TLDR:** The process-level assembler for `edge` — wires ingress + proxy + domain + egress,
> lifecycle, and systemd notify into a single deployable service via a fluent `RuntimeBuilder`.
> See [Overview](scm/docs/README.md) for details.

The assembler crate for `edge` — wires all port contracts (ingress, egress, proxy, domain,
observability, lifecycle) into a single deployable process via a fluent `RuntimeBuilder`.

## Key capabilities

- **`RuntimeBuilder`** — fluent builder; configure HTTP/gRPC servers, TLS, auth, egress clients,
  message broker, and lifecycle monitor in one call chain, then call `serve()` to block
- **`RuntimeManager`** — async trait for starting, stopping, and health-checking a running edge process
- **`ServerSvc`** — SAF factory for `RuntimeBuilder`; the single entry point for assembling a runtime
- **`FeatureRegistryExt`** — config-driven handler wiring; reads a TOML section, resolves `FeatureState`,
  and builds the appropriate `Handler` impl without consumer code branching on feature flags
- **`ApplicationConfigLoader`** — loads `application.toml` from XDG config paths with workspace override
- **`CompositeIngress`** / **`CompositeGrpcIngress`** — aggregate multiple `Handler` registries behind
  a single ingress dispatch surface; useful when HTTP and gRPC share handler implementations
- **`ConfigValidator`** — validates `RuntimeConfig` at startup; rejects configs missing required fields
  before any server socket is bound
- **`LifecycleMonitor`** — wires `GrpcLoadMonitor` + `HttpLoadMonitor` + `Sampler` into a single
  observer; drives autoscale decisions and exposes a `/health` surface

## Building

```bash
cd scm
cargo build
cargo test
cargo clippy -- -D warnings
```
