# swe-edge-bootstrap

## WHAT

The assembler crate for swe-edge — wires all port contracts (ingress, egress, proxy, domain,
observability, lifecycle) into a single deployable process via a fluent `RuntimeBuilder`.

Key capabilities:

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

## WHY

| Problem | Solution |
|---------|----------|
| Wiring ingress + egress + lifecycle requires 200+ lines of boilerplate per service | `RuntimeBuilder` assembles all layers in one fluent chain; defaults are production-safe |
| Feature flag logic bleeds into handler code | `FeatureRegistryExt::build_handler` reads TOML config and returns the right `Handler` impl; caller never branches |
| Config errors discovered at request time, not at startup | `ConfigValidator` validates the full `RuntimeConfig` before any socket is bound |
| HTTP and gRPC share handler logic but live on separate dispatch registries | `CompositeIngress` / `CompositeGrpcIngress` unify routing without duplicating handler registration |
| Observability config spread across multiple crates | `ApplicationConfigLoader` merges `[tracing]` + `[metrics]` from a single TOML file via XDG path resolution |
| Graceful shutdown logic re-implemented per service | `RuntimeManager::stop()` coordinates shutdown across HTTP, gRPC, and Prometheus servers with signal handling |

## HOW

## Documents

| Document | Description |
|----------|-------------|
| [3-design/](3-design/) | Architecture decision records |
| [7-operations/](7-operations/) | Compliance and structural audit reports |
