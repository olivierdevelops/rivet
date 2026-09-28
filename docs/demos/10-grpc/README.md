---
document_id: DEMO-2026-0010
title: "All four gRPC call modes"
document_type: demo
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 5
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [grpc, sessions, serve, cli, http, poll, ws, mcp]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development]
audience: [developers, reviewers]
scope: Runnable gRPC demo — unary, server-streaming, client-streaming and bidirectional calls from a pinned FileDescriptorSet, live input for the bidirectional call over the CLI, polling, WebSocket and MCP sessions, and typed gRPC failures, against a shipped grpcio fixture.
reason: User requested sample files in folders with READMEs showing usage; UQ-15 asks for gRPC and live streams on every surface; UQ-17 adds declared outputs; TASK-067 executed every step against the 0.1.0 release candidate.
dependencies: [PROP-2026-0001, REF-2026-0002]
related_documents: [PLAN-2026-0001, DEMO-2026-0015, DEMO-2026-0013, MAN-2026-0006, MAN-2026-0008, API-2026-0001, API-2026-0002, API-2026-0003, TEST-2026-0017, TEST-2026-0019, TEST-2026-0022]
supersedes: null
superseded_by: null
tags: [rivet, demo, grpc, sessions, polling, websocket, mcp]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-28
verified_against: "0.1.0"
---

# All four gRPC call modes

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** grpc, sessions, serve, cli, http, poll, ws, mcp

## Purpose

All four gRPC call modes. Delivery stage: **B**. Read [app.rivet](app.rivet) alongside this walkthrough.

| Operation ID | Mode | Output | Emits / receives | Behavior |
|---|---|---|---|---|
| `users.grpc_get` | unary | `object {id, name}` | — | GetUser. Named so it never collides with the HTTP `users.get` in 03-http. |
| `users.watch` | server streaming | `json` (gRPC completion) | emits `object {text}` | Watch. |
| `users.upload` | client streaming | `object {count}` | — | Upload of a finite list. |
| `chat.exchange` | bidirectional | `json` (gRPC completion) | emits and receives `object {text}` | Live Chat. |

```text
   users.grpc_get   1 req  -> 1 resp            users.watch    1 req  -> N resp (emit)
   users.upload     N req  -> 1 resp            chat.exchange  N req (receives) <-> N resp (emit)

   live input for chat.exchange, by surface:
     CLI        --input-jsonl - --stream          (stdin EOF = finish_input)
     polling    POST /v1/requests/{id}/input      POST /v1/requests/{id}/finish_input
     WebSocket  {"type":"input",...}              {"type":"finish_input",...}
     MCP        tools/call chat.exchange -> SessionReceipt, then rivet.sessions.* tools
```

## Verified Against Version

0.1.0. Verified on 0.1.0-dev at commit `829ca43`, the release candidate (the version bump to 0.1.0 happens at release, P5), with `target/release/rivet` on macOS 26.4.1 (Darwin 25.4.0, arm64), 2026-09-28. Every output block below was pasted from that run. Request, trace and session IDs, timestamps, hashes and ports vary.

## Prerequisites

```sh
cargo build --release                       # from the repository root
export PATH="$PWD/target/release:$PATH"     # `rivet --version` prints rivet 0.1.0
python3 -m venv "$TMPDIR/rivet-grpc-venv" && "$TMPDIR/rivet-grpc-venv/bin/pip" install grpcio protobuf websockets
```

- `protoc` to generate the descriptor set (host preparation, never an implicit subprocess inside Rivet).
- [fixtures/grpc_fixture.py](fixtures/grpc_fixture.py) serves `example.Users` over plaintext HTTP/2 from the same `users.pb`. Contracts: `GetUser("42")` → Ada, other IDs → NOT_FOUND; `Watch("changes")` → two messages then OK, `Watch("fail_after_one")` → one message then UNAVAILABLE; `Upload` counts messages; `Chat` echoes each message and finishes after the client half-closes.
- [fixtures/ws_client.py](fixtures/ws_client.py) is a minimal `rivet.v1` WebSocket client (needs `websockets`).
- `curl`. Loopback ports 18880 (fixture) and 18881 (serve) free.

## Setup

```sh
cd docs/demos/10-grpc
protoc --proto_path=schemas --include_imports --descriptor_set_out=schemas/users.pb schemas/users.proto
```

[schemas/users.proto](schemas/users.proto) declares all four methods. Without `schemas/users.pb` every command fails with `not_found.descriptor` (exit 4) and the hint `generate it with protoc --include_imports --descriptor_set_out=… before loading the bundle`. [policy.json](policy.json) grants the exact origin and the four methods, and its `serve` block mounts all five surfaces with loopback auth `none`. `https://users.example.com:443` is a placeholder; steps 3–8 run against the fixture in a scratch copy.

## Steps

### 1. Check, list and view outputs

#### Command / Request

```sh
rivet --file app.rivet check --strict-docs
rivet --file app.rivet list
rivet --file app.rivet outputs chat.exchange
rivet --file app.rivet outputs --all --json
```

#### Expected Output / Response

```text
ok: 4 operations, 1 connectors, 0 auth profiles
```

```text
ID              NAME                   DESCRIPTION
users.grpc_get  Get a user over gRPC   Read one user from the gRPC service.
users.watch     Watch user changes     Emit changes until the server completes or the deadline expires.
users.upload    Upload messages        Upload a finite batch and return the accepted count.
chat.exchange   Chat with the service  Send and receive chat messages within one bounded call.
```

```text
chat.exchange — Chat with the service
output  json     gRPC completion (final OK status and bounded trailers); opaque, not structurally checked.
emits   object   One echoed example.ChatMessage per server message.
  text    text     required  Echoed message text.
receives object   One example.ChatMessage per caller input item.
  text    text     required  Message text to send.
errors   —
```

`outputs --all --json` prints four entries on one line; the first:

```json
{"id":"chat.exchange","output":{"description":"gRPC completion (final OK status and bounded trailers); opaque, not structurally checked."},"emits":{"type":"object","properties":{"text":{"type":"string","description":"Echoed message text."}},"required":["text"],"additionalProperties":false,"description":"One echoed example.ChatMessage per server message."},"receives":{"type":"object","properties":{"text":{"type":"string","description":"Message text to send."}},"required":["text"],"additionalProperties":false,"description":"One example.ChatMessage per caller input item."},"errors":[]}
```

All exit 0. The `emits`/`receives` descriptions and the field descriptions are all shown.

### 2. I/O manifest and the descriptor bootstrap read

#### Command / Request

```sh
rivet --file app.rivet io --by target
rivet --file app.rivet io --check-policy
rivet --file app.rivet io --check-files
rivet --file app.rivet io --needs --include-bootstrap
```

#### Expected Output / Response

A network site for the connector endpoint and one call site per mode:

```text
TARGET                         ACCESS               CAPABILITY     ORIGIN     PHASE    NEEDS FILE  USED BY
https://users.example.com:443  connect POST (grpc)  allow_network  endpoint   connect  —           chat.exchange, users.grpc_get, users.upload, users.watch
users/example.Users/Chat       call bidi            allow_grpc     with grpc  body     —           chat.exchange
users/example.Users/GetUser    call unary           allow_grpc     grpc       body     —           users.grpc_get
users/example.Users/Upload     call client_stream   allow_grpc     with grpc  body     —           users.upload
users/example.Users/Watch      call server_stream   allow_grpc     with grpc  body     —           users.watch
```

`--check-policy` (exit 0):

```text
OPERATION       KIND     ACCESS              TARGET                         KNOWLEDGE  SOURCE        DECISION
chat.exchange   network  connect POST        https://users.example.com:443  exact      app.rivet:2   allowed
chat.exchange   grpc     call bidi           users/example.Users/Chat       exact      app.rivet:68  allowed
users.grpc_get  network  connect POST        https://users.example.com:443  exact      app.rivet:2   allowed
users.grpc_get  grpc     call unary          users/example.Users/GetUser    exact      app.rivet:15  allowed
users.upload    network  connect POST        https://users.example.com:443  exact      app.rivet:2   allowed
users.upload    grpc     call client_stream  users/example.Users/Upload     exact      app.rivet:47  allowed
users.watch     network  connect POST        https://users.example.com:443  exact      app.rivet:2   allowed
users.watch     grpc     call server_stream  users/example.Users/Watch      exact      app.rivet:30  allowed
8 allowed
```

`io --check-files` prints four `… needs no existing files.` lines and `0 files` (exit 0). The descriptor is a bootstrap read, never granted by policy.json (exit 0):

```text
bundle load needs:
  ./schemas/users.pb  (descriptor)
chat.exchange needs no existing files.
users.grpc_get needs no existing files.
users.upload needs no existing files.
users.watch needs no existing files.
```

### 3. Local fixture run

#### Command / Request

```sh
WORK="$(mktemp -d)"; cp -R app.rivet policy.json schemas requests fixtures chat-input.jsonl "$WORK/"; cd "$WORK"
sed -i.bak -e 's#https://users\.example\.com:443#http://127.0.0.1:18880#' app.rivet policy.json
"$TMPDIR/rivet-grpc-venv/bin/python" fixtures/grpc_fixture.py schemas/users.pb 18880 > fixture.log 2>&1 & FX=$!
rivet --file app.rivet io --check-policy
```

#### Expected Output / Response

`fixture.log` shows `grpc fixture on http://127.0.0.1:18880 (example.Users)` (wait for it). The eight rows are `allowed` with target `http://127.0.0.1:18880` for the network rows, and the summary is `8 allowed` (exit 0).

### 4. The four call modes on the CLI

#### Command / Request

```sh
rivet --file app.rivet request users.grpc_get --params '{"id":"42"}'
rivet --file app.rivet request users.watch --params '{"topic":"changes"}' --stream
rivet --file app.rivet request users.upload --params '{"items":[{"text":"a"},{"text":"b"}]}'
rivet --file app.rivet request chat.exchange --params '{}' --input-jsonl - --stream < chat-input.jsonl
```

#### Expected Output / Response

All exit 0:

```json
{"request_id":"req_0103d5979d","trace_id":"tr_0103d5979d","result":{"id":"42","name":"Ada"},"data_count":0,"effects":"committed"}
```

```json
{"request_id":"req_0100491695","trace_id":"tr_0100491695","seq":1,"type":"data","data":{"text":"change 1"}}
{"request_id":"req_0100491695","trace_id":"tr_0100491695","seq":2,"type":"data","data":{"text":"change 2"}}
{"request_id":"req_0100491695","trace_id":"tr_0100491695","result":{"initial_metadata":{},"trailers":{},"status":"OK","status_code":0,"data_count":2},"data_count":2,"effects":"none","type":"result"}
```

```json
{"request_id":"req_01ffde6cfd","trace_id":"tr_01ffde6cfd","result":{"count":2},"data_count":0,"effects":"committed"}
```

```json
{"request_id":"req_01fe1f91c5","trace_id":"tr_01fe1f91c5","seq":1,"type":"data","data":{"text":"hello"}}
{"request_id":"req_01fe1f91c5","trace_id":"tr_01fe1f91c5","seq":2,"type":"data","data":{"text":"goodbye"}}
{"request_id":"req_01fe1f91c5","trace_id":"tr_01fe1f91c5","result":{"initial_metadata":{},"trailers":{},"status":"OK","status_code":0,"data_count":2},"data_count":2,"effects":"committed","type":"result"}
```

stdin EOF is `finish_input`; the completion is the opaque gRPC status record.

### 5. gRPC and input failures

#### Command / Request

```sh
rivet --file app.rivet request users.grpc_get --params '{"id":"7"}'
rivet --file app.rivet request users.watch --params '{"topic":"fail_after_one"}' --stream
rivet --file app.rivet request chat.exchange --params '{}'
printf '{"text":"ok"}\nnot json\n' | rivet --file app.rivet request chat.exchange --params '{}' --input-jsonl - --stream
printf '{"txt":"x"}\n' | rivet --file app.rivet request chat.exchange --params '{}' --input-jsonl - --stream
```

#### Expected Output / Response

NOT_FOUND maps to kind `not_found` (exit 4):

```json
{"request_id":"req_01016f3b45","trace_id":"tr_01016f3b45","error":{"kind":"not_found","code":"grpc.not_found","message":"gRPC /example.Users/GetUser ended with NOT_FOUND (5): no user 7","retryable":false,"effects":"none","source":{"file":"app.rivet","line":15,"column":5,"end_line":18,"end_column":8},"operation_id":"users.grpc_get","details":{"grpc_status":5,"grpc_code":"NOT_FOUND","grpc_message":"no user 7","method":"users/example.Users/GetUser","data_count":0,"trailers":{}}}}
```

A late UNAVAILABLE keeps the data already emitted and ends with one terminal error (exit 5):

```json
{"request_id":"req_01008bf25d","trace_id":"tr_01008bf25d","seq":1,"type":"data","data":{"text":"change 1"}}
{"request_id":"req_01008bf25d","trace_id":"tr_01008bf25d","error":{"kind":"protocol","code":"grpc.unavailable","message":"gRPC /example.Users/Watch ended with UNAVAILABLE (14): backend went away","retryable":false,"effects":"none","source":{"file":"app.rivet","line":33,"column":9,"end_line":35,"end_column":12},"operation_id":"users.watch","details":{"grpc_status":14,"grpc_code":"UNAVAILABLE","grpc_message":"backend went away","method":"users/example.Users/Watch","data_count":1,"trailers":{}}}}
```

Live input is required (exit 2):

```json
{"request_id":"req_01fd9cad05","trace_id":"tr_01fd9cad05","error":{"kind":"validation","code":"stream.input_required","message":"`chat.exchange` receives live input; open a session or use --input-jsonl","retryable":false,"effects":"none","operation_id":"chat.exchange"}}
```

A malformed line, or an item that does not match `receives`, cancels the request (exit 2):

```json
{"request_id":"req_…","trace_id":"tr_…","error":{"kind":"validation","code":"validation.input","message":"stdin line 2 is not JSON (column 2); the request was cancelled","retryable":false,"effects":"none","details":{"seq":2,"line":2}}}
{"request_id":"req_…","trace_id":"tr_…","error":{"kind":"validation","code":"validation.input","message":"stdin line 1: input item at text must be text, got missing; the request was cancelled","retryable":false,"effects":"none","details":{"seq":1,"line":1}}}
```

### 6. Polling session with live input

```text
  POST /v1/requests ----> 202 SessionReceipt {session_id, events_url, input_schema, next_send_seq:1}
  POST .../input  {send_seq:1, data}  ----> SessionAck {accepted_seq:1, input_closed:false}
  POST .../finish_input {}            ----> SessionAck {accepted_seq:null, input_closed:true}
  GET  .../events?after_seq=0         ----> SessionBatch {events:[data..., result], terminal:true}
```

#### Command / Request

```sh
rivet --file app.rivet serve --listen 127.0.0.1:18881 2>serve.err & SV=$!
curl -sS -X POST http://127.0.0.1:18881/v1/requests \
  -H 'Content-Type: application/json' --data-binary @requests/open.http.json
S='REPLACE_WITH_SESSION_ID'
curl -sS -X POST http://127.0.0.1:18881/v1/requests/$S/input \
  -H 'Content-Type: application/json' --data-binary @requests/send.http.json
curl -sS -X POST http://127.0.0.1:18881/v1/requests/$S/finish_input \
  -H 'Content-Type: application/json' --data-binary @requests/finish_input.http.json
curl -sS "http://127.0.0.1:18881/v1/requests/$S/events?after_seq=0&wait_ms=1000"
```

To stop early instead, open another session and post [requests/cancel.http.json](requests/cancel.http.json) to `/v1/requests/$S/cancel`.

#### Expected Output / Response

The receipt (HTTP 202) carries the `receives` schema as `input_schema`:

```json
{"session_id":"ses_01bd7838d5","request_id":"req_01bd7840e5","trace_id":"tr_01bd7840e5","catalog_version":"sha256:3a4953a7995c576b72bb71c1a25a690ff7754efcac2f893eb9852fb7bc8bbab1","input_schema":{"type":"object","properties":{"text":{"type":"string","description":"Message text to send."}},"required":["text"],"additionalProperties":false},"emits_schema":{"type":"object","properties":{"text":{"type":"string","description":"Echoed message text."}},"required":["text"],"additionalProperties":false},"next_send_seq":1,"expires_at":"2026-09-28T10:02:07Z","events_url":"/v1/requests/ses_01bd7838d5/events"}
```

```json
{"session_id":"ses_01bd7838d5","accepted_seq":1,"input_closed":false}
{"session_id":"ses_01bd7838d5","accepted_seq":null,"input_closed":true}
{"session_id":"ses_01bd7838d5","events":[{"request_id":"req_01bd7840e5","trace_id":"tr_01bd7840e5","seq":1,"type":"data","data":{"text":"hello"}},{"request_id":"req_01bd7840e5","trace_id":"tr_01bd7840e5","result":{"initial_metadata":{},"trailers":{},"status":"OK","status_code":0,"data_count":1},"data_count":1,"effects":"committed","type":"result","seq":2}],"last_seq":2,"terminal":true}
```

A send acknowledgment means queue acceptance, not remote processing; re-posting the same `send_seq` returns the same ack without sending twice. Cancelling an open session returns `{"session_id":"ses_02f8e2663a","request_id":"req_02f8e26e6a","state":"cancelled"}` and its events end with one `cancelled.session` error event (`"terminal":true`).

### 7. WebSocket with live input

#### Command / Request

```sh
"$TMPDIR/rivet-grpc-venv/bin/python" fixtures/ws_client.py ws://127.0.0.1:18881/v1/ws < requests/ws-chat.jsonl
```

[requests/ws-chat.jsonl](requests/ws-chat.jsonl) holds a request frame, two input frames and a finish_input frame for ref `chat1`.

#### Expected Output / Response

```json
{"type":"data","ref":"chat1","request_id":"req_036dfeaa1f","trace_id":"tr_036dfeaa1f","seq":1,"data":{"text":"hello"}}
{"type":"data","ref":"chat1","request_id":"req_036dfeaa1f","trace_id":"tr_036dfeaa1f","seq":2,"data":{"text":"goodbye"}}
{"type":"result","ref":"chat1","completion":{"request_id":"req_036dfeaa1f","trace_id":"tr_036dfeaa1f","result":{"initial_metadata":{},"trailers":{},"status":"OK","status_code":0,"data_count":2},"data_count":2,"effects":"committed"}}
```

If the socket closes before the result frame, Rivet cancels `chat1` and joins its cleanup; unlike a polling session, it cannot be resumed.

### 8. MCP session tools

#### Command / Request

Initialize as in [01-catalog](../01-catalog/README.md) step 8 using [requests/initialize.mcp.json](requests/initialize.mcp.json) and [requests/initialized.mcp.json](requests/initialized.mcp.json) against `http://127.0.0.1:18881/mcp`, keep the `mcp-session-id`, then:

```sh
MS='returned-mcp-session-id'
M() { curl -sS http://127.0.0.1:18881/mcp -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' \
        -H "MCP-Session-Id: $MS" -H 'MCP-Protocol-Version: 2025-11-25' "$@"; echo; }
M --data-binary @requests/open.mcp.json                   # note structuredContent.session_id
CS='returned-rivet-session-id'
for f in send finish read; do sed "s/SESSION_ID/$CS/" requests/sessions-$f.mcp.json > body.json; M --data-binary @body.json; done
M -d '{"jsonrpc":"2.0","id":20,"method":"tools/call","params":{"name":"users.grpc_get","arguments":{"id":"42"}}}'
kill $SV $FX
```

The bodies are [open](requests/open.mcp.json) (direct tool `chat.exchange`), [send](requests/sessions-send.mcp.json), [finish](requests/sessions-finish.mcp.json) and [read](requests/sessions-read.mcp.json) (`rivet.sessions.*`).

#### Expected Output / Response

`tools/call chat.exchange` returns a SessionReceipt in `structuredContent` (`"session_id":"ses_04e92e888c"`, `input_schema`, `emits_schema`, `next_send_seq:1`, `expires_at`). Each `rivet.sessions.*` call returns a Completion whose `result` is the ack or batch:

```json
{"jsonrpc":"2.0","id":11,"result":{"content":[…],"structuredContent":{"request_id":"req_056b2605b9","trace_id":"tr_056b2605b9","result":{"session_id":"ses_04e92e888c","accepted_seq":1,"input_closed":false},"data_count":0,"effects":"none"},"isError":false}}
{"jsonrpc":"2.0","id":12,"result":{"content":[…],"structuredContent":{"request_id":"req_06e3e1ef0e","trace_id":"tr_06e3e1ef0e","result":{"session_id":"ses_04e92e888c","accepted_seq":null,"input_closed":true},"data_count":0,"effects":"none"},"isError":false}}
{"jsonrpc":"2.0","id":13,"result":{"content":[…],"structuredContent":{"request_id":"req_0762697c9b","trace_id":"tr_0762697c9b","result":{"session_id":"ses_04e92e888c","events":[{"request_id":"req_04e92e815c","trace_id":"tr_04e92e815c","seq":1,"type":"data","data":{"text":"hello"}},{"request_id":"req_04e92e815c","trace_id":"tr_04e92e815c","result":{"initial_metadata":{},"trailers":{},"status":"OK","status_code":0,"data_count":1},"data_count":1,"effects":"committed","type":"result","seq":2}],"last_seq":2,"terminal":true},"data_count":0,"effects":"none"},"isError":false}}
```

Unary `users.grpc_get` is an ordinary direct tool: `"structuredContent":{…,"result":{"id":"42","name":"Ada"},"data_count":0,"effects":"committed"},"isError":false`.

## Effects and policy

```text
  outcome table (verified in steps 4–8)
  ┌──────────────────────────────────────────┬───────────────────────┬──────┬───────────┐
  │ situation                                │ code                  │ exit │ effects   │
  ├──────────────────────────────────────────┼───────────────────────┼──────┼───────────┤
  │ unary / client stream / bidi succeed     │ —                     │  0   │ committed │
  │ server stream succeeds                   │ —                     │  0   │ none      │
  │ status NOT_FOUND                         │ grpc.not_found        │  4   │ none      │
  │ UNAVAILABLE after one message            │ grpc.unavailable      │  5   │ none      │
  │ bidi without live input                  │ stream.input_required │  2   │ none      │
  │ malformed or schema-invalid input line   │ validation.input      │  2   │ none      │
  │ polling session cancelled                │ cancelled.session     │  —   │ none      │
  └──────────────────────────────────────────┴───────────────────────┴──────┴───────────┘
```

[policy.json](policy.json) grants the exact origin and four specific methods; removing one method from `allow_grpc` denies only that operation's call row. All channels and streams belong to the request scope.

## Release Updates

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-19 | UQ-15 / R19 | gRPC unary, server, client and bidi streaming from a pinned descriptor | Steps 4–5 | Ada; two changes; `{count:2}`; echo; typed gRPC codes | This README steps 4–5 (2026-09-28, 829ca43); TEST-2026-0017 |
| U-22 | UQ-15/08/17 / R22 | Live input sessions over CLI, polling, WebSocket and MCP | Steps 4, 6–8 | Echoed `hello` on every surface | This README steps 4, 6–8; TEST-2026-0019 |
| U-25 | UQ-17 / R25 | One serve mounts polling, WS and MCP for the same catalog | Steps 6–8 | 202 receipt; WS frames; MCP session tools | This README steps 6–8; TEST-2026-0022 |
| U-23 | UQ-17 / R23 | Declared `emits` and `receives`; input validated against `receives` | Steps 1, 5 | Schemas; `validation.input` | This README steps 1, 5; TEST-2026-0020 |

## Cleanup

```sh
kill $SV $FX 2>/dev/null
cd - && rm -rf "$WORK"
rm -f schemas/users.pb                       # generated in Setup; keep users.proto
rm -rf "$TMPDIR/rivet-grpc-venv"             # optional
```

## Verification Record

| Step | Verified By | Verified At | Result |
|---|---|---|---|
| Setup: `protoc`; missing descriptor → `not_found.descriptor` | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 1. `check --strict-docs`, `list`, `outputs` | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 2. `io --by target`, `io --check-policy` (0), `io --check-files` (0), `--needs --include-bootstrap` | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 3. Scratch copy and fixture | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 4. Four call modes on the CLI | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 5. NOT_FOUND, late UNAVAILABLE, input required, bad input lines | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 6. Polling open, input, finish, events, cancel | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 7. WebSocket `chat1` | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 8. MCP direct session tool and `rivet.sessions.*` | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |

Build: `cargo build` and `cargo build --release` at 829ca43; every command above was executed from this folder or the scratch copy and the output pasted from that run.

## Known Caveats

- The fixture is plaintext HTTP/2 (h2c) on loopback; a real service uses `https://` with a trusted certificate (or `tls ca_file`, see [09-quic](../09-quic/README.md)).
- `users.pb` is generated, not committed; the conformance tests compile the same `.proto`.

## Related Documents

- [All sample folders](../README.md) · [demos index](../index.md) · [release verification guide](../demo-2026-0015-v0-1-0-release-verification.md)
- [Serving and surfaces](../../manuals/man-2026-0006-serving-and-surfaces.md) · [Protocols and connectors](../../manuals/man-2026-0008-protocols-and-connectors.md)
- [REST/SSE/polling API](../../api/api-2026-0001-http-rest-sse-polling.md) · [WebSocket API](../../api/api-2026-0002-websocket-rivet-v1.md) · [MCP API](../../api/api-2026-0003-mcp-server-tools.md)
- [gRPC tests TEST-2026-0017](../../testing/test-2026-0017-grpc.md) · [Session tests TEST-2026-0019](../../testing/test-2026-0019-sessions.md)
- [Usage reference](../../references/ref-2026-0002-language-and-usage.md) · [Proposal](../../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 5 | 2026-09-28 | Claude | TASK-067: added fixtures/grpc_fixture.py (port 18880), fixtures/ws_client.py and MCP session request bodies; executed every step against 0.1.0-dev (829ca43) and pasted real output: `not_found.descriptor`, check/list/outputs, manifest (summary line), four call modes, `grpc.not_found` (exit 4), `grpc.unavailable`, `stream.input_required`, `validation.input`, polling (receipt with `input_schema`, ack, batch, cancel), WebSocket frames (with `request_id`/`trace_id`), MCP session tools; completion shape is `{initial_metadata, trailers, status, status_code, data_count}`; websocat replaced by the bundled client; removed draft disclaimers; status active; verified_against 0.1.0. |
| 4 | 2026-09-28 | Claude | TASK-005/R26 (ADR-0001, proposal revision 8): `io --by target` gains ORIGIN (`endpoint`, `grpc`, `with grpc`), PHASE and NEEDS FILE; connector `descriptor` shown as a bootstrap `load` read with new fields; `io --needs --include-bootstrap` excerpt. |
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest: endpoint + four call-mode sites by target and `io --check-policy` (all eight allowed). |
| 2 | 2026-09-28 | Claude | UQ-17: renamed `users.get` → `users.grpc_get` (E12); declared outputs/emits/receives; quoted durations with options first; `--policy policy.json` dropped (auto-discovered, with `serve` block); one `serve` replaces `--transport … --mcp`; polling routes replace generic session request files (removed `requests/read.http.json`); added `requests/ws-chat.jsonl`; View outputs, strict-docs, bootstrap and exit codes. |
| 1 | 2026-09-28 | Codex | Added draft source files, prerequisites, invocation examples and expected behavior. |
