---
document_id: DEMO-2026-0010
title: "All four gRPC call modes"
document_type: demo
status: active
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 7
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [grpc, sessions, serve, cli, http, poll, ws, mcp]
affected_versions:
  from: "0.2.0"
  to: null
applicable_environments: [development]
audience: [developers, reviewers]
scope: Runnable gRPC demo — unary, server-streaming, client-streaming and bidirectional calls from a pinned FileDescriptorSet, live input for the bidirectional call over the CLI, polling, WebSocket and MCP sessions, and typed gRPC failures, against a shipped grpcio fixture.
reason: User requested sample files in folders with READMEs showing usage; UQ-15 asks for gRPC and live streams on every surface; UQ-17 adds declared outputs; TASK-067 executed every step against the 0.1.0 release candidate. TASK-076 (PLAN-2026-0002) re-executed it against the 0.2.0 release candidate.
dependencies: [PROP-2026-0001, REF-2026-0002]
related_documents: [PLAN-2026-0001, DEMO-2026-0015, DEMO-2026-0013, MAN-2026-0006, MAN-2026-0008, API-2026-0001, API-2026-0002, API-2026-0003, TEST-2026-0017, TEST-2026-0019, TEST-2026-0022, PLAN-2026-0002, DEMO-2026-0020, MIG-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, demo, grpc, sessions, polling, websocket, mcp]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-28
verified_against: "0.2.0"
---

# All four gRPC call modes

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0 and later
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

0.2.0. Verified on 0.2.0-dev at commit `8031baa`, the release candidate (the version string is bumped to 0.2.0 at release, P5), with `target/release/rivet` on macOS 26.4.1 (Darwin 25.4.0, arm64), 2026-09-29. Since 0.2.0 every record on every surface (CLI NDJSON, polling batches, WebSocket frames, MCP `structuredContent`) is a ResponseEnvelope or a stream record, and input is `--data` / `{operation, data}` ([envelope reference](../../api/api-2026-0006-envelopes.md)). gRPC needs the `grpc` Cargo feature (on by default). Every output block below was pasted from that run. Request, trace and session IDs, timestamps, hashes and ports vary.

## Prerequisites

```sh
cargo build --release --features cli       # from the repository root (grpc is a default feature)
export PATH="$PWD/target/release:$PATH"     # the release candidate prints rivet 0.1.0 until the P5 bump
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

`outputs --all --json` prints one `rivet.outputs` envelope on one line; the first of the four entries in its `data`:

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
rivet --file app.rivet request users.grpc_get --data '{"id":"42"}'
rivet --file app.rivet request users.watch --data '{"topic":"changes"}' --stream
rivet --file app.rivet request users.upload --data '{"items":[{"text":"a"},{"text":"b"}]}'
rivet --file app.rivet request chat.exchange --input-jsonl - --stream < chat-input.jsonl
```

#### Expected Output / Response

All exit 0:

```json
{"request_id":"req_01e4b6227d","trace_id":"tr_01e4b6227d","operation":"users.grpc_get","type":"result","status":"ok","data":{"id":"42","name":"Ada"},"error":null,"effects":"committed","data_count":0}
```

```json
{"request_id":"req_01e2a8497d","trace_id":"tr_01e2a8497d","operation":"users.watch","type":"data","seq":1,"data":{"text":"change 1"},"error":null}
{"request_id":"req_01e2a8497d","trace_id":"tr_01e2a8497d","operation":"users.watch","type":"data","seq":2,"data":{"text":"change 2"},"error":null}
{"request_id":"req_01e2a8497d","trace_id":"tr_01e2a8497d","operation":"users.watch","type":"result","seq":3,"status":"ok","data":{"initial_metadata":{},"trailers":{},"status":"OK","status_code":0,"data_count":2},"error":null,"effects":"none","data_count":2}
```

```json
{"request_id":"req_01e1a83c5d","trace_id":"tr_01e1a83c5d","operation":"users.upload","type":"result","status":"ok","data":{"count":2},"error":null,"effects":"committed","data_count":0}
```

```json
{"request_id":"req_01e0976b35","trace_id":"tr_01e0976b35","operation":"chat.exchange","type":"data","seq":1,"data":{"text":"hello"},"error":null}
{"request_id":"req_01e0976b35","trace_id":"tr_01e0976b35","operation":"chat.exchange","type":"data","seq":2,"data":{"text":"goodbye"},"error":null}
{"request_id":"req_01e0976b35","trace_id":"tr_01e0976b35","operation":"chat.exchange","type":"result","seq":3,"status":"ok","data":{"initial_metadata":{},"trailers":{},"status":"OK","status_code":0,"data_count":2},"error":null,"effects":"committed","data_count":2}
```

stdin EOF is `finish_input`; the terminal record's `data` is the opaque gRPC status record (its inner `status: "OK"` is the gRPC status, not the envelope `status`).

### 5. gRPC and input failures

#### Command / Request

```sh
rivet --file app.rivet request users.grpc_get --data '{"id":"7"}'
rivet --file app.rivet request users.watch --data '{"topic":"fail_after_one"}' --stream
rivet --file app.rivet request chat.exchange
printf '{"text":"ok"}\nnot json\n' | rivet --file app.rivet request chat.exchange --input-jsonl - --stream
printf '{"txt":"x"}\n' | rivet --file app.rivet request chat.exchange --input-jsonl - --stream
```

#### Expected Output / Response

NOT_FOUND maps to kind `not_found` (exit 4):

```json
{"request_id":"req_01df8d590d","trace_id":"tr_01df8d590d","operation":"users.grpc_get","type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"grpc.not_found","message":"gRPC /example.Users/GetUser ended with NOT_FOUND (5): no user 7","retryable":false,"source":{"file":"app.rivet","line":15,"column":5,"end_line":18,"end_column":8},"operation_id":"users.grpc_get","details":{"grpc_status":5,"grpc_code":"NOT_FOUND","grpc_message":"no user 7","method":"users/example.Users/GetUser","data_count":0,"trailers":{}}},"effects":"none","data_count":0}
```

A late UNAVAILABLE keeps the data already emitted and ends with one terminal `type: result` record whose `status` is `error` (exit 5):

```json
{"request_id":"req_01def5ce15","trace_id":"tr_01def5ce15","operation":"users.watch","type":"data","seq":1,"data":{"text":"change 1"},"error":null}
{"request_id":"req_01def5ce15","trace_id":"tr_01def5ce15","operation":"users.watch","type":"result","seq":2,"status":"error","data":null,"error":{"kind":"protocol","code":"grpc.unavailable","message":"gRPC /example.Users/Watch ended with UNAVAILABLE (14): backend went away","retryable":false,"source":{"file":"app.rivet","line":33,"column":9,"end_line":35,"end_column":12},"operation_id":"users.watch","details":{"grpc_status":14,"grpc_code":"UNAVAILABLE","grpc_message":"backend went away","method":"users/example.Users/Watch","data_count":1,"trailers":{}}},"effects":"none","data_count":1}
```

Live input is required (exit 2):

```json
{"request_id":"req_01ddf7237d","trace_id":"tr_01ddf7237d","operation":"chat.exchange","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"stream.input_required","message":"`chat.exchange` receives live input; open a session or use --input-jsonl","retryable":false,"operation_id":"chat.exchange"},"effects":"none","data_count":0}
```

A malformed line, or an item that does not match `receives`, cancels the request (exit 2):

```json
{"request_id":"req_01dc80932d","trace_id":"tr_01dc80932d","operation":"chat.exchange","type":"result","seq":1,"status":"error","data":null,"error":{"kind":"validation","code":"validation.input","message":"stdin line 2 is not JSON (column 2); the request was cancelled","retryable":false,"details":{"seq":2,"line":2}},"effects":"none","data_count":0}
{"request_id":"req_01dba8789d","trace_id":"tr_01dba8789d","operation":"chat.exchange","type":"result","seq":1,"status":"error","data":null,"error":{"kind":"validation","code":"validation.input","message":"stdin line 1: input item at text must be text, got missing; the request was cancelled","retryable":false,"details":{"seq":1,"line":1}},"effects":"none","data_count":0}
```

### 6. Polling session with live input

```text
  POST /v1/requests ----> 202 envelope status accepted, data = SessionReceipt {session_id, events_url, input_schema, next_send_seq:1}
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

The HTTP 202 envelope (`status: "accepted"`) carries the SessionReceipt as `data`, with the `receives` schema as `input_schema`:

```json
{"request_id":"req_020060ed42","trace_id":"tr_020060ed42","operation":"chat.exchange","type":"result","status":"accepted","data":{"session_id":"ses_0181be612d","request_id":"req_020060ed42","trace_id":"tr_020060ed42","catalog_version":"sha256:3a4953a7995c576b72bb71c1a25a690ff7754efcac2f893eb9852fb7bc8bbab1","input_schema":{"type":"object","properties":{"text":{"type":"string","description":"Message text to send."}},"required":["text"],"additionalProperties":false},"emits_schema":{"type":"object","properties":{"text":{"type":"string","description":"Echoed message text."}},"required":["text"],"additionalProperties":false},"next_send_seq":1,"expires_at":"2026-09-28T22:27:01Z","events_url":"/v1/requests/ses_0181be612d/events"},"error":null,"effects":"none","data_count":0}
```

The input and finish acks (SessionAck) and the events batch (SessionBatch) keep their session bodies; the batch's `events` are stream records:

```json
{"session_id":"ses_0181be612d","accepted_seq":1,"input_closed":false}
{"session_id":"ses_0181be612d","accepted_seq":null,"input_closed":true}
{"session_id":"ses_0181be612d","events":[{"request_id":"req_020060ed42","trace_id":"tr_020060ed42","operation":"chat.exchange","type":"data","seq":1,"data":{"text":"hello"},"error":null},{"request_id":"req_020060ed42","trace_id":"tr_020060ed42","operation":"chat.exchange","type":"result","seq":2,"status":"ok","data":{"initial_metadata":{},"trailers":{},"status":"OK","status_code":0,"data_count":1},"error":null,"effects":"committed","data_count":1}],"last_seq":2,"terminal":true}
```

A send acknowledgment means queue acceptance, not remote processing; re-posting the same `send_seq` returns the same ack without sending twice. Cancelling an open session returns `{"session_id":"ses_02f91bea6a","request_id":"req_037a507a67","state":"cancelled"}` and its events end with one terminal record whose `status` is `cancelled`:

```json
{"session_id":"ses_02f91bea6a","events":[{"request_id":"req_037a507a67","trace_id":"tr_037a507a67","operation":"chat.exchange","type":"result","seq":1,"status":"cancelled","data":null,"error":{"kind":"cancelled","code":"cancelled.session","message":"the session was cancelled","retryable":false},"effects":"none","data_count":0}],"last_seq":1,"terminal":true}
```

### 7. WebSocket with live input

#### Command / Request

```sh
"$TMPDIR/rivet-grpc-venv/bin/python" fixtures/ws_client.py ws://127.0.0.1:18881/v1/ws < requests/ws-chat.jsonl
```

[requests/ws-chat.jsonl](requests/ws-chat.jsonl) holds a request frame (`{"type":"request","ref":"chat1","operation":"chat.exchange","data":{}}`), two input frames and a finish_input frame for ref `chat1`.

#### Expected Output / Response

```json
{"ref":"chat1","request_id":"req_012af56bed","trace_id":"tr_012af56bed","operation":"chat.exchange","type":"data","seq":1,"data":{"text":"hello"},"error":null}
{"ref":"chat1","request_id":"req_012af56bed","trace_id":"tr_012af56bed","operation":"chat.exchange","type":"data","seq":2,"data":{"text":"goodbye"},"error":null}
{"ref":"chat1","request_id":"req_012af56bed","trace_id":"tr_012af56bed","operation":"chat.exchange","type":"result","seq":3,"status":"ok","data":{"initial_metadata":{},"trailers":{},"status":"OK","status_code":0,"data_count":2},"error":null,"effects":"committed","data_count":2}
```

If the socket closes before the result frame, Rivet cancels `chat1` and joins its cleanup; unlike a polling session, it cannot be resumed.

An input frame that skips a sequence number ends the ref. Its terminal record is numbered after the data records already sent and carries the real `data_count` (INC-2026-0012); here the client waited for the `hello` echo before sending `seq` 3:

```text
 client                                   server
 request chat2 · input seq 1 "hello" ──▶  chat2 data   seq 1 {"text":"hello"}
 input seq 3 "skipped"               ──▶  chat2 result seq 2 status error conflict.input_sequence, data_count 1
```

```json
{"ref":"chat2","request_id":"req_06dc7203d6","trace_id":"tr_06dc7203d6","operation":"chat.exchange","type":"data","seq":1,"data":{"text":"hello"},"error":null}
{"ref":"chat2","request_id":"req_06dc7203d6","trace_id":"tr_06dc7203d6","operation":"chat.exchange","type":"result","seq":2,"status":"error","data":null,"error":{"kind":"conflict","code":"conflict.input_sequence","message":"expected send_seq 2, got 3","retryable":false},"effects":"none","data_count":1}
```

A request refused at open (a wrong param type, an unknown operation) became a request, so it carries request and trace IDs; on polling the same refusal is HTTP 422 with IDs:

```json
{"ref":"u1","request_id":"req_04b6d62a54","trace_id":"tr_04b6d62a54","operation":"users.grpc_get","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.type","message":"parameter `id` must be text, got integer","retryable":false,"operation_id":"users.grpc_get","details":{"field":"id"}},"effects":"none","data_count":0}
{"ref":"u2","request_id":"req_0530740ac9","trace_id":"tr_0530740ac9","operation":"nope.nope","type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"not_found.operation","message":"no operation `nope.nope`","retryable":false,"operation_id":"nope.nope"},"effects":"none","data_count":0}
```

The CLI as a remote client drives the same WebSocket route for live input, and `--timeout` travels in the request frame as `deadline_ms`:

```sh
(echo '{"text":"hello"}'; sleep 6) | rivet --endpoint http://127.0.0.1:18881 request chat.exchange --input-jsonl - --stream --timeout 2s
```

```json
{"request_id":"req_1093b9fbc2","trace_id":"tr_1093b9fbc2","operation":"chat.exchange","type":"data","seq":1,"data":{"text":"hello"},"error":null}
{"request_id":"req_1093b9fbc2","trace_id":"tr_1093b9fbc2","operation":"chat.exchange","type":"result","seq":2,"status":"error","data":null,"error":{"kind":"timeout","code":"grpc.deadline_exceeded","message":"gRPC /example.Users/Chat ended with DEADLINE_EXCEEDED (4): deadline of 1999 ms exceeded","retryable":false,"source":{"file":"app.rivet","line":78,"column":17,"end_line":80,"end_column":20},"operation_id":"chat.exchange","details":{"grpc_status":4,"grpc_code":"DEADLINE_EXCEEDED","grpc_message":"deadline of 1999 ms exceeded","method":"users/example.Users/Chat","data_count":1,"trailers":{}}},"effects":"committed","data_count":1}
```

The deadline fires after 2 s and the exit code is 6. The process itself exits only when stdin reaches EOF (after 6 s here), even though the terminal record has already arrived (see Known Caveats). Without `--timeout`, `rivet --endpoint http://127.0.0.1:18881 request chat.exchange --input-jsonl - --stream < chat-input.jsonl` prints the same three records as step 4 (exit 0).

### 8. MCP session tools

#### Command / Request

Initialize as in [01-catalog](../01-catalog/README.md) step 8 using [requests/initialize.mcp.json](requests/initialize.mcp.json) and [requests/initialized.mcp.json](requests/initialized.mcp.json) against `http://127.0.0.1:18881/mcp`, keep the `mcp-session-id`, then:

```sh
MS='returned-mcp-session-id'
M() { curl -sS http://127.0.0.1:18881/mcp -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' \
        -H "MCP-Session-Id: $MS" -H 'MCP-Protocol-Version: 2025-11-25' "$@"; echo; }
M --data-binary @requests/open.mcp.json                   # note structuredContent.data.session_id
CS='returned-rivet-session-id'
for f in send finish read; do sed "s/SESSION_ID/$CS/" requests/sessions-$f.mcp.json > body.json; M --data-binary @body.json; done
M -d '{"jsonrpc":"2.0","id":20,"method":"tools/call","params":{"name":"users.grpc_get","arguments":{"id":"42"}}}'
kill $SV $FX
```

The bodies are [open](requests/open.mcp.json) (direct tool `chat.exchange`), [send](requests/sessions-send.mcp.json), [finish](requests/sessions-finish.mcp.json) and [read](requests/sessions-read.mcp.json) (`rivet.sessions.*`).

#### Expected Output / Response

`tools/call chat.exchange` returns an `accepted` envelope in `structuredContent` whose `data` is the SessionReceipt (`"session_id":"ses_078e0be28b"`, `input_schema`, `emits_schema`, `next_send_seq:1`, `expires_at`). Each `rivet.sessions.*` call returns an envelope of that built-in whose `data` is the ack or batch:

```json
{"jsonrpc":"2.0","id":11,"result":{"content":["…"],"structuredContent":{"request_id":"req_138df806a1","trace_id":"tr_138df806a1","operation":"rivet.sessions.send","type":"result","status":"ok","data":{"session_id":"ses_078e0be28b","accepted_seq":1,"input_closed":false},"error":null,"effects":"none","data_count":0},"isError":false}}
{"jsonrpc":"2.0","id":12,"result":{"content":["…"],"structuredContent":{"request_id":"req_140b9f8bee","trace_id":"tr_140b9f8bee","operation":"rivet.sessions.finish_input","type":"result","status":"ok","data":{"session_id":"ses_078e0be28b","accepted_seq":null,"input_closed":true},"error":null,"effects":"none","data_count":0},"isError":false}}
{"jsonrpc":"2.0","id":13,"result":{"content":["…"],"structuredContent":{"request_id":"req_158a8e17a3","trace_id":"tr_158a8e17a3","operation":"rivet.sessions.read","type":"result","status":"ok","data":{"session_id":"ses_078e0be28b","events":[{"request_id":"req_12027fa26c","trace_id":"tr_12027fa26c","operation":"chat.exchange","type":"data","seq":1,"data":{"text":"hello"},"error":null},{"request_id":"req_12027fa26c","trace_id":"tr_12027fa26c","operation":"chat.exchange","type":"result","seq":2,"status":"ok","data":{"initial_metadata":{},"trailers":{},"status":"OK","status_code":0,"data_count":1},"error":null,"effects":"committed","data_count":1}],"last_seq":2,"terminal":true},"error":null,"effects":"none","data_count":0},"isError":false}}
```

Unary `users.grpc_get` is an ordinary direct tool: `"structuredContent":{…,"operation":"users.grpc_get","type":"result","status":"ok","data":{"id":"42","name":"Ada"},"error":null,"effects":"committed","data_count":0},"isError":false`. Called through the built-in instead (`{"name":"rivet.request","arguments":{"operation":"users.grpc_get","data":{"id":"7"}}}`), the error envelope names the target operation, not `rivet.request` (INC-2026-0012), and `isError` is true:

```json
{"request_id":"req_177004c16d","trace_id":"tr_177004c16d","operation":"users.grpc_get","type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"grpc.not_found","message":"gRPC /example.Users/GetUser ended with NOT_FOUND (5): no user 7","retryable":false,"source":{"file":"app.rivet","line":15,"column":5,"end_line":18,"end_column":8},"operation_id":"users.grpc_get","details":{"grpc_status":5,"grpc_code":"NOT_FOUND","grpc_message":"no user 7","method":"users/example.Users/GetUser","data_count":0,"trailers":{}}},"effects":"none","data_count":0}
```

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

0.2.0 updates shown here (numbering of the [v0.2.0 release verification guide](../demo-2026-0020-v0-2-0-release-verification.md)):

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-01 | UQ-03/05 / R1 | One record shape on CLI NDJSON, polling batches, WebSocket frames and MCP `structuredContent` | Steps 4–8 | The same `chat.exchange` records on every surface | This README steps 4–8 (2026-09-29, 8031baa) |
| U-02 | UQ-05 / R2 | `status` accepted (polling/MCP open), error (gRPC failures), cancelled (session cancel) | Steps 5, 6, 8 | As shown | This README steps 5, 6, 8 |
| U-04 | UQ-06 / R4 | `{operation, data}` on polling and WS; `--data` on the CLI | Steps 4, 6, 7 | Same results | [requests/open.http.json](requests/open.http.json), [requests/ws-chat.jsonl](requests/ws-chat.jsonl) |

Still verified from 0.1.0 (numbering of [DEMO-2026-0015](../demo-2026-0015-v0-1-0-release-verification.md)):

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-19 | UQ-15 / R19 | gRPC unary, server, client and bidi streaming from a pinned descriptor | Steps 4–5 | Ada; two changes; `{count:2}`; echo; typed gRPC codes | This README steps 4–5 (re-run 2026-09-29, 8031baa); TEST-2026-0017 |
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
| Setup: `protoc`; missing descriptor → `not_found.descriptor` | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 1. `check --strict-docs`, `list`, `outputs` | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 2. `io --by target`, `io --check-policy` (0), `io --check-files` (0), `--needs --include-bootstrap` | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 3. Scratch copy and fixture | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 4. Four call modes on the CLI | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 5. NOT_FOUND, late UNAVAILABLE, input required, bad input lines | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 6. Polling open, input, finish, events, cancel | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 7. WebSocket `chat1` | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 8. MCP direct session tool and `rivet.sessions.*` | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 7. (re-run after INC-2026-0012) WebSocket `chat1`; refused input `chat2` (terminal `seq` 2, `data_count` 1); open refusals `u1`/`u2` with IDs; polling open refusal 422 with IDs; remote CLI `--timeout 2s` over WS → `grpc.deadline_exceeded` after 2 s (exit 6) | Claude | 2026-09-29, commit 7c25175, macOS 26.4.1 arm64 | PASS |
| 8. (re-run after INC-2026-0012) MCP direct session tool, `rivet.sessions.*`, `users.grpc_get`; `rivet.request` error names `users.grpc_get` | Claude | 2026-09-29, commit 7c25175, macOS 26.4.1 arm64 | PASS |

Verified on 0.2.0-dev at commit `8031baa`, the release candidate (`cargo build --release --workspace --all-features`); every command above was executed from this folder or the scratch copy (grpcio fixture, descriptor from `protoc`) and the output pasted from that run. Steps 7 and 8 were re-run on 2026-09-29 at commit `7c25175` (source = `14750b8`) after the INC-2026-0012 fixes, with the same scratch copy, fixture (port 18880) and server (port 18881); their output above is from that run. The request fixtures under `requests/` were already in the 0.2.0 input-envelope form (TASK-019) and needed no change. The 0.1.0 verification (TASK-067, commit 829ca43) is recorded in an earlier revision below.

## Known Caveats

- The fixture is plaintext HTTP/2 (h2c) on loopback; a real service uses `https://` with a trusted certificate (or `tls ca_file`, see [09-quic](../09-quic/README.md)).
- `users.pb` is generated, not committed; the conformance tests compile the same `.proto`.
- The remote CLI (`--endpoint … --input-jsonl -`) exits only when stdin reaches EOF, even after the terminal record has arrived (INC-2026-0012 Remaining Risks; pre-existing).

## Related Documents

- [All sample folders](../README.md) · [demos index](../index.md) · [v0.2.0 release verification guide](../demo-2026-0020-v0-2-0-release-verification.md) · [v0.1.0 guide](../demo-2026-0015-v0-1-0-release-verification.md) · [envelope reference](../../api/api-2026-0006-envelopes.md)
- [Serving and surfaces](../../manuals/man-2026-0006-serving-and-surfaces.md) · [Protocols and connectors](../../manuals/man-2026-0008-protocols-and-connectors.md)
- [REST/SSE/polling API](../../api/api-2026-0001-http-rest-sse-polling.md) · [WebSocket API](../../api/api-2026-0002-websocket-rivet-v1.md) · [MCP API](../../api/api-2026-0003-mcp-server-tools.md)
- [gRPC tests TEST-2026-0017](../../testing/test-2026-0017-grpc.md) · [Session tests TEST-2026-0019](../../testing/test-2026-0019-sessions.md)
- [Usage reference](../../references/ref-2026-0002-language-and-usage.md) · [Proposal](../../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 7 | 2026-09-29 | Claude | INC-2026-0012 re-verification (T-30) at 7c25175: steps 7–8 re-run against the grpcio fixture; step 7 adds the refused-input terminal record (`seq`, real `data_count`), open refusals with IDs and the remote CLI `--timeout` over WS (`deadline_ms`); step 8 re-captured and adds the `rivet.request` error naming the target; remote-CLI stdin caveat; two Verification Record rows. |
| 6 | 2026-09-29 | Claude | TASK-076 (PLAN-2026-0002 D-59): re-executed every step against the 0.2.0 release candidate (8031baa) with the grpcio fixture; `--params` → `--data`; CLI NDJSON, gRPC errors, polling (`accepted` receipt envelope, batches, cancelled terminal record), WebSocket envelope frames with `ref` and MCP session-tool `structuredContent` replaced by 0.2.0 output; 0.2.0 Release Updates; verified_against 0.2.0 |
| 5 | 2026-09-28 | Claude | TASK-067: added fixtures/grpc_fixture.py (port 18880), fixtures/ws_client.py and MCP session request bodies; executed every step against 0.1.0-dev (829ca43) and pasted real output: `not_found.descriptor`, check/list/outputs, manifest (summary line), four call modes, `grpc.not_found` (exit 4), `grpc.unavailable`, `stream.input_required`, `validation.input`, polling (receipt with `input_schema`, ack, batch, cancel), WebSocket frames (with `request_id`/`trace_id`), MCP session tools; completion shape is `{initial_metadata, trailers, status, status_code, data_count}`; websocat replaced by the bundled client; removed draft disclaimers; status active; verified_against 0.1.0. |
| 4 | 2026-09-28 | Claude | TASK-005/R26 (ADR-0001, proposal revision 8): `io --by target` gains ORIGIN (`endpoint`, `grpc`, `with grpc`), PHASE and NEEDS FILE; connector `descriptor` shown as a bootstrap `load` read with new fields; `io --needs --include-bootstrap` excerpt. |
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest: endpoint + four call-mode sites by target and `io --check-policy` (all eight allowed). |
| 2 | 2026-09-28 | Claude | UQ-17: renamed `users.get` → `users.grpc_get` (E12); declared outputs/emits/receives; quoted durations with options first; `--policy policy.json` dropped (auto-discovered, with `serve` block); one `serve` replaces `--transport … --mcp`; polling routes replace generic session request files (removed `requests/read.http.json`); added `requests/ws-chat.jsonl`; View outputs, strict-docs, bootstrap and exit codes. |
| 1 | 2026-09-28 | Codex | Added draft source files, prerequisites, invocation examples and expected behavior. |
