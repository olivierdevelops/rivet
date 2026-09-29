---
document_id: API-2026-0003
title: "Rivet MCP server: tools over Streamable HTTP and stdio"
document_type: api
status: active
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 4
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [serve, mcp, sessions, registry]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, server]
audience: [developers, integrators, agent-builders]
scope: Rivet as an MCP server (protocol 2025-11-25) — `/mcp` Streamable HTTP and `rivet serve --stdio`; initialize, tools/list shape, direct named tools, built-in tools, outputSchema (the 0.2.0 ResponseEnvelope), `structuredContent` envelopes, session delivery `_meta`, bridge `_meta`, JSON-RPC and tool errors. Rivet as an MCP *client* (connectors) is covered by SEC-2026-0001 and the language reference.
reason: DOCUMENTATION.md §31 API impact for PLAN-2026-0001 row D-26 (0.1.0 contract) and PLAN-2026-0002 row D-42 (0.2.0 envelope sweep, TASK-070).
related_documents: [PLAN-2026-0001, PLAN-2026-0002, PROP-2026-0001, PROP-2026-0002, API-2026-0001, API-2026-0005, API-2026-0006, MIG-2026-0001, SEC-2026-0001, ARCH-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, api, mcp, json-rpc, tools, envelope]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.2.0-rc (source at 6f9943f)"
next_review_date: 2026-10-29
---

# Rivet MCP server: tools over Streamable HTTP and stdio

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** serve, mcp, sessions, registry

## Summary

Every public operation in a Rivet bundle is an MCP **tool** whose name is the operation ID. Rivet speaks MCP protocol **2025-11-25**, tools capability only, over two transports:

```text
  ┌──────────────────────────── rivet serve ──────────────────────────────┐
  │                                                                       │
  │  --listen 127.0.0.1:8080          POST /mcp   JSON-RPC in, JSON out    │
  │  (Streamable HTTP, surface "mcp") GET  /mcp   405 (no server stream)   │
  │                                   DELETE /mcp end the MCP session      │
  │                                                                       │
  │  --stdio                          one JSON-RPC message per line on     │
  │  (MCP only, binds nothing)        stdin → replies on stdout; the       │
  │                                   start receipt goes to stderr         │
  └───────────────────────────┬───────────────────────────────────────────┘
                              ▼
            tools/list ─▶ direct tools (authorized public ops) + built-ins the principal may call
            tools/call ─▶ structuredContent = ResponseEnvelope (API-2026-0006), content[0].text = same JSON
                          unary op     : status ok,       data = result          (isError false)
                          streaming op : status accepted, data = SessionReceipt  (_meta rivet/delivery = session)
                          failure      : status error | cancelled, error = {…}   (isError true)
```

Examples were captured on 2026-09-29 (macOS 26.4) from the 0.2.0 release candidate (`cargo build --release --features cli`, source at `6f9943f`): `rivet serve --listen 127.0.0.1:18950` on a scratch bundle (`demo.add`, `demo.countdown`, the duplex `chat.echo`, `demo.read` with `allow_read ./data/**`, `demo.flaky`), and `rivet serve --stdio` on [docs/demos/01-catalog](../demos/01-catalog/app.rivet). **Request, trace, session and MCP session IDs differ on every run.** `serverInfo.version` reads `0.1.0` until the release commit bumps the workspace version.

In 0.2.0 every tool result is the **ResponseEnvelope** of [API-2026-0006](api-2026-0006-envelopes.md) (0.1.0 put a bare
Completion, SessionReceipt or error envelope in `structuredContent`; [MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md#mcp) maps them).

## Audience and Stability

MCP clients and agent hosts. The tool naming (operation ID), result shapes (envelopes from 0.2.0) and `_meta` keys below are stable for 0.2.x. The server targets only protocol version 2025-11-25; resources, prompts, sampling, server-initiated streams, JSON-RPC batches and the legacy HTTP+SSE transport are not offered.

## Authentication

| Transport | Principal |
|---|---|
| HTTP `/mcp` | Same `serve.auth` as every route ([API-2026-0001](api-2026-0001-http-rest-sse-polling.md#authentication)): `local` on loopback with auth `none`, or `Authorization: Bearer TOKEN` on **every** POST/DELETE. |
| stdio | Always the local operator (`local`); the parent process owns the pipe. |

Additional HTTP checks, in order:

```text
 POST /mcp
   ├─ Origin header present and not localhost/127.0.0.1/::1/same host?  ─▶ 403 permission.denied
   ├─ authenticate (bearer)                                              ─▶ 401 auth.required|auth.invalid
   ├─ MCP-Protocol-Version present and ≠ 2025-11-25?                     ─▶ 422 mcp.protocol_version
   ├─ body not one JSON-RPC 2.0 object?                                  ─▶ 400 JSON-RPC error (-32700/-32600)
   ├─ method initialize?  ─▶ 200 result + MCP-Session-Id header (session bound to this principal)
   ├─ no MCP-Session-Id?                                                 ─▶ 422 mcp.session_required
   ├─ session unknown or owned by another principal?                    ─▶ 404 not_found.mcp_session
   ├─ notification (no id)?                                              ─▶ 202, empty body
   └─ request ─▶ 200 JSON-RPC result | JSON-RPC error
```

`tools/list` shows only operations the principal may call; `tools/call` on a hidden or unknown name is `-32602 Unknown tool: NAME` (existence is not revealed).

## Endpoints or Events

| JSON-RPC method | Result |
|---|---|
| `initialize` | `{protocolVersion:"2025-11-25", capabilities:{tools:{}}, serverInfo:{name:"rivet", version}}` |
| `notifications/*` | no response (HTTP 202) |
| `ping` | `{}` |
| `tools/list` | `{tools:[…]}` — direct tools, then the built-ins the principal may call; no pagination |
| `tools/call` | tool result (below) |
| anything else | `-32601 method not found: METHOD` |

### Direct named tools

One per authorized public operation:

| Tool field | Source |
|---|---|
| `name` | operation ID (`demo.add`) |
| `title` | operation `name` |
| `description` | operation `description` |
| `inputSchema` | parameters as JSON Schema (`additionalProperties: false`) |
| `outputSchema` | the ResponseEnvelope schema; its `data` is `anyOf` [the declared `output`, `null`] for a unary tool and [the SessionReceipt schema, `null`] for a streaming one |
| `_meta` | streaming only: `{"rivet/delivery": "session"}` |

### Built-in tools listed beside them

`tools/list` appends every built-in the calling principal is allowed to call (the same `serve.principals` decision as `tools/call`): the local principal sees all twenty below; a network principal sees the generic ones, the `rivet.auth.*` tools its patterns match, and a sensitive tool only when its ID is listed exactly.

| Tool | Arguments | Purpose |
|---|---|---|
| `rivet.request` | `{operation, data?}` (deprecated `{id, params?}`) | Generic dispatch: the operation's envelope (unary) or an `accepted` envelope with a SessionReceipt (streaming) |
| `rivet.list` | `{cursor?, limit?, outputs?}` | Authorized summaries |
| `rivet.describe` | `{id}` | Full descriptor |
| `rivet.outputs` | `{id?, all?}` | Output / emits / receives / errors schema |
| `rivet.sessions.open` | `{operation, data?, deadline_ms?}` (deprecated `{id, params?}`) | Open a session → `status: "ok"`, `data` = SessionReceipt |
| `rivet.sessions.send` | `{session_id, send_seq ≥ 1, data}` | One input item |
| `rivet.sessions.finish_input` | `{session_id}` | Half-close input |
| `rivet.sessions.read` | `{session_id, after_seq?, max_events?, wait_ms? ≤ 5000}` | Events after `after_seq` (acknowledges earlier ones) |
| `rivet.sessions.cancel` | `{session_id}` | Cancel and await cleanup (a finished session reports its terminal state) |
| `rivet.io` | `{ids?, all?, by?, kind?, access?, check_policy?, needs?, strict?, include_bootstrap?, trace?, format?, report?}` | I/O manifest (sensitive) |
| `rivet.policy.generate` | `{ids?, all?}` | Least-privilege draft; never writes (sensitive) |
| `rivet.trace.show` | `{request_id}` | This host's recorded decisions (sensitive) |
| `rivet.trace.export` | `{request_id, path}` (`output` alias) | Write that trace to a new file through the broker (sensitive) |
| `rivet.capabilities` | `{}` | What this build supports (any authenticated principal) |
| `rivet.connectors.sync` | `{name, output}` | New candidate snapshot file (sensitive) |
| `rivet.auth.begin` / `complete` / `status` / `disconnect` / `cancel` | profile / account / transaction parameters | OAuth account management |

Built-in `outputSchema` is the ResponseEnvelope schema with an open `data`. Direct tools of operations that declare `emits`/`receives` carry the declared descriptions in those item schemas (since commit `2a751ab`).

## Request Format

```json
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"demo.add","arguments":{"a":2,"b":3}}}
```

An optional `restrict: {"grants": [...]}` beside `name` and `arguments` narrows the authority of this one call (and its nested calls) to the intersection with policy.json; it never widens, and a malformed value is an `isError` result with `policy.invalid`. A valid W3C `traceparent` header on the POST sets the call's trace, and the answer to a `tools/call` carries a `traceparent` header.

```json
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"demo.read","arguments":{"path":"data/a.txt"},
 "restrict":{"grants":[{"capability":"allow_read","targets":["./data/**"]}]}}}
```

Bridge recursion state may be passed in `params._meta`: `{"rivet/hops": N, "rivet/chain": ["rivet:…", …]}`. Rivet carries it into nested MCP connector calls made while serving the tool; a call at 8 hops fails with `limit.mcp_hops`, and a call whose next hop is already in the chain fails with `limit.mcp_recursion` (both before any connector I/O).

## Response Format

Tool results always carry both a text block (the envelope serialized as JSON) and `structuredContent` (the same
envelope as an object). `isError` is `true` when `status` is `error` **or** `cancelled`:

```json
{"jsonrpc":"2.0","id":3,"result":{
  "content":[{"type":"text","text":"{\"request_id\":\"req_24211eee50\",\"trace_id\":\"tr_24211eee50\",\"operation\":\"demo.add\",\"type\":\"result\",\"status\":\"ok\",\"data\":5,\"error\":null,\"effects\":\"none\",\"data_count\":0}"}],
  "structuredContent":{"request_id":"req_24211eee50","trace_id":"tr_24211eee50","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0},
  "isError":false}}
```

```text
 structuredContent.status ──▶ ok        → data = the operation result          isError false
                              accepted  → data = SessionReceipt (streaming tool) isError false
                              error     → data null, error = {kind, code, …}     isError true
                              cancelled → data null, error = {kind cancelled, …} isError true
```

### Session delivery for streaming operations

A tool whose operation declares `emits` (or `receives`) is marked `_meta: {"rivet/delivery": "session"}`. Calling it **does not block** until the stream ends: it opens a principal-owned session and returns its receipt; the client then drains it with `rivet.sessions.read` (and feeds duplex input with `rivet.sessions.send` / `finish_input`).

```text
 MCP client                                   rivet
   │ tools/call demo.countdown {}              │
   │──────────────────────────────────────────▶│ open session (principal-owned)
   │◀── envelope status accepted ──────────────│ data = {session_id, request_id, next_send_seq, expires_at, …}
   │ tools/call rivet.sessions.read            │
   │   {session_id, after_seq:0, wait_ms:2000} │
   │──────────────────────────────────────────▶│ waits ≤ 5 s for events
   │◀── envelope status ok, data = batch ──────│ {session_id, events:[records…], last_seq, terminal}
   │    … repeat with after_seq = last_seq until terminal = true
```

Captured:

```text
tools/call demo.countdown {}
→ "structuredContent":{"request_id":"req_262264ae0a","trace_id":"tr_262264ae0a","operation":"demo.countdown","type":"result","status":"accepted",
   "data":{"session_id":"ses_23a04511cb","request_id":"req_262264ae0a","trace_id":"tr_262264ae0a",
           "catalog_version":"sha256:d7bc50b8…2787","input_schema":null,"emits_schema":{"type":"integer"},
           "next_send_seq":1,"expires_at":"2026-09-28T21:28:19Z"},
   "error":null,"effects":"none","data_count":0},"isError":false

tools/call rivet.sessions.read {"session_id":"ses_23a04511cb","after_seq":0,"wait_ms":2000}
→ "structuredContent":{"request_id":"req_278ed859ff","trace_id":"tr_278ed859ff","operation":"rivet.sessions.read","type":"result","status":"ok",
   "data":{"session_id":"ses_23a04511cb","events":[
     {"request_id":"req_262264ae0a","trace_id":"tr_262264ae0a","operation":"demo.countdown","type":"data","seq":1,"data":3,"error":null},
     {"request_id":"req_262264ae0a","trace_id":"tr_262264ae0a","operation":"demo.countdown","type":"data","seq":2,"data":2,"error":null},
     {"request_id":"req_262264ae0a","trace_id":"tr_262264ae0a","operation":"demo.countdown","type":"data","seq":3,"data":1,"error":null},
     {"request_id":"req_262264ae0a","trace_id":"tr_262264ae0a","operation":"demo.countdown","type":"result","seq":4,"status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}],
     "last_seq":4,"terminal":true},
   "error":null,"effects":"none","data_count":0},"isError":false

tools/call rivet.sessions.cancel {"session_id":"ses_240cd00a28"}          (a running chat.echo session)
→ "structuredContent":{…,"operation":"rivet.sessions.cancel","type":"result","status":"ok","data":{"session_id":"ses_240cd00a28","request_id":"req_320b642548","state":"cancelled"},…}
tools/call rivet.sessions.read {"session_id":"ses_240cd00a28","after_seq":0,"wait_ms":1000}
→ "data":{"session_id":"ses_240cd00a28","events":[{"request_id":"req_320b642548","trace_id":"tr_320b642548","operation":"chat.echo","type":"result","seq":1,"status":"cancelled","data":null,"error":{"kind":"cancelled","code":"cancelled.session","message":"the session was cancelled","retryable":false},"effects":"none","data_count":0}],"last_seq":1,"terminal":true}
```

The read itself succeeds (`status: "ok"`, `isError: false`); the cancelled record is inside its `data.events`.

### outputSchema of a streaming tool

The ResponseEnvelope schema; `data` is the SessionReceipt or `null` (captured from `tools/list`, `demo.countdown`):

```json
{"type":"object","properties":{"request_id":{"type":"string"},"trace_id":{"type":"string"},"operation":{"type":["string","null"]},
 "type":{"type":"string","enum":["result"]},"status":{"type":"string","enum":["ok","error","cancelled","accepted"]},
 "data":{"anyOf":[{"type":"object","properties":{"session_id":{"type":"string"},"request_id":{"type":"string"},"trace_id":{"type":"string"},
                   "catalog_version":{"type":"string"},"input_schema":{},"emits_schema":{},"next_send_seq":{"type":"integer"},
                   "expires_at":{"type":"string"}},
                   "required":["session_id","request_id","catalog_version","next_send_seq","expires_at"]},{"type":"null"}]},
 "error":{"type":["object","null"]},"effects":{"type":"string","enum":["none","committed","partial","unknown"]},
 "data_count":{"type":"integer","minimum":0}},
 "required":["request_id","trace_id","operation","type","status","data","error","effects","data_count"]}
```

## Error Format

Two layers, never mixed:

| Failure | Shape | Example |
|---|---|---|
| Protocol problem | JSON-RPC `error` | `-32700` parse error, `-32600` not one 2.0 object / batch, `-32601` unknown method, `-32602` unknown tool or missing `name` |
| The operation failed or was cancelled | `result` with `isError: true`; `structuredContent` = envelope with `status: "error"` (or `"cancelled"`), `data: null`, `error` | validation, permission, runtime, timeout, cancel … |
| Transport-level refusal (HTTP) | Rivet error envelope (no JSON-RPC wrapper) with the registry HTTP status | 401, 403 Origin, 404 session, 422 version/session header |

Captured:

```text
tools/call demo.add {"b":1}
→ {"jsonrpc":"2.0","id":4,"result":{"content":[{"type":"text","text":"{…}"}],"structuredContent":{"request_id":"req_25a1ab58b5","trace_id":"tr_25a1ab58b5","operation":"demo.add","type":"result","status":"error","data":null,
   "error":{"kind":"validation","code":"validation.required","message":"missing required parameter `a`","retryable":false,"operation_id":"demo.add","details":{"field":"a"}},"effects":"none","data_count":0},"isError":true}}

tools/call demo.read {"path":"data/a.txt"} + "restrict":{"grants":[{"capability":"allow_read","targets":["./data/other/**"]}]}
→ "structuredContent":{"request_id":"req_358564dab7","trace_id":"tr_358564dab7","operation":"demo.read","type":"result","status":"error","data":null,
   "error":{"kind":"permission","code":"permission.denied","message":"allow_read read on data/a.txt denied: request restriction: no grant for allow_read data/a.txt",…},"effects":"none","data_count":0},"isError":true

tools/call nope.x          → {"jsonrpc":"2.0","id":13,"error":{"code":-32602,"message":"Unknown tool: nope.x"}}
resources/list             → {"jsonrpc":"2.0","id":14,"error":{"code":-32601,"message":"method not found: resources/list"}}
body [1]                   → 400 {"jsonrpc":"2.0","id":null,"error":{"code":-32600,"message":"expected one JSON-RPC message object (batches are not supported)"}}
body x                     → 400 {"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"parse error: expected value at line 1 column 1"}}
POST without session id    → 422 {"request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"validation","code":"mcp.session_required","message":"MCP-Session-Id header is required after initialize","retryable":false},"effects":"none","data_count":0}
MCP-Protocol-Version: 2024-11-05 → 422 {"request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"validation","code":"mcp.protocol_version","message":"unsupported MCP-Protocol-Version 2024-11-05; this server speaks 2025-11-25","retryable":false},"effects":"none","data_count":0}
Origin: https://evil.example → 403 {"request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"permission","code":"permission.denied","message":"Origin is not allowed for /mcp","retryable":false},"effects":"none","data_count":0}
GET /mcp                   → 405, allow: POST, DELETE
DELETE /mcp (live session) → 204;  again → 404 {"request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"not_found.mcp_session","message":"unknown or expired MCP session","retryable":false},"effects":"none","data_count":0}
```

Transport refusals request no operation, so `operation` is `null` (the JSON-RPC method is not an operation ID).
A failing `rivet.request` call names the operation it ran, like a successful one:

```text
tools/call rivet.request {"operation":"demo.add","data":{"b":2}}
→ structuredContent {"request_id":"req_12dc2051b4","trace_id":"tr_12dc2051b4","operation":"demo.add","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.required","message":"missing required parameter `a`","retryable":false,"operation_id":"demo.add","details":{"field":"a"}},"effects":"none","data_count":0}, isError true
```

## Rate Limits

No MCP-specific rate limit. Tool calls share the host budgets: `limits.max_concurrent_requests` (`limit.concurrency`), 8 sessions per principal (`limit.sessions`), and the request deadline (default 30 s).

## Versioning and Deprecation

`protocolVersion` in `initialize` is always `2025-11-25`; a request header naming another version is refused.

```text
 0.1.x  structuredContent = Completion | SessionReceipt | ErrorEnvelope   rivet.request {id, params}
 0.2.0  structuredContent = ResponseEnvelope (status ok|accepted|error|cancelled), outputSchema = envelope
        rivet.request / rivet.sessions.open {operation, data}; {id, params} accepted + HTTP `deprecation: true`
 0.3.0  {id, params} refused (validation.input_envelope)
```

A `rivet.request` call with the deprecated keys over HTTP `/mcp` answers normally and adds the response header
`deprecation: true` (captured below); over stdio there is no header.

## Examples

HTTP (`127.0.0.1:18950`, principal `local`):

```text
$ curl -s -i -X POST http://127.0.0.1:18950/mcp -H 'content-type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"doc","version":"0.2"}}}'
HTTP/1.1 200 OK
content-type: application/json
mcp-session-id: mcp_1a68f2fcea0c76135

{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"rivet","version":"0.1.0"}}}

$ curl -s -i -X POST http://127.0.0.1:18950/mcp -H 'mcp-session-id: mcp_1a68f2fcea0c76135' -H 'content-type: application/json' \
    -d '{"jsonrpc":"2.0","method":"notifications/initialized"}'
HTTP/1.1 202 Accepted

$ … -d '{"jsonrpc":"2.0","id":2,"method":"tools/list"}'
# tool names, in order (principal local — 5 direct tools, then the 20 built-ins):
demo.add, demo.countdown, chat.echo, demo.read, demo.flaky,
rivet.request, rivet.list, rivet.describe, rivet.outputs,
rivet.sessions.open, rivet.sessions.send, rivet.sessions.finish_input, rivet.sessions.read, rivet.sessions.cancel,
rivet.io, rivet.policy.generate, rivet.trace.show, rivet.trace.export, rivet.capabilities, rivet.connectors.sync,
rivet.auth.begin, rivet.auth.complete, rivet.auth.status, rivet.auth.disconnect, rivet.auth.cancel

$ … -d '{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"rivet.request","arguments":{"operation":"demo.add","data":{"a":40,"b":2}}}}'
{"jsonrpc":"2.0","id":7,"result":{"content":[{"type":"text","text":"{…}"}],"structuredContent":{"request_id":"req_280e31a64c","trace_id":"tr_280e31a64c","operation":"demo.add","type":"result","status":"ok","data":42,"error":null,"effects":"none","data_count":0},"isError":false}}

$ … -d '{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"rivet.request","arguments":{"id":"demo.add","params":{"a":40,"b":2}}}}'    # deprecated keys
HTTP/1.1 200 OK
deprecation: true
{"jsonrpc":"2.0","id":8,"result":{"content":[{"type":"text","text":"{…}"}],"structuredContent":{"request_id":"req_30089a94be","trace_id":"tr_30089a94be","operation":"demo.add","type":"result","status":"ok","data":42,"error":null,"effects":"none","data_count":0},"isError":false}}

$ … -d '{"jsonrpc":"2.0","id":15,"method":"tools/call","params":{"name":"rivet.capabilities","arguments":{}}}'     # abridged
{"jsonrpc":"2.0","id":15,"result":{"content":[{"type":"text","text":"{…}"}],"structuredContent":{"request_id":"req_360403c8ec","trace_id":"tr_360403c8ec","operation":"rivet.capabilities","type":"result","status":"ok",
 "data":{"version":"0.1.0","platform":{"os":"macos","arch":"aarch64"},"stages":{"A":"supported","B":"supported","C":"unsupported"},"features":[…],
         "sandbox":{"backend":"macos-seatbelt","status":"active",…},"serve":{…},"build_features":["serve","grpc","quic","oauth","cli"],"abi_version":1},
 "error":null,"effects":"none","data_count":0},"isError":false}}
```

The `demo.add` descriptor from that list (`outputSchema` is the envelope; `data` is the declared output or `null`):

```json
{"name":"demo.add","title":"Add two integers",
 "inputSchema":{"type":"object","properties":{"a":{"type":"integer","description":"First operand."},
   "b":{"type":"integer","description":"Second operand; defaults to zero.","default":0}},"required":["a"],"additionalProperties":false},
 "description":"Add two signed integers and return their sum.",
 "outputSchema":{"type":"object","properties":{"request_id":{"type":"string"},"trace_id":{"type":"string"},"operation":{"type":["string","null"]},
   "type":{"type":"string","enum":["result"]},"status":{"type":"string","enum":["ok","error","cancelled","accepted"]},
   "data":{"anyOf":[{"type":"integer","description":"Sum of a and b."},{"type":"null"}]},"error":{"type":["object","null"]},
   "effects":{"type":"string","enum":["none","committed","partial","unknown"]},"data_count":{"type":"integer","minimum":0}},
   "required":["request_id","trace_id","operation","type","status","data","error","effects","data_count"]}}
```

The `rivet.request` input schema lists the deprecated keys with `"deprecated": true`:

```json
{"type":"object","properties":{"operation":{"type":"string","description":"Operation ID."},"data":{"type":"object","description":"Operation input (default {})."},
 "id":{"type":"string","deprecated":true,"description":"Deprecated alias of `operation` (removed in 0.3.0)."},
 "params":{"type":"object","deprecated":true,"description":"Deprecated alias of `data` (removed in 0.3.0)."}},"required":[],"additionalProperties":false}
```

stdio (three lines in, two replies out — the notification gets none):

```text
$ printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"rivet-demo","version":"0.1"}}}' \
    '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
    '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"demo.add","arguments":{"a":2,"b":3}}}' \
  | rivet --file app.rivet serve --stdio
{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"rivet","version":"0.1.0"}}}
{"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"{\"request_id\":\"req_01b9f53ad5\",\"trace_id\":\"tr_01b9f53ad5\",\"operation\":\"demo.add\",\"type\":\"result\",\"status\":\"ok\",\"data\":5,\"error\":null,\"effects\":\"none\",\"data_count\":0}"}],"structuredContent":{"request_id":"req_01b9f53ad5","trace_id":"tr_01b9f53ad5","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0},"isError":false}}
# stderr:
{"listen_addr":null,"stdio":true,"surfaces":["mcp"],"auth_type":"none","catalog_version":"sha256:67104f0e7faaeafee253db758a668a9b24aa4263677e9d5ea2451b32019f9730","policy_hash":null}
# exit 0 at end of stdin
```

## Compatibility Notes

| Aspect | HTTP `/mcp` | stdio |
|---|---|---|
| Framing | one JSON-RPC object per POST body, JSON response | one object per line |
| MCP session | `MCP-Session-Id` from `initialize`, required afterwards, bound to the principal, ended by `DELETE` | none (the process is the session) |
| Principal | from `serve.auth` | `local` |
| Server → client stream | none (`GET` is 405) | none |
| Other surfaces | share the listener | none mounted |

MCP sessions are held in memory by the serving process; they do not survive a restart. Notifications from the client (`notifications/cancelled` included) are accepted and ignored. A build without the `serve` Cargo feature has no `/mcp` and no `--stdio` server (`rivet serve` exits 5, `unsupported.feature`). Platforms: macOS and Linux; Windows is not supported in 0.2.0 ([INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)). The server does not offer resources, resource templates or prompts, and does not speak the legacy HTTP+SSE MCP transport.

## Related Documents

- [API index](index.md) · [HTTP REST, SSE and polling](api-2026-0001-http-rest-sse-polling.md) · [Error registry](api-2026-0005-error-registry.md)
- [Policy and sandbox model — MCP trust boundary](../security/sec-2026-0001-policy-and-sandbox-model.md#mcp-remote-trust-boundary)
- [API-2026-0006 envelopes](api-2026-0006-envelopes.md) · [MIG-2026-0001 migration](../migrations/mig-2026-0001-response-and-input-envelopes.md) · [MAN-2026-0006 serving and surfaces](../manuals/man-2026-0006-serving-and-surfaces.md)
- [Demo 06-mcp-bridge](../demos/06-mcp-bridge/README.md) · [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) · [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial MCP server contract with exchanges captured from `rivet serve` (HTTP and stdio) at commit f40d4aa. |
| 2 | 2026-09-28 | Claude | Fix batch through 829ca43 and 2a751ab: `tools/list` lists every built-in the principal may call (20, including `rivet.trace.export` and `rivet.capabilities`); emits/receives descriptions; `restrict` on `tools/call`; `traceparent` in/out. |
| 3 | 2026-09-29 | Claude | 0.2.0 envelope sweep (D-42, TASK-070): `structuredContent` and the text block are the ResponseEnvelope (`accepted` for streaming tools; `isError` for `error` and `cancelled`); `outputSchema` is the envelope schema; `rivet.request` / `rivet.sessions.open` take `{operation, data}` (deprecated `{id, params}` with the `deprecation` header); transport refusals as envelopes; all exchanges re-captured on the 0.2.0-rc (source `6f9943f`); platform and feature notes. |
| 4 | 2026-09-29 | Claude | INC-2026-0012: `mcp.session_required` answers `operation: null`; `rivet.request` error envelopes name the requested operation (captured). |
