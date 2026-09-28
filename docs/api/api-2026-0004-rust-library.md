---
document_id: API-2026-0004
title: "Rivet Rust library API"
document_type: api
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 2
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
scope: The public Rust API of the `rivet` crate as implemented — `Runtime` / `RuntimeBuilder`, `policy_from_json`, request and dispatch, streaming through `DataSink`, sessions for duplex, catalog, I/O manifest, policy drafts, traces, cancellation and embedding `serve`.
reason: DOCUMENTATION.md §31 API impact for PLAN-2026-0001 row D-27; the library is one of the five access points and needs a verified contract.
related_documents: [PLAN-2026-0001, PROP-2026-0001, API-2026-0001, API-2026-0005, ARCH-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, api, rust, library, embedding]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.1.0-dev (commit 829ca43)"
next_review_date: 2026-10-28
---

# Rivet Rust library API

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** library, execution, sessions, policy, registry, audit, serve

## Summary

The Rust library is the same runtime the CLI and `rivet serve` use — not a client. A host builds one `Runtime` per bundle and calls the **same dispatcher** every other surface calls, so validation, the policy broker, limits, output checks and error codes are identical.

```text
   host process (tokio)
   ┌─────────────────────────────────────────────────────────────────────────────┐
   │  Runtime::builder()                                                         │
   │     .file("app.rivet")  | .source(path, text, root)                         │
   │     .policy_file(p)     | .policy(Policy::from_file(p)? | from_json(b)?)    │
   │     .ceiling(Policy)    host ceiling: every attempt needs policy ∩ ceiling  │
   │     .build()?  ─────────────▶ compile → load policy → wire adapters         │
   │                                                                             │
   │  rt.request(id, params, sink)          ─▶ Completion   (DataSink may Stop)  │
   │  rt.request_restricted(id, p, restrict, sink) ─▶ narrowed for this request  │
   │  rt.scope(|scope| … scope.stream / scope.duplex …)  ─▶ owned, joined        │
   │  rt.open_session / send_input / finish_input / read_events / cancel_*       │
   │  rt.list / describe / outputs / graph   ─▶ catalog and static call graph    │
   │  rt.io / generate_policy / trace / export_trace ─▶ audit                    │
   │  setup_serve::start(rt, opts)          ─▶ embed every network surface       │
   └─────────────────────────────────────────────────────────────────────────────┘
```

The crate is `rivet` 0.1.0 (edition 2024, Rust ≥ 1.90), used as a path or git dependency; an async host needs Tokio (multi-thread runtime).

## Audience and Stability

Rust hosts embedding Rivet. The items re-exported at the crate root (`rivet::Runtime`, `rivet::RuntimeBuilder`) and the methods listed here are the supported 0.1.0 surface. The crate also exposes its five internal modules (`domain`, `features`, `infra`, `io`, `orchestrator`) as `pub`; items not listed in this document may change without notice.

## Authentication

In-process calls run as `Principal::local()` (`name: "local"`, `authenticated_by: "none"`), which `serve.principals` always allows. Hosts that act on behalf of callers use `request_as(principal, …)` or `dispatch_request(Request{principal, …})`; `serve.principals` then applies exactly as on the network surfaces. An embedded `serve` may replace `serve.auth` with its own `Authenticator` (`ServeOptions.authenticator`).

## Endpoints or Events

### Building

| Item | Signature | Notes |
|---|---|---|
| `Runtime::builder()` | `-> RuntimeBuilder` | |
| `RuntimeBuilder::file` | `(self, path: &str) -> Self` | Reads the entry `.rivet` file (bootstrap I/O). policy.json beside it is discovered. |
| `RuntimeBuilder::source` | `(self, path: &str, text: &str, root: &str) -> Self` | In-memory source; `root` anchors relative paths. **No policy is discovered**: without `.policy…` the runtime is deny-by-default. |
| `RuntimeBuilder::policy_file` | `(self, path: &str) -> Self` | Exactly one policy file (like `--policy PATH`). Missing file → `policy.invalid`. |
| `RuntimeBuilder::policy` | `(self, policy: Policy) -> Self` | An already-parsed policy. |
| `RuntimeBuilder::ceiling` | `(self, ceiling: Policy) -> Self` | **Host ceiling.** Every attempt must be allowed by the loaded policy **and** the ceiling; a deny in either wins and `limits` narrow to the smaller value. Calling it twice intersects both ceilings. A ceiling denial reads `… denied: host ceiling: no grant for …`. |
| `RuntimeBuilder::session_limits` | `(self, limits: SessionLimits) -> Self` | Host session caps (per-principal count, queue frames/bytes, idle lease, retention, wait/max_events). |
| `RuntimeBuilder::connector_discovery` | `(self) -> Self` | Loads MCP connectors whose snapshot is missing/unapproved without their imports (for `sync_connector`). |
| `RuntimeBuilder::build` | `(self) -> RivetResult<Runtime>` | Compiles, loads policy, applies the ceiling, loads connector snapshots and gRPC descriptors (bootstrap reads), wires adapters. |
| `Policy::from_file` | `(path: &str) -> RivetResult<Policy>` | Strict schema v1; relative targets resolve against the file's directory; its sha256 is the policy hash. |
| `Policy::from_json` | `(bytes: &[u8]) -> RivetResult<Policy>` | Strict schema v1 from memory; relative targets resolve against the bundle root of the runtime it is given to. |
| `rivet::orchestrator::runtime::policy_from_json` | `(bytes: &[u8], base_dir: &str) -> RivetResult<Policy>` | Same, with an explicit base directory. |

```text
  policy source decision in build()                        then: effective = policy ∩ ceiling?
  ┌───────────────────────┬───────────────────────────────────────────────┐
  │ .policy(Policy)       │ used as given                                 │
  │ .policy_file(p)       │ load p (strict v1)                            │
  │ .file(entry), neither │ discover policy.json beside entry, else deny  │
  │ .source(…),  neither  │ deny-by-default (pure operations still run)   │
  └───────────────────────┴───────────────────────────────────────────────┘
  per request:  effective ∩ restrict₁ ∩ restrict₂ …   (request_restricted / nested restrictions)
```

`Runtime` is `Clone` (an `Arc` inside); clones share catalog, broker, sessions and trace store.

### Requests

| Method | Signature |
|---|---|
| `request` | `async (&self, operation_id: &str, params: Value, sink: Option<Arc<dyn DataSink>>) -> RivetResult<Completion>` — as `local` |
| `request_restricted` | `async (&self, operation_id: &str, params: Value, restrict: Value, sink: Option<Arc<dyn DataSink>>) -> RivetResult<Completion>` — `restrict` is `{"grants": [...]}` in the policy.json grant format, intersected with the effective policy for this request and its nested calls only (narrows, never widens; malformed → `policy.invalid` with a `/restrict` pointer) |
| `request_as` | `async (&self, principal: Principal, operation_id: &str, params: Value, sink: Option<Arc<dyn DataSink>>) -> RivetResult<Completion>` |
| `new_request` | `(&self, operation_id: &str, params: Value, principal: Principal) -> Request` — fresh `request_id`/`trace_id`, default deadline 30000 ms |
| `dispatch_request` | `async (&self, req: Request, sink: Option<Arc<dyn DataSink>>) -> RivetResult<Completion>` — set `req.deadline_ms`, `req.restrict` etc. first |
| `cancel` | `(&self, request_id: &str, principal: Principal) -> RivetResult<CancelReceipt>` — cancel one of that principal's running top-level requests (idempotent) |
| `shutdown` | `async (&self, drain: Duration)` — cancel every running request and session and wait up to `drain` |

`params` is `rivet::domain::Value` (`Value::from_json(&serde_json::Value)`, `Value::object([...])`, `Value::text(..)`); results convert back with `.to_json()`. **Streaming** uses a `DataSink`:

```rust
#[async_trait::async_trait]
pub trait DataSink: Send + Sync {
    async fn send(&self, event: DataEvent) -> RivetResult<()>;
}
```

| Sink returns | Effect |
|---|---|
| `Ok(())` | keep producing |
| `Err(RivetError::consumer_stop())` | **typed stop**: the producer stops, cleanup runs and the request ends `cancelled` / `consumer.stop` — not a failure |
| any other `Err(e)` | the request fails `consumer_failed` |

Each emitted item arrives in order as `DataEvent {request_id, trace_id, seq, data}`; the `Completion` is returned after the last item. Cancellation is **structured**: `cancel`, a deadline or `shutdown` fires the request's token, the interpreter unwinds at its next await point and closes every `with` handle in reverse order within the 5 s grace (child processes are reaped); the future is dropped only if the run ignores the grace. Dropping the request future also releases its whole scope.

### Scopes: owned streams and duplex handles

`Runtime::scope` runs a body with a `Scope`; every stream or duplex the scope starts is cancelled (if still running) and joined when the body returns, and aborted if the scope future is dropped.

| Item | Signature |
|---|---|
| `Runtime::scope` | `async (&self, body: F) -> RivetResult<T>` where `F: FnOnce(Scope) -> Fut`, `Fut: Future<Output = RivetResult<T>>` |
| `Scope::stream` | `async (&self, id: &str, params: Value) -> RivetResult<StreamHandle>` — an operation that `emits` |
| `Scope::duplex` | `async (&self, id: &str, params: Value) -> RivetResult<DuplexHandle>` — an operation that `receives` |
| `StreamHandle::next` | `async (&mut self) -> RivetResult<Option<Envelope>>` — `Data` items in order, one `Result`, then `None`; a terminal error is returned as `Err` |
| `DuplexHandle::send` | `async (&mut self, item: Value) -> RivetResult<()>` — checked against `receives` (`validation.input`); after `finish_send` or the end → `conflict.input_finished` |
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

### Sessions (duplex and resumable streams)

| Method | Input → Output |
|---|---|
| `open_session` | `SessionOpenInput {id, params, principal, connection_owned, deadline_ms, trace, restrict}` → `SessionReceipt` |
| `send_input` | `SessionSendInput {session_id, send_seq, data, principal}` → `SessionAck` |
| `finish_input` | `SessionRef {session_id, principal}` → `SessionAck` |
| `read_events` | `SessionReadInput {session_id, after_seq, max_events, wait_ms, principal}` → `SessionBatch` |
| `cancel_session` | `SessionRef` → `CancelReceipt` (a finished session reports its terminal state) |
| `sessions` | `-> Arc<dyn SessionDriver>` (the driver shared with polling, WebSocket and MCP) |

A `receives` operation iterates its input as `incoming`; items sent with `send_input` feed it in `send_seq` order and are validated against `receives`. `deadline_ms` defaults to 30000 and is capped at 600000; `trace` is an optional W3C `TraceContext`; `restrict` narrows as in `request_restricted`. Limits are those of [polling sessions](api-2026-0001-http-rest-sse-polling.md#polling-sessions), including the host byte budget `limits.max_buffered_bytes` (`limit.buffered_bytes`).

### Catalog, audit and connectors

| Method | Signature |
|---|---|
| `list` | `(&self) -> RivetResult<Catalog>` — public entries |
| `describe` | `(&self, ids: &[String]) -> RivetResult<Catalog>` |
| `outputs` | `(&self, id: Option<&str>, all: bool) -> RivetResult<Vec<OutputReport>>` |
| `graph` | `(&self, query: &GraphQuery) -> RivetResult<CallGraph>` — `GraphQuery {id, all}`; the static call graph `rivet graph` prints |
| `catalog_version` | `(&self) -> String` — `sha256:` of the bundle sources |
| `io` | `(&self, query: &IoQuery) -> RivetResult<IoReport>` — the I/O manifest (sync; reads only catalog and policy) |
| `generate_policy` | `(&self, ids: &[&str]) -> RivetResult<PolicyDraft>` — empty = every public op; never writes |
| `generate_policy_draft` | `(&self, ids: &[String], all: bool, output: Option<&str>) -> RivetResult<PolicyDraft>` — `output` creates a new file, refusing to overwrite |
| `trace` | `(&self, request_id: &str) -> RivetResult<TraceResult>` — this process's recorded broker decisions |
| `export_trace` | `async (&self, request_id: &str, path: &str) -> RivetResult<TraceExport>` — writes the sanitized trace JSON to a **new** bundle-relative file through the broker (`allow_write` access `create`); an existing file is `conflict.already_exists`, an unknown request `not_found.trace`. `TraceExport {request_id, path, events, bytes}` |
| `decisions` | `(&self) -> Vec<Permit>` — every broker decision so far |
| `policy` | `(&self) -> &Policy` — the effective policy (ceiling applied) |
| `sync_connector` | `async (&self, name: &str, output: &str) -> RivetResult<ConnectorSync>` — refuses an existing `output` before contacting the server, then discovers and exclusively creates a candidate snapshot |
| `connector_imports` | `(&self) -> Vec<String>` |

### Embedding `serve`

`rivet::orchestrator::setup_serve::start(rt, ServeOptions {listen: Some("127.0.0.1:0".into()), ..ServeOptions::default()}).await? -> ServeHandle` binds one listener with every enabled surface (plus `GET /v1/health`); `ServeOptions` also takes `stdio`, an `authenticator` that replaces `serve.auth`, and an `access_log` sink (`Arc<dyn Fn(&str) + Send + Sync>`, default stderr) that receives one JSON line per request. `handle.addr` is the bound address and `handle.shutdown().await` drains like SIGTERM (cancel in-flight requests and sessions, up to 6 s). The same start-up refusals apply (`serve.auth_required`, `unsupported.serve_mtls`).

## Request Format

Parameters are validated exactly as on HTTP: unknown fields (`validation.unknown_field`), missing required (`validation.required`), wrong types (`validation.type`), `min`/`max`/`enum`, then defaults are injected. Built-in IDs (`rivet.list`, `rivet.io`, …) are accepted by `request` too.

## Response Format

`Completion {request_id, trace_id, result: Value, data_count: u64, effects: EffectsStatus}`; `.to_json()` gives the same JSON as HTTP. Session events serialize as `{request_id, trace_id, seq, type: data|result|error, …}`.

## Error Format

Every fallible call returns `RivetResult<T> = Result<T, RivetError>`. `RivetError` carries `kind: ErrorKind`, `code`, `message`, `retryable`, `effects`, optional `source` span, `operation_id`, `request_id`, `trace_id`, `node_id`, `hint`, `details`, `cause`, `suppressed`. Helpers: `exit_code()`, `http_status()`, `to_value()` (the envelope `error` object), `render(Some(source_text))` (terminal rendering with a caret). Codes and mappings: [API-2026-0005](api-2026-0005-error-registry.md).

## Rate Limits

The policy's `limits` apply to library calls as to any surface: `max_concurrent_requests` (top-level; `limit.concurrency`), `max_call_depth` (`limit.call_depth`), and the session limits.

## Versioning and Deprecation

The crate version is `0.1.0`; semantic versioning applies from the first release (pre-1.0: a minor bump may break). Nothing is deprecated.

## Examples

The program below was compiled and run against the crate at commit `829ca43` (a scratch binary crate `libcheck` with `rivet = { path = … }`, `tokio` (rt-multi-thread, macros), `async-trait` and `serde_json`; run from a folder containing `data/a.txt` = `hello` and an empty `audit/`). It exercises `Policy::from_json`, the host ceiling, `scope.stream`, `scope.duplex`, the typed sink stop, `request_restricted` and `export_trace`:

```rust
use async_trait::async_trait;
use rivet::Runtime;
use rivet::domain::contracts::DataEvent;
use rivet::domain::policy::Policy;
use rivet::domain::ports::DataSink;
use rivet::domain::{RivetError, RivetResult, Value};
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
    async fn send(&self, event: DataEvent) -> RivetResult<()> {
        println!("sink got seq={} {}", event.seq, event.data.to_json());
        Err(RivetError::consumer_stop())
    }
}

#[tokio::main]
async fn main() -> Result<(), RivetError> {
    let policy = Policy::from_json(
        br#"{"version":1,"grants":[{"capability":"allow_read","targets":["./data/**"],"access":["read"]},
                                   {"capability":"allow_write","targets":["./audit/**"],"access":["create"]}]}"#,
    )?;
    // The host ceiling can only narrow: it does not grant ./audit/**, so trace export is denied.
    let ceiling = Policy::from_json(
        br#"{"version":1,"grants":[{"capability":"allow_read","targets":["./data/**"],"access":["read"]}],
             "limits":{"max_concurrent_requests":8}}"#,
    )?;
    let rt = Runtime::builder().source("app.rivet", APP, ".").policy(policy).ceiling(ceiling).build()?;

    // Scope-owned stream and duplex.
    rt.scope(|scope| async move {
        let mut s = scope.stream("demo.countdown", Value::Object(vec![])).await?;
        while let Some(env) = s.next().await? {
            println!("stream {}", env.to_json());
        }
        let mut d = scope.duplex("chat.echo", Value::Object(vec![])).await?;
        d.send(Value::text("hi")).await?;
        let bad = d.send(Value::Int(5)).await.unwrap_err();
        println!("duplex bad item -> {} {}", bad.code, bad.message);
        d.finish_send();
        while let Some(env) = d.next().await? {
            println!("duplex {}", env.to_json());
        }
        Ok(())
    })
    .await?;

    // Typed DataSink stop.
    let e = rt.request("demo.countdown", Value::Object(vec![]), Some(Arc::new(FirstOnly))).await.unwrap_err();
    println!("sink stop -> kind={} code={}", e.kind.as_str(), e.code);

    // Per-request restriction (narrows only).
    let p = Value::from_json(&serde_json::json!({"path": "data/a.txt"}));
    let ok = rt.request("files.read", p.clone(), None).await?;
    println!("read -> {}", ok.result.to_json());
    let narrow = Value::from_json(&serde_json::json!({"grants": [{"capability": "allow_read", "targets": ["./data/other/**"]}]}));
    let e = rt.request_restricted("files.read", p, narrow, None).await.unwrap_err();
    println!("restricted -> {} {}", e.code, e.message);

    // Trace export goes through the broker as allow_write create; the ceiling denies ./audit/**.
    let e = rt.export_trace(&ok.request_id, "./audit/trace.json").await.unwrap_err();
    println!("export -> {} {}", e.code, e.message);

    // Without the ceiling the same export writes a NEW file (never overwrites).
    let rt2 = Runtime::builder().source("app.rivet", APP, ".").policy(Policy::from_json(
        br#"{"version":1,"grants":[{"capability":"allow_read","targets":["./data/**"],"access":["read"]},
                                   {"capability":"allow_write","targets":["./audit/**"],"access":["create"]}]}"#)?).build()?;
    let c = rt2.request("files.read", Value::from_json(&serde_json::json!({"path": "data/a.txt"})), None).await?;
    let receipt = rt2.export_trace(&c.request_id, "./audit/trace.json").await?;
    println!("export -> {}", receipt.to_json());
    let again = rt2.export_trace(&c.request_id, "./audit/trace.json").await.unwrap_err();
    println!("export again -> {}", again.code);
    Ok(())
}
```

Output (IDs differ per run):

```text
stream {"request_id":"req_01956a2955","trace_id":"tr_01956a2955","seq":1,"type":"data","data":3}
stream {"request_id":"req_01956a2955","trace_id":"tr_01956a2955","seq":2,"type":"data","data":2}
stream {"request_id":"req_01956a2955","trace_id":"tr_01956a2955","seq":3,"type":"data","data":1}
stream {"request_id":"req_01956a2955","trace_id":"tr_01956a2955","result":{"count":3},"data_count":3,"effects":"none","type":"result"}
duplex bad item -> validation.input input item at $ must be text, got integer
duplex {"request_id":"req_0214b2b09a","trace_id":"tr_0214b2b09a","seq":1,"type":"data","data":"hi"}
duplex {"request_id":"req_0214b2b09a","trace_id":"tr_0214b2b09a","result":{"echoed":1},"data_count":1,"effects":"none","type":"result"}
sink got seq=1 3
sink stop -> kind=cancelled code=consumer.stop
read -> "hello"
restricted -> permission.denied allow_read read on data/a.txt denied: request restriction: no grant for allow_read data/a.txt
export -> permission.denied allow_write create on ./audit/trace.json denied: host ceiling: no grant for allow_write ./audit/trace.json
export -> {"request_id":"req_0195ed6d65","path":"./audit/trace.json","events":1,"bytes":649}
export again -> conflict.already_exists
```

```text
  what happened, step by step
  build(policy ∩ ceiling) ─▶ scope: stream demo.countdown ─▶ 3 × Data, Result {count:3}
                           ─▶ scope: duplex chat.echo ─▶ send "hi" ✓, send 5 ✗ validation.input,
                                                         finish_send ─▶ Data "hi", Result {echoed:1}
                           ─▶ request + FirstOnly sink ─▶ stop after seq 1 ─▶ cancelled / consumer.stop
                           ─▶ files.read ✓ ─▶ request_restricted(./data/other/**) ✗ request restriction
                           ─▶ export_trace ✗ host ceiling (no allow_write ./audit/**)
  rt2 (no ceiling)         ─▶ export_trace ✓ new file ─▶ again ✗ conflict.already_exists
```

The demo [12-library/embedding.rs.txt](../demos/12-library/embedding.rs.txt) shows the same API with `Policy::from_file("policy.json")`, `rt.outputs`, `rt.io` and `rt.generate_policy`.

## Compatibility Notes

The 0.1.0 library differs from the approved design sketch in these details; the code is authoritative:

| Design sketch | 0.1.0 |
|---|---|
| `.source(src)` | `.source(path, text, root)`. |
| `rt.request("id", json!({…}), None)` | params are `rivet::domain::Value` (`Value::from_json(&json!({…}))`). |
| Sink returning `Continue` / `Stop` | `DataSink::send` returns `Ok(())` to continue or `Err(RivetError::consumer_stop())` to stop (the request ends `cancelled` / `consumer.stop`); any other `Err` is `consumer_failed`. |
| `rt.outputs("id") -> OutputSpec` | `rt.outputs(Some("id"), false) -> Vec<OutputReport>`. |
| per-request restriction as a request option | `rt.request_restricted(id, params, restrict, sink)` or `Request.restrict` with `dispatch_request`. |

`Runtime::scope` (`stream`/`duplex`), `Policy::from_file`/`Policy::from_json`, `.ceiling(Policy)` and W3C trace context (`SessionOpenInput.trace`, `Runtime::new_request_traced`) are provided as designed. The trace store is in-memory per `Runtime` (no persistent trace store).

## Related Documents

- [API index](index.md) · [HTTP API](api-2026-0001-http-rest-sse-polling.md) · [Error registry](api-2026-0005-error-registry.md)
- [Runtime architecture](../architecture/arch-2026-0001-rivet-runtime-architecture.md) · [Demo 12-library](../demos/12-library/README.md)
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) · [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial library contract; example compiled and run against the crate at commit f40d4aa. |
| 2 | 2026-09-28 | Claude | Fix batch through 829ca43: `Runtime::scope` with `stream`/`duplex` handles, `Policy::from_file`/`from_json`, `.ceiling(Policy)`, `session_limits`, typed `DataSink` stop (`consumer.stop`), structured cancellation, `request_restricted`, `export_trace`, `graph`, `shutdown`, `SessionOpenInput.trace/restrict`, `ServeOptions.access_log`; new example compiled and run at 829ca43; compatibility table reduced to the remaining differences. |
