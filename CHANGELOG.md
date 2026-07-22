# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial project structure — extracted from the `sweengineeringlabs/edge` monorepo
  (`scm/bootstrap`), full history preserved.

### Known Issues

`edge-domain` (dependency key `edge-domain`, package `edge-application`) is pinned to
`v0.18.0`, the current latest tag. Bootstrap's other direct dependencies have not yet been
updated to match, and each is at a different point in the same org-wide `edge-domain` →
`edge-application` rename/rewrite:

- **`edge-proxy@v0.3.5`** — `LifecycleMonitor`'s methods (`start_background_tasks`,
  `shutdown`, `status`, `component`, `health`) were changed to take/return request-envelope
  types (`ShutdownRequest`, `StatusRequest`, `ComponentRequest`, `HealthRequest`, …) instead of
  raw arguments. Bootstrap's `main/src/core/monitor/lifecycle_monitor.rs` and
  `main/src/core/runtime/manager/default_runtime_manager.rs` still call the old, unenveloped
  signatures — 25 compile errors as of this writing. **Needs a rewrite of the `LifecycleMonitor`
  adapter to the new request/response shape.**
- **`swe-edge-ingress-http@v0.5.1`** / **`swe-edge-ingress-grpc@v0.6.1`** — still pinned to old,
  pre-rename `edge-domain` tags internally. Once the `LifecycleMonitor` issue above is fixed and
  the build gets further, these will very likely reproduce the same `Handler`/`HandlerRegistry`
  trait-duplication error described below for `edge-proxy`/`edge-dispatch`, since they resolve a
  different, older copy of the `Handler` trait than bootstrap's own `v0.18.0` pin. Will need
  bumping to whatever tag (if any) targets `edge-application`.
- **`swe-edge-egress-grpc@v0.5.0`** / **`swe-edge-egress-http@v0.4.2`** — pin very old
  `edge-domain` tags (`v0.8.30`) with **no newer tag published at all** as of this writing —
  these two cannot be aligned by a version bump alone; would need a new release cut in their
  own repos first.
- **`HandlerFactory`** was removed upstream with no replacement (config-driven handler
  construction was dropped from the domain contract). Already worked around: the trait is now
  owned locally in `main/src/api/config/traits/feature_registry_ext.rs` (same shape, no
  behavior change).
- **`InMemoryCache` → `MemoryCache`** (upstream rename) and the removal of the public
  `HandlerRegistryImpl` constructor (replaced by `HandlerComposer::create_registry`, upstream's
  own documented factory pattern) are already adapted for in `runtime_builder.rs` and
  `tests/dep_coverage_int_test.rs`.
- **`edge-llm-provider`** is pinned via a floating `branch = "main"` rather than a tag —
  observed to cause non-deterministic build results (an unrelated commit landing on that branch
  between builds changed what compiled). Should be pinned to a specific tag/rev.

Net effect: `cargo build --all-features` currently fails. Fixing it means working through the
above dependency-by-dependency — `edge-proxy`'s `LifecycleMonitor` rewrite first, then whatever
`ingress-http`/`ingress-grpc` need, in that order.
