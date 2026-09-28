---
document_id: MAN-2026-0006
title: "Rivet serving and surfaces"
document_type: manual
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [serve, http, poll, ws, mcp, sessions, auth, policy, cli]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, server]
audience: [operators, integrators, developers]
scope: Running `rivet serve`, authenticating callers (none, bearer), per-principal operation lists and surfaces, and task workflows for REST, SSE, polling sessions, WebSocket rivet.v1, MCP (Streamable HTTP and stdio), built-in rivet.* operations and the --endpoint CLI client.
reason: PLAN-2026-0001 row D-39 — operator guide for the implemented serve listener; every request and response captured from a live 0.1.0-dev server on 127.0.0.1.
related_documents: [MAN-2026-0001, MAN-2026-0004, MAN-2026-0005, API-2026-0001, API-2026-0002, API-2026-0003, OPS-2026-0001, RUN-2026-0001, SYS-2026-0004, DEMO-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, manual, serve, rest, sse, polling, websocket, mcp, auth]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.1.0-dev (commit f40d4aa)"
---

# Rivet serving and surfaces

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** serve, http, poll, ws, mcp, sessions, auth, policy, cli

## Purpose

One `rivet serve` process exposes the bundle's operations on one listener through five surfaces. This volume shows
how to start it, secure it, and use each surface, with captured requests and responses. Exact wire contracts are
in [API-2026-0001](../api/api-2026-0001-http-rest-sse-polling.md) (REST, SSE, polling),
[API-2026-0002](../api/api-2026-0002-websocket-rivet-v1.md) (WebSocket) and
[API-2026-0003](../api/api-2026-0003-mcp-server-tools.md) (MCP); deployment is in
[OPS-2026-0001](../operations/ops-2026-0001-operating-rivet-serve.md).

## Reading Order

```text
 start ─► auth + principals + surfaces ─► REST ─► SSE ─► polling ─► WebSocket ─► MCP ─► built-ins ─► --endpoint
```

## Concepts

```text
                        rivet serve --file app.rivet --listen 127.0.0.1:8080
                                             │ one TCP listener
     ┌───────────────┬───────────────┬───────┴────────┬───────────────┬──────────────────┐
     │ http (REST)   │ sse           │ poll           │ ws            │ mcp              │
     │ POST /v1/request              │ POST /v1/requests               │ POST|DELETE /mcp │
     │ GET /v1/operations[/{id}[/outputs]]  GET …/events │ GET /v1/ws    │ (GET → 405)      │
     │ GET /v1/io    │ same route +  │ POST …/input   │ subprotocol   │ Streamable HTTP  │
     │ POST /v1/policy/generate      │ Accept: text/ │ …/finish_input│ rivet.v1       │ protocol         │
     │               │ event-stream  │ …/cancel       │ ≤ 8 refs      │ 2025-11-25       │
     └───────────────┴───────────────┴───────┬────────┴───────────────┴──────────────────┘
                                             │ authenticate (serve.auth) → principal
                                             │ authorize (serve.principals) → 403 if unlisted
                                             ▼
                                   shared dispatcher → policy broker → effects
```

| Setting (`policy.json` → `serve`) | Default | Effect |
|---|---|---|
| `surfaces` | all five | mount only these (`http`, `sse`, `poll`, `ws`, `mcp`); others answer `404 not_found.route` |
| `auth.type` | `none` | `none` (loopback only), `bearer` (SHA-256 token hashes), `mtls` (refuses to start in 0.1.0) |
| `principals` | unrestricted | `{NAME: {operations: [patterns]}}`; unlisted operation → `403 permission.denied` |

## Task-Oriented Workflows

Captured from `docs/demos/01-catalog/` (and a scratch bundle for live input). Response headers `date` and
`content-length` are omitted; IDs vary per run.

### Start a server

```bash
rivet serve --file app.rivet --listen 127.0.0.1:18401
```

stderr receipt:

```text
{"listen_addr":"127.0.0.1:18401","stdio":false,"surfaces":["http","sse","poll","ws","mcp"],"auth_type":"none","catalog_version":"sha256:67104f0e7faaeafee253db758a668a9b24aa4263677e9d5ea2451b32019f9730","policy_hash":null}
```

```text
 state:  compile bundle ─► load policy ─► validate --listen / auth ─► bind ─► receipt ─► SERVING ─► SIGINT ─► exit 0
                │ error exit 2     │ policy.invalid exit 2  │ serve.auth_required exit 2
                                                          │ unsupported.serve_mtls exit 5
```

| Failure | Output | Exit |
|---|---|---|
| `--listen 0.0.0.0:18404` with no `serve.auth` | `serve.auth_required` "non-loopback listener requires serve.auth in policy.json" | 2 |
| `--listen nothost` | `validation.usage` "--listen nothost: use HOST:PORT with an IP address or localhost" | 2 |
| `serve.auth` `{"type":"mtls",…}` | `unsupported.serve_mtls` "serve.auth type mtls needs a TLS listener, which this build does not provide yet; use bearer behind a TLS-terminating proxy" | 5 |
| `mtls` without `client_ca` | `policy.invalid` "/serve/auth/client_ca: is required for mtls" | 2 |

### Authenticate callers and authorize operations

Bearer tokens are configured as SHA-256 hex hashes; the server never stores the token itself.

```bash
printf %s 'dev-token-ada' | shasum -a 256      # Linux: sha256sum
# bf7e9889975e8d9483fed456e9651d9c73470c82857bc3e6384222a1d5f7f9c6
```

`docs/demos/01-catalog/policies/team.json`:

```json
{
  "version": 1,
  "limits": {"max_concurrent_requests": 64, "max_call_depth": 16},
  "serve": {
    "surfaces": ["http", "sse", "poll", "mcp"],
    "auth": {
      "type": "bearer",
      "tokens": [
        {"principal": "ada", "sha256": "bf7e9889975e8d9483fed456e9651d9c73470c82857bc3e6384222a1d5f7f9c6"},
        {"principal": "ci",  "sha256": "a6c6871b8f3568d985f17c7458aff2582992a5007442b589ed5af28854bb8501"}
      ]
    },
    "principals": {
      "ada": {"operations": ["demo.*"]},
      "ci":  {"operations": ["demo.health"]}
    }
  }
}
```

```bash
rivet serve --file app.rivet --policy policies/team.json --listen 127.0.0.1:18406
```

```text
 client ──Authorization: Bearer T──► sha256(T) in tokens? ── no header ─► 401 auth.required
                                            │ no match ──────────────────► 401 auth.invalid
                                            ▼ principal
                              operation matches principals[p]? ── no ───► 403 permission.denied
                                            ▼ yes
                                        dispatch
```

| Request | Status | Body (abridged) |
|---|---|---|
| `GET /v1/operations` (no header) | 401, `www-authenticate: Bearer` | `auth.required` "missing bearer token" |
| `… -H 'authorization: Bearer wrong'` | 401 | `auth.invalid` "invalid bearer token" |
| `… Bearer dev-token-ci` `GET /v1/operations` | 200 | only `demo.health` is listed |
| `… Bearer dev-token-ci` `POST /v1/request {"id":"demo.add",…}` | 403 | `permission.denied` "principal \`ci\` may not call \`demo.add\`" |
| `… Bearer dev-token-ada` `POST /v1/request {"id":"demo.add","params":{"a":1}}` | 200 | `"result":1` |
| `… Bearer dev-token-ada` `GET /v1/io` | 403 | "principal \`ada\` may not call \`rivet.io\`" |
| WebSocket upgrade (ws not in `surfaces`) | 404 | `not_found.route` |

`rivet.io`, `rivet.policy.generate`, `rivet.trace.show` and `rivet.connectors.sync` need the local principal or
an **exact** listing: a principal with `["*"]` still gets 403 on `/v1/io`; `["demo.health", "rivet.io"]` gets
200 on `/v1/io` and 403 on `/v1/policy/generate`. With `auth` `none` the caller is the local principal.

Rotating tokens: [RUN-2026-0001](../runbooks/run-2026-0001-rotate-serve-bearer-tokens.md).

### Call over REST

```bash
B=http://127.0.0.1:18401
curl -sS $B/v1/operations
curl -sS $B/v1/operations/demo.add
curl -sS $B/v1/operations/demo.add/outputs
curl -sS -X POST $B/v1/request -H 'content-type: application/json' \
  -d '{"id":"demo.add","params":{"a":2,"b":3}}'
```

```text
HTTP/1.1 200 OK
{"operations":[{"id":"demo.greet",…,"streaming":false},…,{"id":"demo.countdown",…,"streaming":true}],"next_cursor":null}
HTTP/1.1 200 OK
{"id":"demo.add","name":"Add two integers",…,"input":{"type":"object","properties":{"a":{"type":"integer",…},"b":{…,"default":0}},"required":["a"],"additionalProperties":false},"output":{"type":"integer","description":"Sum of a and b."},"emits":null,"receives":null,"errors":[],"delivery":"unary","source":{"file":"app.rivet","line":9}}
HTTP/1.1 200 OK
{"id":"demo.add","output":{"type":"integer","description":"Sum of a and b."},"emits":null,"receives":null,"errors":[]}
HTTP/1.1 200 OK
{"request_id":"req_010c731ded","trace_id":"tr_010c731ded","result":5,"data_count":0,"effects":"none"}
```

`/v1/request` body: `{"id": "…", "params": {…}, "deadline_ms": N?}` (`deadline_ms` optional, capped at 600000;
default 30000).

| Failure | Status | Code |
|---|---|---|
| `{"id":"demo.add","params":{"a":"x"}}` | 422 | `validation.type` |
| body `nope` | 400 | `validation.malformed_json` |
| `{"id":"demo.nope"}` | 404 | `not_found.operation` |
| streaming operation without `Accept: text/event-stream` | 422 | `stream.required` "use Accept: text/event-stream, POST /v1/requests, /v1/ws or rivet.sessions.open" |
| `{"id":"lang.slow","deadline_ms":100}` | 504 | `timeout.request` |
| any unknown path | 404 | `not_found.route` |

### Stream over SSE

```bash
curl -sS -N -X POST $B/v1/request -H 'content-type: application/json' \
  -H 'accept: text/event-stream' -d '{"id":"demo.countdown","params":{}}'
```

```text
HTTP/1.1 200 OK
content-type: text/event-stream
cache-control: no-cache

id: 1
event: data
data: {"request_id":"req_030925657f","trace_id":"tr_030925657f","seq":1,"type":"data","data":3}

id: 2
event: data
data: {"request_id":"req_030925657f","trace_id":"tr_030925657f","seq":2,"type":"data","data":2}

id: 3
event: data
data: {"request_id":"req_030925657f","trace_id":"tr_030925657f","seq":3,"type":"data","data":1}

id: 4
event: result
data: {"request_id":"req_030925657f","trace_id":"tr_030925657f","result":{"count":3},"data_count":3,"effects":"none","type":"result","seq":4}
```

A failure after the stream started arrives as a terminal `event: error` carrying the error envelope.

### Poll a session

```text
 POST /v1/requests {id, params} ─► 202 {session_id, events_url, next_send_seq, expires_at, …}
      │
      ├─ POST …/input {send_seq, data}   (receives operations; send_seq starts at 1)
      ├─ POST …/finish_input             (half-close input)
      ├─ GET  …/events?after_seq=N&wait_ms=M   (long-poll ≤ 5000 ms; acknowledges ≤ N)
      │        ─► {events:[…], last_seq, terminal}
      └─ POST …/cancel ─► {state:"cancelled"}
```

```bash
curl -sS -X POST $B/v1/requests -d '{"id":"demo.countdown","params":{}}'
curl -sS "$B/v1/requests/ses_013c71b16d/events?after_seq=0&wait_ms=1000"
```

```text
HTTP/1.1 202 Accepted
{"session_id":"ses_013c71b16d","request_id":"req_013c71c1cd","trace_id":"tr_013c71c1cd","catalog_version":"sha256:6710…","input_schema":null,"emits_schema":{"type":"integer"},"next_send_seq":1,"expires_at":"2026-09-28T05:12:02Z","events_url":"/v1/requests/ses_013c71b16d/events"}
HTTP/1.1 200 OK
{"session_id":"ses_013c71b16d","events":[{…"seq":1,"type":"data","data":3},{…"seq":2,"data":2},{…"seq":3,"data":1},{…"result":{"count":3},"data_count":3,"effects":"none","type":"result","seq":4}],"last_seq":4,"terminal":true}
```

Live input to a `receives` operation:

```text
POST …/input {"send_seq":1,"data":{"a":1}}   → 200 {"session_id":"ses_01f4f231cd","accepted_seq":1,"input_closed":false}
POST …/finish_input                          → 200 {"session_id":"ses_01f4f231cd","accepted_seq":null,"input_closed":true}
GET  …/events?after_seq=0&wait_ms=2000       → 200 {"events":[{…"type":"data","data":{"a":1}},{…"result":{"received":1},…}],"last_seq":2,"terminal":true}
```

Resending the same `send_seq` returns the same acknowledgement (idempotent). Unknown session:
`404 not_found.session` "no session \`ses_bogus\`". Sessions are owned by the principal that opened them.

### Multiplex over WebSocket

Connect to `GET /v1/ws` offering subprotocol `rivet.v1` (bearer auth on the upgrade request). Each client frame
names a `ref`; every ref ends with exactly one `result` or `error` frame; at most 8 refs may be in flight.

```text
 client                                            server
   │ {"type":"request","ref":"c1","id":"demo.add","params":{"a":2,"b":3}} ─►
   │ {"type":"request","ref":"c2","id":"demo.countdown","params":{}}      ─►
   │ ◄─ {"type":"result","ref":"c1","completion":{…"result":5…}}
   │ ◄─ {"type":"data","ref":"c2",…,"seq":1,"data":3}   … seq 2, seq 3
   │ ◄─ {"type":"result","ref":"c2","completion":{…"result":{"count":3}…}}
   │ {"type":"input","ref":"r1","seq":1,"data":"x"} / {"type":"finish_input","ref":"r1"} / {"type":"cancel","ref":"r2"}
```

Captured frames (`docs/demos/01-catalog/requests/ws-frames.jsonl` sent over one connection):

```text
< {"type":"error","ref":"c3","error":{"kind":"validation","code":"validation.required","message":"missing required parameter `a`",…}}
< {"type":"result","ref":"c1","completion":{"request_id":"req_0276cfb472","trace_id":"tr_0276cfb472","result":5,"data_count":0,"effects":"none"}}
< {"type":"data","ref":"c2","request_id":"req_03f581a2a7","trace_id":"tr_03f581a2a7","seq":1,"data":3}
< {"type":"data","ref":"c2","request_id":"req_03f581a2a7","trace_id":"tr_03f581a2a7","seq":2,"data":2}
< {"type":"data","ref":"c2","request_id":"req_03f581a2a7","trace_id":"tr_03f581a2a7","seq":3,"data":1}
< {"type":"result","ref":"c2","completion":{"request_id":"req_03f581a2a7","trace_id":"tr_03f581a2a7","result":{"count":3},"data_count":3,"effects":"none"}}
```

| Misuse | Result |
|---|---|
| upgrade without `Sec-WebSocket-Protocol: rivet.v1` | HTTP 422 `validation.subprotocol` |
| `input`/`finish_input`/`cancel` for an unknown ref | `{"type":"error","ref":"zz","error":{"code":"not_found.ref",…}}` |
| reusing a ref that is in flight | `conflict.ref` "ref \`r2\` is already in flight" |
| a ninth concurrent ref | `limit.ws_refs` error frame |
| `cancel` on a running ref | `{"type":"error","ref":"r2",…,"error":{"kind":"cancelled","code":"cancelled.session",…}}` |

Closing the socket cancels every in-flight ref and waits for cleanup.

### Expose tools over MCP

`/mcp` implements MCP Streamable HTTP (protocol `2025-11-25`). Every public operation is a tool named by its ID;
the `rivet.*` built-ins are tools too.

```text
 POST /mcp initialize ─► 200 + header mcp-session-id: mcp_…
 POST /mcp notifications/initialized (with mcp-session-id) ─► 202
 POST /mcp tools/list | tools/call (with mcp-session-id) ─► 200 JSON-RPC result
 DELETE /mcp (with mcp-session-id) ─► 204   ·   GET /mcp ─► 405 (allow: POST, DELETE)
```

```bash
H=(-H 'content-type: application/json' -H 'accept: application/json, text/event-stream')
curl -sS -i "${H[@]}" -X POST $B/mcp -d @requests/initialize.mcp.json
curl -sS "${H[@]}" -H 'mcp-session-id: mcp_1c380c9408750a025' -X POST $B/mcp -d @requests/add.mcp.json
```

```text
HTTP/1.1 200 OK
mcp-session-id: mcp_1c380c9408750a025
{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"rivet","version":"0.1.0-dev"}}}

{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"{\"request_id\":\"req_017deba705\",…,\"result\":5,…}"}],"structuredContent":{"request_id":"req_017deba705","trace_id":"tr_017deba705","result":5,"data_count":0,"effects":"none"},"isError":false}}
```

- Unary tools return the Completion in `structuredContent`; streaming operations (`_meta: {"rivet/delivery":
  "session"}`) return a session receipt — read events with `rivet.sessions.read`.
- A failed call is a tool result with `"isError": true` and the error envelope in `structuredContent`.
- Missing session header after initialize: 422 `mcp.session_required`; after `DELETE`: 404
  `not_found.mcp_session`.
- `resources/list` and prompts are not served (`-32601 method not found`).

**MCP over stdio** (for agent hosts that spawn a process):

```bash
rivet serve --file app.rivet --stdio
```

The receipt goes to stderr; stdout carries only newline-delimited JSON-RPC. `--stdio` mounts MCP only and ignores
`--listen`.

### Built-in operations

Callable through every surface (`POST /v1/request {"id":"rivet.list"}`, MCP tools, CLI `request rivet.list`):

| Built-in | Purpose | Principal requirement |
|---|---|---|
| `rivet.request` | generic dispatch `{id, params}` | the target must be allowed |
| `rivet.list`, `rivet.describe`, `rivet.outputs` | catalog discovery | normal pattern matching |
| `rivet.sessions.open/send/finish_input/read/cancel` | session control (`read` waits ≤ 5000 ms) | normal pattern matching |
| `rivet.auth.begin/complete/status/disconnect/cancel` | OAuth accounts (MAN-2026-0008) | normal pattern matching + `allow_auth` grants |
| `rivet.io`, `rivet.policy.generate`, `rivet.trace.show`, `rivet.connectors.sync {name, output}` | inspection and admin | local principal or an **exact** listing |

### Use the CLI against a server

```text
 rivet --endpoint URL --token-file FILE  request | list | describe | outputs | io | trace | auth
        (not with --file/--policy)          check | policy | serve  → refused (exit 2)
```

```bash
printf %s 'dev-token-ada' > ada.token && chmod 600 ada.token
rivet --endpoint http://127.0.0.1:18406 --token-file ada.token request demo.add --params '{"a":4,"b":5}'
rivet --endpoint http://127.0.0.1:18406 --token-file ada.token request demo.countdown --stream
```

```text
{"request_id":"req_046718b54c","trace_id":"tr_046718b54c","result":9,"data_count":0,"effects":"none"}
{"request_id":"req_06674cfab6","trace_id":"tr_06674cfab6","seq":1,"type":"data","data":3}
…
{"request_id":"req_06674cfab6","trace_id":"tr_06674cfab6","result":{"count":3},"data_count":3,"effects":"none","type":"result"}
```

| Failure | Output | Exit |
|---|---|---|
| no `--token-file` against a bearer server | `error[auth.required]: missing bearer token` | 3 |
| `ci` token calling `demo.add` | `permission.denied` "principal \`ci\` may not call \`demo.add\`" | 3 |
| `--endpoint … check` | "check, policy and serve work on a local bundle (--file); they are not available with --endpoint" | 2 |
| `--endpoint … --file app.rivet list` | "--endpoint cannot be combined with --file or --policy: the server owns the bundle and its policy" | 2 |
| `--endpoint … io --check-files` | `validation.check_files_remote` | 2 |

Outputs and exit codes match local mode, apart from generated IDs.

### Expected Result and Side Effects

Serving binds one TCP port. Sessions, MCP sessions, OAuth transactions and traces live in memory and are lost on
restart. Stopping with SIGINT exits 0 and cancels in-flight work.

### Verified Demo

[01-catalog (DEMO-2026-0001)](../demos/01-catalog/README.md) with its `requests/` files and
`policies/team.json`; live-input examples used a scratch bundle containing `lang.echo_input` (MAN-2026-0003).
Verified with `rivet 0.1.0-dev` (commit `f40d4aa`) on `127.0.0.1:184xx`.

## Complete API and Event Reference

| Method / Route / Event | Purpose | Auth | Request | Success Response | Errors / Status | Idempotency / Side Effects | Example | Since |
|---|---|---|---|---|---|---|---|---|
| `POST /v1/request` | invoke | bearer if configured | `{id, params, deadline_ms?}` | 200 Completion; SSE with `Accept: text/event-stream` | 400/401/403/404/422/5xx | runs the operation | above | 0.1.0 |
| `GET /v1/operations` | list | same | — | 200 `{operations, next_cursor}` | 401 | none | above | 0.1.0 |
| `GET /v1/operations/{id}` | describe | same | — | 200 descriptor | 404 | none | above | 0.1.0 |
| `GET /v1/operations/{id}/outputs` | declared outputs | same | — | 200 `{id, output, emits, receives, errors}` | 404 | none | above | 0.1.0 |
| `GET /v1/io?by=…&check_policy=…&needs=…` | I/O manifest | exact `rivet.io` | query | 200 manifest JSON | 403 | none | MAN-0005 | 0.1.0 |
| `POST /v1/policy/generate` | policy draft | exact listing | `{ids, all}` | 200 `{policy, review, complete}` | 403 | never writes | MAN-0005 | 0.1.0 |
| `POST /v1/requests` | open session | same | `{id, params}` | 202 receipt | 403/404/422 | starts the request | above | 0.1.0 |
| `GET /v1/requests/{id}/events` | read events | owner | `after_seq`, `wait_ms` | 200 batch | 404 | acknowledges ≤ after_seq | above | 0.1.0 |
| `POST /v1/requests/{id}/input` / `finish_input` / `cancel` | drive session | owner | `{send_seq, data}` | 200 | 404 | same seq is idempotent | above | 0.1.0 |
| `GET /v1/ws` | WebSocket | on upgrade | frames | 101 | 422 subprotocol | per-ref sessions | above | 0.1.0 |
| `POST`/`DELETE /mcp` | MCP | same | JSON-RPC | 200/202/204 | 404/405/422 | MCP session state | above | 0.1.0 |

## Errors and Recovery Reference

| Error / Code / Message | Surface | Cause | User-Visible Result | Recovery | Retry Safe | Related Feature |
|---|---|---|---|---|---|---|
| `serve.auth_required` | startup | non-loopback + auth none | exit 2 | configure bearer | no | auth |
| `unsupported.serve_mtls` | startup | mtls configured | exit 5 | bearer + TLS proxy | no | auth |
| `auth.required` / `auth.invalid` | all | missing/unknown token | 401 | fix token | no | auth |
| `permission.denied` | all | principal not allowed | 403 | add to `principals` | no | principals |
| `not_found.route` | all | unknown path or disabled surface | 404 | enable in `serve.surfaces` | no | surfaces |
| `stream.required` | REST | streaming op without SSE | 422 | use SSE/polling/WS | no | streams |
| `validation.subprotocol` | WS | missing `rivet.v1` | 422 | offer `rivet.v1` | no | WS |
| `mcp.session_required` / `not_found.mcp_session` | MCP | missing/expired session | 422 / 404 | re-initialize | yes | MCP |
| `not_found.session`, `not_found.ref`, `conflict.ref`, `limit.ws_refs` | polling, WS | session/ref misuse | 404/error frames | fix IDs; wait for refs | depends | sessions |

## Limitations

- No mTLS and no TLS listener in 0.1.0: terminate TLS in a proxy and use bearer auth.
- `--timeout` is not applied over the WebSocket duplex path; deadlines inside the operation still apply.
- MCP server exposes tools only (no resources, resource templates or prompts); no legacy HTTP+SSE MCP transport.
- Sessions, traces and OAuth transactions are in memory only.

## Version Applicability

| Feature / Interface | Introduced | Changed | Deprecated / Removed | Applicable Environment |
|---|---|---|---|---|
| `rivet serve`, REST/SSE/polling/WS/MCP, bearer auth, principals, surfaces | 0.1.0 | — | — | server, development |
| `--endpoint` client | 0.1.0 | — | — | any |

## Related Documents

- [Rivet manual](man-2026-0001-rivet-manual.md) · [CLI reference](man-2026-0004-cli-reference.md) ·
  [Policy guide](man-2026-0005-policy-and-io-manifest-guide.md)
- [API-2026-0001](../api/api-2026-0001-http-rest-sse-polling.md) · [API-2026-0002](../api/api-2026-0002-websocket-rivet-v1.md) ·
  [API-2026-0003](../api/api-2026-0003-mcp-server-tools.md)
- [OPS-2026-0001](../operations/ops-2026-0001-operating-rivet-serve.md) ·
  [RUN-2026-0001](../runbooks/run-2026-0001-rotate-serve-bearer-tokens.md) ·
  [SYS-2026-0004](../system/components/sys-2026-0004-surfaces-and-serve.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial serving and surfaces guide for 0.1.0, captured from a live 0.1.0-dev (commit f40d4aa) server. |
