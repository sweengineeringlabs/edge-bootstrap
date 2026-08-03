# ADR-004: Backend-Pool Load Balancing Moves to the `RuntimeBuilder` Composition Root, Out of `edge-transport-http-egress`

**Audience**: Developers, architects
**WHAT**: `edge-bootstrap` now owns client-side backend-pool load balancing for outbound HTTP calls — `LoadBalancedHttpEgress` (wrapping `swe-edge-loadbalancer`'s egress subdomain), a `[services.<name>.loadbalancer]` `RuntimeConfig` section, and a `ServiceRegistry` extended to hold one independently-resolved client per named target service — instead of `edge-transport-http-egress`'s `transport` crate doing it internally via its (now-removed) `loadbalancer` feature
**WHY**: Backend selection is a routing/topology decision — *which* backend a call goes to — not a "how do I execute one HTTP call" decision, which is what every other middleware in `transport`'s stack (auth/retry/rate/breaker/cache/cassette) actually does. `transport` mutating a request's destination conflated the two; `edge-bootstrap`, the process-level composition root, is where topology should be resolved instead
**HOW**: Defines why `edge-bootstrap` (not `ingress`'s ALB/Envoy/Nginx-adapter pattern, not `transport` itself) is the right home, what capability gap that required closing first (`ServiceRegistry` held one client per process, not per target service), and how outcome reporting (the equivalent of `breaker`'s removed trip/recovery-driven eviction) was preserved

---

**Status**: Accepted
**Date**: 2026-08-01
**Deciders**: Core team

## Context

`edge-transport-http-egress`'s `transport` crate read a static `[loadbalancer]` TOML section (backend URLs/weights/strategy), built a `BackendPoolInstance` internally, and rewrote every outgoing request's URL to a selected backend via a `reqwest_middleware::Middleware` — wired identically to `transport`'s other optional middleware (`retry`/`rate`/`breaker`/`cache`/`cassette`) and, as of the commit introducing it, a *default-on* feature.

That's architecturally different from what those other five middlewares do. Each of them decorates a request already addressed to one resolved destination (retries it, rate-limits it, circuit-breaks it, caches it, records it). `loadbalancer` was the only one that decided *where the request goes* — it selected among multiple possible destinations before the request was sent at all. Bundling that decision inside a low-level HTTP transport library meant every consumer of `transport` inherited one hardcoded backend-topology policy, with no way for a caller upstream (a gateway, a service mesh, plain DNS-based discovery) to make that call instead.

`edge-transport-http-ingress` already draws this line correctly on the inbound side: `spi/loadbalancer/{alb,envoy,nginx,noop}.rs` consult an *external*, already-existing load balancer (AWS ALB, Envoy, Nginx) for a routing hint before dispatch, rather than implementing routing logic itself. `egress` had no equivalent — see `edge-transport-http-egress#25`.

`edge-bootstrap` was evaluated as the new home ahead of this ADR (conversation record, not a separate document) and found to be the right *direction* but not a drop-in fit:

- It's the process-level composition root — `RuntimeBuilder` already wires ingress + egress + lifecycle into one deployable service, and already has the config-driven `OptionalSection`/`FeatureState` pattern (`FeatureRegistryExt`) `transport` used for the same purpose.
- `RuntimeBuilder::egress_http(client: Arc<dyn HttpEgress>)` and `RuntimeBuilder::build_registry()` were already the exact seam needed: hand the runtime (and, through it, any handler holding a `ServiceRegistry`) a client already pointed at a resolved destination.
- The gap: `ServiceRegistry` held exactly one `Arc<dyn HttpEgress>` for the entire process — no per-target-service concept. Real backend-pool ownership (`user-service`'s replicas load-balanced independently from `billing-service`'s) needed that extended; it wasn't a matter of relocating the `[loadbalancer]` TOML section wholesale.
- `breaker`'s optional pool-reporting integration (`report_outcome`/`new_with_pool`) let a circuit trip evict a backend from rotation and a recovery restore it. Removing that from `breaker` without an equivalent hook here would have been a silent capability loss, not a relocation.

Two adjacent, unrelated bugs were found and fixed as prerequisites, not part of this decision: `edge-security-runtime-tls` was `optional` behind the `security` feature in this crate's root `Cargo.toml` yet imported unconditionally in three files (a default-feature build was already broken before this work started); and `swe-edge-egress-http` was pinned to `edge-egress-http.git`, a stale pre-rename remote name for `edge-transport-http-egress.git` (the same class of issue as the `justobserv` → `observability` rename found elsewhere in this dependency graph).

## Decision Drivers

- **Match the shape of the actual decision.** A routing/topology decision belongs where topology is known — the process composition root, not a request-execution library that has no visibility into deployment topology at all.
- **Don't silently drop a capability while relocating it.** `breaker`'s trip → evict / recovery → restore behavior had to have a real equivalent here, tested, before `egress` was allowed to remove its side — see `edge-transport-http-egress#25`'s sequencing requirement.
- **Reuse the existing library, not reinvent it.** `swe-edge-loadbalancer`'s egress subdomain (`BackendPool`/`Strategy`/`Outcome`/health tracking) is the same code `transport`'s middleware already called — this is a relocation of *ownership*, not a rewrite of the selection algorithm.
- **Don't overreach into infra-level load balancing.** `ingress`'s ALB/Envoy/Nginx adapters consult *external* infrastructure; this ADR keeps selection as in-process Rust logic at the composition root. Full symmetry with `ingress`'s pattern (deferring entirely to an external LB/service mesh) is a known, intentional non-goal here — a future ADR's problem if it's ever pursued.

## Considered Options

### 1. `LoadBalancedHttpEgress` wrapping `swe-edge-loadbalancer` directly, `ServiceRegistry` extended per-target-service, built by `RuntimeBuilder::build_registry()` from `RuntimeConfig.services` (chosen)

A new `HttpEgress` implementation (`main/adapter/src/core/egress/load_balanced_http_egress.rs`) holds an inner single-destination `HttpEgress` plus an `Arc<BackendPoolInstance>`. `send`/`send_stream` select a backend, rewrite the request's URL (mirroring `transport`'s own now-removed rewrite logic), delegate to the inner client, and report `Outcome::Success`/`Outcome::Failure` back to the pool automatically based on the result. `report_outcome(&self, backend_id, outcome)` is exposed as a separate public method for a signal not derived from a single call — the direct equivalent of `breaker`'s external trip/recovery reporting. `ServiceRegistry::with_service`/`service`/`service_names` let a registry hold one such client per named target service, distinct from the existing single default client (kept for backward compatibility). `RuntimeConfig.services: BTreeMap<String, ServiceEgressConfig>` (`[services.<name>.loadbalancer]`) reuses `swe_edge_loadbalancer::LoadbalancerConfig` directly as the section shape, rather than inventing a parallel config type.

**Pros:**
- `ServiceRegistry`'s existing single-client behavior and its `build_registry()` construction path are unchanged when `[services]` is empty — fully backward compatible.
- Same underlying selection/health-tracking algorithm as `transport`'s removed middleware; only where it's invoked changed.
- One misconfigured service (e.g. an empty backend list) is skipped with a `tracing::warn!`, not a hard failure for the whole registry — matches this repo's existing "one bad config entry shouldn't take down everything else" posture.

**Cons:**
- `build_registry()` only builds per-service pools when the caller passed an explicit `RuntimeConfig` via `.config(...)` — it does not do the XDG/TOML auto-load `serve()` does when `.config()` was never called. A consumer wanting `[services]` support must call `.config(loaded_config)` before `.build_registry()`, not rely on `serve()`'s own fallback loading.
- Streaming responses (`send_stream`) report `Outcome::Success` on a successful connection only — a mid-stream failure after that point doesn't degrade the backend, since `HttpStreamResponse` doesn't surface a synchronous status the way a buffered `send()` response does.

### 2. Extract `edge-transport-http-egress`'s `scm/loadbalancer` crate as a standalone, publishable dependency, reuse it here instead of building a new wrapper

**Pros:**
- Reuses `egress`'s exact middleware code, including its existing test suite.

**Cons:**
- `scm/loadbalancer` is a `reqwest_middleware::Middleware`, built to plug into a `ClientBuilder` chain — `edge-bootstrap` only ever sees `Arc<dyn HttpEgress>` trait objects, not a `reqwest` client under construction. Reusing it would mean either exposing `reqwest` internals across the `HttpEgress` trait boundary (a bigger, unwanted coupling) or writing an adapter layer nearly as large as `LoadBalancedHttpEgress` itself.
- `scm/loadbalancer` is a path-only workspace member inside the `egress` repo, not published as an independent git-fetchable crate — making it consumable here would require its own extraction work in `egress`, which `edge-transport-http-egress#25` leaves as an open decision, not a settled prerequisite.

### 3. Defer to an external load balancer / service mesh entirely, matching `ingress`'s ALB/Envoy/Nginx adapter pattern

**Pros:**
- Full architectural symmetry with the inbound side; no in-process selection logic to own or test at all.

**Cons:**
- A materially larger scope change — it assumes every deployment has a service mesh or client-side-discovery sidecar in front of outbound calls, which isn't true of every consumer of `edge-bootstrap` today.
- Blocks this work on infrastructure decisions outside this repo's (or even `egress`'s) control, rather than closing the immediate gap `edge-transport-http-egress#25` exists to fix.

## Decision

**Option 1 — `LoadBalancedHttpEgress` + per-service `ServiceRegistry`, built from `RuntimeConfig.services`**, because:

1. It closes the actual gap (backend-pool ownership had nowhere else to live once removed from `transport`) using the seam (`ServiceRegistry`, `build_registry()`) this repo already exposed for exactly this purpose.
2. It reuses `swe-edge-loadbalancer` directly rather than either duplicating its selection/health-tracking logic or taking on a coupling to `egress`'s internal `reqwest_middleware`-shaped crate.
3. Nothing forecloses Option 3 later — if full external-LB deferral is ever wanted, `ServiceRegistry`'s per-service `Arc<dyn HttpEgress>` slots are equally satisfiable by a client backed by a service mesh sidecar instead of `LoadBalancedHttpEgress`, with no trait-level change required.

## Trade-offs Accepted

- **`build_registry()` requires an explicit `.config(...)` call to see `[services]`.** No XDG/TOML auto-load fallback exists in this method, unlike `serve()`. Documented on the method itself; a consumer relying on default XDG loading and wanting per-service pools must load the config themselves and pass it explicitly before calling `build_registry()`.
- **Streaming calls don't get mid-stream outcome tracking.** Only the initial connection's success/failure is reported for `send_stream`; this mirrors a real limitation in what `HttpStreamResponse` exposes synchronously, not an oversight.
- **This does not solve infra-level load balancing.** See Option 3 — treated as a known, intentional non-goal, not deferred work silently dropped.

## Consequences

- `edge-bootstrap`: new `LoadBalancedHttpEgress`/`LoadBalancedHttpEgressError` (`main/adapter/src/core/egress/`), `ServiceEgressConfig` + `RuntimeConfig.services` (`main/port/runtime`), `ServiceRegistry` extended with `with_service`/`service`/`service_names`, `RuntimeBuilder::build_registry()` wired to build per-service pools from config. New direct dependency: `swe-edge-loadbalancer` (`edge-loadbalancer.git`, tag `v0.4.0`) in both the root crate and `main/port/runtime`; new `url` dependency in the root crate for backend-URL rewriting.
- `edge-security-runtime-tls` made a non-optional dependency of the root crate (was incorrectly `optional` behind `security`, unconditionally imported in three files regardless) — an unrelated pre-existing bug fixed as a prerequisite to verifying this work compiles at all under default features.
- `swe-edge-egress-http` repointed from the stale `edge-egress-http.git` remote to the current `edge-transport-http-egress.git`, same tag (`v0.4.5`) — an unrelated pre-existing staleness issue, also fixed as a prerequisite.
- `edge-transport-http-egress#25` (companion issue) may now proceed with removing `loadbalancer` from `transport`/`breaker`, per its own sequencing requirement that this work land first.
- This ADR should be revisited, not silently expanded, if streaming outcome tracking, XDG-auto-load support in `build_registry()`, or a move to Option 3 (external LB deferral) are ever needed — see Trade-offs above for what would have to change.
