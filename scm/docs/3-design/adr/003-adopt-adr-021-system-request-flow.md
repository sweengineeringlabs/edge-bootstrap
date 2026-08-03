# ADR-003: Adopt Upstream ADR-021 (System Request Flow) as Governing Architecture

**Audience**: Developers, architects
**WHAT**: Decision record adopting `edge/docs/3-architecture/adr/ADR-021-system-request-flow.md` (upstream, Status: Accepted, Affects: all workspaces) as the governing description of how a request should travel through the swe-edge stack this repo composes, and recording where the crates this repo actually depends on stand relative to it today
**WHY**: This repo has never referenced ADR-021 since being extracted from the monorepo (`20952dc`); nothing recorded whether the crates it composes (`edge-transport-http-ingress`, `edge-transport-grpc-ingress`, `edge-dispatcher`, `edge-proxy`) still match its layering, and one of its own findings — transport crates dispatch directly to `Handler::execute()` with no `Pipeline`/decorator composition, bypassing the layer ADR-021 assigns to `edge-dispatch` — surfaced only by direct investigation, not by anything this repo's docs said
**HOW:** States what ADR-021 mandates, records the verified current state of every crate this repo composes against that mandate, and makes an explicit adopt/depart decision rather than leaving the question open

---

**Status**: Accepted
**Date**: 2026-07-30
**Deciders**: Core team

## Context

`edge-bootstrap` is the composition root: it wires `edge-transport-http-ingress`, `edge-transport-grpc-ingress`, `edge-proxy`-shaped dispatch, and consumer `Handler`s into one running process via `RuntimeBuilder`. Upstream ADR-021 ("System Request Flow — End-to-End Architecture," Status: Accepted, Affects: all workspaces) is the one document that describes how a request is supposed to move through that whole composition, layer by layer — but it lives in the pre-extraction monorepo, and this repo has carried no reference to it, and no recorded decision to adopt or depart from it, since the extraction (`20952dc`).

ADR-021's layer table:

| Layer | Crate | Responsibility |
|---|---|---|
| Ingress transport | `ingress/http`, `ingress/grpc` | Receive request; verify identity; build `SecurityContext`; call load balancer |
| Proxy | `edge-proxy` | Single entry point `Job::run`; carries `SecurityContext` through |
| Dispatch — routing | `edge-dispatch` | `Router::route` — context-free |
| Dispatch — registry | `edge-dispatch` | `HandlerRegistry::get` — context-free |
| Dispatch — pipeline | `edge-dispatch` | `Pipeline` — ordered `Handler` chain |
| Handler contract | `edge-domain-handler` | `Handler::execute(req, ctx)` — domain boundary |

And its normative Seam Rules include: `SecurityContext` is built once at the ingress boundary and never reconstructed downstream (S1); routing is context-free (S3); context crosses the domain boundary once, as an explicit parameter (S4); no thread-locals/ambient storage (S7).

### Verified current state of every crate this repo composes

Checked directly against each crate's own docs/ADRs/source — not assumed:

- **`edge-dispatcher`** — not diverged. Its own architecture doc still describes exactly the ADR-021 "Dispatch" row (`HandlerRegistry`, `Pipeline`, decorator suite) and explicitly disclaims knowing "who performs the registry lookup at request time" or "what sits in front of this crate at the transport boundary." It is waiting to be composed in, as ADR-021 describes; nothing in it has changed shape.
- **`edge-proxy`** — not diverged. Its own architecture doc still describes exactly the ADR-021 "Proxy" row (`Job`/`Router`/`LifecycleMonitor`) and states outright it has "no transport knowledge of its own (no ingress/egress imports)."
- **`edge-transport-grpc-ingress`** and **`edge-transport-http-ingress`** — diverged from the letter of the layer table, but the substance is narrower than it first looked. Both have zero Cargo dependency on `edge-dispatcher`. **This is not "two competing `HandlerRegistry` implementations," though.** `edge-dispatcher`'s own `HandlerRegistryImpl` is itself just a thin wrapper around `edge_application_handler::InProcessHandlerRegistry` — it doesn't define the `HandlerRegistry` trait (that's owned upstream by `edge-application-handler`) or a distinct storage/lookup implementation. Confirmed directly: `edge-transport-http-ingress`'s divergence commit (`118f628`, 2026-06-13, *"Remove edge-dispatch dep; replace `Dispatch::new_handler_registry` with `InProcessHandlerRegistry`"*) swapped `edge-dispatch`'s wrapper for the same underlying upstream type it wraps, one indirection removed — not a different registry. Judged against ADR-021's actual normative content (Seam Rules S1/S3/S4/S7 — single construction point, context-free lookup, explicit parameter passing, no ambient state), that swap doesn't violate anything; it's a legitimate simplification.

  The real gap is narrower and sharper: `edge-dispatch`'s actual value-add — `Pipeline` (ordered `Handler` chain, `ADR-014` lifecycle events) and its decorator suite (`FallbackHandler`, `CacheAsideHandler`, `TimeoutHandler`, `EventEmittingHandler`) — is simply never invoked by either transport crate. `GrpcHandlerRegistryDispatcher`/`HttpHandlerRegistryDispatcher` call `Handler::execute()` directly after a registry lookup; there is no `Pipeline` stage, no lifecycle-event emission, no decorator composition anywhere in either dispatch path. Each repo's own ADR-004 (port/adapter workspace split, written the same day, independently, for the same reason) never mentions ADR-021, `edge-dispatch`, or `Pipeline` at all — it treats the dispatcher's direct-execute shape as a structural given, not a decision point. The divergence is old, not new: `edge-transport-grpc-ingress`'s direct-execute dispatcher dates to at least `76aea45` (2026-06-07; its already-unused `edge-dispatch` Cargo dependency was only formally dropped seven weeks later, `e29f920`, 2026-07-29); `edge-transport-http-ingress`'s divergence is precisely dated to `118f628` (2026-06-13), and — checked directly, not assumed — the commit right before it (`118f628^`) shows `HttpHandlerRegistryDispatcher::new(Dispatch::new_handler_registry::<...>())` genuinely live in source, confirming that repo really was routed through `edge-dispatch` before that commit. None of this happened inside `edge-bootstrap`'s own repo, and none of it was introduced by this repo's own work.

`edge-bootstrap#17`'s `ObserverContext` wiring (real `TracingBridgeObserverContext`, injected into both `HttpHandlerRegistryDispatcher` and `GrpcHandlerRegistryDispatcher` via `with_observer_context()`) plugs directly into this same per-transport-dispatcher seam. It is consistent with ADR-021's context-propagation rules (built once at the `RuntimeBuilder` composition root, passed explicitly as an `Arc`, never reconstructed downstream — see `002-real-observercontext-invocation-tracking.md`), but it necessarily plugs into two independent dispatchers rather than routing through a shared `Pipeline`, because that is the shape those two upstream crates currently expose.

## Decision Drivers

- **A governing reference must actually govern.** Silence — no record of whether this repo's composed crates match ADR-021 — is itself a defect; this ADR exists to close that specifically, not to re-litigate it later from scratch.
- **Don't conflate "record the gap" with "close the gap."** The missing-`Pipeline` finding is real and worth acting on, but wiring `Pipeline`/decorator composition into two independently-versioned, independently-owned upstream crates' dispatchers is a multi-repo change with its own blast radius (breaking-change version bumps in `edge-transport-http-ingress`/`edge-transport-grpc-ingress`, a rewrite of `RuntimeBuilder`'s composition logic, re-verification of every existing dispatch test) — not something to decide as a side effect of writing this record.
- **The context-propagation principle (S1/S3/S4/S7) is worth affirming now, even before the pipeline gap is closed**, since `edge-bootstrap#17`'s `ObserverContext` work already depends on getting that part right, independent of whether `Pipeline` is composed in.

## Considered Options

### 1. Adopt ADR-021 as-is; treat the missing `Pipeline`/decorator composition as a recorded, open gap — not fixed by this ADR (chosen)

Accept ADR-021's layering as the target. Record, in this document, that `edge-transport-http-ingress`/`edge-transport-grpc-ingress` dispatch directly to `Handler::execute()` without composing `edge-dispatch`'s `Pipeline`/decorator layer — the registry implementations themselves are not meaningfully different (see Context). Do not attempt to wire `Pipeline` in here; require it to be filed and scoped as its own issue before any code changes.

**Pros:**
- Matches what's actually true today without inventing false compliance or silently starting an unscoped migration.
- Keeps this ADR reviewable and small — a record-and-decide document, not a mixed record-and-implement one.
- Leaves the context-propagation rules (S1/S3/S4/S7) affirmed and actionable immediately, independent of the registry question.

**Cons:**
- The known gap persists until a follow-up issue actually closes it; this ADR alone doesn't fix anything.

### 2. Adopt ADR-021 and immediately begin wiring `Pipeline`/decorator composition into both transport dispatchers

**Pros:**
- Closes the gap sooner.

**Cons:**
- Requires breaking-change coordination across three repos this session doesn't have standing to force through as a documentation change; the same class of multi-repo version-skew risk hit repeatedly this session (duplicate-crate-source bugs, stale tag pins) would apply here at a much larger scale.
- No current issue has scoped what composing `Pipeline` means for two already-independently-evolving dispatchers (interceptor chains, health-check protocol, audit sinks, load-balancer hints — features specific to each transport today) or which decorators (if any) each actually needs.

### 3. Do not adopt ADR-021; treat this repo's current per-transport dispatch shape as the intended architecture going forward

**Pros:**
- No further work implied.

**Cons:**
- Contradicts the only "Affects: all workspaces," Accepted architecture document that exists for this concern, with no counter-ADR anywhere recording why it wouldn't apply here.
- Leaves S1/S3/S4/S7's context-propagation discipline unaffirmed, which this repo's own `ObserverContext`/`SecurityContext` wiring already depends on getting right.

## Decision

**Option 1 — adopt ADR-021's layering as governing, record the missing-`Pipeline`-composition gap explicitly (not a registry-duplication problem), and require any work to close it to be scoped as its own separate issue.**

## Trade-offs Accepted

- **The missing `Pipeline`/decorator composition gap is not closed by this ADR.** `HttpHandlerRegistryDispatcher` and `GrpcHandlerRegistryDispatcher` keep dispatching directly to `Handler::execute()`, with no `Pipeline` stage, lifecycle events, or decorators, for now. This is a known, recorded departure from ADR-021's layer table, not a silent one.
- **No timeline is set for closing it.** Doing so requires its own scoped issue(s), touching `edge-transport-http-ingress`, `edge-transport-grpc-ingress`, and this repo's `RuntimeBuilder` composition logic — out of scope here.

## Consequences

- `edge-bootstrap#19` tracked authoring this ADR; closing it should point here.
- Any future work to compose `edge-dispatch`'s `Pipeline`/decorators into either transport dispatcher must reference this ADR and be filed as its own issue — not bundled into unrelated work.
- `edge-bootstrap#17` (`ObserverContext` → gRPC dispatch) is unaffected in substance: its context-propagation shape already matches ADR-021's rules; it simply plugs into two direct-execute dispatchers instead of a shared `Pipeline`, which is exactly the gap this ADR records.
- This repo's `README.md`/`CLAUDE.md` should reference this ADR (and, through it, upstream ADR-021) as the governing description of request flow.
