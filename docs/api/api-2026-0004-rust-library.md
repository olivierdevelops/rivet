---
document_id: API-2026-0004
title: "Rivet Rust library API"
document_type: api
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
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
last_verified_version: "0.1.0-dev (commit f40d4aa)"
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
   ┌────────────────────────────────────────────────────────────────────────┐
   │  Runtime::builder()                                                    │
   │     .file("app.rivet")  | .source(path, text, root)                    │
   │     .policy_file(p)     | .policy(Policy)      (else: discover / deny) │
   │     .build()?  ─────────────▶ compile → load policy → wire adapters    │
   │                                                                        │
   │  rt.request(id, params, sink)      ─▶ Completion         unary/stream  │
   │  rt.open_session / send_input / finish_input / read_events / cancel_*  │
   │                                    ─▶ duplex + resumable streams       │
   │  rt.list / describe / outputs      ─▶ catalog                          │
   │  rt.io / generate_policy / trace   ─▶ audit                            │
   │  setup_serve::start(rt, opts)      ─▶ embed every network surface      │
   └────────────────────────────────────────────────────────────────────────┘
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
| `RuntimeBuilder::connector_discovery` | `(self) -> Self` | Loads MCP connectors whose snapshot is missing/unapproved without their imports (for `sync_connector`). |
| `RuntimeBuilder::build` | `(self) -> RivetResult<Runtime>` | Compiles, loads policy, loads connector snapshots and gRPC descriptors (bootstrap reads), wires adapters. |
| `rivet::orchestrator::runtime::policy_from_json` | `(bytes: &[u8], base_dir: &str) -> RivetResult<Policy>` | Same strict schema v1 as policy.json; relative targets resolve against `base_dir`. |

```text
  policy source decision in build()
  ┌───────────────────────┬───────────────────────────────────────────────┐
  │ .policy(Policy)       │ used as given                                 │
  │ .policy_file(p)       │ load p (strict v1)                            │
  │ .file(entry), neither │ discover policy.json beside entry, else deny  │
  │ .source(…),  neither  │ deny-by-default (pure operations still run)   │
  └───────────────────────┴───────────────────────────────────────────────┘
```

`Runtime` is `Clone` (an `Arc` inside); clones share catalog, broker, sessions and trace store.

### Requests

| Method | Signature |
|---|---|
| `request` | `async (&self, operation_id: &str, params: Value, sink: Option<Arc<dyn DataSink>>) -> RivetResult<Completion>` — as `local` |
| `request_as` | `async (&self, principal: Principal, operation_id: &str, params: Value, sink: Option<Arc<dyn DataSink>>) -> RivetResult<Completion>` |
| `new_request` | `(&self, operation_id: &str, params: Value, principal: Principal) -> Request` — fresh `request_id`/`trace_id`, default deadline 30000 ms |
| `dispatch_request` | `async (&self, req: Request, sink: Option<Arc<dyn DataSink>>) -> RivetResult<Completion>` — set `req.deadline_ms` etc. first |
| `cancel` | `(&self, request_id: &str, principal: Principal) -> RivetResult<CancelReceipt>` — cancel one of that principal's running top-level requests (idempotent) |

`params` is `rivet::domain::Value` (`Value::from_json(&serde_json::Value)`, `Value::object([...])`, `Value::text(..)`); results convert back with `.to_json()`. **Streaming** uses a `DataSink`:

```rust
#[async_trait::async_trait]
pub trait DataSink: Send + Sync {
    async fn send(&self, event: DataEvent) -> RivetResult<()>;   // Err stops the producer → consumer_failed
}
```

Each emitted item arrives in order as `DataEvent {request_id, trace_id, seq, data}`; the `Completion` is returned after the last item. Dropping the request future drops its whole scope (tasks, handles and child processes are released).

### Sessions (duplex and resumable streams)

| Method | Input → Output |
|---|---|
| `open_session` | `SessionOpenInput {id, params, principal, connection_owned, deadline_ms}` → `SessionReceipt` |
| `send_input` | `SessionSendInput {session_id, send_seq, data, principal}` → `SessionAck` |
| `finish_input` | `SessionRef {session_id, principal}` → `SessionAck` |
| `read_events` | `SessionReadInput {session_id, after_seq, max_events, wait_ms, principal}` → `SessionBatch` |
| `cancel_session` | `SessionRef` → `CancelReceipt` |
| `sessions` | `-> Arc<dyn SessionDriver>` (the driver shared with polling, WebSocket and MCP) |

A `receives` operation iterates its input as `incoming`; items sent with `send_input` feed it in `send_seq` order. Limits are those of [polling sessions](api-2026-0001-http-rest-sse-polling.md#polling-sessions).

### Catalog, audit and connectors

| Method | Signature |
|---|---|
| `list` | `(&self) -> RivetResult<Catalog>` — public entries |
| `describe` | `(&self, ids: &[String]) -> RivetResult<Catalog>` |
| `outputs` | `(&self, id: Option<&str>, all: bool) -> RivetResult<Vec<OutputReport>>` |
| `catalog_version` | `(&self) -> String` — `sha256:` of the bundle sources |
| `io` | `(&self, query: &IoQuery) -> RivetResult<IoReport>` — the I/O manifest (sync; reads only catalog and policy) |
| `generate_policy` | `(&self, ids: &[&str]) -> RivetResult<PolicyDraft>` — empty = every public op; never writes |
| `generate_policy_draft` | `(&self, ids: &[String], all: bool, output: Option<&str>) -> RivetResult<PolicyDraft>` — `output` creates a new file, refusing to overwrite |
| `trace` | `(&self, request_id: &str) -> RivetResult<TraceResult>` — this process's recorded broker decisions |
| `decisions` | `(&self) -> Vec<Permit>` — every broker decision so far |
| `policy` | `(&self) -> &Policy` |
| `sync_connector` | `async (&self, name: &str, output: &str) -> RivetResult<ConnectorSync>` — discover an MCP connector and exclusively create a candidate snapshot |
| `connector_imports` | `(&self) -> Vec<String>` |

### Embedding `serve`

`rivet::orchestrator::setup_serve::start(rt, ServeOptions {listen: Some("127.0.0.1:0".into()), stdio: false, authenticator: None}).await? -> ServeHandle` binds one listener with every enabled surface; `handle.addr` is the bound address and `handle.shutdown().await` stops it. The same start-up refusals apply (`serve.auth_required`, `unsupported.serve_mtls`).

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

The program below was compiled and run against the crate at commit `f40d4aa` (a scratch binary crate with `rivet = { path = … }`, `tokio`, `async-trait`, `serde_json`):

```rust
use async_trait::async_trait;
use rivet::Runtime;
use rivet::domain::contracts::{DataEvent, Principal};
use rivet::domain::io_manifest::IoQuery;
use rivet::domain::ports::DataSink;
use rivet::domain::sessions::{SessionOpenInput, SessionReadInput, SessionRef, SessionSendInput};
use rivet::domain::{RivetResult, Value};
use rivet::orchestrator::runtime::policy_from_json;
use std::sync::Arc;

const APP: &str = r#"
operation demo.add
    name "Add two integers"
    description "Add two signed integers and return their sum."
    param a integer required description "First operand."
    param b integer default 0 description "Second operand; defaults to zero."
    output integer description "Sum of a and b."
    return a + b
end

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
"#;

struct Print;

#[async_trait]
impl DataSink for Print {
    async fn send(&self, event: DataEvent) -> RivetResult<()> {
        println!("data seq={} {}", event.seq, event.data.to_json());
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), rivet::domain::RivetError> {
    // In-memory source + explicit in-memory policy (no grants: deny-by-default for effects).
    let policy = policy_from_json(br#"{"version": 1}"#, ".")?;
    let rt = Runtime::builder().source("app.rivet", APP, ".").policy(policy).build()?;

    let add = Value::from_json(&serde_json::json!({"a": 2, "b": 3}));
    println!("add -> {}", rt.request("demo.add", add, None).await?.to_json());

    let c = rt.request("demo.countdown", Value::Object(vec![]), Some(Arc::new(Print))).await?;
    println!("countdown -> {}", c.to_json());

    let ids: Vec<String> = rt.list()?.entries.iter().map(|e| e.id.clone()).collect();
    println!("list -> {ids:?}");
    println!("outputs -> {}", rt.outputs(Some("demo.add"), false)?[0].to_json());

    // Duplex through a session.
    let me = Principal::local();
    let r = rt.open_session(SessionOpenInput {
        id: "chat.echo".into(), params: Value::Object(vec![]),
        principal: me.clone(), connection_owned: false, deadline_ms: None,
    }).await?;
    for (i, word) in ["hi", "there"].iter().enumerate() {
        rt.send_input(SessionSendInput {
            session_id: r.session_id.clone(), send_seq: i as u64 + 1,
            data: Value::text(*word), principal: me.clone(),
        }).await?;
    }
    rt.finish_input(SessionRef { session_id: r.session_id.clone(), principal: me.clone() }).await?;
    let mut after = 0;
    loop {
        let b = rt.read_events(SessionReadInput {
            session_id: r.session_id.clone(), after_seq: after,
            max_events: None, wait_ms: Some(1000), principal: me.clone(),
        }).await?;
        for ev in &b.events { println!("session event {}", ev.to_json()); }
        after = b.last_seq;
        if b.terminal { break; }
    }

    let io = rt.io(&IoQuery { format: "json".into(), ..IoQuery::default() })?;
    println!("io complete={} exit={}", io.manifest.complete, io.exit_code);
    let draft = rt.generate_policy(&[])?;
    println!("draft complete={} grants={}", draft.complete, draft.grants.len());

    let e = rt.request("demo.add", Value::Object(vec![]), None).await.unwrap_err();
    println!("error {} kind={} exit={} http={}", e.code, e.kind.as_str(), e.exit_code(), e.http_status());
    Ok(())
}
```

Output (IDs differ per run):

```text
add -> {"request_id":"req_01cbd43edd","trace_id":"tr_01cbd43edd","result":5,"data_count":0,"effects":"none"}
data seq=1 3
data seq=2 2
data seq=3 1
countdown -> {"request_id":"req_024a3c0902","trace_id":"tr_024a3c0902","result":{"count":3},"data_count":3,"effects":"none"}
list -> ["demo.add", "demo.countdown", "chat.echo"]
outputs -> {"id":"demo.add","output":{"type":"integer","description":"Sum of a and b."},"emits":null,"receives":null,"errors":[]}
session event {"request_id":"req_03c970f8af","trace_id":"tr_03c970f8af","seq":1,"type":"data","data":"hi"}
session event {"request_id":"req_03c970f8af","trace_id":"tr_03c970f8af","seq":2,"type":"data","data":"there"}
session event {"request_id":"req_03c970f8af","trace_id":"tr_03c970f8af","result":{"echoed":2},"data_count":2,"effects":"none","type":"result","seq":3}
io complete=true exit=0
draft complete=true grants=0
error validation.required kind=validation exit=2 http=422
```

```text
  what happened, step by step
  build ─▶ request demo.add ─▶ Completion 5
        ─▶ request demo.countdown + DataSink ─▶ 3 × send(DataEvent) ─▶ Completion {count:3}
        ─▶ open_session chat.echo ─▶ send 1 "hi", send 2 "there" ─▶ finish_input
             └▶ read_events … ─▶ data "hi", data "there", result {echoed:2}, terminal
        ─▶ io / generate_policy (pure bundle: no sites, no grants)
        ─▶ request demo.add {} ─▶ Err(validation.required) → exit 2, HTTP 422
```

## Compatibility Notes

The 0.1.0 library differs from the approved design sketch; the code is authoritative:

| Design sketch | 0.1.0 |
|---|---|
| `rt.scope(|scope| …)`, `scope.stream(…)`, `scope.duplex(…)` handles | Not provided. Use `request` + `DataSink` for streams and the session methods for duplex. |
| `Policy::from_file` / `Policy::from_json` | `RuntimeBuilder::policy_file(path)` and `rivet::orchestrator::runtime::policy_from_json(bytes, base_dir)`. |
| Host policy ceiling on the builder | Not provided; pass the narrowed `Policy` itself. |
| Sink returning `Continue` / `Stop` | `DataSink::send` returns `Ok(())` or `Err(_)`; an error stops the request (`consumer_failed`). |
| `rt.outputs("id") -> OutputSpec` | `rt.outputs(Some("id"), false) -> Vec<OutputReport>`. |
| `.source(src)` | `.source(path, text, root)`. |

Also: W3C `traceparent` is not propagated; the trace store is in-memory per `Runtime`.

## Related Documents

- [API index](index.md) · [HTTP API](api-2026-0001-http-rest-sse-polling.md) · [Error registry](api-2026-0005-error-registry.md)
- [Runtime architecture](../architecture/arch-2026-0001-rivet-runtime-architecture.md) · [Demo 12-library](../demos/12-library/README.md)
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) · [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial library contract; example compiled and run against the crate at commit f40d4aa. |
