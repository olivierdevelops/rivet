---
document_id: ARCH-2026-0001
title: "Rivet runtime architecture"
document_type: architecture
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, registry, execution, files, connectors, audit, policy, auth, datagrams, quic, grpc, sessions, serve, transports, cli, http, library, mcp, ws, poll]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, embedded, server]
audience: [maintainers, contributors, reviewers]
scope: How the implemented 0.1.0 codebase is structured — the five VHCO buckets and what lives in each, the request lifecycle, the policy broker, interpreter/handles/DAG execution, serve fan-out, the port-to-adapter map, data flow and the three deployment shapes (CLI, serve, library).
reason: DOCUMENTATION.md §31 architecture impact for PLAN-2026-0001 row D-14; the runtime is new in 0.1.0 and its structure must be documented from the real src tree.
related_documents: [PLAN-2026-0001, PROP-2026-0001, ADR-0001, ADR-0002, ADR-0003, API-2026-0001, API-2026-0004, SEC-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, architecture, vhco, runtime, serve, policy]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.1.0-dev (commit f40d4aa)"
---

# Rivet runtime architecture

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** all runtime features and all six access points

## Summary

Rivet is one Rust crate (`rivet`, binary `rivet`) that loads a `.rivet` bundle, compiles it into an immutable catalog of described operations, and serves those operations through six access points — CLI, REST, SSE, polling, WebSocket, MCP and the Rust library — all of which call **one dispatcher**. Every effect a script attempts (file, network, process, env, credentials, MCP, gRPC) passes through **one policy broker** before any I/O. The code follows the VHCO five-bucket layout: shared data in `domain`, pure use cases in `features`, surface adapters in `io`, port adapters in `infra`, and all wiring in `orchestrator`.

```text
          ┌───────────────────────────── access points ─────────────────────────────┐
          │  CLI      REST     SSE      polling    WebSocket    MCP        library   │
          │  io/cli   io/http  io/http  io/http/   io/ws        io/mcp     (Runtime  │
          │           ─────────── one serve listener (axum) ───────────    methods)  │
          └────┬────────┬────────┬────────┬───────────┬───────────┬──────────┬──────┘
               └────────┴────────┴────────┴─────┬─────┴───────────┴──────────┘
                     serve.authenticate_principal + serve.authorize_operation
                                                │
                                   Runtime::dispatch_request     (orchestrator/runtime.rs)
                                                │
                  ┌──────────── rivet.* built-in? ── yes ─▶ orchestrator/builtins.rs
                  │ no                          (list, describe, outputs, sessions.*, io,
                  ▼                              policy.generate, trace.show, auth.*, …)
        execution.request_operation ─▶ Interpreter (infra/execution_driver.rs)
                  │                           │ each effect
                  │                           ▼
                  │                 feature use case (exchange_http, apply_file_operation, …)
                  │                           │ authorize every attempt
                  │                           ▼
                  │                 PolicyBroker ◀── policy.json (or deny-by-default)
                  │                           │ allowed
                  │                           ▼
                  │                 infra adapter (hyper, cap-std, quinn, tonic, tokio::process …)
                  ▼
        execution.validate_output ─▶ Completion | RivetError  ─▶ back to the surface
```

## The five buckets

The folders under `src/` are the architecture. Dependencies point inward only: `domain` imports nothing internal; `features` import `domain`; `io` and `infra` import `domain` (and `infra` may call a feature use case only through a closure injected by the orchestrator); `orchestrator` imports everything and is the only place that wires.

```text
  src/
  ├── domain/        shared vocabulary: data types, ports (traits), pure helpers — no I/O
  ├── features/      one folder per feature, one file per use case: pure logic over ports
  ├── io/            surface encoders/decoders: CLI args, HTTP bodies, SSE, WS frames, JSON-RPC
  ├── infra/         adapters that satisfy ports: parser, interpreter, sockets, files, OAuth, …
  ├── orchestrator/  composition root: builds Runtime, mounts surfaces, dispatches built-ins
  ├── lib.rs         pub mod the five; re-exports Runtime, RuntimeBuilder
  └── main.rs        exit(orchestrator::setup_cli::main())
```

### `domain/` — shared data and ports

| Module | Holds |
|---|---|
| `errors` | `ErrorKind` (22), `RivetError`, `EffectsStatus`, registry mappings (exit, HTTP, retryable) |
| `value` | `Value` (null, bool, int, float, text, bytes, list, object) and JSON conversion |
| `source`, `syntax_tree` | `SourceBundle`, `SourceSpan`, the parsed tree handed to lowering |
| `ir` | `CompiledProgram`, `Operation`, `Stmt`, `Expr`, `EffectForm`, DAG nodes |
| `contracts` | `Request`, `Completion`, `DataEvent`, `Envelope`, `RegistryEntry`, `Catalog`, `Principal` |
| `outputs` | declared output / emits / receives specs and JSON Schema |
| `policy` | `Policy`, `Grant`, `Capability` (13), `AccessVerb` (18), `EffectIntent`, `Permit`, `ServePolicy` |
| `effect_checks` | `authorize` (one permit per attempt) and `checked_addr` (post-DNS private-range check) |
| `ports` | every port trait: `Registry`, `ExecutionDriver`, `PolicyEvaluator`, `FileAccess`, `SessionDriver`, `DataSink`, `Dispatcher`, `TraceStore`, `McpClient`, `CredentialProvider`, `Authenticator`, `ServeListener`, … |
| `transports`, `transport` | HTTP / socket / process plans, `SandboxSpec`, URL assembly, `is_private_ip` |
| `files`, `grpc`, `mcp`, `auth` | per-protocol data (file ops, gRPC calls, MCP snapshots and bridge hops, OAuth profiles, `SecretString`) |
| `sessions` | session receipts, batches, acks, limits, input sequencer, bounded event log |
| `serve` | serve receipts, authn input, WS frames, polling routes, `MCP_PROTOCOL_VERSION`, `WS_SUBPROTOCOL` |
| `dag` | DAG state and the `DagExecutor` port |
| `io_manifest` | I/O manifest, effect sites, policy drafts, trace events and queries |

### `features/` — use cases (37)

| Feature | Use cases |
|---|---|
| `language` | `compile_program`, `compile_output_spec` (+ `lowering/` helpers) |
| `registry` | `describe_operations`, `inspect_outputs` |
| `execution` | `request_operation`, `validate_output`, `run_dag`, `cancel_request` |
| `policy` | `load_policy`, `authorize_effect`, `generate_policy` |
| `files` | `apply_file_operation` |
| `transports` | `exchange_http`, `exchange_socket`, `run_process` |
| `datagrams` | `exchange_datagrams` (UDP) |
| `quic` | `exchange_quic` |
| `grpc` | `invoke_rpc` |
| `connectors` | `invoke_mcp` (outbound MCP) |
| `auth` | `begin_authorization`, `complete_authorization`, `cancel_authorization`, `credential_status`, `disconnect_account`, `acquire_credential` |
| `sessions` | `open_session`, `send_input`, `finish_input`, `read_events`, `cancel_session` |
| `serve` | `start_serve`, `authenticate_principal`, `authorize_operation`, `multiplex_ws`, `project_polling` |
| `audit` | `inspect_effects` (I/O manifest), `read_trace` |

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
| `SourceLoader` | `source_loader` | std fs |
| `FileAccess`, `FileProbe` | `file_access` (`ConfinedFiles`) | cap-std (no-follow, hard-link refusal, root-confined) |
| `HttpClient` | `http_adapter` (+ `h3_client`, `net_tls`) | hyper 1 (HTTP/1.1, HTTP/2), h3 + quinn (HTTP/3), rustls/ring |
| `SocketStream` | `socket_adapter` | tokio TCP/Unix, tokio-tungstenite |
| `ProcessRunner` | `process_adapter` (+ `sandbox_macos`, `sandbox_linux`, `sandbox_unsupported`) | tokio::process, Seatbelt / Landlock+seccomp |
| `DatagramDriver` | `udp_adapter` | tokio UDP, socket2 |
| `QuicDriver` | `quic_adapter` | quinn |
| `GrpcDriver` | `grpc_adapter` | tonic + prost-reflect (descriptor-driven) |
| `McpClient` | `mcp_client` (`McpPeer`) | brokered HTTP or sandboxed stdio child |
| `OAuthSessionDriver`, `CredentialProvider` | `oauth_adapter` | brokered HTTP, keyring-core (memory or keychain) |
| `Codec` | `codec`, `wire_codec` | JSON, text, bytes, lines, JSONL, SSE framing |
| `SessionDriver` | `session_driver` (`SessionHost`) | in-memory bounded sessions |
| `RequestControl` | `request_control` | running-request table for cancel |
| `TraceStore` | `trace_store` (`MemoryTraceStore`) | in-memory, per process |
| `ServeListener`, `WsConnection` | `serve_listener` (`AxumListener`, `WsOutbox`) | axum 0.8 |
| `RemoteEndpoint` | `remote_client` | hyper + tungstenite (CLI `--endpoint`) |

`effect_args` holds shared option-parsing helpers for the effect adapters.

### `orchestrator/` — composition root

| File | Role |
|---|---|
| `runtime.rs` | `RuntimeBuilder`, `Runtime`, `policy_from_json`; assembles broker, `TracedEvaluator`, `PolicedFiles`, interpreter, adapters, registry, sessions, OAuth, MCP peer, gRPC; the dispatcher `dispatch_request`; `DagUseCase`, `NestedDispatcher` |
| `transports.rs` | registers `http`, `tcp`, `unix`, `websocket`, `command` adapters with their use cases injected as closures |
| `builtins.rs` | the 18 `rivet.*` built-ins and `visible()` |
| `setup_cli.rs`, `remote_cli.rs` | CLI entry; `--endpoint` remote mode |
| `setup_serve.rs` | `start` / `run_cli`, `ServeState`, shared auth + error responses |
| `setup_http.rs`, `setup_poll.rs`, `setup_ws.rs`, `setup_mcp.rs` | per-surface routes and handlers |
| `setup_library.rs` | session host wiring and the library session methods |

## Request lifecycle

```text
 surface                 orchestrator                       features / infra
 ───────                 ────────────                       ────────────────
 decode body ──────────▶ authenticate_principal (serve only)
                         new_request: request_id, trace_id,
                           deadline (30 s default; HTTP deadline_ms ≤ 600 s)
                         dispatch_request(req, sink?)
                           ├ depth 0: require_operation(principal, id)   ── 403 permission.denied
                           ├ id starts with "rivet." ─▶ builtins::dispatch_builtin
                           ├ imported MCP id ─▶ dispatch_mcp ─▶ connectors.invoke_mcp (deadline)
                           ├ depth 0: acquire concurrency permit        ── 429 limit.concurrency
                           ├ register in RequestControl (cancellable by id)
                           └ execution.request_operation
                                ├ registry.describe(id)                   ── 404 not_found.operation
                                ├ validate params (unknown, required, type, min/max/enum), defaults
                                ├ depth check                             ── 429 limit.call_depth
                                ├ ExecutionDriver.drive(op, params, sink)  ◀── scope owns handles
                                │     each emit ─▶ DataSink (SSE event, WS frame, session log, CLI line)
                                │     each effect ─▶ use case ─▶ broker ─▶ adapter
                                │     nested (request …) ─▶ NestedDispatcher ─▶ dispatch_request(depth+1)
                                └ validate_output(result, declared output) ── 500 output.invalid
                         Completion {request_id, trace_id, result, data_count, effects}
 encode ◀───────────────
```

Cancellation: a top-level request races its run against a cancel signal (`Runtime::cancel`, Ctrl-C, SSE disconnect, WS close, session cancel). Dropping the run drops the whole scope: concurrent branches, DAG nodes and handles are dropped together, children are killed and reaped, and `with` blocks close their handles in reverse order within a 5 s budget.

## Policy broker

```text
      policy.json ──load_policy (strict v1)──▶ Policy { grants, deny, network, limits, serve, approved }
                                                 │ absent → Policy::deny_all
                                                 ▼
  adapter / use case ── EffectIntent{capability, verb, target, operation_id, span} ──▶ TracedEvaluator
                                                                                       │
                              PolicyBroker.evaluate ─▶ authorize_effect(intent, policy, root)
                                 1. no policy file   → denied
                                 2. deny entry match → denied (deny wins)
                                 3. private IP literal & deny_private_ranges, not named literally → denied
                                 4. first grant matching capability + selector + verb → allowed
                                 5. otherwise → denied
                                                                                       │
                              Permit{decision, rule} ── recorded in broker.decisions   │
                                                     └─ TraceEvent (redacted target, effect_id,
                                                        attempt, policy_hash) → MemoryTraceStore
```

Adapters ask again for **every actual attempt**: each redirect hop, each resolved address of a host name (`checked_addr`), each file path a verb touches (source and destination), each gRPC/QUIC/UDP destination, each credential use. The same `StaticEvaluator` logic (no tracing) answers `rivet io --check-policy`. Details and guarantees: [SEC-2026-0001](../security/sec-2026-0001-policy-and-sandbox-model.md).

## Interpreter, handles and the DAG use case

`Interpreter` (`infra/execution_driver.rs`) walks the lowered IR of one operation inside the request's own task:

```text
  Interpreter.drive(op)
   ├ frames: operation-scoped variables; loop vars / error / params block-local
   ├ built-in effects: file verbs (through PolicedFiles → files.apply_file_operation)
   ├ registered adapters per EffectKind:
   │     http · tcp · unix · websocket · command   (orchestrator/transports.rs)
   │     udp · quic                                (runtime.rs register_transports)
   │     grpc                                      (GrpcEffects → grpc.invoke_rpc)
   │     unregistered kind ─▶ unsupported.adapter
   ├ with RESOURCE … as h    ─▶ handle in scope; closed in reverse order on success,
   │                             error, break and cancel (5 s budget → cleanup.timeout)
   ├ try / catch error kind K | code "C"   (no finally in 0.1.0)
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
   every node → envelope {status, result (null unless succeeded), error}
```

Handles never escape their scope: no handle crosses an operation return or a DAG edge.

## Serve fan-out

```text
  rivet serve --listen ADDR
    │
    ├ start_serve: non-loopback + auth none → serve.auth_required (exit 2); mtls → unsupported.serve_mtls
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
    └ stdio (--stdio): MCP only, no bind ─────┘
```

All surfaces share the same `Runtime` (catalog, broker, limits, trace store, sessions). A session's event log holds at most 16 frames, so a slow client applies backpressure to the producer instead of growing memory.

## Data flow

```text
  bootstrap (outside the broker, listed by `rivet io --include-bootstrap`)
    app.rivet (+ imports) · policy.json · connector snapshots / gRPC descriptors · CA bundle · resolver · tzdata
         │
         ▼
  compile ─▶ CompiledProgram ─▶ ProgramRegistry (catalog + effect sites) ─▶ catalog_version = sha256 of sources
                                        │
  request params (JSON) ─▶ Value ─▶ Interpreter ─▶ effects (brokered) ─▶ external systems
                                        │                    │
                                        │                    └▶ TraceEvent (decisions only; payloads never recorded)
                                        ▼
                         emits ─▶ DataEvent(seq) ─▶ sink / session log
                         result ─▶ output check ─▶ Completion ─▶ surface encoding
```

Nothing is persisted by the runtime itself except what a script writes through granted effects, `policy generate --output` / `connectors sync --output` (exclusive create), and OAuth tokens when a profile selects `store keychain`.

## Deployment shapes

```text
 1. CLI (one process per command)
    rivet --file app.rivet request demo.add --params '{"a":2}'
      └ build Runtime ─▶ dispatch ─▶ print ─▶ exit code from the registry

 2. serve (long-running)                       3. library (in-process)
    rivet --file app.rivet serve                  let rt = Runtime::builder().file(p).build()?;
      --listen 127.0.0.1:8080 | --stdio            rt.request(id, params, sink).await
      └ one listener, five surfaces                 (optionally setup_serve::start(rt, opts))

 1b. remote CLI
    rivet --endpoint http://host:8080 --token-file t request …
      └ RemoteClient ─▶ POST /v1/request | SSE | /v1/ws | GET /v1/operations | GET /v1/io
        (no bundle loaded locally; check, policy and serve are refused remotely)
```

| Shape | Principal | Policy source | State lifetime |
|---|---|---|---|
| CLI | `local` | `--policy PATH` or policy.json beside `--file` | one command |
| serve | from `serve.auth` (`local` on loopback/none, bearer principals) | same | process (sessions, MCP sessions, traces in memory) |
| stdio MCP | `local` | same | process |
| library | `local`, or host-supplied `Principal` | `.policy(…)`, `.policy_file(…)`, discovery with `.file`, deny with `.source` | the `Runtime` value |

## Known limitations (0.1.0)

No TLS/mTLS listener; Linux sandbox gated and Windows sandbox absent; `finally` not supported; no Alt-Svc discovery or connection pooling; the trace store and sessions are in-memory only; MCP resource templates and the legacy HTTP+SSE MCP transport are not supported; `--timeout` is not applied over the WebSocket duplex path.

## Related Documents

- [Architecture index](index.md) · [API index](../api/index.md) · [Policy and sandbox model](../security/sec-2026-0001-policy-and-sandbox-model.md)
- [ADR-0001](../decisions/adr-0001-approve-rivet-runtime-design.md) · [ADR-0002 crate selection](../decisions/adr-0002-rust-crate-selection.md) · [ADR-0003 sandbox backends](../decisions/adr-0003-process-sandbox-backends.md)
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) · [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial architecture from the src tree at commit f40d4aa. |
