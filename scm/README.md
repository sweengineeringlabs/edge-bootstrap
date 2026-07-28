# swe-edge-bootstrap

> **TLDR:** The process-level assembler for `edge` — wires ingress + proxy + domain + egress,
> lifecycle, and systemd notify into a single deployable service via a fluent `RuntimeBuilder`.
> See [Overview](docs/README.md) for details.

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

## Layout

`main/port/*` holds the trait contracts (zero implementation); `main/adapter/src/`
holds the one real implementation that wires them together — `core/` for the concrete
adapters (`core/health/default_health_handler.rs`, `core/egress/default_egress.rs`,
etc.), plus `saf/` (the fluent `RuntimeBuilder`/`ServerSvc` assembly layer) and `spi/`
(runtime extension points). `port` and `adapter` are parallel, equally-named concepts
under `main/`.

## Port trait relationships

Each `main/port/*` crate is a standalone, implementation-free trait contract (see
[`docs/`](docs/README.md)). Ports are not siloed — a trait in one port crate may be bounded by,
or reference the types of, a trait in another. Expressing that at the type level (supertraits,
trait bounds, associated types) rather than only in doc comments is what makes the dependency
graph between ports obvious from the code itself.

| Port crate    | Trait                              | Relationship                                                              | Status |
|----------------|-------------------------------------|----------------------------------------------------------------------------|--------|
| `validator`    | `ConfigValidator`                   | `: Validator<Target = runtime::RuntimeConfig, Error = runtime::RuntimeError>` | done |
| `config`       | `ConfigLoader`                      | `load() -> Result<runtime::RuntimeConfig, ConfigError>`                    | done |
| `config`       | `ApplicationConfigLoader`           | `: ConfigLoader` (intra-crate supertrait)                                  | done |
| `metrics`      | `MetricsExporter`                   | `counters() -> &monitor::SharedCounters`                                   | done |
| `runtime`      | `RuntimeConfig`                     | `metrics: Option<monitor::MetricsConfig>`, `autoscale: Option<monitor::AutoscalePolicy>` | done |
| `runner`       | `Runner`                            | `type Manager: RuntimeManager` associated type — `run()` is bound to a real manager, not just `runtime::RuntimeResult` | done — no adapter yet ([#2](https://github.com/sweengineeringlabs/edge-bootstrap/issues/2)) |
| `composite`    | `CompositeIngress`                  | `: ingress::Ingress` supertrait (composite routing is a specialization of ingress supply) | done — no adapter yet ([#2](https://github.com/sweengineeringlabs/edge-bootstrap/issues/2)) |
| `health`       | `HealthHandler`                     | `fn health(&self) -> BoxFuture<'_, runtime::RuntimeHealth>`                | done |
| `metrics`      | `MetricsHandler`                    | `: HttpIngress + MetricsExporter` supertrait (intra-crate)                 | done |
| `monitor`      | `Sampler`                           | `fn counters(&self) -> &monitor::SharedCounters`                           | done |

`composite::CompositeIngress` and `runner::Runner` additionally have no concrete
implementation yet in `main/adapter/src/core/` — tracked in
[#2](https://github.com/sweengineeringlabs/edge-bootstrap/issues/2).

## Building

```bash
cargo build
cargo test
cargo clippy -- -D warnings
```
