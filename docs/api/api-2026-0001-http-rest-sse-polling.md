---
document_id: API-2026-0001
title: "Rivet HTTP API: REST, SSE and polling"
document_type: api
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [serve, http, poll, sessions, auth, audit, policy, registry]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, server]
audience: [developers, integrators, operators]
scope: Every HTTP route mounted by `rivet serve` except /v1/ws (API-2026-0002) and /mcp (API-2026-0003) — REST, SSE framing and the polling session routes, with request/response JSON, auth, status codes and captured examples.
reason: DOCUMENTATION.md §31 API impact for PLAN-2026-0001 row D-24; the HTTP surface is new in 0.1.0 and needs a current-state contract verified against the build.
related_documents: [PLAN-2026-0001, PROP-2026-0001, API-2026-0002, API-2026-0003, API-2026-0005, ARCH-2026-0001, SEC-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, api, http, rest, sse, polling, serve]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.1.0-dev (commit f40d4aa)"
---

# Rivet HTTP API: REST, SSE and polling

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** serve, http, poll, sessions, auth, audit, policy, registry

## Summary

One `rivet serve` process binds **one** listener and mounts every enabled surface on it. This document is the contract for the plain-HTTP routes: REST (`/v1/request`, `/v1/operations…`, `/v1/io`, `/v1/policy/generate`), Server-Sent Events (the same `/v1/request` with `Accept: text/event-stream`) and polling sessions (`/v1/requests…`). WebSocket and MCP on the same listener are in [API-2026-0002](api-2026-0002-websocket-rivet-v1.md) and [API-2026-0003](api-2026-0003-mcp-server-tools.md).

```text
   rivet --file app.rivet serve --listen 127.0.0.1:8080
                              │  one TCP listener (axum, HTTP/1.1 + HTTP/2)
   ┌──────────────┬───────────┴──┬──────────────────┬───────────────┬──────────┐
   │ http (REST)  │ sse          │ poll             │ ws            │ mcp      │
   │ /v1/request  │ /v1/request  │ /v1/requests     │ /v1/ws        │ /mcp     │
   │ /v1/operations│ Accept:     │ /v1/requests/{id}│ API-2026-0002 │ API-2026-│
   │ /v1/io       │ text/event-  │   /events /input │               │ 0003     │
   │ /v1/policy/  │ stream       │   /finish_input  │               │          │
   │   generate   │              │   /cancel        │               │          │
   └──────┬───────┴──────┬───────┴────────┬─────────┴───────┬───────┴────┬─────┘
          └──────────────┴── authenticate_principal ─▶ authorize_operation ┘
                                  │
                          Runtime::dispatch_request  (the same dispatcher as CLI + library)
```

Every example below was captured from a live `rivet serve` built from commit `f40d4aa` running [docs/demos/01-catalog](../demos/01-catalog/app.rivet) on `127.0.0.1:18431` (no policy.json, so auth `none`). Request, trace and session IDs, `expires_at` and `date` headers differ on every run.

## Audience and Stability

- **Audience:** integrators calling Rivet over HTTP, operators exposing `rivet serve`.
- **Stability:** the `/v1` prefix is the 0.1.0 contract. Fields may be *added* to response objects within `/v1`; existing fields are not renamed or removed without a new prefix.
- **Surface names** used by `policy.json` `serve.surfaces`: `http`, `sse`, `poll`, `ws`, `mcp` (default: all five).

## Authentication

Authentication is configured only in `policy.json` → `serve.auth`. It is applied identically on every route of every surface.

| `serve.auth.type` | Behaviour in 0.1.0 | Principal |
|---|---|---|
| absent / `none` | Allowed **only** on a loopback bind. A non-loopback `--listen` refuses to start: `serve.auth_required`, exit 2. | `local` (authenticated_by `none`) — may call everything |
| `bearer` | `Authorization: Bearer TOKEN` on every request. The token's SHA-256 (hex) is compared with `serve.auth.tokens[].sha256`. | the matching `principal` name |
| `mtls` | Refused at start-up: `unsupported.serve_mtls`, exit 5 (no TLS listener in this build). | — |

```text
  request ──▶ serve.auth none?  ── yes ─▶ bind is loopback? ── yes ─▶ principal "local"
                 │ no                          └ no (cannot happen: start refused, exit 2)
                 ▼
           bearer: header present? ── no ─▶ 401 auth.required  + WWW-Authenticate: Bearer
                 │ yes
           sha256(token) listed?   ── no ─▶ 401 auth.invalid   + WWW-Authenticate: Bearer
                 │ yes
           principal = tokens[i].principal ─▶ authorize_operation(principal, id)
                                                   └ not listed ─▶ 403 permission.denied
```

**Operation authorization** (`serve.principals`). With no `serve.principals` map, every authenticated principal may call every public operation *except* the sensitive built-ins. With the map, an entry must match the operation ID exactly, by `*`, or by a trailing `prefix.*`. The four sensitive built-ins — `rivet.io`, `rivet.policy.generate`, `rivet.trace.show`, `rivet.connectors.sync` — match **only an exact entry** for a network principal (never `*` or `demo.*`); the `local` principal may call them. Unlisted IDs are hidden from catalog routes (they answer 404 as if unknown) and refused on `/v1/request` (403).

Captured with [policies/team.json](../demos/01-catalog/policies/team.json) (`ada` → `demo.*`, `ci` → `demo.health`; fixture tokens `dev-token-ada`, `dev-token-ci`) on `127.0.0.1:18433`:

```text
$ curl -s -i http://127.0.0.1:18433/v1/operations
HTTP/1.1 401 Unauthorized
content-type: application/json
www-authenticate: Bearer

{"request_id":"","trace_id":"","error":{"kind":"auth","code":"auth.required","message":"missing bearer token","retryable":false,"effects":"none"}}

$ curl -s -i http://127.0.0.1:18433/v1/operations -H 'Authorization: Bearer wrong'
HTTP/1.1 401 Unauthorized
www-authenticate: Bearer

{"request_id":"","trace_id":"","error":{"kind":"auth","code":"auth.invalid","message":"invalid bearer token","retryable":false,"effects":"none"}}

$ curl -s http://127.0.0.1:18433/v1/operations -H 'Authorization: Bearer dev-token-ci'
{"operations":[{"id":"demo.health","name":"Check availability","description":"Return a constant readiness response without I/O.","streaming":false}],"next_cursor":null}

$ curl -s -i -X POST http://127.0.0.1:18433/v1/request -H 'Authorization: Bearer dev-token-ci' \
       -d '{"id":"demo.add","params":{"a":1}}'
HTTP/1.1 403 Forbidden

{"request_id":"req_010f333a5d","trace_id":"tr_010f333a5d","error":{"kind":"permission","code":"permission.denied","message":"principal `ci` may not call `demo.add`","retryable":false,"effects":"none"}}

$ curl -s -i http://127.0.0.1:18433/v1/io -H 'Authorization: Bearer dev-token-ada'     # demo.* does not reach rivet.io
HTTP/1.1 403 Forbidden

{"request_id":"req_028d671a4a","trace_id":"tr_028d671a4a","error":{"kind":"permission","code":"permission.denied","message":"principal `ada` may not call `rivet.io`","retryable":false,"effects":"none"}}
```

A surface disabled in `serve.surfaces` is not mounted: its routes answer `404 not_found.route` (team.json omits `ws`, so `GET /v1/ws` → 404).

## Endpoints or Events

| Method | Path | Surface | Purpose | Success |
|---|---|---|---|---|
| POST | `/v1/request` | `http` | Invoke one operation or built-in; JSON Completion | 200 |
| POST | `/v1/request` + `Accept: text/event-stream` | `sse` | Same, streamed as SSE events | 200 `text/event-stream` |
| GET | `/v1/operations` | `http` | Catalog summaries visible to the principal | 200 |
| GET | `/v1/operations/{id}` | `http` | Full descriptor (input schema, output, emits, receives, errors, source) | 200 |
| GET | `/v1/operations/{id}/outputs` | `http` | Declared output / emits / receives / errors JSON Schema | 200 |
| GET | `/v1/io?…` | `http` | I/O manifest (the `rivet.io` built-in) | 200 |
| POST | `/v1/policy/generate` | `http` | Least-privilege policy draft (the `rivet.policy.generate` built-in); never writes a file | 200 |
| POST | `/v1/requests` | `poll` | Open a principal-owned session | **202** |
| GET | `/v1/requests/{session_id}/events` | `poll` | Long-poll a batch of session events | 200 |
| POST | `/v1/requests/{session_id}/input` | `poll` | Send one input item (duplex operations) | 200 |
| POST | `/v1/requests/{session_id}/finish_input` | `poll` | Half-close input | 200 |
| POST | `/v1/requests/{session_id}/cancel` | `poll` | Cancel the session | 200 |
| any | anything else | — | — | 404 `not_found.route` |

When `http` is disabled but `sse` is enabled, only `POST /v1/request` is mounted (and it answers only SSE requests).

### Built-ins reachable through `POST /v1/request`

Every reserved `rivet.*` ID is dispatched through the same `/v1/request` route (and through MCP, WebSocket and the library):

| ID | Params | Authorization for a network principal |
|---|---|---|
| `rivet.request` | `{id, params}` | the named `id` is authorized |
| `rivet.list` / `rivet.describe` / `rivet.outputs` | `{}` / `{id}` / `{id?, all?}` | filtered by the principal's listing |
| `rivet.sessions.open` / `send` / `finish_input` / `read` / `cancel` | see [Polling sessions](#polling-sessions) | own sessions only |
| `rivet.auth.begin` / `complete` / `status` / `disconnect` / `cancel` | profile/account parameters | listed (or no `serve.principals` map) |
| `rivet.io` | as `/v1/io` query | **exact** listing |
| `rivet.policy.generate` | `{ids?, all?}` | **exact** listing |
| `rivet.trace.show` | `{request_id}` | **exact** listing |
| `rivet.connectors.sync` | `{name, output}` | **exact** listing |

## Request Format

### `POST /v1/request`

```json
{ "id": "demo.add", "params": { "a": 2, "b": 3 }, "deadline_ms": 5000 }
```

| Field | Type | Required | Notes |
|---|---|---|---|
| `id` | string | yes | Operation ID or `rivet.*` built-in. Empty/missing → 422 `validation.required`. |
| `params` | object | no | Checked against the operation's input schema: unknown fields, missing required, wrong types, min/max/enum. Defaults are injected after validation. |
| `deadline_ms` | integer | no | Request deadline; clamped to 1…600000 (10 minutes). Default deadline is 30000 ms. |

- An empty or whitespace body is treated as `{}`.
- A body that is not JSON → **400** `validation.malformed_json` (the only 400 in the API).
- A JSON body that is not an object → 422 `validation.params`.
- A **streaming** operation (one that declares `emits`) requested without `Accept: text/event-stream` → 422 `stream.required`.

### `GET /v1/io` query

Query keys map onto the `rivet.io` parameters: `by=operation|target|capability`, `kind=K`, `access=V,V`, `check_policy=true`, `needs=true`, `strict=true`, `include_bootstrap=true`, `ids=ID,ID`, `all=true`, `trace=REQ`, `format=json|table|markdown|csv`. The literal values `true` / `false` become booleans; everything else is text. `check_files` is refused remotely (`validation.check_files_remote`).

### `POST /v1/policy/generate`

`{"ids": ["demo.add"]}` or `{"all": true}`; empty body = every public operation. Non-JSON body → 422 `validation.body`.

### Polling bodies

| Route | Body / query |
|---|---|
| `POST /v1/requests` | `{id, params}` (same parser as `/v1/request`; `deadline_ms` is ignored) |
| `GET …/events` | `after_seq=N` (default 0), `wait_ms=M` (default 1000, max 5000), `max_events=K` (default 16, max 16, min 1). Non-integer → 422 `validation.type`. Unknown keys ignored. |
| `POST …/input` | `{"send_seq": 1, "data": <any JSON>}`; `send_seq` required |
| `POST …/finish_input`, `POST …/cancel` | no body |

## Response Format

### Completion (200 from `/v1/request`)

```json
{"request_id":"req_0189176ff5","trace_id":"tr_0189176ff5","result":5,"data_count":0,"effects":"none"}
```

`result` already passed the operation's declared `output` check (a mismatch is `output.invalid`, 500). `effects` ∈ `none | committed | partial | unknown`. `data_count` counts emitted items.

### Catalog

`GET /v1/operations`:

```json
{"operations":[{"id":"demo.greet","name":"Greet a person","description":"Return a greeting for the supplied person.","streaming":false},
               {"id":"demo.add","name":"Add two integers","description":"Add two signed integers and return their sum.","streaming":false},
               {"id":"demo.health","name":"Check availability","description":"Return a constant readiness response without I/O.","streaming":false},
               {"id":"demo.countdown","name":"Count down","description":"Emit 3, 2, 1 as data items and then return a summary.","streaming":true}],
 "next_cursor":null}
```

`GET /v1/operations/demo.add`:

```json
{"id":"demo.add","name":"Add two integers","description":"Add two signed integers and return their sum.","kind":"operation",
 "input":{"type":"object","properties":{"a":{"type":"integer","description":"First operand."},
          "b":{"type":"integer","description":"Second operand; defaults to zero.","default":0}},
          "required":["a"],"additionalProperties":false},
 "output":{"type":"integer","description":"Sum of a and b."},
 "emits":null,"receives":null,"errors":[],"delivery":"unary","source":{"file":"app.rivet","line":9}}
```

`GET /v1/operations/demo.countdown/outputs`:

```json
{"id":"demo.countdown","output":{"type":"object","properties":{"count":{"type":"integer","description":"Number of items emitted."}},
 "required":["count"],"additionalProperties":false,"description":"Summary returned after the last item."},
 "emits":{"type":"integer"},"receives":null,"errors":[]}
```

### I/O manifest and policy draft

`GET /v1/io?format=json` returns the IoReport wrapper — `format`, `by`, `rendered` (the text the CLI would print), `diagnostics`, `exit_code` and the structured `manifest`:

```json
{"format":"json","by":"operation","rendered":"{\n  \"bundle\": {…}\n}\n","diagnostics":"","exit_code":0,
 "manifest":{"bundle":{"file":"app.rivet","sha256":"67104f0e…9730"},"policy":null,"complete":true,
             "sites":[],"targets":[],"needs":[],"bootstrap":[]}}
```

`POST /v1/policy/generate {"all":true}`:

```json
{"policy":{"version":1,"grants":[],"network":{"deny_private_ranges":true}},"review":[],"complete":true}
```

### SSE framing

`POST /v1/request` with `Accept: text/event-stream` (any list entry, parameters ignored) answers `200`, `content-type: text/event-stream`, `cache-control: no-cache`. Each event is:

```text
id: <seq>
event: data | result | error
data: <one-line JSON envelope carrying "seq">
<blank line>
```

```text
  client                        rivet serve
    │ POST /v1/request           │
    │ Accept: text/event-stream  │
    │───────────────────────────▶│ validate + authorize
    │                            │   ├─ error BEFORE the first item ─▶ plain JSON ErrorEnvelope
    │                            │   │                                  with its registry status
    │◀── 200 text/event-stream ──│   └─ ok
    │◀── id:1 event:data ────────│  seq 1..n  one per `emit`
    │◀── id:n+1 event:result ────│  exactly ONE terminal event (result | error), then EOF
    │  (disconnect) ────────────▶│  request aborted: scope closed, handles released
```

Captured `demo.countdown`:

```text
$ curl -s -i -N -X POST http://127.0.0.1:18431/v1/request -H 'accept: text/event-stream' \
       -d '{"id":"demo.countdown","params":{}}'
HTTP/1.1 200 OK
content-type: text/event-stream
cache-control: no-cache
transfer-encoding: chunked

id: 1
event: data
data: {"request_id":"req_0383c7b4ff","trace_id":"tr_0383c7b4ff","seq":1,"type":"data","data":3}

id: 2
event: data
data: {"request_id":"req_0383c7b4ff","trace_id":"tr_0383c7b4ff","seq":2,"type":"data","data":2}

id: 3
event: data
data: {"request_id":"req_0383c7b4ff","trace_id":"tr_0383c7b4ff","seq":3,"type":"data","data":1}

id: 4
event: result
data: {"request_id":"req_0383c7b4ff","trace_id":"tr_0383c7b4ff","result":{"count":3},"data_count":3,"effects":"none","type":"result","seq":4}
```

A unary operation over SSE yields one `result` event with `seq: 1`. A failure after the stream started is an `event: error` whose data is `{request_id, trace_id, seq, type:"error", error:{…}}`. `Last-Event-ID` resumption is not supported; use polling sessions to resume.

### Polling sessions

Polling is the HTTP projection of `rivet.sessions.*`. Sessions are **principal-owned** (they survive the HTTP connection) and bounded.

```text
            POST /v1/requests {id, params}
                     │ 202 SessionReceipt (events_url)
                     ▼
   ┌──────────── open ─────────────┐   GET …/events?after_seq=N  (acks ≤ N, waits ≤ 5 s)
   │  event log ≤ 16 events        │◀──────────────────────────────────────────────┐
   │  producer blocks when full    │── 200 SessionBatch {events, last_seq, terminal}┘
   │  input: send_seq 1,2,3 …      │◀── POST …/input {send_seq, data}   200 SessionAck
   │                               │◀── POST …/finish_input             200 input_closed:true
   └──────┬───────────────┬────────┘
          │ result|error  │ POST …/cancel ─▶ 200 CancelReceipt
          ▼               ▼
      terminal:true   terminal (cancelled)  ── retained 60 s, then 404 not_found.session
```

| Limit | Value |
|---|---|
| Open sessions per principal | 8 (`limit.sessions`, 429) |
| Retained events per session | 16 frames / 32 MiB |
| Idle timeout / retention after terminal | 60 s / 60 s |
| `wait_ms` default / max | 1000 / 5000 ms |
| `max_events` default / cap | 16 / 16 |

Input sequencing: `send_seq` starts at 1 and increases by one. An identical retry of the most recent sequence is acknowledged again without enqueuing; a gap → 409 `conflict.input_sequence`; input after `finish_input` → 409 `conflict.input_closed`. Another principal's, unknown or expired session → 404 `not_found.session`.

Captured (`demo.countdown`, then `chat.echo`, a scratch duplex operation that declares `receives text`/`emits text` and echoes each input):

```text
$ curl -s -i -X POST http://127.0.0.1:18431/v1/requests -d '{"id":"demo.countdown","params":{}}'
HTTP/1.1 202 Accepted

{"session_id":"ses_01db3ed345","request_id":"req_09ddeac775","trace_id":"tr_09ddeac775",
 "catalog_version":"sha256:67104f0e…9730","input_schema":null,"emits_schema":{"type":"integer"},
 "next_send_seq":1,"expires_at":"2026-09-28T05:13:33Z","events_url":"/v1/requests/ses_01db3ed345/events"}

$ curl -s 'http://127.0.0.1:18431/v1/requests/ses_01db3ed345/events?after_seq=0&wait_ms=2000&max_events=2'
{"session_id":"ses_01db3ed345","events":[
  {"request_id":"req_09ddeac775","trace_id":"tr_09ddeac775","seq":1,"type":"data","data":3},
  {"request_id":"req_09ddeac775","trace_id":"tr_09ddeac775","seq":2,"type":"data","data":2}],
 "last_seq":2,"terminal":false}

$ curl -s 'http://127.0.0.1:18431/v1/requests/ses_01db3ed345/events?after_seq=2&wait_ms=2000'
{"session_id":"ses_01db3ed345","events":[
  {"request_id":"req_09ddeac775","trace_id":"tr_09ddeac775","seq":3,"type":"data","data":1},
  {"request_id":"req_09ddeac775","trace_id":"tr_09ddeac775","result":{"count":3},"data_count":3,"effects":"none","type":"result","seq":4}],
 "last_seq":4,"terminal":true}

$ curl -s -X POST http://127.0.0.1:18431/v1/requests/ses_01db3ed345/cancel
{"session_id":"ses_01db3ed345","request_id":"req_09ddeac775","state":"cancelled"}

$ curl -s -i http://127.0.0.1:18431/v1/requests/ses_bogus/events
HTTP/1.1 404 Not Found

{"request_id":"","trace_id":"","error":{"kind":"not_found","code":"not_found.session","message":"no session `ses_bogus`","retryable":false,"effects":"none"}}
```

```text
# duplex: chat.echo (receives text, emits text) on 127.0.0.1:18432
POST /v1/requests {"id":"chat.echo","params":{}}         → 202 {…,"input_schema":{"type":"string"},"emits_schema":{"type":"string"},"next_send_seq":1,…}
POST …/input {"send_seq":1,"data":"hi"}                 → 200 {"session_id":"ses_1434f11686","accepted_seq":1,"input_closed":false}
POST …/input {"send_seq":1,"data":"hi"}   (retry)       → 200 {"session_id":"ses_1434f11686","accepted_seq":1,"input_closed":false}
POST …/input {"send_seq":3,"data":"x"}    (gap)         → 409 {"request_id":"","trace_id":"","error":{"kind":"conflict","code":"conflict.input_sequence","message":"expected send_seq 2, got 3",…}}
POST …/finish_input                                     → 200 {"session_id":"ses_1434f11686","accepted_seq":null,"input_closed":true}
POST …/input {"send_seq":2,"data":"late"}               → 409 {…"code":"conflict.input_closed","message":"input is finished; no further sends are accepted",…}
GET  …/events?after_seq=0&wait_ms=2000                  → 200 {"session_id":"ses_1434f11686","events":[
      {"request_id":"req_1434f11ace","trace_id":"tr_1434f11ace","seq":1,"type":"data","data":"hi"},
      {"request_id":"req_1434f11ace","trace_id":"tr_1434f11ace","result":{"echoed":1},"data_count":1,"effects":"none","type":"result","seq":2}],
      "last_seq":2,"terminal":true}
```

## Error Format

Every non-2xx JSON response is an **ErrorEnvelope**:

```json
{
  "request_id": "req_02062fbf4a",
  "trace_id": "tr_02062fbf4a",
  "error": {
    "kind": "validation",
    "code": "validation.type",
    "message": "parameter `a` must be an integer, got text",
    "retryable": false,
    "effects": "none",
    "operation_id": "demo.add",
    "details": {"field": "a"}
  }
}
```

`request_id` / `trace_id` are empty strings when the failure happened before a request existed (auth, routing, body parsing). Optional error fields: `source` (file/line/column span), `operation_id`, `node_id`, `hint`, `details`, `cause`, `suppressed`. The HTTP status comes from the error **kind** (full table in [API-2026-0005](api-2026-0005-error-registry.md)):

| Status | Kinds / codes |
|---|---|
| 400 | `validation.malformed_json` only |
| 401 | `auth` (adds `WWW-Authenticate: Bearer`) |
| 403 | `permission` |
| 404 | `not_found` (unknown or hidden operation, route, session) |
| 409 | `conflict`, `cancelled` |
| 422 | `syntax`, `validation` |
| 429 | `limit` (the only kind with `retryable: true`) |
| 500 | `output_invalid`, `internal`, `cleanup`, `consumer_failed` |
| 501 | `unsupported` |
| 502 | `connection`, `dns`, `tls`, `http`, `protocol`, `process`, `parse`, `application` |
| 504 | `timeout` |

Captured failures from `127.0.0.1:18431`:

```text
POST /v1/request {nope                               → 400 validation.malformed_json  "request body is not valid JSON: key must be a string at line 1 column 2"
POST /v1/request {"id":"demo.add","params":{"a":"x"}} → 422 validation.type
POST /v1/request {"id":"demo.countdown"}             → 422 stream.required  "`demo.countdown` streams; use Accept: text/event-stream, POST /v1/requests, /v1/ws or rivet.sessions.open"
POST /v1/request {"id":"demo.nope"}                  → 404 not_found.operation (request_id/trace_id set)
GET  /v1/operations/nope                             → 404 not_found.operation (request_id/trace_id empty)
GET  /v1/nothing                                     → 404 not_found.route "no such route on this listener"
```

## Rate Limits

There is no per-client rate limiter in 0.1.0. Bounded resources answer `429 limit.*` with `retryable: true`:

- `limits.max_concurrent_requests` in policy.json (default 64) → `limit.concurrency` for top-level requests beyond the budget;
- 8 open sessions per principal → `limit.sessions`;
- `limits.max_call_depth` (default 16) → `limit.call_depth` for nested `(request …)` chains.

Rivet sets no explicit HTTP body limit; the framework default of the axum server applies to request bodies.

## Versioning and Deprecation

The path prefix `/v1` versions the HTTP contract. A breaking change ships under a new prefix; `/v1` would then be marked deprecated here with a replacement link. Nothing is deprecated in 0.1.0.

## Examples

End to end with curl against a loopback server (no policy.json → auth `none`):

```text
$ cd docs/demos/01-catalog
$ rivet --file app.rivet serve --listen 127.0.0.1:18431 &
{"listen_addr":"127.0.0.1:18431","stdio":false,"surfaces":["http","sse","poll","ws","mcp"],"auth_type":"none","catalog_version":"sha256:67104f0e7faaeafee253db758a668a9b24aa4263677e9d5ea2451b32019f9730","policy_hash":null}

$ curl -s -X POST http://127.0.0.1:18431/v1/request -H 'content-type: application/json' \
       -d '{"id":"demo.add","params":{"a":2,"b":3}}'
{"request_id":"req_0189176ff5","trace_id":"tr_0189176ff5","result":5,"data_count":0,"effects":"none"}

$ curl -s -X POST http://127.0.0.1:18431/v1/request -d '{"id":"demo.add","params":{"a":1},"deadline_ms":900000}'
{"request_id":"req_08589907f8","trace_id":"tr_08589907f8","result":1,"data_count":0,"effects":"none"}     # deadline clamped to 600000

$ curl -s -X POST http://127.0.0.1:18431/v1/request -d '{"id":"rivet.list","params":{}}'
{"request_id":"req_07da2eeb53","trace_id":"tr_07da2eeb53","result":{"operations":[{"id":"demo.greet",…}],…},…}
```

The start-up receipt (first line) is written to stderr. Refused starts, captured from the same folder:

```text
$ rivet --file app.rivet serve --listen 0.0.0.0:18434
{"request_id":"","trace_id":"","error":{"kind":"validation","code":"serve.auth_required","message":"non-loopback listener requires serve.auth in policy.json","retryable":false,"effects":"none"}}
exit 2

$ rivet --file app.rivet --policy mtls.json serve --listen 127.0.0.1:18435     # serve.auth.type "mtls"
{"request_id":"","trace_id":"","error":{"kind":"unsupported","code":"unsupported.serve_mtls","message":"serve.auth type mtls needs a TLS listener, which this build does not provide yet; use bearer behind a TLS-terminating proxy","retryable":false,"effects":"none"}}
exit 5
```

The remote CLI (`rivet --endpoint URL [--token-file PATH] …`) is a client of these same routes: `request` → `POST /v1/request` (SSE with `--stream`), `list/describe/outputs` → `GET /v1/operations…`, `io` → `GET /v1/io`, `auth`/`trace show`/`connectors sync` → `POST /v1/request` with the built-in ID, and `--input-jsonl -` → `/v1/ws`.

## Compatibility Notes

- HTTP/1.1 and HTTP/2 (cleartext) on the listener; TLS termination is the job of a front proxy in 0.1.0.
- Request IDs look like `req_NN…` and trace IDs `tr_NN…`; treat both as opaque strings.
- W3C `traceparent` is neither read nor emitted in 0.1.0.
- Known limitations: no mTLS listener, no persistent trace store (traces live in the serving process), no `Last-Event-ID` resume on SSE, no pagination (`next_cursor` is always `null`).

## Related Documents

- [API index](index.md) · [WebSocket rivet.v1](api-2026-0002-websocket-rivet-v1.md) · [MCP server tools](api-2026-0003-mcp-server-tools.md) · [Error registry](api-2026-0005-error-registry.md)
- [Runtime architecture](../architecture/arch-2026-0001-rivet-runtime-architecture.md) · [Policy and sandbox model](../security/sec-2026-0001-policy-and-sandbox-model.md)
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) · [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md) · [Demo 01-catalog](../demos/01-catalog/README.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial current-state contract for REST, SSE and polling, with examples captured from `rivet serve` at commit f40d4aa. |
