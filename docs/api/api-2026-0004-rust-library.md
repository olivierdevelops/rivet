---
document_id: API-2026-0004
title: "Rivet Rust library API"
document_type: api
status: active
created_date: 2026-09-28
last_updated: 2026-09-30
document_revision: 4
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [library, execution, sessions, policy, registry, audit, serve]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, embedded, server]
audience: [rust-developers, integrators]
scope: The public Rust API of the `rivet-runtime` package (library `rivet`) as implemented in 0.2.0 — the Cargo dependency (git tag) and features, the crate-root facade, `Runtime` / `RuntimeBuilder`, `Runtime::call` / `call_json` envelopes, `request` and dispatch, streaming through `DataSink` and scopes, `Runtime::load` / `load_as` and `Module` objects, sessions, catalog, I/O manifest, policy drafts, traces, highlighting, cancellation and embedding `serve`.
reason: DOCUMENTATION.md §31 API impact for PLAN-2026-0001 row D-27 (0.1.0 contract) and PLAN-2026-0002 row D-43 (facade, features, dependency snippet, call, envelopes, load/load_as/Module; TASK-070).
related_documents: [PLAN-2026-0001, PLAN-2026-0002, PROP-2026-0001, PROP-2026-0002, ADR-0005, API-2026-0001, API-2026-0005, API-2026-0006, API-2026-0007, MIG-2026-0001, MAN-2026-0007, ARCH-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, api, rust, library, embedding, facade, cargo-features, modules]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.2.0-rc (source at 6f9943f)"
next_review_date: 2026-10-29
---

# Rivet Rust library API

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-30
> **Affected Versions:** 0.1.0 and later (the facade, features, `call` and `load` from 0.2.0)
> **Owner:** Project maintainer
> **Affected Components:** library, execution, sessions, policy, registry, audit, serve

## Summary

The Rust library is the same runtime the CLI and `rivet serve` use — not a client. A host builds one `Runtime` per bundle and calls the **same dispatcher** every other surface calls, so validation, the policy broker, limits, output checks and error codes are identical. From 0.2.0 the package is **`rivet-runtime`** (library name still `rivet`), its public API is the **crate-root facade**, and `Runtime::call` answers the same **ResponseEnvelope** as every other surface ([API-2026-0006](api-2026-0006-envelopes.md)).

```text
   host process (tokio)                        use rivet::{Runtime, InputEnvelope, …}
   ┌─────────────────────────────────────────────────────────────────────────────┐
   │  Runtime::builder()                                                         │
   │     .file("app.rivet") | .source(path, text, root) | .root(dir) (no entry)  │
   │     .policy_file(p)    | .policy(Policy::from_file(p)? | from_json(b)?)     │
   │     .ceiling(Policy)   host ceiling: every attempt needs policy ∩ ceiling   │
   │     .build()?  ─────────────▶ compile → load policy → wire adapters         │
   │                                (unsupported.feature for compiled-out ones)  │
   │                                                                             │
   │  rt.call(InputEnvelope) / call_json(&str) ─▶ ResponseEnvelope (never Err)   │
   │  rt.request(id, Value, sink)              ─▶ rivet::Result<Completion>      │
   │  rt.request_restricted(id, v, restrict, sink) ─▶ narrowed for this request  │
   │  rt.scope(|scope| … scope.stream / scope.duplex …) ─▶ owned, joined         │
   │  rt.load(path) / load_as(path, alias)     ─▶ Module (call / stream / …)     │
   │  rt.list / describe / outputs / graph     ─▶ catalog and static call graph  │
   │  rt.io / generate_policy / trace / export_trace ─▶ audit                    │
   │  rivet::highlight::tokens(src)            ─▶ Vec<HighlightToken>            │
   │  rivet::internal::…::setup_serve::start   ─▶ embed every network surface    │
   └─────────────────────────────────────────────────────────────────────────────┘
```

### Dependency and features

The crate is not on crates.io yet (the git dependency `capy-core` blocks `cargo publish`; PLAN-2026-0002 G-PUB);
depend on the git tag:

```toml
[dependencies]
rivet = { package = "rivet-runtime", git = "https://github.com/olivierdevelops/rivet", tag = "v0.2.0" }
# lean: only the serve surfaces
# rivet = { package = "rivet-runtime", git = "https://github.com/olivierdevelops/rivet", tag = "v0.2.0",
#           default-features = false, features = ["serve"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

| Cargo feature | Default | Enables | Without it |
|---|---|---|---|
| `serve` | yes | `rivet serve`: HTTP/SSE/polling/WebSocket/MCP on one listener (axum) | `rivet serve` → `unsupported.feature` (`details.feature: "serve"`) |
| `grpc` | yes | `connector … grpc` and `grpc CONNECTOR.Method` effects | a bundle using them fails at load: `unsupported.feature` (`grpc`) |
| `quic` | yes | `with quic …` and HTTP/3 (`version 3`, `version prefer [3, …]`) | load fails: `unsupported.feature` (`quic`) |
| `oauth` | yes | `auth NAME oauth2` profiles and `store keychain` | load fails: `unsupported.feature` (`oauth`) |
| `cli` | no | the `rivet` binary (clap) | no binary (the library is unaffected) |

```text
 bundle uses grpc/quic/oauth ──▶ Runtime::builder().build()
                                    │ feature compiled in? ── yes ─▶ Ok(Runtime)
                                    └ no ─▶ Err(unsupported.feature) — every use listed (first + suppressed),
                                            before anything runs; exit 5 / HTTP 501 on the other surfaces
 rivet::build_features()  == rivet.capabilities data.build_features  (e.g. ["serve","grpc","quic","oauth"])
```

Edition 2024, Rust ≥ 1.90; an async host needs Tokio (multi-thread runtime). Platforms: macOS and Linux (CI green on
both); Windows is not supported in 0.2.0 ([INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)).
Non-Rust hosts use the C ABI of `librivet` ([API-2026-0007](api-2026-0007-c-abi.md)).

## Audience and Stability

Rust hosts embedding Rivet. The **facade** — the items re-exported at the crate root and in `rivet::types`, and the methods listed here — is the supported 0.2.x surface:

```text
 rivet::{Runtime, RuntimeBuilder, Module, Scope, StreamHandle, DuplexHandle, DuplexSender,
         InputEnvelope, ResponseEnvelope, EnvelopeStatus, RecordType, OutputFormat,
         Envelope, Completion, DataEvent, DataSink, Policy, Value, Error, ErrorKind, Result,
         highlight, build_features, VERSION, ABI_VERSION}
 rivet::types::{GraphQuery, Catalog, OutputReport, Principal, RegistryEntry,
                IoQuery, IoReport, PolicyDraft, ModuleSummary, SessionLimits, SourceSpan}
 rivet::internal::…   #[doc(hidden)] — Rivet's own five folders; not stable, may change in any release
```

Some `Runtime` methods take or return types that are not re-exported (the session inputs and receipts,
`Request`, `TraceResult`, `Permit`, `CallGraph`, `ServeOptions`); they are reachable only under
`rivet::internal::…` and carry no stability promise in 0.2.x. The 0.1.0 paths (`rivet::domain::Value`, …) moved;
[MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md#rust-library) has the full path table.

## Authentication

In-process calls run as `Principal::local()` (`name: "local"`, `authenticated_by: "none"`), which `serve.principals` always allows. Hosts that act on behalf of callers use `request_as(principal, …)` or `dispatch_request(Request{principal, …})`; `serve.principals` then applies exactly as on the network surfaces. An embedded `serve` may replace `serve.auth` with its own `Authenticator` (`ServeOptions.authenticator`).

## Endpoints or Events

### Building

| Item | Signature | Notes |
|---|---|---|
| `Runtime::builder()` | `-> RuntimeBuilder` | |
| `RuntimeBuilder::file` | `(self, path: &str) -> Self` | Reads the entry `.rivet` file (bootstrap I/O). policy.json beside it is discovered. |
| `RuntimeBuilder::source` | `(self, path: &str, text: &str, root: &str) -> Self` | In-memory source; `root` anchors relative paths. **No policy is discovered**: without `.policy…` the runtime is deny-by-default. |
| `RuntimeBuilder::root` | `(self, dir: &str) -> Self` | **0.2.0.** A runtime with no entry file (empty catalog): operations arrive later through `rt.load`; `dir` confines module paths. |
| `RuntimeBuilder::policy_file` | `(self, path: &str) -> Self` | Exactly one policy file (like `--policy PATH`). Missing file → `policy.invalid`. |
| `RuntimeBuilder::policy` | `(self, policy: Policy) -> Self` | An already-parsed policy. |
| `RuntimeBuilder::ceiling` | `(self, ceiling: Policy) -> Self` | **Host ceiling.** Every attempt must be allowed by the loaded policy **and** the ceiling; a deny in either wins and `limits` narrow to the smaller value. Calling it twice intersects both ceilings. A ceiling denial reads `… denied: host ceiling: no grant for …`. |
| `RuntimeBuilder::session_limits` | `(self, limits: SessionLimits) -> Self` | Host session caps (per-principal count, queue frames/bytes, idle lease, retention, wait/max_events). |
| `RuntimeBuilder::connector_discovery` | `(self) -> Self` | Loads MCP connectors whose snapshot is missing/unapproved without their imports (for `sync_connector`). |
| `RuntimeBuilder::build` | `(self) -> rivet::Result<Runtime>` | Compiles (entry file and its `import`s), loads policy, applies the ceiling, loads connector snapshots and gRPC descriptors (bootstrap reads), wires adapters; a compiled-out adapter is `unsupported.feature`. |
| `Policy::from_file` | `(path: &str) -> rivet::Result<Policy>` | Strict schema v1; relative targets resolve against the file's directory; its sha256 is the policy hash. |
| `Policy::from_json` | `(bytes: &[u8]) -> rivet::Result<Policy>` | Strict schema v1 from memory; relative targets resolve against the bundle root of the runtime it is given to. |
| `rivet::internal::orchestrator::runtime::policy_from_json` | `(bytes: &[u8], base_dir: &str) -> rivet::Result<Policy>` | Same, with an explicit base directory (not in the facade). |

```text
  policy source decision in build()                        then: effective = policy ∩ ceiling?
  ┌───────────────────────┬───────────────────────────────────────────────┐
  │ .policy(Policy)       │ used as given                                 │
  │ .policy_file(p)       │ load p (strict v1)                            │
  │ .file(entry), neither │ discover policy.json beside entry, else deny  │
  │ .source(…),  neither  │ deny-by-default (pure operations still run)   │
  │ .root(dir),  neither  │ deny-by-default; modules loaded later share it│
  └───────────────────────┴───────────────────────────────────────────────┘
  per request:  effective ∩ restrict₁ ∩ restrict₂ …   (request_restricted / nested restrictions)
```

`Runtime` is `Clone` (an `Arc` inside); clones share catalog, broker, sessions and trace store.

### Calls and requests

`call` is the **envelope** entry point (one shape for every outcome, the same JSON as the CLI, HTTP, MCP and the C
ABI); `request` is the **Rust-idiomatic** entry point (`?` on `rivet::Result<Completion>`). Both run the same dispatcher.

| Method | Signature |
|---|---|
| `call` | **0.2.0.** `async (&self, input: InputEnvelope) -> ResponseEnvelope` — as `local`; never returns `Err`: failures are envelopes with `status: "error"` |
| `call_json` | **0.2.0.** `async (&self, input_json: &str) -> ResponseEnvelope` — parses the JSON InputEnvelope with the HTTP parser (`validation.input_envelope`, `validation.required`, deprecated `id`/`params` accepted) |
| `request` | `async (&self, operation_id: &str, params: Value, sink: Option<Arc<dyn DataSink>>) -> rivet::Result<Completion>` — as `local` |
| `request_restricted` | `async (&self, operation_id: &str, params: Value, restrict: Value, sink: Option<Arc<dyn DataSink>>) -> rivet::Result<Completion>` — `restrict` is `{"grants": [...]}` in the policy.json grant format, intersected with the effective policy for this request and its nested calls only (narrows, never widens; malformed → `policy.invalid` with a `/restrict` pointer) |
| `request_as` | `async (&self, principal: Principal, operation_id: &str, params: Value, sink: Option<Arc<dyn DataSink>>) -> rivet::Result<Completion>` |
| `new_request` | `(&self, operation_id: &str, params: Value, principal: Principal) -> Request` — fresh `request_id`/`trace_id`, default deadline 30000 ms (`Request` is internal) |
| `dispatch_request` | `async (&self, req: Request, sink: Option<Arc<dyn DataSink>>) -> rivet::Result<Completion>` — set `req.deadline_ms`, `req.restrict` etc. first |
| `cancel` | `(&self, request_id: &str, principal: Principal) -> rivet::Result<CancelReceipt>` — cancel one of that principal's running top-level requests (idempotent) |
| `shutdown` | `async (&self, drain: Duration)` — cancel every running request and session and wait up to `drain` |

| Type | Construct / read |
|---|---|
| `InputEnvelope` | `InputEnvelope::new("demo.add").data(json!({"a":2}))`, `.data_value(Value)`, `.deadline_ms(ms)`, `.restrict(Value)`; `.is_legacy()`; `.to_json()` |
| `ResponseEnvelope` | `.status()` → `EnvelopeStatus::{Ok, Error, Cancelled, Accepted}`; `.to_json()` (`serde_json::Value`), `.to_json_string()` (compact), `.to_json_pretty()` (2-space); `ResponseEnvelope::from_json(&json)`; `ResponseEnvelope::from(completion)` |
| `Value` | `Value::from_json(&serde_json::Value)`, `Value::object([...])`, `Value::text(..)`; back with `.to_json()` |

```text
 rt.call(InputEnvelope)
   └─▶ same dispatcher as HTTP ─▶ Completion ──┐
                                  RivetError ──┼─▶ ResponseEnvelope {request_id, trace_id, operation, type:"result",
                                               │                     status, data, error, effects, data_count}
 rt.request(id, v, sink) ─▶ Ok(Completion) | Err(rivet::Error)   (same outcome, Rust-shaped)
```

**Streaming** with `request` uses a `DataSink`:

```rust
#[async_trait::async_trait]
pub trait DataSink: Send + Sync {
    async fn send(&self, event: rivet::DataEvent) -> rivet::Result<()>;
}
```

| Sink returns | Effect |
|---|---|
| `Ok(())` | keep producing |
| `Err(rivet::Error::consumer_stop())` | **typed stop**: the producer stops, cleanup runs and the request ends `cancelled` / `consumer.stop` — not a failure |
| any other `Err(e)` | the request fails `consumer_failed` |

Each emitted item arrives in order as `DataEvent {request_id, trace_id, seq, data}`; the `Completion` is returned after the last item. Cancellation is **structured**: `cancel`, a deadline or `shutdown` fires the request's token, the interpreter unwinds at its next await point and closes every `with` handle in reverse order within the 5 s grace (child processes are reaped); the future is dropped only if the run ignores the grace. Dropping the request future also releases its whole scope.

### Scopes: owned streams and duplex handles

`Runtime::scope` runs a body with a `Scope`; every stream or duplex the scope starts is cancelled (if still running) and joined when the body returns, and aborted if the scope future is dropped.

| Item | Signature |
|---|---|
| `Runtime::scope` | `async (&self, body: F) -> rivet::Result<T>` where `F: FnOnce(Scope) -> Fut`, `Fut: Future<Output = rivet::Result<T>>` |
| `Scope::stream` | `async (&self, id: &str, params: Value) -> rivet::Result<StreamHandle>` — an operation that `emits` |
| `Scope::duplex` | `async (&self, id: &str, params: Value) -> rivet::Result<DuplexHandle>` — an operation that `receives` |
| `StreamHandle::next` | `async (&mut self) -> rivet::Result<Option<Envelope>>` — `Data` items in order, one `Result`, then `None`; a terminal error is returned as `Err`; `env.record()` renders any `Envelope` as a stream-record `ResponseEnvelope` |
| `DuplexHandle::send` | `async (&mut self, item: Value) -> rivet::Result<()>` — checked against `receives` (`validation.input`); after `finish_send` or the end → `conflict.input_finished` |
| `DuplexHandle::finish_send` / `next` / `into_split` | end input (idempotent) / as `StreamHandle::next` / `(DuplexSender, StreamHandle)` for two tasks |
| `request_id()` | on both handles |

```text
 rt.scope(|scope| async move {
     let mut s = scope.stream("demo.countdown", p).await?;   ─┐ task + request   (16-envelope queue)
     while let Some(env) = s.next().await? { … }               │ Data … Result
     let mut d = scope.duplex("chat.echo", p).await?;        ─┤ task + input feed (16 items)
     d.send(v).await?; d.finish_send(); d.next().await?;       │
     Ok(())                                                    │
 }).await   ── body done ─▶ cancel unfinished requests ─▶ join (≤ 5 s each, then abort) ──┘
```

### Modules (`Runtime::load`, 0.2.0)

A `.rivet` file can be loaded into a running runtime as an **object of operations**, namespaced by an alias. It
shares the runtime's policy (a module's own `policy.json` is ignored: `check.module_policy_ignored` warning), its
catalog and its dispatcher; see the language guide's modules chapter
([MAN-2026-0003](../manuals/man-2026-0003-language-guide.md)) for `import … as` inside `.rivet` files.

| Method | Signature |
|---|---|
| `Runtime::load` | `(&self, path: &str) -> rivet::Result<Module>` — alias = the file stem (`./users.rivet` → `users`) |
| `Runtime::load_as` | `(&self, path: &str, alias: &str) -> rivet::Result<Module>` — an existing alias → `check.import_duplicate`; a path outside the root → `permission.import_outside_root` |
| `Runtime::catalog` | `(&self) -> Arc<CatalogSnapshot>` — the current catalog snapshot (a request and its nested calls keep the snapshot they started on) |
| `Module::alias` / `file` / `summary` / `warnings` | the alias, the file, a `ModuleSummary`, load warnings (`check.module_policy_ignored`) |
| `Module::operations` | `-> Vec<RegistryEntry>` — public operations with **short** IDs (`invoice`) |
| `Module::qualified` | `(&self, id: &str) -> String` — `invoice` → `billing.invoice` |
| `Module::describe` / `outputs` | `(&self, id: &str) -> rivet::Result<RegistryEntry / OutputReport>` |
| `Module::call` | `async (&self, id: &str, data: serde_json::Value) -> ResponseEnvelope` — the envelope's `operation` is namespaced (`billing.invoice`) |
| `Module::stream` / `duplex` | `async (&self, scope: &Scope, id: &str, data: Value) -> rivet::Result<StreamHandle / DuplexHandle>` — scope-owned, like `Scope::stream` |

```text
 Runtime::builder().root(dir).build()          (empty catalog, one policy)
   ├─ rt.load("./users.rivet")                 ─▶ Module "users"    users.get, users.list
   ├─ rt.load_as("./lib/billing.rivet","billing") ─▶ Module "billing"  billing.invoice
   │     loads are serialized; each swaps in a new catalog snapshot
   ├─ billing.call("invoice", {user: 7})       ─▶ ResponseEnvelope{operation:"billing.invoice", …}
   ├─ rt.call(InputEnvelope::new("billing.invoice"))   same operation through the runtime
   └─ rt.load_as(…, "billing") again           ─▶ Err(check.import_duplicate), catalog unchanged
```

### Highlighting (`rivet::highlight`, 0.2.0)

| Function | Signature |
|---|---|
| `highlight::tokens` | `(source: &str) -> Result<Vec<HighlightToken>, (Vec<HighlightToken>, rivet::Error)>` — on a syntax error, the tokens before it plus the error |
| `highlight::tokens_of` | `(path: &str, source: &str) -> …` — same, with `path` in the diagnostic |
| `highlight::highlight` | `(source: &str, format: HighlightFormat) -> Result<String, (String, rivet::Error)>` — ANSI, HTML or JSON lines, as `rivet highlight` |

Token classes, `HighlightToken {line, col, len, class, text}` and `HighlightFormat` are described in
[MAN-2026-0010](../manuals/man-2026-0010-editor-support-and-highlighting.md).

### Build information

| Item | Value |
|---|---|
| `rivet::VERSION` | the workspace version (`"0.2.0"` in the release; `"0.1.0"` on the captures below) |
| `rivet::ABI_VERSION` | the C ABI version of `librivet` (`1`) |
| `rivet::build_features()` | `Vec<&'static str>` of the compiled Cargo features (the library's own: `serve`, `grpc`, `quic`, `oauth`) |

### Sessions (duplex and resumable streams)

The session methods remain on `Runtime`, but their input and output types (`SessionOpenInput`, `SessionAck`, …)
live under `rivet::internal::domain::sessions` and are not part of the stable facade in 0.2.x; prefer
`Runtime::scope` in Rust and the network surfaces or the C ABI's call handles elsewhere.


| Method | Input → Output |
|---|---|
| `open_session` | `SessionOpenInput {id, params, principal, connection_owned, deadline_ms, trace, restrict}` → `SessionReceipt` (the Rust struct keeps its 0.1.0 field names) |
| `send_input` | `SessionSendInput {session_id, send_seq, data, principal}` → `SessionAck` |
| `finish_input` | `SessionRef {session_id, principal}` → `SessionAck` |
| `read_events` | `SessionReadInput {session_id, after_seq, max_events, wait_ms, principal}` → `SessionBatch` |
| `cancel_session` | `SessionRef` → `CancelReceipt` (a finished session reports its terminal state) |
| `sessions` | `-> Arc<dyn SessionDriver>` (the driver shared with polling, WebSocket and MCP) |

A `receives` operation iterates its input as `incoming`; items sent with `send_input` feed it in `send_seq` order and are validated against `receives`. `deadline_ms` defaults to 30000 and is capped at 600000; `trace` is an optional W3C `TraceContext`; `restrict` narrows as in `request_restricted`. Limits are those of [polling sessions](api-2026-0001-http-rest-sse-polling.md#polling-sessions), including the host byte budget `limits.max_buffered_bytes` (`limit.buffered_bytes`).

### Catalog, audit and connectors

| Method | Signature |
|---|---|
| `list` | `(&self) -> rivet::Result<Catalog>` — public entries (modules included, namespaced) |
| `describe` | `(&self, ids: &[String]) -> rivet::Result<Catalog>` |
| `outputs` | `(&self, id: Option<&str>, all: bool) -> rivet::Result<Vec<OutputReport>>` |
| `graph` | `(&self, query: &GraphQuery) -> rivet::Result<CallGraph>` — `GraphQuery {id, all}`; the static call graph `rivet graph` prints |
| `catalog_version` | `(&self) -> String` — `sha256:` of the bundle sources |
| `io` | `(&self, query: &IoQuery) -> rivet::Result<IoReport>` — the I/O manifest (sync; reads only catalog and policy) |
| `generate_policy` | `(&self, ids: &[&str]) -> rivet::Result<PolicyDraft>` — empty = every public op; never writes |
| `generate_policy_draft` | `(&self, ids: &[String], all: bool, output: Option<&str>) -> rivet::Result<PolicyDraft>` — `output` creates a new file, refusing to overwrite |
| `trace` | `(&self, request_id: &str) -> rivet::Result<TraceResult>` — this process's recorded broker decisions |
| `export_trace` | `async (&self, request_id: &str, path: &str) -> rivet::Result<TraceExport>` — writes the sanitized trace JSON to a **new** bundle-relative file through the broker (`allow_write` access `create`); an existing file is `conflict.already_exists`, an unknown request `not_found.trace`. `TraceExport {request_id, path, events, bytes}` |
| `decisions` | `(&self) -> Vec<Permit>` — every broker decision so far |
| `policy` | `(&self) -> &Policy` — the effective policy (ceiling applied) |
| `sync_connector` | `async (&self, name: &str, output: &str) -> rivet::Result<ConnectorSync>` — refuses an existing `output` before contacting the server, then discovers and exclusively creates a candidate snapshot |
| `connector_imports` | `(&self) -> Vec<String>` |

### Embedding `serve`

`rivet::internal::orchestrator::setup_serve::start(rt, ServeOptions {listen: Some("127.0.0.1:0".into()), ..ServeOptions::default()}).await? -> ServeHandle` (needs the `serve` feature; **not in the facade** in 0.2.0 — the path is internal and may move) binds one listener with every enabled surface (plus `GET /v1/health`); `ServeOptions` also takes `stdio`, an `authenticator` that replaces `serve.auth`, and an `access_log` sink (`Arc<dyn Fn(&str) + Send + Sync>`, default stderr) that receives one JSON line per request. `handle.addr` is the bound address and `handle.shutdown().await` drains like SIGTERM (cancel in-flight requests and sessions, up to 6 s). The same start-up refusals apply (`serve.auth_required`, `unsupported.serve_mtls`).

## Request Format

`call` takes an `InputEnvelope` (`operation`, `data`, `deadline_ms?`, `restrict?`); `call_json` takes its JSON text ([API-2026-0006](api-2026-0006-envelopes.md#inputenvelope)). The `data` / `params` are validated exactly as on HTTP: unknown fields (`validation.unknown_field`), missing required (`validation.required`), wrong types (`validation.type`), `min`/`max`/`enum`, then defaults are injected. Built-in IDs (`rivet.list`, `rivet.io`, …) are accepted by `call` and `request` too.

## Response Format

`call` returns a `ResponseEnvelope` whose JSON is byte-for-byte the HTTP body:

```json
{"request_id":"req_0185444e6d","trace_id":"tr_0185444e6d","operation":"files.read","type":"result","status":"ok","data":"hello","error":null,"effects":"none","data_count":0}
```

`request` returns `Completion {request_id, trace_id, result: Value, data_count: u64, effects: EffectsStatus}` (a Rust
struct; `ResponseEnvelope::from(completion)` gives the envelope). Stream handles yield `Envelope::{Data, Result, Error}`;
`env.record()` renders each as a stream record (`type: "data"` with `seq`, or `type: "result"` with `status`).

## Error Format

`call` never fails: its failures are envelopes with `status: "error"` (or `"cancelled"`), `data: null` and the error object. Every other fallible call returns `rivet::Result<T> = Result<T, rivet::Error>` (`rivet::Error` is the 0.1.0 `RivetError`). It carries `kind: ErrorKind`, `code`, `message`, `retryable`, `effects`, optional `source` span, `operation_id`, `request_id`, `trace_id`, `node_id`, `hint`, `details`, `cause`, `suppressed`. Helpers: `exit_code()`, `http_status()`, `to_value()` (the envelope `error` object), `render(Some(source_text))` (terminal rendering with a caret). Codes and mappings: [API-2026-0005](api-2026-0005-error-registry.md). New in 0.2.0 for the library: `unsupported.feature` at `build()`/`load`, and the module codes `check.import_duplicate`, `permission.import_outside_root`, `not_found.import`, `check.import_cycle`, `check.import_collision`, `limit.imports`.

## Rate Limits

The policy's `limits` apply to library calls as to any surface: `max_concurrent_requests` (top-level; `limit.concurrency`), `max_call_depth` (`limit.call_depth`), and the session limits.

## Versioning and Deprecation

Semantic versioning applies (pre-1.0: a minor bump may break). 0.2.0 is such a minor bump:

```text
 0.1.0  package rivet · rivet::domain::… public · request → Completion
 0.2.0  package rivet-runtime (lib rivet) · facade at the crate root (+ rivet::types) · internals under
        #[doc(hidden)] rivet::internal · call/call_json → ResponseEnvelope · load/load_as → Module ·
        Cargo features · rt.bundle() returns an owned SourceBundle (gains `modules`)
 0.2.x  facade stable; `InputEnvelope` accepts the deprecated id/params (is_legacy() == true)
 0.3.0  id/params refused by call_json
```

[MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md#rust-library) lists every moved path.

## Examples

The program below was compiled and run on 2026-09-29 against the 0.2.0 release candidate (source at `6f9943f`) as a
scratch binary crate with `rivet = { package = "rivet-runtime", path = … }` (default features), `tokio`
(rt-multi-thread, macros), `async-trait` and `serde_json`, from a folder holding `data/a.txt` = `hello`, an empty
`audit/` and `lib/billing.rivet` (one operation `invoice`, `param user integer`, returning `{user: user, total: 1200}`).
It uses only the facade: `call`, `call_json`, `Policy::from_json`, the host ceiling, `scope.stream`, `scope.duplex`,
the typed sink stop, `request_restricted`, `export_trace`, `load_as` / `Module` and the build constants.

```rust
use async_trait::async_trait;
use rivet::{DataEvent, DataSink, InputEnvelope, Policy, Runtime, Value};
use serde_json::json;
use std::sync::Arc;

const APP: &str = r#"
operation demo.countdown
    name "Count down"
    description "Emit 3, 2, 1 as data items and then return a summary."
    output object description "Summary returned after the last item."
        field count integer required description "Number of items emitted."
    end
    emits integer description "One countdown value per item."
    for value in [3, 2, 1]
        emit value
    end
    return {count: 3}
end

operation chat.echo
    name "Echo input"
    description "Emit every received text item back, then return how many were echoed."
    output object description "Summary after input finished."
        field echoed integer required description "Number of items echoed."
    end
    emits text description "One item per input item."
    receives text description "One text item per caller input."
    count = 0
    for item in incoming
        emit item
        count = count + 1
    end
    return {echoed: count}
end

operation files.read
    name "Read a file"
    description "Read one text file under ./data."
    param path text required description "Bundle-relative path."
    output text description "File content."
    return file read path as text
end
"#;

/// Stops after the first item: the request ends `cancelled` / consumer.stop.
struct FirstOnly;

#[async_trait]
impl DataSink for FirstOnly {
    async fn send(&self, event: DataEvent) -> rivet::Result<()> {
        println!("sink got seq={} {}", event.seq, event.data.to_json());
        Err(rivet::Error::consumer_stop())
    }
}

const GRANTS: &[u8] = br#"{"version":1,"grants":[{"capability":"allow_read","targets":["./data/**"],"access":["read"]},
                                   {"capability":"allow_write","targets":["./audit/**"],"access":["create"]}]}"#;

#[tokio::main]
async fn main() -> rivet::Result<()> {
    let policy = Policy::from_json(GRANTS)?;
    // The host ceiling can only narrow: it does not grant ./audit/**, so trace export is denied.
    let ceiling = Policy::from_json(
        br#"{"version":1,"grants":[{"capability":"allow_read","targets":["./data/**"],"access":["read"]}],
             "limits":{"max_concurrent_requests":8}}"#,
    )?;
    let rt = Runtime::builder().source("app.rivet", APP, ".").policy(policy).ceiling(ceiling).build()?;

    // One envelope for every outcome (the same JSON as the CLI, HTTP, MCP and C ABI).
    let ok = rt.call(InputEnvelope::new("files.read").data(json!({"path": "data/a.txt"}))).await;
    println!("call -> {}", ok.to_json_string());
    let bad = rt.call_json(r#"{"operation":"files.read","data":{}}"#).await;
    println!("call_json -> {}", bad.to_json_string());

    // Scope-owned stream and duplex.
    rt.scope(|scope| async move {
        let mut s = scope.stream("demo.countdown", Value::Object(vec![])).await?;
        while let Some(env) = s.next().await? {
            println!("stream {}", env.record().to_json_string());
        }
        let mut d = scope.duplex("chat.echo", Value::Object(vec![])).await?;
        d.send(Value::text("hi")).await?;
        let bad = d.send(Value::Int(5)).await.unwrap_err();
        println!("duplex bad item -> {} {}", bad.code, bad.message);
        d.finish_send();
        while let Some(env) = d.next().await? {
            println!("duplex {}", env.record().to_json_string());
        }
        Ok(())
    })
    .await?;

    // Typed DataSink stop.
    let e = rt.request("demo.countdown", Value::Object(vec![]), Some(Arc::new(FirstOnly))).await.unwrap_err();
    println!("sink stop -> kind={} code={}", e.kind.as_str(), e.code);

    // Rust-idiomatic request + per-request restriction (narrows only).
    let p = Value::from_json(&json!({"path": "data/a.txt"}));
    let c = rt.request("files.read", p.clone(), None).await?;
    println!("read -> {}", c.result.to_json());
    let narrow = Value::from_json(&json!({"grants": [{"capability": "allow_read", "targets": ["./data/other/**"]}]}));
    let e = rt.request_restricted("files.read", p, narrow, None).await.unwrap_err();
    println!("restricted -> {} {}", e.code, e.message);

    // Trace export goes through the broker as allow_write create; the ceiling denies ./audit/**.
    let e = rt.export_trace(&c.request_id, "./audit/trace.json").await.unwrap_err();
    println!("export -> {} {}", e.code, e.message);

    // Without the ceiling the same export writes a NEW file (never overwrites).
    let rt2 = Runtime::builder().source("app.rivet", APP, ".").policy(Policy::from_json(GRANTS)?).build()?;
    let c = rt2.request("files.read", Value::from_json(&json!({"path": "data/a.txt"})), None).await?;
    let receipt = rt2.export_trace(&c.request_id, "./audit/trace.json").await?;
    println!("export -> {}", receipt.to_json());
    let again = rt2.export_trace(&c.request_id, "./audit/trace.json").await.unwrap_err();
    println!("export again -> {}", again.code);

    // Load a .rivet file as a module object (namespaced by its alias).
    let billing = rt2.load_as("./lib/billing.rivet", "billing")?;
    for op in billing.operations() {
        println!("module op -> {} ({})", op.id, billing.qualified(&op.id));
    }
    println!("module call -> {}", billing.call("invoice", json!({"user": 7})).await.to_json_string());
    println!("same via rt.call -> {}", rt2.call(InputEnvelope::new("billing.invoice").data(json!({"user": 7}))).await.to_json_string());
    if let Err(e) = rt2.load_as("./lib/billing.rivet", "billing") {
        println!("load again -> {} {}", e.code, e.message);
    }
    println!("build_features -> {:?}  VERSION={} ABI_VERSION={}", rivet::build_features(), rivet::VERSION, rivet::ABI_VERSION);
    Ok(())
}
```

Output (**request and trace IDs differ on every run**):

```text
call -> {"request_id":"req_0185444e6d","trace_id":"tr_0185444e6d","operation":"files.read","type":"result","status":"ok","data":"hello","error":null,"effects":"none","data_count":0}
call_json -> {"request_id":"req_0204b3000a","trace_id":"tr_0204b3000a","operation":"files.read","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.required","message":"missing required parameter `path`","retryable":false,"operation_id":"files.read","details":{"field":"path"}},"effects":"none","data_count":0}
stream {"request_id":"req_0387f7e80f","trace_id":"tr_0387f7e80f","operation":"demo.countdown","type":"data","seq":1,"data":3,"error":null}
stream {"request_id":"req_0387f7e80f","trace_id":"tr_0387f7e80f","operation":"demo.countdown","type":"data","seq":2,"data":2,"error":null}
stream {"request_id":"req_0387f7e80f","trace_id":"tr_0387f7e80f","operation":"demo.countdown","type":"data","seq":3,"data":1,"error":null}
stream {"request_id":"req_0387f7e80f","trace_id":"tr_0387f7e80f","operation":"demo.countdown","type":"result","status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}
duplex bad item -> validation.input input item at $ must be text, got integer
duplex {"request_id":"req_0407055b14","trace_id":"tr_0407055b14","operation":"chat.echo","type":"data","seq":1,"data":"hi","error":null}
duplex {"request_id":"req_0407055b14","trace_id":"tr_0407055b14","operation":"chat.echo","type":"result","status":"ok","data":{"echoed":1},"error":null,"effects":"none","data_count":1}
sink got seq=1 3
sink stop -> kind=cancelled code=consumer.stop
read -> "hello"
restricted -> permission.denied allow_read read on data/a.txt denied: request restriction: no grant for allow_read data/a.txt
export -> permission.denied allow_write create on ./audit/trace.json denied: host ceiling: no grant for allow_write ./audit/trace.json
export -> {"request_id":"req_018511dea5","path":"./audit/trace.json","events":1,"bytes":649}
export again -> conflict.already_exists
module op -> invoice (billing.invoice)
module call -> {"request_id":"req_02044144aa","trace_id":"tr_02044144aa","operation":"billing.invoice","type":"result","status":"ok","data":{"user":7,"total":1200},"error":null,"effects":"none","data_count":0}
same via rt.call -> {"request_id":"req_0387094957","trace_id":"tr_0387094957","operation":"billing.invoice","type":"result","status":"ok","data":{"user":7,"total":1200},"error":null,"effects":"none","data_count":0}
load again -> check.import_duplicate a module named `billing` is already loaded; use load_as(path, alias)
build_features -> ["serve", "grpc", "quic", "oauth"]  VERSION=0.1.0 ABI_VERSION=1
```

```text
  what happened, step by step
  build(policy ∩ ceiling) ─▶ call files.read ✓ envelope ─▶ call_json {} ✗ envelope (validation.required, no Err)
                           ─▶ scope: stream demo.countdown ─▶ 3 × data, result {count:3}
                           ─▶ scope: duplex chat.echo ─▶ send "hi" ✓, send 5 ✗ validation.input,
                                                         finish_send ─▶ data "hi", result {echoed:1}
                           ─▶ request + FirstOnly sink ─▶ stop after seq 1 ─▶ cancelled / consumer.stop
                           ─▶ files.read ✓ ─▶ request_restricted(./data/other/**) ✗ request restriction
                           ─▶ export_trace ✗ host ceiling (no allow_write ./audit/**)
  rt2 (no ceiling)         ─▶ export_trace ✓ new file ─▶ again ✗ conflict.already_exists
                           ─▶ load_as billing ─▶ Module call ✓ billing.invoice ─▶ load_as again ✗ import_duplicate
```

Library stream records rendered with `env.record()` have **no `seq` on the terminal `result` record**, unlike the CLI,
SSE, WebSocket, polling and C ABI terminal records (API-2026-0006 compatibility note; a P4 finding).

The compiled examples in the repository show the same facade: [`examples/embed.rs`](../../examples/embed.rs)
(`cargo run --example embed`, the compiled form of [demo 12-library](../demos/12-library/README.md)) and
[`examples/modules.rs`](../../examples/modules.rs) (`cargo run --example modules`).

## Compatibility Notes

The library differs from the approved design sketches in these details; the code is authoritative:

| Design sketch | As implemented |
|---|---|
| `.source(src)` | `.source(path, text, root)`. |
| `rt.request("id", json!({…}), None)` | params are `rivet::Value` (`Value::from_json(&json!({…}))`); `rt.call(InputEnvelope::new("id").data(json!({…})))` takes JSON directly. |
| Sink returning `Continue` / `Stop` | `DataSink::send` returns `Ok(())` to continue or `Err(rivet::Error::consumer_stop())` to stop (the request ends `cancelled` / `consumer.stop`); any other `Err` is `consumer_failed`. |
| `rt.outputs("id") -> OutputSpec` | `rt.outputs(Some("id"), false) -> Vec<OutputReport>`. |
| per-request restriction as a request option | `rt.request_restricted(id, params, restrict, sink)`, `InputEnvelope::restrict`, or `Request.restrict` with `dispatch_request`. |
| `module.stream(id, data)` (PROP-2026-0002) | `module.stream(&scope, id, data)` — streams are scope-owned (structured concurrency). |
| `module.operations()` with namespaced IDs | short IDs (`invoice`); `module.qualified(id)` and the envelope's `operation` are namespaced (`billing.invoice`). |

`Runtime::scope` (`stream`/`duplex`), `Policy::from_file`/`Policy::from_json`, `.ceiling(Policy)` and W3C trace context (`SessionOpenInput.trace`, `Runtime::new_request_traced`) are provided as designed. The trace store is in-memory per `Runtime` (no persistent trace store).

## Related Documents

- [API index](index.md) · [HTTP API](api-2026-0001-http-rest-sse-polling.md) · [Error registry](api-2026-0005-error-registry.md) · [Envelopes](api-2026-0006-envelopes.md) · [C ABI](api-2026-0007-c-abi.md)
- [MAN-2026-0007 embedding guide](../manuals/man-2026-0007-embedding-library.md) · [MIG-2026-0001 migration](../migrations/mig-2026-0001-response-and-input-envelopes.md) · [ADR-0005 workspace and features](../decisions/adr-0005-workspace-package-and-features.md) · [DEMO-2026-0019](../demos/17-modules/README.md)
- [Runtime architecture](../architecture/arch-2026-0001-rivet-runtime-architecture.md) · [Demo 12-library](../demos/12-library/README.md)
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) · [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 4 | 2026-09-30 | Claude | v0.2.0 release (PLAN-2026-0002 TASK-091): version strings and current-release wording updated to 0.2.0. |
| 1 | 2026-09-28 | Claude | Initial library contract; example compiled and run against the crate at commit f40d4aa. |
| 2 | 2026-09-28 | Claude | Fix batch through 829ca43: `Runtime::scope` with `stream`/`duplex` handles, `Policy::from_file`/`from_json`, `.ceiling(Policy)`, `session_limits`, typed `DataSink` stop (`consumer.stop`), structured cancellation, `request_restricted`, `export_trace`, `graph`, `shutdown`, `SessionOpenInput.trace/restrict`, `ServeOptions.access_log`; new example compiled and run at 829ca43; compatibility table reduced to the remaining differences. |
| 3 | 2026-09-29 | Claude | 0.2.0 (D-43, TASK-070): package `rivet-runtime` as a git-tag dependency; Cargo features table and `unsupported.feature`; the crate-root facade and `rivet::types` (internals under `rivet::internal`); `Runtime::call` / `call_json` envelopes; `RuntimeBuilder::root`, `Runtime::load` / `load_as` and `Module`; `rivet::highlight`; `VERSION` / `ABI_VERSION` / `build_features`; `rivet::Result` / `rivet::Error`; new example compiled and run against the 0.2.0-rc (source `6f9943f`); compatibility table updated; platform note. |
