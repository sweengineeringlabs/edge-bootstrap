# ADR-005: Runtime-Neutral TypeScript Support Uses a Protocol Boundary, Not N-API or a Rust Runtime Rewrite

**Audience**: Developers, architects
**WHAT**: TypeScript support for `edge-bootstrap` will be provided as a runtime-neutral SDK over a versioned network protocol while the process runtime remains implemented in Rust
**WHY**: N-API binds consumers to Node-compatible runtimes, while rewriting the server in TypeScript would duplicate mature Rust behavior without making operating-system capabilities portable
**HOW**: Keep `scm/` as the process-level composition root, expose supported operations through HTTP or another explicitly versioned protocol, and implement the TypeScript package using web-standard APIs without Node-specific globals or imports

---

**Status**: Accepted
**Date**: 2026-08-01
**Deciders**: Core team

## Context

`edge-bootstrap` is a Rust process runtime, not a language-neutral utility library. Its adapters assemble HTTP and gRPC ingress, outbound transports, TLS, filesystem-backed configuration, lifecycle management, signals, observability, and systemd notification. These capabilities depend on an operating-system host and on the Rust/Tokio ecosystem.

TypeScript is a language that compiles to JavaScript; it does not itself define a runtime. A TypeScript consumer may execute under Node.js, Deno, Bun, a browser, an edge-worker environment, or another JavaScript host. A package that exposes Rust through N-API is therefore a Node native add-on even if its public declarations are written in TypeScript. It cannot be treated as general TypeScript support.

The desired capability is for TypeScript applications to configure, inspect, and call `edge-bootstrap` without forcing them to use Node.js. That requires the public integration boundary to use facilities shared by the intended JavaScript runtimes. It does not require the existing process runtime to be reimplemented in JavaScript.

## Decision Drivers

- **Runtime neutrality.** The TypeScript API must not require Node.js or Node-compatible native-add-on support.
- **Preserve the existing runtime.** Networking, TLS, lifecycle, configuration, and observability behavior already implemented and tested in Rust should remain authoritative.
- **Make the boundary explicit and versionable.** Rust and TypeScript types evolve independently; their wire contract must be testable and governed as a public API.
- **Use broadly available platform APIs.** A client based on `fetch`, `URL`, `AbortSignal`, `Uint8Array`, and Web Streams can work across substantially more JavaScript hosts than a native add-on.
- **Avoid false portability.** WebAssembly alone does not make native sockets, signals, arbitrary filesystem access, systemd integration, or the current Tokio-based server portable to every JavaScript runtime.

## Considered Options

### 1. Keep the Rust runtime and add a runtime-neutral TypeScript SDK over a versioned protocol (chosen)

`edge-bootstrap` remains a separately deployed Rust process. A TypeScript package exposes typed clients and data-transfer objects for the supported public operations. Communication uses HTTP with an OpenAPI or JSON Schema contract by default; gRPC-Web or another protocol may be added only where its runtime support and generated client do not compromise the portability requirement.

The TypeScript source uses ECMAScript modules and web-standard APIs: `fetch`, `Request`, `Response`, `Headers`, `URL`, `URLSearchParams`, `AbortController`/`AbortSignal`, `Uint8Array`, `TextEncoder`/`TextDecoder`, and Web Streams where necessary. Its runtime code must not import `node:*`, access `process`, use `Buffer`, call `require`, or assume `__dirname`/`__filename`. Development and publishing tools may run on Node.js as an implementation detail, provided the emitted package has no Node runtime dependency.

**Pros:**

- Supports Node.js, Deno, Bun, browsers, and compatible worker environments from one API surface.
- Keeps process-level behavior in the existing Rust implementation.
- Provides a language-independent boundary that can support future SDKs.
- Allows the Rust service and TypeScript SDK to be released and deployed independently.

**Cons:**

- Requires serialization, transport, and compatibility handling.
- Does not provide in-process calls or shared memory.
- Requires a running or reachable `edge-bootstrap` service.

### 2. Expose the Rust crate through N-API or `napi-rs`

**Pros:**

- Low-latency, in-process calls.
- Rust implementation can be reused behind TypeScript declarations.

**Cons:**

- Ties the package to Node.js and runtimes implementing sufficient Node-API compatibility.
- Requires native artifacts for supported platforms and architectures.
- Native server lifecycle and process behavior become embedded inside the JavaScript host.
- Does not meet the runtime-neutrality requirement.

### 3. Compile the complete Rust runtime to WebAssembly

**Pros:**

- Retains Rust source and exposes a JavaScript-callable artifact.
- Can be portable for deterministic computation with a narrow host interface.

**Cons:**

- The current runtime assumes native networking, Tokio, signals, filesystem paths, TLS integration, and systemd behavior.
- Browser, worker, and WASI hosts provide materially different capabilities.
- A large host-adapter layer would be required and would still not produce equivalent behavior everywhere.

WebAssembly remains suitable for isolated, deterministic components such as validation, codecs, routing rules, or policy evaluation when their dependencies support the target. It is not the process-runtime boundary selected by this ADR.

### 4. Rewrite `edge-bootstrap` in TypeScript

**Pros:**

- Produces a JavaScript-native implementation.
- May simplify direct use inside a selected JavaScript runtime.

**Cons:**

- Duplicates the Rust implementation, tests, and security-sensitive integration behavior.
- Still requires a particular host for sockets, filesystem access, signals, TLS, and server lifecycle.
- Creates two implementations whose behavior can diverge.
- TypeScript source alone does not make those host capabilities runtime-neutral.

## Decision

**Option 1 — a protocol-based, runtime-neutral TypeScript SDK with the Rust process retained** is selected.

The initial SDK should live under `sdk/typescript/` and publish ECMAScript modules plus TypeScript declarations. Its public models should correspond to explicitly exposed wire DTOs rather than mirror arbitrary internal Rust structs. The initial transport should prefer HTTP and a documented schema because `fetch` is the broadest common client capability among the target runtimes.

TypeScript-authored request handlers compile to WebAssembly components hosted by the Rust runtime, as defined by [ADR-006](006-typescript-handlers-as-webassembly-components.md). This ADR establishes the runtime-neutral SDK boundary; ADR-006 defines the handler ABI, host capabilities, lifecycle, limits, cancellation, streaming, context propagation, and compatibility semantics.

## Portability Contract

The TypeScript SDK runtime code:

- may use standard ECMAScript and documented Web APIs, including `fetch`, `Request`, `Response`, `Headers`, `URL`, `URLSearchParams`, `AbortController`/`AbortSignal`, `Uint8Array`, `TextEncoder`/`TextDecoder`, and Web Streams where necessary;
- must ship without native binaries or install scripts;
- must not depend on Node built-ins or Node globals;
- must be tested against at least two materially different JavaScript hosts before being described as runtime-neutral;
- must treat protocol compatibility and error payloads as part of its public API.

Node.js may be used to compile, test, bundle, or publish the package. Tooling choice does not create a runtime dependency when the emitted library and its transitive runtime dependencies remain portable.

## Trade-offs Accepted

- **Calls cross a process or network boundary.** Serialization and transport latency are accepted in exchange for portability and independent deployment.
- **Not every Rust API is exposed.** Only stable use cases receive wire-level operations and DTOs; internal traits and implementation types remain Rust APIs.
- **A host is still required.** The Rust service owns native process capabilities, while the TypeScript client requires a host providing the selected web APIs.
- **Portable does not mean identical everywhere.** Streaming, TLS trust, cancellation, and connection behavior can vary by host and must be covered by compatibility tests and documentation.

## Consequences

- `scm/` remains the authoritative process runtime and composition root.
- A future `sdk/typescript/` package will contain the portable client, generated or maintained DTOs, compatibility tests, and package metadata.
- A versioned API description must be established before exposing Rust types to the SDK; Rust serialization layout alone is not the contract.
- CI should type-check the SDK and run its conformance suite on the explicitly supported JavaScript runtimes.
- N-API may only be introduced later as an optional, Node-specific package with a distinct name and support promise; it must not become the implementation beneath the runtime-neutral SDK.
- TypeScript handler components are governed by ADR-006. Any proposal for an embedded general-purpose JavaScript engine, N-API binding, out-of-process JavaScript handler host, or complete TypeScript rewrite requires a separate ADR because each changes the deployment and lifecycle model selected here.
