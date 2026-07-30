# Architecture Decision Records

> [← Back to Design](../README.md)

**Audience**: Architects, developers

## WHAT

A chronological log of architectural decisions made during `edge-bootstrap` development, following the ADR format (context, decision, consequences).

## WHY

Records the reasoning behind design choices so future contributors understand why the system is built the way it is, rather than guessing or repeating past analysis.

## HOW

## Index

| ADR | Decision | Status |
|-----|----------|--------|
| [001](001-ingress-response-caching-per-route-opt-in.md) | Ingress response caching — per-route opt-in via `CacheAsideHandler`, not a whole-service decorator | Accepted |
| [002](002-real-observercontext-invocation-tracking.md) | Real `ObserverContext` for invocation tracking — bridge to `swe-observability-tracing`, wired at the `RuntimeBuilder` composition root | Accepted |
| [003](003-adopt-adr-021-system-request-flow.md) | Adopt upstream ADR-021 (System Request Flow) as governing architecture; record the per-transport `HandlerRegistry` duplication gap | Accepted |
