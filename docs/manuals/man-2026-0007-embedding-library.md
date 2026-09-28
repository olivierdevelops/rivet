---
document_id: MAN-2026-0007
title: "Embedding Rivet as a Rust library"
document_type: manual
status: active
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 3
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
scope: Using Rivet from a Rust host — the Cargo dependency on the git tag (package `rivet-runtime`) and its features, the crate-root facade, building from a file, in-memory source or a bare root, supplying policy and a host ceiling, `Runtime::call` envelopes and `request`, streaming with a sink or scopes, per-request restrictions, errors, loading `.rivet` files as `Module` objects, outputs, the I/O manifest, policy drafts, traces, cancellation, sessions and serving the same runtime — with programs compiled and run against the 0.2.0 release candidate.
reason: PLAN-2026-0001 row D-40 and PLAN-2026-0002 rows D-26, D-46 — developer guide for the implemented library API; the examples are real Cargo programs compiled against the repository.
related_documents: [PLAN-2026-0002, API-2026-0004, API-2026-0006, MIG-2026-0001, ADR-0005, MAN-2026-0009, MAN-2026-0001, MAN-2026-0003, MAN-2026-0005, MAN-2026-0006, SYS-2026-0001, SYS-2026-0002, DEMO-2026-0012, PLAN-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, manual, library, rust, embedding, facade, cargo-features, modules]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.2.0-rc (source at 6f9943f)"
next_review_date: 2026-10-29
---

# Embedding Rivet as a Rust library

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** library, registry, execution, policy, audit, sessions, serve

## Purpose

Call the same `.rivet` catalog from Rust with the same validation, policy broker, errors and results as the CLI
and every serve surface. Part of the [Rivet manual](man-2026-0001-rivet-manual.md). The
[12-library demo](../demos/12-library/README.md) holds the catalog; this volume holds compiled host programs;
signatures are in [API-2026-0004](../api/api-2026-0004-rust-library.md). Hosts in C, Python or Go use the C ABI
instead ([MAN-2026-0009](man-2026-0009-c-abi-and-ffi.md)).

The latest published release is 0.1.0; **0.2.0 is in progress** and this volume describes its release candidate
(source `6f9943f`, programs run on 2026-09-29, macOS 26.4). What changed for Rust hosts in 0.2.0: the package is
`rivet-runtime`, the public API is the crate-root **facade**, `Runtime::call` returns the **envelope** every surface
prints, Cargo **features** trim the build, and `Runtime::load` turns a `.rivet` file into a **`Module`** object.
Moving an existing host: [MIG-2026-0001 §Rust library](../migrations/mig-2026-0001-response-and-input-envelopes.md#rust-library).

## Reading Order

```text
 add the dependency + choose features ─► build a Runtime (policy + ceiling) ─► call (envelope) / request (Result)
   ─► request with a sink / scope.stream / scope.duplex ─► restrict one request ─► handle errors
   ─► load files as module objects ─► outputs / list / describe / graph ─► io / generate_policy / trace
   ─► sessions + cancel ─► serve
```

## Concepts

```text
   Runtime::builder()
      .file("app.rivet")            ── or ──  .source("mem.rivet", text, root)  ── or ── .root(dir) (no entry file)
      .policy_file("ci.json")       ── or ──  .policy(Policy::from_file(p)? | Policy::from_json(bytes)?)  (default: discover)
      .ceiling(Policy)              ── optional host ceiling: every attempt needs policy ∩ ceiling
      .build()?                     ── compile (+ imports) + load policy; errors are rivet::Error
         │                             (syntax.*, check.*, policy.invalid, unsupported.feature, …)
         ▼
   Runtime (cheap to clone; share across tasks)
      .call(InputEnvelope)                ─► ResponseEnvelope  the wire envelope; never Err
      .call_json(&str)                    ─► ResponseEnvelope  parses {operation, data, …} like HTTP
      .request(id, params, sink)          ─► rivet::Result<Completion>   principal "local"
      .request_restricted(id, p, restrict, sink) ─► narrowed to policy ∩ restrict
      .scope(|scope| … scope.stream(id, p) / scope.duplex(id, p) …)  owned handles, joined at the end
      .load(path) / .load_as(path, alias) ─► Module  (module.call / stream / duplex / operations)
      .request_as(principal, id, …) · .dispatch_request(Request, sink) · .cancel(request_id, principal)
      .list() / .describe(&ids) / .outputs(id, all) / .graph(&q)
      .io(&IoQuery) / .generate_policy(&ids) / .trace(request_id) / .export_trace(request_id, path)
      .sessions()  ─► SessionDriver: open · send · finish_input · read · cancel
```

```text
 call vs request — same dispatcher, two shapes
   rt.call(InputEnvelope::new("demo.add").data(json!({"a":2})))  ─▶ {"…","status":"ok","data":…}   (like HTTP)
   rt.request("demo.add", Value::from_json(&json!({"a":2})), None)?  ─▶ Completion { result, … }  (like Rust)
```

| Item (0.2.0 facade) | Path | 0.1.0 path |
|---|---|---|
| `Runtime`, `RuntimeBuilder` | `rivet::Runtime`, `rivet::RuntimeBuilder` | same |
| `InputEnvelope`, `ResponseEnvelope`, `EnvelopeStatus` | `rivet::…` | — (new) |
| `Module` | `rivet::Module` | — (new) |
| `Policy` (`from_file`, `from_json`) | `rivet::Policy` | `rivet::domain::policy::Policy` |
| `Scope`, `StreamHandle`, `DuplexHandle`, `DuplexSender` | `rivet::…` | `rivet::orchestrator::setup_library` |
| `Envelope` (`Data`, `Result`, `Error`), `Completion`, `DataEvent` | `rivet::…` | `rivet::domain::contracts` |
| `Value` (`from_json`, `to_json`) | `rivet::Value` | `rivet::domain::Value` |
| `Error`, `ErrorKind`, `Result<T>` | `rivet::Error`, `rivet::ErrorKind`, `rivet::Result` | `rivet::domain::{RivetError, RivetResult}` |
| `DataSink` (async trait) | `rivet::DataSink` | `rivet::domain::ports::DataSink` |
| `IoQuery`, `IoReport`, `PolicyDraft`, `Catalog`, `OutputReport`, `Principal`, `GraphQuery`, `SessionLimits`, `ModuleSummary` | `rivet::types::…` | `rivet::domain::…` |
| `highlight::{tokens, highlight}` | `rivet::highlight` | — (new) |
| `build_features()`, `VERSION`, `ABI_VERSION` | `rivet::…` | — (new) |
| `start`, `ServeOptions`, `ServeHandle`; `policy_from_json`; `Request`, `TraceResult`, session inputs | `rivet::internal::…` (**not** in the facade; may change) | `rivet::orchestrator::…` / `rivet::domain::…` |

## Installation and Setup

### Add the dependency and choose features

Rivet is not on crates.io (its `capy-core` git dependency blocks `cargo publish`); depend on the **git tag**. The
package is `rivet-runtime`, the library name stays `rivet`, so code keeps `use rivet::…`:

```toml
[dependencies]
rivet = { package = "rivet-runtime", git = "https://github.com/olivierdevelops/rivet", tag = "v0.2.0" }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
serde_json = "1"
async-trait = "0.1"          # only to implement DataSink
```

| Feature | Default | Pulls in | Needed for |
|---|---|---|---|
| `serve` | yes | axum | embedding `serve` (`rivet::internal::orchestrator::setup_serve::start`) |
| `grpc` | yes | tonic, prost, prost-reflect | bundles with `connector … grpc` / `grpc X.Method` |
| `quic` | yes | quinn, h3 | `with quic …`, HTTP/3 (`version 3`, `version prefer [3, …]`) |
| `oauth` | yes | keyring stores | `auth NAME oauth2` profiles, `store keychain` |
| `cli` | no | clap | only the `rivet` binary; a library host never needs it |

```text
 lean host: rivet = { package = "rivet-runtime", git = "…", tag = "v0.2.0", default-features = false }
     bundle uses grpc/quic/oauth? ── no ─▶ builds and runs; smaller binary, fewer dependencies
                                  └ yes ─▶ Runtime::builder().build() → Err(unsupported.feature), details.feature
                                           (every use listed, before anything runs)
 check at run time: rivet::build_features()  → ["serve","grpc","quic","oauth"] with the defaults
```

The crate uses edition 2024 and Rust 1.90.0. Copy the repository's `Cargo.lock` into your host project to get the
same dependency versions that were tested. Platforms: macOS and Linux; Windows is not supported
([INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)).

## Task-Oriented Workflows

### Journey Overview

```text
 host main ─► builder().file(p).build() ── Err(rivet::Error) ─► print e.code, exit e.exit_code()
                      │ Ok(rt)
                      ▼
            rt.call(InputEnvelope::new("demo.add").data(json!({…}))).await
                      │
                      ├─ env.status() == Ok        ─► env.to_json()["data"]
                      └─ Error / Cancelled         ─► env.to_json()["error"]["code"]   (never an Err)
            or rt.request("demo.add", params, None).await? ─► Completion { result, effects, … }
```

### Complete host program

This program uses only the facade (plus `rivet::internal` for `serve`, which is not in the facade in 0.2.0). It was
compiled with `cargo build --release` in a scratch Cargo project depending on the repository by path (default
features) and run with the two demo bundles as arguments.

```rust
use rivet::internal::orchestrator::setup_serve::{ServeOptions, start};
use rivet::types::IoQuery;
use rivet::{DataEvent, DataSink, InputEnvelope, Policy, Runtime, Value};
use serde_json::json;
use std::sync::{Arc, Mutex};

/// Collects emitted data items (the `emits` stream) of one request.
struct Collect(Mutex<Vec<serde_json::Value>>);

#[async_trait::async_trait]
impl DataSink for Collect {
    async fn send(&self, event: DataEvent) -> rivet::Result<()> {
        self.0.lock().unwrap().push(event.data.to_json());
        Ok(())
    }
}

#[tokio::main]
async fn main() -> rivet::Result<()> {
    let catalog = std::env::args().nth(1).expect("path to 01-catalog/app.rivet");
    let sandbox = std::env::args().nth(2).expect("path to 11-sandbox/app.rivet");

    // 1. Build from a file; policy.json beside it is discovered (absent = deny-by-default).
    let rt = Runtime::builder().file(&catalog).build()?;
    for e in rt.list()?.entries {
        println!("op {} — {}", e.id, e.name);
    }

    // 2. One envelope for every outcome: the same JSON as the CLI, HTTP, MCP and the C ABI.
    let out = rt.call(InputEnvelope::new("demo.add").data(json!({"a": 2, "b": 3}))).await;
    println!("call => {}", out.to_json_string());
    let bad = rt.call(InputEnvelope::new("demo.add").data(json!({"a": "x"}))).await;
    println!("call (bad) => {}", bad.to_json_string());

    // 3. Rust-idiomatic request: Result<Completion>; a streaming request feeds the sink.
    let c = rt.request("demo.add", Value::from_json(&json!({"a": 2, "b": 3})), None).await?;
    println!("request => result {} effects {:?}", c.result.to_json(), c.effects);
    let sink = Arc::new(Collect(Mutex::new(Vec::new())));
    let c = rt.request("demo.countdown", Value::from_json(&json!({})), Some(sink.clone())).await?;
    println!("countdown data {:?} result {}", sink.0.lock().unwrap(), c.result.to_json());

    // 4. Failures of `request` are rivet::Error values with kind, code and the CLI exit code.
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
    let denied = sb.call(InputEnvelope::new("data.private")).await;
    println!("denied => {}", denied.to_json_string());

    // 7. In-memory source + in-memory policy (no file reads).
    let src = "operation hi.say\n    output text\n    return \"hi\"\nend\n";
    let mem = Runtime::builder().source("mem.rivet", src, ".").policy(Policy::from_json(br#"{"version":1}"#)?).build()?;
    println!("mem => {}", mem.call(InputEnvelope::new("hi.say")).await.to_json_string());

    // 8. Serve the same Runtime on an ephemeral loopback port, then stop it (needs the `serve` feature;
    //    `start` is not in the facade in 0.2.0).
    let handle = start(rt.clone(), ServeOptions { listen: Some("127.0.0.1:0".into()), ..ServeOptions::default() }).await?;
    println!("serving on {}", handle.addr.unwrap());
    handle.shutdown().await;
    println!("build features {:?}", rivet::build_features());
    Ok(())
}
```

Run:

```bash
cargo run --release -- /path/to/rivet/docs/demos/01-catalog/app.rivet /path/to/rivet/docs/demos/11-sandbox/app.rivet
```

Output (**IDs and the ephemeral port vary**; `source.file` is the path as given on the command line):

```text
op demo.greet — Greet a person
op demo.add — Add two integers
op demo.health — Check availability
op demo.countdown — Count down
call => {"request_id":"req_015a0a0dfd","trace_id":"tr_015a0a0dfd","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}
call (bad) => {"request_id":"req_02dbd209d2","trace_id":"tr_02dbd209d2","operation":"demo.add","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.type","message":"parameter `a` must be an integer, got text","retryable":false,"operation_id":"demo.add","details":{"field":"a"}},"effects":"none","data_count":0}
request => result 5 effects None
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
trace req_015ad256e5 events=1 complete=true
denied => {"request_id":"req_02db3276a2","trace_id":"tr_02db3276a2","operation":"data.private","type":"result","status":"error","data":null,"error":{"kind":"permission","code":"permission.denied","message":"allow_read read on ./data/private/secret.json denied: deny allow_read ./data/private/**","retryable":false,"source":{"file":"/path/to/rivet/docs/demos/11-sandbox/app.rivet","line":26,"column":5,"end_line":26,"end_column":59},"operation_id":"data.private","details":{"capability":"allow_read","access":"read","target":"./data/private/secret.json"}},"effects":"none","data_count":0}
mem => {"request_id":"req_015af96985","trace_id":"tr_015af96985","operation":"hi.say","type":"result","status":"ok","data":"hi","error":null,"effects":"none","data_count":0}
serving on 127.0.0.1:52767
build features ["serve", "grpc", "quic", "oauth"]
```

(`/path/to/rivet` replaces the absolute checkout path of the capture.) The repository's own compiled example of
the facade is [`examples/embed.rs`](../../examples/embed.rs) (`cargo run --example embed`).

### Scopes, ceilings, restrictions and trace export

The second program — a host ceiling, `scope.stream`, `scope.duplex`, a typed sink stop, `request_restricted`,
`export_trace` and `load_as` — is compiled and run in [API-2026-0004 §Examples](../api/api-2026-0004-rust-library.md#examples)
(0.2.0-rc, facade only). Its flow:

```text
  Policy::from_json(policy) ──┐
  Policy::from_json(ceiling) ─┴─ .ceiling ─► rt ─► call / call_json ─► envelopes
                                             rt ─► scope ─► stream demo.countdown ─► 3 data records, result
                                                        └──► duplex chat.echo ─► send ✓ / send ✗ validation.input / finish / next
                                             rt ─► request + FirstOnly sink ─► consumer.stop (cancelled)
                                             rt ─► request_restricted ─► request restriction denial
                                             rt ─► export_trace ─► host ceiling denial
                         (no ceiling) rt2 ─► export_trace ✓ ─► again ✗ conflict.already_exists
                                      rt2 ─► load_as("./lib/billing.rivet","billing") ─► module call ✓ ─► again ✗ check.import_duplicate
```

Records from `StreamHandle::next` render with `env.record()` as stream records
(`{"request_id",…,"operation":"demo.countdown","type":"data","seq":1,"data":3,"error":null}`); the library's terminal
`type: "result"` record has no `seq` (a known difference from the other surfaces, API-2026-0006).

### Load files as module objects

Why (0.2.0): a host that discovers `.rivet` files at run time (plugins, per-tenant catalogs) loads them into one
runtime as **module objects**, namespaced by an alias, under the runtime's single policy. Inside `.rivet` files the
same idea is `import "./users.rivet" as users` ([MAN-2026-0003 §Modules](man-2026-0003-language-guide.md#modules-import)).

```text
 Runtime::builder().root("examples/modules").build()     empty catalog; root confines every module path
   ├─ rt.load("./users.rivet")                 ─▶ Module alias "users" (the file stem): users.get, users.list
   ├─ rt.load_as("./lib/billing.rivet", "billing") ─▶ Module "billing": billing.invoice
   ├─ users.call("get", json!({"id": 42}))     ─▶ envelope, operation "users.get"
   ├─ rt.call(InputEnvelope::new("users.list")) ─▶ the runtime sees the namespaced IDs too
   └─ rt.load("./users.rivet") again           ─▶ Err(check.import_duplicate), catalog unchanged
 loads are serialized; each swaps in a new catalog snapshot; a running request keeps the snapshot it started on
```

[`examples/modules.rs`](../../examples/modules.rs), run with `cargo run --release --example modules` from the checkout:

```rust
use rivet::{InputEnvelope, Runtime};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // No policy.json: deny-by-default, which these pure modules never need.
    let rt = Runtime::builder().root("examples/modules").build()?;

    let users = rt.load("./users.rivet")?; // alias "users" (the file stem)
    for op in users.operations() {
        println!("{} — {}", op.id, op.description.unwrap_or_default());
    }
    let out = users.call("get", json!({"id": 42})).await;
    println!("{}", out.to_json_pretty());

    let billing = rt.load_as("./lib/billing.rivet", "billing")?;
    let invoice = billing.call("invoice", json!({"user": 7})).await;
    println!("{}", invoice.to_json_pretty());

    // The runtime's own dispatcher sees the namespaced IDs too.
    let same = rt
        .call(InputEnvelope::new("users.list").data(json!({})))
        .await;
    println!("{}", same.to_json_string());

    // Loading the same alias twice is refused; the catalog stays as it was.
    if let Err(e) = rt.load("./users.rivet") {
        println!("{e}");
    }
    Ok(())
}
```

Output (IDs vary):

```text
get — One user by id.
list — The first two users (calls inside a module use its own IDs).
{
  "request_id": "req_0144d1f095",
  "trace_id": "tr_0144d1f095",
  "operation": "users.get",
  "type": "result",
  "status": "ok",
  "data": {
    "id": 42,
    "name": "Hello, user 42"
  },
  "error": null,
  "effects": "none",
  "data_count": 0
}
{
  "request_id": "req_02c54a0e7a",
  "trace_id": "tr_02c54a0e7a",
  "operation": "billing.invoice",
  "type": "result",
  "status": "ok",
  "data": {
    "user": {
      "id": 7,
      "name": "Hello, user 7"
    },
    "amount": 20.0
  },
  "error": null,
  "effects": "none",
  "data_count": 0
}
{"request_id":"req_03463a97ff","trace_id":"tr_03463a97ff","operation":"users.list","type":"result","status":"ok","data":[{"id":1,"name":"Hello, user 1"},{"id":2,"name":"Hello, user 2"}],"error":null,"effects":"none","data_count":0}
check.import_duplicate (syntax): a module named `users` is already loaded; use load_as(path, alias)
```

| `Module` method | Returns |
|---|---|
| `alias()`, `file()`, `summary()`, `warnings()` | the alias, the file, a `ModuleSummary`, load warnings (`check.module_policy_ignored`) |
| `operations()` | `Vec<RegistryEntry>` with **short** IDs (`get`); `qualified("get")` → `users.get` |
| `describe(id)`, `outputs(id)` | `RegistryEntry` / `OutputReport` |
| `call(id, serde_json::Value).await` | `ResponseEnvelope` (namespaced `operation`) |
| `stream(&scope, id, Value)`, `duplex(&scope, id, Value)` | scope-owned handles, like `Scope::stream`/`duplex` |

Load failures: `not_found.import` (missing file), `permission.import_outside_root` (path escapes the root),
`check.import_duplicate` (alias already loaded), `check.import_collision` (an ID, connector or auth profile collides),
`syntax.*` / `check.*` from the module's own source, `unsupported.feature`. Demo: `docs/demos/17-modules/`
(DEMO-2026-0019, being added). C hosts: `rivet_load` ([MAN-2026-0009](man-2026-0009-c-abi-and-ffi.md#module-objects)).

### Build a runtime

| Builder call | Effect | Failure |
|---|---|---|
| `.file(path)` | read and compile the entry file and its `import`s; discover `policy.json` beside it | `not_found.source`, `syntax.*`, `check.*`, import codes, `unsupported.feature` |
| `.source(path, text, root)` | compile in-memory text; `root` anchors relative paths; policy defaults to deny-all | `syntax.*` |
| `.root(dir)` | (0.2.0) no entry file: an empty catalog that `rt.load` fills; `dir` confines module paths | — |
| `.policy_file(path)` | like `--policy PATH` | `policy.invalid` |
| `.policy(Policy)` | an already-parsed policy: `Policy::from_file(path)?` (targets relative to that file) or `Policy::from_json(bytes)?` (targets relative to the bundle root) | `policy.invalid` from the constructor |
| `.ceiling(Policy)` | **host ceiling**: every attempt must be allowed by the loaded policy **and** the ceiling; deny in either wins; `limits` narrow to the smaller value; a second call intersects | denials read `… denied: host ceiling: no grant for …` |
| `.session_limits(SessionLimits)` | host caps for sessions (count, queues, idle lease, retention) | — |
| `.build()` | compile + load; none of `.file`, `.source`, `.root` → `validation.usage` | any of the above |

`Policy::from_json`, `Policy::from_file` and `rivet::internal::orchestrator::runtime::policy_from_json(bytes,
base_dir)` use the same strict schema v1 as `policy.json`; `base_dir` anchors relative file targets.

### Calls, unary and streaming requests

- `call(InputEnvelope)` / `call_json(&str)` return the `ResponseEnvelope` (`.status()`, `.to_json()`,
  `.to_json_string()`, `.to_json_pretty()`); failures are envelopes with `status: "error"`, never `Err`.
  `InputEnvelope::new(id).data(json).deadline_ms(ms).restrict(value)` builds the input.
- `request(id, params, None)` returns `Completion { request_id, trace_id, result, data_count, effects }`
  (`ResponseEnvelope::from(completion)` gives the wire form).
- Pass `Some(Arc<dyn DataSink>)` to receive each emitted item as a `DataEvent { request_id, trace_id, seq, data }`;
  the sink must be `Send + Sync + 'static`. Return `Ok(())` to continue, `Err(rivet::Error::consumer_stop())` to
  **stop** (the producer stops, cleanup runs, the request ends `cancelled` / `consumer.stop` — not a failure), or
  any other error to fail the request (`consumer_failed`).
- `rt.scope(|scope| async move { … })` owns streams and duplexes: `scope.stream(id, params)` returns a
  `StreamHandle` whose `next().await?` yields `Envelope::Data` items, then one `Envelope::Result`, then `None`
  (a terminal error is returned as `Err`); `scope.duplex(id, params)` adds `send(item)` (validated against
  `receives`: `validation.input`; after `finish_send` → `conflict.input_finished`), `finish_send()` and
  `into_split()`. When the body returns, unfinished requests are cancelled and joined (5 s each, then aborted).
- `request_restricted(id, params, restrict, sink)` narrows one request (and its nested calls) to
  `policy ∩ restrict`, where `restrict` is `{"grants": [...]}`; it never widens.
- Params and results are `rivet::Value`; convert with `Value::from_json(&serde_json::Value)` and
  `value.to_json()`. `call` takes `serde_json` directly.
- The default deadline is 30 s; set `InputEnvelope::deadline_ms(ms)` with `call`, or use `dispatch_request` with
  `Request { deadline_ms, … }` (built by `rt.new_request(id, params, principal)`; `Request` is internal) to change it.

### Handle errors

With `call`, read `env.status()` and the `error` object of the envelope. Every other failure is a `rivet::Error`
with `kind` (`rivet::ErrorKind`), `code`, `message`, `details`, `effects`, `operation_id`, `request_id` and an
`exit_code()` identical to the CLI's. Typical matches:

| `e.code` | Meaning | Host action |
|---|---|---|
| `validation.*` | bad params | report to the caller; do not retry |
| `permission.denied` | policy or principal denies | fix policy; `e.details` names capability, access, target |
| `not_found.operation` | unknown or private ID | check the catalog |
| `timeout.*` | deadline | retry only if the operation is replay-safe |
| `cancelled.request` | cancelled via `rt.cancel` | — |
| `consumer.stop` | your sink returned `rivet::Error::consumer_stop()` | expected; not a failure |
| `unsupported.feature` | the bundle needs a Cargo feature this build lacks (`details.feature`) | enable the feature |
| `check.import_duplicate`, `not_found.import`, `permission.import_outside_root` | `load` / `load_as` refused | new alias / fix the path |
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

`rivet::internal::orchestrator::setup_serve::start(runtime, ServeOptions { listen, stdio, authenticator, access_log })`
(needs the `serve` feature; not in the facade in 0.2.0) mounts every surface allowed by the
runtime's `serve` policy plus `GET /v1/health` and returns a `ServeHandle` (`addr`, `receipt`,
`shutdown().await` — the SIGTERM drain). An `authenticator` (`Arc<dyn Authenticator>`) replaces the policy's
`serve.auth` for library hosts; `access_log` (`Arc<dyn Fn(&str) + Send + Sync>`) receives one JSON access-log line
per request instead of stderr.

### Expected Result and Side Effects

A runtime performs only the effects its policy grants. Dropping a request future cancels it and cleans up its
scope (tasks, handles, child processes). Traces and sessions live in the runtime's memory.

### Verified Demo

[12-library (DEMO-2026-0012)](../demos/12-library/README.md) (its `embedding.rs.txt` is being moved to the facade
in P4; the compiled form is `examples/embed.rs`), the host program above and `examples/modules.rs`, compiled and run
on 2026-09-29 against the 0.2.0 release candidate (source `6f9943f`), and the API-2026-0004 program. Modules demo:
`docs/demos/17-modules/` (being added).

## Errors and Recovery Reference

| Error / Code / Message | Surface | Cause | User-Visible Result | Recovery | Retry Safe | Related Feature |
|---|---|---|---|---|---|---|
| `validation.usage` "no source" | builder | no `.file`/`.source`/`.root` | `Err` from `build()` | add a source | no | builder |
| `unsupported.feature` | builder, `load` | a compiled-out Cargo feature is needed | `Err`, `details.feature` | enable the feature | no | features |
| `check.import_duplicate`, `check.import_collision`, `not_found.import`, `permission.import_outside_root`, `limit.imports` | `load`, `load_as`, `build` | module refused | `Err` | new alias / rename / fix the path / flatten | no | modules |
| `policy.invalid` | builder, `Policy::from_*`, `policy_from_json`, `request_restricted` | schema violation | `Err` with JSON pointer (`/restrict/…` for a restriction) | fix policy | no | policy |
| `permission.denied` "host ceiling: …" / "request restriction: …" | requests | the ceiling or the restriction does not grant it | `Err` | widen the ceiling / restriction (never beyond policy.json) | no | ceiling, restrict |
| `conflict.input_finished` | `DuplexHandle::send` | input finished or the request ended | `Err` | stop sending | no | scope |
| `not_found.trace` | `trace()` | request not recorded by this runtime | `Err` | trace requests with effects on the same runtime | no | trace |
| any request error | `request()` | see the root manual | `Err(rivet::Error)` (`call`: an error envelope) | per code | per code | requests |

## Limitations

- Not published to crates.io; depend on the git tag. `rivet::internal::…` (serve embedding, `Request`, session
  input types) is not a stable API in 0.2.x.
- No persistent trace store: traces are in the runtime's memory and bounded (a
  [known limitation](man-2026-0001-rivet-manual.md#known-limitations)); export them with `export_trace` from the
  runtime that ran the request.
- Processes are sandboxed only on macOS (the Linux sandbox is gated: `unsupported.sandbox_backend`, exit 5);
  macOS and Linux are the supported platforms, Windows is not ([INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)).
- Library terminal stream records (`env.record()`) have no `seq`; the other surfaces' do.

## Version Applicability

| Feature / Interface | Introduced | Changed | Deprecated / Removed | Applicable Environment |
|---|---|---|---|---|
| `Runtime`, `RuntimeBuilder`, `policy_from_json`, `start` | 0.1.0 | 0.2.0: package `rivet-runtime`; `policy_from_json` and `start` under `rivet::internal` | `rivet::domain::…` / `rivet::orchestrator::…` paths removed | embedded |
| Facade, `call`/`call_json`, `InputEnvelope`/`ResponseEnvelope`, `.root`, `load`/`load_as`/`Module`, Cargo features, `highlight`, `build_features` | 0.2.0 | — | — | embedded (macOS, Linux) |
| `Policy::from_file/from_json`, `.ceiling`, `Runtime::scope`, `request_restricted`, `export_trace`, `graph`, `consumer_stop` | 0.1.0 | — | — | embedded |

## Related Documents

- [Rivet manual](man-2026-0001-rivet-manual.md) · [Language guide](man-2026-0003-language-guide.md) ·
  [Policy guide](man-2026-0005-policy-and-io-manifest-guide.md) · [Serving](man-2026-0006-serving-and-surfaces.md)
- [SYS-2026-0001](../system/components/sys-2026-0001-compiler-and-catalog.md) ·
  [SYS-2026-0002](../system/runtime/sys-2026-0002-execution-scopes-and-dag.md)
- [API-2026-0004 Rust library API](../api/api-2026-0004-rust-library.md) (signatures) · [API-2026-0006 envelopes](../api/api-2026-0006-envelopes.md) ·
  [MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md) · [ADR-0005 workspace and features](../decisions/adr-0005-workspace-package-and-features.md) ·
  [MAN-2026-0009 C ABI](man-2026-0009-c-abi-and-ffi.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial library guide for 0.1.0 with a host program compiled and run against 0.1.0-dev commit f40d4aa. |
| 2 | 2026-09-28 | Claude | Fix batch through 829ca43: `Policy::from_file/from_json`, `.ceiling`, `Runtime::scope` with stream/duplex handles, typed `DataSink` stop, `request_restricted`, `export_trace`, `graph`, structured cancellation, `shutdown`, `ServeOptions.access_log`; second program compiled and run; host program re-run unchanged; obsolete limitation removed. |
| 3 | 2026-09-29 | Claude | 0.2.0 (D-26, D-46): Cargo dependency on the git tag (`rivet-runtime`) with a features table and lean-build flow; facade path table (0.2.0 vs 0.1.0); `call`/`call_json` envelopes; host program ported to the facade and re-run on the 0.2.0-rc; scopes/ceilings program linked to API-2026-0004 (not duplicated); new **Load files as module objects** section with `examples/modules.rs` and its real output; `.root`; errors, limitations and version rows. |
