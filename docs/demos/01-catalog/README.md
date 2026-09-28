---
document_id: DEMO-2026-0001
title: "One file, four operations, every access point"
document_type: demo
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 7
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [registry, cli, serve, http, poll, ws, mcp, sessions, auth]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development]
audience: [developers, reviewers]
scope: One app.rivet with four pure, described operations, called from the CLI and from one `rivet serve` over REST, SSE, polling, WebSocket and MCP, with bearer authentication through an alternate policy file.
reason: User requested sample folders with READMEs showing usage; UQ-17 adds declared outputs, policy.json-only policy and one serve for every surface; UQ-18 adds the generated I/O manifest. TASK-067 executed every step against the 0.1.0 release candidate.
dependencies: [PROP-2026-0001, REF-2026-0002]
related_documents: [PLAN-2026-0001, DEMO-2026-0015, DEMO-2026-0013, MAN-2026-0002, MAN-2026-0006, API-2026-0001, API-2026-0002, API-2026-0003, TEST-2026-0002, TEST-2026-0020, TEST-2026-0022]
supersedes: null
superseded_by: null
tags: [rivet, demo, catalog, serve, rest, sse, polling, websocket, mcp]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-28
verified_against: "0.1.0"
---

# One file, four operations, every access point

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
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

0.1.0. Verified on 0.1.0-dev at commit `829ca43`, the release candidate (the version bump to 0.1.0 happens at release, P5), with `target/release/rivet` on macOS 26.4.1 (Darwin 25.4.0, arm64), 2026-09-28. Every output block below was pasted from that run; the verification run listened on `127.0.0.1:18800` instead of 8080. Request, trace and session IDs, timestamps, hashes and ports vary from run to run.

## Prerequisites

Build Rivet and put it on `PATH` (from the repository root):

```sh
cargo build --release
export PATH="$PWD/target/release:$PATH"     # `rivet --version` prints rivet 0.1.0
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
rivet --file app.rivet request demo.add --params '{"a":2,"b":3}'
rivet --file app.rivet request demo.greet --params '{"person":"Ada"}'
rivet --file app.rivet request demo.health --params '{}'
rivet --file app.rivet request demo.countdown --params '{}' --stream
```

#### Expected Output / Response

```text
ID              NAME                OUTPUT      DESCRIPTION
demo.greet      Greet a person      text        Return a greeting for the supplied person.
demo.add        Add two integers    integer     Add two signed integers and return their sum.
demo.health     Check availability  object      Return a constant readiness response without I/O.
demo.countdown  Count down          object      Emit 3, 2, 1 as data items and then return a summary.
```

```json
{"id":"demo.add","name":"Add two integers","description":"Add two signed integers and return their sum.","kind":"operation","input":{"type":"object","properties":{"a":{"type":"integer","description":"First operand."},"b":{"type":"integer","description":"Second operand; defaults to zero.","default":0}},"required":["a"],"additionalProperties":false},"output":{"type":"integer","description":"Sum of a and b."},"emits":null,"receives":null,"errors":[],"delivery":"unary","source":{"file":"app.rivet","line":9}}
```

Each `request` prints one Completion line and exits 0:

```json
{"request_id":"req_01517202b5","trace_id":"tr_01517202b5","result":5,"data_count":0,"effects":"none"}
{"request_id":"req_0150318b2d","trace_id":"tr_0150318b2d","result":"Hello, Ada!","data_count":0,"effects":"none"}
{"request_id":"req_014f84d655","trace_id":"tr_014f84d655","result":{"ready":true},"data_count":0,"effects":"none"}
```

`--stream` prints NDJSON: one line per data item, then the terminal result line:

```json
{"request_id":"req_014d425c4d","trace_id":"tr_014d425c4d","seq":1,"type":"data","data":3}
{"request_id":"req_014d425c4d","trace_id":"tr_014d425c4d","seq":2,"type":"data","data":2}
{"request_id":"req_014d425c4d","trace_id":"tr_014d425c4d","seq":3,"type":"data","data":1}
{"request_id":"req_014d425c4d","trace_id":"tr_014d425c4d","result":{"count":3},"data_count":3,"effects":"none","type":"result"}
```

**Failing examples.** Omitting `a` fails validation before execution (exit 2); an unknown ID is `not_found.operation` (exit 4):

```sh
rivet --file app.rivet request demo.add --params '{"b":3}'
rivet --file app.rivet request demo.nope --params '{}'
```

```json
{"request_id":"req_01eacbbe45","trace_id":"tr_01eacbbe45","error":{"kind":"validation","code":"validation.required","message":"missing required parameter `a`","retryable":false,"effects":"none","operation_id":"demo.add","details":{"field":"a"}}}
{"request_id":"req_01e76c91dd","trace_id":"tr_01e76c91dd","error":{"kind":"not_found","code":"not_found.operation","message":"no operation `demo.nope`","retryable":false,"effects":"none","operation_id":"demo.nope"}}
```

The build's own capability report is a built-in operation too (output abbreviated):

```sh
rivet --file app.rivet request rivet.capabilities --params '{}'
```

```json
{"request_id":"req_01ace5705d","trace_id":"tr_01ace5705d","result":{"version":"0.1.0","platform":{"os":"macos","arch":"aarch64"},"stages":{"A":"supported","B":"supported","C":"unsupported"},"features":[{"name":"http","stage":"A","support":"supported","versions":["1.1","2","3"],"streaming":["sse","jsonl","lines","bytes"]}, …],"sandbox":{"backend":"macos-seatbelt","status":"active","reason":"Seatbelt via /usr/bin/sandbox-exec with a deny-default profile"},"serve":{"surfaces":["cli","http","sse","poll","websocket","mcp","library"],"auth":["none","bearer"]}},"data_count":0,"effects":"none"}
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

`outputs --all --json` prints one entry per public operation, sorted by ID, on one line (wrapped here):

```json
[{"id":"demo.add","output":{"type":"integer","description":"Sum of a and b."},"emits":null,"receives":null,"errors":[]},
 {"id":"demo.countdown","output":{"type":"object","properties":{"count":{"type":"integer","description":"Number of items emitted."}},"required":["count"],"additionalProperties":false,"description":"Summary returned after the last item."},"emits":{"type":"integer","description":"One countdown value per item."},"receives":null,"errors":[]},
 {"id":"demo.greet","output":{"type":"string","description":"Greeting that contains the person's name."},"emits":null,"receives":null,"errors":[]},
 {"id":"demo.health","output":{"type":"object","properties":{"ready":{"type":"boolean","description":"True whenever the host can run pure operations."}},"required":["ready"],"additionalProperties":false,"description":"Readiness report for this catalog."},"emits":null,"receives":null,"errors":[]}]
```

`GET /v1/operations/demo.add/outputs` and the MCP `rivet.outputs` tool return the same JSON for one ID. The `emits` entry carries the item type only; see Known Caveats.

### 3. Start one server for every surface

#### Command / Request

In a dedicated terminal (from this folder):

```sh
rivet --file app.rivet serve --listen 127.0.0.1:8080
```

#### Expected Output / Response

The first stderr line is the startup record; every request then adds one JSON access-log line on stderr:

```json
{"listen_addr":"127.0.0.1:8080","stdio":false,"surfaces":["http","sse","poll","ws","mcp"],"auth_type":"none","catalog_version":"sha256:67104f0e7faaeafee253db758a668a9b24aa4263677e9d5ea2451b32019f9730","policy_hash":null}
{"time":"2026-09-28T06:42:38.600Z","surface":"http","method":"POST","route":"/v1/request","principal":"local","operation":"demo.add","status":200,"duration_ms":0}
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
```

#### Expected Output / Response

```json
{"status":"ok","catalog_version":"sha256:67104f0e7faaeafee253db758a668a9b24aa4263677e9d5ea2451b32019f9730"}
{"operations":[{"id":"demo.greet","name":"Greet a person","description":"Return a greeting for the supplied person.","streaming":false},{"id":"demo.add","name":"Add two integers","description":"Add two signed integers and return their sum.","streaming":false},{"id":"demo.health","name":"Check availability","description":"Return a constant readiness response without I/O.","streaming":false},{"id":"demo.countdown","name":"Count down","description":"Emit 3, 2, 1 as data items and then return a summary.","streaming":true}],"next_cursor":null}
```

`/v1/operations/demo.add` returns the same descriptor as `describe demo.add --json` (step 1), `/outputs` returns `{"id":"demo.add","output":{"type":"integer","description":"Sum of a and b."},"emits":null,"receives":null,"errors":[]}`, and `POST /v1/request` returns HTTP 200:

```json
{"request_id":"req_01735e65a5","trace_id":"tr_01735e65a5","result":5,"data_count":0,"effects":"none"}
```

The dispatcher validates each result against the declared output before returning; a mismatch would be `output.invalid` (HTTP 500, exit 5).

**Failing examples** (the HTTP status follows the body):

```sh
curl -sS -w ' %{http_code}\n' http://127.0.0.1:8080/v1/request -H 'Content-Type: application/json' -d '{"id":"demo.add","params":{"b":3}}'
curl -sS -w ' %{http_code}\n' http://127.0.0.1:8080/v1/request -H 'Content-Type: application/json' -d '{"id":"demo.nope","params":{}}'
```

```text
{"request_id":"req_057f6f5649","trace_id":"tr_057f6f5649","error":{"kind":"validation","code":"validation.required","message":"missing required parameter `a`","retryable":false,"effects":"none","operation_id":"demo.add","details":{"field":"a"}}} 422
{"request_id":"req_06f84617ee","trace_id":"tr_06f84617ee","error":{"kind":"not_found","code":"not_found.operation","message":"no operation `demo.nope`","retryable":false,"effects":"none","operation_id":"demo.nope"}} 404
```

**W3C trace context.** A `traceparent` header is accepted; the Completion's `trace_id` becomes its trace ID and the response carries a new `traceparent` with the same trace ID:

```sh
curl -sS -i http://127.0.0.1:8080/v1/request -H 'traceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01' \
  -H 'Content-Type: application/json' --data-binary @requests/add.http.json
```

```text
HTTP/1.1 200 OK
content-type: application/json
traceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-66e207f1cfec19a8-01
…
{"request_id":"req_04e8703e14","trace_id":"4bf92f3577b34da6a3ce929d0e0e4736","result":5,"data_count":0,"effects":"none"}
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
data: {"request_id":"req_02f38843e2","trace_id":"tr_02f38843e2","seq":1,"type":"data","data":3}

id: 2
event: data
data: {"request_id":"req_02f38843e2","trace_id":"tr_02f38843e2","seq":2,"type":"data","data":2}

id: 3
event: data
data: {"request_id":"req_02f38843e2","trace_id":"tr_02f38843e2","seq":3,"type":"data","data":1}

id: 4
event: result
data: {"request_id":"req_02f38843e2","trace_id":"tr_02f38843e2","result":{"count":3},"data_count":3,"effects":"none","type":"result","seq":4}
```

### 6. Polling: open → events → terminal

```text
 client                                         rivet serve
   | POST /v1/requests {id, params}                 |
   |----------------------------------------------->|  open (rivet.sessions.open)
   |<-------------- 202 SessionReceipt + events_url |
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

The open call returns HTTP 202 with a SessionReceipt:

```json
{"session_id":"ses_017150dba5","request_id":"req_0373c5efc7","trace_id":"tr_0373c5efc7","catalog_version":"sha256:67104f0e7faaeafee253db758a668a9b24aa4263677e9d5ea2451b32019f9730","input_schema":null,"emits_schema":{"type":"integer"},"next_send_seq":1,"expires_at":"2026-09-28T06:43:08Z","events_url":"/v1/requests/ses_017150dba5/events"}
```

Polling `events_url` returns a SessionBatch; the countdown fits in one batch:

```json
{"session_id":"ses_017150dba5","events":[{"request_id":"req_0373c5efc7","trace_id":"tr_0373c5efc7","seq":1,"type":"data","data":3},{"request_id":"req_0373c5efc7","trace_id":"tr_0373c5efc7","seq":2,"type":"data","data":2},{"request_id":"req_0373c5efc7","trace_id":"tr_0373c5efc7","seq":3,"type":"data","data":1},{"request_id":"req_0373c5efc7","trace_id":"tr_0373c5efc7","result":{"count":3},"data_count":3,"effects":"none","type":"result","seq":4}],"last_seq":4,"terminal":true}
```

Polling again with `after_seq=4` returns `{"session_id":"ses_017150dba5","events":[],"last_seq":4,"terminal":true}`. The unary `demo.add` job returns one batch with one terminal event:

```json
{"session_id":"ses_0201e20dd2","events":[{"request_id":"req_04025f578c","trace_id":"tr_04025f578c","result":5,"data_count":0,"effects":"none","type":"result","seq":1}],"last_seq":1,"terminal":true}
```

Cancelling a session that already finished reports its terminal state instead of cancelling it:

```sh
curl -sS -X POST http://127.0.0.1:8080/v1/requests/ses_017150dba5/cancel
```

```json
{"session_id":"ses_017150dba5","request_id":"req_0373c5efc7","state":"succeeded"}
```

Polling sessions survive client reconnects until they expire (`expires_at`) or are cancelled.

### 7. WebSocket frames

[requests/ws-frames.jsonl](requests/ws-frames.jsonl) holds three client frames, multiplexed by client-chosen `ref`.

#### Command / Request

```sh
"$TMPDIR/rivet-demo-venv/bin/python" fixtures/ws_client.py ws://127.0.0.1:8080/v1/ws < requests/ws-frames.jsonl
```

The client connects with subprotocol `rivet.v1`, sends each line as one text frame and prints server frames until every ref has a terminal frame. With websocat the equivalent is `websocat --protocol rivet.v1 ws://127.0.0.1:8080/v1/ws < requests/ws-frames.jsonl`.

```text
 client frames (ref)                      server frames (per-ref order guaranteed)
 c1 request demo.add {a:2,b:3}   ------>  c1 result  completion.result = 5
 c2 request demo.countdown {}    ------>  c2 data seq 1..3, then c2 result {count:3}
 c3 request demo.add {b:3}       ------>  c3 error   validation.required
                                          (frames of different refs may interleave)
```

#### Expected Output / Response

The order of different refs varies between runs; the order within a ref does not:

```json
{"type":"error","ref":"c3","error":{"kind":"validation","code":"validation.required","message":"missing required parameter `a`","retryable":false,"effects":"none","operation_id":"demo.add","details":{"field":"a"}}}
{"type":"result","ref":"c1","completion":{"request_id":"req_08cb342438","trace_id":"tr_08cb342438","result":5,"data_count":0,"effects":"none"}}
{"type":"data","ref":"c2","request_id":"req_0948f72145","trace_id":"tr_0948f72145","seq":1,"data":3}
{"type":"data","ref":"c2","request_id":"req_0948f72145","trace_id":"tr_0948f72145","seq":2,"data":2}
{"type":"data","ref":"c2","request_id":"req_0948f72145","trace_id":"tr_0948f72145","seq":3,"data":1}
{"type":"result","ref":"c2","completion":{"request_id":"req_0948f72145","trace_id":"tr_0948f72145","result":{"count":3},"data_count":3,"effects":"none"}}
```

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
mcp-session-id: mcp_192d307ae7f5842cd
…
{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"rivet","version":"0.1.0"}}}
```

`notifications/initialized` returns HTTP 202 with an empty body. `tools/list` returns the four direct tools, each with `name`, `title`, `description`, `inputSchema` and `outputSchema` (the Completion envelope whose `result` is the declared output), followed by the built-ins `rivet.request`, `rivet.list`, `rivet.describe`, `rivet.outputs`, `rivet.sessions.open|send|finish_input|read|cancel`, `rivet.io`, `rivet.policy.generate`, `rivet.trace.show`, `rivet.trace.export`, `rivet.capabilities`, `rivet.connectors.sync` and `rivet.auth.begin|complete|status|disconnect|cancel`. One entry, abbreviated:

```json
{"name":"demo.countdown","title":"Count down","inputSchema":{"type":"object","properties":{},"required":[],"additionalProperties":false},"description":"Emit 3, 2, 1 as data items and then return a summary.","outputSchema":{"type":"object","properties":{"session_id":{"type":"string"}, …},"required":["session_id","request_id","catalog_version","next_send_seq","expires_at"]},"_meta":{"rivet/delivery":"session"}}
```

`demo.countdown` is a session tool: calling it returns a SessionReceipt, read with `rivet.sessions.read`. The direct `demo.add` call and `rivet.outputs` return the Completion in `structuredContent` (and as text):

```json
{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"{\"request_id\":\"req_1071fc88ca\",\"trace_id\":\"tr_1071fc88ca\",\"result\":5,\"data_count\":0,\"effects\":\"none\"}"}],"structuredContent":{"request_id":"req_1071fc88ca","trace_id":"tr_1071fc88ca","result":5,"data_count":0,"effects":"none"},"isError":false}}
{"jsonrpc":"2.0","id":4,"result":{"content":[{"type":"text","text":"…"}],"structuredContent":{"request_id":"req_11f2323bdf","trace_id":"tr_11f2323bdf","result":{"id":"demo.add","output":{"type":"integer","description":"Sum of a and b."},"emits":null,"receives":null,"errors":[]},"data_count":0,"effects":"none"},"isError":false}}
```

For an MCP client that launches Rivet as a subprocess, use stdio instead. It serves MCP only; stdout carries only protocol messages and the startup record goes to stderr:

```sh
(tr -d '\n' < requests/initialize.mcp.json; echo; tr -d '\n' < requests/initialized.mcp.json; echo; tr -d '\n' < requests/add.mcp.json; echo) \
  | rivet --file app.rivet serve --stdio 2>/dev/null
```

```json
{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"rivet","version":"0.1.0"}}}
{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"{\"request_id\":\"req_01aabacb0d\",\"trace_id\":\"tr_01aabacb0d\",\"result\":5,\"data_count\":0,\"effects\":\"none\"}"}],"structuredContent":{"request_id":"req_01aabacb0d","trace_id":"tr_01aabacb0d","result":5,"data_count":0,"effects":"none"},"isError":false}}
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
rivet --endpoint http://127.0.0.1:8080 --token-file "$TOKEN_FILE" request demo.add --params '{"a":2,"b":3}'
rivet --endpoint http://127.0.0.1:8080 request demo.add --params '{"a":2}'
rivet --endpoint http://127.0.0.1:8080 --token-file "$TOKEN_FILE" check
rm -f "$TOKEN_FILE"
```

#### Expected Output / Response

The startup record now shows the narrowed surfaces and bearer auth:

```json
{"listen_addr":"127.0.0.1:8080","stdio":false,"surfaces":["http","sse","poll","mcp"],"auth_type":"bearer","catalog_version":"sha256:67104f0e7faaeafee253db758a668a9b24aa4263677e9d5ea2451b32019f9730","policy_hash":"sha256:461d2215049fbb93f51c91c0fcfe35e3247f2f0509c8d8a104b310599841c5a1"}
```

```text
{"request_id":"req_01299f569d","trace_id":"tr_01299f569d","result":5,"data_count":0,"effects":"none"}
{"request_id":"req_02a92a9dba","trace_id":"tr_02a92a9dba","error":{"kind":"permission","code":"permission.denied","message":"principal `ci` may not call `demo.add`","retryable":false,"effects":"none"}} 403
{"request_id":"","trace_id":"","error":{"kind":"auth","code":"auth.required","message":"missing bearer token","retryable":false,"effects":"none"}} 401
{"request_id":"","trace_id":"","error":{"kind":"auth","code":"auth.invalid","message":"invalid bearer token","retryable":false,"effects":"none"}} 401
404
{"operations":[{"id":"demo.health","name":"Check availability","description":"Return a constant readiness response without I/O.","streaming":false}],"next_cursor":null}
```

`ci` only sees the one operation it may call. The `--endpoint` client prints the same Completion as a local run (exit 0); without a token it gets `auth.required` (exit 3); `check`, `graph`, `policy` and `serve` are refused remotely (exit 2):

```text
{"request_id":"req_036f555ba7","trace_id":"tr_036f555ba7","result":5,"data_count":0,"effects":"none"}
{"request_id":"","trace_id":"","error":{"kind":"auth","code":"auth.required","message":"missing bearer token","retryable":false,"effects":"none"}}
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
{"request_id":"","trace_id":"","error":{"kind":"validation","code":"serve.auth_required","message":"non-loopback listener requires serve.auth in policy.json","retryable":false,"effects":"none"}}
```

It exits 2. The embedding equivalent is in [12-library](../12-library/README.md).

### Error and exit summary

| Condition | Code | HTTP | CLI exit |
|---|---|---|---|
| Success | — | 200 (202 for polling open) | 0 |
| Missing `a` | `validation.required` | 422 | 2 |
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

The JSON manifest has no sites (`"policy": null, "complete": true, "sites": [], "targets": [], "needs": []`), so `--strict` exits 0. `--include-bootstrap` adds the fixed runtime-internal list under a separate `bootstrap` key. Abbreviated to the templates it lists:

```text
bootstrap (phase load, never granted to scripts):
  file read   ./app.rivet (+ imports)
  file read   ./policy.json (when present)
  file read   system CA bundle
  file read   /etc/resolv.conf / system resolver
  file read   tzdata
  file read   descriptor/schema files named by connectors (none here)
  pipe r/w    stdin, stdout, stderr
```

`io --check-policy` finds no policy.json, so the effective policy is deny-by-default; with zero sites nothing can be denied. `--policy ./policies/team.json io --check-policy` gives the same result. `io` performs no I/O and evaluates no source expression. Exit codes: 3 when `--check-policy` finds a reachable site denied or partial; 7 with `--strict` when any site is dynamic or opaque; otherwise 0.

## Release Updates

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-08 | UQ-07/09 / R8 | One dispatcher behind CLI, REST, SSE, polling, WS, MCP | Steps 1, 4–8 | `demo.add` returns 5 on every surface | This README steps 1, 4–8 (2026-09-28, 829ca43); TEST-2026-0002 |
| U-20 | UQ-15 / R20 | Many described operations in one file | Step 1 `list --outputs` | Four IDs with names, outputs, descriptions | This README step 1; TEST-2026-0016 |
| U-21 | UQ-15 / R21 | Incoming MCP: direct named tools plus built-ins | Step 8 | `tools/list` shows `demo.*` and `rivet.*`; `demo.add` → 5 | This README step 8; TEST-2026-0018 |
| U-22 | UQ-15/08/17 / R22 | Live streams on every surface | Steps 1, 5, 6, 7 | Items 3, 2, 1 then `{count:3}` | This README steps 1, 5–7; TEST-2026-0019 |
| U-23 | UQ-17 / R23 | Operations declare described outputs | Step 2 | Output schema per operation | This README step 2; TEST-2026-0020 |
| U-24 | UQ-17 / R24 | No policy.json = deny-by-default; `--policy PATH` only | Setup, step 9 | Pure ops succeed; team.json loads by path | This README steps 1, 9; TEST-2026-0021 |
| U-25 | UQ-17 / R25 | One `serve` mounts REST, SSE, polling, WS and MCP with one auth model | Steps 3–9 | 200/401/403/404 as shown | This README steps 3–9; TEST-2026-0022 |
| U-26 | UQ-18 / R26 | `io` is the generated I/O manifest | Inspect before invoking | `(no I/O sites)`, exit 0 | This README "Inspect"; TEST-2026-0025 |

The current command list is in the [CLI reference](../../manuals/man-2026-0004-cli-reference.md); the release-wide checklist is the [release verification guide](../demo-2026-0015-v0-1-0-release-verification.md).

## Cleanup

Stop any server you started with Ctrl-C (or `kill -TERM`, which drains the same way). Stopping the server cancels open polling sessions and WebSocket refs and awaits their cleanup. Remove the optional venv with `rm -rf "$TMPDIR/rivet-demo-venv"`. Nothing is written to this folder.

## Verification Record

| Step | Verified By | Verified At | Result |
|---|---|---|---|
| 1. CLI list, describe, request ×3, `--stream`, failures, `rivet.capabilities` | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 2. `outputs demo.health`, `outputs --all --json` | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 3. `serve --listen` (run on 127.0.0.1:18800) startup record and access log | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 4. REST health, operations, describe, outputs, request, 422/404, traceparent | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 5. SSE countdown | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 6. Polling open (202), events, unary job, cancel after finish | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 7. WebSocket three refs via fixtures/ws_client.py | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 8. MCP initialize, initialized, tools/list, direct tool, `rivet.outputs`, stdio | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 9. team.json: ada 200, ci 403, no/wrong token 401, ws 404, `--endpoint`, non-loopback refusal | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| `check --strict-docs` (exit 0) | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| `io --check-policy` (exit 0) | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| `io --check-files` (exit 0), `io --by target`, `graph`, JSON manifest | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |

Build: `cargo build` and `cargo build --release` at `829ca43`. Every command above was executed from this folder and the output pasted from that run.

## Known Caveats

- Request, trace, session and MCP session IDs, timestamps, `catalog_version` and `policy_hash` values are generated or content-derived; compare application results, codes and HTTP statuses, not literal IDs.
- Frames of different WebSocket refs interleave differently on each run.
- Port 8080 is a common development port; when it is taken, pick another loopback port consistently.

## Related Documents

- [All sample folders](../README.md) · [demos index](../index.md) · [release verification guide](../demo-2026-0015-v0-1-0-release-verification.md) · [CLI reference](../../manuals/man-2026-0004-cli-reference.md)
- [Installation and quickstart (MAN-2026-0002)](../../manuals/man-2026-0002-installation-and-quickstart.md) · [Serving and surfaces (MAN-2026-0006)](../../manuals/man-2026-0006-serving-and-surfaces.md)
- [Usage reference](../../references/ref-2026-0002-language-and-usage.md) · [Proposal](../../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 7 | 2026-09-28 | Claude | TASK-067 re-verification at 829ca43 (after the INC-2026-0005/0006 fixes): every step re-run with the release binary on 127.0.0.1:18800; output identical except IDs, timestamps and ports; commit references updated; linked the release verification guide DEMO-2026-0015 |
| 6 | 2026-09-28 | Codex | Repaired two links to the absent release-verification guide by linking the current CLI reference. |
| 5 | 2026-09-28 | Claude | TASK-067: executed every step against 0.1.0-dev (073d944) and pasted real output. Fixes: `list --outputs` columns; `describe`, `outputs --all --json`, Completion, SSE, polling receipt/batch (session IDs `ses_…`, `trace_id` in receipts, `seq` on the result event) and WebSocket frames (data frames carry `request_id`/`trace_id`) as printed; MCP `tools/list` built-in set; `/v1/health`, traceparent, cancel-after-finish, `--endpoint`, 401 codes, `rivet.capabilities`, `graph`, `io --check-files`; WebSocket step uses fixtures/ws_client.py (websocat is optional); §7 header; status active; verified_against 0.1.0. |
| 4 | 2026-09-28 | Claude | TASK-005/R26 (ADR-0001, proposal revision 8): `io --by target` header gains ORIGIN, PHASE, NEEDS FILE (still no sites); verified-against revision 8. |
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest: `io --by target` shows no application sites; `io --check-policy` exits 0 with no policy.json. |
| 2 | 2026-09-28 | Claude | UQ-17 rework: added `demo.countdown` and declared/described outputs; removed `--sandbox` (no policy.json = deny-by-default) and added `policies/team.json` for bearer auth; replaced `--transport`/`--mcp` with one `serve` showing REST, SSE, polling, WebSocket (`requests/ws-frames.jsonl`) and direct-tool MCP; added View outputs, strict-docs, bootstrap and exit-code expectations. |
| 1 | 2026-09-28 | Codex | Added draft source files, prerequisites, invocation examples and expected behavior. |
