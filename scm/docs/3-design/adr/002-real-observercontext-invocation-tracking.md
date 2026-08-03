# ADR-002: Real `ObserverContext` for Invocation Tracking — Bridge to `swe-observability-tracing`, Wired at the `RuntimeBuilder` Composition Root

**Audience**: Developers, architects
**WHAT**: Decision record for how `edge-bootstrap` answers "which nodes actually handled this request, and did they succeed" — a `TracingBridgeObserverContext` (backed by `swe-observability-tracing`'s `TracerProvider`) implementing `edge_application_observer::ObserverContext`, wired into `RuntimeBuilder::serve()` in place of the framework's noop default, so every `Handler::execute()` call — infra's own and any consumer's — opens real, exportable spans through `HandlerContext.observer`
**WHY**: Ad hoc `tracing::debug!`/`info!` calls added at each pipeline node (`HttpLoadMonitor`, `HttpIntrusionGuard`, `HttpHandlerRegistryDispatcher`) answer "did a request reach this node" one log line at a time, but don't give consumers a shared, structured mechanism to plug their own handlers into the same tracking — and `Handler`'s own contract already carries exactly that seam (`HandlerContext.observer`), unused
**HOW**: Defines the bridge's scope (spans only, not logs/metrics), why it lives privately inside `edge-bootstrap` rather than as a new public port, and why the backend is `swe-observability-tracing` rather than a bespoke implementation

---

**Status**: Accepted
**Date**: 2026-07-30
**Deciders**: Core team

## Context

Tracing instrumentation added across `edge-bootstrap`'s own pipeline (`HttpLoadMonitor`, `GrpcLoadMonitor`, `HttpIntrusionGuard`, `GrpcIntrusionGuard`) and, upstream, `edge-transport-http-ingress`'s `HttpHandlerRegistryDispatcher`, answers "which nodes did this request touch" through discrete `tracing::debug!`/`info!` calls at each decision point. That closed the immediate visibility gap, but left a sharper question open: there was still no structured, shared mechanism a *consumer's own* `Handler` could plug into to get the same tracking — every consumer would have to hand-roll their own `#[tracing::instrument]`, independently, with no shared span/trace shape.

`edge_application_handler::HandlerContext` — passed by reference into every single `Handler::execute()` call, both by `edge-bootstrap`'s own infra (`HttpHandlerRegistryDispatcher`) and by any consumer's own handler — already carries an `observer: &dyn ObserverContext` field bundling `tracer()`/`drain()`/`metrics()`. This is the correct extension point by construction: it's already threaded through the one call every handler, ours or a consumer's, must go through. The gap is that it's wired to `StdObserveFactory::noop_observer_context()` everywhere in this ecosystem — confirmed in both this repo's own `hello_edge.rs` example and `edge-transport-http-ingress`'s `registry_dispatcher.rs`. No real (non-noop) implementation of `ObserverContext`/`HandlerTracer`/`Span` existed anywhere in the `edge-application` ecosystem, including its own Phase-1 `-api` split repos (`edge-application-observer-api` — port-only, no `-core` implementation yet, per `edge-application#144`).

Separately, `sweengineeringlabs/observability` (renamed upstream from its original repo name; already a dependency of both `edge-bootstrap` and `edge-transport-http-ingress` for metrics/log-context) turned out to carry exactly the missing piece: a real, tested, multi-backend `TracerProvider` trait (`export_span`/`flush`/`recent_spans`, backends for in-memory/file/Jaeger/OTel), reachable at the pinned `v0.2.9` tag via free functions (`create_default_tracer_arc()`, etc.) re-exported at the crate root. It predates that repo's later `TracerSvcFactory` refactor, and its own `Handler`/`HandlerContext` usage targets the *older* `edge-domain-handler` contract, not `edge-application-handler` — so it isn't a drop-in `ObserverContext`, but its `TracerProvider` trait is a clean, backend-agnostic delegate target.

## Decision Drivers

- **Reuse the contract that already exists.** `HandlerContext.observer` is already the seam; the fix is filling it in, not inventing a new one.
- **Consumers must get this for free.** A consumer writing their own `Handler` should reach the same real tracker infra uses through `req.ctx.observer`, without importing anything `edge-bootstrap`-internal.
- **Reuse a real backend over a bespoke one.** `swe-observability-tracing`'s `TracerProvider` is already tested (811 tests across the workspace) and already partially adopted by this dependency chain (`swe_observability_context`'s trace-ID propagation is already live in `registry_dispatcher.rs`) — building a from-scratch span backend would duplicate infrastructure this org already operates.
- **Scope discipline.** The user's ask was specifically a method-invocation tracker (spans) — `drain()`/`metrics()` (logs/metric registries) are a separate, not-yet-requested concern and should not be silently built out alongside this.

## Considered Options

### 1. Bridge crate implementing `edge_application_observer::ObserverContext`, delegating to `TracerProvider`, private inside `edge-bootstrap` (chosen)

`TracingBridgeObserverContext`/`TracingBridgeHandlerTracer`/`TracingBridgeSpan` implement the canonical `edge_application_observer` trait set; `Span::finish()` flattens `handler_id`/`operation`/`duration_us`/annotations into one JSON object and hands it to an injected `Arc<dyn TracerProvider>`. `RuntimeBuilder::serve()` constructs one (`default_observer_context()`, backed by the in-memory default tracer) and injects it via a new `HttpHandlerRegistryDispatcher::with_observer_context()` builder method upstream in `edge-transport-http-ingress`, mirroring that struct's existing `with_metrics`/`with_ingress_lb` pattern.

**Pros:**
- `domain-handler`'s own blanket impl (`impl<T: obs::ObserverContext + ?Sized> ObserverContext for T`) means implementing the canonical trait is sufficient — no separate local-mirror implementation needed.
- Consumers get it automatically through `HandlerContext.observer` — zero per-handler wiring, confirmed live in `bootstrap-e2e`'s `EchoHandler`.
- Kept private (`pub(crate)`): the user confirmed this is fine, since consumers only ever interact with it through the already-public `HandlerContext.observer` field, never by importing the bridge type directly.

**Cons:**
- Only the in-memory default backend is wired as the built-in default; file/Jaeger/OTel backends exist in `swe-observability-tracing` and are reachable the same way, but aren't exposed as `RuntimeBuilder` options yet.
- `drain()`/`metrics()` pass through to `StdObserveFactory`'s noop primitives — deliberately out of scope (see Trade-offs).

### 2. Build a bespoke `tracing`-crate-backed implementation from scratch

Implement `ObserverContext`/`HandlerTracer`/`Span` directly against the `tracing` crate's own `Span`/event API, with no dependency on `swe-observability-tracing`.

**Pros:**
- No new git dependency; `tracing` is already a dependency of `edge-bootstrap`.

**Cons:**
- Duplicates infrastructure (`swe-observability-tracing`) this org already builds, tests, and operates, including backend integrations (Jaeger/OTel) this repo would otherwise have to build itself.
- Loses the `recent_spans()`/multi-backend story `TracerProvider` already provides.

### 3. Propose the bridge upstream, in `edge-application-observer` (or its `-api` split) itself

Contribute a real `ObserverContext` implementation directly into the crate that defines the noop, rather than building it downstream in `edge-bootstrap`.

**Pros:**
- Every consumer of `edge-application-handler`, not just `edge-bootstrap`, benefits.

**Cons:**
- `edge-application`'s own Phase-1 `-api` split (issue #144) explicitly has not started Phase 2 (`-core` implementations) — introducing one now, upstream, is a larger, cross-team decision this ADR doesn't have standing to make unilaterally.
- Blocks on a release cycle this repo doesn't control; the immediate need (this repo's own observability gap) doesn't require waiting on that.

## Decision

**Option 1 — private bridge crate inside `edge-bootstrap`, delegating to `swe-observability-tracing`'s `TracerProvider`**, because:

1. It closes the actual gap (`HandlerContext.observer` wired to noop) using the exact seam the `Handler` contract already provides, with no upstream dependency on a release this repo doesn't control.
2. It reuses tested, already-partially-adopted infrastructure instead of building a parallel bespoke tracer.
3. Nothing forecloses option 3 later — if this bridge proves broadly useful, extracting it upstream is a natural follow-up, not a rewrite.

## Trade-offs Accepted

- **`drain()`/`metrics()` are noop passthroughs, not real implementations.** This ADR is scoped to invocation tracing (spans) only, per the originating ask. A real `LogDrain`/`MetricRegistry` bridge is a legitimate future extension, not built here.
- **Only the in-memory default `TracerProvider` backend is wired as `RuntimeBuilder`'s default.** File/Jaeger/OTel backends are reachable in `swe-observability-tracing` but not yet exposed as `RuntimeBuilder` configuration — a consumer needing one today would need to extend `default_observer_context()` directly.
- **`HttpHandlerRegistryDispatcher::with_observer_context()` required an upstream change to `edge-transport-http-ingress`** (v0.8.1 → v0.8.2), and that change's own dependency on `swe-observability-context`/`-metrics` needed a compound version bump in `edge-runtime` (its `http`/`http-adapter` port+adapter crates still pinned the pre-split `edge-transport-http-ingress` v0.8.0) to avoid a duplicate-crate-in-dependency-graph conflict — the same class of issue fixed under `edge-runtime#41`.
- **gRPC dispatch is not covered.** `GrpcHandlerRegistryDispatcher` (in `edge-ingress-grpc`) was not given the same `with_observer_context()` treatment — this ADR's scope was the HTTP path where the original gap was found; gRPC parity is a follow-up, not assumed done.

## Consequences

- `edge-transport-http-ingress` v0.8.2: `HttpHandlerRegistryDispatcher::with_observer_context()`, defaulting to the existing noop when unset (backward compatible).
- `edge-runtime`: `http`/`http-adapter` crates' `edge-transport-http-ingress` pin bumped v0.8.0 → v0.8.2.
- `edge-bootstrap`: new private `core/observability/` module (`TracingBridgeObserverContext`/`HandlerTracer`/`Span`), gated behind the existing `observability` feature; `RuntimeBuilder::serve()` injects it for the HTTP path.
- `examples/bootstrap-e2e`'s `EchoHandler` opens its own span through `req.ctx.observer` — the reference demonstration that a consumer handler reaches the same tracker with no framework-internal import.
- This ADR should be revisited, not silently expanded, if gRPC parity, non-default backends, or a real `drain()`/`metrics()` bridge are needed — see the Trade-offs above for what would need to change.
