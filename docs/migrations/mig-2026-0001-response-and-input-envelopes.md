---
document_id: MIG-2026-0001
title: "Migrating from Rivet 0.1.0 to 0.2.0: response and input envelopes, CLI flags and the Rust facade"
document_type: migration
status: active
created_date: 2026-09-29
last_updated: 2026-09-30
document_revision: 3
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [cli, http, poll, ws, mcp, library]
affected_versions:
  from: "0.1.0"
  to: "0.2.x"
applicable_environments: [development, embedded, server]
audience: [developers, integrators, operators]
scope: Every client-visible breaking change between Rivet 0.1.0 and 0.2.0 — the output envelope on the CLI, HTTP, SSE, polling, WebSocket and MCP; the input envelope and its deprecated aliases; CLI flags (`--data`, `--input`, `--params`); the installation change (`--features cli`); and the Rust package and facade move (`rivet-runtime`, `rivet::domain::X` → `rivet::X`) — with before/after examples, a jq mapping, the 0.2 → 0.3 deprecation timeline, a client checklist and a rollback to v0.1.0.
reason: PLAN-2026-0002 row D-09 (TASK-074, requirement R5) — DOCUMENTATION §4.21 requires a migration document with a rollback plan for a breaking change; REL-0.2.0 "Breaking Changes" links here.
related_documents: [PROP-2026-0002, PLAN-2026-0002, ADR-0004, ADR-0005, API-2026-0006, API-2026-0001, API-2026-0002, API-2026-0003, API-2026-0004, MAN-2026-0004, MAN-2026-0007]
supersedes: null
superseded_by: null
tags: [rivet, migration, envelope, breaking-change, deprecation, cli, rust]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-29
---

# Migrating from Rivet 0.1.0 to 0.2.0: response and input envelopes, CLI flags and the Rust facade

> **Status:** Active
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-30
> **Affected Versions:** 0.1.0 to 0.2.x
> **Owner:** Project maintainer
> **Affected Components:** envelope, cli, http, sse, poll, ws, mcp, library, packaging

## Summary

Rivet 0.2.0 answers with **one envelope on every surface** and accepts **one input envelope**. Clients that read
0.1.0 output (`"result"`, the separate error envelope, SSE `event: error`) must change. Clients that **send**
0.1.0 input (`{id, params}`, `--params`) keep working through 0.2.x, with a deprecation signal.

```text
                      0.1.0                                         0.2.0
 input    {"id":"demo.add","params":{…}}              {"operation":"demo.add","data":{…}}   (old keys: deprecated)
 success  {"request_id","trace_id","result":5,        {"request_id","trace_id","operation":"demo.add","type":"result",
           "data_count":0,"effects":"none"}             "status":"ok","data":5,"error":null,"effects":"none","data_count":0}
 failure  {"request_id","trace_id",                   {…,"type":"result","status":"error","data":null,
           "error":{kind,code,message,retryable,        "error":{kind,code,message,retryable,…},"effects":"none","data_count":0}
                    "effects":"none"}}                  (effects moved to the top level)
 records  {…,"seq":1,"type":"data","data":3}           {…,"operation":…,"type":"data","seq":1,"data":3,"error":null}
 SSE      event: data | result | error                event: data | result      (errors: event result + status)
 CLI      --params JSON                               --data JSON | --input FILE|-    (--params: deprecated)
 install  cargo install --path .                      cargo install … --features cli
 Rust     package rivet, rivet::domain::Value …       package rivet-runtime, rivet::Value …
```

The canonical shape is [API-2026-0006](../api/api-2026-0006-envelopes.md). All "after" examples below are real
output of the 0.2.0 release candidate (`main` at `e7ed8ed`, 2026-09-29); **request and trace IDs differ on every
run**. "Before" examples are the 0.1.0 captures recorded in the 0.1.0 API documents.

## Background

0.1.0 had five slightly different shapes: a completion (`result`), an error envelope, stream records with
`type: data|result|error`, WebSocket error frames and MCP results without `structuredContent`. A client needed
one parser per surface, and a success and a failure did not share keys. PROP-2026-0002 (R1–R6) replaced them
with one envelope whose keys are always present.

## Problem

Any code that does `body.result`, checks for the presence of `error` to detect failure, switches on SSE
`event: error`, reads `error.effects`, or imports `rivet::domain::…` breaks on 0.2.0.

## Goals

- Every client can switch to 0.2.0 by following one checklist per surface.
- 0.1.0 **inputs** keep working for the whole 0.2.x line, with visible deprecation signals.
- A rollback to v0.1.0 is one pin away.

## Non-Goals

- No compatibility mode for 0.1.0 **outputs**: there is no flag that restores `"result"`.
- The `.rivet` language is unchanged by this migration (globals and `import` are additions). The script-level
  `completion` object of a request stream keeps its 0.1.0 shape.

## Scope

CLI, HTTP REST, SSE, polling, WebSocket `rivet.v1`, MCP (`/mcp` and `serve --stdio`), the Rust library and the
installation of the `rivet` binary. The C ABI is new in 0.2.0 and has nothing to migrate.

## Affected Systems

Any program that runs `rivet … --json`, parses `rivet request` output, calls `rivet serve` over HTTP, WebSocket
or MCP, or depends on the `rivet` crate.

## Affected Versions

From 0.1.0 (every 0.1.x build) to 0.2.x. The input aliases are removed in 0.3.0.

## Proposed or Current Design

### Field mapping

| 0.1.0 | 0.2.0 | Note |
|---|---|---|
| `result` | `data` (with `status: "ok"`) | For every unary answer and terminal record |
| presence of `error` | `status` ∈ `error`, `cancelled` | `error` is now always present (`null` on success) |
| `error.effects` | top-level `effects` | Same values: `none`, `committed`, `partial`, `unknown` |
| — | `operation` | New: the operation or built-in the answer belongs to |
| `type: "error"` record | `type: "result"`, `status: "error"` | A stream always ends with one `type: "result"` record |
| — | `status: "accepted"` | New: polling `POST /v1/requests` and MCP calls of streaming tools |
| built-in bare JSON (`GET /v1/operations` → `{operations,…}`) | envelope with `data` = that JSON | `operation` is the built-in (`rivet.list`, …) |
| input `id` | `operation` | `id` accepted through 0.2.x |
| input `params` | `data` | `params` accepted through 0.2.x |
| CLI `--params` | `--data` | `--params` accepted through 0.2.x |

### jq mapping (read either version)

```sh
# normalise a 0.1.0 or 0.2.0 answer to {ok, value, error}
jq '{ ok:    (if has("status") then .status == "ok" else (has("error") | not) end),
      value: (if has("status") then .data else .result end),
      error: .error }'

# 0.2.0 only: the result, or fail with the code
jq -e 'if .status == "ok" then .data else error("\(.error.code): \(.error.message)") end'

# 0.2.0 stream (NDJSON / SSE data lines): items, then the final value
jq -c 'select(.type == "data") | .data'
jq -c 'select(.type == "result") | {status, data, error, data_count}'

# 0.1.0 input -> 0.2.0 input
jq '{operation: .id, data: (.params // {})} + (del(.id, .params))'
```

### Migration journey

```text
 [0.1.0 client] ──upgrade server to 0.2.0──▶ inputs still accepted (Deprecation: true / warning)
      │                                            │
      │                                            └─▶ outputs changed ─▶ client parser FAILS on .result
      │
      └─▶ recommended order:
           1. update the parser to read .status/.data/.error (the jq normaliser reads both versions)
           2. upgrade the server / CLI to 0.2.0
           3. switch inputs to {operation, data} / --data ; watch `deprecated` in the access log reach 0
           4. before 0.3.0: no `deprecation: true` header, no warning[deprecated.*] on stderr
```

## Alternatives

A dual-output mode (a header or flag that restores 0.1.0 output) was rejected in PROP-2026-0002: it would keep
two shapes alive on every surface. Input aliases were kept because they cost one parser branch.

## Risks

| Risk | Detection | Mitigation |
|---|---|---|
| A client checks `if "error" in body` to detect failure | Every 0.2.0 answer has `error` (null on success): all calls look failed | Check `status` instead |
| A client reads `body.result` | `undefined`/`KeyError` | Read `data` |
| An SSE client waits for `event: error` | Never arrives; the terminal event is `event: result` with `status: "error"` | Switch on the record's `status` |
| A Rust host imports `rivet::domain::…` | Compile error `could not find domain in rivet` | Use the facade paths below |
| A script calls `cargo install --path .` without features | Cargo builds no `rivet` binary (it has `required-features = ["cli"]`) | Add `--features cli` |

## Security Considerations

None of the changes widen authority. The `Deprecation` signal names no secrets; the access log records only
`"deprecated":1` (never bodies, params or tokens).

## Operational Considerations

Watch the access log (`rivet serve` stderr, one JSON line per request) for legacy clients:

```text
{"time":"2026-09-28T20:19:14.585Z","surface":"http","method":"POST","route":"/v1/request","principal":"local","operation":"demo.add","status":200,"duration_ms":0,"deprecated":1}
```

```sh
rivet --file app.rivet serve 2> access.log &
jq -r 'select(.deprecated == 1) | "\(.principal) \(.surface) \(.operation)"' access.log | sort | uniq -c
```

## Compatibility

### CLI

Before (0.1.0):

```text
$ rivet request --file app.rivet demo.add --params '{"a":2,"b":3}'
{"request_id":"req_01fcf2a8bd","trace_id":"tr_01fcf2a8bd","result":5,"data_count":0,"effects":"none"}
```

After (0.2.0):

```text
$ rivet --file docs/demos/01-catalog/app.rivet request demo.add --data '{"a":2,"b":3}'
{"request_id":"req_011f655065","trace_id":"tr_011f655065","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}

$ rivet --file docs/demos/01-catalog/app.rivet request demo.add --params '{"a":2,"b":3}'
warning[deprecated.params]: --params is deprecated; use --data (removed in 0.3.0)
{"request_id":"req_011d5f08ad","trace_id":"tr_011d5f08ad","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}

$ echo '{"id":"demo.add","params":{"a":40,"b":2}}' | rivet --file docs/demos/01-catalog/app.rivet request --input -
warning[deprecated.input]: input keys `id` and `params` are deprecated; use `operation` and `data` (removed in 0.3.0)
{"request_id":"req_011c1c6ad5","trace_id":"tr_011c1c6ad5","operation":"demo.add","type":"result","status":"ok","data":42,"error":null,"effects":"none","data_count":0}
```

- `rivet request` prints an envelope (as in 0.1.0, no `--json` is needed). A success goes to **stdout**; an
  error or cancelled envelope goes to **stderr**, with the registry exit code.
- Every `--json` command (`list`, `describe`, `outputs`, `io`, `check`, `policy generate`, …) prints an envelope
  whose `data` is the 0.1.0 payload: `rivet list --json | jq .data.operations`.
- `rivet check --json` now prints an envelope; `rivet policy generate` **without** `--json` still prints the bare
  policy draft, so `rivet policy generate > policy.json` keeps working.
- `--pretty` indents any JSON output; it is refused with `--stream` (`validation.usage`, exit 2).

### HTTP (REST)

Before (0.1.0):

```text
POST /v1/request {"id":"demo.add","params":{"a":2,"b":3}}
200 {"request_id":"req_0189176ff5","trace_id":"tr_0189176ff5","result":5,"data_count":0,"effects":"none"}
```

After (0.2.0):

```text
$ curl -s -i http://127.0.0.1:18951/v1/request -H 'content-type: application/json' \
       -d '{"operation":"demo.add","data":{"a":2,"b":3}}'
HTTP/1.1 200 OK
content-type: application/json
traceparent: 00-acd00d769d2de71a298c0b65c22ea92b-e0559a5da1d8cfd2-01

{"request_id":"req_01b1f0b3fd","trace_id":"tr_01b1f0b3fd","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}

$ curl -s -i http://127.0.0.1:18951/v1/request -H 'content-type: application/json' \
       -d '{"id":"demo.add","params":{"a":2,"b":3}}'
HTTP/1.1 200 OK
content-type: application/json
traceparent: 00-df82d9208ed581fe04605cf691504bdb-05c8d9c6114d8b36-01
deprecation: true

{"request_id":"req_03adbe3067","trace_id":"tr_03adbe3067","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}

$ curl -s -i http://127.0.0.1:18951/v1/request -H 'content-type: application/json' \
       -d '{"operation":"demo.add","id":"demo.add"}'
HTTP/1.1 422 Unprocessable Entity

{"request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.input_envelope","message":"use `operation` or the deprecated `id`, not both","retryable":false,"details":{"key":"operation","alias":"id"}},"effects":"none","data_count":0}
```

- Status codes are unchanged (the registry decides them). A JSON body that is not an object is now
  `422 validation.input_envelope` (0.1.0: `validation.params`).
- `GET /v1/operations`, `/v1/operations/{id}`, `/v1/operations/{id}/outputs`, `/v1/io`, `/v1/health` and
  `POST /v1/policy/generate` wrap their 0.1.0 payload in `data`.
- `?pretty=true` indents any JSON answer.

### SSE

Before (0.1.0): `event: data | result | error`; the terminal data carried `result` or `error`.

After (0.2.0): `event: data` for items and `event: result` for the one terminal record; failures are
`event: result` with `"status":"error"`:

```text
id: 3
event: data
data: {"request_id":"req_042cd2bd4c","trace_id":"tr_042cd2bd4c","operation":"demo.countdown","type":"data","seq":3,"data":1,"error":null}

id: 4
event: result
data: {"request_id":"req_042cd2bd4c","trace_id":"tr_042cd2bd4c","operation":"demo.countdown","type":"result","seq":4,"status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}
```

An error **before** the stream starts is still a plain JSON envelope with its HTTP status (e.g. 404). `?pretty=true`
with `Accept: text/event-stream` is `400 validation.pretty_stream`.

### Polling

Before (0.1.0): `POST /v1/requests` → `202` with a bare SessionReceipt.

After (0.2.0): `202` with an **accepted envelope**; the receipt moved to `data`:

```text
{"request_id":"req_1165fcfbc7","trace_id":"tr_1165fcfbc7","operation":"demo.countdown","type":"result","status":"accepted","data":{"session_id":"ses_016285bebd","request_id":"req_1165fcfbc7","trace_id":"tr_1165fcfbc7","catalog_version":"sha256:e557…c2bc","input_schema":null,"emits_schema":{"type":"integer"},"next_send_seq":1,"expires_at":"2026-09-28T20:38:09Z","events_url":"/v1/requests/ses_016285bebd/events"},"error":null,"effects":"none","data_count":0}
```

`GET …/events` keeps its batch body `{session_id, events, last_seq, terminal}`; each event is now a record
(`type: data` … `type: result`). `…/input`, `…/finish_input` and `…/cancel` keep their ack bodies; their errors
are envelopes (`operation` = `rivet.sessions.send`, `rivet.sessions.read`, …).

### WebSocket (`rivet.v1`)

Before (0.1.0, captured for API-2026-0002 revision 2):

```text
-> {"type":"request","ref":"c2","id":"demo.countdown","params":{}}
<- {"type":"data","ref":"c2","request_id":"req_02712b8232","trace_id":"tr_02712b8232","seq":1,"data":3}
<- {"type":"result","ref":"c2","completion":{"request_id":"req_02712b8232","trace_id":"tr_02712b8232","result":{"count":3},"data_count":3,"effects":"none"}}
<- {"type":"error","ref":"c3","error":{"kind":"conflict","code":"conflict.ref","message":"ref `c3` is already in flight","retryable":false,"effects":"none"}}
```

The `completion` wrapper and the `type: "error"` frame are gone; every server frame is a record with `ref`
first.

After (0.2.0):

```text
-> {"type":"request","ref":"c1","operation":"demo.countdown","data":{}}
<- {"ref":"c1","request_id":"req_13f45ac261","trace_id":"tr_13f45ac261","operation":"demo.countdown","type":"data","seq":1,"data":3,"error":null}
<- {"ref":"c1","request_id":"req_13f45ac261","trace_id":"tr_13f45ac261","operation":"demo.countdown","type":"result","seq":4,"status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}
-> {"type":"request","ref":"c3","id":"demo.add","params":{"a":1,"b":2}}                   (deprecated, accepted)
<- {"ref":"c3","request_id":"req_147487fe06","trace_id":"tr_147487fe06","operation":"demo.add","type":"result","seq":1,"status":"ok","data":3,"error":null,"effects":"none","data_count":0}
```

`input`, `finish_input` and `cancel` frames are unchanged (`input` already used `data`).

### MCP

Before (0.1.0): `structuredContent` was the bare completion or error envelope, and streaming tools returned
a bare SessionReceipt:

```text
"structuredContent":{"request_id":"req_10052512ea","trace_id":"tr_10052512ea","result":5,"data_count":0,"effects":"none"}
```

`rivet.request` took `{id, params}`.

After (0.2.0): `structuredContent` (and the text content) is the envelope; a streaming tool answers an
`accepted` envelope whose `data` is the receipt; `isError` is `true` for `status` `error` **and** `cancelled`;
every tool's `outputSchema` describes the envelope:

```text
{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"{\"request_id\":\"req_03c07007e7\",\"trace_id\":\"tr_03c07007e7\",\"operation\":\"demo.add\",\"type\":\"result\",\"status\":\"ok\",\"data\":5,\"error\":null,\"effects\":\"none\",\"data_count\":0}"}],"structuredContent":{"request_id":"req_03c07007e7","trace_id":"tr_03c07007e7","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0},"isError":false}}
```

`rivet.request` takes `{"operation":"demo.add","data":{…}}`; `{id, params}` still works and the HTTP answer carries
`deprecation: true`. JSON-RPC framing, `initialize` and `tools/list` are unchanged apart from the new
`outputSchema`.

### Rust library

Before (0.1.0):

```toml
[dependencies]
rivet = { path = "/path/to/rivet" }
```

```rust
use rivet::Runtime;
use rivet::domain::policy::Policy;
use rivet::domain::{RivetError, RivetResult, Value};
use rivet::domain::contracts::{Completion, DataEvent, Envelope};
use rivet::domain::ports::DataSink;
use rivet::domain::io_manifest::IoQuery;
```

After (0.2.0):

```toml
[dependencies]
rivet = { package = "rivet-runtime", git = "https://github.com/olivierdevelops/rivet", tag = "v0.2.1" }
# lean: default-features = false, features = ["serve"]   (see MAN-2026-0007)
```

```rust
use rivet::{Runtime, Policy, Value, Completion, DataEvent, Envelope, DataSink};
use rivet::{InputEnvelope, ResponseEnvelope, Module};
use rivet::types::IoQuery;
fn f() -> rivet::Result<()> { Ok(()) }            // rivet::Error = the old RivetError
```

| 0.1.0 path | 0.2.0 path |
|---|---|
| package `rivet` | package `rivet-runtime` (library name still `rivet`, so `use rivet::…` stays) |
| `rivet::domain::Value` | `rivet::Value` |
| `rivet::domain::RivetError` / `RivetResult<T>` | `rivet::Error` / `rivet::Result<T>` |
| `rivet::domain::ErrorKind` | `rivet::ErrorKind` |
| `rivet::domain::policy::Policy` | `rivet::Policy` |
| `rivet::domain::contracts::{Completion, DataEvent, Envelope}` | `rivet::{Completion, DataEvent, Envelope}` |
| `rivet::domain::ports::DataSink` | `rivet::DataSink` |
| `rivet::domain::io_manifest::{IoQuery, IoReport, PolicyDraft}` | `rivet::types::{IoQuery, IoReport, PolicyDraft}` |
| `rivet::domain::contracts::{Catalog, OutputReport, Principal, RegistryEntry}` | `rivet::types::…` |
| `rivet::domain::sessions::SessionLimits`, `rivet::domain::source::SourceSpan` | `rivet::types::…` |
| `rivet::{domain, features, io, infra, orchestrator}::…` (anything else) | `rivet::internal::…` — hidden, not a stable API |
| `rivet::orchestrator::setup_serve::{ServeOptions, start}` | `rivet::serve::{ServeOptions, start}` (feature `serve`); session types, `Request`, `TraceQuery` → `rivet::types::…`; `TraceResult` → `rivet::TraceResult` |
| `rt.bundle()` → `&SourceBundle` | `rt.bundle()` → `SourceBundle` (owned; gains `modules`) |
| — | `rt.call(InputEnvelope) -> ResponseEnvelope`, `rt.call_json(&str)`, `rt.load` / `load_as` → `Module`, `Runtime::builder().root(dir)`, `rivet::highlight`, `rivet::build_features()`, `rivet::VERSION`, `rivet::ABI_VERSION` |

`Runtime::request(id, Value, sink) -> Result<Completion>` is unchanged; `ResponseEnvelope::from(completion)`
converts. `rivet.capabilities` data gains `build_features` and `abi_version`.

### Installing the binary

```sh
# 0.1.0
cargo install --path .
# 0.2.0 (the binary needs the `cli` feature)
cargo install --path . --features cli
cargo install rivet-runtime --git https://github.com/olivierdevelops/rivet --tag v0.2.1 --features cli
```

## Migration or Rollout

### Deprecation timeline

```text
 2026-09  0.2.0  ── outputs: envelopes only
                 ── inputs: {operation,data} canonical; {id,params}, --params accepted
                    signals: HTTP `deprecation: true`, access log "deprecated":1,
                             stderr warning[deprecated.params] / warning[deprecated.input]
          0.2.x  ── same (no new breaking change)
          0.3.0  ── {id,params} and --params refused: validation.input_envelope with a hint
```

### Client checklist

- [ ] Parse `status`; treat `ok` as success, `error`/`cancelled` as failure, `accepted` as "session opened".
- [ ] Read the value from `data` (not `result`); read `effects` at the top level.
- [ ] Streams: items are `type: "data"`; stop at the one `type: "result"` record; stop switching on SSE `event: error`.
- [ ] Built-ins over HTTP and `--json` commands: read the payload from `data`.
- [ ] MCP: read `structuredContent` (or parse `content[0].text`); `isError` also covers `cancelled`.
- [ ] Send `{operation, data}`; replace `--params` with `--data`, or use `--input FILE|-`.
- [ ] Expect `validation.input_envelope` (422 / exit 2) for mixed keys or a non-object body.
- [ ] CLI scripts: read errors from **stderr** (success envelopes are on stdout).
- [ ] Rust: rename the dependency to `rivet-runtime`; replace `rivet::domain::…` with facade paths.
- [ ] Build/install the binary with `--features cli`.
- [ ] Watch `deprecated` in the access log and `warning[deprecated.*]` on stderr until both are gone.

## Validation

- Schemas: every answer validates against [response.schema.json](../api/schemas/response.schema.json) or
  [stream-record.schema.json](../api/schemas/stream-record.schema.json); `tests/conformance_envelope.rs` checks
  this on the CLI, HTTP, SSE, NDJSON, polling, WebSocket, MCP and the library.
- A 0.1.0 input body run against 0.2.0 must succeed and carry `deprecation: true`.

## Open Questions

None. Removal of the aliases in 0.3.0 is fixed by R5.

## Decision or Outcome

Rollback plan: **pin v0.1.0.**

```text
 problem after upgrade ──▶ pin the previous version ──▶ restore 0.1.0 parsers ──▶ re-plan the migration
```

```sh
# CLI binary
cargo install --git https://github.com/olivierdevelops/rivet --tag v0.1.0 --force      # 0.1.0 needs no --features
# Rust dependency
rivet = { git = "https://github.com/olivierdevelops/rivet", tag = "v0.1.0" }
# From a checkout
git checkout v0.1.0 && cargo build --release
```

A 0.1.0 server rejects nothing a 0.2.0 client sends as long as the client still uses `{id, params}`; a client
that already sends `{operation, data}` must be rolled back with the server (0.1.0 answers `validation.required`
for a missing `id`). No data or state migration is involved: policies, `.rivet` files (without `global` or
`import`) and trace files are unchanged.

## Related Documents

- [API-2026-0006](../api/api-2026-0006-envelopes.md) — the envelope reference
- [API-2026-0001](../api/api-2026-0001-http-rest-sse-polling.md), [API-2026-0002](../api/api-2026-0002-websocket-rivet-v1.md),
  [API-2026-0003](../api/api-2026-0003-mcp-server-tools.md), [API-2026-0004](../api/api-2026-0004-rust-library.md)
- [MAN-2026-0004](../manuals/man-2026-0004-cli-reference.md) — CLI flags; [MAN-2026-0007](../manuals/man-2026-0007-embedding-library.md) — Cargo dependency
- [PROP-2026-0002](../proposals/implemented/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md) R1–R6, R10–R12;
  [ADR-0005](../decisions/adr-0005-workspace-package-and-features.md)
- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) D-09

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 3 | 2026-09-30 | Claude | v0.2.1 patch (PLAN-2026-0002 TASK-097): version strings, install tag v0.2.1; INC-2026-0013 behaviour where described. |
| 1 | 2026-09-29 | Claude | Initial migration guide (TASK-074, D-09): before/after per surface, jq mapping, timeline, checklist, rollback |
| 2 | 2026-09-29 | Claude | INC-2026-0012: serving, session and trace types are in the facade (`rivet::serve`, `rivet::types`, `rivet::TraceResult`). |
