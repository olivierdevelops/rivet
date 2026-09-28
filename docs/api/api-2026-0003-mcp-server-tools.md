---
document_id: API-2026-0003
title: "Rivet MCP server: tools over Streamable HTTP and stdio"
document_type: api
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
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
scope: Rivet as an MCP server (protocol 2025-11-25) — `/mcp` Streamable HTTP and `rivet serve --stdio`; initialize, tools/list shape, direct named tools, built-in tools, outputSchema, session delivery `_meta`, bridge `_meta`, JSON-RPC and tool errors. Rivet as an MCP *client* (connectors) is covered by SEC-2026-0001 and the language reference.
reason: DOCUMENTATION.md §31 API impact for PLAN-2026-0001 row D-26; the MCP server surface is new in 0.1.0.
related_documents: [PLAN-2026-0001, PROP-2026-0001, API-2026-0001, API-2026-0005, SEC-2026-0001, ARCH-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, api, mcp, json-rpc, tools]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.1.0-dev (commit f40d4aa)"
---

# Rivet MCP server: tools over Streamable HTTP and stdio

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
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
            tools/list ─▶ direct tools (authorized public ops) + built-ins
            tools/call ─▶ unary op     : Completion         (isError false)
                          streaming op : SessionReceipt     (_meta rivet/delivery = session)
                          failure      : ErrorEnvelope      (isError true)
```

Examples were captured from `rivet serve --listen 127.0.0.1:18431` and `rivet serve --stdio` (commit `f40d4aa`) on [docs/demos/01-catalog](../demos/01-catalog/app.rivet), using the request files in [requests/](../demos/01-catalog/requests/initialize.mcp.json). IDs vary per run.

## Audience and Stability

MCP clients and agent hosts. The tool naming (operation ID), result shapes and `_meta` keys below are stable for 0.1.0. The server targets only protocol version 2025-11-25; resources, prompts, sampling, server-initiated streams, JSON-RPC batches and the legacy HTTP+SSE transport are not offered.

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
| `tools/list` | `{tools:[…]}` — no pagination |
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
| `outputSchema` | unary: Completion schema whose `result` is the declared `output`; streaming: SessionReceipt schema |
| `_meta` | streaming only: `{"rivet/delivery": "session"}` |

### Built-in tools listed beside them

| Tool | Arguments | Purpose |
|---|---|---|
| `rivet.request` | `{id, params?}` | Generic dispatch: Completion (unary) or SessionReceipt (streaming) |
| `rivet.list` | `{cursor?, limit?, outputs?}` | Authorized summaries |
| `rivet.describe` | `{id}` | Full descriptor |
| `rivet.outputs` | `{id?, all?}` | Output / emits / receives / errors schema |
| `rivet.sessions.open` | `{id, params?}` | Open a session → SessionReceipt |
| `rivet.sessions.send` | `{session_id, send_seq ≥ 1, data}` | One input item |
| `rivet.sessions.finish_input` | `{session_id}` | Half-close input |
| `rivet.sessions.read` | `{session_id, after_seq?, max_events?, wait_ms? ≤ 5000}` | Events after `after_seq` (acknowledges earlier ones) |
| `rivet.sessions.cancel` | `{session_id}` | Cancel and await cleanup |

Built-in `outputSchema` is the Completion schema with an open `result`. The other built-ins (`rivet.io`, `rivet.policy.generate`, `rivet.trace.show`, `rivet.connectors.sync`, `rivet.auth.begin|complete|status|disconnect|cancel`) are **callable** by name with `tools/call` but are **not listed** in `tools/list`; the sensitive four need an exact `serve.principals` listing for a network principal.

## Request Format

```json
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"demo.add","arguments":{"a":2,"b":3}}}
```

Bridge recursion state may be passed in `params._meta`: `{"rivet/hops": N, "rivet/chain": ["rivet:…", …]}`. Rivet carries it into nested MCP connector calls made while serving the tool; a call at 8 hops fails with `limit.mcp_hops`, and a call whose next hop is already in the chain fails with `limit.mcp_recursion` (both before any connector I/O).

## Response Format

Tool results always carry both a text block (the serialized JSON) and `structuredContent`:

```json
{"jsonrpc":"2.0","id":3,"result":{
  "content":[{"type":"text","text":"{\"request_id\":\"req_10052512ea\",\"trace_id\":\"tr_10052512ea\",\"result\":5,\"data_count\":0,\"effects\":\"none\"}"}],
  "structuredContent":{"request_id":"req_10052512ea","trace_id":"tr_10052512ea","result":5,"data_count":0,"effects":"none"},
  "isError":false}}
```

### Session delivery for streaming operations

A tool whose operation declares `emits` (or `receives`) is marked `_meta: {"rivet/delivery": "session"}`. Calling it **does not block** until the stream ends: it opens a principal-owned session and returns its receipt; the client then drains it with `rivet.sessions.read` (and feeds duplex input with `rivet.sessions.send` / `finish_input`).

```text
 MCP client                                   rivet
   │ tools/call demo.countdown {}              │
   │──────────────────────────────────────────▶│ open session (principal-owned)
   │◀── structuredContent: SessionReceipt ─────│ {session_id, request_id, next_send_seq, expires_at, …}
   │ tools/call rivet.sessions.read            │
   │   {session_id, after_seq:0, wait_ms:2000} │
   │──────────────────────────────────────────▶│ waits ≤ 5 s for events
   │◀── Completion{result: SessionBatch} ──────│ {events:[data…, result], last_seq, terminal}
   │    … repeat with after_seq = last_seq until terminal = true
```

Captured:

```text
tools/call demo.countdown {}
→ "structuredContent":{"session_id":"ses_02011003fa","request_id":"req_1208f924fc","trace_id":"tr_1208f924fc",
   "catalog_version":"sha256:67104f0e…9730","input_schema":null,"emits_schema":{"type":"integer"},
   "next_send_seq":1,"expires_at":"2026-09-28T05:13:52Z"},"isError":false

tools/call rivet.sessions.read {"session_id":"ses_03c84fae7f","after_seq":0,"wait_ms":2000}
→ structuredContent: {"request_id":"req_144ee8c8be","trace_id":"tr_144ee8c8be","result":{"session_id":"ses_03c84fae7f","events":[
     {"request_id":"req_13c3588ba9","trace_id":"tr_13c3588ba9","seq":1,"type":"data","data":3},
     {"request_id":"req_13c3588ba9","trace_id":"tr_13c3588ba9","seq":2,"type":"data","data":2},
     {"request_id":"req_13c3588ba9","trace_id":"tr_13c3588ba9","seq":3,"type":"data","data":1},
     {"request_id":"req_13c3588ba9","trace_id":"tr_13c3588ba9","result":{"count":3},"data_count":3,"effects":"none","type":"result","seq":4}],
   "last_seq":4,"terminal":true},"data_count":0,"effects":"none"}
```

### outputSchema of a streaming tool

```json
{"type":"object","properties":{"session_id":{"type":"string"},"request_id":{"type":"string"},"trace_id":{"type":"string"},
 "catalog_version":{"type":"string"},"input_schema":{},"emits_schema":{},"next_send_seq":{"type":"integer"},
 "expires_at":{"type":"string"}},
 "required":["session_id","request_id","catalog_version","next_send_seq","expires_at"]}
```

## Error Format

Two layers, never mixed:

| Failure | Shape | Example |
|---|---|---|
| Protocol problem | JSON-RPC `error` | `-32700` parse error, `-32600` not one 2.0 object / batch, `-32601` unknown method, `-32602` unknown tool or missing `name` |
| The operation failed | `result` with `isError: true`; `structuredContent` = ErrorEnvelope `{request_id, trace_id, error}` | validation, permission, runtime, timeout … |
| Transport-level refusal (HTTP) | Rivet ErrorEnvelope with registry HTTP status | 401, 403 Origin, 404 session, 422 version/session header |

Captured:

```text
tools/call demo.add {"b":1}
→ {"jsonrpc":"2.0","id":5,"result":{"content":[{"type":"text","text":"{…}"}],"structuredContent":{"request_id":"req_118695f517","trace_id":"tr_118695f517",
   "error":{"kind":"validation","code":"validation.required","message":"missing required parameter `a`","retryable":false,"effects":"none","operation_id":"demo.add","details":{"field":"a"}}},"isError":true}}

tools/call nope.x          → {"jsonrpc":"2.0","id":7,"error":{"code":-32602,"message":"Unknown tool: nope.x"}}
resources/list             → {"jsonrpc":"2.0","id":8,"error":{"code":-32601,"message":"method not found: resources/list"}}
body [1]                   → {"jsonrpc":"2.0","id":null,"error":{"code":-32600,"message":"expected one JSON-RPC message object (batches are not supported)"}}
body x                     → {"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"parse error: expected value at line 1 column 1"}}
POST without session id    → 422 {"request_id":"","trace_id":"","error":{"kind":"validation","code":"mcp.session_required","message":"MCP-Session-Id header is required after initialize",…}}
MCP-Protocol-Version: 2024-11-05 → 422 {…"code":"mcp.protocol_version","message":"unsupported MCP-Protocol-Version 2024-11-05; this server speaks 2025-11-25",…}
Origin: https://evil.example → 403 {…"code":"permission.denied","message":"Origin is not allowed for /mcp",…}
GET /mcp                   → 405, Allow: POST, DELETE
DELETE /mcp (live session) → 204;  again → 404 not_found.mcp_session
```

## Rate Limits

No MCP-specific rate limit. Tool calls share the host budgets: `limits.max_concurrent_requests` (`limit.concurrency`), 8 sessions per principal (`limit.sessions`), and the request deadline (default 30 s).

## Versioning and Deprecation

`protocolVersion` in `initialize` is always `2025-11-25`; a request header naming another version is refused. Nothing is deprecated in 0.1.0.

## Examples

HTTP, with the demo request files:

```text
$ cd docs/demos/01-catalog/requests
$ curl -s -i -X POST http://127.0.0.1:18431/mcp -H 'content-type: application/json' -d @initialize.mcp.json
HTTP/1.1 200 OK
content-type: application/json
mcp-session-id: mcp_1b9c7c55f832a92a5

{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"rivet","version":"0.1.0-dev"}}}

$ curl -s -i -X POST http://127.0.0.1:18431/mcp -H 'mcp-session-id: mcp_1b9c7c55f832a92a5' -d @initialized.mcp.json
HTTP/1.1 202 Accepted

$ curl -s -X POST http://127.0.0.1:18431/mcp -H 'mcp-session-id: mcp_1b9c7c55f832a92a5' \
       -H 'mcp-protocol-version: 2025-11-25' -d @list.mcp.json
# tool names, in order:
demo.greet, demo.add, demo.health, demo.countdown,
rivet.request, rivet.list, rivet.describe, rivet.outputs,
rivet.sessions.open, rivet.sessions.send, rivet.sessions.finish_input, rivet.sessions.read, rivet.sessions.cancel
```

The `demo.add` descriptor from that list:

```json
{"name":"demo.add","title":"Add two integers",
 "inputSchema":{"type":"object","properties":{"a":{"type":"integer","description":"First operand."},
   "b":{"type":"integer","description":"Second operand; defaults to zero.","default":0}},"required":["a"],"additionalProperties":false},
 "description":"Add two signed integers and return their sum.",
 "outputSchema":{"type":"object","properties":{"request_id":{"type":"string"},"trace_id":{"type":"string"},
   "result":{"type":"integer","description":"Sum of a and b."},"data_count":{"type":"integer","minimum":0},
   "effects":{"type":"string","enum":["none","committed","partial","unknown"]}},
   "required":["request_id","trace_id","result","data_count","effects"]}}
```

stdio (three lines in, two replies out — the notification gets none):

```text
$ printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"rivet-demo","version":"0.1"}}}' \
    '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
    '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"demo.add","arguments":{"a":2,"b":3}}}' \
  | rivet --file app.rivet serve --stdio
{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"rivet","version":"0.1.0-dev"}}}
{"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"{\"request_id\":\"req_018287d0ad\",…}"}],"structuredContent":{"request_id":"req_018287d0ad","trace_id":"tr_018287d0ad","result":5,"data_count":0,"effects":"none"},"isError":false}}
# stderr:
{"listen_addr":null,"stdio":true,"surfaces":["mcp"],"auth_type":"none","catalog_version":"sha256:67104f0e…9730","policy_hash":null}
```

## Compatibility Notes

| Aspect | HTTP `/mcp` | stdio |
|---|---|---|
| Framing | one JSON-RPC object per POST body, JSON response | one object per line |
| MCP session | `MCP-Session-Id` from `initialize`, required afterwards, bound to the principal, ended by `DELETE` | none (the process is the session) |
| Principal | from `serve.auth` | `local` |
| Server → client stream | none (`GET` is 405) | none |
| Other surfaces | share the listener | none mounted |

MCP sessions are held in memory by the serving process; they do not survive a restart. Notifications from the client (`notifications/cancelled` included) are accepted and ignored in 0.1.0.

## Related Documents

- [API index](index.md) · [HTTP REST, SSE and polling](api-2026-0001-http-rest-sse-polling.md) · [Error registry](api-2026-0005-error-registry.md)
- [Policy and sandbox model — MCP trust boundary](../security/sec-2026-0001-policy-and-sandbox-model.md#mcp-remote-trust-boundary)
- [Demo 06-mcp-bridge](../demos/06-mcp-bridge/README.md) · [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial MCP server contract with exchanges captured from `rivet serve` (HTTP and stdio) at commit f40d4aa. |
