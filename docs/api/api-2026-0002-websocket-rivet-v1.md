---
document_id: API-2026-0002
title: "Rivet WebSocket API: subprotocol rivet.v1"
document_type: api
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 2
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
scope: The `/v1/ws` route of `rivet serve` — upgrade, authentication, client and server frame types, refs, limits, sequencing and close semantics, with frames captured from a live server.
reason: DOCUMENTATION.md §31 API impact for PLAN-2026-0001 row D-25; the multiplexed WebSocket surface is new in 0.1.0.
related_documents: [PLAN-2026-0001, PROP-2026-0001, API-2026-0001, API-2026-0005, ARCH-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, api, websocket, sessions, duplex]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.1.0-dev (commit 829ca43)"
next_review_date: 2026-10-28
---

# Rivet WebSocket API: subprotocol rivet.v1

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** serve, ws, sessions

## Summary

`GET /v1/ws` upgrades to a WebSocket that carries JSON text frames under the subprotocol **`rivet.v1`**. One socket multiplexes up to **8 in-flight requests**, each identified by a client-chosen `ref`. Every ref is a *connection-owned* session: unary, streaming (`emits`) and duplex (`receives`) operations all run the same way, and each ref ends with **exactly one** terminal frame (`result` or `error`). WebSocket is a projection only — there are no WebSocket-only operations.

```text
 client                                         rivet serve  /v1/ws
   │ GET /v1/ws                                     │
   │ Sec-WebSocket-Protocol: rivet.v1               │
   │ Authorization: Bearer …   (when bearer)        │── authenticate once (upgrade request)
   │◀───────────── 101 Switching Protocols ─────────│   sec-websocket-protocol: rivet.v1
   │                                                │
   │ {"type":"request","ref":"c1","id":…,           │
   │  "restrict"?:{grants:[…]}}  ──────────────────▶│── authorize id for the principal
   │                                                │── open connection-owned session ── pump(c1)
   │                                                │      └─ per-ref lane (16 frames) ─┐
   │                                                │   lanes c1, c2 … merged into one ─┘ socket writer
   │◀──── {"type":"data","ref":"c1","seq":1,…} ─────│
   │ {"type":"input","ref":"c1","seq":1,"data":…} ─▶│   (duplex only)
   │ {"type":"finish_input","ref":"c1"} ──────────▶│
   │◀──── {"type":"result","ref":"c1",…} ───────────│   exactly one terminal frame per ref
   │                                                │
   │ close ────────────────────────────────────────▶│── cancel + join every in-flight ref
```

Frames below were captured from `rivet serve --listen 127.0.0.1:18432` (commit `f40d4aa`) serving [docs/demos/01-catalog/app.rivet](../demos/01-catalog/app.rivet) plus a scratch duplex operation `chat.echo` (`receives text`, `emits text`, echoes every input item, returns `{echoed: N}`); the refusal and `restrict` frames were re-captured on commit `829ca43` (`127.0.0.1:18904`, the same `chat.echo` plus a file-reading `demo.read`). IDs differ on every run.

## Audience and Stability

Integrators that need many concurrent or bidirectional requests over one connection. The frame vocabulary below is stable for `rivet.v1`; new optional fields may be added to server frames. A breaking change would use a new subprotocol name.

## Authentication

- Authentication runs **once**, on the HTTP upgrade request, with the same rules as every other route ([API-2026-0001 §Authentication](api-2026-0001-http-rest-sse-polling.md#authentication)): auth `none` on loopback → principal `local`; bearer → `Authorization: Bearer TOKEN`. Failure answers the upgrade with the JSON ErrorEnvelope (401) instead of 101.
- A valid W3C `traceparent` header on the upgrade request becomes the trace of every ref opened on that socket.
- The client **must** offer `rivet.v1` in `Sec-WebSocket-Protocol`; otherwise the upgrade is answered `422 validation.subprotocol` (`"the WebSocket client must offer subprotocol rivet.v1"`).
- **Each `request` frame** is authorized separately against `serve.principals` for the connection's principal (refused → `error` frame with `permission.denied`).
- The `ws` surface must be enabled in `serve.surfaces`; otherwise `/v1/ws` is `404 not_found.route`.

## Endpoints or Events

| Direction | `type` | Fields | Meaning |
|---|---|---|---|
| client → server | `request` | `ref`, `id`, `params?`, `restrict?` | Start an operation on a new ref; `restrict: {grants:[…]}` narrows this ref's authority (never widens) |
| client → server | `input` | `ref`, `seq`, `data` | One input item for a duplex ref (`seq` starts at 1) |
| client → server | `finish_input` | `ref` | Half-close the ref's input |
| client → server | `cancel` | `ref` | Cancel the ref |
| server → client | `data` | `ref`, `request_id`, `trace_id`, `seq`, `data` | One emitted item |
| server → client | `result` | `ref`, `completion` | Terminal success; `completion` = Completion `{request_id, trace_id, result, data_count, effects}` |
| server → client | `error` | `ref`, `error`, `request_id?`, `trace_id?` | Terminal failure of a ref, a refused `input`/`finish_input` (terminal for that ref), **or** a refusal of a frame for a ref that is not in flight |

```text
  per-ref state machine (server side)

   (none) ──request──▶ open ──data*──▶ open ──result|error──▶ ended (ref may be reused)
                        │  ▲ input seq=n (ok)                    ▲
                        │  └───────────┘                         │
                        ├── finish_input ─▶ input closed ────────┤
                        ├── cancel ──────────────────────────────┤  error cancelled.session
                        ├── refused input / finish ──────────────┤  error = the SPECIFIC refusal
                        │     (sent first, then the session is   │  (conflict.input_sequence,
                        │      cancelled; its own frame dropped) │   validation.input, conflict.input_closed)
                        └── socket closed ── cancel + join ──────┘  (no frame: socket gone)
```

## Request Format

Client frames are **JSON text** frames. Every frame needs a non-empty string `ref`.

```json
{"type":"request","ref":"c1","id":"demo.add","params":{"a":2,"b":3}}
{"type":"request","ref":"r1","id":"demo.read","params":{"path":"data/a.txt"},"restrict":{"grants":[{"capability":"allow_read","targets":["./data/**"]}]}}
{"type":"input","ref":"c3","seq":1,"data":"hi"}
{"type":"finish_input","ref":"c3"}
{"type":"cancel","ref":"c3"}
```

Rules:

| Rule | Violation → error frame (socket stays open) |
|---|---|
| Frame is JSON text | not JSON / binary → `validation.frame` with `ref: ""` |
| `ref` present | missing → `validation.frame`, `ref: ""` |
| `type` ∈ request, input, finish_input, cancel | else `validation.frame` for that ref |
| `request` has non-empty `id` | `validation.frame` |
| `input` has integer `seq` | `validation.frame` |
| `request` ref not already in flight | `conflict.ref` (409 semantics) |
| ≤ 8 refs in flight per connection | 9th → `limit.ws_refs` (retryable) |
| `input` / `finish_input` / `cancel` target an in-flight ref | `not_found.ref` |
| `input.seq` = next expected (1, 2, …); identical retry of the last seq is accepted | otherwise the ref ends with `conflict.input_sequence` (then its session is cancelled) |
| `input.data` matches the operation's `receives` | otherwise the ref ends with `validation.input` (`details {seq, path, expected, found}`) |
| no `input` after `finish_input` | otherwise the ref ends with `conflict.input_closed` |
| `restrict` (optional) is `{grants:[…]}` | other keys / malformed grants → `policy.invalid` (`details.pointer` under `/restrict`) for that ref |

## Response Format

Server frames are JSON text. `data` frames carry the request's `request_id` / `trace_id`; `result` frames carry them inside `completion`.

```json
{"type":"data","ref":"c2","request_id":"req_02712b8232","trace_id":"tr_02712b8232","seq":1,"data":3}
{"type":"result","ref":"c2","completion":{"request_id":"req_02712b8232","trace_id":"tr_02712b8232","result":{"count":3},"data_count":3,"effects":"none"}}
{"type":"error","ref":"e2","request_id":"req_05db0cb0b1","trace_id":"tr_05db0cb0b1","error":{"kind":"cancelled","code":"cancelled.session","message":"the session was cancelled","retryable":false,"effects":"none"}}
```

**Flow control.** Each ref has its own outbound **lane of 16 frames**; the lanes are merged into the one socket writer. A slow consumer of one ref blocks only that ref's producer (backpressure), never the other refs. Each ref's session also keeps a bounded event log (16 frames), and its retained bytes count against the host budget `limits.max_buffered_bytes` (`limit.buffered_bytes`). The per-ref pump reads with a 5 s bounded wait and forwards frames in `seq` order.

```text
  ref c1 session ─▶ lane c1 [16] ─┐
  ref c2 session ─▶ lane c2 [16] ─┼─▶ writer ─▶ socket        a full lane blocks only its own producer
  ref c3 session ─▶ lane c3 [16] ─┘
```

## Error Format

The `error` object is the same RivetError used everywhere ([API-2026-0005](api-2026-0005-error-registry.md)): `kind`, `code`, `message`, `retryable`, `effects`, plus optional `operation_id`, `details`, `hint`, `source`. A refused frame never closes the socket; it gets an `error` frame for its `ref` (or `""` when no ref could be read). Frame-level errors for a ref that was never opened do **not** count as that ref's terminal frame. A refused `input` or `finish_input` on an open ref **is** that ref's terminal frame and carries the specific code.

## Rate Limits

| Limit | Value | Code |
|---|---|---|
| Refs in flight per connection | 8 | `limit.ws_refs` |
| Sessions per principal (shared with polling / MCP / `rivet.sessions.*`) | 8 | `limit.sessions` |
| Outbound lane per ref | 16 frames | backpressure (per ref) |
| Event log per ref | 16 frames / 32 MiB | backpressure |
| Bytes retained by all sessions of the host | `limits.max_buffered_bytes` (default 256 MiB) | `limit.buffered_bytes` |
| Deadline per ref | the default request deadline (30 s); a `request` frame has no `deadline_ms` | `timeout.request` |
| Top-level concurrent requests | `limits.max_concurrent_requests` (default 64) | `limit.concurrency` |

## Versioning and Deprecation

The subprotocol string is the version. `rivet.v1` is the only protocol in 0.1.0; nothing is deprecated.

## Examples

Captured session 1 — unary, streaming, duplex, duplicate ref:

```text
-> {"type":"request","ref":"c1","id":"demo.add","params":{"a":2,"b":3}}
<- {"type":"result","ref":"c1","completion":{"request_id":"req_01f0fdda25","trace_id":"tr_01f0fdda25","result":5,"data_count":0,"effects":"none"}}
-> {"type":"request","ref":"c2","id":"demo.countdown","params":{}}
<- {"type":"data","ref":"c2","request_id":"req_02712b8232","trace_id":"tr_02712b8232","seq":1,"data":3}
<- {"type":"data","ref":"c2","request_id":"req_02712b8232","trace_id":"tr_02712b8232","seq":2,"data":2}
<- {"type":"data","ref":"c2","request_id":"req_02712b8232","trace_id":"tr_02712b8232","seq":3,"data":1}
<- {"type":"result","ref":"c2","completion":{"request_id":"req_02712b8232","trace_id":"tr_02712b8232","result":{"count":3},"data_count":3,"effects":"none"}}
-> {"type":"request","ref":"c3","id":"chat.echo","params":{}}
-> {"type":"input","ref":"c3","seq":1,"data":"hi"}
<- {"type":"data","ref":"c3","request_id":"req_03f21a2227","trace_id":"tr_03f21a2227","seq":1,"data":"hi"}
-> {"type":"request","ref":"c3","id":"demo.add","params":{"a":1}}
<- {"type":"error","ref":"c3","error":{"kind":"conflict","code":"conflict.ref","message":"ref `c3` is already in flight","retryable":false,"effects":"none"}}
```

Captured session 1b (commit `829ca43`) — refused input frames end their ref with the specific code; `restrict` narrows one ref:

```text
-> {"type":"request","ref":"c1","id":"chat.echo","params":{}}
-> {"type":"input","ref":"c1","seq":1,"data":"hi"}
<- {"type":"data","ref":"c1","request_id":"req_01c6d564cd","trace_id":"tr_01c6d564cd","seq":1,"data":"hi"}
-> {"type":"input","ref":"c1","seq":3,"data":"skip"}
<- {"type":"error","ref":"c1","request_id":"req_…","trace_id":"tr_…","error":{"kind":"conflict","code":"conflict.input_sequence","message":"expected send_seq 2, got 3","retryable":false,"effects":"none"}}
-> {"type":"request","ref":"c2","id":"chat.echo","params":{}}
-> {"type":"input","ref":"c2","seq":1,"data":5}
<- {"type":"error","ref":"c2","request_id":"req_…","trace_id":"tr_…","error":{"kind":"validation","code":"validation.input","message":"input item 1 at $ must be text, got integer","retryable":false,"effects":"none","details":{"seq":1,"path":"$","expected":"text","found":"integer"}}}
-> {"type":"request","ref":"c3","id":"chat.echo","params":{}}
-> {"type":"finish_input","ref":"c3"}
<- {"type":"result","ref":"c3","completion":{"request_id":"req_03c4611c47","trace_id":"tr_03c4611c47","result":{"echoed":0},"data_count":0,"effects":"none"}}
-> {"type":"request","ref":"r1","id":"demo.read","params":{"path":"data/a.txt"},"restrict":{"grants":[{"capability":"allow_read","targets":["./data/other/**"]}]}}
<- {"type":"error","ref":"r1","request_id":"req_0444eace94","trace_id":"tr_0444eace94","error":{"kind":"permission","code":"permission.denied","message":"allow_read read on data/a.txt denied: request restriction: no grant for allow_read data/a.txt","retryable":false,"effects":"none","source":{"file":"app.rivet","line":73,"column":5,"end_line":73,"end_column":34},"operation_id":"demo.read","details":{"capability":"allow_read","access":"read","target":"data/a.txt"}}}
```

Captured session 2 — finish, cancel, malformed frames, limits, close:

```text
-> {"type":"request","ref":"e1","id":"chat.echo","params":{}}
-> {"type":"input","ref":"e1","seq":1,"data":"hi"}
<- {"type":"data","ref":"e1","request_id":"req_045a2619fc","trace_id":"tr_045a2619fc","seq":1,"data":"hi"}
-> {"type":"input","ref":"e1","seq":2,"data":"there"}
<- {"type":"data","ref":"e1","request_id":"req_045a2619fc","trace_id":"tr_045a2619fc","seq":2,"data":"there"}
-> {"type":"finish_input","ref":"e1"}
<- {"type":"result","ref":"e1","completion":{"request_id":"req_045a2619fc","trace_id":"tr_045a2619fc","result":{"echoed":2},"data_count":2,"effects":"none"}}
-> {"type":"request","ref":"e2","id":"chat.echo","params":{}}
-> {"type":"cancel","ref":"e2"}
<- {"type":"error","ref":"e2","request_id":"req_05db0cb0b1","trace_id":"tr_05db0cb0b1","error":{"kind":"cancelled","code":"cancelled.session","message":"the session was cancelled","retryable":false,"effects":"none"}}
-> not json
<- {"type":"error","ref":"","error":{"kind":"validation","code":"validation.frame","message":"frame is not JSON: expected ident at line 1 column 2","retryable":false,"effects":"none"}}
-> {"type":"request","id":"demo.add"}
<- {"type":"error","ref":"","error":{"kind":"validation","code":"validation.frame","message":"frame needs a client-chosen `ref`","retryable":false,"effects":"none"}}
-> {"type":"bogus","ref":"x1"}
<- {"type":"error","ref":"x1","error":{"kind":"validation","code":"validation.frame","message":"type must be request, input, finish_input or cancel","retryable":false,"effects":"none"}}
-> {"type":"cancel","ref":"zz"}
<- {"type":"error","ref":"zz","error":{"kind":"not_found","code":"not_found.ref","message":"ref `zz` is not in flight","retryable":false,"effects":"none"}}
-> {"type":"request","ref":"d1","id":"demo.nope"}
<- {"type":"error","ref":"d1","error":{"kind":"not_found","code":"not_found.operation","message":"no operation `demo.nope`","retryable":false,"effects":"none"}}
-> (binary frame 00 01)
<- {"type":"error","ref":"","error":{"kind":"validation","code":"validation.frame","message":"frames must be JSON text","retryable":false,"effects":"none"}}
-> request h0 … h7 (eight chat.echo refs left open)
-> {"type":"request","ref":"h8","id":"chat.echo","params":{}}
<- {"type":"error","ref":"h8","error":{"kind":"limit","code":"limit.ws_refs","message":"at most 8 refs may be in flight per connection","retryable":true,"effects":"none"}}
-> (close frame, code 1000)
<- (close frame, code 1000)      h0 … h7 are cancelled and joined server-side; no further frames
```

Upgrade without the subprotocol:

```text
GET /v1/ws  (no Sec-WebSocket-Protocol)
HTTP/1.1 422 Unprocessable Entity
content-type: application/json
{"request_id":"","trace_id":"","error":{"kind":"validation","code":"validation.subprotocol",…}}
```

## Compatibility Notes

**Close semantics.** Refs are connection-owned: when the socket closes (client close frame, network drop, or server shutdown on SIGINT/SIGTERM) the server fires each in-flight ref's cancellation token through the session driver, waits for each scope to clean up (handles close in reverse order within the 5 s grace), and then closes. There is no resume across sockets — use polling sessions ([API-2026-0001](api-2026-0001-http-rest-sse-polling.md#polling-sessions)) when a request must survive reconnects. The server does not use custom close codes in 0.1.0.

**Refused input.** A refused `input` or `finish_input` (sequence gap, an item that does not match `receives`, input already closed) sends that ref's terminal `error` frame with the specific code first and then cancels its session, so the ref still ends with exactly one terminal frame — the same codes polling reports.

**Remote CLI.** `rivet --endpoint URL request ID --stream --input-jsonl -` uses this route: one `request` frame, one `input` frame per stdin line, `finish_input` at EOF (or `cancel` on Ctrl-C). `--timeout` is not applied over this duplex path in 0.1.0 (a known limitation; the ref runs under the default 30 s request deadline).

## Related Documents

- [API index](index.md) · [HTTP REST, SSE and polling](api-2026-0001-http-rest-sse-polling.md) · [Error registry](api-2026-0005-error-registry.md)
- [Runtime architecture](../architecture/arch-2026-0001-rivet-runtime-architecture.md)
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) · [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial rivet.v1 contract with frames captured from `rivet serve` at commit f40d4aa. |
| 2 | 2026-09-28 | Claude | Fix batch through 829ca43: per-ref 16-frame lanes, specific refusal frames (`conflict.input_sequence`, `validation.input`, `conflict.input_closed`), `receives` item validation, `restrict` on request frames, `traceparent` on the upgrade, host byte budget, drain on SIGTERM; frames re-captured. |
