# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed
- Migrated to `edge-application@v0.18.0` (dependency key `edge-domain`), completing the
  org-wide `edge-domain` → `edge-application` rename/rewrite for this crate. All direct and
  transitive dependencies now resolve a single, unified `Handler`/`HandlerRegistry`/`Request`/
  `Response` trait set:
  - `edge-dispatch` (`edge-dispatcher`) bumped to `v0.11.2`, which re-exports `Handler`/
    `HandlerRegistry` directly from `edge-application-handler` instead of defining its own
    copies.
  - `swe-edge-ingress-http`/`swe-edge-ingress-grpc` bumped to their `v0.5.3`/`v0.6.3` tags;
    `HttpIngress`/`GrpcIngress` trait methods now take single envelope request types
    (`InboundRequest`, `UnaryRequest`, `StreamRequest`, `HealthCheckRequest`, ...) and
    `HttpIngress::handle`/`health_check` return a local `HttpFuture` newtype rather than
    `futures::future::BoxFuture`.
  - `edge-security-runtime`/`-authz`/`-credential`/`-tls` added as direct dependencies
    (`v0.3.7`) to source `PemTlsConfig`, `SecurityContext`, `Principal`, and `TenantId`, none
    of which are re-exported by `edge-application` itself anymore.
  - `swe-edge-runtime-grpc`/`swe-edge-runtime-http` (`edge-runtime@v0.3.17`) added as new
    dependencies — `TonicGrpcServer`/`AxumHttpServer` were extracted out of the transport
    crates into this repo.
  - `edge-proxy`'s `LifecycleMonitor` trait now takes/returns request-envelope types
    (`ShutdownRequest`, `StatusRequest`, `ComponentRequest`, `HealthRequest`, `HealthResponse`).
- `examples/hello_edge.rs` no longer routes through `edge-proxy`'s `Job`/`Router` traits or
  `edge-llm-provider`'s handler-construction API — both were incompatible with the migrated
  `Handler`/`HandlerContext` shape (`edge-proxy`'s `Job`/`Router` still key off a separate,
  unmigrated `edge-domain` package; `edge-llm-provider`'s `StdProviderFactory::provider_handler`
  requires a full `ExecutionModel` impl unrelated to this crate's wiring). Dispatch is now
  inlined directly against the `HandlerRegistry`, with a self-contained echo handler standing
  in for the provider handler. `edge-proxy`'s `ProxySvc` lifecycle monitor is unchanged.
- `HandlerFactory` remains owned locally in `main/src/api/config/traits/feature_registry_ext.rs`
  (dropped upstream with no replacement; same shape, no behavior change).

### Fixed
- `main/src/api/runtime/types/runtime_builder.rs`'s `http_route`/`grpc_route` registration
  path now constructs the wire-level `HandlerRegistry` directly (`InProcessHandlerRegistry<
  HttpRequest, HttpResponse>` / `<GrpcBytes, GrpcBytes>`) instead of going through
  `HandlerComposer::create_registry`, which upstream changed to a type-witness pattern (`H:
  Handler`, not `Req, Resp` directly) that no longer fits a route builder generic only over
  the application-level `Req`/`Resp` types.

### Known Issues
- `edge-security-runtime*` is pinned to `v0.3.7` to match `edge-runtime@v0.3.17`'s own pin
  (rather than the newer `v0.3.22`) — two differently-keyed dependencies on the same
  underlying git repo at different tags do not unify in Cargo's dependency graph, so this
  crate's direct pin must track whatever the transitive `edge-runtime` dependency uses.
- `arch audit --rs` reports a pre-existing structural backlog inherited from this crate's
  extraction out of the `edge` monorepo (weak/missing test assertions, `saf/mod.rs` re-export
  layout, explicit `[[test]]` Cargo.toml entries, mock usage in a few `*_int_test.rs` files)
  — out of scope for this migration and tracked separately.
