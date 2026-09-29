---
document_id: API-2026-0002
title: "Rivet WebSocket API: subprotocol rivet.v1"
document_type: api
status: active
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 4
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [serve, ws, sessions]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, server]
audience: [developers, integrators]
scope: The `/v1/ws` route of `rivet serve` — upgrade, authentication, client and server frame types, refs, limits, sequencing and close semantics, with 0.2.0 frames (request frames carry an InputEnvelope; every server frame is an envelope record with `ref` first) captured from a live server.
reason: DOCUMENTATION.md §31 API impact for PLAN-2026-0001 row D-25 (0.1.0 contract) and PLAN-2026-0002 row D-41 (0.2.0 envelope sweep, TASK-070).
related_documents: [PLAN-2026-0001, PLAN-2026-0002, PROP-2026-0001, PROP-2026-0002, API-2026-0001, API-2026-0005, API-2026-0006, MIG-2026-0001, ARCH-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, api, websocket, sessions, duplex, envelope]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.2.0-rc (source at 6f9943f)"
next_review_date: 2026-10-29
---

# Rivet WebSocket API: subprotocol rivet.v1

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0 and later (frames as below from 0.2.0)
> **Owner:** Project maintainer
> **Affected Components:** serve, ws, sessions

## Summary

`GET /v1/ws` upgrades to a WebSocket that carries JSON text frames under the subprotocol **`rivet.v1`**. One socket multiplexes up to **8 in-flight requests**, each identified by a client-chosen `ref`. Every ref is a *connection-owned* session: unary, streaming (`emits`) and duplex (`receives`) operations all run the same way, and each ref ends with **exactly one** terminal record (`type: "result"`, `status` `ok`, `error` or `cancelled`). WebSocket is a projection only — there are no WebSocket-only operations.

From 0.2.0 a `request` frame carries an **InputEnvelope** (`operation`, `data`, optional `restrict`) and every server frame is an **envelope record** ([API-2026-0006](api-2026-0006-envelopes.md)) with `ref` as its first key. The 0.1.0 `completion` wrapper and the `type: "error"` frame are gone; [MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md#websocket-rivetv1) maps them.

```text
 client                                         rivet serve  /v1/ws
   │ GET /v1/ws                                     │
   │ Sec-WebSocket-Protocol: rivet.v1               │
   │ Authorization: Bearer …   (when bearer)        │── authenticate once (upgrade request)
   │◀───────────── 101 Switching Protocols ─────────│   sec-websocket-protocol: rivet.v1
   │                                                │
   │ {"type":"request","ref":"c1",                  │
   │  "operation":…,"data":{…},                     │── serve.parse_input (same parser as HTTP)
   │  "restrict"?:{grants:[…]}}  ──────────────────▶│── authorize operation for the principal
   │                                                │── open connection-owned session ── pump(c1)
   │                                                │      └─ per-ref lane (16 frames) ─┐
   │                                                │   lanes c1, c2 … merged into one ─┘ socket writer
   │◀── {"ref":"c1",…,"type":"data","seq":1,…} ─────│
   │ {"type":"input","ref":"c1","seq":1,"data":…} ─▶│   (duplex only)
   │ {"type":"finish_input","ref":"c1"} ──────────▶│
   │◀── {"ref":"c1",…,"type":"result","status":…} ──│   exactly one terminal record per ref
   │                                                │
   │ close ────────────────────────────────────────▶│── cancel + join every in-flight ref
```

Frames below were captured on 2026-09-29 (macOS 26.4) from the 0.2.0 release candidate (`cargo build --release --features cli`, source at `6f9943f`) serving a scratch bundle on `127.0.0.1:18950`: `demo.add` and `demo.countdown` (as in [docs/demos/01-catalog](../demos/01-catalog/app.rivet)), the duplex `chat.echo` (`receives text`, `emits text`, echoes every input item, returns `{echoed: N}`), the file reader `demo.read` (policy grants `allow_read ./data/**`) and `demo.flaky` (emits `1`, then fails reading a missing file). The client was a 150-line Python stdlib WebSocket client. **Request and trace IDs differ on every run.**

## Audience and Stability

Integrators that need many concurrent or bidirectional requests over one connection. The frame vocabulary below is stable for `rivet.v1` from 0.2.0; new optional keys may be added to the error object. The 0.2.0 change of server frames to envelope records kept the subprotocol name (the one exception approved in PROP-2026-0002, see [Versioning and Deprecation](#versioning-and-deprecation)); a future breaking change would use a new subprotocol name.

## Authentication

- Authentication runs **once**, on the HTTP upgrade request, with the same rules as every other route ([API-2026-0001 §Authentication](api-2026-0001-http-rest-sse-polling.md#authentication)): auth `none` on loopback → principal `local`; bearer → `Authorization: Bearer TOKEN`. Failure answers the upgrade with the JSON error envelope (401) instead of 101.
- A valid W3C `traceparent` header on the upgrade request becomes the trace of every ref opened on that socket.
- The client **must** offer `rivet.v1` in `Sec-WebSocket-Protocol`; otherwise the upgrade is answered `422 validation.subprotocol` (captured in [Examples](#examples)).
- **Each `request` frame** is authorized separately against `serve.principals` for the connection's principal (refused → the ref's terminal record with `permission.denied`).
- The `ws` surface must be enabled in `serve.surfaces`; otherwise `/v1/ws` is `404 not_found.route`.

## Endpoints or Events

| Direction | `type` | Keys | Meaning |
|---|---|---|---|
| client → server | `request` | `ref`, `operation`, `data?`, `deadline_ms?`, `restrict?` (deprecated: `id`, `params`) | Start an operation on a new ref; `deadline_ms` bounds this ref (default 30 s, capped at 600000); `restrict: {grants:[…]}` narrows this ref's authority (never widens) |
| client → server | `input` | `ref`, `seq`, `data` | One input item for a duplex ref (`seq` starts at 1) |
| client → server | `finish_input` | `ref` | Half-close the ref's input |
| client → server | `cancel` | `ref` | Cancel the ref |
| server → client | record `type: "data"` | `ref`, `request_id`, `trace_id`, `operation`, `type`, `seq`, `data`, `error: null` | One emitted item |
| server → client | record `type: "result"` | `ref`, `request_id`, `trace_id`, `operation`, `type`, `seq`, `status`, `data`, `error`, `effects`, `data_count` | Terminal record of a ref (`status` `ok`, `error` or `cancelled`; always with `seq` — every ref is a session, so a unary ref ends with `seq: 1`), **or** the refusal of a frame that opened no ref (`status: "error"`, no `seq`; `ref: ""` when the frame named a ref that is already in flight) |

```text
  per-ref state machine (server side)

   (none) ──request──▶ open ──data*──▶ open ──result (ok|error)──▶ ended (ref may be reused)
                        │  ▲ input seq=n (ok)                        ▲
                        │  └───────────┘                             │
                        ├── finish_input ─▶ input closed ────────────┤
                        ├── cancel ──────────────────────────────────┤  result status cancelled
                        ├── refused input / finish ──────────────────┤  result status error = the SPECIFIC
                        │     (sent first, then the session is       │  refusal (conflict.input_sequence,
                        │      cancelled; its own record dropped)    │  validation.input, conflict.input_closed)
                        └── socket closed ── cancel + join ──────────┘  (no record: socket gone)
```

## Request Format

Client frames are **JSON text** frames. Every frame needs a non-empty string `ref`. A `request` frame is parsed by `serve.parse_input`, the same InputEnvelope parser as `POST /v1/request`:

```json
{"type":"request","ref":"c1","operation":"demo.add","data":{"a":2,"b":3}}
{"type":"request","ref":"r1","operation":"demo.read","data":{"path":"data/a.txt"},"restrict":{"grants":[{"capability":"allow_read","targets":["./data/**"]}]}}
{"type":"input","ref":"c3","seq":1,"data":"hi"}
{"type":"finish_input","ref":"c3"}
{"type":"cancel","ref":"c3"}
```

Rules:

| Rule | Violation → error record (socket stays open) |
|---|---|
| Frame is JSON text | not JSON / binary → `validation.frame` with `ref: ""` |
| `ref` present | missing → `validation.frame`, `ref: ""` |
| `type` ∈ request, input, finish_input, cancel | else `validation.frame` for that ref |
| `request` names an operation (`operation`, or the deprecated `id`) | `validation.required` (`details.field: "operation"`) |
| `request` does not mix a key with its alias (`operation` + `id`, `data` + `params`) | `validation.input_envelope` (`details {key, alias}`) |
| `input` has integer `seq` | `validation.frame` |
| `request` ref not already in flight | `conflict.ref` (409 semantics), sent with `ref: ""` and `error.details.ref` so it never reads as that ref's terminal record |
| ≤ 8 refs in flight per connection | 9th → `limit.ws_refs` (retryable) |
| `input` / `finish_input` / `cancel` target an in-flight ref | `not_found.ref` |
| `input.seq` = next expected (1, 2, …); identical retry of the last seq is accepted | otherwise the ref ends with `conflict.input_sequence` (then its session is cancelled) |
| `input.data` matches the operation's `receives` | otherwise the ref ends with `validation.input` (`details {seq, path, expected, found}`) |
| no `input` after `finish_input` | otherwise the ref ends with `conflict.input_closed` |
| `restrict` (optional) is `{grants:[…]}` | other keys / malformed grants → `policy.invalid` (`details.pointer` under `/restrict`) for that ref |

The deprecated 0.1.0 request keys `id` and `params` are still accepted through 0.2.x. A socket has no per-frame
header and the record shape is fixed by the schema, so the signal is the request's trace note (`rivet.trace.show`:
phase `input`, decision `deprecated`, target `id,params`), as on HTTP and MCP
([API-2026-0006](api-2026-0006-envelopes.md#deprecation-signals-02x)).
From 0.2.0 a request frame's `deadline_ms` bounds that ref (0.1.0 frames had none); without it the ref runs under
the default request deadline (30 s):

```text
-> {"type":"request","ref":"t1","operation":"chat.echo","data":{},"deadline_ms":300}
<- {"ref":"t1","request_id":"req_37e5920d79","trace_id":"tr_37e5920d79","operation":"chat.echo","type":"result","seq":1,"status":"error","data":null,"error":{"kind":"timeout","code":"timeout.request","message":"`chat.echo` exceeded its 300 ms deadline","retryable":false,"source":{"file":"app.rivet","line":32,"column":5,"end_line":35,"end_column":8},"operation_id":"chat.echo"},"effects":"none","data_count":0}
```

## Response Format

Server frames are JSON text envelope records ([API-2026-0006](api-2026-0006-envelopes.md)) with `ref` first. A
data record never carries `status`; the terminal record always does:

```json
{"ref":"c2","request_id":"req_07cd5f6a43","trace_id":"tr_07cd5f6a43","operation":"demo.countdown","type":"data","seq":1,"data":3,"error":null}
{"ref":"c2","request_id":"req_07cd5f6a43","trace_id":"tr_07cd5f6a43","operation":"demo.countdown","type":"result","seq":4,"status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}
{"ref":"e2","request_id":"req_15fdf90b73","trace_id":"tr_15fdf90b73","operation":"chat.echo","type":"result","seq":1,"status":"cancelled","data":null,"error":{"kind":"cancelled","code":"cancelled.session","message":"the session was cancelled","retryable":false},"effects":"none","data_count":0}
```

```text
 reading a rivet.v1 socket
   frame ─▶ parse ─▶ ref ──▶ route to that ref's handler
                      │
                      ├─ type "data"   ─▶ item (seq 1, 2, …)
                      └─ type "result" ─▶ status ok        ─▶ done, data = result
                                          status error     ─▶ done, error.code / error.retryable
                                          status cancelled ─▶ done, data_count items arrived first
   ref "" (or a ref you never opened) with status error ─▶ a refused frame, not a terminal record
```

**Flow control.** Each ref has its own outbound **lane of 16 frames**; the lanes are merged into the one socket writer. A slow consumer of one ref blocks only that ref's producer (backpressure), never the other refs. Each ref's session also keeps a bounded event log (16 frames), and its retained bytes count against the host budget `limits.max_buffered_bytes` (`limit.buffered_bytes`). The per-ref pump reads with a 5 s bounded wait and forwards frames in `seq` order.

```text
  ref c1 session ─▶ lane c1 [16] ─┐
  ref c2 session ─▶ lane c2 [16] ─┼─▶ writer ─▶ socket        a full lane blocks only its own producer
  ref c3 session ─▶ lane c3 [16] ─┘
```

## Error Format

The `error` object is the same one every surface uses ([API-2026-0005](api-2026-0005-error-registry.md), [API-2026-0006](api-2026-0006-envelopes.md#error-format)): `kind`, `code`, `message`, `retryable`, plus optional `operation_id`, `details`, `hint`, `source`, `cause`, `suppressed`; `effects` sits at the top level of the record only (never inside `cause` or `suppressed[]`). A refused frame never closes the socket; it gets an error record for its `ref` (or `""` when no ref could be read, or when the frame named a ref that is already in flight — then `error.details.ref` names it). Frame-level refusals for a ref that was never opened (`validation.frame`, `not_found.ref`, `conflict.ref`, `limit.ws_refs`, a bad envelope) have no `seq` and do **not** count as that ref's terminal record; they have empty `request_id`/`trace_id` unless the frame became a request (a request refused at open — `validation.required` on its params, `not_found.operation`, `permission.denied` — carries its request and trace IDs, like REST). A refused `input` or `finish_input` on an open ref **is** that ref's terminal record: it carries the specific code, the next `seq` and the real `data_count`.

## Rate Limits

| Limit | Value | Code |
|---|---|---|
| Refs in flight per connection | 8 | `limit.ws_refs` |
| Sessions per principal (shared with polling / MCP / `rivet.sessions.*`) | 8 | `limit.sessions` |
| Outbound lane per ref | 16 frames | backpressure (per ref) |
| Event log per ref | 16 frames / 32 MiB | backpressure |
| Bytes retained by all sessions of the host | `limits.max_buffered_bytes` (default 256 MiB) | `limit.buffered_bytes` |
| Deadline per ref | `deadline_ms` of the request frame, else the default request deadline (30 s); capped at 600000 ms | `timeout.request` |
| Top-level concurrent requests | `limits.max_concurrent_requests` (default 64) | `limit.concurrency` |

## Versioning and Deprecation

The subprotocol string is the version; `rivet.v1` is the only protocol. In 0.2.0 its server frames became envelope
records and request frames gained `operation`/`data`, under the same name (the project had no external clients;
PROP-2026-0002 R2, R4):

```text
 0.1.x  -> {"type":"request","ref","id","params"}      <- {"type":"data",…} · {"type":"result","ref","completion":{…}} · {"type":"error","ref","error"}
 0.2.0  -> {"type":"request","ref","operation","data"}  <- {"ref",…,"type":"data","seq",…} · {"ref",…,"type":"result","status",…}
        id/params still accepted (no per-frame signal on a socket)
 0.3.0  id/params refused (validation.input_envelope)
```

## Examples

Captured session 1 — unary, streaming, duplex, duplicate ref, refused inputs, `restrict`, a deprecated request and a
mid-stream failure (`->` client, `<-` server):

```text
-> {"type":"request","ref":"c1","operation":"demo.add","data":{"a":2,"b":3}}
<- {"ref":"c1","request_id":"req_0665d9604e","trace_id":"tr_0665d9604e","operation":"demo.add","type":"result","seq":1,"status":"ok","data":5,"error":null,"effects":"none","data_count":0}
-> {"type":"request","ref":"c2","operation":"demo.countdown","data":{}}
<- {"ref":"c2","request_id":"req_07cd5f6a43","trace_id":"tr_07cd5f6a43","operation":"demo.countdown","type":"data","seq":1,"data":3,"error":null}
<- {"ref":"c2","request_id":"req_07cd5f6a43","trace_id":"tr_07cd5f6a43","operation":"demo.countdown","type":"data","seq":2,"data":2,"error":null}
<- {"ref":"c2","request_id":"req_07cd5f6a43","trace_id":"tr_07cd5f6a43","operation":"demo.countdown","type":"data","seq":3,"data":1,"error":null}
<- {"ref":"c2","request_id":"req_07cd5f6a43","trace_id":"tr_07cd5f6a43","operation":"demo.countdown","type":"result","seq":4,"status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}
-> {"type":"request","ref":"c3","operation":"chat.echo","data":{}}
-> {"type":"input","ref":"c3","seq":1,"data":"hi"}
<- {"ref":"c3","request_id":"req_0834168ce0","trace_id":"tr_0834168ce0","operation":"chat.echo","type":"data","seq":1,"data":"hi","error":null}
-> {"type":"request","ref":"c3","operation":"demo.add","data":{"a":1}}
<- {"ref":"","request_id":"","trace_id":"","operation":"demo.add","type":"result","status":"error","data":null,"error":{"kind":"conflict","code":"conflict.ref","message":"ref `c3` is already in flight","retryable":false,"details":{"ref":"c3"}},"effects":"none","data_count":0}
-> {"type":"input","ref":"c3","seq":3,"data":"skip"}
<- {"ref":"c3","request_id":"req_0834168ce0","trace_id":"tr_0834168ce0","operation":"chat.echo","type":"result","seq":2,"status":"error","data":null,"error":{"kind":"conflict","code":"conflict.input_sequence","message":"expected send_seq 2, got 3","retryable":false},"effects":"none","data_count":1}
-> {"type":"request","ref":"c4","operation":"chat.echo","data":{}}
-> {"type":"input","ref":"c4","seq":1,"data":5}
<- {"ref":"c4","request_id":"req_0957da492d","trace_id":"tr_0957da492d","operation":"chat.echo","type":"result","seq":1,"status":"error","data":null,"error":{"kind":"validation","code":"validation.input","message":"input item 1 at $ must be text, got integer","retryable":false,"details":{"seq":1,"path":"$","expected":"text","found":"integer"}},"effects":"none","data_count":0}
-> {"type":"request","ref":"c5","operation":"chat.echo","data":{}}
-> {"type":"finish_input","ref":"c5"}
<- {"ref":"c5","request_id":"req_10a6b9cc92","trace_id":"tr_10a6b9cc92","operation":"chat.echo","type":"result","seq":1,"status":"ok","data":{"echoed":0},"error":null,"effects":"none","data_count":0}
-> {"type":"request","ref":"r1","operation":"demo.read","data":{"path":"data/a.txt"},"restrict":{"grants":[{"capability":"allow_read","targets":["./data/other/**"]}]}}
<- {"ref":"r1","request_id":"req_11f612fae7","trace_id":"tr_11f612fae7","operation":"demo.read","type":"result","seq":1,"status":"error","data":null,"error":{"kind":"permission","code":"permission.denied","message":"allow_read read on data/a.txt denied: request restriction: no grant for allow_read data/a.txt","retryable":false,"source":{"file":"app.rivet","line":44,"column":5,"end_line":44,"end_column":34},"operation_id":"demo.read","details":{"capability":"allow_read","access":"read","target":"data/a.txt"}},"effects":"none","data_count":0}
-> {"type":"request","ref":"l1","id":"demo.add","params":{"a":1,"b":2}}                     (0.1.0 keys: deprecated, accepted)
<- {"ref":"l1","request_id":"req_1251691c94","trace_id":"tr_1251691c94","operation":"demo.add","type":"result","seq":1,"status":"ok","data":3,"error":null,"effects":"none","data_count":0}
-> {"type":"request","ref":"f1","operation":"demo.flaky","data":{}}
<- {"ref":"f1","request_id":"req_13c8ccebf1","trace_id":"tr_13c8ccebf1","operation":"demo.flaky","type":"data","seq":1,"data":1,"error":null}
<- {"ref":"f1","request_id":"req_13c8ccebf1","trace_id":"tr_13c8ccebf1","operation":"demo.flaky","type":"result","seq":2,"status":"error","data":null,"error":{"kind":"not_found","code":"not_found.file","message":"./data/missing.txt: no such file","retryable":false,"source":{"file":"app.rivet","line":53,"column":5,"end_line":53,"end_column":50},"operation_id":"demo.flaky"},"effects":"none","data_count":1}
-> (close frame, code 1000)
<- (close frame, code 1000)
```

The terminal record of a ref **ended by a refused input** (`c3`, `c4` above) carries the next `seq` and the real
`data_count` (`c3` had sent `seq 1`, so it ends with `seq 2`, `data_count 1`), as
[`stream-record.schema.json`](schemas/stream-record.schema.json) requires (fixed in 0.2.0, INC-2026-0012).

Captured session 2 — finish, cancel, malformed frames, bad envelopes, limits, close:

```text
-> {"type":"request","ref":"e1","operation":"chat.echo","data":{}}
-> {"type":"input","ref":"e1","seq":1,"data":"hi"}
<- {"ref":"e1","request_id":"req_14deeb3a2e","trace_id":"tr_14deeb3a2e","operation":"chat.echo","type":"data","seq":1,"data":"hi","error":null}
-> {"type":"input","ref":"e1","seq":2,"data":"there"}
<- {"ref":"e1","request_id":"req_14deeb3a2e","trace_id":"tr_14deeb3a2e","operation":"chat.echo","type":"data","seq":2,"data":"there","error":null}
-> {"type":"finish_input","ref":"e1"}
<- {"ref":"e1","request_id":"req_14deeb3a2e","trace_id":"tr_14deeb3a2e","operation":"chat.echo","type":"result","seq":3,"status":"ok","data":{"echoed":2},"error":null,"effects":"none","data_count":2}
-> {"type":"request","ref":"e2","operation":"chat.echo","data":{}}
-> {"type":"cancel","ref":"e2"}
<- {"ref":"e2","request_id":"req_15fdf90b73","trace_id":"tr_15fdf90b73","operation":"chat.echo","type":"result","seq":1,"status":"cancelled","data":null,"error":{"kind":"cancelled","code":"cancelled.session","message":"the session was cancelled","retryable":false},"effects":"none","data_count":0}
-> not json
<- {"ref":"","request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.frame","message":"frame is not JSON: expected ident at line 1 column 2","retryable":false},"effects":"none","data_count":0}
-> {"type":"request","operation":"demo.add"}
<- {"ref":"","request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.frame","message":"frame needs a client-chosen `ref`","retryable":false},"effects":"none","data_count":0}
-> {"type":"bogus","ref":"x1"}
<- {"ref":"x1","request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.frame","message":"type must be request, input, finish_input or cancel","retryable":false},"effects":"none","data_count":0}
-> {"type":"cancel","ref":"zz"}
<- {"ref":"zz","request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"not_found.ref","message":"ref `zz` is not in flight","retryable":false},"effects":"none","data_count":0}
-> {"type":"request","ref":"d1","operation":"demo.nope"}
<- {"ref":"d1","request_id":"req_035762f467","trace_id":"tr_035762f467","operation":"demo.nope","type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"not_found.operation","message":"no operation `demo.nope`","retryable":false,"operation_id":"demo.nope"},"effects":"none","data_count":0}
-> {"type":"request","ref":"d2"}
<- {"ref":"d2","request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.required","message":"the input envelope needs `operation`","retryable":false,"details":{"field":"operation"}},"effects":"none","data_count":0}
-> {"type":"request","ref":"d3","operation":"demo.add","id":"demo.add"}
<- {"ref":"d3","request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.input_envelope","message":"use `operation` or the deprecated `id`, not both","retryable":false,"details":{"key":"operation","alias":"id"}},"effects":"none","data_count":0}
-> (binary frame 00 01)
<- {"ref":"","request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.frame","message":"frames must be JSON text","retryable":false},"effects":"none","data_count":0}
-> request h0 … h7 (eight chat.echo refs left open)
-> {"type":"request","ref":"h8","operation":"chat.echo","data":{}}
<- {"ref":"h8","request_id":"","trace_id":"","operation":"chat.echo","type":"result","status":"error","data":null,"error":{"kind":"limit","code":"limit.ws_refs","message":"at most 8 refs may be in flight per connection","retryable":true},"effects":"none","data_count":0}
-> (close frame, code 1000)
<- (close frame, code 1000)      h0 … h7 are cancelled and joined server-side; no further frames
```

Upgrade without the subprotocol:

```text
GET /v1/ws  (no Sec-WebSocket-Protocol)
HTTP/1.1 422 Unprocessable Entity
content-type: application/json

{"request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.subprotocol","message":"the WebSocket client must offer subprotocol rivet.v1","retryable":false},"effects":"none","data_count":0}
```

## Compatibility Notes

**Close semantics.** Refs are connection-owned: when the socket closes (client close frame, network drop, or server shutdown on SIGINT/SIGTERM) the server fires each in-flight ref's cancellation token through the session driver, waits for each scope to clean up (handles close in reverse order within the 5 s grace), and then closes. There is no resume across sockets — use polling sessions ([API-2026-0001](api-2026-0001-http-rest-sse-polling.md#polling-sessions)) when a request must survive reconnects. The server does not use custom close codes.

**Refused input.** A refused `input` or `finish_input` (sequence gap, an item that does not match `receives`, input already closed) sends that ref's terminal error record with the specific code first and then cancels its session, so the ref still ends with exactly one terminal record — the same codes polling reports.

**Remote CLI.** `rivet --endpoint URL request ID --stream --input-jsonl - [--timeout D]` uses this route: one `request` frame (with `deadline_ms` from `--timeout`), one `input` frame per stdin line, `finish_input` at EOF (or `cancel` on Ctrl-C), and prints each record as NDJSON without `ref`. With `--timeout 300ms` the ref ends with `timeout.request` ("`demo.relay` exceeded its 300 ms deadline", exit 6).

**Platforms.** macOS and Linux; Windows is not supported in 0.2.0 ([INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)).

## Related Documents

- [API index](index.md) · [HTTP REST, SSE and polling](api-2026-0001-http-rest-sse-polling.md) · [Error registry](api-2026-0005-error-registry.md)
- [API-2026-0006 envelopes](api-2026-0006-envelopes.md) · [MIG-2026-0001 migration](../migrations/mig-2026-0001-response-and-input-envelopes.md) · [MAN-2026-0006 serving and surfaces](../manuals/man-2026-0006-serving-and-surfaces.md)
- [Runtime architecture](../architecture/arch-2026-0001-rivet-runtime-architecture.md)
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) · [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) · [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial rivet.v1 contract with frames captured from `rivet serve` at commit f40d4aa. |
| 2 | 2026-09-28 | Claude | Fix batch through 829ca43: per-ref 16-frame lanes, specific refusal frames (`conflict.input_sequence`, `validation.input`, `conflict.input_closed`), `receives` item validation, `restrict` on request frames, `traceparent` on the upgrade, host byte budget, drain on SIGTERM; frames re-captured. |
| 3 | 2026-09-29 | Claude | 0.2.0 envelope sweep (D-41, TASK-070): request frames carry `operation`/`data` (deprecated `id`/`params` accepted); every server frame is an envelope record with `ref` first (no `completion`, no `type:"error"`); new refusals `validation.required` / `validation.input_envelope`; per-ref `deadline_ms` (captured); reading diagram; versioning timeline; all frames re-captured on the 0.2.0-rc (source `6f9943f`); known deviation of refused-input terminal records (no `seq`, `data_count` 0); platform note. |
| 4 | 2026-09-29 | Claude | INC-2026-0012 fixes: refused-input terminal records carry `seq` and the real `data_count`; `conflict.ref` for an in-flight ref is sent with `ref: ""` + `details.ref`; refusals at open carry request/trace IDs; unary refs keep `seq: 1` (decision recorded in INC-2026-0012); legacy frames' trace-note deprecation signal; remote CLI sends `deadline_ms`; frames `c3`, `c4`, `d1` re-captured. |
