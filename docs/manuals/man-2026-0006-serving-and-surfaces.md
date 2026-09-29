---
document_id: MAN-2026-0006
title: "Rivet serving and surfaces"
document_type: manual
status: active
created_date: 2026-09-28
last_updated: 2026-09-30
document_revision: 6
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
scope: Running `rivet serve`, authenticating callers (none, bearer), per-principal operation lists and surfaces, the 0.2.0 envelopes on every surface, `?pretty=true`, deprecation monitoring (`deprecation` header, `deprecated=1` in the access log), and task workflows for REST, SSE, polling sessions, WebSocket rivet.v1, MCP (Streamable HTTP and stdio), built-in rivet.* operations and the --endpoint CLI client.
reason: PLAN-2026-0001 row D-39 and PLAN-2026-0002 rows D-25, D-46 — operator guide for the implemented serve listener; every request and response captured from a live server on 127.0.0.1.
related_documents: [PLAN-2026-0002, API-2026-0006, MIG-2026-0001, MAN-2026-0001, MAN-2026-0004, MAN-2026-0005, API-2026-0001, API-2026-0002, API-2026-0003, OPS-2026-0001, RUN-2026-0001, SYS-2026-0004, DEMO-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, manual, serve, rest, sse, polling, websocket, mcp, auth, envelope, deprecation]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.2.0-rc (source at 6f9943f)"
next_review_date: 2026-10-29
---

# Rivet serving and surfaces

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-30
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** serve, http, poll, ws, mcp, sessions, auth, policy, cli

## Purpose

One `rivet serve` process exposes the bundle's operations on one listener through five surfaces. This volume shows
how to start it, secure it, and use each surface, with captured requests and responses. Exact wire contracts are
in [API-2026-0001](../api/api-2026-0001-http-rest-sse-polling.md) (REST, SSE, polling),
[API-2026-0002](../api/api-2026-0002-websocket-rivet-v1.md) (WebSocket) and
[API-2026-0003](../api/api-2026-0003-mcp-server-tools.md) (MCP); the shared shapes are
[API-2026-0006](../api/api-2026-0006-envelopes.md); deployment is in
[OPS-2026-0001](../operations/ops-2026-0001-operating-rivet-serve.md). The current release is **0.2.0**,
which changed every body to an envelope — 0.1.0 clients read
[MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md).

## Reading Order

```text
 envelopes on every surface ─► deprecation monitoring ─► start ─► health, access log, shutdown
   ─► auth + principals + surfaces ─► REST (restrict, traceparent, pretty) ─► SSE ─► polling ─► WebSocket ─► MCP
   ─► built-ins ─► --endpoint
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
                          shared dispatcher → policy broker (∩ restrict) → effects
      GET /v1/health (always mounted)   ·   one access-log line per request on stderr
```

| Setting (`policy.json` → `serve`) | Default | Effect |
|---|---|---|
| `surfaces` | all five | mount only these (`http`, `sse`, `poll`, `ws`, `mcp`); others answer `404 not_found.route` |
| `auth.type` | `none` | `none` (loopback only), `bearer` (SHA-256 token hashes), `mtls` (refuses to start: `unsupported.serve_mtls`, exit 5) |
| `principals` | unrestricted | `{NAME: {operations: [patterns]}}`; unlisted operation → `403 permission.denied` |

## Task-Oriented Workflows

Captured on 2026-09-29 (macOS 26.4) from the 0.2.0 release candidate (`cargo build --release --features cli`,
source `6f9943f`): `docs/demos/01-catalog/` on `127.0.0.1:18951` (auth `none`), the same bundle with
`policies/team.json` on `18955` (bearer), and a scratch bundle on `18950`/`18952` (`demo.read` reads a file under
`./data` with `allow_read ./data/**`; `chat.echo` echoes live input and returns `{echoed: N}`; `demo.flaky` emits
`1`, then fails). Response headers `date` and `content-length` are omitted. **Request, trace, session and MCP
session IDs, span IDs, `expires_at` and `time` differ on every run.** `version` reads `0.2.0`.

### Envelopes on every surface

From 0.2.0 **every surface answers with the same response envelope** and accepts the same input envelope
([API-2026-0006](../api/api-2026-0006-envelopes.md)). What differs is only the wrapping:

```text
                        input {operation, data, deadline_ms?, restrict?}
                                           │
   POST /v1/request ── POST /v1/requests ── WS {type:"request",ref,…} ── MCP tools/call / rivet.request ── rivet --endpoint
                                           │
                       {request_id, trace_id, operation, type, status, data, error, effects, data_count}
                                           │
 ┌───────────────┬──────────────────┬──────┴───────────────┬─────────────────────┬─────────────────────────────┐
 │ REST          │ SSE              │ polling              │ WebSocket           │ MCP                         │
 │ body = the    │ event: data|result│ 202 status:"accepted"│ each frame = a      │ structuredContent = envelope │
 │ envelope;     │ data: one record │ data = receipt;      │ record with "ref"   │ content[0].text = same JSON │
 │ HTTP status = │ per event; errors│ …/events: {events:   │ first; one terminal │ isError: status error or    │
 │ registry      │ = event: result  │ [records], last_seq, │ type:"result" per   │ cancelled                   │
 │ (200/4xx/5xx) │ + status "error" │ terminal}            │ ref                 │                             │
 └───────────────┴──────────────────┴──────────────────────┴─────────────────────┴─────────────────────────────┘
```

| Surface | Success | Failure | Stream | Pretty |
|---|---|---|---|---|
| REST `/v1/request` | 200 envelope `status: "ok"` | registry status, envelope `status: "error"` | — | `?pretty=true` |
| SSE | `event: result`, `status: "ok"` | before the first item: a plain JSON envelope with its status; after: `event: result` with `status: "error"` | `event: data` records | refused: `400 validation.pretty_stream` |
| Polling | `202` envelope `status: "accepted"` (receipt in `data`) | envelope with the registry status | `…/events` batch of records | `?pretty=true` on the envelope routes |
| WebSocket | terminal record `status: "ok"` | terminal record `status: "error"`/`"cancelled"`; refused frames are error records | `type: "data"` records | — |
| MCP | `isError: false`, envelope `status: "ok"` (or `"accepted"` for a streaming tool) | `isError: true`, envelope `status: "error"`/`"cancelled"` | via `rivet.sessions.read` | — |
| `rivet --endpoint` | stdout envelope, exit 0 | stderr envelope, registry exit code | NDJSON records | `--pretty` |

**Pretty output.** Add `?pretty=true` to any JSON route to indent the answer (2 spaces, same key order); it is refused
with `Accept: text/event-stream` because each SSE event must stay on one line:

```text
$ curl -s 'http://127.0.0.1:18952/v1/request?pretty=true' -H 'content-type: application/json' -d '{"operation":"demo.add","data":{"a":2,"b":3}}'
{
  "request_id": "req_08121261a8",
  "trace_id": "tr_08121261a8",
  "operation": "demo.add",
  "type": "result",
  "status": "ok",
  "data": 5,
  "error": null,
  "effects": "none",
  "data_count": 0
}

$ curl -s -i 'http://127.0.0.1:18951/v1/request?pretty=true' -H 'accept: text/event-stream' -H 'content-type: application/json' \
       -d '{"operation":"demo.countdown","data":{}}'
HTTP/1.1 400 Bad Request
…  "code": "validation.pretty_stream",
    "message": "pretty JSON cannot be used with an event stream (Accept: text/event-stream); drop ?pretty=true", …
```

### Deprecation monitoring

0.1.0 clients that still send `{"id":…,"params":…}` keep working through 0.2.x. Every such request is visible twice:
the **response header `deprecation: true`** (REST, polling, `/mcp` with `rivet.request`) and **`"deprecated":1` on its
access-log line**. WebSocket frames and stdio MCP have no header; watch the log for HTTP clients and migrate the rest
by inventory. The keys are refused in 0.3.0 ([MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md)).

```text
$ curl -s -i http://127.0.0.1:18955/v1/request -H 'Authorization: Bearer dev-token-ada' \
       -H 'content-type: application/json' -d '{"id":"demo.add","params":{"a":1}}'
HTTP/1.1 200 OK
deprecation: true

{"request_id":"req_02be143d32","trace_id":"tr_02be143d32","operation":"demo.add","type":"result","status":"ok","data":1,"error":null,"effects":"none","data_count":0}
```

```text
 access log (stderr)                                      monitoring
 {…,"principal":"ada","operation":"demo.add",           grep -c '"deprecated":1' access.log         → how many
  "status":200,"duration_ms":0,"deprecated":1}           jq -r 'select(.deprecated==1)|.principal' access.log | sort | uniq -c
                                                                                                     → who to migrate
 0.2.x: count falls to 0 ─▶ safe to upgrade to 0.3.0 (id/params refused: validation.input_envelope)
```

### Start a server

```bash
rivet serve --file app.rivet --listen 127.0.0.1:18951
```

stderr receipt:

```text
{"listen_addr":"127.0.0.1:18951","stdio":false,"surfaces":["http","sse","poll","ws","mcp"],"auth_type":"none","catalog_version":"sha256:67104f0e7faaeafee253db758a668a9b24aa4263677e9d5ea2451b32019f9730","policy_hash":null}
```

```text
 state:  compile bundle ─► load policy ─► validate --listen / auth ─► bind ─► receipt ─► SERVING
                │ error exit 2     │ policy.invalid exit 2  │ serve.auth_required exit 2      │
                │                                         │ unsupported.serve_mtls exit 5   │ SIGINT or SIGTERM
                │ unsupported.feature exit 5 (a build without `serve`, or a bundle needing   ▼
                │   grpc/quic/oauth in a build without them)
                                   DRAINING: stop accepting ─► cancel requests + sessions (5 s grace) ─► exit 0
```

### Health, access log and shutdown

**Health.** `GET /v1/health` is mounted whatever `serve.surfaces` says. On a loopback bind it needs no
credentials; on any other bind it is authenticated like every route. It answers the `rivet.health` envelope:

```text
$ curl -s -i http://127.0.0.1:18952/v1/health
HTTP/1.1 200 OK
content-type: application/json
traceparent: 00-9b1a1e95b4d67fd4870aec50f28c0a31-dd05364efca207bc-01

{"request_id":"req_039e215277","trace_id":"tr_039e215277","operation":"rivet.health","type":"result","status":"ok","data":{"status":"ok","catalog_version":"sha256:d7bc50b83b6b4e5c6eda533ddba582129a8edc30bcafe80ad3844972d6282787","version":"0.2.1"},"error":null,"effects":"none","data_count":0}
```

**Access log.** After the receipt, every request on every surface writes one JSON line to stderr:
`time`, `surface`, `method`, `route` (the matched pattern, never the query string), `principal`, `operation`,
`status`, `duration_ms`, plus `"deprecated":1` for 0.1.0 input keys. Params, bodies, query strings and tokens are
never logged. Captured on `18955` (bearer, `team.json`):

```text
{"time":"2026-09-28T22:04:17.167Z","surface":"http","method":"POST","route":"/v1/request","principal":"ada","operation":"demo.add","status":200,"duration_ms":3}
{"time":"2026-09-28T22:04:17.183Z","surface":"http","method":"POST","route":"/v1/request","principal":"ada","operation":"demo.add","status":200,"duration_ms":0,"deprecated":1}
{"time":"2026-09-28T22:04:17.202Z","surface":"sse","method":"POST","route":"/v1/request","principal":"ada","operation":"demo.countdown","status":200,"duration_ms":0}
{"time":"2026-09-28T22:04:17.209Z","surface":"http","method":"POST","route":"/v1/request","principal":null,"operation":null,"status":401,"duration_ms":0}
{"time":"2026-09-28T22:04:17.215Z","surface":"http","method":"POST","route":"/v1/request","principal":"ci","operation":"demo.add","status":403,"duration_ms":0}
{"time":"2026-09-28T22:04:17.223Z","surface":"http","method":"GET","route":"/v1/io","principal":"ada","operation":"rivet.io","status":403,"duration_ms":0}
{"time":"2026-09-28T22:04:17.236Z","surface":"http","method":"GET","route":"/v1/ws","principal":null,"operation":null,"status":404,"duration_ms":0}
{"time":"2026-09-28T22:04:17.249Z","surface":"http","method":"GET","route":"/v1/health","principal":null,"operation":"rivet.health","status":200,"duration_ms":0}
```

**Shutdown.** SIGINT (Ctrl-C) and SIGTERM drain the same way: stop accepting, cancel every in-flight request and
session (sessions end `cancelled.shutdown`; each run closes its handles in reverse order within the 5 s grace),
let responses flush (up to 6 s) and exit **0**:

```text
$ rivet --file app.rivet serve --listen 127.0.0.1:18953 2>/dev/null & P=$!
$ kill -TERM $P; wait $P; echo "serve exit after SIGTERM: $?"
serve exit after SIGTERM: 0
```

**Trace correlation.** Send a W3C `traceparent` header and its trace-id becomes the request's `trace_id`; every
answer that belongs to a request carries `traceparent` back:

```text
$ curl -s -i http://127.0.0.1:18952/v1/request -H 'content-type: application/json' \
       -H 'traceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01' \
       -d '{"operation":"demo.read","data":{"path":"data/a.txt"}}'
HTTP/1.1 200 OK
content-type: application/json
traceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-2c62e9adf172c086-01

{"request_id":"req_0419e663c4","trace_id":"4bf92f3577b34da6a3ce929d0e0e4736","operation":"demo.read","type":"result","status":"ok","data":"hello world!","error":null,"effects":"none","data_count":0}
```

| Failure | Output (stderr envelope, abridged) | Exit |
|---|---|---|
| `--listen 0.0.0.0:18956` with no `serve.auth` | `serve.auth_required` "non-loopback listener requires serve.auth in policy.json" | 2 |
| `--listen nothost` | `validation.usage` "--listen nothost: use HOST:PORT with an IP address or localhost" | 2 |
| `serve.auth` `{"type":"mtls",…}` | `unsupported.serve_mtls` "serve.auth type mtls needs a TLS listener, which this build does not provide yet; use bearer behind a TLS-terminating proxy" | 5 |
| `mtls` without `client_ca` | `policy.invalid` "/serve/auth/client_ca: is required for mtls" | 2 |
| a binary built without the `serve` feature | `unsupported.feature` "`rivet serve` needs the `serve` feature, which this build was compiled without (rebuild rivet-runtime with `--features serve`)", `details.feature: "serve"` | 5 |

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
rivet serve --file app.rivet --policy policies/team.json --listen 127.0.0.1:18955
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
| `GET /v1/operations` (no header) | 401, `www-authenticate: Bearer` | envelope `operation: "rivet.list"`, `auth.required` "missing bearer token" |
| `… -H 'authorization: Bearer wrong'` | 401 | `auth.invalid` "invalid bearer token" |
| `… Bearer dev-token-ci` `GET /v1/operations` | 200 | `data.operations` lists only `demo.health` |
| `… Bearer dev-token-ci` `POST /v1/request {"operation":"demo.add",…}` | 403 | `permission.denied` "principal \`ci\` may not call \`demo.add\`" |
| `… Bearer dev-token-ada` `POST /v1/request {"operation":"demo.add","data":{"a":1}}` | 200 | `"status":"ok","data":1` |
| `… Bearer dev-token-ada` `GET /v1/io` | 403 | "principal \`ada\` may not call \`rivet.io\`" |
| WebSocket upgrade (ws not in `surfaces`) | 404 | `not_found.route` |

Full envelopes: [API-2026-0001 §Authentication](../api/api-2026-0001-http-rest-sse-polling.md#authentication).

`rivet.io`, `rivet.policy.generate`, `rivet.trace.show`, `rivet.trace.export` and `rivet.connectors.sync` need the
local principal or an **exact** listing: a principal with `["*"]` still gets 403 on `/v1/io`;
`["demo.health", "rivet.io"]` gets 200 on `/v1/io` and 403 on `/v1/policy/generate`. With `auth` `none` the caller
is the local principal. Note that `["*"]` **does** match the `rivet.auth.*` built-ins (a known limitation); what
they can do is then decided by `allow_auth` grants.

Rotating tokens: [RUN-2026-0001](../runbooks/run-2026-0001-rotate-serve-bearer-tokens.md).

### Call over REST

```bash
B=http://127.0.0.1:18951
curl -sS $B/v1/operations
curl -sS $B/v1/operations/demo.add
curl -sS $B/v1/operations/demo.countdown/outputs
curl -sS $B/v1/request -H 'content-type: application/json' -d '{"operation":"demo.add","data":{"a":2,"b":3}}'
```

```text
{"request_id":"req_02b1ea0422","trace_id":"tr_02b1ea0422","operation":"rivet.list","type":"result","status":"ok","data":{"operations":[{"id":"demo.greet",…,"streaming":false},…,{"id":"demo.countdown",…,"streaming":true}],"next_cursor":null},"error":null,"effects":"none","data_count":0}
{"request_id":"req_032d464237","trace_id":"tr_032d464237","operation":"rivet.describe","type":"result","status":"ok","data":{"id":"demo.add","name":"Add two integers",…,"input":{"type":"object","properties":{"a":{"type":"integer",…},"b":{…,"default":0}},"required":["a"],"additionalProperties":false},"output":{"type":"integer","description":"Sum of a and b."},"emits":null,"receives":null,"errors":[],"delivery":"unary","source":{"file":"app.rivet","line":9}},"error":null,"effects":"none","data_count":0}
{"request_id":"req_04ac80234c","trace_id":"tr_04ac80234c","operation":"rivet.outputs","type":"result","status":"ok","data":{"id":"demo.countdown","output":{…},"emits":{"type":"integer","description":"One countdown value per item."},"receives":null,"errors":[]},"error":null,"effects":"none","data_count":0}
{"request_id":"req_01b1f0b3fd","trace_id":"tr_01b1f0b3fd","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}
```

`/v1/request` body is the input envelope `{"operation": "…", "data": {…}, "deadline_ms": N?, "restrict": {"grants": [...]}?}`
(`deadline_ms` optional, capped at 600000; default 30000). `restrict` narrows this request's authority to its
intersection with policy.json and never widens it ([MAN-2026-0005](man-2026-0005-policy-and-io-manifest-guide.md#narrow-one-request-with-restrict)):

```text
$ curl -s http://127.0.0.1:18952/v1/request -H 'content-type: application/json' -d '{"operation":"demo.read","data":{"path":"data/a.txt"},
       "restrict":{"grants":[{"capability":"allow_read","targets":["./data/other/**"],"access":["read"]}]}}'          [403]
{"request_id":"req_0599d1cc19","trace_id":"tr_0599d1cc19","operation":"demo.read","type":"result","status":"error","data":null,"error":{"kind":"permission","code":"permission.denied","message":"allow_read read on data/a.txt denied: request restriction: no grant for allow_read data/a.txt",…},"effects":"none","data_count":0}
```

`GET /v1/io` returns the `rivet.io` envelope with the I/O manifest in `data`; add `format=table|markdown|csv` or
`report=true` for the rendered report (`data` = `{format, by, rendered, diagnostics, exit_code, manifest}`).

| Failure | Status | Code |
|---|---|---|
| `{"operation":"demo.add","data":{"a":"x"}}` | 422 | `validation.type` |
| body `{nope` | 400 | `validation.malformed_json` |
| `[1]`, or `{"operation":…,"id":…}` | 422 | `validation.input_envelope` |
| `{"data":{…}}` (no operation) | 422 | `validation.required` (`details.field: "operation"`) |
| `{"operation":"demo.nope"}` | 404 | `not_found.operation` |
| streaming operation without `Accept: text/event-stream` | 422 | `stream.required` "use Accept: text/event-stream, POST /v1/requests, /v1/ws or rivet.sessions.open" |
| the request outlives its `deadline_ms` | 504 | `timeout.request` |
| `"restrict":{"bogus":1}` | 422 | `policy.invalid` `details.pointer` `/restrict/bogus` |
| `?pretty=true` with SSE | 400 | `validation.pretty_stream` |
| any unknown path | 404 | `not_found.route` |

### Stream over SSE

```bash
curl -sS -N $B/v1/request -H 'content-type: application/json' \
  -H 'accept: text/event-stream' -d '{"operation":"demo.countdown","data":{}}'
```

```text
HTTP/1.1 200 OK
content-type: text/event-stream
cache-control: no-cache
traceparent: 00-f766e7c657f95fa0518268ea4b24e225-86ec5e7105f83e98-01

id: 1
event: data
data: {"request_id":"req_06ec2288c6","trace_id":"tr_06ec2288c6","operation":"demo.countdown","type":"data","seq":1,"data":3,"error":null}

id: 2
event: data
data: {"request_id":"req_06ec2288c6","trace_id":"tr_06ec2288c6","operation":"demo.countdown","type":"data","seq":2,"data":2,"error":null}

id: 3
event: data
data: {"request_id":"req_06ec2288c6","trace_id":"tr_06ec2288c6","operation":"demo.countdown","type":"data","seq":3,"data":1,"error":null}

id: 4
event: result
data: {"request_id":"req_06ec2288c6","trace_id":"tr_06ec2288c6","operation":"demo.countdown","type":"result","seq":4,"status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}
```

A failure **after** the stream started is the terminal `event: result` whose record has `"status":"error"` (there is
no `event: error` any more); a failure before the first item is a plain JSON envelope with its HTTP status:

```text
$ curl -sS -N http://127.0.0.1:18950/v1/request -H 'content-type: application/json' -H 'accept: text/event-stream' \
       -d '{"operation":"demo.flaky","data":{}}'
id: 1
event: data
data: {"request_id":"req_01581bae8d","trace_id":"tr_01581bae8d","operation":"demo.flaky","type":"data","seq":1,"data":1,"error":null}

id: 2
event: result
data: {"request_id":"req_01581bae8d",…,"type":"result","seq":2,"status":"error","data":null,"error":{"kind":"not_found","code":"not_found.file","message":"./data/missing.txt: no such file",…},"effects":"none","data_count":1}
```

```text
 SSE client loop:  for each event → parse data → type "data" → handle item
                                              → type "result" → status ok: done · error/cancelled: raise error.code
```

### Poll a session

```text
 POST /v1/requests {operation, data} ─► 202 envelope status "accepted", data = {session_id, events_url, next_send_seq, expires_at, …}
      │
      ├─ POST …/input {send_seq, data}   (receives operations; send_seq starts at 1)   ─► 200 ack
      ├─ POST …/finish_input             (half-close input)                            ─► 200 ack
      ├─ GET  …/events?after_seq=N&wait_ms=M   (long-poll ≤ 5000 ms; acknowledges ≤ N)
      │        ─► {session_id, events:[records…], last_seq, terminal}
      └─ POST …/cancel ─► {state:"cancelled"}  (already finished: its terminal state, e.g. "succeeded")
      errors of every sub-route ─► error envelope (operation rivet.sessions.*) with the registry status
```

`POST /v1/requests` also accepts `deadline_ms` (the session's total deadline, default 30000, capped at 600000)
and `restrict`. A background sweeper enforces the 60 s idle lease (the session is cancelled, `cancelled.idle`) and
the 60 s retention after the terminal event (then `404 not_found.session`) without any client call. A cancel that
arrives before a racing completion wins. The retained events of all sessions count against
`limits.max_buffered_bytes` (`429 limit.buffered_bytes`).

```bash
B=http://127.0.0.1:18950
curl -sS -X POST $B/v1/requests -H 'content-type: application/json' -d '{"operation":"demo.countdown","data":{}}'
curl -sS "$B/v1/requests/ses_0157bf2ec5/events?after_seq=0&wait_ms=2000&max_events=2"
curl -sS "$B/v1/requests/ses_0157bf2ec5/events?after_seq=2&wait_ms=2000"
```

```text
HTTP/1.1 202 Accepted
{"request_id":"req_02d661b32a","trace_id":"tr_02d661b32a","operation":"demo.countdown","type":"result","status":"accepted","data":{"session_id":"ses_0157bf2ec5","request_id":"req_02d661b32a","trace_id":"tr_02d661b32a","catalog_version":"sha256:d7bc50b8…2787","input_schema":null,"emits_schema":{"type":"integer"},"next_send_seq":1,"expires_at":"2026-09-28T21:23:07Z","events_url":"/v1/requests/ses_0157bf2ec5/events"},"error":null,"effects":"none","data_count":0}
{"session_id":"ses_0157bf2ec5","events":[{…,"operation":"demo.countdown","type":"data","seq":1,"data":3,"error":null},{…,"type":"data","seq":2,"data":2,"error":null}],"last_seq":2,"terminal":false}
{"session_id":"ses_0157bf2ec5","events":[{…,"type":"data","seq":3,"data":1,"error":null},{…,"type":"result","seq":4,"status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}],"last_seq":4,"terminal":true}
```

Live input to a `receives` operation (`chat.echo`):

```text
POST …/input {"send_seq":1,"data":"hi"}   → 200 {"session_id":"ses_02bb57670a","accepted_seq":1,"input_closed":false}
POST …/input {"send_seq":3,"data":"x"}    → 409 {…,"operation":"rivet.sessions.send",…,"status":"error",…,"error":{"kind":"conflict","code":"conflict.input_sequence","message":"expected send_seq 2, got 3",…},…}
POST …/finish_input                       → 200 {"session_id":"ses_02bb57670a","accepted_seq":null,"input_closed":true}
GET  …/events?after_seq=0&wait_ms=2000    → 200 {"session_id":"ses_02bb57670a","events":[{…,"type":"data","seq":1,"data":"hi","error":null},{…,"type":"result","seq":2,"status":"ok","data":{"echoed":1},"error":null,"effects":"none","data_count":1}],"last_seq":2,"terminal":true}
```

Resending the same `send_seq` returns the same acknowledgement (idempotent). An item that does not match the
operation's `receives` is refused with `422 validation.input` (`details {seq, path, expected, found}`); input after
`finish_input` is `409 conflict.input_closed`. Unknown session: `404 not_found.session` "no session \`ses_bogus\`".
Sessions are owned by the principal that opened them. Cancelling a running session and reading it:

```text
POST …/cancel                           → 200 {"session_id":"ses_04b2b2ef04","request_id":"req_0533ef7701","state":"cancelled"}
GET  …/events?after_seq=0&wait_ms=1000  → 200 {…,"events":[{…,"operation":"chat.echo","type":"result","seq":1,"status":"cancelled","data":null,"error":{"kind":"cancelled","code":"cancelled.session","message":"the session was cancelled","retryable":false},"effects":"none","data_count":0}],"last_seq":1,"terminal":true}
```

Full captures: [API-2026-0001 §Polling sessions](../api/api-2026-0001-http-rest-sse-polling.md#polling-sessions).

### Multiplex over WebSocket

Connect to `GET /v1/ws` offering subprotocol `rivet.v1` (bearer auth on the upgrade request). Each client frame
names a `ref`; request frames carry the input envelope; every server frame is an envelope record with `ref` first;
every ref ends with exactly one `type: "result"` record; at most 8 refs may be in flight.

```text
 client                                            server
   │ {"type":"request","ref":"c1","operation":"demo.add","data":{"a":2,"b":3}} ─►
   │ {"type":"request","ref":"c2","operation":"demo.countdown","data":{}}      ─►
   │ ◄─ {"ref":"c1",…,"type":"result","seq":1,"status":"ok","data":5,…}
   │ ◄─ {"ref":"c2",…,"type":"data","seq":1,"data":3,"error":null}   … seq 2, seq 3
   │ ◄─ {"ref":"c2",…,"type":"result","seq":4,"status":"ok","data":{"count":3},…,"data_count":3}
   │ {"type":"input","ref":"r1","seq":1,"data":"x"} / {"type":"finish_input","ref":"r1"} / {"type":"cancel","ref":"r2"}
```

Captured frames (`18950`):

```text
-> {"type":"request","ref":"c1","operation":"demo.add","data":{"a":2,"b":3}}
<- {"ref":"c1","request_id":"req_0665d9604e","trace_id":"tr_0665d9604e","operation":"demo.add","type":"result","seq":1,"status":"ok","data":5,"error":null,"effects":"none","data_count":0}
-> {"type":"request","ref":"c2","operation":"demo.countdown","data":{}}
<- {"ref":"c2","request_id":"req_07cd5f6a43","trace_id":"tr_07cd5f6a43","operation":"demo.countdown","type":"data","seq":1,"data":3,"error":null}
<- {"ref":"c2","request_id":"req_07cd5f6a43","trace_id":"tr_07cd5f6a43","operation":"demo.countdown","type":"data","seq":2,"data":2,"error":null}
<- {"ref":"c2","request_id":"req_07cd5f6a43","trace_id":"tr_07cd5f6a43","operation":"demo.countdown","type":"data","seq":3,"data":1,"error":null}
<- {"ref":"c2","request_id":"req_07cd5f6a43","trace_id":"tr_07cd5f6a43","operation":"demo.countdown","type":"result","seq":4,"status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}
```

| Misuse | Result |
|---|---|
| upgrade without `Sec-WebSocket-Protocol: rivet.v1` | HTTP 422 `validation.subprotocol` (an envelope) |
| `input`/`finish_input`/`cancel` for an unknown ref | `{"ref":"zz","request_id":"","trace_id":"","operation":null,"type":"result","status":"error",…,"error":{…"code":"not_found.ref",…}}` |
| reusing a ref that is in flight | `conflict.ref` "ref \`c3\` is already in flight", sent with `ref: ""` and `error.details.ref: "c3"` (not a terminal record of `c3`) |
| a ninth concurrent ref | `limit.ws_refs` error record (`retryable: true`) |
| a request frame without `operation`, or with `operation` + `id` | `validation.required` / `validation.input_envelope` error record |
| `cancel` on a running ref | terminal `{"ref":"e2",…,"type":"result","seq":1,"status":"cancelled",…,"error":{"kind":"cancelled","code":"cancelled.session",…}}` |
| `input` with a skipped `seq` | terminal record `status: "error"`, `conflict.input_sequence` "expected send_seq 2, got 3", with the next `seq` and the real `data_count` |
| `input` that does not match `receives` | terminal record `validation.input` "input item 1 at $ must be text, got integer" |
| `input` after `finish_input` | terminal record `conflict.input_closed` |

Each ref has its own outbound lane of 16 frames, so one slow ref never stalls the others. A `request` frame may
carry `"deadline_ms"` (0.2.0) and `"restrict": {"grants": [...]}`; a `traceparent` header on the upgrade sets the
trace of every ref. Closing the socket (or a server drain) cancels every in-flight ref and waits for cleanup. All
frames: [API-2026-0002](../api/api-2026-0002-websocket-rivet-v1.md#examples).

### Expose tools over MCP

`/mcp` implements MCP Streamable HTTP (protocol `2025-11-25`). Every public operation is a tool named by its ID;
the `rivet.*` built-ins are tools too.

```text
 POST /mcp initialize ─► 200 + header mcp-session-id: mcp_…
 POST /mcp notifications/initialized (with mcp-session-id) ─► 202
 POST /mcp tools/list | tools/call (with mcp-session-id) ─► 200 JSON-RPC result; structuredContent = envelope
 DELETE /mcp (with mcp-session-id) ─► 204   ·   GET /mcp ─► 405 (allow: POST, DELETE)
```

```bash
H=(-H 'content-type: application/json')
curl -sS -i "${H[@]}" -X POST $B/mcp -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"doc","version":"0.2"}}}'
curl -sS "${H[@]}" -H 'mcp-session-id: mcp_1a68f2fcea0c76135' -X POST $B/mcp \
  -d '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"demo.add","arguments":{"a":2,"b":3}}}'
```

```text
HTTP/1.1 200 OK
mcp-session-id: mcp_1a68f2fcea0c76135
{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"rivet","version":"0.2.1"}}}

{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"{\"request_id\":\"req_24211eee50\",…,\"status\":\"ok\",\"data\":5,…}"}],"structuredContent":{"request_id":"req_24211eee50","trace_id":"tr_24211eee50","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0},"isError":false}}
```

- `structuredContent` is the response envelope and `content[0].text` the same JSON as text; every tool's
  `outputSchema` is the envelope schema.
- Streaming operations (`_meta: {"rivet/delivery": "session"}`) return an envelope with `status: "accepted"` and the
  session receipt in `data` — read events with `rivet.sessions.read`.
- A failed or cancelled call is a tool result with `"isError": true` and the error envelope in `structuredContent`.
- `rivet.request` and `rivet.sessions.open` take `{"operation":…,"data":…}`; the deprecated `{"id":…,"params":…}` still
  works over HTTP and the answer carries `deprecation: true`.
- Missing session header after initialize: 422 `mcp.session_required`; after `DELETE`: 404
  `not_found.mcp_session` (both as envelopes, without JSON-RPC wrapping).
- `tools/list` lists the direct tools, then every built-in the principal may call (the local principal sees 20:
  `rivet.request`, `rivet.list`, `rivet.describe`, `rivet.outputs`, `rivet.sessions.*`, `rivet.io`,
  `rivet.policy.generate`, `rivet.trace.show`, `rivet.trace.export`, `rivet.capabilities`,
  `rivet.connectors.sync`, `rivet.auth.*`).
- `tools/call` params may carry `"restrict": {"grants": [...]}` beside `name` and `arguments`.
- `resources/list` and prompts are not served (`-32601 method not found`).

Full exchanges: [API-2026-0003](../api/api-2026-0003-mcp-server-tools.md).

**MCP over stdio** (for agent hosts that spawn a process):

```bash
rivet serve --file app.rivet --stdio
```

The receipt goes to stderr; stdout carries only newline-delimited JSON-RPC (tool results carry the same envelope).
`--stdio` mounts MCP only and ignores `--listen`.

### Built-in operations

Callable through every surface (`POST /v1/request {"operation":"rivet.list"}`, MCP tools, CLI `request rivet.list`):

| Built-in | Purpose | Principal requirement |
|---|---|---|
| `rivet.capabilities` | what this build supports (stages, HTTP versions, sandbox backend/status, surfaces; 0.2.0: `build_features`, `abi_version`) | any authenticated principal |
| `rivet.request` | generic dispatch `{operation, data}` (deprecated `{id, params}`) | the target must be allowed |
| `rivet.list`, `rivet.describe`, `rivet.outputs` | catalog discovery | normal pattern matching |
| `rivet.sessions.open/send/finish_input/read/cancel` | session control (`read` waits ≤ 5000 ms) | normal pattern matching |
| `rivet.auth.begin/complete/status/disconnect/cancel` | OAuth accounts (MAN-2026-0008) | normal pattern matching + `allow_auth` grants |
| `rivet.io`, `rivet.policy.generate`, `rivet.trace.show`, `rivet.trace.export {request_id, path}`, `rivet.connectors.sync {name, output}` | inspection and admin | local principal or an **exact** listing |

```text
$ rivet --file app.rivet request rivet.capabilities          # abridged; also POST /v1/request, MCP tools/call
{"request_id":"req_01ab1fac65","trace_id":"tr_01ab1fac65","operation":"rivet.capabilities","type":"result","status":"ok",
 "data":{"version":"0.2.1","platform":{"os":"macos","arch":"aarch64"},"stages":{"A":"supported","B":"supported","C":"unsupported"},
         "features":[{"name":"http","stage":"A",…},…],"sandbox":{"backend":"macos-seatbelt","status":"active",…},
         "serve":{"surfaces":["cli","http","sse","poll","websocket","mcp","library"],"auth":["none","bearer"]},
         "build_features":["serve","grpc","quic","oauth","cli"],"abi_version":1},
 "error":null,"effects":"none","data_count":0}
```

On Linux `sandbox` reads `"status":"gated"` (process spawns that need the sandbox are refused with
`unsupported.sandbox_backend`, exit 5 / 501).

`rivet.trace.export {request_id, path}` (`output` is accepted as an alias) writes this server's trace of one
request to a new file under the bundle, through the broker; `rivet --endpoint URL trace export REQ --output PATH`
calls it (MAN-2026-0004).

### Use the CLI against a server

```text
 rivet --endpoint URL --token-file FILE  request | list | describe | outputs | io | trace | auth
        (not with --file/--policy)          check | graph | policy | serve  → refused (exit 2)
```

```bash
printf %s 'dev-token-ada' > ada.token && chmod 600 ada.token
rivet --endpoint http://127.0.0.1:18955 --token-file ada.token request demo.add --data '{"a":4,"b":5}'
rivet --endpoint http://127.0.0.1:18955 --token-file ada.token request demo.countdown --stream
```

```text
{"request_id":"req_033cf82187","trace_id":"tr_033cf82187","operation":"demo.add","type":"result","status":"ok","data":9,"error":null,"effects":"none","data_count":0}
{"request_id":"req_04bcbc681c","trace_id":"tr_04bcbc681c","operation":"demo.countdown","type":"data","seq":1,"data":3,"error":null}
{"request_id":"req_04bcbc681c","trace_id":"tr_04bcbc681c","operation":"demo.countdown","type":"data","seq":2,"data":2,"error":null}
{"request_id":"req_04bcbc681c","trace_id":"tr_04bcbc681c","operation":"demo.countdown","type":"data","seq":3,"data":1,"error":null}
{"request_id":"req_04bcbc681c","trace_id":"tr_04bcbc681c","operation":"demo.countdown","type":"result","seq":4,"status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}
```

| Failure | Output (stderr) | Exit |
|---|---|---|
| no `--token-file` against a bearer server | envelope `auth.required` "missing bearer token" | 3 |
| `ci` token calling `demo.add` | envelope `permission.denied` "principal \`ci\` may not call \`demo.add\`" | 3 |
| `ada` token, `io` (not listed exactly) | `error[permission.denied]: principal \`ada\` may not call \`rivet.io\`` | 3 |
| `--endpoint … check` | "check, graph, policy and serve work on a local bundle (--file); they are not available with --endpoint" | 2 |
| `--endpoint … --file app.rivet list` | "--endpoint cannot be combined with --file or --policy: the server owns the bundle and its policy" | 2 |
| `--endpoint … io --check-files` (a principal allowed `rivet.io`) | `validation.check_files_remote` | 2 |

Outputs and exit codes match local mode, apart from generated IDs.

### Expected Result and Side Effects

Serving binds one TCP port and writes the receipt plus one access-log line per request to stderr. Sessions, MCP
sessions, OAuth transactions and traces live in memory and are lost on restart. Stopping with SIGINT or SIGTERM
drains (cancel in-flight work, close handles within 5 s) and exits 0.

### Verified Demo

[01-catalog (DEMO-2026-0001)](../demos/01-catalog/README.md) with `policies/team.json` (its `requests/` files are
being re-verified for 0.2.0 in P4), plus the scratch bundle described at the top of this section. Every capture above
was taken on 2026-09-29 from the 0.2.0 release candidate on `127.0.0.1:18950`–`18956`.

## Complete API and Event Reference

| Method / Route / Event | Purpose | Auth | Request | Success Response | Errors / Status | Idempotency / Side Effects | Example | Since |
|---|---|---|---|---|---|---|---|---|
| `POST /v1/request` | invoke | bearer if configured | InputEnvelope `{operation, data, deadline_ms?, restrict?}` (deprecated `{id, params}` → `deprecation: true`) + `traceparent?`; `?pretty=true` | 200 envelope (+ `traceparent`); SSE records with `Accept: text/event-stream` | 400/401/403/404/422/5xx envelopes | runs the operation | above | 0.1.0 (envelopes 0.2.0) |
| `GET /v1/health` | liveness | none on loopback, else serve.auth | — | 200 `rivet.health` envelope, `data` `{status, catalog_version, version}` | 401 | none | above | 0.1.0 |
| `GET /v1/operations` | list | same | — | 200 `rivet.list` envelope, `data` `{operations, next_cursor}` | 401 | none | above | 0.1.0 |
| `GET /v1/operations/{id}` | describe | same | — | 200 `rivet.describe` envelope | 404 | none | above | 0.1.0 |
| `GET /v1/operations/{id}/outputs` | declared outputs | same | — | 200 `rivet.outputs` envelope, `data` `{id, output, emits, receives, errors}` | 404 | none | above | 0.1.0 |
| `GET /v1/io?by=…&check_policy=…&needs=…` | I/O manifest | exact `rivet.io` | query | 200 `rivet.io` envelope, manifest in `data` (`format=table…`/`report=true`: IoReport) | 403 | none | MAN-0005 | 0.1.0 |
| `POST /v1/policy/generate` | policy draft | exact listing | `{ids, all}` (built-in parameters, not an InputEnvelope) | 200 `rivet.policy.generate` envelope, `data` `{policy, review, complete}` | 403, 422 `validation.body` | never writes | MAN-0005 | 0.1.0 |
| `POST /v1/requests` | open session | same | InputEnvelope | 202 envelope `status: "accepted"`, receipt in `data` | 403/404/422/429 | starts the request | above | 0.1.0 (envelope 0.2.0) |
| `GET /v1/requests/{id}/events` | read events | owner | `after_seq`, `wait_ms`, `max_events` | 200 `{session_id, events:[records], last_seq, terminal}` | 404 | acknowledges ≤ after_seq | above | 0.1.0 |
| `POST /v1/requests/{id}/input` / `finish_input` / `cancel` | drive session | owner | `{send_seq, data}` | 200 ack / cancel receipt | 404/409/422 envelopes | same seq is idempotent | above | 0.1.0 |
| `GET /v1/ws` | WebSocket | on upgrade | frames (`request` = InputEnvelope + `ref`) | 101; records with `ref` | 422 subprotocol | per-ref sessions | above | 0.1.0 (records 0.2.0) |
| `POST`/`DELETE /mcp` | MCP | same | JSON-RPC | 200/202/204; `structuredContent` = envelope | 404/405/422 | MCP session state | above | 0.1.0 (envelopes 0.2.0) |

## Errors and Recovery Reference

| Error / Code / Message | Surface | Cause | User-Visible Result | Recovery | Retry Safe | Related Feature |
|---|---|---|---|---|---|---|
| `serve.auth_required` | startup | non-loopback + auth none | exit 2 | configure bearer | no | auth |
| `unsupported.serve_mtls` | startup | mtls configured | exit 5 | bearer + TLS proxy | no | auth |
| `auth.required` / `auth.invalid` | all | missing/unknown token | 401 | fix token | no | auth |
| `permission.denied` | all | principal not allowed | 403 | add to `principals` | no | principals |
| `not_found.route` | all | unknown path or disabled surface | 404 | enable in `serve.surfaces` | no | surfaces |
| `stream.required` | REST | streaming op without SSE | 422 | use SSE/polling/WS | no | streams |
| `validation.input_envelope` / `validation.required` | REST, polling, WS, MCP `rivet.request` | input not an object, mixed key and alias, bad field type / no `operation` | 422 / WS error record | send `{operation, data}` | no | envelopes |
| `validation.pretty_stream` | SSE | `?pretty=true` with `Accept: text/event-stream` | 400 | drop `pretty` | no | pretty |
| `unsupported.feature` | startup | binary built without `serve` | exit 5 | rebuild with default features | no | features |
| `validation.subprotocol` | WS | missing `rivet.v1` | 422 | offer `rivet.v1` | no | WS |
| `mcp.session_required` / `not_found.mcp_session` | MCP | missing/expired session | 422 / 404 | re-initialize | yes | MCP |
| `not_found.session`, `not_found.ref`, `conflict.ref`, `limit.ws_refs` | polling, WS | session/ref misuse | 404/error frames | fix IDs; wait for refs | depends | sessions |
| `conflict.input_sequence`, `validation.input`, `conflict.input_closed` | polling, WS | refused input | 409/422; WS terminal frame for that ref | resend in order / fix the item | depends | sessions |
| `limit.buffered_bytes` | polling, WS, MCP sessions | host byte budget full | 429 | read and acknowledge events; raise the limit | yes, after backoff | limits |
| `cancelled.idle`, `cancelled.shutdown` | sessions | idle lease expired / server drain | terminal cancel event | read sooner / restart | re-run | sessions |

## Limitations

The serving rows of the [manual's Known Limitations](man-2026-0001-rivet-manual.md#known-limitations):

- No mTLS (`unsupported.serve_mtls`, exit 5) and no TLS listener: terminate TLS in a proxy and use bearer auth.
- The `*` principal pattern matches `rivet.auth.*` (governed by `allow_auth`).
- Platforms: macOS and Linux; Windows is not supported ([INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)).
- MCP server exposes tools only (no resources, resource templates or prompts); no legacy HTTP+SSE MCP transport.
- No persistent trace store; sessions and OAuth transactions are in memory only.

## Version Applicability

| Feature / Interface | Introduced | Changed | Deprecated / Removed | Applicable Environment |
|---|---|---|---|---|
| `rivet serve`, REST/SSE/polling/WS/MCP, bearer auth, principals, surfaces | 0.1.0 | — | — | server, development |
| `/v1/health`, access log, SIGTERM drain, `traceparent`, `restrict` | 0.1.0 | 0.2.0: health is an envelope; access log gains `deprecated` | — | server, development |
| Response envelopes on every surface, `?pretty=true`, `deprecation` header | 0.2.0 | — | 0.1.0 bodies removed; `id`/`params` deprecated (removed 0.3.0) | server, development |
| `--endpoint` client | 0.1.0 | — | — | any |

## Related Documents

- [Rivet manual](man-2026-0001-rivet-manual.md) · [CLI reference](man-2026-0004-cli-reference.md) ·
  [Policy guide](man-2026-0005-policy-and-io-manifest-guide.md)
- [API-2026-0001](../api/api-2026-0001-http-rest-sse-polling.md) · [API-2026-0002](../api/api-2026-0002-websocket-rivet-v1.md) ·
  [API-2026-0003](../api/api-2026-0003-mcp-server-tools.md)
- [API-2026-0006 envelopes](../api/api-2026-0006-envelopes.md) · [MIG-2026-0001 migration](../migrations/mig-2026-0001-response-and-input-envelopes.md)
- [OPS-2026-0001](../operations/ops-2026-0001-operating-rivet-serve.md) ·
  [RUN-2026-0001](../runbooks/run-2026-0001-rotate-serve-bearer-tokens.md) ·
  [SYS-2026-0004](../system/components/sys-2026-0004-surfaces-and-serve.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 6 | 2026-09-30 | Claude | v0.2.1 patch (PLAN-2026-0002 TASK-097): version strings, install tag v0.2.1; INC-2026-0013 behaviour where described. |
| 5 | 2026-09-30 | Claude | v0.2.0 release (PLAN-2026-0002 TASK-091): version strings and current-release wording updated to 0.2.0. |
| 1 | 2026-09-28 | Claude | Initial serving and surfaces guide for 0.1.0, captured from a live 0.1.0-dev (commit f40d4aa) server. |
| 2 | 2026-09-28 | Claude | Fix batch through 829ca43 and 2a751ab: health/access-log/shutdown section, `traceparent`, `restrict`, bare `/v1/io`, polling `deadline_ms`, sweeper, cancel after finish, WebSocket lanes and specific refusal frames, MCP `tools/list` built-ins, `rivet.capabilities` and `rivet.trace.export` (dispatched and listed since 2a751ab); limitations aligned with MAN-2026-0001. |
| 3 | 2026-09-29 | Claude | 0.2.0 (D-25, D-46): new **Envelopes on every surface** (per-surface wrapping table and diagram, `?pretty=true` and its SSE refusal) and **Deprecation monitoring** (`deprecation: true`, `"deprecated":1`, jq recipes, 0.3.0 gate) sections; every example re-captured as envelopes on the 0.2.0-rc (REST, SSE incl. a mid-stream failure, polling, WebSocket records, MCP `structuredContent`, built-ins, `--endpoint`, access log with bearer principals); input envelope in all requests; API reference, errors, limitations and version rows updated. |
| 4 | 2026-09-29 | Claude | INC-2026-0012: removed the WS `--timeout` limitation and the envelope deviations (fixed). |
