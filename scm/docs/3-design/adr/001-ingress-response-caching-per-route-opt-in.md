# ADR-001: Ingress Response Caching — Per-Route Opt-In via `CacheAsideHandler`

**Audience**: Developers, architects
**WHAT**: Decision record for how `edge-bootstrap` supports ingress-side (inbound request/response) caching: new `RuntimeBuilder` convenience methods that wrap a registered route's `Handler` in `edge_dispatch`'s existing `CacheAsideHandler`/`CacheAsideMergingHandler`, opt-in per route — not an automatic, whole-service decorator that caches every request
**WHY**: `edge_dispatch::Cache`/`CacheAsideHandler`/`MemoryCache` are already re-exported from this crate (`main/adapter/src/saf/mod.rs`) but never constructed anywhere — consumers who want ingress caching today get no ergonomic path to it. A decision is needed because two materially different integration shapes were considered (per-route vs. whole-service), with different safety properties
**HOW**: Defines the new builder methods, the default backing store, and why the caller supplies the cache key and (implicitly, by which routes they opt in) the idempotency guarantee, rather than this crate inferring either automatically

---

**Status**: Accepted
**Date**: 2026-07-30
**Deciders**: Core team

## Context

`edge-bootstrap` already re-exports `edge_dispatch::{Cache, CacheAsideHandler, CacheAsideResponse, MemoryCache}` from `main/adapter/src/saf/mod.rs`, but nothing in this repo constructs or uses them — they're dead re-exports. Separately, while reviewing this crate's *egress*-side HTTP caching (`swe-edge-egress-http`'s `build_default_egress` unconditionally wires in an RFC 7234 response cache with no opt-out — see [edge-transport-http-egress#24](https://github.com/sweengineeringlabs/edge-transport-http-egress/issues/24)), the same question came up for the *ingress* side: should `edge-bootstrap` itself support caching inbound responses, and if so, how.

`edge_dispatch::CacheAsideHandler<H, C, KeyFn>` wraps any `Handler` (the same trait `HttpHandlerAdapter`/`GrpcHandlerAdapter` wrap when a consumer calls `.http_route()`/`.grpc_route()`) — checking `Cache::get` before calling the inner handler, writing the result to `Cache::set` on a miss. It operates at the **per-route `Handler` level**, not at the whole-service `HttpIngress`/`GrpcIngress` level where `HttpLoadMonitor`/`HttpIntrusionGuard` (ADR-precedent decorators in this repo) sit. Critically, it has **no built-in HTTP-method awareness** — nothing stops a caller from wrapping a `POST` handler and silently caching a mutating operation's response.

## Decision Drivers

- **Reuse over reinvention.** `Cache`, `CacheAsideHandler`, `CacheAsideMergingHandler`, and `MemoryCache` already exist, are already a dependency (`edge_dispatch`/`edge-dispatcher`, unconditional, no new Cargo feature needed), and are already re-exported. The gap is ergonomics and documentation, not missing primitives.
- **Must not blanket-cache non-idempotent operations.** `CacheAsideHandler` has zero method-awareness — whichever integration point is chosen must not make it easy to accidentally cache a `POST`/`PUT`/`DELETE` response.
- **Opt-in/opt-out, not an invisible default.** This is the same principle the egress-cache RFC is asking the egress crate to adopt — caching that activates itself with no visible toggle is the thing being pushed back on there; this repo's own ingress design should not repeat it.
- **No universal "safe to cache" signal for gRPC.** Unlike HTTP's GET/HEAD vs. POST/PUT/DELETE distinction, gRPC method names carry no such convention — a whole-service decorator that auto-detects "cacheable" traffic has an answer for HTTP and none for gRPC.

## Considered Options

### 1. Per-route opt-in via new `RuntimeBuilder` convenience methods (chosen)

Add `RuntimeBuilder::http_route_cached(handler, cache, key_fn, ttl)` (and a gRPC equivalent), thin wrappers that construct `CacheAsideHandler::new(handler, cache, key_fn, ttl)` and register the result exactly like `.http_route()` does today. `MemoryCache::new()` is offered as a zero-config default; consumers may supply their own `Cache` impl (Redis, etc.) since `Cache<K, V>` is technology-neutral.

**Pros:**
- Safety by construction: a consumer can only cache a route they explicitly registered through the `_cached` variant — there's no code path where a plain `.http_route()` call gets caching applied to it silently.
- Directly reuses `edge_dispatch`'s existing, tested primitives — no new caching logic to write or verify in this repo.
- Naturally sidesteps the gRPC "no safe-to-cache signal" problem: the consumer decides per-route, the same as HTTP.
- Matches this repo's own precedent for consumer-driven config (`.http_route_with()` alongside `.http_route()` — offering a more explicit variant next to the convenient default is already this crate's idiom).

**Cons:**
- No caching benefit for routes a consumer doesn't explicitly opt in — if most routes in a service are cacheable GETs, this means repeating `_cached` at every call site rather than one whole-service switch.
- Consumer must supply a `key_fn` themselves; no automatic request-based key derivation (method+path+query) ships with this — see Trade-offs.

### 2. Whole-service decorator wrapping `HttpIngress`/`GrpcIngress`

A new `HttpCacheGuard`/`GrpcCacheGuard`, mirroring `HttpLoadMonitor`/`HttpIntrusionGuard`'s exact shape (wrap `Arc<dyn HttpIngress>`, act before delegating), automatically caching GET/HEAD `InboundRequest`s keyed by method+URL+relevant headers.

**Pros:**
- Zero per-route wiring — a consumer with many cacheable GET routes gets caching for all of them by turning on one `RuntimeBuilder` flag.
- Consistent with the load-monitor/intrusion-guard precedent already established in this repo for cross-cutting, whole-service concerns.

**Cons:**
- Requires building new GET/HEAD-detection logic ourselves — `edge_dispatch` provides no such gating, so this repo would own that safety-critical logic, not reuse something already tested.
- No answer for gRPC without inventing a convention this org hasn't established (e.g. a method-name allowlist, or requiring `edge-bootstrap` to guess "read" vs. "write" gRPC methods).
- A blanket, automatic mechanism is exactly the "invisible default" shape the egress-cache RFC is pushing back on — building a second instance of that pattern here, even a better-gated one, cuts against the opt-in principle driving this decision.

### 3. Do nothing — leave the existing dead re-exports as-is

Consumers who want ingress caching already have access to `Cache`/`CacheAsideHandler`/`MemoryCache` via the existing re-exports; they can construct and wire a `CacheAsideHandler` themselves before calling `.http_route_with()`.

**Pros:**
- Zero new code, zero new surface to maintain or document.

**Cons:**
- The re-exports are undiscovered and undocumented today — in practice "already possible" has meant "never used," which is the actual problem being solved here, not a reason to leave it unsolved.
- A consumer doing this manually has to correctly adapt `CacheAsideHandler`'s response type (`CacheAsideResponse<H::Response>`) into whatever `.http_route_with()` expects — friction this repo can remove once, instead of every consumer solving it independently.

## Decision

**Option 1 — per-route opt-in via new `RuntimeBuilder` convenience methods**, because:

1. **It's the only option that fully satisfies "opt-in/opt-out."** Option 2 still auto-applies caching to every route matching a heuristic (GET/HEAD); option 1 requires an explicit, per-route decision, which is the strongest form of opt-in available.
2. **It reuses tested primitives instead of building new safety-critical logic.** `CacheAsideHandler`'s cache-aside behavior is `edge_dispatch`'s to test and maintain; this repo only needs to verify the thin wrapper compiles and dispatches correctly.
3. **It has a real, symmetric answer for both HTTP and gRPC**, unlike option 2, without inventing an org-wide convention for "what's a safe-to-cache gRPC method" that doesn't otherwise exist.

## Trade-offs Accepted

- **No automatic cache-key derivation.** The consumer supplies `key_fn: Fn(&Req) -> K` themselves. A future convenience (e.g. a default `key_fn` for `HttpRequest` deriving from method+path+query) is not precluded by this design but isn't built now — most real services need more control over cache-key shape (e.g. excluding a tracking query param) than an automatic derivation could safely guess.
- **No org-wide default cache backend beyond `MemoryCache`.** Unbounded growth and cross-process consistency (a multi-instance deployment won't share cache state with an in-memory store) are the consumer's responsibility to consider — `Cache<K, V>`'s technology-neutral design means swapping in a shared backend (Redis, etc.) is possible, just not shipped as a default here.
- **Whole-service, zero-config ingress caching (option 2) is not built.** If a real need for that shape shows up later, it should get its own ADR — this one doesn't foreclose it, but doesn't build it speculatively either.

## Consequences

- `RuntimeBuilder::http_route_cached`/`grpc_route_cached` (or equivalent naming decided at implementation time) become new public API surface, additive to the existing `.http_route()`/`.grpc_route()` methods — no breaking change to existing consumers.
- `main/adapter/src/saf/mod.rs`'s `Cache`/`CacheAsideHandler`/`CacheAsideResponse`/`MemoryCache` re-exports go from dead to load-bearing.
- Documentation (`scm/README.md`'s "Key capabilities" list, following this repo's existing convention) should gain an entry once implemented, the same way `edge-intrusion`'s wiring did.
- This ADR should be revisited, not silently expanded, if a whole-service caching need resurfaces — see option 2's rejection reasoning above for what would need to change.
