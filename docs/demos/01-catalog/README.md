---
document_id: DEMO-2026-0001
title: "One file, four operations, every access point"
document_type: demo
status: active
created_date: 2026-09-28
last_updated: 2026-09-30
document_revision: 10
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [registry, cli, serve, http, poll, ws, mcp, sessions, auth]
affected_versions:
  from: "0.2.0"
  to: null
applicable_environments: [development]
audience: [developers, reviewers]
scope: One app.rivet with four pure, described operations, called from the CLI and from one `rivet serve` over REST, SSE, polling, WebSocket and MCP, with bearer authentication through an alternate policy file.
reason: User requested sample folders with READMEs showing usage; UQ-17 adds declared outputs, policy.json-only policy and one serve for every surface; UQ-18 adds the generated I/O manifest. TASK-067 executed every step against the 0.1.0 release candidate; TASK-076 (PLAN-2026-0002) re-executed it against the 0.2.0 release candidate (ResponseEnvelope/InputEnvelope wire, `--data`, `--input`, `--pretty`).
dependencies: [PROP-2026-0001, REF-2026-0002]
related_documents: [PLAN-2026-0001, PLAN-2026-0002, DEMO-2026-0015, DEMO-2026-0020, API-2026-0006, MIG-2026-0001, DEMO-2026-0013, MAN-2026-0002, MAN-2026-0006, API-2026-0001, API-2026-0002, API-2026-0003, TEST-2026-0002, TEST-2026-0020, TEST-2026-0022]
supersedes: null
superseded_by: null
tags: [rivet, demo, catalog, serve, rest, sse, polling, websocket, mcp]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-28
verified_against: "0.2.0"
---

# One file, four operations, every access point

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-30
> **Affected Versions:** 0.2.0 and later
> **Owner:** Project maintainer
> **Affected Components:** registry, cli, serve, http, poll, ws, mcp, sessions, auth

## Purpose

One [app.rivet](app.rivet) declares four pure operations, each with a declared, described output. One `rivet serve` then exposes the same catalog through REST, SSE, polling, WebSocket and MCP at once. Delivery stage: **A; MCP in B**.

| Operation ID | Output | Behavior |
|---|---|---|
| `demo.greet` | `text` | Return a greeting. |
| `demo.add` | `integer` | Add two integers; `b` defaults to zero. |
| `demo.health` | `object {ready}` | Return a pure readiness response. |
| `demo.countdown` | `object {count}`, emits `integer` | Emit 3, 2, 1, then return a summary. Used for the streaming surfaces. |

```text
                app.rivet (4 operations, no policy.json)
                               |
           +-------------------+--------------------+
           |                                        |
     rivet CLI (one process)              rivet serve --listen 127.0.0.1:8080
     list / describe / outputs /                    |
     request / graph                 REST  SSE  poll  WS  MCP  (one listener)
           |                                        |
           +------------ same immutable catalog ----+
```

## Verified Against Version

0.2.0. Steps 7 and 8 were re-run on 2026-09-29 at commit `7c25175` (source = `14750b8`) after the INC-2026-0012 fixes; their output above is from that run. The other steps were verified on 0.2.0-dev at commit `8031baa`, the release candidate (the version string is bumped from 0.1.0 to 0.2.0 at release, P5, so `rivet --version` and `serverInfo.version` still print 0.1.0), with `target/release/rivet` on macOS 26.4.1 (Darwin 25.4.0, arm64), 2026-09-29. Every output block below was pasted from that run; the verification run listened on `127.0.0.1:18800` instead of 8080. Request, trace and session IDs, timestamps, hashes and ports vary from run to run.

What changed from 0.1.0 ([migration guide](../../migrations/mig-2026-0001-response-and-input-envelopes.md), [envelope reference](../../api/api-2026-0006-envelopes.md)):

```text
  0.1.0                                         0.2.0
  ─────────────────────────────────────────     ──────────────────────────────────────────────────────
  input   {"id": ID, "params": {...}}       ──▶  {"operation": ID, "data": {...}}     (old keys: deprecated aliases)
  CLI     --params JSON                     ──▶  --data JSON · --input FILE|- · --pretty
  output  {request_id, trace_id, result}    ──▶  {request_id, trace_id, operation, type, status,
          {request_id, trace_id, error}           data, error, effects, data_count}   (one shape everywhere)
  SSE     event: error                      ──▶  event: result  + "status": "error"
  WS      {"type":"result","completion":…}  ──▶  the envelope itself, plus "ref"
  MCP     structuredContent = Completion    ──▶  structuredContent = ResponseEnvelope
```

## Prerequisites

Build Rivet and put it on `PATH` (from the repository root):

```sh
cargo build --release --features cli
export PATH="$PWD/target/release:$PATH"     # rivet --version prints rivet 0.2.0
```

`curl` is needed for the HTTP steps. The WebSocket step uses the small client in [fixtures/ws_client.py](fixtures/ws_client.py), which needs the `websockets` Python package (any WebSocket client that can set the `rivet.v1` subprotocol works, for example websocat):

```sh
python3 -m venv "$TMPDIR/rivet-demo-venv"
"$TMPDIR/rivet-demo-venv/bin/pip" install websockets
```

Port 8080 must be free on 127.0.0.1. If it is not, use another port in `--listen` and in every URL below.

## Setup

```sh
cd docs/demos/01-catalog
```

No data files or remote fixtures are needed. The four operations are pure.

**This folder deliberately has no `policy.json`.** When no policy file sits next to the entry `.rivet` file (and no `--policy PATH` is passed), Rivet is **deny-by-default** for all application I/O. Pure operations still run; any read, write, network, environment or credential effect fails with `permission.denied` (exit 3). There is no implicit allow-all mode.

```text
  rivet --file app.rivet ...
        |
        +-- --policy PATH given? ----yes----> load that file (path only, never grants)
        |          no
        +-- ./policy.json next to app.rivet? --yes--> load it
        |          no
        +-- deny-by-default: pure ops run, every application effect is denied
```

The alternate file [policies/team.json](policies/team.json) is used only in the authentication step (step 9).

## Steps

### 1. Discover and call from the CLI

#### Command / Request

```sh
rivet --file app.rivet list --outputs
rivet --file app.rivet describe demo.add --json
rivet --file app.rivet request demo.add --data '{"a":2,"b":3}'
rivet --file app.rivet request demo.greet --data '{"person":"Ada"}'
rivet --file app.rivet request demo.health
rivet --file app.rivet request demo.countdown --stream
```

`--data` defaults to `{}`, so operations without parameters need no flag. The same call can come from a whole input envelope (`--input FILE`, or `-` for stdin), and `--pretty` indents one envelope:

```sh
echo '{"operation":"demo.add","data":{"a":40,"b":2}}' | rivet --file app.rivet request --input -
rivet --file app.rivet request demo.add --data '{"a":2,"b":3}' --pretty
```

#### Expected Output / Response

```text
ID              NAME                OUTPUT      DESCRIPTION
demo.greet      Greet a person      text        Return a greeting for the supplied person.
demo.add        Add two integers    integer     Add two signed integers and return their sum.
demo.health     Check availability  object      Return a constant readiness response without I/O.
demo.countdown  Count down          object      Emit 3, 2, 1 as data items and then return a summary.
```

`describe --json` is a ResponseEnvelope of the built-in `rivet.describe`; the descriptor is its `data`:

```json
{"request_id":"req_018241671d","trace_id":"tr_018241671d","operation":"rivet.describe","type":"result","status":"ok","data":{"id":"demo.add","name":"Add two integers","description":"Add two signed integers and return their sum.","kind":"operation","input":{"type":"object","properties":{"a":{"type":"integer","description":"First operand."},"b":{"type":"integer","description":"Second operand; defaults to zero.","default":0}},"required":["a"],"additionalProperties":false},"output":{"type":"integer","description":"Sum of a and b."},"emits":null,"receives":null,"errors":[],"delivery":"unary","source":{"file":"app.rivet","line":9}},"error":null,"effects":"none","data_count":0}
```

Each `request` prints one ResponseEnvelope line and exits 0. The keys always come in this order, and `data` and `error` are always present (exactly one is non-null):

```json
{"request_id":"req_0182f81705","trace_id":"tr_0182f81705","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}
{"request_id":"req_018137eadd","trace_id":"tr_018137eadd","operation":"demo.greet","type":"result","status":"ok","data":"Hello, Ada!","error":null,"effects":"none","data_count":0}
{"request_id":"req_018068dcdd","trace_id":"tr_018068dcdd","operation":"demo.health","type":"result","status":"ok","data":{"ready":true},"error":null,"effects":"none","data_count":0}
```

```text
  request_id  trace_id  operation  type            status                              data   error   effects  data_count
  ──────────  ────────  ─────────  ──────────────  ──────────────────────────────────  ─────  ──────  ───────  ──────────
  req_…       tr_…      demo.add   result | data   ok | error | cancelled | accepted   5      null    none     0
```

The `--input -` call prints `…"operation":"demo.add","type":"result","status":"ok","data":42,…`, and `--pretty` prints the same envelope indented by two spaces, keys in the same order:

```json
{
  "request_id": "req_017f286a1d",
  "trace_id": "tr_017f286a1d",
  "operation": "demo.add",
  "type": "result",
  "status": "ok",
  "data": 5,
  "error": null,
  "effects": "none",
  "data_count": 0
}
```

`--stream` prints NDJSON: one `type: data` record per item (with `seq`, without `status`/`effects`), then the terminal `type: result` record:

```json
{"request_id":"req_018089f425","trace_id":"tr_018089f425","operation":"demo.countdown","type":"data","seq":1,"data":3,"error":null}
{"request_id":"req_018089f425","trace_id":"tr_018089f425","operation":"demo.countdown","type":"data","seq":2,"data":2,"error":null}
{"request_id":"req_018089f425","trace_id":"tr_018089f425","operation":"demo.countdown","type":"data","seq":3,"data":1,"error":null}
{"request_id":"req_018089f425","trace_id":"tr_018089f425","operation":"demo.countdown","type":"result","seq":4,"status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}
```

**Failing examples.** Omitting `a` fails validation before execution (exit 2); an unknown ID is `not_found.operation` (exit 4). Errors use the same envelope with `status: "error"` and `data: null`:

```sh
rivet --file app.rivet request demo.add --data '{"b":3}'
rivet --file app.rivet request demo.nope
```

```json
{"request_id":"req_018e167b1d","trace_id":"tr_018e167b1d","operation":"demo.add","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.required","message":"missing required parameter `a`","retryable":false,"operation_id":"demo.add","details":{"field":"a"}},"effects":"none","data_count":0}
{"request_id":"req_018d5928c5","trace_id":"tr_018d5928c5","operation":"demo.nope","type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"not_found.operation","message":"no operation `demo.nope`","retryable":false,"operation_id":"demo.nope"},"effects":"none","data_count":0}
```

**Deprecated 0.1.0 input.** `--params` still works in 0.2.x and prints a warning on stderr; mixing it with `--data`, or `operation` with `id` in one envelope, is refused (exit 2). `--pretty` with `--stream` is refused too, because NDJSON must stay one record per line (exit 2):

```sh
rivet --file app.rivet request demo.add --params '{"a":2,"b":3}'
echo '{"operation":"demo.add","id":"demo.add","data":{"a":1}}' | rivet --file app.rivet request --input -
rivet --file app.rivet request demo.countdown --stream --pretty
```

```text
warning[deprecated.params]: --params is deprecated; use --data (removed in 0.3.0)
{"request_id":"req_017e45e80d","trace_id":"tr_017e45e80d","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}
{"request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.input_envelope","message":"use `operation` or the deprecated `id`, not both","retryable":false,"details":{"key":"operation","alias":"id"}},"effects":"none","data_count":0}
{
  "request_id": "",
  "trace_id": "",
  "operation": "demo.countdown",
  "type": "result",
  "status": "error",
  "data": null,
  "error": {
    "kind": "validation",
    "code": "validation.usage",
    "message": "--pretty cannot be used with --stream: NDJSON records must stay one per line",
    "retryable": false
  },
  "effects": "none",
  "data_count": 0
}
```

The build's own capability report is a built-in operation too. 0.2.0 adds `build_features` (the compiled Cargo features) and `abi_version` (the C ABI major version). Output abbreviated:

```sh
rivet --file app.rivet request rivet.capabilities
```

```json
{"request_id":"req_018b6d075d","trace_id":"tr_018b6d075d","operation":"rivet.capabilities","type":"result","status":"ok","data":{"version":"0.2.0","platform":{"os":"macos","arch":"aarch64"},"stages":{"A":"supported","B":"supported","C":"unsupported"},"features":[{"name":"http","stage":"A","support":"supported","versions":["1.1","2","3"],"streaming":["sse","jsonl","lines","bytes"]}, …],"sandbox":{"backend":"macos-seatbelt","status":"active","reason":"Seatbelt via /usr/bin/sandbox-exec with a deny-default profile"},"serve":{"surfaces":["cli","http","sse","poll","websocket","mcp","library"],"auth":["none","bearer"]},"build_features":["serve","grpc","quic","oauth","cli"],"abi_version":1},"error":null,"effects":"none","data_count":0}
```

### 2. View outputs

#### Command / Request

```sh
rivet --file app.rivet outputs demo.health
rivet --file app.rivet outputs --all --json
```

#### Expected Output / Response

```text
demo.health — Check availability
output  object   Readiness report for this catalog.
  ready   boolean  required  True whenever the host can run pure operations.
emits    —
receives —
errors   —
```

`outputs --all --json` prints one `rivet.outputs` envelope whose `data` holds one entry per public operation, sorted by ID, on one line (wrapped here):

```json
{"request_id":"req_018ad690cd","trace_id":"tr_018ad690cd","operation":"rivet.outputs","type":"result","status":"ok","data":[
 {"id":"demo.add","output":{"type":"integer","description":"Sum of a and b."},"emits":null,"receives":null,"errors":[]},
 {"id":"demo.countdown","output":{"type":"object","properties":{"count":{"type":"integer","description":"Number of items emitted."}},"required":["count"],"additionalProperties":false,"description":"Summary returned after the last item."},"emits":{"type":"integer","description":"One countdown value per item."},"receives":null,"errors":[]},
 {"id":"demo.greet","output":{"type":"string","description":"Greeting that contains the person's name."},"emits":null,"receives":null,"errors":[]},
 {"id":"demo.health","output":{"type":"object","properties":{"ready":{"type":"boolean","description":"True whenever the host can run pure operations."}},"required":["ready"],"additionalProperties":false,"description":"Readiness report for this catalog."},"emits":null,"receives":null,"errors":[]}
 ],"error":null,"effects":"none","data_count":0}
```

`GET /v1/operations/demo.add/outputs` and the MCP `rivet.outputs` tool return the same entry for one ID, inside a `rivet.outputs` envelope. The `emits` entry carries the item type and its description.

### 3. Start one server for every surface

#### Command / Request

In a dedicated terminal (from this folder):

```sh
rivet --file app.rivet serve --listen 127.0.0.1:8080
```

#### Expected Output / Response

The first stderr line is the startup record; every request then adds one JSON access-log line on stderr. A request that used the deprecated `id`/`params` keys is marked `"deprecated":1`, so operators can find 0.1.0 clients:

```json
{"listen_addr":"127.0.0.1:8080","stdio":false,"surfaces":["http","sse","poll","ws","mcp"],"auth_type":"none","catalog_version":"sha256:67104f0e7faaeafee253db758a668a9b24aa4263677e9d5ea2451b32019f9730","policy_hash":null}
{"time":"2026-09-28T21:15:49.910Z","surface":"http","method":"GET","route":"/v1/health","principal":null,"operation":"rivet.health","status":200,"duration_ms":0}
{"time":"2026-09-28T21:15:50.065Z","surface":"http","method":"POST","route":"/v1/request","principal":"local","operation":"demo.add","status":200,"duration_ms":0,"deprecated":1}
```

`--listen 127.0.0.1:8080` is the default and may be omitted. With no policy.json the server binds loopback only, uses auth `none` (principal `local`), mounts all five surfaces, runs pure operations and denies every effect.

```text
                           rivet serve --listen 127.0.0.1:8080
                                         |
      +----------------+-----------------+------------------+-----------------+
      |                |                 |                  |                 |
   REST (http)      SSE (sse)       polling (poll)     WebSocket (ws)     MCP (mcp)
 POST /v1/request  POST /v1/request POST /v1/requests   GET /v1/ws        POST|GET|DELETE /mcp
 GET /v1/operations Accept: text/   GET /v1/requests/   subprotocol       Streamable HTTP
 GET /v1/operations/{id}  event-stream   {id}/events     rivet.v1         (GET → 405)
 GET /v1/operations/{id}/outputs          ?after_seq&wait_ms
 GET /v1/health                       POST /v1/requests/{id}/input
                                      POST /v1/requests/{id}/finish_input
                                      POST /v1/requests/{id}/cancel
      +----------------+-----------------+------------------+-----------------+
                                         |
                   one auth -> principal -> operation authorization
                                         |
                     shared dispatcher / immutable catalog / broker
```

Run every following step from a second terminal in this folder.

### 4. REST

#### Command / Request

```sh
curl -sS http://127.0.0.1:8080/v1/health
curl -sS http://127.0.0.1:8080/v1/operations
curl -sS http://127.0.0.1:8080/v1/operations/demo.add
curl -sS http://127.0.0.1:8080/v1/operations/demo.add/outputs
curl -sS http://127.0.0.1:8080/v1/request -H 'Content-Type: application/json' --data-binary @requests/add.http.json
curl -sS 'http://127.0.0.1:8080/v1/request?pretty=true' -H 'Content-Type: application/json' --data-binary @requests/add.http.json
```

[requests/add.http.json](requests/add.http.json) is an input envelope: `{"operation": "demo.add", "data": {"a": 2, "b": 3}}`.

#### Expected Output / Response

Every GET route answers with an envelope of the matching built-in (`rivet.health`, `rivet.list`, `rivet.describe`, `rivet.outputs`):

```json
{"request_id":"req_02a4cf4782","trace_id":"tr_02a4cf4782","operation":"rivet.health","type":"result","status":"ok","data":{"status":"ok","catalog_version":"sha256:67104f0e7faaeafee253db758a668a9b24aa4263677e9d5ea2451b32019f9730","version":"0.2.0"},"error":null,"effects":"none","data_count":0}
{"request_id":"req_0326f0907f","trace_id":"tr_0326f0907f","operation":"rivet.list","type":"result","status":"ok","data":{"operations":[{"id":"demo.greet","name":"Greet a person","description":"Return a greeting for the supplied person.","streaming":false},{"id":"demo.add","name":"Add two integers","description":"Add two signed integers and return their sum.","streaming":false},{"id":"demo.health","name":"Check availability","description":"Return a constant readiness response without I/O.","streaming":false},{"id":"demo.countdown","name":"Count down","description":"Emit 3, 2, 1 as data items and then return a summary.","streaming":true}],"next_cursor":null},"error":null,"effects":"none","data_count":0}
```

`/v1/operations/demo.add` returns the same `rivet.describe` envelope as `describe demo.add --json` (step 1), `/outputs` returns `{…"operation":"rivet.outputs",…,"data":{"id":"demo.add","output":{"type":"integer","description":"Sum of a and b."},"emits":null,"receives":null,"errors":[]},…}`, and `POST /v1/request` returns HTTP 200:

```json
{"request_id":"req_06a5b4d57e","trace_id":"tr_06a5b4d57e","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}
```

`?pretty=true` returns the same envelope indented (two spaces, same key order), exactly like `--pretty` in step 1.

The dispatcher validates each result against the declared output before returning; a mismatch would be `output.invalid` (HTTP 500, exit 5).

**Failing examples** (the HTTP status follows the body):

```sh
curl -sS -w ' %{http_code}\n' http://127.0.0.1:8080/v1/request -H 'Content-Type: application/json' -d '{"operation":"demo.add","data":{"b":3}}'
curl -sS -w ' %{http_code}\n' http://127.0.0.1:8080/v1/request -H 'Content-Type: application/json' -d '{"operation":"demo.nope","data":{}}'
curl -sS -w ' %{http_code}\n' http://127.0.0.1:8080/v1/request -H 'Content-Type: application/json' -d '{"operation":"demo.add","id":"demo.add","data":{"a":1}}'
```

```text
{"request_id":"req_08a5a51510","trace_id":"tr_08a5a51510","operation":"demo.add","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.required","message":"missing required parameter `a`","retryable":false,"operation_id":"demo.add","details":{"field":"a"}},"effects":"none","data_count":0} 422
{"request_id":"req_091954749d","trace_id":"tr_091954749d","operation":"demo.nope","type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"not_found.operation","message":"no operation `demo.nope`","retryable":false,"operation_id":"demo.nope"},"effects":"none","data_count":0} 404
{"request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.input_envelope","message":"use `operation` or the deprecated `id`, not both","retryable":false,"details":{"key":"operation","alias":"id"}},"effects":"none","data_count":0} 422
```

**A 0.1.0 client still works in 0.2.x.** The legacy body `{"id", "params"}` is accepted, and the response carries `Deprecation: true` (and the access log `"deprecated":1`):

```sh
curl -sS -i http://127.0.0.1:8080/v1/request -H 'Content-Type: application/json' -d '{"id":"demo.add","params":{"a":2,"b":3}}'
```

```text
HTTP/1.1 200 OK
content-type: application/json
traceparent: 00-418af828f130d81693a9b9a4f579dfb7-ff6e720c6ab8902e-01
deprecation: true
…
{"request_id":"req_109a2976aa","trace_id":"tr_109a2976aa","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}
```

**W3C trace context.** A `traceparent` header is accepted; the envelope's `trace_id` becomes its trace ID and the response carries a new `traceparent` with the same trace ID:

```sh
curl -sS -i http://127.0.0.1:8080/v1/request -H 'traceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01' \
  -H 'Content-Type: application/json' --data-binary @requests/add.http.json
```

```text
HTTP/1.1 200 OK
content-type: application/json
traceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-ef1195a42751f92a-01
…
{"request_id":"req_111beda15f","trace_id":"4bf92f3577b34da6a3ce929d0e0e4736","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}
```

### 5. SSE (same route, different `Accept`)

#### Command / Request

```sh
curl -sSN http://127.0.0.1:8080/v1/request \
  -H 'Content-Type: application/json' -H 'Accept: text/event-stream' \
  --data-binary @requests/countdown.http.json
```

#### Expected Output / Response

```text
id: 1
event: data
data: {"request_id":"req_1293872424","trace_id":"tr_1293872424","operation":"demo.countdown","type":"data","seq":1,"data":3,"error":null}

id: 2
event: data
data: {"request_id":"req_1293872424","trace_id":"tr_1293872424","operation":"demo.countdown","type":"data","seq":2,"data":2,"error":null}

id: 3
event: data
data: {"request_id":"req_1293872424","trace_id":"tr_1293872424","operation":"demo.countdown","type":"data","seq":3,"data":1,"error":null}

id: 4
event: result
data: {"request_id":"req_1293872424","trace_id":"tr_1293872424","operation":"demo.countdown","type":"result","seq":4,"status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}
```

The event name mirrors the record `type`. There is no `event: error` any more: a failed stream ends with `event: result` whose `status` is `error` (or `cancelled`), so a client switches on `status`.

**Failing example.** Pretty JSON cannot be combined with an event stream (HTTP 400):

```sh
curl -sS -w ' %{http_code}\n' 'http://127.0.0.1:8080/v1/request?pretty=true' \
  -H 'Content-Type: application/json' -H 'Accept: text/event-stream' --data-binary @requests/countdown.http.json
```

```text
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
} 400
```

### 6. Polling: open → events → terminal

```text
 client                                         rivet serve
   | POST /v1/requests {operation, data}            |
   |----------------------------------------------->|  open (rivet.sessions.open)
   |<-- 202 envelope status accepted, data = receipt|
   | GET {events_url}?after_seq=0&wait_ms=1000      |
   |----------------------------------------------->|  read (rivet.sessions.read)
   |<----------- 200 SessionBatch  terminal:false   |
   | GET {events_url}?after_seq=<last_seq>&...      |
   |----------------------------------------------->|
   |<----------- 200 SessionBatch  terminal:true    |  exactly one terminal event
```

#### Command / Request

Open a polling session for the streaming operation:

```sh
curl -sS -X POST http://127.0.0.1:8080/v1/requests \
  -H 'Content-Type: application/json' --data-binary @requests/countdown.http.json
```

Copy the returned `events_url` (it contains the real session ID) and poll it:

```sh
EVENTS_URL='/v1/requests/REPLACE_WITH_SESSION_ID/events'
curl -sS "http://127.0.0.1:8080${EVENTS_URL}?after_seq=0&wait_ms=1000"
```

Repeat with `after_seq` set to the returned `last_seq` until `terminal` is `true`. A unary operation can be submitted the same way as an asynchronous job:

```sh
curl -sS -X POST http://127.0.0.1:8080/v1/requests \
  -H 'Content-Type: application/json' --data-binary @requests/add.http.json
```

#### Expected Output / Response

The open call returns HTTP 202 with an envelope whose `status` is `accepted` and whose `data` is the SessionReceipt (the only case where `data` is set and the request has not finished):

```json
{"request_id":"req_13ab13a8e1","trace_id":"tr_13ab13a8e1","operation":"demo.countdown","type":"result","status":"accepted","data":{"session_id":"ses_01a29191d5","request_id":"req_13ab13a8e1","trace_id":"tr_13ab13a8e1","catalog_version":"sha256:67104f0e7faaeafee253db758a668a9b24aa4263677e9d5ea2451b32019f9730","input_schema":null,"emits_schema":{"type":"integer"},"next_send_seq":1,"expires_at":"2026-09-28T21:16:26Z","events_url":"/v1/requests/ses_01a29191d5/events"},"error":null,"effects":"none","data_count":0}
```

Polling `events_url` returns a SessionBatch (not an envelope; its `events` are stream records, the same records as NDJSON and SSE). The countdown fits in one batch:

```json
{"session_id":"ses_01a29191d5","events":[{"request_id":"req_13ab13a8e1","trace_id":"tr_13ab13a8e1","operation":"demo.countdown","type":"data","seq":1,"data":3,"error":null},{"request_id":"req_13ab13a8e1","trace_id":"tr_13ab13a8e1","operation":"demo.countdown","type":"data","seq":2,"data":2,"error":null},{"request_id":"req_13ab13a8e1","trace_id":"tr_13ab13a8e1","operation":"demo.countdown","type":"data","seq":3,"data":1,"error":null},{"request_id":"req_13ab13a8e1","trace_id":"tr_13ab13a8e1","operation":"demo.countdown","type":"result","seq":4,"status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}],"last_seq":4,"terminal":true}
```

Polling again with `after_seq=4` returns `{"session_id":"ses_01a29191d5","events":[],"last_seq":4,"terminal":true}`. The unary `demo.add` job returns one batch with one terminal record:

```json
{"session_id":"ses_02ddba1c42","events":[{"request_id":"req_14d53c29a6","trace_id":"tr_14d53c29a6","operation":"demo.add","type":"result","seq":1,"status":"ok","data":5,"error":null,"effects":"none","data_count":0}],"last_seq":1,"terminal":true}
```

Cancelling a session that already finished reports its terminal state instead of cancelling it:

```sh
curl -sS -X POST http://127.0.0.1:8080/v1/requests/ses_01a29191d5/cancel
```

```json
{"session_id":"ses_01a29191d5","request_id":"req_13ab13a8e1","state":"succeeded"}
```

Polling sessions survive client reconnects until they expire (`expires_at`) or are cancelled.

### 7. WebSocket frames

[requests/ws-frames.jsonl](requests/ws-frames.jsonl) holds three client frames, multiplexed by client-chosen `ref`. Each is an input envelope plus `type` and `ref`: `{"type":"request","ref":"c1","operation":"demo.add","data":{"a":2,"b":3}}`.

#### Command / Request

```sh
"$TMPDIR/rivet-demo-venv/bin/python" fixtures/ws_client.py ws://127.0.0.1:8080/v1/ws < requests/ws-frames.jsonl
```

The client connects with subprotocol `rivet.v1`, sends each line as one text frame and prints server frames until every ref has a terminal frame. With websocat the equivalent is `websocat --protocol rivet.v1 ws://127.0.0.1:8080/v1/ws < requests/ws-frames.jsonl`.

```text
 client frames (ref)                      server frames (per-ref order guaranteed)
 c1 request demo.add {a:2,b:3}   ------>  c1 result  status ok, data 5
 c2 request demo.countdown {}    ------>  c2 data seq 1..3, then c2 result status ok, data {count:3}
 c3 request demo.add {b:3}       ------>  c3 result  status error, validation.required
                                          (frames of different refs may interleave)
```

#### Expected Output / Response

Every server frame is a stream record (the same envelope as NDJSON and SSE) with `ref` first. There is no separate `error` frame and no nested `completion`. The order of different refs varies between runs; the order within a ref does not:

```json
{"ref":"c3","request_id":"req_0330526a9f","trace_id":"tr_0330526a9f","operation":"demo.add","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.required","message":"missing required parameter `a`","retryable":false,"operation_id":"demo.add","details":{"field":"a"}},"effects":"none","data_count":0}
{"ref":"c2","request_id":"req_02b31861fa","trace_id":"tr_02b31861fa","operation":"demo.countdown","type":"data","seq":1,"data":3,"error":null}
{"ref":"c1","request_id":"req_0132c3fccd","trace_id":"tr_0132c3fccd","operation":"demo.add","type":"result","seq":1,"status":"ok","data":5,"error":null,"effects":"none","data_count":0}
{"ref":"c2","request_id":"req_02b31861fa","trace_id":"tr_02b31861fa","operation":"demo.countdown","type":"data","seq":2,"data":2,"error":null}
{"ref":"c2","request_id":"req_02b31861fa","trace_id":"tr_02b31861fa","operation":"demo.countdown","type":"data","seq":3,"data":1,"error":null}
{"ref":"c2","request_id":"req_02b31861fa","trace_id":"tr_02b31861fa","operation":"demo.countdown","type":"result","seq":4,"status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}
```

The validation failure of `c3` happens after the request is created, so it carries request and trace IDs like the HTTP 422 of step 4 (INC-2026-0012). A unary WS result keeps `"seq":1`: every WS ref is a session (API-2026-0002). Only a frame that never became a request keeps empty IDs, and a refusal that is not a ref's terminal record has `ref: ""`:

```text
  frame                                            reply                                      IDs    ref
  request demo.add {b:3}              (c3)   -->   validation.required terminal record         yes    "c3"
  second request on in-flight ref     (d1)   -->   conflict.ref, error.details.ref = "d1"      empty  ""
  `operation` and `id` together       (e1)   -->   validation.input_envelope, operation echoed empty  "e1"
  not JSON                                   -->   validation.frame, operation null            empty  ""
```

```json
{"ref":"","request_id":"","trace_id":"","operation":"demo.add","type":"result","status":"error","data":null,"error":{"kind":"conflict","code":"conflict.ref","message":"ref `d1` is already in flight","retryable":false,"details":{"ref":"d1"}},"effects":"none","data_count":0}
{"ref":"e1","request_id":"","trace_id":"","operation":"demo.add","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.input_envelope","message":"use `operation` or the deprecated `id`, not both","retryable":false,"operation_id":"demo.add","details":{"key":"operation","alias":"id"}},"effects":"none","data_count":0}
{"ref":"","request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.frame","message":"frame is not JSON: expected ident at line 1 column 2","retryable":false},"effects":"none","data_count":0}
```

These three lines came from sending `{"type":"request","ref":"d1","operation":"demo.countdown","data":{}}`, the same `ref` again with `demo.add`, `{"type":"request","ref":"e1","operation":"demo.add","id":"demo.add","data":{"a":1}}` and `not json` on one connection; `d1`'s countdown then completed normally (`seq` 1–4).

A connection may hold at most 8 in-flight refs; each ref has a bounded 16-frame queue. Closing the socket cancels and joins all of its refs. Unlike polling sessions, WebSocket requests are owned by the connection.

### 8. MCP (Streamable HTTP at /mcp)

#### Command / Request

Initialize and note the returned `mcp-session-id` header:

```sh
curl -i -sS http://127.0.0.1:8080/mcp \
  -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' \
  --data-binary @requests/initialize.mcp.json
```

Replace the value below with that header, complete initialization, list tools, then call the **direct named tool** (the canonical MCP form) and the built-in `rivet.outputs` tool:

```sh
RIVET_DEMO_SESSION='returned-session-id'
for body in initialized list add outputs; do
  curl -sS http://127.0.0.1:8080/mcp \
    -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' \
    -H "MCP-Session-Id: $RIVET_DEMO_SESSION" -H 'MCP-Protocol-Version: 2025-11-25' \
    --data-binary @requests/$body.mcp.json; echo
done
```

The bodies are [initialized](requests/initialized.mcp.json), [list](requests/list.mcp.json), [add](requests/add.mcp.json) (`tools/call {name:"demo.add"}`) and [outputs](requests/outputs.mcp.json) (`tools/call {name:"rivet.outputs"}`).

#### Expected Output / Response

```text
HTTP/1.1 200 OK
content-type: application/json
mcp-session-id: mcp_126a8c57f065c6f75
…
{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"rivet","version":"0.2.0"}}}
```

`notifications/initialized` returns HTTP 202 with an empty body. `tools/list` returns the four direct tools, each with `name`, `title`, `description`, `inputSchema` and `outputSchema` (the ResponseEnvelope schema whose `data` is the declared output or null), followed by the built-ins `rivet.request`, `rivet.list`, `rivet.describe`, `rivet.outputs`, `rivet.sessions.open|send|finish_input|read|cancel`, `rivet.io`, `rivet.policy.generate`, `rivet.trace.show`, `rivet.trace.export`, `rivet.capabilities`, `rivet.connectors.sync` and `rivet.auth.begin|complete|status|disconnect|cancel`. One entry:

```json
{"name":"demo.add","title":"Add two integers","inputSchema":{"type":"object","properties":{"a":{"type":"integer","description":"First operand."},"b":{"type":"integer","description":"Second operand; defaults to zero.","default":0}},"required":["a"],"additionalProperties":false},"description":"Add two signed integers and return their sum.","outputSchema":{"type":"object","properties":{"request_id":{"type":"string"},"trace_id":{"type":"string"},"operation":{"type":["string","null"]},"type":{"type":"string","enum":["result"]},"status":{"type":"string","enum":["ok","error","cancelled","accepted"]},"data":{"anyOf":[{"type":"integer","description":"Sum of a and b."},{"type":"null"}]},"error":{"type":["object","null"]},"effects":{"type":"string","enum":["none","committed","partial","unknown"]},"data_count":{"type":"integer","minimum":0}},"required":["request_id","trace_id","operation","type","status","data","error","effects","data_count"]}}
```

`demo.countdown` is a session tool (`"_meta":{"rivet/delivery":"session"}`): calling it returns an envelope whose `data` is a SessionReceipt, read with `rivet.sessions.read`. The direct `demo.add` call and `rivet.outputs` return the ResponseEnvelope in `structuredContent` (and as text); `isError` is true when `status` is `error` or `cancelled`:

```json
{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"{\"request_id\":\"req_0411129554\",\"trace_id\":\"tr_0411129554\",\"operation\":\"demo.add\",\"type\":\"result\",\"status\":\"ok\",\"data\":5,\"error\":null,\"effects\":\"none\",\"data_count\":0}"}],"structuredContent":{"request_id":"req_0411129554","trace_id":"tr_0411129554","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0},"isError":false}}
{"jsonrpc":"2.0","id":4,"result":{"content":[{"type":"text","text":"…"}],"structuredContent":{"request_id":"req_059161dbc9","trace_id":"tr_059161dbc9","operation":"rivet.outputs","type":"result","status":"ok","data":{"id":"demo.add","output":{"type":"integer","description":"Sum of a and b."},"emits":null,"receives":null,"errors":[]},"error":null,"effects":"none","data_count":0},"isError":false}}
```

A failing call through the built-in `rivet.request` names the target operation, like a success (INC-2026-0012), and a protocol-level refusal (here `tools/list` without the `MCP-Session-Id` header, HTTP 422) has `operation: null` and empty IDs:

```sh
curl -sS http://127.0.0.1:8080/mcp -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' \
  -H "MCP-Session-Id: $RIVET_DEMO_SESSION" -H 'MCP-Protocol-Version: 2025-11-25' \
  --data '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"rivet.request","arguments":{"operation":"demo.add","data":{"b":3}}}}'; echo
curl -sS -w ' %{http_code}\n' http://127.0.0.1:8080/mcp -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' \
  -H 'MCP-Protocol-Version: 2025-11-25' --data '{"jsonrpc":"2.0","id":6,"method":"tools/list"}'
```

```text
{"jsonrpc":"2.0","id":5,"result":{"content":[{"type":"text","text":"…"}],"structuredContent":{"request_id":"req_0684cfb12e","trace_id":"tr_0684cfb12e","operation":"demo.add","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.required","message":"missing required parameter `a`","retryable":false,"operation_id":"demo.add","details":{"field":"a"}},"effects":"none","data_count":0},"isError":true}}
{"request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"validation","code":"mcp.session_required","message":"MCP-Session-Id header is required after initialize","retryable":false},"effects":"none","data_count":0} 422
```

For an MCP client that launches Rivet as a subprocess, use stdio instead. It serves MCP only; stdout carries only protocol messages and the startup record goes to stderr:

```sh
(tr -d '\n' < requests/initialize.mcp.json; echo; tr -d '\n' < requests/initialized.mcp.json; echo; tr -d '\n' < requests/add.mcp.json; echo) \
  | rivet --file app.rivet serve --stdio 2>/dev/null
```

```json
{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"rivet","version":"0.2.0"}}}
{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"{\"request_id\":\"req_014a894aad\",\"trace_id\":\"tr_014a894aad\",\"operation\":\"demo.add\",\"type\":\"result\",\"status\":\"ok\",\"data\":5,\"error\":null,\"effects\":\"none\",\"data_count\":0}"}],"structuredContent":{"request_id":"req_014a894aad","trace_id":"tr_014a894aad","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0},"isError":false}}
```

### 9. Authentication and narrowed surfaces (alternate policy file)

#### Command / Request

Stop the server (Ctrl-C) and restart it with the alternate policy:

```sh
rivet --file app.rivet --policy ./policies/team.json serve --listen 127.0.0.1:8080
```

[policies/team.json](policies/team.json) grants no I/O. It configures `serve.auth` as bearer tokens (stored only as sha256 hashes), gives principal `ada` `demo.*` and principal `ci` only `demo.health`, and mounts every surface except `ws`. The fixture-only tokens are `dev-token-ada` and `dev-token-ci`:

```sh
curl -sS http://127.0.0.1:8080/v1/request -H 'Authorization: Bearer dev-token-ada' \
  -H 'Content-Type: application/json' --data-binary @requests/add.http.json; echo
curl -sS -w ' %{http_code}\n' http://127.0.0.1:8080/v1/request -H 'Authorization: Bearer dev-token-ci' \
  -H 'Content-Type: application/json' --data-binary @requests/add.http.json
curl -sS -w ' %{http_code}\n' http://127.0.0.1:8080/v1/request \
  -H 'Content-Type: application/json' --data-binary @requests/add.http.json
curl -sS -w ' %{http_code}\n' http://127.0.0.1:8080/v1/request -H 'Authorization: Bearer wrong' \
  -H 'Content-Type: application/json' --data-binary @requests/add.http.json
curl -sS -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8080/v1/ws
curl -sS http://127.0.0.1:8080/v1/operations -H 'Authorization: Bearer dev-token-ci'; echo
```

The CLI can act as a thin client of the same server. The token is read from a file, never from argv or the environment:

```sh
TOKEN_FILE="$(mktemp)"; printf 'dev-token-ada' > "$TOKEN_FILE"
rivet --endpoint http://127.0.0.1:8080 --token-file "$TOKEN_FILE" request demo.add --data '{"a":2,"b":3}'
rivet --endpoint http://127.0.0.1:8080 request demo.add --data '{"a":2}'
rivet --endpoint http://127.0.0.1:8080 --token-file "$TOKEN_FILE" check
rm -f "$TOKEN_FILE"
```

#### Expected Output / Response

The startup record now shows the narrowed surfaces and bearer auth:

```json
{"listen_addr":"127.0.0.1:8080","stdio":false,"surfaces":["http","sse","poll","mcp"],"auth_type":"bearer","catalog_version":"sha256:67104f0e7faaeafee253db758a668a9b24aa4263677e9d5ea2451b32019f9730","policy_hash":"sha256:461d2215049fbb93f51c91c0fcfe35e3247f2f0509c8d8a104b310599841c5a1"}
```

```text
{"request_id":"req_02ef6e364a","trace_id":"tr_02ef6e364a","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}
{"request_id":"req_036f163dff","trace_id":"tr_036f163dff","operation":"demo.add","type":"result","status":"error","data":null,"error":{"kind":"permission","code":"permission.denied","message":"principal `ci` may not call `demo.add`","retryable":false},"effects":"none","data_count":0} 403
{"request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"auth","code":"auth.required","message":"missing bearer token","retryable":false},"effects":"none","data_count":0} 401
{"request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"auth","code":"auth.invalid","message":"invalid bearer token","retryable":false},"effects":"none","data_count":0} 401
404
{"request_id":"req_04eb22d6d4","trace_id":"tr_04eb22d6d4","operation":"rivet.list","type":"result","status":"ok","data":{"operations":[{"id":"demo.health","name":"Check availability","description":"Return a constant readiness response without I/O.","streaming":false}],"next_cursor":null},"error":null,"effects":"none","data_count":0}
```

`ci` only sees the one operation it may call. The `--endpoint` client prints the same envelope as a local run (exit 0); without a token it gets `auth.required` (exit 3); `check`, `graph`, `policy` and `serve` are refused remotely (exit 2):

```text
{"request_id":"req_056a8ca619","trace_id":"tr_056a8ca619","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}
{"request_id":"","trace_id":"","operation":"demo.add","type":"result","status":"error","data":null,"error":{"kind":"auth","code":"auth.required","message":"missing bearer token","retryable":false},"effects":"none","data_count":0}
error[validation.usage]: check, graph, policy and serve work on a local bundle (--file); they are not available with --endpoint
```

```text
  Authorization header ---> serve.auth (bearer sha256) ---> principal ---> serve.principals[p].operations
        missing / wrong ---> 401 auth.required / auth.invalid  not listed ---> 403 permission.denied
  surface not in serve.surfaces ---------------------------------------------> 404
```

The same principal model applies to the `Authorization` header on REST/SSE/polling, the WebSocket upgrade request and every MCP HTTP request. A non-loopback `--listen` with auth `none` refuses to start:

```sh
rivet --file app.rivet serve --listen 0.0.0.0:8080
```

```json
{"request_id":"","trace_id":"","operation":"rivet.serve","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"serve.auth_required","message":"non-loopback listener requires serve.auth in policy.json","retryable":false},"effects":"none","data_count":0}
```

It exits 2. The embedding equivalent is in [12-library](../12-library/README.md).

### Error and exit summary

| Condition | Code | HTTP | CLI exit |
|---|---|---|---|
| Success | — | 200 (202 for polling open) | 0 |
| Missing `a` | `validation.required` | 422 | 2 |
| `operation` and `id` (or `--data` and `--params`) together | `validation.input_envelope` / `validation.usage` | 422 | 2 |
| `?pretty=true` with SSE / `--pretty` with `--stream` | `validation.pretty_stream` / `validation.usage` | 400 | 2 |
| Malformed policy file | `policy.invalid` | — | 2 |
| Non-loopback listen with auth `none` | `serve.auth_required` | — | 2 |
| Missing / wrong bearer token | `auth.required` / `auth.invalid` | 401 | 3 |
| Principal not authorized | `permission.denied` | 403 | 3 |
| Surface not mounted | — | 404 | — |
| Unknown operation ID | `not_found.operation` | 404 | 4 |
| Result does not match declared output | `output.invalid` | 500 | 5 |

## Effects and policy

No application effects, so no policy.json is needed. The host owns bootstrap reads and the listener. MCP stdio stdout carries only protocol messages, never logs. An HTTP or MCP caller can only narrow authority for its own request (`restrict`), never widen it. Effective authority is the host ceiling ∩ policy file ∩ per-request restriction.

## Inspect before invoking

```sh
rivet --file app.rivet check --strict-docs
rivet --file app.rivet io --by target
rivet --file app.rivet io --check-policy
rivet --file app.rivet io --check-files
rivet --file app.rivet graph demo.countdown
rivet --file app.rivet io --include-bootstrap --strict --format json
```

```text
ok: 4 operations, 0 connectors, 0 auth profiles
```

```text
TARGET  ACCESS  CAPABILITY  ORIGIN  PHASE  NEEDS FILE  USED BY
(no I/O sites)
```

```text
OPERATION  KIND  ACCESS  TARGET  KNOWLEDGE  SOURCE  DECISION
(no I/O sites)
```

```text
demo.add needs no existing files.
demo.countdown needs no existing files.
demo.greet needs no existing files.
demo.health needs no existing files.
0 files
```

```text
demo.countdown  app.rivet:27
```

All of these exit 0. `check --strict-docs` requires descriptions on every param, output and output field and an `error` line for every code a body can `fail` with; removing any `description` from an `output` or `field` line makes it fail with exit 2. The graph of a pure operation is just its node; see [05-dag](../05-dag/README.md) for a larger one.

`check --json` prints a `rivet.check` envelope (`"data":{"operations":4,"connectors":0,"auth_profiles":0,"warnings":0}`). The JSON manifest is a `rivet.io` envelope whose `data` has no sites (`"policy":null,"complete":true,"sites":[],"targets":[],"needs":[]`), so `--strict` exits 0. `--include-bootstrap` adds the fixed runtime-internal list under a separate `bootstrap` key. The table form (`rivet --file app.rivet io --include-bootstrap`) prints:

```text
OPERATION  KIND  ACCESS  TARGET  KNOWLEDGE  SOURCE
(no I/O sites)

BOOTSTRAP (runtime-internal; listed, not governed by policy.json)
KIND  ACCESS       TARGET
file  read         ./app.rivet
file  read         ./policy.json (when present)
file  read         system CA bundle
file  read         /etc/resolv.conf / system resolver
file  read         tzdata
file  read         descriptor/schema files named by connectors (none here)
pipe  read, write  stdin, stdout, stderr
```

Since 0.2.0 the bundle row names the entry file and, one row each, every imported module file (0.1.0 printed the placeholder `./app.rivet (+ imports)`); this bundle has no imports. See [17-modules](../17-modules/README.md) for a bundle that has.

`io --check-policy` finds no policy.json, so the effective policy is deny-by-default; with zero sites nothing can be denied. `--policy ./policies/team.json io --check-policy` gives the same result. `io` performs no I/O and evaluates no source expression. Exit codes: 3 when `--check-policy` finds a reachable site denied or partial; 7 with `--strict` when any site is dynamic or opaque; otherwise 0.

## Release Updates

0.2.0 updates shown here (numbering of the [v0.2.0 release verification guide](../demo-2026-0020-v0-2-0-release-verification.md)):

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-01 | UQ-03/05 / R1 | One ResponseEnvelope on CLI, HTTP, SSE, polling, WS and MCP | Steps 1, 4–8 | Same nine keys in the same order on every surface; `data` 5 | This README steps 1, 4–8 (2026-09-29, 8031baa) |
| U-02 | UQ-05 / R2 | `status` ok/error/cancelled/accepted; `type` result/data; `effects` top level | Steps 1, 4, 6 | `status: error` + `data: null` for failures; `accepted` for the polling open | This README steps 1, 4, 6 |
| U-03 | UQ-03 / R3 | Built-ins, GET routes and `--json` outputs are envelopes | Steps 1, 2, 4; Inspect | `rivet.describe`, `rivet.outputs`, `rivet.list`, `rivet.health`, `rivet.check`, `rivet.io` envelopes | This README steps 1, 2, 4 |
| U-04 | UQ-06 / R4 | One input envelope `{operation, data}`; `--data`, `--input` | Steps 1, 4, 6, 7 | Same body accepted by CLI `--input`, REST, polling and WS | This README; [requests/](requests/add.http.json) |
| U-05 | 0.1.0 clients / R5 | `id`/`params` and `--params` still accepted with a deprecation signal; mixing refused | Steps 1, 4 | `warning[deprecated.params]`; `deprecation: true`; `validation.input_envelope` 422 / exit 2 | This README steps 1, 3, 4 |
| U-06 | UQ-04 / R6 | `--pretty`, `?pretty=true`; refused with NDJSON/SSE | Steps 1, 4, 5 | Indented envelope; `validation.usage` exit 2; `validation.pretty_stream` 400 | This README steps 1, 4, 5 |

Still verified from 0.1.0 (numbering of [DEMO-2026-0015](../demo-2026-0015-v0-1-0-release-verification.md)):

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-08 | UQ-07/09 / R8 | One dispatcher behind CLI, REST, SSE, polling, WS, MCP | Steps 1, 4–8 | `demo.add` returns 5 on every surface | This README steps 1, 4–8 (re-run 2026-09-29, 8031baa); TEST-2026-0002 |
| U-20 | UQ-15 / R20 | Many described operations in one file | Step 1 `list --outputs` | Four IDs with names, outputs, descriptions | This README step 1; TEST-2026-0016 |
| U-21 | UQ-15 / R21 | Incoming MCP: direct named tools plus built-ins | Step 8 | `tools/list` shows `demo.*` and `rivet.*`; `demo.add` → 5 | This README step 8; TEST-2026-0018 |
| U-22 | UQ-15/08/17 / R22 | Live streams on every surface | Steps 1, 5, 6, 7 | Items 3, 2, 1 then `{count:3}` | This README steps 1, 5–7; TEST-2026-0019 |
| U-23 | UQ-17 / R23 | Operations declare described outputs | Step 2 | Output schema per operation | This README step 2; TEST-2026-0020 |
| U-24 | UQ-17 / R24 | No policy.json = deny-by-default; `--policy PATH` only | Setup, step 9 | Pure ops succeed; team.json loads by path | This README steps 1, 9; TEST-2026-0021 |
| U-25 | UQ-17 / R25 | One `serve` mounts REST, SSE, polling, WS and MCP with one auth model | Steps 3–9 | 200/401/403/404 as shown | This README steps 3–9; TEST-2026-0022 |
| U-26 | UQ-18 / R26 | `io` is the generated I/O manifest | Inspect before invoking | `(no I/O sites)`, exit 0 | This README "Inspect"; TEST-2026-0025 |

The current command list is in the [CLI reference](../../manuals/man-2026-0004-cli-reference.md); the release-wide checklists are the [v0.2.0 release verification guide](../demo-2026-0020-v0-2-0-release-verification.md) and, for 0.1.0, [DEMO-2026-0015](../demo-2026-0015-v0-1-0-release-verification.md).

## Cleanup

Stop any server you started with Ctrl-C (or `kill -TERM`, which drains the same way). Stopping the server cancels open polling sessions and WebSocket refs and awaits their cleanup. Remove the optional venv with `rm -rf "$TMPDIR/rivet-demo-venv"`. Nothing is written to this folder.

## Verification Record

| Step | Verified By | Verified At | Result |
|---|---|---|---|
| 1. CLI list, describe, request ×3 with `--data`, `--input -`, `--pretty`, `--stream`, failures, `--params` warning, mixed-key and `--stream --pretty` refusals, `rivet.capabilities` | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 2. `outputs demo.health`, `outputs --all --json` (envelope) | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 3. `serve --listen` (run on 127.0.0.1:18800) startup record, access log with `deprecated` | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 4. REST GET routes (envelopes), request, `?pretty=true`, 422/404/422, legacy body + `Deprecation`, traceparent | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 5. SSE countdown (`event: result`), `?pretty=true` refusal 400 | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 6. Polling open (202 `accepted`), events, unary job, cancel after finish | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 7. WebSocket three refs via fixtures/ws_client.py (envelope frames with `ref`) | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 8. MCP initialize, initialized, tools/list (`outputSchema` = envelope), direct tool, `rivet.outputs`, stdio | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 7. (re-run after INC-2026-0012) WebSocket three refs: `c3` refusal carries request/trace IDs; extra frames: `conflict.ref` for in-flight `d1` detached (`ref: ""`, `error.details.ref`), `validation.input_envelope` echoes `operation`, not-JSON frame keeps empty IDs; server on 127.0.0.1:18801 | Claude | 2026-09-29, commit 7c25175, macOS 26.4.1 arm64 | PASS |
| 8. (re-run after INC-2026-0012) MCP initialize, initialized, tools/list, direct tool, `rivet.outputs`, stdio; `rivet.request` error names `demo.add`; `mcp.session_required` has `operation: null` | Claude | 2026-09-29, commit 7c25175, macOS 26.4.1 arm64 | PASS |
| 9. team.json: ada 200, ci 403, no/wrong token 401, ws 404, `--endpoint`, non-loopback refusal | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| `check --strict-docs`, `check --json` (exit 0) | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| `io --check-policy` (exit 0) | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| `io --check-files` (exit 0), `io --by target`, `graph`, `io --include-bootstrap` (no `(+ imports)` placeholder), JSON manifest | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |

Verified on 0.2.0-dev at commit `8031baa`, the release candidate (`cargo build --release --workspace --all-features`). Every command above was executed from this folder and the output pasted from that run. The 0.1.0 verification (TASK-067, commit 829ca43, 2026-09-28) is recorded in revision 7 below.

## Known Caveats

- Request, trace, session and MCP session IDs, timestamps, `catalog_version` and `policy_hash` values are generated or content-derived; compare application results, codes and HTTP statuses, not literal IDs.
- Frames of different WebSocket refs interleave differently on each run.
- Port 8080 is a common development port; when it is taken, pick another loopback port consistently.
- The release candidate still reports version `0.1.0` (`rivet --version`, `/v1/health`, `serverInfo.version`, `rivet.capabilities`); the bump to 0.2.0 happens at release (PLAN-2026-0002 P5).

## Related Documents

- [All sample folders](../README.md) · [demos index](../index.md) · [v0.2.0 release verification guide](../demo-2026-0020-v0-2-0-release-verification.md) · [v0.1.0 guide](../demo-2026-0015-v0-1-0-release-verification.md) · [envelope reference (API-2026-0006)](../../api/api-2026-0006-envelopes.md) · [migration guide (MIG-2026-0001)](../../migrations/mig-2026-0001-response-and-input-envelopes.md) · [CLI reference](../../manuals/man-2026-0004-cli-reference.md)
- [Installation and quickstart (MAN-2026-0002)](../../manuals/man-2026-0002-installation-and-quickstart.md) · [Serving and surfaces (MAN-2026-0006)](../../manuals/man-2026-0006-serving-and-surfaces.md)
- [Usage reference](../../references/ref-2026-0002-language-and-usage.md) · [Proposal](../../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 10 | 2026-09-30 | Claude | v0.2.0 release (PLAN-2026-0002 TASK-091): version strings and current-release wording updated to 0.2.0. |
| 9 | 2026-09-29 | Claude | INC-2026-0012 re-verification (T-30) at 7c25175: step 7 re-run, `c3` now has request/trace IDs, added the detached `conflict.ref`, `operation`-echoing envelope refusal and not-JSON frame; step 8 re-run (new IDs), added the `rivet.request` error naming the target and `mcp.session_required` with `operation: null`; two Verification Record rows. |
| 8 | 2026-09-29 | Claude | TASK-076 (PLAN-2026-0002 D-50): re-executed every step against the 0.2.0 release candidate (8031baa). Commands use `--data` (plus `--input -`, `--pretty`, the `--params` deprecation warning and the refusals); every output replaced by 0.2.0 ResponseEnvelopes and stream records: GET routes, `describe`/`outputs`/`check`/`io` JSON, SSE `event: result`, polling `accepted` receipt, WS envelope frames with `ref`, MCP `structuredContent` and `outputSchema`; legacy-body `Deprecation` example; `(+ imports)` placeholder replaced by the real bootstrap table; `build_features`/`abi_version` in `rivet.capabilities`; 0.2.0 Release Updates; verified_against 0.2.0 |
| 7 | 2026-09-28 | Claude | TASK-067 re-verification at 829ca43 (after the INC-2026-0005/0006 fixes): every step re-run with the release binary on 127.0.0.1:18800; output identical except IDs, timestamps and ports; commit references updated; linked the release verification guide DEMO-2026-0015 |
| 6 | 2026-09-28 | Codex | Repaired two links to the absent release-verification guide by linking the current CLI reference. |
| 5 | 2026-09-28 | Claude | TASK-067: executed every step against 0.1.0-dev (073d944) and pasted real output. Fixes: `list --outputs` columns; `describe`, `outputs --all --json`, Completion, SSE, polling receipt/batch (session IDs `ses_…`, `trace_id` in receipts, `seq` on the result event) and WebSocket frames (data frames carry `request_id`/`trace_id`) as printed; MCP `tools/list` built-in set; `/v1/health`, traceparent, cancel-after-finish, `--endpoint`, 401 codes, `rivet.capabilities`, `graph`, `io --check-files`; WebSocket step uses fixtures/ws_client.py (websocat is optional); §7 header; status active; verified_against 0.1.0. |
| 4 | 2026-09-28 | Claude | TASK-005/R26 (ADR-0001, proposal revision 8): `io --by target` header gains ORIGIN, PHASE, NEEDS FILE (still no sites); verified-against revision 8. |
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest: `io --by target` shows no application sites; `io --check-policy` exits 0 with no policy.json. |
| 2 | 2026-09-28 | Claude | UQ-17 rework: added `demo.countdown` and declared/described outputs; removed `--sandbox` (no policy.json = deny-by-default) and added `policies/team.json` for bearer auth; replaced `--transport`/`--mcp` with one `serve` showing REST, SSE, polling, WebSocket (`requests/ws-frames.jsonl`) and direct-tool MCP; added View outputs, strict-docs, bootstrap and exit-code expectations. |
| 1 | 2026-09-28 | Codex | Added draft source files, prerequisites, invocation examples and expected behavior. |
