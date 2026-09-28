---
document_id: API-2026-0002
title: "Rivet WebSocket API: subprotocol rivet.v1"
document_type: api
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
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
last_verified_version: "0.1.0-dev (commit f40d4aa)"
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
   │ {"type":"request","ref":"c1","id":…}  ────────▶│── authorize id for the principal
   │                                                │── open connection-owned session ── pump(c1)
   │◀──── {"type":"data","ref":"c1","seq":1,…} ─────│
   │ {"type":"input","ref":"c1","seq":1,"data":…} ─▶│   (duplex only)
   │ {"type":"finish_input","ref":"c1"} ──────────▶│
   │◀──── {"type":"result","ref":"c1",…} ───────────│   exactly one terminal frame per ref
   │                                                │
   │ close ────────────────────────────────────────▶│── cancel + join every in-flight ref
```

Frames below were captured from `rivet serve --listen 127.0.0.1:18432` (commit `f40d4aa`) serving [docs/demos/01-catalog/app.rivet](../demos/01-catalog/app.rivet) plus a scratch duplex operation `chat.echo` (`receives text`, `emits text`, echoes every input item, returns `{echoed: N}`). IDs differ on every run.

## Audience and Stability

Integrators that need many concurrent or bidirectional requests over one connection. The frame vocabulary below is stable for `rivet.v1`; new optional fields may be added to server frames. A breaking change would use a new subprotocol name.

## Authentication

- Authentication runs **once**, on the HTTP upgrade request, with the same rules as every other route ([API-2026-0001 §Authentication](api-2026-0001-http-rest-sse-polling.md#authentication)): auth `none` on loopback → principal `local`; bearer → `Authorization: Bearer TOKEN`. Failure answers the upgrade with the JSON ErrorEnvelope (401) instead of 101.
- The client **must** offer `rivet.v1` in `Sec-WebSocket-Protocol`; otherwise the upgrade is answered `422 validation.subprotocol` (`"the WebSocket client must offer subprotocol rivet.v1"`).
- **Each `request` frame** is authorized separately against `serve.principals` for the connection's principal (refused → `error` frame with `permission.denied`).
- The `ws` surface must be enabled in `serve.surfaces`; otherwise `/v1/ws` is `404 not_found.route`.

## Endpoints or Events

| Direction | `type` | Fields | Meaning |
|---|---|---|---|
| client → server | `request` | `ref`, `id`, `params?` | Start an operation on a new ref |
| client → server | `input` | `ref`, `seq`, `data` | One input item for a duplex ref (`seq` starts at 1) |
| client → server | `finish_input` | `ref` | Half-close the ref's input |
| client → server | `cancel` | `ref` | Cancel the ref |
| server → client | `data` | `ref`, `request_id`, `trace_id`, `seq`, `data` | One emitted item |
| server → client | `result` | `ref`, `completion` | Terminal success; `completion` = Completion `{request_id, trace_id, result, data_count, effects}` |
| server → client | `error` | `ref`, `error`, `request_id?`, `trace_id?` | Terminal failure of a ref, **or** a refusal of one client frame |

```text
  per-ref state machine (server side)

   (none) ──request──▶ open ──data*──▶ open ──result|error──▶ ended (ref may be reused)
                        │  ▲ input seq=n (ok)                    ▲
                        │  └───────────┘                         │
                        ├── finish_input ─▶ input closed ────────┤
                        ├── cancel ──────────────────────────────┤  error cancelled.session
                        ├── refused input / finish ── cancels ───┤  error cancelled.session
                        └── socket closed ── cancel + join ──────┘  (no frame: socket gone)
```

## Request Format

Client frames are **JSON text** frames. Every frame needs a non-empty string `ref`.

```json
{"type":"request","ref":"c1","id":"demo.add","params":{"a":2,"b":3}}
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
| `input.seq` = next expected (1, 2, …); identical retry of the last seq is accepted | otherwise the ref is **cancelled** (terminal `cancelled.session`) |

## Response Format

Server frames are JSON text. `data` frames carry the request's `request_id` / `trace_id`; `result` frames carry them inside `completion`.

```json
{"type":"data","ref":"c2","request_id":"req_02712b8232","trace_id":"tr_02712b8232","seq":1,"data":3}
{"type":"result","ref":"c2","completion":{"request_id":"req_02712b8232","trace_id":"tr_02712b8232","result":{"count":3},"data_count":3,"effects":"none"}}
{"type":"error","ref":"e2","request_id":"req_05db0cb0b1","trace_id":"tr_05db0cb0b1","error":{"kind":"cancelled","code":"cancelled.session","message":"the session was cancelled","retryable":false,"effects":"none"}}
```

**Flow control.** Each ref's session keeps a bounded event log (16 frames); a producer that outruns the socket blocks (backpressure) rather than buffering without bound. The per-ref pump reads with a 5 s bounded wait and forwards frames in `seq` order.

## Error Format

The `error` object is the same RivetError used everywhere ([API-2026-0005](api-2026-0005-error-registry.md)): `kind`, `code`, `message`, `retryable`, `effects`, plus optional `operation_id`, `details`, `hint`, `source`. A refused frame never closes the socket; it gets an `error` frame for its `ref` (or `""` when no ref could be read). Frame-level errors for a ref that was never opened do **not** count as that ref's terminal frame.

## Rate Limits

| Limit | Value | Code |
|---|---|---|
| Refs in flight per connection | 8 | `limit.ws_refs` |
| Sessions per principal (shared with polling / MCP / `rivet.sessions.*`) | 8 | `limit.sessions` |
| Event log per ref | 16 frames / 32 MiB | backpressure |
| Top-level concurrent requests | `limits.max_concurrent_requests` (default 64) | `limit.concurrency` |

## Versioning and Deprecation

The subprotocol string is the version. `rivet.v1` is the only protocol in 0.1.0; nothing is deprecated.

## Examples

Captured session 1 — unary, streaming, duplex, duplicate ref, sequence gap:

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
-> {"type":"input","ref":"c3","seq":3,"data":"skip"}
<- {"type":"error","ref":"c3","request_id":"req_03f21a2227","trace_id":"tr_03f21a2227","error":{"kind":"cancelled","code":"cancelled.session","message":"the session was cancelled","retryable":false,"effects":"none"}}
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

**Close semantics.** Refs are connection-owned: when the socket closes (client close frame, network drop, or server shutdown) the server cancels every in-flight ref through the session driver, waits for each scope to clean up (handles close in reverse order), and then closes. There is no resume across sockets — use polling sessions ([API-2026-0001](api-2026-0001-http-rest-sse-polling.md#polling-sessions)) when a request must survive reconnects. The server does not use custom close codes in 0.1.0.

**Refused input.** A refused `input` or `finish_input` (sequence gap, input already closed) cancels its ref so the ref still ends with exactly one terminal frame; that terminal frame is `cancelled.session`, and the specific refusal code (for example `conflict.input_sequence`) is not reported on WebSocket in 0.1.0. Polling reports the specific code.

**Remote CLI.** `rivet --endpoint URL request ID --stream --input-jsonl -` uses this route: one `request` frame, one `input` frame per stdin line, `finish_input` at EOF (or `cancel` on Ctrl-C). `--timeout` is not applied over this duplex path in 0.1.0.

## Related Documents

- [API index](index.md) · [HTTP REST, SSE and polling](api-2026-0001-http-rest-sse-polling.md) · [Error registry](api-2026-0005-error-registry.md)
- [Runtime architecture](../architecture/arch-2026-0001-rivet-runtime-architecture.md)
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) · [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial rivet.v1 contract with frames captured from `rivet serve` at commit f40d4aa. |
