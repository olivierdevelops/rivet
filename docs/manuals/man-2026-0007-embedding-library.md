---
document_id: MAN-2026-0007
title: "Embedding Rivet as a Rust library"
document_type: manual
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
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
scope: Using rivet::Runtime from a Rust host — building from a file or in-memory source, supplying policy, unary and streaming requests, errors, declared outputs, the I/O manifest, policy drafts, traces, cancellation, sessions and serving the same runtime — with a program compiled and run against the 0.1.0-dev crate.
reason: PLAN-2026-0001 row D-40 — developer guide for the implemented library API; the example is a real Cargo project compiled against the repository, replacing the proposal-era sketch in demos/12-library.
related_documents: [MAN-2026-0001, MAN-2026-0003, MAN-2026-0005, MAN-2026-0006, SYS-2026-0001, SYS-2026-0002, DEMO-2026-0012, PLAN-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, manual, library, rust, embedding]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.1.0-dev (commit f40d4aa)"
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
 add the dependency ─► build a Runtime ─► request (unary) ─► request with a sink (stream)
   ─► handle errors ─► outputs / list / describe ─► io / generate_policy / trace ─► sessions + cancel ─► serve
```

## Concepts

```text
   Runtime::builder()
      .file("app.rivet")            ── or ──  .source("mem.rivet", text, root)
      .policy_file("ci.json")       ── or ──  .policy(policy_from_json(bytes, base_dir)?)   (default: discover)
      .build()?                     ── compile + load policy; errors are RivetError (syntax.*, policy.invalid, …)
         │
         ▼
   Runtime (cheap to clone; share across tasks)
      .request(id, params, sink)          ─► Completion        principal "local"
      .request_as(principal, id, …)       ─► Completion        a named principal (serve.principals applies)
      .dispatch_request(Request, sink)    ─► Completion        fully formed request (IDs, deadline, principal)
      .cancel(request_id, principal)      ─► CancelReceipt
      .list() / .describe(&ids) / .outputs(id, all)
      .io(&IoQuery) / .generate_policy(&ids) / .trace(request_id)
      .sessions()  ─► SessionDriver: open · send · finish_input · read · cancel
```

| Item | Path |
|---|---|
| `Runtime`, `RuntimeBuilder` | `rivet::Runtime` (re-exported from `rivet::orchestrator::runtime`) |
| `policy_from_json(bytes, base_dir)` | `rivet::orchestrator::runtime::policy_from_json` |
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

### Build a runtime

| Builder call | Effect | Failure |
|---|---|---|
| `.file(path)` | read and compile the entry file; discover `policy.json` beside it | `not_found.source`, `syntax.*`, `check.*` |
| `.source(path, text, root)` | compile in-memory text; `root` anchors relative paths; policy defaults to deny-all | `syntax.*` |
| `.policy_file(path)` | like `--policy PATH` | `policy.invalid` |
| `.policy(Policy)` | an already-parsed policy (use `policy_from_json`) | — |
| `.build()` | compile + load; neither `.file` nor `.source` → `validation.usage` "no source: use .file(PATH) or .source(…)" | any of the above |

`policy_from_json(bytes, base_dir)` uses the same strict schema v1 as `policy.json`; `base_dir` anchors relative
file targets.

### Unary and streaming requests

- `request(id, params, None)` returns `Completion { request_id, trace_id, result, data_count, effects }`.
- Pass `Some(Arc<dyn DataSink>)` to receive each emitted item as a `DataEvent { request_id, trace_id, seq, data }`;
  the sink must be `Send + Sync + 'static`. Returning an error from `send` fails the request.
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

### Inspect and govern

| Method | Returns | Notes |
|---|---|---|
| `list()` | `Catalog { entries }` | public operations and imported MCP tools |
| `describe(&[String])` | `Catalog` | empty slice = all |
| `outputs(Some(id), false)` / `outputs(None, true)` | `Vec<OutputReport>` (`to_json()`) | declared output/emits/receives/errors |
| `io(&IoQuery)` | `IoReport { manifest, rendered, diagnostics, exit_code }` | same options as `rivet io` (`by`, `kind`, `access`, `format`, `check_policy`, `strict`, `needs`, `check_files`, `trace_request_id`) |
| `generate_policy(&[&str])` | `PolicyDraft { grants, review, complete }`; `policy_json()` | never writes a file |
| `trace(request_id)` | `TraceResult { events, complete, … }` | only requests run by **this** runtime; pure requests record nothing (`not_found.trace`) |

### Sessions and cancellation

- `rt.cancel(request_id, principal)` cancels one of that principal's running top-level requests and returns a
  `CancelReceipt { request_id, session_id, state }` (`cancelled`, or the terminal state it already had).
- `rt.sessions()` returns the `SessionDriver` used by polling, WebSocket and `rivet.sessions.*`:
  `open`, `send`, `finish_input`, `read`, `cancel`. Use it to drive `receives` operations from a host.

### Serve the same runtime

`start(runtime, ServeOptions { listen, stdio, authenticator })` mounts every surface allowed by the runtime's
`serve` policy and returns a `ServeHandle` (`addr`, `receipt`, `shutdown().await`). An `authenticator`
(`Arc<dyn Authenticator>`) replaces the policy's `serve.auth` for library hosts.

### Expected Result and Side Effects

A runtime performs only the effects its policy grants. Dropping a request future cancels it and cleans up its
scope (tasks, handles, child processes). Traces and sessions live in the runtime's memory.

### Verified Demo

[12-library (DEMO-2026-0012)](../demos/12-library/README.md) and the program above, compiled and run against
`rivet 0.1.0-dev` (commit `f40d4aa`) with the `01-catalog` and `11-sandbox` bundles.

## Errors and Recovery Reference

| Error / Code / Message | Surface | Cause | User-Visible Result | Recovery | Retry Safe | Related Feature |
|---|---|---|---|---|---|---|
| `validation.usage` "no source" | builder | no `.file`/`.source` | `Err` from `build()` | add a source | no | builder |
| `policy.invalid` | builder, `policy_from_json` | schema violation | `Err` with JSON pointer | fix policy | no | policy |
| `not_found.trace` | `trace()` | request not recorded by this runtime | `Err` | trace requests with effects on the same runtime | no | trace |
| any request error | `request()` | see the root manual | `Err(RivetError)` | per code | per code | requests |

## Limitations

- Not published to crates.io in 0.1.0; the module paths above are the crate's current public modules and may be
  narrowed before 1.0.
- The proposal-era `rt.scope(…)` / `scope.stream(…)` / `Policy::from_file` sketch in
  `docs/demos/12-library/embedding.rs.txt` does not exist; use a `DataSink`, `policy_file` and `sessions()`.
- Traces are in memory and bounded.

## Version Applicability

| Feature / Interface | Introduced | Changed | Deprecated / Removed | Applicable Environment |
|---|---|---|---|---|
| `Runtime`, `RuntimeBuilder`, `policy_from_json`, `start` | 0.1.0 | — | — | embedded |

## Related Documents

- [Rivet manual](man-2026-0001-rivet-manual.md) · [Language guide](man-2026-0003-language-guide.md) ·
  [Policy guide](man-2026-0005-policy-and-io-manifest-guide.md) · [Serving](man-2026-0006-serving-and-surfaces.md)
- [SYS-2026-0001](../system/components/sys-2026-0001-compiler-and-catalog.md) ·
  [SYS-2026-0002](../system/runtime/sys-2026-0002-execution-scopes-and-dag.md)
- API-2026-0004 (Rust library API document, planned in PLAN-2026-0001) when published.

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial library guide for 0.1.0 with a host program compiled and run against 0.1.0-dev commit f40d4aa. |
