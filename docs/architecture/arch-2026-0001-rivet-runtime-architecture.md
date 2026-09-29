---
document_id: ARCH-2026-0001
title: "Rivet runtime architecture"
document_type: architecture
status: active
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 4
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, registry, execution, files, connectors, audit, policy, auth, datagrams, quic, grpc, sessions, serve, transports, cli, http, library, mcp, ws, poll, ffi]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, embedded, server]
audience: [maintainers, contributors, reviewers]
scope: How the implemented 0.1.0/0.2.0 codebase is structured — the Cargo workspace (rivet-runtime, rivet-ffi), the public facade versus the hidden five VHCO buckets (src/internal.rs), what lives in each bucket, the envelope edge layer every surface shares, Cargo feature gates, file modules and catalog snapshots, the ffi surface, the request lifecycle, the policy broker, interpreter/handles/DAG execution, serve fan-out, the port-to-adapter map, data flow and the deployment shapes (CLI, serve, library, C ABI).
reason: DOCUMENTATION.md §31 architecture impact for PLAN-2026-0001 row D-14 and PLAN-2026-0002 row D-30 (TASK-079); 0.2.0 changed packaging, boundaries, the wire layer and added a surface, all documented from the real tree.
related_documents: [PLAN-2026-0001, PROP-2026-0001, PLAN-2026-0002, PROP-2026-0002, ADR-0001, ADR-0002, ADR-0003, ADR-0004, ADR-0005, API-2026-0001, API-2026-0004, API-2026-0006, API-2026-0007, MIG-2026-0001, SEC-2026-0001, SYS-2026-0001, SYS-2026-0004, SYS-2026-0010, SYS-2026-0011]
supersedes: null
superseded_by: null
tags: [rivet, architecture, vhco, runtime, serve, policy, workspace, ffi, envelope, features, modules]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.2.0-rc (main at 8031baa)"
next_review_date: 2026-10-29
---

# Rivet runtime architecture

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** all runtime features, all access points and the C ABI

## Summary

Rivet 0.2.0 is a Cargo **workspace** of two packages. `rivet-runtime` provides the library `rivet` and, with the `cli`
feature, the binary `rivet`. `rivet-ffi` builds `librivet` for C hosts. Rivet loads a `.rivet` bundle (an entry file
and the file modules it imports), compiles it into an immutable **catalog snapshot** of described operations, and
serves those operations through its access points: CLI, REST, SSE, polling, WebSocket, MCP, the Rust library and,
new in 0.2.0, the C ABI. All of them call **one dispatcher**, and all of them read and write one wire shape: an
`InputEnvelope` in and a `ResponseEnvelope` out ([API-2026-0006](../api/api-2026-0006-envelopes.md)). Every effect a
script attempts (file, network, process, env, credentials, MCP, gRPC) passes through **one policy broker** before any
I/O. The code follows the VHCO five-bucket layout: shared data in `domain`, pure use cases in `features`, surface
adapters in `io`, port adapters in `infra`, and all wiring in `orchestrator`. Since 0.2.0 the buckets sit behind a
small public **facade** (`src/lib.rs`) and are reachable only as the hidden `rivet::internal::…`.

```text
          ┌───────────────────────────────── access points ──────────────────────────────────┐
          │  CLI      REST     SSE      polling    WebSocket    MCP        library     C ABI   │
          │  io/cli   io/http  io/http  io/http/   io/ws        io/mcp     (facade)    ffi/ +  │
          │  [cli]    ─────────── one serve listener (axum) [serve] ───────            setup_ffi│
          └────┬────────┬────────┬────────┬───────────┬───────────┬──────────┬──────────┬─────┘
               └────────┴────────┴────────┴─────┬─────┴───────────┴──────────┴──────────┘
          envelope edge: serve.parse_input (InputEnvelope, deprecated id/params) ··· ResponseEnvelope out
                     serve.authenticate_principal + serve.authorize_operation (network surfaces)
                                                │
                                   Runtime::dispatch_request     (orchestrator/runtime.rs)
                                   on the current catalog snapshot (RwLock<Arc<Snapshot>>)
                                                │
                  ┌──────────── rivet.* built-in? ── yes ─▶ orchestrator/builtins.rs
                  │ no                          (capabilities, list, describe, outputs, sessions.*,
                  ▼                              io, policy.generate, trace.show, auth.*, …)
        execution.request_operation ─▶ Interpreter (infra/execution_driver.rs)
                  │                           │ each effect
                  │                           ▼
                  │                 feature use case (exchange_http, apply_file_operation, …)
                  │                           │ authorize every attempt
                  │                           ▼
                  │                 PolicyBroker ◀── policy.json (or deny-by-default) ∩ host ceiling
                  │                           │     then TracedEvaluator ∩ per-request restrict stack
                  │                           │ allowed
                  │                           ▼
                  │                 infra adapter (hyper, cap-std, quinn, tonic, tokio::process …)
                  ▼
        execution.validate_output ─▶ Completion | RivetError ─▶ ResponseEnvelope ─▶ back to the surface
```

`[cli]` and `[serve]` mark Cargo features; `grpc`, `quic` and `oauth` gate adapters further down (see
[Cargo feature gates](#cargo-feature-gates)).

## Workspace and packages (0.2.0)

```text
 rivet/  (Cargo workspace: members ".", "ffi"; one version in [workspace.package])
 │
 ├── package rivet-runtime  ─── lib  "rivet"   src/lib.rs  ── facade (stable API)
 │   (crates.io name;             │                          └─ #[doc(hidden)] pub mod internal (src/internal.rs)
 │    `use rivet::…` in code)     │                               #[path] → src/{domain,features,io,infra,orchestrator}
 │                                └─ bin  "rivet"   src/main.rs   required-features = ["cli"]
 │
 ├── package rivet-ffi (publish = false) ── lib "rivet": cdylib + staticlib  → librivet.{dylib,so,a}
 │     ffi/src/lib.rs   #[no_mangle] extern "C" shims only (CStr ↔ bytes, handle ↔ u64)
 │     ffi/include/rivet.h (cbindgen, checked in; CI fails on drift) · rivet.pc (render_pc.py)
 │     depends on rivet-runtime (default-features = false, features passed through)
 │
 └── editors/  keywords.json (generated from rivet.capy; include_str!'d by the highlighter)
               vscode/ TextMate grammar + package_vsix.py (Python, no Node)       → SYS-2026-0011
```

All FFI **logic** lives in the runtime crate as the `ffi` surface (`src/orchestrator/setup_ffi.rs`), because the
VHCO model reads only `src/` ([ADR-0005](../decisions/adr-0005-workspace-package-and-features.md),
PLAN-2026-0002 findings). The shim crate stays thin. Details: [SYS-2026-0010](../system/components/sys-2026-0010-ffi-surface-and-packaging.md).

## Facade boundary (`src/lib.rs` / `src/internal.rs`)

```text
                 embedders (Rust)                              Rivet itself
                 ────────────────                              ────────────
  use rivet::{Runtime, RuntimeBuilder, Module, Scope,       tests/, examples/, rivet-ffi
      StreamHandle, DuplexHandle, DuplexSender,                     │
      InputEnvelope, ResponseEnvelope, Envelope,                    │ may use
      EnvelopeStatus, RecordType, OutputFormat,                     ▼
      Completion, DataEvent, DataSink, Policy, Value,    rivet::internal::{domain, features, io,
      Error, ErrorKind, Result, highlight,                                 infra, orchestrator}
      build_features, VERSION, ABI_VERSION,                    (#[doc(hidden)], no stability promise)
      types::{…}}                                                   ▲
                 │                                                  │ inside the crate:
                 └──────── re-exports of ────────────────────────── pub(crate) use internal::{domain,…}
                                                                    keeps every crate::domain::… path
```

The buckets' folders and paths did not move: `src/internal.rs` declares them with `#[path]`. `rivet::domain::X` in
0.1.0 code becomes `rivet::X` (or `rivet::internal::domain::X`); [MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md)
lists the moves. Embedding `rivet serve` uses `rivet::serve::{start, ServeOptions}` (feature `serve`); session and trace types are in the facade too (INC-2026-0012).

## The envelope edge layer (0.2.0)

The envelope is a boundary concern. Execution still produces `Completion | RivetError | DataEvent`, and the edge
converts them. Input goes through one use case and output through one domain type:

```text
  body · frame · MCP arguments · --data/--input · call_json · C string
        │
        ▼  features/serve/parse_input.rs  (serve.parse_input, pure)
  InputEnvelope{operation, data, deadline_ms?, restrict?, stream?, pretty?} + aliases[] (id/params)
        │                                     └─▶ Deprecation: header, access-log "deprecated":1,
        ▼                                         trace row (phase input), CLI warning
  Runtime::dispatch_request / dispatch_session
        │
        ▼  domain/envelope.rs (pure)
  ResponseEnvelope::from_outcome · from_data · accepted · with_ref · with_seq · render(Compact|Pretty)
        │
        ├─ CLI: stdout (ok) / stderr (error, cancelled)      ├─ HTTP body, ?pretty=true (access-log middleware)
        ├─ SSE: event = record type (data | result)          ├─ polling: events[] inside the session batch
        ├─ WS: record with `ref` first                        ├─ MCP: structuredContent + content[0].text
        └─ library: ResponseEnvelope / Envelope records       └─ C ABI: char* per envelope / record
```

The remote client (`infra/remote_client.rs`) decodes the same envelopes back into `Completion | RivetError`. When a
2xx body is not an envelope, it reports a 0.1.x server (`protocol.endpoint`). Surface details:
[SYS-2026-0004](../system/components/sys-2026-0004-surfaces-and-serve.md#the-wire-edge-020).

## Cargo feature gates

```text
 default = [serve, grpc, quic, oauth]              cli (off) ── clap + the `rivet` binary
   serve ── axum listener ─────────────────────── rivet serve (without: unsupported.feature, exit 5)
   grpc  ── tonic, prost, prost-reflect, tower ── grpc_adapter      ┐ compiled out: stand-ins in
   quic  ── quinn, h3, h3-quinn ───────────────── quic_adapter, h3  │ infra/unsupported_features.rs
   oauth ── keyring-core + platform stores ────── oauth_adapter     ┘ (refuse again if ever reached)

 load ─▶ compile ─▶ domain::capabilities::require_build_features(program)
              ├─ all present ─▶ Runtime
              └─ missing ─▶ unsupported.feature {feature}  (kind unsupported, exit 5, HTTP 501; every use,
                                                           source order; before anything runs)
 rivet.capabilities: build_features [...], abi_version, protocol rows "unsupported" with the reason
```

A lean build never degrades silently. It refuses the bundle at load. The CI feature matrix builds `rivet-runtime`
with each feature alone. Per-adapter table: [SYS-2026-0005](../system/integrations/sys-2026-0005-protocol-adapters.md#deployment).

## File modules and catalog snapshots (0.2.0)

```text
  app.rivet ──import "./users.rivet" as users──────────▶ users.rivet          (alias users, internal)
      └────import "./lib/billing.rivet" as billing public ▶ billing.rivet     (alias billing, public)
                                                             └─import "../users.rivet" as people ─▶ same file,
                                                                compiled once under the shallowest alias
  language.resolve_imports ─▶ language.compile_program (namespace_modules, compile_globals) ─▶ CatalogSnapshot
                                                                                                   │
  Runtime { snapshot: RwLock<Arc<Snapshot{catalog, registry, interpreter, mcp, oauth, …}>> } ◀────┘
        ▲                                      requests clone the Arc they start with
        │ registry.load_module (Runtime::load / rivet_load): compile current roots + new root, swap the Arc
        └─ loads are serialized; running requests (and their nested calls) finish on their old snapshot
```

A bundle of any number of files runs under **one** policy (the loader's). Each file has its own frozen global scope,
and a module's connectors, auth profiles and globals stay inside it. Details:
[SYS-2026-0001](../system/components/sys-2026-0001-compiler-and-catalog.md#run-time-module-loads-and-catalog-snapshots) and
[SEC-2026-0001](../security/sec-2026-0001-policy-and-sandbox-model.md).

## The `ffi` surface (0.2.0)

```text
  C host ── rivet_*() ──▶ ffi/src/lib.rs ──▶ orchestrator/setup_ffi.rs (safe Rust, panic-guarded)
                                                 │ handle table: never-reused tokens (generation<<2 | kind)
                                                 │   runtime(1) · call(2) · module(3); unknown/freed → error
                                                 ▼
             FfiRuntime{Runtime, tokio "rivet-ffi", pretty}   (one per rivet_runtime_new)
               rivet_request      ─▶ Runtime::call_json ─▶ envelope string
               rivet_call_start   ─▶ connection-owned session ─▶ rivet_call_next: one record per string
               rivet_load         ─▶ Runtime::load(_as) ─▶ module handle ─▶ rivet_module_call[_start]
               rivet_highlight    ─▶ language.highlight_source
             every returned char* tracked; rivet_string_free checks it (double free → RIVET_ERROR)
```

The host process is the principal (`local`), and the options' policy is its only grant. Reference:
[API-2026-0007](../api/api-2026-0007-c-abi.md) and [SYS-2026-0010](../system/components/sys-2026-0010-ffi-surface-and-packaging.md).

## The five buckets

The folders under `src/` are the architecture. Dependencies point inward only: `domain` imports nothing internal; `features` import `domain`; `io` and `infra` import `domain` (and `infra` may call a feature use case only through a closure injected by the orchestrator); `orchestrator` imports everything and is the only place that wires.

```text
  src/
  ├── domain/        shared vocabulary: data types, ports (traits), pure helpers — no I/O
  ├── features/      one folder per feature, one file per use case: pure logic over ports
  ├── io/            surface encoders/decoders: CLI args, HTTP bodies, SSE, WS frames, JSON-RPC
  ├── infra/         adapters that satisfy ports: parser, interpreter, sockets, files, OAuth, …
  ├── orchestrator/  composition root: builds Runtime, mounts surfaces, dispatches built-ins, ffi surface
  ├── internal.rs    #[doc(hidden)] pub mod internal: #[path] declarations of the five (0.2.0)
  ├── lib.rs         the facade: re-exports Runtime, Module, envelopes, Policy, Value, Error, highlight, …
  └── main.rs        exit(orchestrator::setup_cli::main())            (only with --features cli)
```

### `domain/` — shared data and ports

| Module | Holds |
|---|---|
| `errors` | `ErrorKind` (22), `RivetError`, `EffectsStatus`, registry mappings (exit, HTTP, retryable) |
| `value` | `Value` (null, bool, int, float, text, bytes, list, object) and JSON conversion |
| `source`, `syntax_tree` | `SourceBundle` (+ `modules`, `ModuleRef`, `ImportDecl`, 0.2.0), `SourceSpan`, the parsed tree handed to lowering |
| `envelope` (0.2.0) | `ResponseEnvelope`, `InputEnvelope`, `RawInput`, `EnvelopeStatus`, `RecordType`, `OutputFormat`, `error_object` |
| `modules` (0.2.0) | `ModuleLoad`, `ModuleSummary`, `CatalogSnapshot` |
| `const_eval` (0.2.0) | the pure constant evaluator behind `global` |
| `highlight` (0.2.0) | `HighlightToken`, `HighlightFormat`, the renderers (ansi, html, json) |
| `ffi` (0.2.0) | `FfiOptions` (runtime options JSON of the C ABI) |
| `ir` | `CompiledProgram` (+ `globals`, `global_scopes`, `modules`, 0.2.0), `Operation`, `Stmt`, `Expr`, `EffectForm`, DAG nodes, `BUILTIN_FUNCTIONS` |
| `cancel` | `CancelToken` tree and `CancelReason` (structured cancellation) |
| `call_graph` | `GraphQuery`, `CallGraph` (`rivet graph`) |
| `capabilities` | `BuildProbe` (+ `build_features`, `abi_version`), `require_build_features`, `ABI_VERSION`, the `rivet.capabilities` model |
| `contracts` | `Request`, `Completion`, `DataEvent`, `Envelope`, `RegistryEntry`, `Catalog`, `Principal` |
| `outputs` | declared output / emits / receives specs and JSON Schema |
| `policy` | `Policy` (+ optional host `ceiling`, `with_ceiling`), `Grant`, `Capability` (13), `AccessVerb` (18), `EffectIntent`, `Permit`, `ServePolicy` |
| `effect_checks` | `authorize` (one permit per attempt) and `checked_addr` (post-DNS private-range check) |
| `ports` | every port trait: `Registry`, `ExecutionDriver`, `PolicyEvaluator`, `FileAccess`, `SessionDriver`, `DataSink`, `Dispatcher`, `TraceStore`, `McpClient`, `CredentialProvider`, `Authenticator`, `ServeListener`, … |
| `transports`, `transport` | HTTP / socket / process plans, `SandboxSpec`, URL assembly, `is_private_ip` |
| `files`, `grpc`, `mcp`, `auth` | per-protocol data (file ops, gRPC calls, MCP snapshots and bridge hops, OAuth profiles, `SecretString`) |
| `sessions` | session receipts, batches, acks, limits, input sequencer, bounded event log |
| `serve` | serve receipts, authn input, WS frames, polling routes, `MCP_PROTOCOL_VERSION`, `WS_SUBPROTOCOL` |
| `dag` | DAG state and the `DagExecutor` port |
| `io_manifest` | I/O manifest, effect sites, policy drafts, trace events and queries |

### `features/` — use cases (45)

| Feature | Use cases |
|---|---|
| `language` | `compile_program`, `compile_output_spec`, `resolve_imports` (0.2.0), `compile_globals` (0.2.0), `highlight_source` (0.2.0) (+ `lowering/` helpers incl. `modules.rs`) |
| `registry` | `describe_operations`, `inspect_outputs`, `describe_capabilities`, `load_module` (0.2.0) |
| `execution` | `request_operation`, `validate_output`, `run_dag`, `cancel_request` |
| `policy` | `load_policy`, `authorize_effect`, `generate_policy` |
| `files` | `apply_file_operation`, `open_file_stream` (`with file open`) |
| `transports` | `exchange_http`, `exchange_socket`, `run_process` |
| `datagrams` | `exchange_datagrams` (UDP) |
| `quic` | `exchange_quic` |
| `grpc` | `invoke_rpc` |
| `connectors` | `invoke_mcp` (outbound MCP) |
| `auth` | `begin_authorization`, `complete_authorization`, `cancel_authorization`, `credential_status`, `disconnect_account`, `acquire_credential` |
| `sessions` | `open_session`, `send_input`, `finish_input`, `read_events`, `cancel_session` |
| `serve` | `start_serve`, `authenticate_principal`, `authorize_operation`, `multiplex_ws`, `project_polling`, `parse_input` (0.2.0) |
| `audit` | `inspect_effects` (I/O manifest), `read_trace`, `build_graph` (`rivet graph`) |

Each feature folder also has a `ports.rs` re-exporting the domain ports it needs.

### `io/` — surfaces

| Module | Surface | Role |
|---|---|---|
| `io/cli` | CLI | clap argument model, table/JSON rendering |
| `io/http` | REST + SSE | request-body parsing, `deadline_ms` cap, error status, SSE event encoding, catalog JSON |
| `io/http/poll` | polling | events query and input body parsing |
| `io/ws` | WebSocket | client frame parsing (`request`, `input`, `finish_input`, `cancel`) |
| `io/mcp` | MCP | JSON-RPC parsing, tool descriptors, built-in tool list, tool results |

Handlers themselves live in `orchestrator/setup_*.rs`; `io` only decodes and encodes.

### `infra/` — adapters (port → adapter)

| Port | Adapter (`src/infra/…`) | Backed by |
|---|---|---|
| `Parser` | `capy_parser` | Capy (`capy-core`, pinned git rev) |
| `ExecutionDriver` | `execution_driver` (`Interpreter`) | tree-walking interpreter, tokio |
| `Registry` | `registry` (`ProgramRegistry`) | compiled program + MCP imports + effect sites |
| `PolicyEvaluator` | `policy_broker` (`PolicyBroker`) | injected `authorize_effect` |
| `PolicyFileReader` / `PolicyDraftWriter` | `policy_file_reader` / `policy_draft_writer` | std fs (exclusive create for drafts) |
| `SourceLoader` | `source_loader` (`DiskSourceLoader`: entry, `read_module` below the root without symlinks, `policy_beside`) | std fs |
| `CatalogStore` (0.2.0) | `RuntimeCatalog` in `orchestrator/runtime.rs` | resolve + compile + atomic snapshot swap |
| `FileAccess`, `FileProbe` | `file_access` (`ConfinedFiles`), `file_stream` (`FileStreams`) | cap-std (no-follow, hard-link refusal, root-confined), flock compare-and-replace |
| `HttpClient` | `http_adapter` (+ `h3_client`, `net_tls`) | hyper 1 (HTTP/1.1, HTTP/2), h3 + quinn (HTTP/3), rustls/ring |
| `SocketStream` | `socket_adapter` | tokio TCP/Unix, tokio-tungstenite |
| `ProcessRunner` | `process_adapter` (+ `sandbox_macos`, `sandbox_linux`, `sandbox_unsupported`) | tokio::process, Seatbelt / Landlock+seccomp |
| `DatagramDriver` | `udp_adapter` | tokio UDP, socket2 |
| `QuicDriver` | `quic_adapter` | quinn |
| `GrpcDriver` | `grpc_adapter` | tonic + prost-reflect (descriptor-driven) |
| `McpClient` | `mcp_client` (`McpPeer`) | brokered HTTP or sandboxed stdio child |
| `OAuthSessionDriver`, `CredentialProvider` | `oauth_adapter` | brokered HTTP, keyring-core (memory or keychain) |
| `Codec` | `codec`, `wire_codec` | JSON, text, bytes, lines, JSONL, SSE framing |
| `SessionDriver` | `session_driver` (`SessionHost`, `BufferBudget`) | in-memory bounded sessions, background sweeper, host byte budget |
| `RequestControl` | `request_control` | running-request table for cancel |
| `TraceStore` | `trace_store` (`MemoryTraceStore`) | in-memory, per process |
| `ServeListener`, `WsConnection` | `serve_listener` (`AxumListener`, `WsOutbox`) | axum 0.8 |
| `RemoteEndpoint` | `remote_client` | hyper + tungstenite (CLI `--endpoint`; decodes envelopes, detects 0.1.x servers) |
| (compiled-out adapters) | `unsupported_features` (0.2.0) | stand-ins for `quic` / `oauth` that refuse with `unsupported.feature` |

`effect_args` holds shared option-parsing helpers for the effect adapters.

### `orchestrator/` — composition root

| File | Role |
|---|---|
| `runtime.rs` | `RuntimeBuilder` (+ `ceiling`, `session_limits`, `root`), `Runtime` (catalog snapshot, `load`, `build_features`, `note_deprecated_input`), `Policy::from_file/from_json`, `policy_from_json`; assembles broker, `TracedEvaluator` (restriction stack), `PolicedFiles`, interpreter, adapters, registry, sessions, OAuth, MCP peer, gRPC; the dispatcher `dispatch_request` / `request_restricted`; `export_trace`, `graph`, `shutdown`; `DagUseCase`, `NestedDispatcher` |
| `transports.rs` | registers `http`, `tcp`, `unix`, `websocket`, `command` adapters with their use cases injected as closures |
| `builtins.rs` | the 20 `rivet.*` built-in IDs (`BUILTIN_IDS`), their dispatch, and `visible()` |
| `setup_cli.rs`, `remote_cli.rs` | CLI entry; `--endpoint` remote mode |
| `setup_serve.rs` | `start` / `run_cli`, `ServeState`, shared auth + error responses, access-log middleware, `/v1/health`, `traceparent`, SIGINT/SIGTERM drain |
| `setup_http.rs`, `setup_poll.rs`, `setup_ws.rs`, `setup_mcp.rs` | per-surface routes and handlers |
| `setup_library.rs` | session host wiring, the library session methods, `Runtime::call` / `call_json`, `Runtime::scope` with `StreamHandle` / `DuplexHandle`, `Runtime::load` → `Module`, `highlight` |
| `setup_ffi.rs` (0.2.0) | the `ffi` surface behind `librivet`: handle table, guards, runtime/call/module/highlight functions |

## Request lifecycle

```text
 surface                 orchestrator                       features / infra
 ───────                 ────────────                       ────────────────
 decode body ──────────▶ serve.parse_input ─▶ InputEnvelope (aliases → Deprecation signals)
                         authenticate_principal (serve only)
                         new_request(_traced): request_id, trace_id (from a W3C traceparent when given),
                           deadline (30 s default; HTTP deadline_ms / CLI --timeout ≤ 600 s),
                           restrict? (narrow-only), CancelToken
                         dispatch_request(req, sink?)
                           ├ depth 0: require_operation(principal, id)   ── 403 permission.denied
                           ├ id starts with "rivet." ─▶ builtins::dispatch_builtin
                           ├ imported MCP id ─▶ dispatch_mcp ─▶ connectors.invoke_mcp (deadline)
                           ├ depth 0: acquire concurrency permit        ── 429 limit.concurrency
                           ├ restrict given? push it on the task's restriction stack
                           ├ register in RequestControl (cancellable by id: fires the token)
                           └ execution.request_operation
                                ├ registry.describe(id)                   ── 404 not_found.operation
                                ├ validate params (unknown, required, type, min/max/enum), defaults
                                ├ depth check                             ── 429 limit.call_depth
                                ├ ExecutionDriver.drive(op, params, sink)  ◀── scope owns handles
                                │     each emit ─▶ check `emits` + secret taint ─▶ DataSink (SSE event,
                                │                   WS lane, session log, CLI line, library handle)
                                │     each effect ─▶ secret sink check ─▶ use case ─▶ broker ─▶ adapter
                                │     nested (request …) ─▶ NestedDispatcher ─▶ dispatch_request(depth+1)
                                └ validate_output(result, declared output) ── 500 output.invalid
                         Completion {request_id, trace_id, result, data_count, effects} | RivetError
 encode ◀─────────────── ResponseEnvelope {request_id, trace_id, operation, type, status, data, error, effects, data_count}
```

Cancellation is structured: a cancel signal (`Runtime::cancel`, Ctrl-C, SSE disconnect, WS close, session cancel, serve drain) or the deadline fires the request's `CancelToken`; child tokens reach nested requests, DAG nodes, concurrent branches and `request.stream` children. The interpreter observes the token at every await point, leaves its `with` blocks normally so handles close in reverse order within the 5 s grace, and child processes are reaped. Only a run still going after the grace is dropped (the error then carries `cleanup.timeout` under `suppressed`).

```text
  cancel / deadline / drain ─▶ token ─▶ child tokens ─▶ unwind ─▶ close handles (≤ 5 s) ─▶ one cancelled|timeout
                                                              └─ grace exceeded ─▶ drop (last resort)
```

## Policy broker

```text
      policy.json ──load_policy (strict v1)──▶ Policy { grants, deny, network, limits, serve, approved }
                                                 │ absent → Policy::deny_all
                                                 ▼
  adapter / use case ── EffectIntent{capability, verb, target, operation_id, span} ──▶ TracedEvaluator
                                                                                       │
                              PolicyBroker.evaluate ─▶ authorize_effect(intent, policy, root)
                                 0. host ceiling (library) denies → denied "host ceiling: …"
                                 1. no policy file   → denied
                                 2. deny entry match → denied (deny wins)
                                 3. private IP literal & deny_private_ranges, not named literally → denied
                                 4. first grant matching capability + selector + verb → allowed
                                 5. otherwise → denied
                              TracedEvaluator: every per-request restrict must allow  │
                                 too, else denied "request restriction: …"            │
                              Permit{decision, rule} ── recorded in broker.decisions   │
                                                     └─ TraceEvent (redacted target, effect_id,
                                                        attempt, policy_hash) → MemoryTraceStore
```

Adapters ask again for **every actual attempt**: each redirect hop, each resolved address of a host name (`checked_addr`), each file path a verb touches (source and destination), each gRPC/QUIC/UDP destination, each credential use. The same `StaticEvaluator` logic (no tracing) answers `rivet io --check-policy`. Details and guarantees: [SEC-2026-0001](../security/sec-2026-0001-policy-and-sandbox-model.md).

## Interpreter, handles and the DAG use case

`Interpreter` (`infra/execution_driver.rs`) walks the lowered IR of one operation inside the request's own task:

```text
  Interpreter.drive(op)
   ├ frames: operation-scoped variables; loop vars / error / params block-local;
   │         lookup locals → params → the operation file's frozen globals (0.2.0)
   ├ built-in effects: file verbs (through PolicedFiles → files.apply_file_operation)
   ├ registered adapters per EffectKind:
   │     http · tcp · unix · websocket · command   (orchestrator/transports.rs)
   │     udp · quic                                (runtime.rs register_transports)
   │     grpc                                      (GrpcEffects → grpc.invoke_rpc)
   │     file open (with)                      (FileStreams → files.open_file_stream)
   │     unregistered kind ─▶ unsupported.adapter   (file watch ─▶ unsupported.stage_c)
   ├ with RESOURCE … as h    ─▶ handle in scope; closed in reverse order on success,
   │                             error, break and cancel (5 s grace → cleanup.timeout)
   ├ if … else … end · try / catch error kind K | code "C"   (no finally in 0.1.0 or 0.2.0)
   ├ secret taint            ─▶ every sink checked against the secret's bound origins
   ├ concurrent / map / poll ─▶ cooperative branches in the same task (FuturesUnordered)
   ├ (request ID {…})        ─▶ Dispatcher port → Runtime::dispatch_request(depth + 1)
   └ dag …                   ─▶ DagExecutor port → DagUseCase → execution.run_dag
```

```text
  execution.run_dag  (state machine per node)

   pending ──deps all succeeded & running < limit──▶ running ──▶ succeeded
      │                                                 ├──────▶ failed ──(fail fast: stop scheduling)
      ├──a dependency failed/blocked/cancelled/skipped──▶ blocked
      └──never started after a fatal error (fail fast)──▶ skipped
   running at the end of a fatal run ──▶ cancelled
   every node → node value {status, result (null unless succeeded), error, started_at, ended_at}
   (a language value inside `data`, not a wire envelope)
   fatal error (fail fast) → details.nodes merged in
```

Handles never escape their scope: no handle crosses an operation return or a DAG edge.

## Serve fan-out

```text
  rivet serve --listen ADDR
    │
    ├ start_serve: non-loopback + auth none → serve.auth_required (exit 2); mtls → unsupported.serve_mtls (exit 5)
    ├ bind ONE listener (AxumListener)
    ├ mount routers for serve.surfaces:  http · sse · poll · ws · mcp      (others → 404 not_found.route)
    │
    │   ServeState { runtime, serve policy, loopback flag, authenticator?, mcp_sessions }
    │
    ├ http  POST /v1/request ─────────────────┐
    ├ sse   POST /v1/request (event-stream) ──┤ ChannelSink(mpsc 16) → SSE body; drop = abort
    ├ poll  /v1/requests… ─▶ project_polling ─┤
    ├ ws    /v1/ws ─▶ multiplex_ws ───────────┼─▶ SessionHost (principal- or connection-owned sessions)
    ├ mcp   /mcp  ─▶ tools/call ──────────────┤      └▶ Runtime::dispatch_session / dispatch_request
    ├ stdio (--stdio): MCP only, no bind ─────┘
    ├ GET /v1/health (always mounted) · every router wrapped by the access log (one JSON line / request)
    └ SIGINT | SIGTERM ─▶ drain: stop accepting ─▶ Runtime::shutdown (requests + sessions) ─▶ exit 0
```

All surfaces share the same `Runtime` (catalog, broker, limits, trace store, sessions). A session's event log holds at most 16 frames / 32 MiB, every retained event reserves bytes against the host budget `limits.max_buffered_bytes`, and each WebSocket ref has its own 16-frame outbound lane, so a slow client applies backpressure to its own producer instead of growing memory.

## Data flow

```text
  bootstrap (outside the broker, listed by `rivet io --include-bootstrap`, one row per file)
    app.rivet + each imported module (below the root, no symlinks) · policy.json · connector snapshots /
    gRPC descriptors · CA bundle · resolver · tzdata
         │
         ▼
  resolve imports ─▶ compile (+ globals) ─▶ CompiledProgram ─▶ CatalogSnapshot ─▶ ProgramRegistry
                                            (catalog + effect sites; catalog_version = sha256 of every file)
                                        │
  InputEnvelope.data (JSON) ─▶ Value ─▶ Interpreter ─▶ effects (brokered) ─▶ external systems
                                        │                    │
                                        │                    └▶ TraceEvent (decisions only; payloads never recorded)
                                        ▼
                         emits ─▶ DataEvent(seq) ─▶ sink / session log
                         result ─▶ output check ─▶ Completion ─▶ ResponseEnvelope ─▶ surface framing
```

Nothing is persisted by the runtime itself except what a script writes through granted effects, `policy generate --output` / `connectors sync --output` / `Runtime::export_trace` (exclusive create), and OAuth tokens when a profile selects `store keychain`.

## Deployment shapes

```text
 1. CLI (one process per command; binary built with --features cli)
    rivet --file app.rivet request demo.add --data '{"a":2}'
      └ build Runtime ─▶ dispatch ─▶ print envelope ─▶ exit code from the registry

 2. serve (long-running, feature serve)        3. library (in-process, package rivet-runtime)
    rivet --file app.rivet serve                  let rt = Runtime::builder().file(p).build()?;
      --listen 127.0.0.1:8080 | --stdio            rt.call(InputEnvelope::new(id).data(json)).await
      └ one listener, five surfaces                 rt.load("./users.rivet")?  ─▶ Module
                                                    (serve embedding: rivet::internal::…::setup_serve)

 4. C ABI (in-process, librivet from rivet-ffi)
    rivet_runtime_new(options_json) ─▶ rivet_request / rivet_call_start / rivet_load ─▶ char* envelopes

 1b. remote CLI
    rivet --endpoint http://host:8080 --token-file t request …
      └ RemoteClient ─▶ POST /v1/request | SSE | /v1/ws | GET /v1/operations | GET /v1/io
        (no bundle loaded locally; check, graph, policy and serve are refused remotely)
```

| Shape | Principal | Policy source | State lifetime |
|---|---|---|---|
| CLI | `local` | `--policy PATH` or policy.json beside `--file` | one command |
| serve | from `serve.auth` (`local` on loopback/none, bearer principals) | same | process (sessions, MCP sessions, traces in memory) |
| stdio MCP | `local` | same | process |
| library | `local`, or host-supplied `Principal` | `.policy(…)`, `.policy_file(…)`, discovery with `.file`, deny with `.source`/`.root`; optional `.ceiling(…)` | the `Runtime` value (catalog snapshots swap on `load`) |
| C ABI | `local` (the host process) | options JSON: `policy_file` / `policy_json`, `ceiling_json` | the runtime handle |

Supported platforms are **macOS and Linux**, and CI (fmt, clippy, tests, feature matrix) is green on both. Windows
is not supported in 0.2.0 ([INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)).

## Known limitations

The complete list is the [manual's Known Limitations](../manuals/man-2026-0001-rivet-manual.md#known-limitations): no URL imports and no module unloading; the serve-embedding API is outside the facade; no TLS/mTLS listener (`unsupported.serve_mtls`, exit 5); Linux sandbox gated until verified on kernel ≥ 6.12; Windows and other OSes unsupported (INC-2026-0011); no crates.io publication yet (git dependency, G-PUB); no `finally`; `approved.overlaps` unused; the `*` principal pattern matches `rivet.auth.*`; `--timeout` not applied over the WebSocket duplex path; MCP 401 invalidates the lease without retry; no Alt-Svc discovery or connection pooling; no persistent trace store (traces and sessions are in memory); no MCP resource templates or legacy HTTP+SSE MCP transport; Stage C forms refused.

Architecture drift found at `829ca43` (TASK-092) — `BUILTIN_IDS` listed `rivet.trace.export` without a dispatch arm, and `io/mcp::builtin_tools` had no descriptor for it or for `rivet.capabilities` — was fixed in commit `2a751ab` (INC-2026-0007).

## Related Documents

- [Architecture index](index.md) · [API index](../api/index.md) · [Policy and sandbox model](../security/sec-2026-0001-policy-and-sandbox-model.md)
- [ADR-0001](../decisions/adr-0001-approve-rivet-runtime-design.md) · [ADR-0002 crate selection](../decisions/adr-0002-rust-crate-selection.md) · [ADR-0003 sandbox backends](../decisions/adr-0003-process-sandbox-backends.md)
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) · [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)
- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) (D-30) · [PROP-2026-0002](../proposals/implemented/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md) · [ADR-0004](../decisions/adr-0004-approve-envelopes-globals-library-ffi-highlighting.md) · [ADR-0005 workspace and features](../decisions/adr-0005-workspace-package-and-features.md)
- [API-2026-0006 envelopes](../api/api-2026-0006-envelopes.md) · [API-2026-0007 C ABI](../api/api-2026-0007-c-abi.md) · [MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md)
- [SYS-2026-0010 FFI, packaging, features](../system/components/sys-2026-0010-ffi-surface-and-packaging.md) · [SYS-2026-0011 highlighting](../system/components/sys-2026-0011-highlighting-and-grammar-generation.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial architecture from the src tree at commit f40d4aa. |
| 2 | 2026-09-28 | Claude | Fix batch through 829ca43: 40 use cases (`build_graph`, `open_file_stream`, `describe_capabilities`), new domain modules (`cancel`, `call_graph`, `capabilities`), host ceiling and restriction stack in the broker flow, structured cancellation, secret taint, DAG timestamps, serve health/access log/drain/`traceparent`, WS lanes and byte budget, library scopes; limitations aligned with the manual; drift recorded (fixed in 2a751ab). |
| 3 | 2026-09-29 | Claude | PLAN-2026-0002 D-30 (TASK-079): workspace and packages, facade boundary (`src/internal.rs`), envelope edge layer, Cargo feature gates, file modules and catalog snapshots, the `ffi` surface (diagrams); 45 use cases and new domain/infra modules; lifecycle, data flow and deployment shapes (C ABI) updated; macOS/Linux only. |
| 4 | 2026-09-29 | Claude | INC-2026-0012: serve embedding is in the facade (`rivet::serve`). |
