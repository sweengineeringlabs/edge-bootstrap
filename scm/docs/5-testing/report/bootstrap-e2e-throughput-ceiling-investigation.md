# Report: `bootstrap-e2e` Throughput Ceiling Investigation

**Audience**: Developers, architects
**WHAT**: A black-box, client-side investigation into why load tests against the `bootstrap-e2e`
example plateau at ~2700-2760 rps over HTTP and ~1300-1460 rps over gRPC, regardless of client
concurrency
**WHY**: A concurrency sweep (20 / 100 / 300) showed flat throughput with linearly-scaling latency
at both ceilings — the textbook signature of a saturated queue — but that signature alone doesn't
say *what* is saturated. This report documents the follow-up experiments run to find out
**HOW**: Server-process CPU sampling during a saturating run, plus a controlled multi-channel /
multi-process comparison, isolate how much of each ceiling is attributable to the client's own
connection model versus the server

---

**Status**: Findings recorded, root cause partially isolated
**Date**: 2026-08-05

## Context

`bootstrap-e2e` (`scm/main/adapter/examples/bootstrap-e2e`) was used as a live target for a
separate load-generation tool (`edge-bench`, a different repository) to exercise real HTTP and gRPC
traffic against a `RuntimeBuilder`-assembled service — HTTP bound at `127.0.0.1:18090` (`/echo`),
gRPC at `127.0.0.1:19090` (`echo`), default Cargo features only (no `intrusion`, no
`observability`). Both routes dispatch to the same hand-written `EchoHandler`, which per request:
opens/finishes an observer span, increments a metrics counter, and emits one structured log line,
in addition to the JSON (HTTP) / protobuf (gRPC) codec work `RuntimeBuilder` performs.

A concurrency sweep (10s runs at concurrency 20/100/300, both protocols, zero errors throughout)
found:

| Concurrency | HTTP rps | HTTP p50 | gRPC rps | gRPC p50 |
|---|---|---|---|---|
| 20  | 2682.17 | 6.4ms  | 1296.80 | 14.8ms |
| 100 | 2730.83 | 34.7ms | 1427.92 | 67.4ms |
| 300 | 2759.48 | 95.9ms | 1456.82 | 202.0ms |

Throughput is essentially flat across a 15x concurrency increase while latency scales roughly
linearly — consistent with requests queuing in front of a fixed-rate server. That result doesn't
distinguish *why* the rate is fixed, or why gRPC's ceiling sits at roughly half of HTTP's. This
report covers the three follow-up experiments run to narrow that down. All three were run from the
client side only — nothing in `edge-bootstrap` was modified, instrumented, or made aware of the
benchmarking tool as part of this investigation.

## Method and results

### 1. Server CPU during a saturating run

Sampled the `bootstrap-e2e` process's cumulative CPU time (Windows `Get-Process`, ~1.5s interval)
during a concurrency=300, 20s HTTP run (2282 rps observed) on an 8-logical-core machine.

| Time | Cumulative CPU (s) |
|---|---|
| t+0s  | 39.39 |
| t+2s  | 39.73 |
| t+4s through t+11s | 39.73 – 39.75 |

Cumulative CPU time moved by ~0.36s over an 11-second window that overlapped sustained ~2282 rps
traffic (roughly 25,000 requests). That is far below what 8 available cores could absorb —
**the server is not CPU-bound at the measured ceiling.** This rules out per-request compute cost
(JSON/protobuf codec, span creation, metrics increment, log formatting) as the limiting factor.

### 2. gRPC: single shared channel vs. independent channels

The load-generation client holds one gRPC transport (one HTTP/2 connection) shared across every
concurrent worker in a run — at concurrency=300, all 300 in-flight calls multiplex over that one
connection, subject to whatever concurrent-stream limit applies to it.

Tested by running 4 independent client **processes** in parallel (each opens its own channel),
concurrency=50 each (200 total) — vs. the single-channel concurrency=300 baseline (1456.82 rps):

| Run | rps |
|---|---|
| Process 1 | 488.34 |
| Process 2 | 460.17 |
| Process 3 | 474.53 |
| Process 4 | 475.42 |
| **Aggregate** | **1898.46** |

Spreading the same order of concurrency across independent HTTP/2 connections raised aggregate
gRPC throughput by ~30% (1456.82 → 1898.46 rps). **The client's single-shared-channel design is a
real, measurable contributor to the gRPC ceiling.**

### 3. HTTP control: same test, opposite result

Same experiment on the HTTP path — 4 independent processes, concurrency=75 each (300 total) vs.
the single-process concurrency=300 baseline (2759.48 rps):

| Run | rps |
|---|---|
| Process 1 | 484.56 |
| Process 2 | 485.69 |
| Process 3 | 451.68 |
| Process 4 | 472.89 |
| **Aggregate** | **1894.82** |

Splitting HTTP across processes at the same total concurrency *reduced* aggregate throughput
(2759.48 → 1894.82 rps). HTTP was never limited by single-connection multiplexing — its
underlying client already pools multiple TCP connections per process — so splitting only adds
process/runtime overhead. This is the control that confirms result #2 is a real gRPC-specific
effect, not a generic "more processes helps" artifact.

## Findings

- **Confirmed**: the server is not CPU-bound at either ceiling (near-idle CPU during saturation).
- **Confirmed**: part of the gRPC ceiling is attributable to the load-generation client's own
  single-shared-HTTP/2-channel design — using independent channels raises it by ~30%.
- **Confirmed (control)**: the same technique does not help, and mildly hurts, HTTP — ruling out a
  generic multi-process effect as the explanation for #2.
- **Unresolved**: even with independent gRPC channels, aggregate throughput (~1898 rps) still sits
  roughly 31% below HTTP's single-process ceiling (2759 rps). Combined with near-idle server CPU,
  this residual gap is most likely HTTP/2 framing/protobuf-codec overhead or the server's gRPC
  dispatch path being less parallel than its HTTP path — but confirming that would require
  profiling `bootstrap-e2e` itself (flamegraph or tracing spans around the gRPC dispatch path),
  which was out of scope for this investigation.

## Scope and limitations

This was a black-box investigation from the client side only. No source file in `edge-bootstrap`
was read with intent to change it, no code was modified, and no benchmarking hooks were added to
`bootstrap-e2e` or any other example. Confirming the residual gRPC-vs-HTTP gap in finding 4 above
would require server-side profiling, which is a separate, larger piece of work and was not
attempted here.
