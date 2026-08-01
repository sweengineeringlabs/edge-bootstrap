# ADR-006: TypeScript Handlers Compile to WebAssembly Components Hosted by the Rust Runtime

**Audience**: Developers, architects
**WHAT**: TypeScript application handlers are built into WebAssembly components and executed in process by `edge-bootstrap`; the deployed handler requires no Node.js, Deno, Bun, browser, or separate JavaScript service
**WHY**: A client-only SDK cannot reproduce `scm/examples/hello_edge.rs`, while N-API and separate JavaScript handler processes introduce runtime coupling or distributed-system overhead that is unnecessary when handlers can cross a capability-controlled Wasm boundary
**HOW**: Define handler and host contracts in WIT, provide a constrained TypeScript SDK and build toolchain that emits compatible components, and load those components through a Rust WebAssembly Component Model host

---

**Status**: Accepted
**Date**: 2026-08-01
**Deciders**: Core team

## Context

ADR-005 establishes runtime-neutral TypeScript support and rejects N-API as the primary boundary. The required developer experience goes beyond calling an existing Rust service: users must be able to write application handlers equivalent to those in `scm/examples/hello_edge.rs` and, incrementally, `scm/examples/bootstrap-e2e`.

TypeScript source cannot execute by itself. It normally compiles to JavaScript and needs a JavaScript engine such as Node.js, Deno, Bun, or an embedded engine. Requiring one of those runtimes in production would create a second application process or embed a general-purpose JavaScript runtime inside `edge-bootstrap`.

WebAssembly provides a different deployment boundary. TypeScript handler source is processed at build time into a `.wasm` component. The existing Rust process loads and invokes that artifact, retaining ownership of sockets, HTTP/gRPC ingress, TLS, routing, security, observability, configuration, and lifecycle. The deployed handler does not need the TypeScript compiler or a standalone JavaScript host.

WebAssembly does not execute itself. A compiler is still required to translate TypeScript source into Wasm, and a Wasm engine embedded in `edge-bootstrap` is required to validate, instantiate, execute, and limit the resulting component. In this ADR, “runtime” must therefore be qualified carefully:

- the **JustScript compiler** is a build-time tool and is not deployed with the service;
- the **Wasm engine** is the production execution runtime embedded in the Rust `edge-bootstrap` process;
- the complete **JustScript runtime/`justr` daemon** is not required in production;
- a small **guest-support library** may be compiled into the component, or implemented through explicit Rust host imports, only where generated code needs language services such as allocation, strings, arrays, or structured errors.

Ordinary TypeScript is not inherently a direct-to-Wasm language. JavaScript semantics, dynamic values, garbage collection, npm packages, and host APIs cannot be assumed to map automatically to portable Wasm. The SDK must therefore define a supported TypeScript handler profile and a controlled set of host capabilities rather than claim compatibility with every TypeScript program.

## Decision Drivers

- A TypeScript equivalent of `hello_edge.rs` must be possible without writing Rust application code.
- Production deployment must not require Node.js, Deno, Bun, a browser, N-API, or a second handler process.
- Rust remains authoritative for ingress, egress policy, TLS, routing, security enforcement, metrics, tracing integration, configuration, and graceful shutdown.
- Handler isolation must include deterministic memory, CPU, deadline, and capability limits.
- The boundary must be language-neutral, versioned, testable, and independent of Rust memory layout.
- TypeScript limitations must be explicit; unsupported JavaScript and npm behavior must fail during the build where possible.
- `hello_edge` parity must precede the substantially broader `bootstrap-e2e` surface.

## Considered Options

### 1. Compile a constrained TypeScript handler profile to WebAssembly components (chosen)

Applications use a TypeScript SDK containing wire-safe types and handler declarations. A build tool validates the supported profile and emits a WebAssembly component conforming to versioned WIT worlds. `edge-bootstrap` loads the component and adapts its exported handler functions to the existing dispatch boundary.

The compiler implementation may lower supported TypeScript directly or package JavaScript with a Wasm-compatible engine, but that implementation detail must not alter the component contract or introduce a production dependency on an external JavaScript runtime. The chosen implementation must satisfy the resource, startup, artifact-size, security, and compatibility requirements defined here.

**Pros:**

- One production process and no Node/Deno/Bun runtime dependency.
- Portable, versioned component artifacts with an explicit capability boundary.
- In-process calls avoid a network hop and remote registration lifecycle.
- Fuel, epoch interruption, memory limits, and capability controls can isolate handlers.
- WIT makes the boundary reusable by other component-producing languages.

**Cons:**

- Only the documented TypeScript profile and SDK are supported.
- Arbitrary npm packages, Node APIs, browser APIs, dynamic module loading, and unrestricted JavaScript reflection may not work.
- Component calls still incur canonical ABI conversion, copying, and sandbox overhead.
- Async operations, streams, resources, and cancellation need explicit component interfaces and host integration.
- The build toolchain and Wasm runtime become compatibility and supply-chain dependencies.

### 2. Package unrestricted JavaScript with a JavaScript engine such as QuickJS inside every Wasm artifact

**Pros:**

- Supports more familiar JavaScript semantics and potentially more existing packages.
- Does not require a standalone JavaScript process at deployment time.

**Cons:**

- Larger artifacts, slower startup, and duplicated engine memory across components.
- Host API emulation and npm compatibility remain incomplete.
- JavaScript garbage collection, promises, event-loop behavior, and async host calls must be bridged correctly to the Wasm component host.
- QuickJS or another embedded engine becomes an additional security and compatibility dependency; engine fixes require rebuilding and redeploying affected handler artifacts.
- Engine patching, provenance, and sandbox security become part of the handler artifact lifecycle.
- Weakens the ability to reject unsupported behavior at build time.

This may be an implementation technique for the chosen constrained profile only if it meets all limits; unrestricted compatibility is not promised.

### 3. Use AssemblyScript as the handler language

**Pros:**

- Compiles directly to compact WebAssembly.
- Familiar syntax for TypeScript developers.

**Cons:**

- AssemblyScript is TypeScript-like, not general TypeScript.
- Its type system, runtime behavior, standard library, and package compatibility differ.
- Advertising it simply as TypeScript would create misleading compatibility expectations.

An AssemblyScript adapter may be added later, but it does not define the TypeScript SDK promised by this ADR.

### 4. Run TypeScript handlers out of process under Node.js, Deno, or Bun

**Pros:**

- Broad JavaScript and package compatibility.
- Native debugging and runtime tooling.

**Cons:**

- Requires another production runtime and supervised process.
- Introduces serialization, transport, discovery, health, reconnection, and partial-failure behavior.
- Makes handler availability a distributed-systems concern.

A browser or edge-worker variant using a TypeScript-initiated WebSocket was also considered. Connection inversion would avoid requiring the JavaScript host to bind a listener, but would add multiplexing, heartbeat, bounded flow control, registration leases, reconnection, authentication, and route-claim authorization. It would also imply browser-hosted application logic, which is not a requirement. Deno and Bun are standalone server runtimes, not browsers; using browser-standard APIs in them does not make browser execution necessary.

### 5. Embed a general-purpose JavaScript engine directly in `edge-bootstrap`

**Pros:**

- One process and broad JavaScript semantics.
- No component build step is necessarily required.

**Cons:**

- Couples the Rust runtime to engine upgrades, module loading, garbage collection, event-loop integration, and a larger native attack surface.
- Makes strong per-handler isolation and resource accounting harder.
- Does not establish a language-neutral component boundary.

### 6. Provide only a runtime-neutral TypeScript client SDK over HTTP or gRPC-Web

**Pros:**

- Smallest implementation and broad compatibility through `fetch` and other Web APIs.
- Clean versioned service boundary with independent client and server releases.

**Cons:**

- Can invoke, configure, or inspect handlers already supplied by Rust, but cannot author new TypeScript handlers.
- Cannot reproduce the application-authoring behavior of `hello_edge.rs` or `bootstrap-e2e`.
- Does not satisfy the stated goal even though it remains useful as one SDK surface under ADR-005.

### 7. Expose Rust handlers through N-API or `napi-rs`

**Pros:**

- Low-latency in-process calls and familiar TypeScript declarations.
- Reuses the existing Rust implementation.

**Cons:**

- Requires Node.js or a runtime with sufficient Node-API compatibility.
- Requires native artifacts for every supported operating system and architecture.
- Couples JavaScript and Rust process lifecycles and expands the native boundary.
- Does not meet the requirement to avoid tying TypeScript support to Node.js.

### 8. Rewrite the complete `edge-bootstrap` runtime in TypeScript

**Pros:**

- Handler and runtime implementation share one language.
- Direct integration with the selected JavaScript runtime's tooling.

**Cons:**

- Duplicates mature Rust networking, TLS, security, lifecycle, configuration, and observability behavior.
- Still requires Node.js, Deno, Bun, or another host for native server capabilities.
- Creates two runtime implementations whose behavior and security fixes can diverge.
- TypeScript source does not itself provide runtime neutrality.

### 9. Compile the complete existing Rust runtime to WebAssembly

**Pros:**

- Retains one Rust implementation and produces a Wasm artifact.

**Cons:**

- The current runtime assumes Tokio networking, native sockets, signals, filesystem paths, TLS integration, and systemd behavior.
- Browser, worker, and WASI hosts expose materially different capabilities and cannot provide equivalent behavior without a large host-adapter layer.
- Conflates portable application handlers with inherently host-specific process infrastructure.
- Wasm is appropriate for the handler boundary selected here, not as a claim that the entire native server is portable unchanged.

## Decision

**Option 1 — constrained TypeScript handlers compiled to WebAssembly components** is selected.

The portable artifact is the WebAssembly component, not TypeScript source and not a JavaScript runtime. Build tooling may use Node.js, Deno, Bun, or another compiler host as a development dependency, but the emitted component and `edge-bootstrap` production process must not depend on that host.

The intended source shape is:

```ts
import type { EdgeHandler, EdgeRequest, EdgeResponse } from "@swe/edge-bootstrap";

export const echo: EdgeHandler = (
  request: EdgeRequest,
): EdgeResponse => ({
  status: 200,
  body: request.body,
});
```

This API is illustrative, not frozen. The build validates the supported language and dependency profile, generates or consumes component bindings, and emits a component plus manifest. Rust loads the manifest, validates declared routes and capabilities, and instantiates the component.

## JustScript Toolchain Assessment

The existing SWE JustScript platform is the preferred **build-time compiler** candidate and must be evaluated before introducing another TypeScript-to-Wasm compiler. Its compiler already parses TypeScript/JavaScript into JustIR and has a `CompileTarget::Wasm`; its tests demonstrate emission of valid core Wasm v1 modules for basic numeric expressions and functions. `justscript_runtime` also contains a structural Wasm validator and native runtime facilities that may provide reusable implementation pieces.

This decision does not adopt the complete `justscript_runtime`, `justr` daemon, JIT worker model, event loop, filesystem layer, networking layer, or process runtime. Reuse from that repository is limited to small libraries or contracts proven necessary for emitted components. Such code either becomes part of the component artifact or is satisfied by an explicit capability implemented by the Rust host.

It is not yet a drop-in implementation of this ADR:

- the compiler lives in the sibling `justscript_compiler` repository; `justscript_runtime` consumes it as `swe_justc_compiler`;
- the current backend emits core Wasm modules, not WIT-bound Component Model components;
- Wasm values are currently primarily `f64`; strings are stubbed and `Uint8Array`/general arrays are not available at the required boundary;
- the Wasm backend does not yet provide its native backend's GC, promises/async behavior, exceptions, fetch, or event loop;
- the documented Wasm stdlib roadmap assumes JavaScript loader glue for strings, arrays, fetch, DOM, and timers, which is incompatible with this ADR's no-production-JavaScript-host requirement;
- `justscript_runtime`'s daemon-level `CompiledHandler::call() -> Result<f64, String>` is too narrow for request/response handlers;
- structural validation exists, but a Rust Component Model execution host does not yet.

The JustScript compiler is therefore a candidate with a concrete extension path, not an already-complete dependency. Its conformance spike is a release gate. Failure of the spike requires revisiting this ADR before selecting QuickJS, AssemblyScript, or another compiler.

## Build and Execution Workflow

The development/build environment and the production environment are deliberately different:

```text
BUILD TIME

handler.ts + edge-handler.toml + versioned WIT
                    │
                    ▼
       JustScript compiler/tooling
       - type-check supported profile
       - lower TypeScript to core Wasm
       - generate/adapt canonical ABI
       - componentize and validate
                    │
                    ▼
       edge-handler.wasm + validated manifest


PRODUCTION

HTTP/gRPC request
       │
       ▼
Rust edge-bootstrap ingress and policy
       │
       ▼
Wasm handler adapter
       │
       ▼
Embedded Wasm engine
       │  loads edge-handler.wasm
       │  enforces memory/CPU/deadline/capabilities
       ▼
Compiled TypeScript handler logic
       │
       ▼
WIT response → Rust ingress → caller
```

The production deployment contains `edge-bootstrap`, its embedded Wasm engine, the `.wasm` component, and its manifest. It does not contain the TypeScript source, JustScript compiler, `justr` daemon, Node.js, Deno, Bun, QuickJS, or a separate JavaScript handler process. If the selected compiler packages an engine such as QuickJS inside the component, that would be an explicit exception governed by Considered Option 2 and must be measured and approved; it is not implied by the chosen option.

## Conformance Gate

Before production integration, JustScript must compile a minimal handler equivalent to:

```ts
export function echo(body: Uint8Array): Uint8Array {
  return body;
}
```

Rust must instantiate the resulting artifact and verify that it:

- exports a callable handler;
- transfers arbitrary bytes in both directions without JavaScript loader glue;
- has explicit, safe allocation and deallocation ownership;
- traps and rejects malformed values predictably;
- can be adapted to or emitted as a Component Model component;
- requires no Node.js, Deno, Bun, browser, or external JavaScript engine at execution time.

This byte-oriented gate intentionally avoids general strings, objects, GC, async operations, and npm compatibility. Those features are not prerequisites for the first echo milestone.

## Component Contract

Versioned WIT packages define:

- handler metadata and identity;
- HTTP and gRPC route intent without exposing transport-internal Rust types;
- unary request and response records;
- structured application, timeout, cancellation, resource-limit, and unavailable errors;
- absolute deadlines and cancellation;
- trace context, correlation identifiers, and security claims required by handler logic;
- readiness, initialization, draining, and shutdown hooks where needed;
- request and response streams with backpressure;
- explicitly granted outbound-call, logging, metrics, configuration, secret-reference, clock, and randomness capabilities;
- compatibility behavior across contract versions.

The first WIT version is deliberately smaller than the eventual contract: one unary handler, method/path/headers/body request data, status/headers/body response data, and structured invalid-request/application/cancelled/deadline errors. Streaming, gRPC-specific metadata, lifecycle hooks, egress, secrets, metrics, clocks, and randomness are added only after the unary contract passes conformance and isolation tests.

The contract does not expose Rust pointers, memory layouts, trait objects, or arbitrary `HandlerContext` contents. Immutable, wire-safe snapshots are preferred. Host callbacks are narrow capabilities that require explicit manifest declarations, authorization, limits, and cancellation behavior. Raw credentials and secrets are not copied into handler context by default.

## Execution and Isolation

Every component instance executes with:

- bounded linear memory and table growth;
- CPU limits enforced through fuel, epoch interruption, or an equivalent mechanism;
- invocation deadlines and cancellation propagation;
- bounded request, response, header, and stream sizes;
- bounded concurrency and queues;
- no filesystem, network, environment, clock, randomness, or secret access unless explicitly granted;
- deterministic mapping of traps, exhausted resources, invalid output, and unavailable capabilities to framework errors.

A handler trap or exhausted limit must fail the affected invocation and must not terminate the Rust runtime. Instance reuse, pooling, and isolation granularity must preserve these guarantees and prevent state leakage between tenants or handlers.

## Component Manifest

Route and capability declarations live outside arbitrary handler code in a validated manifest. Its initial shape includes component path and identity, HTTP method/path, memory and execution limits, concurrency, and explicitly granted capabilities. Rust validates the manifest before registering any route. A component cannot obtain a capability merely by importing or requesting it when the manifest and deployment policy do not grant it.

The manifest and WIT package are separately versioned public contracts. Route collisions, replacement, component digest/provenance, and incompatible contract versions fail before traffic reaches the handler.

## Repository Responsibilities

- **`justscript_compiler`:** TypeScript-profile validation, JustIR lowering, byte/string/record ABI support, exported handler generation, actionable unsupported-feature diagnostics, and core-Wasm-to-component emission or packaging.
- **`justscript_runtime`:** no whole-runtime or daemon dependency. Only reusable guest-support or validation pieces needed for allocation conventions, byte/string/record ABI support, and component/import validation may be extracted or consumed. The existing scalar `CompiledHandler` daemon abstraction is not the edge handler API and may be bypassed entirely.
- **`edge-bootstrap`:** versioned WIT and manifest ownership, the embedded Wasm engine, component loading, the Wasm-to-existing-handler adapter, route registration, host capabilities, resource enforcement, caching/pooling, health, tracing, and framework error mapping.
- **`sdk/typescript`:** handler types, supported-profile documentation, build command integration, example source, generated bindings/artifacts, and developer diagnostics.

## Example Parity

Parity means equivalent externally observable behavior, not one-to-one reproduction of Rust builder APIs or ownership.

For `hello_edge`, TypeScript defines an echo handler and route intent; the build emits a component; Rust loads it, exposes `/echo`, dispatches a request into the component, returns the response, reports readiness, and shuts down cleanly.

For `bootstrap-e2e`, TypeScript defines application handler logic and HTTP/gRPC route intent. Rust continues to construct and own ingress, egress, TLS, authentication, intrusion protection, metrics, tracing, configuration, and lifecycle. TypeScript does not recreate those infrastructure implementations.

## Delivery Sequence

1. **Compiler spike:** use JustScript to compile the `Uint8Array` echo gate and invoke it from Rust with no JavaScript glue. Record actual language, ABI, artifact-size, and runtime gaps. Do not claim general TypeScript compatibility before this passes.
2. **Unary ABI:** define the first versioned WIT world and manifest for metadata, byte-oriented unary invocation, context, errors, and health; add component generation or a deterministic componentization step.
3. **Guest support:** implement required allocation, byte/string, record, and error lowering in `justscript_compiler`/`justscript_runtime`, with build-time rejection of unsupported features.
4. **Rust host:** add the Component Model host, Wasm handler adapter, manifest validation, compiled-component caching, isolation, and resource limits without changing local Rust handlers.
5. **SDK and build:** implement TypeScript declarations and an `edge-ts build`-style command that type-checks, validates the profile, invokes JustScript, componentizes, validates WIT compatibility, and emits the component plus manifest.
6. **MVP example:** create `sdk/typescript/examples/hello-edge/handler.ts`, its manifest and documentation, and verify end-to-end behavioral parity through Rust-owned HTTP ingress.
7. **Hardening:** add cross-language ABI, malformed-component, invalid-output, trap, timeout, cancellation, capability-denial, oversized-payload, and resource-exhaustion tests.
8. **Expansion:** extend WIT and host capabilities for async egress, observability, streaming, and the additional behavior needed by `bootstrap-e2e`.

## Acceptance Criteria

- `sdk/typescript/examples/hello-edge/handler.ts` builds into a component and serves `/echo` through Rust-owned HTTP ingress.
- The JustScript conformance gate transfers non-text and empty byte payloads correctly without JavaScript loader glue.
- The deployed example runs with no Node.js, Deno, Bun, browser, N-API module, or separate JavaScript process.
- Cancellation and deadlines interrupt component work and map to deterministic framework errors.
- Memory, CPU, concurrency, queue, and payload limits are enforced and tested.
- Undeclared filesystem, network, environment, clock, randomness, and secret access is unavailable.
- A trap or malicious component cannot terminate the Rust runtime or affect unrelated handlers.
- WIT compatibility and malformed input/output behavior have cross-language contract tests.
- The manifest rejects undeclared capabilities, route collisions, incompatible interface versions, and untrusted or invalid component artifacts before registration.
- The SDK build rejects unsupported language features or dependencies with actionable diagnostics where feasible.
- Existing in-process Rust handlers and examples continue to behave unchanged.

## Trade-offs Accepted

- **TypeScript compatibility is intentionally constrained.** The SDK supports a documented handler profile, not arbitrary TypeScript, JavaScript, npm packages, Node APIs, or browser APIs. Portability and enforceable isolation take priority over ecosystem compatibility.
- **A build-time compiler and a production Wasm engine are required.** The compiler translates TypeScript into a component and is absent from production. The embedded engine executes and isolates that component in production. WebAssembly removes the standalone JavaScript-runtime dependency; it does not eliminate compilation or execution machinery.
- **Component calls are not free.** Canonical ABI conversion, copying, sandbox checks, compilation, and instantiation add latency and memory overhead relative to native Rust handlers. Pooling and ahead-of-time compilation may optimize this without weakening isolation.
- **Capabilities replace ambient access.** Filesystem, network, environment, time, randomness, configuration, secrets, logging, metrics, and outbound calls are unavailable unless the host grants narrow interfaces explicitly.
- **Host and guest evolve together through WIT.** Interface versions, generated bindings, runtime support, and component artifacts require a compatibility policy and coordinated testing.
- **Feature parity is behavioral and incremental.** `hello_edge` parity precedes `bootstrap-e2e`; TypeScript owns application logic while Rust retains infrastructure ownership.
- **The Wasm runtime becomes security-critical.** Runtime updates, resource-limit correctness, component validation, and toolchain provenance require active maintenance and security review.

## Consequences

- ADR-005's SDK is not complete when it can only call Rust; the component build path, Rust host, and TypeScript `hello-edge` conformance example are required.
- `edge-bootstrap` gains a Wasm component handler adapter alongside, not in place of, local Rust handler registration.
- The WIT packages and component manifest become public compatibility surfaces with independent versions and conformance tests.
- No browser-hosted server or handler process is implied. Browser compatibility is outside this ADR.
- Node.js, Deno, and Bun may be development-tool hosts but are not production runtime dependencies.
- The complete `justscript_runtime` and `justr` daemon are not production dependencies. Reusable guest-support code may be linked into components or exposed as narrow Rust host capabilities only when required.
- An out-of-process handler protocol, embedded general-purpose JavaScript engine, N-API binding, or TypeScript rewrite requires a separate ADR and cannot silently replace this component path.
