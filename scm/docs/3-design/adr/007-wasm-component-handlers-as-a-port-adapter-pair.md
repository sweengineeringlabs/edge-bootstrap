# ADR-007: Wasm Component Handlers Are a Port + Adapter Pair, Bridged Into the Existing `Handler` Boundary

**Audience**: Developers, architects
**WHAT**: A new `swe-edge-bootstrap-wasm` port crate declares a `ComponentEngine`/`ComponentValidator` trait contract and DTOs (manifest, load/invoke requests, structured errors) with zero implementation; a concrete `wasmtime`-backed adapter lives inside the existing `scm/main/adapter` crate under `core/wasm/`, gated behind a new `wasm` Cargo feature; a `WasmHandler` wrapper adapts a loaded component to the already-existing `edge_application::Handler` trait so `DefaultHttpJob`/`DefaultGrpcJob`/`RuntimeBuilder` need no changes
**WHY**: ADR-006 decided TypeScript handlers compile to Wasm components hosted in-process, but left open how that fits this repo's port/adapter split — every other capability here (`intrusion`, `metrics`, `health`, `runtime`, …) follows a trait-contract-only port plus a concrete adapter, and ADR-006 itself already requires components to "adapt to the existing handler and HTTP ingress dispatch boundary," not create a parallel one
**HOW**: Defines the new port's trait/DTO shape, why the adapter stays inside the single existing adapter crate rather than a dedicated one, why the engine is reached through a `Handler`-implementing wrapper instead of a new dispatch path, and the feature-flag/SAF conventions it must follow to match every other optional capability in this crate

---

**Status**: Proposed
**Date**: 2026-08-08
**Deciders**: Core team

## Context

ADR-006 selected compiling constrained TypeScript handlers to WebAssembly Component Model artifacts, hosted by an embedded Wasm engine inside the `edge-bootstrap` process. It assigns `edge-bootstrap` "the embedded Wasm engine, component loading, the Wasm-to-existing-handler adapter, route registration, host capabilities, resource enforcement, caching/pooling, health, tracing, and framework error mapping" — but it does not decide *where in this crate's existing structure* that lands, only that it must land somewhere in `edge-bootstrap`.

This repo already has an established, consistently-applied pattern for every other optional capability (`intrusion`, `security`, `scheduler`, `message-broker`, `observability`): a dedicated port crate under `scm/main/port/*` declaring trait contracts and DTOs with zero implementation, a concrete implementation living inside the single `scm/main/adapter` crate (`swe-edge-bootstrap` — this repo does not split one adapter crate per port, unlike some of its own upstream dependencies), and the whole capability gated behind a Cargo feature so consumers who don't need it pay nothing for it. `RuntimeBuilder` composes each capability at `serve()` time from config or explicit builder calls (`.with_scheduler()`, `.with_message_broker()`, `.http_bearer_auth()`).

Separately, `DefaultHttpJob`/`DefaultGrpcJob` already own route registration, dispatch, and health aggregation for anything implementing `edge_application::Handler` — the same trait every example and test fixture in this repo already implements. ADR-006's own text is explicit that Wasm components should be *adapted into* that boundary, not given a parallel one: "`edge-bootstrap` gains a Wasm component handler adapter alongside, not in place of, local Rust handler registration."

Neither of these facts individually settles the question of trait/crate layout; this ADR makes that decision explicit before any implementation starts, following the same practice ADR-004 used to operationalize a higher-level decision into this repo's concrete layout.

## Decision Drivers

- **Match the established port/adapter split.** Every other optional capability in this crate separates a zero-implementation trait contract from its concrete implementation. A Wasm engine is exactly the kind of swappable, heavy, optional dependency this split exists for — introducing an exception here would be inconsistent without a reason specific to Wasm that doesn't actually exist.
- **Don't build a second dispatch path.** ADR-006 requires component exports to adapt into the existing handler/ingress boundary. `DefaultHttpJob`/`DefaultGrpcJob`'s registry, routing, and health-check logic already do everything a Wasm-backed route needs; duplicating that for Wasm specifically would be pure waste and a second thing to keep in sync.
- **Keep `wasmtime` out of every consumer who doesn't need it.** The engine and its dependencies are large and only relevant to TypeScript-handler users. It must be feature-gated exactly like `intrusion`/`security`/`scheduler`/`message-broker` already are, not a default-on dependency.
- **Preserve ADR-006's capability-denial-by-default posture at the trait level, not just the engine level.** The manifest and capability-validation contract belongs in the port (so it's testable and swappable independent of `wasmtime`), not buried inside one engine implementation.

## Considered Options

### 1. New `swe-edge-bootstrap-wasm` port crate; adapter inside the existing single adapter crate, feature-gated; bridged via a `Handler`-implementing wrapper (chosen)

A new port crate under `scm/main/port/wasm/` declares:

- `ComponentEngine` trait: `load(ComponentLoadRequest) -> Result<ComponentHandle, ComponentError>`, `invoke(ComponentInvokeRequest) -> Result<ComponentInvokeResponse, ComponentError>` — request/response DTOs throughout, matching every other trait method in this org's convention (`IdRequest`/`PatternRequest`-style), not bare arguments.
- `ComponentValidator` trait: validates a `ComponentManifest` (identity, route intent, resource limits, capabilities, contract version, artifact provenance) and the component's declared imports/exports against the versioned `swe:edge-handler` WIT contract before a route is ever registered.
- `ComponentError`: structured variants for traps, invalid output, resource exhaustion, cancellation, and unavailable capabilities — mapped to deterministic framework errors per ADR-006's acceptance criteria.

The concrete implementation (`WasmtimeComponentEngine`, named for the technology it wraps — matching `TokioListener`/`RustlsAcceptor`/`PemAcceptorProvider`'s convention, not a generic `*Manager`) lives inside the existing `scm/main/adapter` crate under a new `core/wasm/` module, gated behind a new `wasm` Cargo feature (optional `wasmtime`/`wasmtime-wasi` dependency). It gets its own `saf/` factory and `WASM_ENGINE_SVC`/`WASM_ENGINE_SVC_FACTORY` constants, matching `TLS_SVC`, `SECURITY_CONTEXT_SVC`, and every other port's SAF convention.

A `WasmHandler` struct implements the already-existing `edge_application::Handler` trait, delegating `execute()` to `ComponentEngine::invoke()`. `RuntimeBuilder` gains one new builder method (e.g. `.wasm_route(manifest, component_bytes)`, mirroring `.with_scheduler()`'s shape) that constructs a `WasmHandler` and registers it exactly like any native Rust handler. `DefaultHttpJob`, `DefaultGrpcJob`, `HttpIngressJob`, `GrpcIngressJob`, and Pipeline mediation require **zero changes**.

**Pros:**

- Consistent with every other capability's existing structure — no special case for a reviewer or new contributor to learn.
- The port (manifest shape, validation rules, error taxonomy) is testable and mockable without ever linking `wasmtime`.
- Dispatch, routing, health aggregation, and Pipeline mediation are inherited for free; a Wasm-backed route behaves identically to a Rust one from the composition root's perspective.
- `wasmtime` stays fully opt-in; a consumer who never calls `.wasm_route()` never compiles it.

**Cons:**

- One extra crate to version and publish alongside the other eleven port crates.
- The `Handler` trait's `execute()` signature was designed around native Rust call semantics; bridging async cancellation/deadlines/fuel-based CPU limits through it needs care so `ComponentError`'s richer taxonomy doesn't get flattened into `HandlerError` prematurely. This is an implementation concern for the eventual host work, not a reason to change the structural decision here.

### 2. Put the `ComponentEngine` trait and its `wasmtime` implementation entirely inside the adapter crate, no separate port crate

**Pros:**

- One fewer crate; faster to stand up initially.

**Cons:**

- Breaks the established pattern for no Wasm-specific reason — every other optional capability in this crate has a port, and reviewers would reasonably ask why this one doesn't.
- Makes the manifest/capability-validation contract untestable independent of `wasmtime` being linked in, undermining ADR-006's own emphasis on validating manifests and capabilities before registration.
- Forecloses swapping the engine (e.g. a future non-`wasmtime` Component Model runtime) without an API-breaking change, since callers would depend on the concrete adapter type directly rather than a trait.

### 3. Give Wasm components their own parallel ingress/dispatch path (a `WasmIngressJob` separate from `DefaultHttpJob`/`DefaultGrpcJob`)

**Pros:**

- Total isolation from the native Rust dispatch path; nothing about `Handler`'s existing shape constrains the Wasm boundary's design.

**Cons:**

- Directly contradicts ADR-006's explicit requirement that components adapt into the *existing* dispatch boundary, not get a new one.
- Duplicates routing, registry, and health-aggregation logic `DefaultHttpJob`/`DefaultGrpcJob` already provide, with no corresponding benefit — the two paths would need to be kept behaviorally consistent by hand indefinitely.
- Every existing HTTP/gRPC middleware (load monitor, intrusion guard, reflection) wraps `HttpIngress`/`GrpcIngress` generically; a second dispatch path would need every one of those re-verified against it separately.

### 4. Fold the `ComponentEngine` port into the existing `swe-edge-bootstrap-runtime` port crate instead of a new dedicated crate

**Pros:**

- Avoids adding a twelfth port crate; `port/runtime` already aggregates several runtime-level contracts (config, health, error, manager).

**Cons:**

- `port/runtime` is already the largest port crate by contract count; folding in a manifest type, two traits, and a structured error taxonomy this size blurs its existing scope rather than extending it naturally.
- Every other substantial capability (`intrusion`, `metrics`, `json`, `monitor`) got its own dedicated port crate rather than being absorbed into `runtime` — doing otherwise here needs a Wasm-specific reason that doesn't exist.

## Decision

**Option 1 — a dedicated `swe-edge-bootstrap-wasm` port crate, with the concrete engine inside the existing adapter crate behind a `wasm` feature, bridged into dispatch via a `Handler`-implementing `WasmHandler`** — because it is the only option that is simultaneously consistent with this repo's established port/adapter convention *and* satisfies ADR-006's explicit requirement that components adapt into the existing dispatch boundary rather than acquire a new one.

## Trade-offs Accepted

- **A twelfth port crate.** Consistent with the existing pattern's own logic (one port per substantial, independently-testable capability), but it is a real crate to maintain, version, and keep zero-implementation.
- **`Handler::execute()`'s native-call shape must absorb Wasm-specific concerns (fuel/CPU limits, deadlines, cancellation, trap mapping) at the `WasmHandler` boundary.** This is deferred implementation detail for the eventual host work (ADR-006's Delivery Sequence step 4), not resolved by this ADR — only the fact that it happens at that specific boundary, not a new one, is decided here.
- **Nothing here can start until `js-runtime#52`'s guest-ABI scope lands.** This ADR fixes the structural target so that work isn't blocked re-litigating crate layout once the WIT contract and compiler output are ready — but it does not itself unblock or accelerate that dependency.

## Consequences

- `edge-bootstrap` gains a new port crate (`scm/main/port/wasm/`, package `swe-edge-bootstrap-wasm`) declaring `ComponentEngine`, `ComponentValidator`, `ComponentManifest`, and `ComponentError` with zero implementation, registered in the workspace alongside the existing eleven port crates.
- The adapter crate (`scm/main/adapter`) gains a new `core/wasm/` module and a `wasm` Cargo feature gating an optional `wasmtime`/`wasmtime-wasi` dependency, following the exact shape of `intrusion`/`security`/`scheduler`/`message-broker`.
- `RuntimeBuilder` gains one new builder method for registering Wasm-backed routes; no other public API changes.
- `DefaultHttpJob`, `DefaultGrpcJob`, `HttpIngressJob`, `GrpcIngressJob`, the `HandlerRegistry`/`Pipeline` mediation they use, and every existing HTTP/gRPC middleware require zero changes — a Wasm-backed route is indistinguishable from a native Rust one at the dispatch layer.
- This ADR should be revisited, not silently reinterpreted, if a future decision needs the Wasm boundary to bypass `Handler` entirely (e.g. for true streaming component invocation that `Handler::execute()`'s shape cannot express) — see ADR-006's own Delivery Sequence step 8 (streaming expansion) as the likely trigger point for that reassessment.
