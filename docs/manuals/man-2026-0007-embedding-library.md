---
document_id: MAN-2026-0007
title: "Embedding Rivet as a Rust library"
document_type: manual
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 2
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [library, registry, execution, policy, audit, sessions, serve]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [embedded, development]
audience: [rust-developers, integrators]
scope: Using rivet::Runtime from a Rust host — building from a file or in-memory source, supplying policy (Policy::from_file/from_json) and a host ceiling, unary and streaming requests with typed sink stop, scope-owned streams and duplex handles, per-request restrictions, errors, declared outputs, the I/O manifest, policy drafts, traces and trace export, cancellation, sessions and serving the same runtime — with programs compiled and run against the 0.1.0-dev crate.
reason: PLAN-2026-0001 row D-40 — developer guide for the implemented library API; the example is a real Cargo project compiled against the repository, replacing the proposal-era sketch in demos/12-library.
related_documents: [MAN-2026-0001, MAN-2026-0003, MAN-2026-0005, MAN-2026-0006, SYS-2026-0001, SYS-2026-0002, DEMO-2026-0012, PLAN-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, manual, library, rust, embedding]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.1.0-dev (commit 829ca43)"
next_review_date: 2026-10-28
---

# Embedding Rivet as a Rust library

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** library, registry, execution, policy, audit, sessions, serve

## Purpose

Call the same `.rivet` catalog from Rust with the same validation, policy broker, errors and results as the CLI
and every serve surface. Part of the [Rivet manual](man-2026-0001-rivet-manual.md). The
[12-library demo](../demos/12-library/README.md) holds the catalog; this volume holds a compiled host program.

## Reading Order

```text
 add the dependency ─► build a Runtime (policy + ceiling) ─► request (unary) ─► request with a sink (stream, stop)
   ─► scope.stream / scope.duplex ─► restrict one request ─► handle errors ─► outputs / list / describe / graph
   ─► io / generate_policy / trace / export_trace ─► sessions + cancel ─► serve
```

## Concepts

```text
   Runtime::builder()
      .file("app.rivet")            ── or ──  .source("mem.rivet", text, root)
      .policy_file("ci.json")       ── or ──  .policy(Policy::from_file(p)? | Policy::from_json(bytes)?)  (default: discover)
      .ceiling(Policy)              ── optional host ceiling: every attempt needs policy ∩ ceiling
      .build()?                     ── compile + load policy; errors are RivetError (syntax.*, policy.invalid, …)
         │
         ▼
   Runtime (cheap to clone; share across tasks)
      .request(id, params, sink)          ─► Completion        principal "local"
      .request_restricted(id, p, restrict, sink) ─► Completion  narrowed to policy ∩ restrict
      .scope(|scope| … scope.stream(id, p) / scope.duplex(id, p) …)  owned handles, joined at the end
      .request_as(principal, id, …)       ─► Completion        a named principal (serve.principals applies)
      .dispatch_request(Request, sink)    ─► Completion        fully formed request (IDs, deadline, principal)
      .cancel(request_id, principal)      ─► CancelReceipt
      .list() / .describe(&ids) / .outputs(id, all)
      .io(&IoQuery) / .generate_policy(&ids) / .trace(request_id) / .export_trace(request_id, path) / .graph(&q)
      .sessions()  ─► SessionDriver: open · send · finish_input · read · cancel
```

| Item | Path |
|---|---|
| `Runtime`, `RuntimeBuilder` | `rivet::Runtime` (re-exported from `rivet::orchestrator::runtime`) |
| `Policy` (`from_file`, `from_json`) | `rivet::domain::policy::Policy` |
| `policy_from_json(bytes, base_dir)` | `rivet::orchestrator::runtime::policy_from_json` |
| `Scope`, `StreamHandle`, `DuplexHandle`, `DuplexSender` | `rivet::orchestrator::setup_library` |
| `Envelope` (`Data`, `Result`, `Error`) | `rivet::domain::contracts::Envelope` |
| `Value` (`from_json`, `to_json`), `RivetError`, `RivetResult` | `rivet::domain` |
| `Completion`, `DataEvent`, `Principal`, `Request` | `rivet::domain::contracts` |
| `DataSink` (async trait) | `rivet::domain::ports::DataSink` |
| `IoQuery`, `IoReport`, `PolicyDraft`, `TraceResult` | `rivet::domain::io_manifest` |
| `start`, `ServeOptions`, `ServeHandle` | `rivet::orchestrator::setup_serve` |

## Installation and Setup

0.1.0 is not published to crates.io. Depend on the repository by path (or by git):

```toml
[dependencies]
rivet = { path = "/path/to/rivet" }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
serde_json = "1"
async-trait = "0.1"
```

The crate uses edition 2024 and Rust 1.90.0. Copy the repository's `Cargo.lock` into your host project to get
the same dependency versions that were tested.

## Task-Oriented Workflows

### Journey Overview

```text
 host main ─► builder().file(p).build() ── Err(RivetError) ─► print e.code, exit e.exit_code()
                      │ Ok(rt)
                      ▼
            rt.request("demo.add", params, None).await ── Err(e) ─► e.kind / e.code / e.details
                      │ Ok(Completion{result, effects, …})
                      ▼
            use c.result (Value) ─► c.to_json() for the wire form
```

### Complete host program

This program was compiled with `cargo build` in a scratch Cargo project depending on the repository by path,
and run with the two demo bundles as arguments.

```rust
use rivet::Runtime;
use rivet::domain::contracts::DataEvent;
use rivet::domain::io_manifest::IoQuery;
use rivet::domain::ports::DataSink;
use rivet::domain::{RivetResult, Value};
use rivet::orchestrator::runtime::policy_from_json;
use rivet::orchestrator::setup_serve::{ServeOptions, start};
use serde_json::json;
use std::sync::{Arc, Mutex};

/// Collects emitted data items (the `emits` stream) of one request.
struct Collect(Mutex<Vec<serde_json::Value>>);

#[async_trait::async_trait]
impl DataSink for Collect {
    async fn send(&self, event: DataEvent) -> RivetResult<()> {
        self.0.lock().unwrap().push(event.data.to_json());
        Ok(())
    }
}

#[tokio::main]
async fn main() -> RivetResult<()> {
    let catalog = std::env::args().nth(1).expect("path to 01-catalog/app.rivet");
    let sandbox = std::env::args().nth(2).expect("path to 11-sandbox/app.rivet");

    // 1. Build from a file; policy.json beside it is discovered (absent = deny-by-default).
    let rt = Runtime::builder().file(&catalog).build()?;
    for e in rt.list()?.entries {
        println!("op {} — {}", e.id, e.name);
    }

    // 2. Unary request.
    let c = rt.request("demo.add", Value::from_json(&json!({"a": 2, "b": 3})), None).await?;
    println!("add => {}", c.to_json());

    // 3. Streaming request: data items go to the sink, the Completion is returned.
    let sink = Arc::new(Collect(Mutex::new(Vec::new())));
    let c = rt.request("demo.countdown", Value::from_json(&json!({})), Some(sink.clone())).await?;
    println!("countdown data {:?} result {}", sink.0.lock().unwrap(), c.result.to_json());

    // 4. Failures are RivetError values with kind, code and the CLI exit code.
    match rt.request("demo.add", Value::from_json(&json!({"a": "x"})), None).await {
        Ok(_) => unreachable!(),
        Err(e) => println!("error {} ({:?}) exit {}", e.code, e.kind, e.exit_code()),
    }

    // 5. Declared outputs.
    let outs = rt.outputs(Some("demo.add"), false)?;
    println!("outputs => {}", outs[0].to_json());

    // 6. Effects: I/O manifest, policy draft, request and its trace (11-sandbox has a policy.json).
    let sb = Runtime::builder().file(&sandbox).build()?;
    let io = sb.io(&IoQuery { all: true, check_policy: true, ..IoQuery::default() })?;
    print!("{}", io.rendered);
    println!("io exit code {} complete={}", io.exit_code, io.manifest.complete);
    let draft = sb.generate_policy(&["data.snapshot"])?;
    println!("draft => {}", draft.policy_json());
    let c = sb.request("data.read", Value::from_json(&json!({})), None).await?;
    let tr = sb.trace(&c.request_id)?;
    println!("trace {} events={} complete={}", tr.request_id, tr.events.len(), tr.complete);
    match sb.request("data.private", Value::from_json(&json!({})), None).await {
        Ok(_) => unreachable!(),
        Err(e) => println!("denied: {} — {}", e.code, e.message),
    }

    // 7. In-memory source + in-memory policy (no file reads).
    let src = "operation hi.say\n    output text\n    return \"hi\"\nend\n";
    let policy = policy_from_json(br#"{"version":1}"#, ".")?;
    let mem = Runtime::builder().source("mem.rivet", src, ".").policy(policy).build()?;
    println!("mem => {}", mem.request("hi.say", Value::from_json(&json!({})), None).await?.to_json());

    // 8. Serve the same Runtime on an ephemeral loopback port, then stop it.
    let handle = start(rt.clone(), ServeOptions { listen: Some("127.0.0.1:0".into()), ..ServeOptions::default() }).await?;
    println!("serving on {}", handle.addr.unwrap());
    handle.shutdown().await;
    Ok(())
}
```

Run:

```bash
cargo run -- /path/to/rivet/docs/demos/01-catalog/app.rivet /path/to/rivet/docs/demos/11-sandbox/app.rivet
```

Output (IDs and the ephemeral port vary):

```text
op demo.greet — Greet a person
op demo.add — Add two integers
op demo.health — Check availability
op demo.countdown — Count down
add => {"request_id":"req_018bb9170d","trace_id":"tr_018bb9170d","result":5,"data_count":0,"effects":"none"}
countdown data [Number(3), Number(2), Number(1)] result {"count":3}
error validation.type (Validation) exit 2
outputs => {"id":"demo.add","output":{"type":"integer","description":"Sum of a and b."},"emits":null,"receives":null,"errors":[]}
OPERATION      KIND  ACCESS  TARGET                      KNOWLEDGE  SOURCE        DECISION
data.private   file  read    ./data/private/secret.json  exact      app.rivet:26  denied
data.read      file  read    ./data/public.json          exact      app.rivet:15  allowed
data.snapshot  (calls data.read — see above)                        app.rivet:37
data.snapshot  file  create  ./out/snapshot.json         exact      app.rivet:38  allowed
io exit code 3 complete=true
draft => {"version":1,"grants":[{"capability":"allow_read","targets":["./data/public.json"],"access":["read"]},{"capability":"allow_write","targets":["./out/snapshot.json"],"access":["create"]}],"network":{"deny_private_ranges":true}}
trace req_018a81a155 events=1 complete=true
denied: permission.denied — allow_read read on ./data/private/secret.json denied: deny allow_read ./data/private/**
mem => {"request_id":"req_018952275d","trace_id":"tr_018952275d","result":"hi","data_count":0,"effects":"none"}
serving on 127.0.0.1:55823
```

### Scopes, ceilings, restrictions and trace export

A second program, compiled and run at commit `829ca43` (scratch crate `libcheck`, run from a folder with
`data/a.txt` = `hello` and an empty `audit/`), exercises the rest of the API:

```text
  Policy::from_json(policy) ──┐
  Policy::from_json(ceiling) ─┴─ .ceiling ─► rt ─► scope ─► stream demo.countdown ─► Data×3, Result
                                                       └──► duplex chat.echo ─► send ✓ / send ✗ / finish / next
                                             rt ─► request + FirstOnly sink ─► consumer.stop (cancelled)
                                             rt ─► request_restricted ─► request restriction denial
                                             rt ─► export_trace ─► host ceiling denial
                         (no ceiling) rt2 ─► export_trace ✓ ─► again ✗ conflict.already_exists
```

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

Output (IDs vary):

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

### Build a runtime

| Builder call | Effect | Failure |
|---|---|---|
| `.file(path)` | read and compile the entry file; discover `policy.json` beside it | `not_found.source`, `syntax.*`, `check.*` |
| `.source(path, text, root)` | compile in-memory text; `root` anchors relative paths; policy defaults to deny-all | `syntax.*` |
| `.policy_file(path)` | like `--policy PATH` | `policy.invalid` |
| `.policy(Policy)` | an already-parsed policy: `Policy::from_file(path)?` (targets relative to that file) or `Policy::from_json(bytes)?` (targets relative to the bundle root) | `policy.invalid` from the constructor |
| `.ceiling(Policy)` | **host ceiling**: every attempt must be allowed by the loaded policy **and** the ceiling; deny in either wins; `limits` narrow to the smaller value; a second call intersects | denials read `… denied: host ceiling: no grant for …` |
| `.session_limits(SessionLimits)` | host caps for sessions (count, queues, idle lease, retention) | — |
| `.build()` | compile + load; neither `.file` nor `.source` → `validation.usage` "no source: use .file(PATH) or .source(…)" | any of the above |

`Policy::from_json`, `Policy::from_file` and `policy_from_json(bytes, base_dir)` use the same strict schema v1 as
`policy.json`; `base_dir` anchors relative file targets.

### Unary and streaming requests

- `request(id, params, None)` returns `Completion { request_id, trace_id, result, data_count, effects }`.
- Pass `Some(Arc<dyn DataSink>)` to receive each emitted item as a `DataEvent { request_id, trace_id, seq, data }`;
  the sink must be `Send + Sync + 'static`. Return `Ok(())` to continue, `Err(RivetError::consumer_stop())` to
  **stop** (the producer stops, cleanup runs, the request ends `cancelled` / `consumer.stop` — not a failure), or
  any other error to fail the request (`consumer_failed`).
- `rt.scope(|scope| async move { … })` owns streams and duplexes: `scope.stream(id, params)` returns a
  `StreamHandle` whose `next().await?` yields `Envelope::Data` items, then one `Envelope::Result`, then `None`
  (a terminal error is returned as `Err`); `scope.duplex(id, params)` adds `send(item)` (validated against
  `receives`: `validation.input`; after `finish_send` → `conflict.input_finished`), `finish_send()` and
  `into_split()`. When the body returns, unfinished requests are cancelled and joined (5 s each, then aborted).
- `request_restricted(id, params, restrict, sink)` narrows one request (and its nested calls) to
  `policy ∩ restrict`, where `restrict` is `{"grants": [...]}`; it never widens.
- Params and results are `rivet::domain::Value`; convert with `Value::from_json(&serde_json::Value)` and
  `value.to_json()`.
- The default deadline is 30 s; use `dispatch_request` with `Request { deadline_ms, … }` (built by
  `rt.new_request(id, params, principal)`) to change it.

### Handle errors

Every failure is a `RivetError` with `kind` (`ErrorKind`), `code`, `message`, `details`, `effects`,
`operation_id`, `request_id` and an `exit_code()` identical to the CLI's. Typical matches:

| `e.code` | Meaning | Host action |
|---|---|---|
| `validation.*` | bad params | report to the caller; do not retry |
| `permission.denied` | policy or principal denies | fix policy; `e.details` names capability, access, target |
| `not_found.operation` | unknown or private ID | check the catalog |
| `timeout.*` | deadline | retry only if the operation is replay-safe |
| `cancelled.request` | cancelled via `rt.cancel` | — |
| `consumer.stop` | your sink returned `RivetError::consumer_stop()` | expected; not a failure |
| `limit.buffered_bytes` | the host byte budget (`limits.max_buffered_bytes`) is full | drain sessions, raise the limit |

### Inspect and govern

| Method | Returns | Notes |
|---|---|---|
| `list()` | `Catalog { entries }` | public operations and imported MCP tools |
| `describe(&[String])` | `Catalog` | empty slice = all |
| `outputs(Some(id), false)` / `outputs(None, true)` | `Vec<OutputReport>` (`to_json()`) | declared output/emits/receives/errors (emits/receives schemas carry their `description` since `2a751ab`) |
| `io(&IoQuery)` | `IoReport { manifest, rendered, diagnostics, exit_code }` | same options as `rivet io` (`by`, `kind`, `access`, `format`, `check_policy`, `strict`, `needs`, `check_files`, `trace_request_id`) |
| `generate_policy(&[&str])` | `PolicyDraft { grants, review, complete }`; `policy_json()` | never writes a file |
| `trace(request_id)` | `TraceResult { events, complete, … }` | only requests run by **this** runtime; pure requests record nothing (`not_found.trace`) |
| `export_trace(request_id, path).await` | `TraceExport { request_id, path, events, bytes }` | writes a **new** bundle-relative file through the broker (`allow_write` create); existing → `conflict.already_exists`; the same use as `rivet.trace.export` / `rivet --endpoint … trace export` |
| `graph(&GraphQuery { id, all })` | `CallGraph { operation_id, root, nodes, edges }` | the `rivet graph` model |

### Sessions and cancellation

- `rt.cancel(request_id, principal)` cancels one of that principal's running top-level requests and returns a
  `CancelReceipt { request_id, session_id, state }` (`cancelled`, or the terminal state it already had).
  Cancellation is structured: the request's token fires, the run unwinds at its next await point and closes every
  `with` handle in reverse order within 5 s; child processes are reaped.
- `rt.shutdown(drain)` cancels every running request and session (what `serve` does on SIGTERM).
- `rt.sessions()` returns the `SessionDriver` used by polling, WebSocket and `rivet.sessions.*`:
  `open`, `send`, `finish_input`, `read`, `cancel`. Use it to drive `receives` operations from a host.
  `SessionOpenInput` carries `deadline_ms` (default 30000, cap 600000), an optional W3C `trace` context and an
  optional `restrict`.

### Serve the same runtime

`start(runtime, ServeOptions { listen, stdio, authenticator, access_log })` mounts every surface allowed by the
runtime's `serve` policy plus `GET /v1/health` and returns a `ServeHandle` (`addr`, `receipt`,
`shutdown().await` — the SIGTERM drain). An `authenticator` (`Arc<dyn Authenticator>`) replaces the policy's
`serve.auth` for library hosts; `access_log` (`Arc<dyn Fn(&str) + Send + Sync>`) receives one JSON access-log line
per request instead of stderr.

### Expected Result and Side Effects

A runtime performs only the effects its policy grants. Dropping a request future cancels it and cleans up its
scope (tasks, handles, child processes). Traces and sessions live in the runtime's memory.

### Verified Demo

[12-library (DEMO-2026-0012)](../demos/12-library/README.md) (its `embedding.rs.txt` uses `Policy::from_file`,
`.ceiling`, `rt.scope` and `scope.stream`) and the two programs above: the host program was compiled and run
against `rivet 0.1.0-dev` at commit `f40d4aa` and again, unchanged, at commit `829ca43` (same output apart from
IDs and the port); the scope/ceiling program at `829ca43`.

## Errors and Recovery Reference

| Error / Code / Message | Surface | Cause | User-Visible Result | Recovery | Retry Safe | Related Feature |
|---|---|---|---|---|---|---|
| `validation.usage` "no source" | builder | no `.file`/`.source` | `Err` from `build()` | add a source | no | builder |
| `policy.invalid` | builder, `Policy::from_*`, `policy_from_json`, `request_restricted` | schema violation | `Err` with JSON pointer (`/restrict/…` for a restriction) | fix policy | no | policy |
| `permission.denied` "host ceiling: …" / "request restriction: …" | requests | the ceiling or the restriction does not grant it | `Err` | widen the ceiling / restriction (never beyond policy.json) | no | ceiling, restrict |
| `conflict.input_finished` | `DuplexHandle::send` | input finished or the request ended | `Err` | stop sending | no | scope |
| `not_found.trace` | `trace()` | request not recorded by this runtime | `Err` | trace requests with effects on the same runtime | no | trace |
| any request error | `request()` | see the root manual | `Err(RivetError)` | per code | per code | requests |

## Limitations

- Not published to crates.io in 0.1.0; the module paths above are the crate's current public modules and may be
  narrowed before 1.0.
- No persistent trace store: traces are in the runtime's memory and bounded (a
  [known limitation](man-2026-0001-rivet-manual.md#known-limitations)); export them with `export_trace` from the
  runtime that ran the request.
- Processes are sandboxed only on macOS (the Linux sandbox is gated; Windows/other OSes unsupported).

## Version Applicability

| Feature / Interface | Introduced | Changed | Deprecated / Removed | Applicable Environment |
|---|---|---|---|---|
| `Runtime`, `RuntimeBuilder`, `policy_from_json`, `start` | 0.1.0 | — | — | embedded |
| `Policy::from_file/from_json`, `.ceiling`, `Runtime::scope`, `request_restricted`, `export_trace`, `graph`, `consumer_stop` | 0.1.0 | — | — | embedded |

## Related Documents

- [Rivet manual](man-2026-0001-rivet-manual.md) · [Language guide](man-2026-0003-language-guide.md) ·
  [Policy guide](man-2026-0005-policy-and-io-manifest-guide.md) · [Serving](man-2026-0006-serving-and-surfaces.md)
- [SYS-2026-0001](../system/components/sys-2026-0001-compiler-and-catalog.md) ·
  [SYS-2026-0002](../system/runtime/sys-2026-0002-execution-scopes-and-dag.md)
- [API-2026-0004 Rust library API](../api/api-2026-0004-rust-library.md) (signatures).

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial library guide for 0.1.0 with a host program compiled and run against 0.1.0-dev commit f40d4aa. |
| 2 | 2026-09-28 | Claude | Fix batch through 829ca43: `Policy::from_file/from_json`, `.ceiling`, `Runtime::scope` with stream/duplex handles, typed `DataSink` stop, `request_restricted`, `export_trace`, `graph`, structured cancellation, `shutdown`, `ServeOptions.access_log`; second program compiled and run; host program re-run unchanged; obsolete limitation removed. |
