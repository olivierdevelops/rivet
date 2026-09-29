---
document_id: API-2026-0001
title: "Rivet HTTP API: REST, SSE and polling"
document_type: api
status: active
created_date: 2026-09-28
last_updated: 2026-09-30
document_revision: 4
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
scope: Every HTTP route mounted by `rivet serve` except /v1/ws (API-2026-0002) and /mcp (API-2026-0003) — REST, SSE framing, the polling session routes and GET /v1/health, with 0.2.0 envelope JSON (API-2026-0006), `?pretty=true`, the `Deprecation` header, per-request `restrict`, W3C `traceparent`, auth, status codes, the access log and captured examples.
reason: DOCUMENTATION.md §31 API impact for PLAN-2026-0001 row D-24 (0.1.0 contract) and PLAN-2026-0002 row D-40 (0.2.0 envelope sweep, TASK-070); every example is real output of the build.
related_documents: [PLAN-2026-0001, PLAN-2026-0002, PROP-2026-0001, PROP-2026-0002, API-2026-0002, API-2026-0003, API-2026-0005, API-2026-0006, MIG-2026-0001, ARCH-2026-0001, SEC-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, api, http, rest, sse, polling, serve]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.2.0-rc (source at 6f9943f)"
next_review_date: 2026-10-29
---

# Rivet HTTP API: REST, SSE and polling

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-30
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** serve, http, poll, sessions, auth, audit, policy, registry

## Summary

One `rivet serve` process binds **one** listener and mounts every enabled surface on it. This document is the contract for the plain-HTTP routes: REST (`/v1/request`, `/v1/operations…`, `/v1/io`, `/v1/policy/generate`), Server-Sent Events (the same `/v1/request` with `Accept: text/event-stream`), polling sessions (`/v1/requests…`) and the liveness route `GET /v1/health`. WebSocket and MCP on the same listener are in [API-2026-0002](api-2026-0002-websocket-rivet-v1.md) and [API-2026-0003](api-2026-0003-mcp-server-tools.md).

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
                                  │         GET /v1/health (always mounted)
                          Runtime::dispatch_request  (the same dispatcher as CLI + library)
                                  │   effective authority = policy.json ∩ restrict (per request)
                                  ▼
          every response on every surface ─▶ one JSON access-log line on stderr
```

Every request and response body is a 0.2.0 envelope: input `{operation, data, deadline_ms?, restrict?}`, output `{request_id, trace_id, operation, type, status, data, error, effects, data_count}` ([API-2026-0006](api-2026-0006-envelopes.md)). Clients of 0.1.0 read [MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md).

All examples were captured on 2026-09-29 (macOS 26.4) from the 0.2.0 release candidate, `cargo build --release --features cli` with the source at `6f9943f`:

```text
 port   bundle                                                   auth / policy
 18951  docs/demos/01-catalog/app.rivet                           none (loopback), no policy
 18955  docs/demos/01-catalog/app.rivet                           bearer, policies/team.json (surfaces without ws)
 18950  scratch bundle: demo.add, demo.countdown, chat.echo        none; policy.json grants allow_read ./data/**
 18952    (duplex, returns {echoed: N}), demo.read (`return file      and allow_write create ./out/**
          read path as text`), demo.flaky (emits 1, then fails)   (18952 = same bundle, fresh process for the log)
 18956/7 refused starts (never listening); 18958 = refused by a build without the `serve` feature
```

**Request IDs, trace IDs, session IDs, `traceparent` span IDs, `expires_at`, `time` and `date` differ on every run.** `version` fields read `0.2.0`.

## Audience and Stability

- **Audience:** integrators calling Rivet over HTTP, operators exposing `rivet serve`.
- **Stability:** the `/v1` prefix is the contract; its bodies became envelopes in 0.2.0 (see [Versioning and Deprecation](#versioning-and-deprecation)). Fields may be *added* to response objects within `/v1`; existing fields are not renamed or removed without a new prefix.
- **Surface names** used by `policy.json` `serve.surfaces`: `http`, `sse`, `poll`, `ws`, `mcp` (default: all five).

## Authentication

Authentication is configured only in `policy.json` → `serve.auth`. It is applied identically on every route of every surface.

| `serve.auth.type` | Behaviour | Principal |
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

**Operation authorization** (`serve.principals`). With no `serve.principals` map, every authenticated principal may call every public operation *except* the sensitive built-ins. With the map, an entry must match the operation ID exactly, by `*`, or by a trailing `prefix.*`. The five sensitive built-ins — `rivet.io`, `rivet.policy.generate`, `rivet.trace.show`, `rivet.trace.export`, `rivet.connectors.sync` — match **only an exact entry** for a network principal (never `*` or `demo.*`); the `local` principal may call them. `rivet.capabilities` and the generic built-ins (`rivet.request`, `rivet.list`, `rivet.describe`, `rivet.outputs`, `rivet.sessions.*`) are open to every authenticated principal; the generic ones authorize the operation they name. Note that a `*` pattern also matches `rivet.auth.*` (their use is then governed by `allow_auth` grants). Unlisted IDs are hidden from catalog routes (they answer 404 as if unknown) and refused on `/v1/request` (403).

Captured with [policies/team.json](../demos/01-catalog/policies/team.json) (`ada` → `demo.*`, `ci` → `demo.health`; fixture tokens `dev-token-ada`, `dev-token-ci`) on `127.0.0.1:18955`:

```text
$ curl -s -i http://127.0.0.1:18955/v1/operations
HTTP/1.1 401 Unauthorized
content-type: application/json
www-authenticate: Bearer

{"request_id":"","trace_id":"","operation":"rivet.list","type":"result","status":"error","data":null,"error":{"kind":"auth","code":"auth.required","message":"missing bearer token","retryable":false},"effects":"none","data_count":0}

$ curl -s -i http://127.0.0.1:18955/v1/operations -H 'Authorization: Bearer wrong'
HTTP/1.1 401 Unauthorized
content-type: application/json
www-authenticate: Bearer

{"request_id":"","trace_id":"","operation":"rivet.list","type":"result","status":"error","data":null,"error":{"kind":"auth","code":"auth.invalid","message":"invalid bearer token","retryable":false},"effects":"none","data_count":0}

$ curl -s http://127.0.0.1:18955/v1/operations -H 'Authorization: Bearer dev-token-ci'
{"request_id":"req_01d2663eb5","trace_id":"tr_01d2663eb5","operation":"rivet.list","type":"result","status":"ok","data":{"operations":[{"id":"demo.health","name":"Check availability","description":"Return a constant readiness response without I/O.","streaming":false}],"next_cursor":null},"error":null,"effects":"none","data_count":0}

$ curl -s -i http://127.0.0.1:18955/v1/request -H 'Authorization: Bearer dev-token-ci' \
       -H 'content-type: application/json' -d '{"operation":"demo.add","data":{"a":1}}'
HTTP/1.1 403 Forbidden
content-type: application/json
traceparent: 00-e64865a577a68a458b6e8847e930bd78-f43485654b84d3bd-01

{"request_id":"req_02509de59a","trace_id":"tr_02509de59a","operation":"demo.add","type":"result","status":"error","data":null,"error":{"kind":"permission","code":"permission.denied","message":"principal `ci` may not call `demo.add`","retryable":false},"effects":"none","data_count":0}

$ curl -s -i http://127.0.0.1:18955/v1/io -H 'Authorization: Bearer dev-token-ada'     # demo.* does not reach rivet.io
HTTP/1.1 403 Forbidden
content-type: application/json
traceparent: 00-67fb51855356140cbeb063b589efb72d-98cd6df2832dfaa4-01

{"request_id":"req_03d33356df","trace_id":"tr_03d33356df","operation":"rivet.io","type":"result","status":"error","data":null,"error":{"kind":"permission","code":"permission.denied","message":"principal `ada` may not call `rivet.io`","retryable":false},"effects":"none","data_count":0}
```

A surface disabled in `serve.surfaces` is not mounted: its routes answer `404 not_found.route` (team.json omits `ws`, so `GET /v1/ws` → 404). `GET /v1/health` is mounted whatever `serve.surfaces` says: unauthenticated on a loopback bind, authenticated per `serve.auth` otherwise.

## Endpoints or Events

| Method | Path | Surface | Purpose | Success |
|---|---|---|---|---|
| POST | `/v1/request` | `http` | Invoke one operation or built-in; one ResponseEnvelope (`?pretty=true` indents) | 200 |
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
| POST | `/v1/requests/{session_id}/cancel` | `poll` | Cancel the session (a finished session reports its terminal state) | 200 |
| GET | `/v1/health` | always | Liveness: envelope of `rivet.health`, `data` = `{status:"ok", catalog_version, version}` | 200 |
| any | anything else | — | — | 404 `not_found.route` |

When `http` is disabled but `sse` is enabled, only `POST /v1/request` is mounted (and it answers only SSE requests).

### Built-ins reachable through `POST /v1/request`

Every reserved `rivet.*` ID is dispatched through the same `/v1/request` route (and through MCP, WebSocket and the library):

| ID | `data` | Authorization for a network principal |
|---|---|---|
| `rivet.request` | `{operation, data}` (deprecated `{id, params}`) | the named operation is authorized |
| `rivet.list` / `rivet.describe` / `rivet.outputs` | `{}` / `{id}` / `{id?, all?}` | filtered by the principal's listing |
| `rivet.sessions.open` / `send` / `finish_input` / `read` / `cancel` | see [Polling sessions](#polling-sessions) | own sessions only |
| `rivet.auth.begin` / `complete` / `status` / `disconnect` / `cancel` | profile/account parameters | listed (or no `serve.principals` map) |
| `rivet.capabilities` | `{}` | any authenticated principal |
| `rivet.io` | as `/v1/io` query | **exact** listing |
| `rivet.policy.generate` | `{ids?, all?}` | **exact** listing |
| `rivet.trace.show` | `{request_id}` | **exact** listing |
| `rivet.trace.export` | `{request_id, path}` (`output` accepted as an alias) | **exact** listing |
| `rivet.connectors.sync` | `{name, output}` | **exact** listing |

`rivet.capabilities` answers what this build supports (Stage A/B/C features, HTTP versions, sandbox backend and status, serve surfaces and auth types, and from 0.2.0 the compiled Cargo features `build_features` and the C `abi_version`); it performs no I/O:

```text
$ curl -s http://127.0.0.1:18951/v1/request -H 'content-type: application/json' -d '{"operation":"rivet.capabilities"}'   # abridged
{"request_id":"req_01cff26d05","trace_id":"tr_01cff26d05","operation":"rivet.capabilities","type":"result","status":"ok",
 "data":{"version":"0.2.0","platform":{"os":"macos","arch":"aarch64"},"stages":{"A":"supported","B":"supported","C":"unsupported"},
  "features":[{"name":"http","stage":"A","support":"supported","versions":["1.1","2","3"],"streaming":["sse","jsonl","lines","bytes"]},…],
  "sandbox":{"backend":"macos-seatbelt","status":"active","reason":"Seatbelt via /usr/bin/sandbox-exec with a deny-default profile"},
  "serve":{"surfaces":["cli","http","sse","poll","websocket","mcp","library"],"auth":["none","bearer"]},
  "build_features":["serve","grpc","quic","oauth","cli"],"abi_version":1},
 "error":null,"effects":"none","data_count":0}
```

`rivet.trace.export` writes the serving process's trace of one request as JSON to a **new** bundle-relative file through the broker (`allow_write` access `create`; an existing file is `409 conflict.already_exists`):

```text
$ curl -s http://127.0.0.1:18950/v1/request -H 'content-type: application/json' \
       -d '{"operation":"rivet.trace.export","data":{"request_id":"req_012b19b65d","path":"./out/trace.json"}}'
{"request_id":"req_02a491c782","trace_id":"tr_02a491c782","operation":"rivet.trace.export","type":"result","status":"ok","data":{"request_id":"req_012b19b65d","path":"./out/trace.json","events":1,"bytes":718},"error":null,"effects":"committed","data_count":0}

$ # the same call again: the file exists                                                          → HTTP 409
{"request_id":"req_03265fbf87","trace_id":"tr_03265fbf87","operation":"rivet.trace.export","type":"result","status":"error","data":null,"error":{"kind":"conflict","code":"conflict.already_exists","message":"./out/trace.json already exists","retryable":false,"operation_id":"rivet.trace.export"},"effects":"none","data_count":0}
```

## Request Format

### `POST /v1/request`

```json
{ "operation": "demo.read", "data": { "path": "data/a.txt" }, "deadline_ms": 5000,
  "restrict": { "grants": [ { "capability": "allow_read", "targets": ["./data/**"], "access": ["read"] } ] } }
```

| Field | Type | Required | Notes |
|---|---|---|---|
| `operation` | string | yes | Operation ID or `rivet.*` built-in. Missing → 422 `validation.required` (`details.field: "operation"`). Deprecated alias: `id`. |
| `data` | object | no (default `{}`) | Checked against the operation's input schema: unknown fields, missing required, wrong types, min/max/enum. Defaults are injected after validation. Deprecated alias: `params`. |
| `deadline_ms` | integer | no | Request deadline; clamped to 1…600000 (10 minutes). Default deadline is 30000 ms. |
| `restrict` | object | no | `{"grants": [...]}` in the policy.json grant format. Intersected with the loaded policy for **this request and its nested calls only**: it can narrow, never widen. Any other key, or a malformed grant, is 422 `policy.invalid` with `details.pointer` under `/restrict`. |

| Header | Notes |
|---|---|
| `traceparent` | Optional W3C trace context `00-<32 hex trace-id>-<16 hex parent-id>-<flags>`. A valid value makes its trace-id the request's `trace_id`; an invalid one is ignored. Every answer that belongs to a request carries a `traceparent` response header (same trace-id, the request's span id). |

```text
  caller authority                     server authority (policy.json)
  restrict {grants:[./data/other/**]}  grants: allow_read ./data/** , allow_write ./out/**
            │                                     │
            └──────────── ∩ (every broker decision, nested calls included) ─────────▶ effective
  restrict naming ./**  ─▶ still only ./data/**  (a restriction never adds a grant)
```

- An empty or whitespace body is treated as `{}`, which names no operation → 422 `validation.required`.
- A body that is not JSON → **400** `validation.malformed_json`.
- A JSON body that is not an object, that carries an `error` key, or that mixes a key with its alias (`operation` + `id`, `data` + `params`) → 422 `validation.input_envelope`.
- The deprecated aliases alone (`{"id":…,"params":…}`) still work in 0.2.x; the answer carries `deprecation: true` and the access-log line `"deprecated":1`.
- `?pretty=true` indents a JSON answer (2 spaces, same key order); with `Accept: text/event-stream` it is **400** `validation.pretty_stream`.
- A **streaming** operation (one that declares `emits`) requested without `Accept: text/event-stream` → 422 `stream.required`.

### `GET /v1/io` query

Query keys map onto the `rivet.io` parameters: `by=operation|target|capability`, `kind=K`, `access=V,V`, `check_policy=true`, `needs=true`, `strict=true`, `include_bootstrap=true`, `ids=ID,ID`, `all=true`, `trace=REQ`, `format=json|table|markdown|csv`, `report=true`. The literal values `true` / `false` become booleans; everything else is text. `check_files` is refused remotely (`validation.check_files_remote`). The answer is the bare **IoManifest**; `format=table|markdown|csv` or `report=true` returns the rendered IoReport instead (below).

### `POST /v1/policy/generate`

`{"ids": ["demo.add"]}` or `{"all": true}`; empty body = every public operation. Non-JSON body → 422 `validation.body`. (This route takes the built-in's parameters directly, not an InputEnvelope.)

### Polling bodies

| Route | Body / query |
|---|---|
| `POST /v1/requests` | `{operation, data, deadline_ms?, restrict?}` (same parser as `/v1/request`, same deprecated aliases); `deadline_ms` is the session's total deadline (default 30000, capped at 600000); a `traceparent` header sets the session's trace |
| `GET …/events` | `after_seq=N` (default 0), `wait_ms=M` (default 1000, max 5000), `max_events=K` (default 16, max 16, min 1). Non-integer → 422 `validation.type`. Unknown keys ignored. |
| `POST …/input` | `{"send_seq": 1, "data": <any JSON>}`; `send_seq` required |
| `POST …/finish_input`, `POST …/cancel` | no body |

## Response Format

### ResponseEnvelope (200 from `/v1/request`)

```json
{"request_id":"req_01b1f0b3fd","trace_id":"tr_01b1f0b3fd","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}
```

`data` already passed the operation's declared `output` check (a mismatch is `output.invalid`, 500). `effects` ∈ `none | committed | partial | unknown`. `data_count` counts emitted items.

### Catalog

Catalog routes answer the envelope of the matching built-in (`rivet.list`, `rivet.describe`, `rivet.outputs`);
the 0.1.0 payload is its `data`. `GET /v1/operations`:

```json
{"request_id":"req_02b1ea0422","trace_id":"tr_02b1ea0422","operation":"rivet.list","type":"result","status":"ok",
 "data":{"operations":[{"id":"demo.greet","name":"Greet a person","description":"Return a greeting for the supplied person.","streaming":false},
                       {"id":"demo.add","name":"Add two integers","description":"Add two signed integers and return their sum.","streaming":false},
                       {"id":"demo.health","name":"Check availability","description":"Return a constant readiness response without I/O.","streaming":false},
                       {"id":"demo.countdown","name":"Count down","description":"Emit 3, 2, 1 as data items and then return a summary.","streaming":true}],
         "next_cursor":null},
 "error":null,"effects":"none","data_count":0}
```

`GET /v1/operations/demo.add` (`data` shown):

```json
{"id":"demo.add","name":"Add two integers","description":"Add two signed integers and return their sum.","kind":"operation",
 "input":{"type":"object","properties":{"a":{"type":"integer","description":"First operand."},
          "b":{"type":"integer","description":"Second operand; defaults to zero.","default":0}},
          "required":["a"],"additionalProperties":false},
 "output":{"type":"integer","description":"Sum of a and b."},
 "emits":null,"receives":null,"errors":[],"delivery":"unary","source":{"file":"app.rivet","line":9}}
```

`GET /v1/operations/demo.countdown/outputs` (`data` shown):

```json
{"id":"demo.countdown","output":{"type":"object","properties":{"count":{"type":"integer","description":"Number of items emitted."}},
 "required":["count"],"additionalProperties":false,"description":"Summary returned after the last item."},
 "emits":{"type":"integer","description":"One countdown value per item."},"receives":null,"errors":[]}
```

### I/O manifest and policy draft

`GET /v1/io` (or `format=json`) answers the `rivet.io` envelope whose `data` is the IoManifest;
`format=table|markdown|csv` or `report=true` puts the IoReport in `data` — `format`, `by`, `rendered` (the text
the CLI would print), `diagnostics`, `exit_code` and the structured `manifest`. Captured on `18950` (the
`demo.read` bundle; `sites` and `targets` abridged to their first fields):

```text
$ curl -s 'http://127.0.0.1:18950/v1/io'
{"request_id":"req_04afb7ee7c","trace_id":"tr_04afb7ee7c","operation":"rivet.io","type":"result","status":"ok",
 "data":{"bundle":{"file":"app.rivet","sha256":"2d62b1f1…b4e2c5"},"policy":{"file":"policy.json","sha256":"sha256:d33d79eb…97f1"},"complete":true,
         "sites":[{"effect_id":"demo.read#1","operation_id":"demo.read","kind":"file","access":["read"],"capability":"allow_read",
                   "target":{"template":"{path}",…,"glob":"*","params":["path"]},"knowledge":"param_dependent",…,"source":{"file":"app.rivet","line":44,"column":5},…}],
         "targets":[{"target":"*","capability":"allow_read","access":["read"],…,"knowledge":"param_dependent","decision":null}],
         "needs":[],"bootstrap":[]},
 "error":null,"effects":"none","data_count":0}

$ curl -s 'http://127.0.0.1:18950/v1/io?format=table&ids=demo.read'
{"request_id":"req_052fd98809","trace_id":"tr_052fd98809","operation":"rivet.io","type":"result","status":"ok",
 "data":{"format":"table","by":"operation","rendered":"OPERATION  KIND  ACCESS  TARGET  KNOWLEDGE        SOURCE\ndemo.read  file  read    {path}  param_dependent  app.rivet:44\n",
         "diagnostics":"","exit_code":0,"manifest":{…}},"error":null,"effects":"none","data_count":0}

$ curl -s -w ' %{http_code}' 'http://127.0.0.1:18950/v1/io?check_files=true'
{"request_id":"req_06af147fb6","trace_id":"tr_06af147fb6","operation":"rivet.io","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.check_files_remote","message":"check_files probes the host's files and is available from the CLI and library only","retryable":false,"operation_id":"rivet.io"},"effects":"none","data_count":0} 422
```

`POST /v1/policy/generate {"all":true}`:

```json
{"request_id":"req_0529ebc191","trace_id":"tr_0529ebc191","operation":"rivet.policy.generate","type":"result","status":"ok","data":{"policy":{"version":1,"grants":[],"network":{"deny_private_ranges":true}},"review":[],"complete":true},"error":null,"effects":"none","data_count":0}
```

On `18950`, `{"ids":["demo.read"]}` drafts the glob of the param-dependent site:

```json
{"request_id":"req_072c5801ab","trace_id":"tr_072c5801ab","operation":"rivet.policy.generate","type":"result","status":"ok","data":{"policy":{"version":1,"grants":[{"capability":"allow_read","targets":["*"],"access":["read"]}],"network":{"deny_private_ranges":true}},"review":[],"complete":true},"error":null,"effects":"none","data_count":0}
```

A body that is not JSON is 422 `validation.body` (this route answers the envelope of `rivet.policy.generate` with empty IDs):

```json
{"request_id":"","trace_id":"","operation":"rivet.policy.generate","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.body","message":"request body is not JSON: expected ident at line 1 column 2","retryable":false},"effects":"none","data_count":0}
```

### SSE framing

`POST /v1/request` with `Accept: text/event-stream` (any list entry, parameters ignored) answers `200`, `content-type: text/event-stream`, `cache-control: no-cache`. Each event is:

```text
id: <seq>
event: data | result
data: <one-line record: type "data" (seq, data) or the one terminal type "result" (seq, status, data, error, effects, data_count)>
<blank line>
```

```text
  client                        rivet serve
    │ POST /v1/request           │
    │ Accept: text/event-stream  │
    │───────────────────────────▶│ validate + authorize
    │                            │   ├─ error BEFORE the first item ─▶ plain JSON error envelope
    │                            │   │                                  with its registry status
    │◀── 200 text/event-stream ──│   └─ ok
    │◀── id:1 event:data ────────│  seq 1..n  one per `emit`
    │◀── id:n+1 event:result ────│  exactly ONE event:result (status ok | error | cancelled), then EOF
    │  (disconnect) ────────────▶│  request aborted: scope closed, handles released
```

Captured `demo.countdown` (`18951`):

```text
$ curl -s -i -N http://127.0.0.1:18951/v1/request -H 'content-type: application/json' -H 'accept: text/event-stream' \
       -d '{"operation":"demo.countdown","data":{}}'
HTTP/1.1 200 OK
content-type: text/event-stream
cache-control: no-cache
traceparent: 00-f766e7c657f95fa0518268ea4b24e225-86ec5e7105f83e98-01
transfer-encoding: chunked

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

A unary operation over SSE (`demo.add {"a":2,"b":3}`):

```text
id: 1
event: result
data: {"request_id":"req_0763ea5813","trace_id":"tr_0763ea5813","operation":"demo.add","type":"result","seq":1,"status":"ok","data":5,"error":null,"effects":"none","data_count":0}
```

A failure **after** the first item (`demo.flaky` on `18950` emits `1`, then reads a missing file) is the terminal
`event: result` with `"status":"error"`:

```text
id: 1
event: data
data: {"request_id":"req_01581bae8d","trace_id":"tr_01581bae8d","operation":"demo.flaky","type":"data","seq":1,"data":1,"error":null}

id: 2
event: result
data: {"request_id":"req_01581bae8d","trace_id":"tr_01581bae8d","operation":"demo.flaky","type":"result","seq":2,"status":"error","data":null,"error":{"kind":"not_found","code":"not_found.file","message":"./data/missing.txt: no such file","retryable":false,"source":{"file":"app.rivet","line":53,"column":5,"end_line":53,"end_column":50},"operation_id":"demo.flaky"},"effects":"none","data_count":1}
```

A failure **before** the first item keeps its registry status and answers a plain JSON envelope, not an event
stream (here `demo.nope` → 404; a unary `demo.read` of a missing file likewise answers the JSON error envelope):

```text
HTTP/1.1 404 Not Found
content-type: application/json

{"request_id":"req_08e3bb31f0","trace_id":"tr_08e3bb31f0","operation":"demo.nope","type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"not_found.operation","message":"no operation `demo.nope`","retryable":false,"operation_id":"demo.nope"},"effects":"none","data_count":0}
```

A unary operation over SSE yields one `event: result` with `seq: 1`. A failure after the stream started is also an `event: result` whose record has `"status":"error"` (or `"cancelled"`), `data: null` and the error object; the 0.1.0 `event: error` no longer exists ([MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md#sse)). `Last-Event-ID` resumption is not supported; use polling sessions to resume. `?pretty=true` is refused on SSE (400 `validation.pretty_stream`).

```text
$ curl -s -i 'http://127.0.0.1:18951/v1/request?pretty=true' -H 'content-type: application/json' \
       -H 'accept: text/event-stream' -d '{"operation":"demo.countdown","data":{}}'
HTTP/1.1 400 Bad Request
content-type: application/json

{
  "request_id": "",
  "trace_id": "",
  "operation": "demo.countdown",
  "type": "result",
  "status": "error",
  "data": null,
  "error": {
    "kind": "validation",
    "code": "validation.pretty_stream",
    "message": "pretty JSON cannot be used with an event stream (Accept: text/event-stream); drop ?pretty=true",
    "retryable": false
  },
  "effects": "none",
  "data_count": 0
}
```

### Polling sessions

Polling is the HTTP projection of `rivet.sessions.*`. Sessions are **principal-owned** (they survive the HTTP connection) and bounded.

```text
            POST /v1/requests {operation, data}
                     │ 202 accepted envelope, data = SessionReceipt (events_url)
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
| Bytes held by all session queues of the host | `limits.max_buffered_bytes` (default 256 MiB) → `limit.buffered_bytes` (429) |
| Total deadline | `deadline_ms` (default 30000, cap 600000) → `timeout.request` |
| Idle lease / retention after terminal | 60 s / 60 s, enforced by a background sweeper (no client call needed): an idle session is cancelled (`cancelled.idle`), a terminal one is evicted |
| `wait_ms` default / max | 1000 / 5000 ms |
| `max_events` default / cap | 16 / 16 |

**Cancel.** `POST …/cancel` fires the session's cancellation token; the run closes its handles within the 5 s grace and the session ends `cancelled`. When the cancel signal arrives before a racing completion, `cancelled` wins. Cancelling a session that already ended does nothing and reports its terminal state (`succeeded`, `failed` or `cancelled`) instead of `cancelled`.

Input sequencing: `send_seq` starts at 1 and increases by one. An identical retry of the most recent sequence is acknowledged again without enqueuing; a gap → 409 `conflict.input_sequence`; input after `finish_input` → 409 `conflict.input_closed`. Another principal's, unknown or expired session → 404 `not_found.session`.

Captured on `18950` (`demo.countdown` emits 3, 2, 1; `chat.echo` receives and emits text, returns `{echoed: N}`):

```text
$ curl -s -X POST http://127.0.0.1:18950/v1/requests -H 'content-type: application/json' -d '{"operation":"demo.countdown","data":{}}'     # 202
{"request_id":"req_02d661b32a","trace_id":"tr_02d661b32a","operation":"demo.countdown","type":"result","status":"accepted","data":{"session_id":"ses_0157bf2ec5","request_id":"req_02d661b32a","trace_id":"tr_02d661b32a","catalog_version":"sha256:d7bc50b83b6b4e5c6eda533ddba582129a8edc30bcafe80ad3844972d6282787","input_schema":null,"emits_schema":{"type":"integer"},"next_send_seq":1,"expires_at":"2026-09-28T21:23:07Z","events_url":"/v1/requests/ses_0157bf2ec5/events"},"error":null,"effects":"none","data_count":0}

$ curl -s 'http://127.0.0.1:18950/v1/requests/ses_0157bf2ec5/events?after_seq=0&wait_ms=2000&max_events=2'
{"session_id":"ses_0157bf2ec5","events":[{"request_id":"req_02d661b32a","trace_id":"tr_02d661b32a","operation":"demo.countdown","type":"data","seq":1,"data":3,"error":null},{"request_id":"req_02d661b32a","trace_id":"tr_02d661b32a","operation":"demo.countdown","type":"data","seq":2,"data":2,"error":null}],"last_seq":2,"terminal":false}
$ curl -s 'http://127.0.0.1:18950/v1/requests/ses_0157bf2ec5/events?after_seq=2&wait_ms=2000'
{"session_id":"ses_0157bf2ec5","events":[{"request_id":"req_02d661b32a","trace_id":"tr_02d661b32a","operation":"demo.countdown","type":"data","seq":3,"data":1,"error":null},{"request_id":"req_02d661b32a","trace_id":"tr_02d661b32a","operation":"demo.countdown","type":"result","seq":4,"status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}],"last_seq":4,"terminal":true}
$ curl -s -X POST http://127.0.0.1:18950/v1/requests/ses_0157bf2ec5/cancel          # after the terminal event: reports it
{"session_id":"ses_0157bf2ec5","request_id":"req_02d661b32a","state":"succeeded"}
$ curl -s -w ' %{http_code}' http://127.0.0.1:18950/v1/requests/ses_bogus/events
{"request_id":"","trace_id":"","operation":"rivet.sessions.read","type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"not_found.session","message":"no session `ses_bogus`","retryable":false},"effects":"none","data_count":0} 404
$ curl -s -w ' %{http_code}' 'http://127.0.0.1:18950/v1/requests/ses_0157bf2ec5/events?after_seq=x'
{"request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.type","message":"query `after_seq` must be a non-negative integer","retryable":false},"effects":"none","data_count":0} 422
```

```text
# duplex: chat.echo
POST /v1/requests {"operation":"chat.echo","data":{}} →
{"request_id":"req_03381cef07","trace_id":"tr_03381cef07","operation":"chat.echo","type":"result","status":"accepted","data":{"session_id":"ses_02bb57670a","request_id":"req_03381cef07","trace_id":"tr_03381cef07","catalog_version":"sha256:d7bc50b83b6b4e5c6eda533ddba582129a8edc30bcafe80ad3844972d6282787","input_schema":{"type":"string"},"emits_schema":{"type":"string"},"next_send_seq":1,"expires_at":"2026-09-28T21:23:07Z","events_url":"/v1/requests/ses_02bb57670a/events"},"error":null,"effects":"none","data_count":0} 202
POST …/input {"send_seq":1,"data":"hi"} →
{"session_id":"ses_02bb57670a","accepted_seq":1,"input_closed":false} 200
POST …/input {"send_seq":1,"data":"hi"} →                       (identical retry: acknowledged, not enqueued)
{"session_id":"ses_02bb57670a","accepted_seq":1,"input_closed":false} 200
POST …/input {"send_seq":3,"data":"x"} →
{"request_id":"req_03381cef07","trace_id":"tr_03381cef07","operation":"rivet.sessions.send","type":"result","status":"error","data":null,"error":{"kind":"conflict","code":"conflict.input_sequence","message":"expected send_seq 2, got 3","retryable":false},"effects":"none","data_count":0} 409
POST …/finish_input →
{"session_id":"ses_02bb57670a","accepted_seq":null,"input_closed":true} 200
POST …/input {"send_seq":2,"data":"late"} →
{"request_id":"req_03381cef07","trace_id":"tr_03381cef07","operation":"rivet.sessions.send","type":"result","status":"error","data":null,"error":{"kind":"conflict","code":"conflict.input_closed","message":"input is finished; no further sends are accepted","retryable":false},"effects":"none","data_count":0} 409
GET …/events?after_seq=0&wait_ms=2000 →
{"session_id":"ses_02bb57670a","events":[{"request_id":"req_03381cef07","trace_id":"tr_03381cef07","operation":"chat.echo","type":"data","seq":1,"data":"hi","error":null},{"request_id":"req_03381cef07","trace_id":"tr_03381cef07","operation":"chat.echo","type":"result","seq":2,"status":"ok","data":{"echoed":1},"error":null,"effects":"none","data_count":1}],"last_seq":2,"terminal":true} 200

# cancel of a running duplex session, then its terminal record
POST …/cancel →
{"session_id":"ses_04b2b2ef04","request_id":"req_0533ef7701","state":"cancelled"} 200
GET …/events?after_seq=0&wait_ms=1000 →
{"session_id":"ses_04b2b2ef04","events":[{"request_id":"req_0533ef7701","trace_id":"tr_0533ef7701","operation":"chat.echo","type":"result","seq":1,"status":"cancelled","data":null,"error":{"kind":"cancelled","code":"cancelled.session","message":"the session was cancelled","retryable":false},"effects":"none","data_count":0}],"last_seq":1,"terminal":true} 200

# 0.1.0 body on the polling route: accepted, with the deprecation signal
POST /v1/requests {"id":"demo.countdown","params":{}} → HTTP/1.1 202 Accepted, deprecation: true
```

## Error Format

Every non-2xx JSON response is a ResponseEnvelope with `status: "error"` (or `"cancelled"`), `data: null` and the error object ([API-2026-0006](api-2026-0006-envelopes.md#error-format)); `effects` sits at the top level:

```text
$ curl -s -i 'http://127.0.0.1:18951/v1/request?pretty=true' -H 'content-type: application/json' -d '{"operation":"demo.add","data":{"a":"two"}}'
HTTP/1.1 422 Unprocessable Entity
content-type: application/json
traceparent: 00-97d9f90b56bc783093e7ce97729f302d-64a6c4060207ace8-01

{
  "request_id": "req_099d4c589d",
  "trace_id": "tr_099d4c589d",
  "operation": "demo.add",
  "type": "result",
  "status": "error",
  "data": null,
  "error": {
    "kind": "validation",
    "code": "validation.type",
    "message": "parameter `a` must be an integer, got text",
    "retryable": false,
    "operation_id": "demo.add",
    "details": {
      "field": "a"
    }
  },
  "effects": "none",
  "data_count": 0
}
```

`request_id` / `trace_id` are empty strings when the failure happened before a request existed (auth, routing, body parsing); `operation` is then the built-in or requested ID, or `null` when the input named none. Optional error fields: `source` (file/line/column span), `operation_id`, `node_id`, `hint`, `details`, `cause`, `suppressed`. The HTTP status comes from the error **kind** (full table in [API-2026-0005](api-2026-0005-error-registry.md)):

| Status | Kinds / codes |
|---|---|
| 400 | `validation.malformed_json`, `validation.pretty_stream` |
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

Captured failures (`18951`, demo 01-catalog):

```text
POST /v1/request {nope
{"request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.malformed_json","message":"request body is not valid JSON: key must be a string at line 1 column 2","retryable":false},"effects":"none","data_count":0}   → HTTP 400
POST /v1/request {"operation":"demo.add","data":{"a":"x"}}
{"request_id":"req_101e9dfdd2","trace_id":"tr_101e9dfdd2","operation":"demo.add","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.type","message":"parameter `a` must be an integer, got text","retryable":false,"operation_id":"demo.add","details":{"field":"a"}},"effects":"none","data_count":0}   → HTTP 422
POST /v1/request {"operation":"demo.countdown"}
{"request_id":"","trace_id":"","operation":"demo.countdown","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"stream.required","message":"`demo.countdown` streams; use Accept: text/event-stream, POST /v1/requests, /v1/ws or rivet.sessions.open","retryable":false},"effects":"none","data_count":0}   → HTTP 422
POST /v1/request {"operation":"demo.nope"}
{"request_id":"req_119fcd62e7","trace_id":"tr_119fcd62e7","operation":"demo.nope","type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"not_found.operation","message":"no operation `demo.nope`","retryable":false,"operation_id":"demo.nope"},"effects":"none","data_count":0}   → HTTP 404
POST /v1/request [1]
{"request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.input_envelope","message":"the input envelope must be a JSON object","retryable":false},"effects":"none","data_count":0}   → HTTP 422
POST /v1/request {"operation":"demo.add","id":"demo.add"}
{"request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.input_envelope","message":"use `operation` or the deprecated `id`, not both","retryable":false,"details":{"key":"operation","alias":"id"}},"effects":"none","data_count":0}   → HTTP 422
POST /v1/request {"operation":"demo.add","data":{"a":1},"deadline_ms":"soon"}
{"request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.input_envelope","message":"`deadline_ms` must be a non-negative integer","retryable":false},"effects":"none","data_count":0}   → HTTP 422
POST /v1/request {"operation":"demo.add","data":{"a":1},"error":{}}
{"request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.input_envelope","message":"an input envelope has no `error` key (errors only appear in responses)","retryable":false},"effects":"none","data_count":0}   → HTTP 422
POST /v1/request (empty body) · {"data":{"a":1}}
{"request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.required","message":"the input envelope needs `operation`","retryable":false,"details":{"field":"operation"}},"effects":"none","data_count":0}   → HTTP 422
GET  /v1/operations/nope
{"request_id":"","trace_id":"","operation":"rivet.describe","type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"not_found.operation","message":"no operation `nope`","retryable":false},"effects":"none","data_count":0}   → HTTP 404
GET  /v1/nothing
{"request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"not_found.route","message":"no such route on this listener","retryable":false},"effects":"none","data_count":0}   → HTTP 404
```

A rejected input envelope answers `operation: null` even when it named an operation (the `deadline_ms` and
`error` rows above named `demo.add`); match on `error.code`, not on `operation`, for these refusals.

## Rate Limits

There is no per-client rate limiter. Bounded resources answer `429 limit.*` with `retryable: true`:

- `limits.max_concurrent_requests` in policy.json (default 64) → `limit.concurrency` for top-level requests beyond the budget (refused immediately, not queued);
- 8 open sessions per principal → `limit.sessions`;
- `limits.max_call_depth` (default 16) → `limit.call_depth` for nested `(request …)` chains;
- `limits.max_buffered_bytes` (default 256 MiB) → `limit.buffered_bytes` when the retained events of all sessions of the host would exceed it.

Rivet sets no explicit HTTP request-body limit; the framework default of the axum server applies to request bodies. Outbound adapters default to 8 MiB per response body, stream item, socket/QUIC frame and process output (see [SYS-2026-0005](../system/integrations/sys-2026-0005-protocol-adapters.md)).

## Versioning and Deprecation

The path prefix `/v1` versions the HTTP contract. 0.2.0 is the one exception to the prefix rule, approved in PROP-2026-0002: every `/v1` body became an envelope while the routes stayed (the project had no external clients; [MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md) maps each shape).

```text
 0.1.x  {id, params} in · {"result":…} / {request_id, trace_id, error} out · SSE event: error
 0.2.0  {operation, data} in · envelopes out · SSE event: data | result
        {id, params} still accepted ─▶ response header `deprecation: true` + access log "deprecated":1
 0.3.0  {id, params} refused (validation.input_envelope, with a hint)
```

## Examples

End to end with curl against a loopback server (the scratch `demo.read` bundle; auth `none`; a fresh process on
`18952` so the access log below is complete):

```text
$ rivet --file app.rivet serve --listen 127.0.0.1:18952 &
{"listen_addr":"127.0.0.1:18952","stdio":false,"surfaces":["http","sse","poll","ws","mcp"],"auth_type":"none","catalog_version":"sha256:d7bc50b83b6b4e5c6eda533ddba582129a8edc30bcafe80ad3844972d6282787","policy_hash":"sha256:d33d79eb7d533e53de04c36a33e11c31451a5da5670a9e34daca4f97751997f1"}

$ curl -s http://127.0.0.1:18952/v1/request -H 'content-type: application/json' -d '{"operation":"demo.read","data":{"path":"data/a.txt"},"deadline_ms":900000}'   # deadline clamped to 600000
{"request_id":"req_019d7ffbd5","trace_id":"tr_019d7ffbd5","operation":"demo.read","type":"result","status":"ok","data":"hello world!","error":null,"effects":"none","data_count":0}

$ curl -s -i http://127.0.0.1:18952/v1/request -H 'content-type: application/json' -d '{"id":"demo.read","params":{"path":"data/a.txt"}}'   # 0.1.0 body: deprecated
HTTP/1.1 200 OK
content-type: application/json
traceparent: 00-0704547fc57f5727c1f31021d5d056f2-a53c4609c12ec05f-01
deprecation: true

{"request_id":"req_021db3baba","trace_id":"tr_021db3baba","operation":"demo.read","type":"result","status":"ok","data":"hello world!","error":null,"effects":"none","data_count":0}

$ curl -s -i http://127.0.0.1:18952/v1/health
HTTP/1.1 200 OK
content-type: application/json
traceparent: 00-9b1a1e95b4d67fd4870aec50f28c0a31-dd05364efca207bc-01

{"request_id":"req_039e215277","trace_id":"tr_039e215277","operation":"rivet.health","type":"result","status":"ok","data":{"status":"ok","catalog_version":"sha256:d7bc50b83b6b4e5c6eda533ddba582129a8edc30bcafe80ad3844972d6282787","version":"0.2.0"},"error":null,"effects":"none","data_count":0}

$ curl -s -i http://127.0.0.1:18952/v1/request -H 'content-type: application/json' -H 'traceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01' -d '{"operation":"demo.read","data":{"path":"data/a.txt"}}'
HTTP/1.1 200 OK
content-type: application/json
traceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-2c62e9adf172c086-01

{"request_id":"req_0419e663c4","trace_id":"4bf92f3577b34da6a3ce929d0e0e4736","operation":"demo.read","type":"result","status":"ok","data":"hello world!","error":null,"effects":"none","data_count":0}

$ curl -s http://127.0.0.1:18952/v1/request -H 'content-type: application/json' -d '{"operation":"demo.read","data":{"path":"data/a.txt"},"restrict":{"grants":[{"capability":"allow_read","targets":["./data/other/**"],"access":["read"]}]}}'   # 403
{"request_id":"req_0599d1cc19","trace_id":"tr_0599d1cc19","operation":"demo.read","type":"result","status":"error","data":null,"error":{"kind":"permission","code":"permission.denied","message":"allow_read read on data/a.txt denied: request restriction: no grant for allow_read data/a.txt","retryable":false,"source":{"file":"app.rivet","line":44,"column":5,"end_line":44,"end_column":34},"operation_id":"demo.read","details":{"capability":"allow_read","access":"read","target":"data/a.txt"}},"effects":"none","data_count":0}

$ curl -s http://127.0.0.1:18952/v1/request -H 'content-type: application/json' -d '{"operation":"demo.read","data":{"path":"app.rivet"},"restrict":{"grants":[{"capability":"allow_read","targets":["./**"],"access":["read"]}]}}'   # 403: never widens
{"request_id":"req_061d3ec1be","trace_id":"tr_061d3ec1be","operation":"demo.read","type":"result","status":"error","data":null,"error":{"kind":"permission","code":"permission.denied","message":"allow_read read on app.rivet denied: no grant for allow_read app.rivet","retryable":false,"source":{"file":"app.rivet","line":44,"column":5,"end_line":44,"end_column":34},"operation_id":"demo.read","details":{"capability":"allow_read","access":"read","target":"app.rivet"}},"effects":"none","data_count":0}

$ curl -s http://127.0.0.1:18952/v1/request -H 'content-type: application/json' -d '{"operation":"demo.read","data":{"path":"data/a.txt"},"restrict":{"bogus":1}}'   # 422
{"request_id":"req_079c557633","trace_id":"tr_079c557633","operation":"demo.read","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"policy.invalid","message":"restrict accepts only `grants` (got `bogus`); it can narrow, never grant","retryable":false,"operation_id":"demo.read","details":{"pointer":"/restrict/bogus"}},"effects":"none","data_count":0}

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

$ kill -TERM %1        # drains, then exits 0
```

The start-up receipt (first line) is written to stderr, followed by **one JSON access-log line per request** on stderr — `{time, surface, method, route, principal, operation, status, duration_ms}`, plus `"deprecated":1` when the input used `id`/`params`; the route is the matched pattern, and params, bodies, query strings and tokens are never logged. The stderr of the session above:

```text
{"listen_addr":"127.0.0.1:18952","stdio":false,"surfaces":["http","sse","poll","ws","mcp"],"auth_type":"none","catalog_version":"sha256:d7bc50b83b6b4e5c6eda533ddba582129a8edc30bcafe80ad3844972d6282787","policy_hash":"sha256:d33d79eb7d533e53de04c36a33e11c31451a5da5670a9e34daca4f97751997f1"}
{"time":"2026-09-28T21:22:57.406Z","surface":"http","method":"POST","route":"/v1/request","principal":"local","operation":"demo.read","status":200,"duration_ms":0}
{"time":"2026-09-28T21:22:57.421Z","surface":"http","method":"POST","route":"/v1/request","principal":"local","operation":"demo.read","status":200,"duration_ms":0,"deprecated":1}
{"time":"2026-09-28T21:22:57.435Z","surface":"http","method":"GET","route":"/v1/health","principal":null,"operation":"rivet.health","status":200,"duration_ms":0}
{"time":"2026-09-28T21:22:57.449Z","surface":"http","method":"POST","route":"/v1/request","principal":"local","operation":"demo.read","status":200,"duration_ms":0}
{"time":"2026-09-28T21:22:57.464Z","surface":"http","method":"POST","route":"/v1/request","principal":"local","operation":"demo.read","status":403,"duration_ms":0}
{"time":"2026-09-28T21:22:57.477Z","surface":"http","method":"POST","route":"/v1/request","principal":"local","operation":"demo.read","status":403,"duration_ms":0}
{"time":"2026-09-28T21:22:57.491Z","surface":"http","method":"POST","route":"/v1/request","principal":"local","operation":"demo.read","status":422,"duration_ms":0}
{"time":"2026-09-28T21:22:57.507Z","surface":"http","method":"POST","route":"/v1/request","principal":"local","operation":"demo.add","status":200,"duration_ms":0}
```

```text
 monitoring the 0.2.x deprecation window from the access log
   rivet serve 2> access.log
   grep -c '"deprecated":1' access.log          → clients still sending {id, params}
   jq -r 'select(.deprecated==1) | "\(.principal) \(.route) \(.operation)"' access.log | sort | uniq -c
```

**Shutdown.** SIGINT (Ctrl-C) and SIGTERM drain the same way: stop accepting, cancel every in-flight request and session (sessions end `cancelled.shutdown`; each run closes its handles within the 5 s grace), let in-flight responses flush (up to 6 s), then exit **0** (the `kill -TERM` above exited 0).

Refused starts, captured from `docs/demos/01-catalog` (nothing listens; the error envelope goes to stderr):

```text
$ rivet --file app.rivet serve --listen 0.0.0.0:18956
{"request_id":"","trace_id":"","operation":"rivet.serve","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"serve.auth_required","message":"non-loopback listener requires serve.auth in policy.json","retryable":false},"effects":"none","data_count":0}
$ echo $?
2

$ rivet --file app.rivet --policy mtls.json serve --listen 127.0.0.1:18957     # serve.auth {"type":"mtls","client_ca":"./ca.pem"}
{"request_id":"","trace_id":"","operation":"rivet.serve","type":"result","status":"error","data":null,"error":{"kind":"unsupported","code":"unsupported.serve_mtls","message":"serve.auth type mtls needs a TLS listener, which this build does not provide yet; use bearer behind a TLS-terminating proxy","retryable":false},"effects":"none","data_count":0}
$ echo $?
5
```

A binary built without the `serve` Cargo feature (`cargo build --no-default-features --features cli`) keeps the
command and refuses it:

```text
$ rivet --file app.rivet serve --listen 127.0.0.1:18958
{"request_id":"","trace_id":"","operation":"rivet.serve","type":"result","status":"error","data":null,"error":{"kind":"unsupported","code":"unsupported.feature","message":"`rivet serve` needs the `serve` feature, which this build was compiled without (rebuild rivet-runtime with `--features serve`)","retryable":false,"details":{"feature":"serve"}},"effects":"none","data_count":0}
$ echo $?
5
```

The remote CLI (`rivet --endpoint URL [--token-file PATH] …`) is a client of these same routes: `request` → `POST /v1/request` (SSE with `--stream`), `list/describe/outputs` → `GET /v1/operations…`, `io` → `GET /v1/io`, `auth`/`trace show`/`trace export`/`connectors sync` → `POST /v1/request` with the built-in ID, and `--input-jsonl -` → `/v1/ws`.

## Compatibility Notes

- HTTP/1.1 and HTTP/2 (cleartext) on the listener; TLS termination is the job of a front proxy.
- 0.2.0 changed every body to an envelope (breaking for readers, compatible for senders through 0.2.x): see [MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md).
- A build without the `serve` Cargo feature has no listener: `rivet serve` exits 5 with `unsupported.feature` (`details.feature: "serve"`), captured above.
- Platforms: macOS and Linux (CI green on both). Windows is not supported in 0.2.0 ([INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)).
- Request IDs look like `req_NN…` and trace IDs `tr_NN…`; treat both as opaque strings.
- W3C `traceparent` is accepted on every HTTP route (REST, SSE, polling, the WebSocket upgrade and `/mcp`) and emitted on responses that belong to a request.
- Known limitations: no mTLS listener (`unsupported.serve_mtls`, exit 5), no persistent trace store (traces live in the serving process), no `Last-Event-ID` resume on SSE, no pagination (`next_cursor` is always `null`); see the [manual's limitations chapter](../manuals/man-2026-0001-rivet-manual.md#known-limitations).

## Related Documents

- [API index](index.md) · [WebSocket rivet.v1](api-2026-0002-websocket-rivet-v1.md) · [MCP server tools](api-2026-0003-mcp-server-tools.md) · [Error registry](api-2026-0005-error-registry.md)
- [Runtime architecture](../architecture/arch-2026-0001-rivet-runtime-architecture.md) · [Policy and sandbox model](../security/sec-2026-0001-policy-and-sandbox-model.md)
- [API-2026-0006 envelopes](api-2026-0006-envelopes.md) · [MIG-2026-0001 migration](../migrations/mig-2026-0001-response-and-input-envelopes.md) · [MAN-2026-0006 serving and surfaces](../manuals/man-2026-0006-serving-and-surfaces.md)
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) · [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md) · [Demo 01-catalog](../demos/01-catalog/README.md) · [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 4 | 2026-09-30 | Claude | v0.2.0 release (PLAN-2026-0002 TASK-091): version strings and current-release wording updated to 0.2.0. |
| 1 | 2026-09-28 | Claude | Initial current-state contract for REST, SSE and polling, with examples captured from `rivet serve` at commit f40d4aa. |
| 2 | 2026-09-28 | Claude | Fix batch through 829ca43 and 2a751ab (`rivet.trace.export` dispatched with `{request_id, path}`; emits/receives descriptions): `GET /v1/health`, per-request `restrict`, W3C `traceparent` in/out, access-log line, SIGTERM drain, bare `/v1/io` manifest, polling `deadline_ms`/`restrict`, background sweeper, cancel of a finished session, `limit.buffered_bytes`, `rivet.capabilities` and `rivet.trace.export` (known dispatcher defect) built-ins, five sensitive IDs. |
| 3 | 2026-09-29 | Claude | 0.2.0 envelope sweep (D-40, TASK-070): every example re-captured on the 0.2.0-rc (source `6f9943f`, ports 18950–18958) — envelopes on REST, SSE (incl. a mid-stream failure), polling (incl. cancel and a legacy body), catalog, io, policy generate, health and errors; InputEnvelope fields and refusals; `?pretty=true` and `validation.pretty_stream`; `deprecation` header and access-log `deprecated` monitoring; SSE event names data/result; `unsupported.feature` without `serve`; macOS/Linux platform note; versioning section |
