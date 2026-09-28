---
document_id: SYS-2026-0007
title: "Rivet duplex sessions"
document_type: system
status: active
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 3
authors: [Claude]
owner: Project maintainer
component_owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [sessions, execution, ws, poll]
affected_versions:
  from: "0.1.0"
  to: null
last_verified_version: "0.2.0-rc (main at 8031baa)"
next_review_date: 2026-10-29
review_cycle: on-release
confidentiality: internal
scope: The host-owned live session runtime (open, send, finish input, read, cancel, retention, limits) and how the CLI, rivet.sessions.* built-ins, HTTP polling and WebSocket surfaces project it in Rivet 0.1.0.
reason: Streaming and duplex operations (emits / receives) outlive a single HTTP exchange; maintainers and client authors need the exact session state machine, sequencing rules, limits and per-surface behaviour as implemented (PLAN-2026-0001 D-21).
related_documents: [PLAN-2026-0002, API-2026-0006, SYS-2026-0010, PROP-2026-0001, PLAN-2026-0001, SYS-2026-0002, SYS-2026-0004, SYS-2026-0005, SYS-2026-0008]
supersedes: null
superseded_by: null
tags: [rivet, system, sessions, streaming, duplex, polling, websocket]
---

# Rivet duplex sessions

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** sessions, execution, ws, poll
> **Last Verified Version:** 0.2.0-rc (main at 8031baa)

## Summary

A **session** is a principal-owned, host-owned lifetime for one top-level request. It gives
an operation that `emits` items and/or `receives` live input a sequenced input queue and a
bounded, replayable output event log. The five session use cases live in
`src/features/sessions/`; the only driver is `SessionHost` (`src/infra/session_driver.rs`),
built once per `Runtime` by `session_host` in `src/orchestrator/setup_library.rs`. The run
itself is an ordinary request through the shared dispatcher (`Runtime::dispatch_session` →
`dispatch_request`, SYS-2026-0002); its input channel appears inside the operation as
`incoming`.

```text
          rivet.sessions.*        POST/GET /v1/requests…      GET /v1/ws (rivet.v1)       library
          (/v1/request, MCP,      (setup_poll.rs →            (setup_ws.rs →              Runtime::open_session …
           CLI request)            serve.project_polling)      serve.multiplex_ws)
                 \                       |                          |                       /
                  \                      |                          |                      /
                   v                     v                          v                     v
   +-------------------------------------------------------------------------------------------+
   | sessions use cases: open_session · send_input · finish_input · read_events · cancel_session |
   +---------------------------------------------+---------------------------------------------+
                                                 v
   +-------------------------------------------------------------------------------------------+
   | SessionHost (SessionDriver)   map<session_id, Session>                                    |
   |   Session { owner, request_id, trace_id, InputSequencer, EventLog(≤16 frames, ≤32 MiB),   |
   |             input tx(16), CancelToken, run task, read_lock }                              |
   |   BufferBudget (host-wide, limits.max_buffered_bytes) · background sweeper                |
   +--------+-------------------------------------------------+--------------------------------+
            | launch (spawned task)                           ^ SessionSink.send (blocks at 16)
            v                                                 |
   Runtime::dispatch_session ─▶ Interpreter.attach_input ─▶ dispatch_request ─▶ `incoming`, `emit`

   CLI `request ID --stream --input-jsonl -` (local) feeds dispatch_session directly (no SessionHost);
   with --endpoint it drives a WebSocket ref instead.
```

## Responsibilities

| Responsibility | Where |
|---|---|
| Validate open input (non-empty ID, object params) | `sessions.open_session` (`src/features/sessions/open_session.rs`) |
| Resolve the public catalog entry, validate params with the dispatcher rules, enforce 8 live sessions per principal, mint IDs, spawn the run | `SessionHost::open` |
| Sequence and enqueue input (`send_seq`, payload hash, 16-slot queue, 5 s action deadline) | `sessions.send_input`, `InputSequencer` (`src/domain/sessions.rs`), `SessionHost::send` |
| Half-close input | `sessions.finish_input`, `SessionHost::finish_input` |
| Long-poll the event log with acknowledgement cursor | `sessions.read_events`, `EventLog`, `SessionHost::read` |
| Cancel the run (fire its token), join within the 5 s grace, record one terminal event — a cancel requested before completion wins | `sessions.cancel_session`, `SessionHost::cancel`, `Session::request_cancel` |
| Expire retained sessions (60 s) and cancel idle ones (60 s) from a background sweeper | `SessionHost::start_sweeper`, `sweep` |
| Reserve every retained output event against the host byte budget | `BufferBudget` (`limits.max_buffered_bytes`) |
| Cancel every session on host shutdown | `SessionHost::cancel_all` (`cancelled.shutdown`) |
| Project sessions onto HTTP polling and WebSocket | `serve.project_polling`, `serve.multiplex_ws` |

## Boundaries and Non-Responsibilities

- Sessions do not execute operation bodies; the dispatcher and interpreter do
  (SYS-2026-0002). Sessions supply the sink and the input feed.
- Authentication and the per-operation principal check are done by the surfaces and
  `serve.authorize_operation` (SYS-2026-0004) before `open`.
- Protocol-level duplex I/O (gRPC bidi, WebSocket client, processes) is in adapters
  (SYS-2026-0005); a session only feeds `incoming` and collects `emit`.
- No persistence: sessions vanish with the process; there is no resume.

## Architecture

### Session state machine

```text
                          open (validated, ≤ 8 live per principal)
                                        │
                                        ▼
            ┌──────────────────────────────────────────────┐
            │ active, input open                           │  (only when the op declares `receives`;
            │   send(seq) ─▶ enqueue | duplicate ack       │   otherwise input starts closed)
            │   read(after_seq) ─▶ batch                   │
            └──────────┬──────────────────────┬────────────┘
                       │ finish_input         │ cancel / idle 60 s (sweeper) / shutdown
                       ▼                      │
            ┌──────────────────────────┐      │
            │ active, input closed     │      │
            │   sends → conflict.      │      │
            │     input_closed         │      │
            │   read(after_seq)        │      │
            └──────────┬───────────────┘      │
                       │ run ends             │ cancel: record cancel-requested, drop input,
                       │ (result | error)     │ fire token, join ≤ 5 s (+1 s, then abort),
                       │                      │ push cancelled error (wins over a later result)
                       ▼                      ▼
            ┌──────────────────────────────────────────────┐
            │ terminal: exactly one result|error event      │
            │ retained ≤ 60 s (retention_ms) for reads      │
            └──────────────────────┬───────────────────────┘
                                   │ background sweep after 60 s
                                   ▼
                               expired (removed → not_found.session)
```

Terminal state names come from the last event (`terminal_state` in
`src/domain/sessions.rs`): `result` → `succeeded`; `error` of kind `cancelled` →
`cancelled`; any other error → `failed`. The state is kept on the session even after its
terminal event was acknowledged and evicted from the log, so a later `cancel` of a finished
session reports it.

### Event sequence (polling example)

```text
 client                            SessionHost                          run task (dispatch_session)
   │ POST /v1/requests {id,params}   │                                          │
   │────────────────────────────────▶│ validate, count live, mint ses_/req_/tr_ │
   │                                 │ spawn launch(req, SessionSink, rx) ─────▶│ attach_input → `incoming`
   │◀──── 202 SessionReceipt ────────│                                          │
   │ POST …/input {send_seq:1,data}  │                                          │
   │────────────────────────────────▶│ check schema, sequencer, tx.send (≤5 s) ▶│ for message in incoming
   │◀──── SessionAck accepted_seq 1 ─│                                          │ emit {…}
   │                                 │◀──────── SessionSink.send (waits if 16 retained) ──│
   │ GET …/events?after_seq=0        │ ack(0), wait ≤ wait_ms for seq > 0       │
   │◀──── SessionBatch [seq 1 data] ─│                                          │
   │ POST …/finish_input             │ drop tx, closed = true ─────────────────▶│ loop ends
   │◀──── SessionAck input_closed ───│                                          │ return {…}
   │                                 │◀──────── finish(Ok(Completion)) ─────────│
   │ GET …/events?after_seq=1        │ ack(1) evicts seq 1                      │
   │◀── SessionBatch [seq 2 result], terminal:true                              │
   │        … retained 60 s, then removed on a later sweep …                    │
```

### Module map

```text
 src/domain/sessions.rs          SessionLimits, SessionOpenInput, SessionReceipt, SessionSendInput,
                                 SessionRef, SessionAck, SessionReadInput, SessionEvent, SessionBatch,
                                 CancelReceipt, InputSequencer, EventLog, terminal_state/envelope,
                                 MAX_WS_REFS = 8
 src/features/sessions/          open_session · send_input · finish_input · read_events · cancel_session
 src/infra/session_driver.rs     SessionHost (SessionDriver), Session, SessionSink, BufferBudget, sweeper,
                                 ACTION_DEADLINE = 5 s, CLEANUP_GRACE = 5 s, JOIN_MARGIN = 1 s
 src/orchestrator/setup_library.rs  session_host(): launch = Runtime::dispatch_session,
                                 mint = Runtime::new_request, validate = validate_params
 src/orchestrator/builtins.rs    rivet.sessions.{open,send,finish_input,read,cancel}, rivet.request
 src/orchestrator/setup_poll.rs  /v1/requests… routes → serve.project_polling
 src/orchestrator/setup_ws.rs    /v1/ws upgrade, per-ref pump + 16-frame lane (WsOutbox), close → cancel all refs
 src/orchestrator/setup_cli.rs   run_duplex (local --input-jsonl -)
 src/orchestrator/remote_cli.rs  feed_stdin_jsonl, INPUT_QUEUE = 16, remote duplex over WS
```

## Interfaces

### Use cases and port

| Use case | Input → output | Early checks in the use case |
|---|---|---|
| `sessions.open_session` | `SessionOpenInput {id, params, principal, connection_owned, deadline_ms?, trace?, restrict?}` → `SessionReceipt` (the surfaces fill `id`/`params` from the InputEnvelope's `operation`/`data`) | blank `id` → `validation.required`; non-object params → `validation.params` |
| `sessions.send_input` | `SessionSendInput {session_id, send_seq, data, principal}` → `SessionAck` | `send_seq` 0 → `conflict.input_sequence` |
| `sessions.finish_input` | `SessionRef` → `SessionAck {accepted_seq: null, input_closed: true}` | blank ID → `not_found.session` |
| `sessions.read_events` | `SessionReadInput {session_id, after_seq, max_events?, wait_ms?}` → `SessionBatch` | blank ID → `not_found.session`; clamp wait/max |
| `sessions.cancel_session` | `SessionRef` → `CancelReceipt {session_id, request_id, state}` | blank ID → `not_found.session` |

Port: `SessionDriver { open; send; finish_input; read; cancel; limits }`
(`src/features/sessions/ports.rs` re-exporting `src/domain/ports.rs`).

### Wire shapes

```text
 SessionReceipt  {session_id, request_id, trace_id, catalog_version, input_schema|null,
                  emits_schema|null, next_send_seq: 1, expires_at: RFC 3339, events_url?}
                  (0.2.0: delivered as the `data` of a ResponseEnvelope — status "accepted" on
                   POST /v1/requests and MCP streaming tools, status "ok" from rivet.sessions.open)
 SessionAck      {session_id, accepted_seq: int|null, input_closed: bool}          (bare body)
 SessionBatch    {session_id, events: [record…], last_seq, terminal: bool}          (bare body)
   record (0.2.0, stream records of API-2026-0006):
                 {request_id, trace_id, operation, type:"data", seq, data, error:null}
               | {request_id, trace_id, operation, type:"result", seq, status:"ok"|"error"|"cancelled",
                  data, error, effects, data_count}
   (0.1.0: {…, seq, type:"result", result, …} and a separate {…, type:"error", error} event)
 CancelReceipt   {session_id, request_id, state: "cancelled"|"succeeded"|"failed"}  (bare body)
 refusals        every error on a session route is an error ResponseEnvelope (operation = the
                 built-in, e.g. rivet.sessions.send), with the session's request/trace IDs when known
```

`expires_at` is `now + min(idle_ms, deadline_ms)`; with the defaults that is 30 s after
open (the request deadline is shorter than the 60 s idle lease). `deadline_ms` is the
caller's requested total deadline (default 30 000, clamped to 1 … 600 000).

### Surfaces

| Surface | Open | Send | Finish | Read | Cancel |
|---|---|---|---|---|---|
| Built-ins (`/v1/request`, MCP tools, CLI `request`, library) | `rivet.sessions.open {operation, data, deadline_ms?}` (deprecated `{id, params}`); `rivet.request {operation, data}` also returns a receipt when the target streams | `rivet.sessions.send {session_id, send_seq, data}` | `rivet.sessions.finish_input {session_id}` | `rivet.sessions.read {session_id, after_seq, max_events?, wait_ms?}` | `rivet.sessions.cancel {session_id}` |
| HTTP polling (`setup_poll.rs`) | `POST /v1/requests {operation, data, deadline_ms?, restrict?}` (+ `traceparent`) → 202 envelope `status: "accepted"` + `events_url` | `POST /v1/requests/{id}/input {send_seq, data}` | `POST /v1/requests/{id}/finish_input` | `GET /v1/requests/{id}/events?after_seq=N&wait_ms=M&max_events=K` | `POST /v1/requests/{id}/cancel` |
| WebSocket `/v1/ws`, subprotocol `rivet.v1` (`setup_ws.rs`) | `{type:"request", ref, operation, data, restrict?}` (no deadline field: 30 s; deprecated `id`/`params` accepted) | `{type:"input", ref, seq, data}` | `{type:"finish_input", ref}` | server pushes records with `ref` first: `type:"data"`… then one `type:"result"` per ref | `{type:"cancel", ref}`; socket close cancels all refs |
| CLI local | `rivet request ID --stream --input-jsonl -` — stdin JSONL lines are input, EOF finishes input, NDJSON records on stdout (no SessionHost; direct `dispatch_session`) | | | | Ctrl-C or a bad line → `Runtime::cancel` |
| CLI remote | same flags with `--endpoint URL` → one WebSocket ref (`src/infra/remote_client.rs`) | | | | Ctrl-C or a bad line → `cancel` frame |

Principal and operation checks: every surface authenticates the caller first
(SYS-2026-0004); `rivet.sessions.open`, polling open and each WS `request` frame run
`require_operation` for the target ID. Sessions are looked up only for their owning
principal, so foreign session IDs are `not_found.session`.

## Configuration

Session limits are compiled-in defaults (`SessionLimits::default()` in
`src/domain/sessions.rs`; a library host may pass its own with `RuntimeBuilder::session_limits`).
The only related policy.json key is the host-wide `limits.max_buffered_bytes`.

| Limit | Value | Effect | Error |
|---|---|---|---|
| `per_principal` | 8 | live (non-terminal) sessions per principal; WS refs excluded | `limit.sessions` (429 / 5, retryable) |
| WS refs per connection | 8 (`MAX_WS_REFS`) | in-flight refs on one socket | `limit.ws_refs` |
| `queue_frames` | 16 | input channel capacity **and** retained output events | producer waits (backpressure) |
| `queue_bytes` | 32 MiB | retained output bytes per session (an event always fits an empty log) | producer waits (backpressure) |
| `limits.max_buffered_bytes` (policy.json) | 256 MiB | bytes retained by **all** sessions of the host; each retained event holds a reservation until acknowledged | `limit.buffered_bytes` (429 / 5) ends the run |
| `retention_ms` | 60 000 | terminal session kept for reads | then `not_found.session` |
| `idle_ms` | 60 000 | no open/send/finish/read/cancel touch → cancelled by the background sweeper | terminal `cancelled.idle` |
| `wait_default_ms` / `wait_max_ms` | 1000 / 5000 | read long-poll wait | — |
| `max_events_default` / `max_events_cap` | 16 / 16 (min 1) | events per batch | — |
| Action deadline | 5 s (`ACTION_DEADLINE`) | a send waiting for queue space | `limit.input_queue` |
| Cleanup grace | 5 s (`CLEANUP_GRACE`) + 1 s (`JOIN_MARGIN`) | join of the cancelled run; abort only after it | — |
| Run deadline | 30 000 ms default; `deadline_ms` from polling / `rivet.sessions.open` / library, capped at 600 000 | every session run (WS refs and MCP streaming tools use the default) | terminal `timeout.request` |
| CLI stdin queue | 16 (`INPUT_QUEUE`) | lines read ahead of the run | backpressure on stdin |

## Runtime Behaviour

### Input sequencing (`InputSequencer`)

```text
 send_seq == last_seq && same sha256(data)   -> Duplicate: ack again, no second enqueue
 send_seq == last_seq && different payload   -> conflict.input_sequence
 input closed                                -> conflict.input_closed
 send_seq != last_seq + 1                    -> conflict.input_sequence ("expected send_seq N, got M")
 otherwise                                   -> enqueue (≤ 5 s for space) -> accept(seq, hash)
```

Before sequencing, `SessionHost::send` requires the operation to declare `receives`
(`validation.no_input`) and checks the item against the `receives` type
(`validation.input`, 422, `details {seq, path, expected, found}`). A send after the run ended is `conflict.session_terminal`. An
acknowledgement only proves local queue acceptance.

### Event log and cursor (`EventLog`)

```text
 seq:        1      2      3      4 (terminal)
           [data] [data] [data] [result]      ≤ 16 retained; producer blocks at 16
             ▲             ▲
          acked         delivered
 read(after_seq = A):
   A > delivered          -> conflict.cursor            (409)
   A < acked              -> stream.cursor_expired      (409)
   else: evict seq ≤ A, acked = A, wake producer,
         wait ≤ wait_ms for seq > A, return ≤ max_events, delivered = last returned seq
   terminal:true once the single terminal event is in the batch (or already past it)
```

Reads are serialized per session (`read_lock`); re-reading the same cursor replays retained
events, so clients deduplicate by `seq`. Nothing is appended after the terminal event.

### Cancel, idle and retention

```text
 cancel(session)
   ├─ touch; request_cancel: if no terminal event yet, record cancel-requested
   │   (from now on the session WILL end cancelled), close input, fire the run's token
   ├─ drop input sender
   ├─ join ≤ 5 s grace (+1 s margin): the run unwinds and closes its handles (SYS-2026-0002);
   │   abort only if it ignored the grace
   ├─ finish(Err cancelled.session)   (no-op if a terminal event already exists; a result that
   │   lands after the cancel was requested is replaced by cancelled, keeping its effects)
   └─ CancelReceipt.state = terminal_state   (already finished → succeeded | failed | cancelled)

 background sweeper (starts with the first session; interval min(idle, retention)/4 clamped
 to 20 ms … 1 s; stops when the host is dropped) — no session call needed:
   ├─ terminal and older than 60 s   -> removed (later calls: not_found.session)
   └─ live and untouched for 60 s    -> cancel-requested + token fired ─▶ terminal cancelled.idle

 shutdown (serve drain, Runtime::shutdown) -> every live session ─▶ cancelled.shutdown
```

A cancel that is requested before the run completes now always wins: the session ends
`cancelled` even if the operation's own `result` arrives while it unwinds. Cancelling a
session that had already recorded its terminal event changes nothing and reports that
state. Verified over polling on the 0.2.0-rc (`chat.echo` finished with no input):

```text
POST …/finish_input                     → {"session_id":"ses_03e90f330f","accepted_seq":null,"input_closed":true}
GET  …/events?after_seq=0&wait_ms=1000  → {"session_id":"ses_03e90f330f","events":[{…,"operation":"chat.echo","type":"result","seq":1,"status":"ok","data":{"count":0},"error":null,"effects":"none","data_count":0}],"last_seq":1,"terminal":true}
POST …/cancel                           → {"session_id":"ses_03e90f330f","request_id":"req_03e90f5fe7","state":"succeeded"}
```

### Verified: CLI duplex (local bundle)

Scratch bundle: `chat.echo` (`receives object {text}`, `emits object {text}`, echoes each input and
returns `{count}`), `chat.text` (`receives text`, `emits text`), `demo.add` and `demo.countdown` (emits 3, 2, 1,
returns `{done: true}`). `chat-input.jsonl` is `docs/demos/10-grpc/chat-input.jsonl`. Captured on the
0.2.0-rc; IDs vary per run.

```sh
$ rivet --file app.rivet request chat.echo --stream --input-jsonl - < chat-input.jsonl
{"request_id":"req_01d650f8f5","trace_id":"tr_01d650f8f5","operation":"chat.echo","type":"data","seq":1,"data":{"text":"hello"},"error":null}
{"request_id":"req_01d650f8f5","trace_id":"tr_01d650f8f5","operation":"chat.echo","type":"data","seq":2,"data":{"text":"goodbye"},"error":null}
{"request_id":"req_01d650f8f5","trace_id":"tr_01d650f8f5","operation":"chat.echo","type":"result","seq":3,"status":"ok","data":{"count":2},"error":null,"effects":"none","data_count":2}
exit=0
$ printf '{"text":"hi"}\n{"txt":1}\n' | rivet --file app.rivet request chat.echo --stream --input-jsonl -
{"request_id":"req_01d55a2275","trace_id":"tr_01d55a2275","operation":"chat.echo","type":"data","seq":1,"data":{"text":"hi"},"error":null}
{"request_id":"req_01d55a2275","trace_id":"tr_01d55a2275","operation":"chat.echo","type":"result","seq":2,"status":"error","data":null,"error":{"kind":"validation","code":"validation.input","message":"stdin line 2: input item at text must be text, got missing; the request was cancelled","retryable":false,"details":{"seq":2,"line":2}},"effects":"none","data_count":1}
exit=2
$ rivet --file app.rivet request chat.echo --input-jsonl - </dev/null
{"request_id":"","trace_id":"","operation":"chat.echo","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.usage","message":"--input-jsonl - needs --stream (output is NDJSON envelopes)","retryable":false},"effects":"none","data_count":0}
exit=2
$ rivet --file app.rivet request chat.echo
{"request_id":"req_01d4a89c3d","trace_id":"tr_01d4a89c3d","operation":"chat.echo","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"stream.input_required","message":"`chat.echo` receives live input; open a session or use --input-jsonl","retryable":false,"operation_id":"chat.echo"},"effects":"none","data_count":0}
exit=2
$ rivet --file app.rivet request demo.countdown --stream --input-jsonl - </dev/null
{"request_id":"","trace_id":"","operation":"demo.countdown","type":"result","seq":1,"status":"error","data":null,"error":{"kind":"validation","code":"validation.no_input","message":"`demo.countdown` does not declare `receives`; drop --input-jsonl","retryable":false},"effects":"none","data_count":0}
exit=2
$ rivet --file app.rivet request chat.echo --stream --input-jsonl chat-input.jsonl
{"request_id":"","trace_id":"","operation":"chat.echo","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.usage","message":"--input-jsonl chat-input.jsonl: only `-` (stdin) is supported","retryable":false},"effects":"none","data_count":0}
exit=2
```

Error records go to stderr. In 0.2.0 a bad stdin line ends the stream with a terminal `type: "result"` record
(`status: "error"`, `data_count` = items already emitted, `details {seq, line}`); the data records before it
are printed first.

### Verified: HTTP polling (`rivet serve --listen 127.0.0.1:18920`)

```sh
$ curl -s -X POST $B/v1/requests -H 'content-type: application/json' -d '{"operation":"chat.echo","data":{}}'   # [HTTP 202]
{"request_id":"req_0100cdd34d","trace_id":"tr_0100cdd34d","operation":"chat.echo","type":"result","status":"accepted","data":{"session_id":"ses_0100cdcb3d","request_id":"req_0100cdd34d","trace_id":"tr_0100cdd34d","catalog_version":"sha256:3bff5923f5d074ba1d150c9178a1c6ff00e972b27dc3243538f267c0d80a2e81","input_schema":{"type":"object","properties":{"text":{"type":"string","description":"Message text."}},"required":["text"],"additionalProperties":false},"emits_schema":{"type":"object","properties":{"text":{"type":"string","description":"Echoed text."}},"required":["text"],"additionalProperties":false},"next_send_seq":1,"expires_at":"2026-09-28T22:44:01Z","events_url":"/v1/requests/ses_0100cdcb3d/events"},"error":null,"effects":"none","data_count":0}
# input 1                              POST …/input {"send_seq":1,"data":{"text":"hello"}}
{"session_id":"ses_0100cdcb3d","accepted_seq":1,"input_closed":false}
# identical retry of 1
{"session_id":"ses_0100cdcb3d","accepted_seq":1,"input_closed":false}
# retry of 1 with a different payload                                           [HTTP 409]
{"request_id":"req_0100cdd34d","trace_id":"tr_0100cdd34d","operation":"rivet.sessions.send","type":"result","status":"error","data":null,"error":{"kind":"conflict","code":"conflict.input_sequence","message":"send_seq 1 was already accepted with a different payload","retryable":false},"effects":"none","data_count":0}
# send_seq 3 (gap)                                                              [HTTP 409]
{…,"operation":"rivet.sessions.send",…,"error":{"kind":"conflict","code":"conflict.input_sequence","message":"expected send_seq 2, got 3","retryable":false},…}
# send_seq 2 with {"txt":"x"}                                                   [HTTP 422]
{…,"operation":"rivet.sessions.send",…,"error":{"kind":"validation","code":"validation.input","message":"input item 2 at text must be text, got missing","retryable":false,"details":{"seq":2,"path":"text","expected":"text","found":"missing"}},…}
# input 2
{"session_id":"ses_0100cdcb3d","accepted_seq":2,"input_closed":false}
$ curl -s "$B/v1/requests/$SID/events?after_seq=0&wait_ms=500"
{"session_id":"ses_0100cdcb3d","events":[{"request_id":"req_0100cdd34d","trace_id":"tr_0100cdd34d","operation":"chat.echo","type":"data","seq":1,"data":{"text":"hello"},"error":null},{"request_id":"req_0100cdd34d","trace_id":"tr_0100cdd34d","operation":"chat.echo","type":"data","seq":2,"data":{"text":"goodbye"},"error":null}],"last_seq":2,"terminal":false}
$ curl -s -X POST $B/v1/requests/$SID/finish_input
{"session_id":"ses_0100cdcb3d","accepted_seq":null,"input_closed":true}
# send after finish                                                             [HTTP 409]
{…,"operation":"rivet.sessions.send",…,"error":{"kind":"conflict","code":"conflict.input_closed","message":"input is finished; no further sends are accepted","retryable":false},…}
$ curl -s "$B/v1/requests/$SID/events?after_seq=2&wait_ms=1000"
{"session_id":"ses_0100cdcb3d","events":[{"request_id":"req_0100cdd34d","trace_id":"tr_0100cdd34d","operation":"chat.echo","type":"result","seq":3,"status":"ok","data":{"count":2},"error":null,"effects":"none","data_count":2}],"last_seq":3,"terminal":true}
# after_seq=0 again                                                             [HTTP 409]
{"request_id":"","trace_id":"","operation":"rivet.sessions.read","type":"result","status":"error","data":null,"error":{"kind":"conflict","code":"stream.cursor_expired","message":"events after 0 were already acknowledged up to 2 and evicted","retryable":false},"effects":"none","data_count":0}
# after_seq=9                                                                   [HTTP 409]
{…,"operation":"rivet.sessions.read",…,"error":{"kind":"conflict","code":"conflict.cursor","message":"after_seq 9 is ahead of the last delivered event 3","retryable":false},…}
$ curl -s -X POST $B/v1/requests/$SID/cancel        # already terminal
{"session_id":"ses_0100cdcb3d","request_id":"req_0100cdd34d","state":"succeeded"}
```

Unary operations work the same way; their batch holds one terminal record:

```sh
$ curl -s -X POST $B/v1/requests -d '{"operation":"demo.add","data":{"a":2,"b":3}}' -H 'content-type: application/json'
{…,"operation":"demo.add","type":"result","status":"accepted","data":{"session_id":"ses_0271ef4b02",…,"input_schema":null,"emits_schema":null,"next_send_seq":1,…},…}
$ curl -s "$B/v1/requests/ses_0271ef4b02/events?after_seq=0"
{"session_id":"ses_0271ef4b02","events":[{"request_id":"req_0271ef5372","trace_id":"tr_0271ef5372","operation":"demo.add","type":"result","seq":1,"status":"ok","data":5,"error":null,"effects":"none","data_count":0}],"last_seq":1,"terminal":true}
# input to it                                                                   [HTTP 422]
{…,"operation":"rivet.sessions.send",…,"error":{"kind":"validation","code":"validation.no_input","message":"this operation does not declare `receives`","retryable":false},…}
```

Cancelling a live `chat.echo` session, an unknown session, and the per-principal cap:

```sh
$ curl -s -X POST $B/v1/requests/$SID/cancel
{"session_id":"ses_0466c8b494","request_id":"req_0466c8cca4","state":"cancelled"}
$ curl -s "$B/v1/requests/$SID/events?after_seq=0"
{"session_id":"ses_0466c8b494","events":[{"request_id":"req_0466c8cca4","trace_id":"tr_0466c8cca4","operation":"chat.echo","type":"result","seq":1,"status":"cancelled","data":null,"error":{"kind":"cancelled","code":"cancelled.session","message":"the session was cancelled","retryable":false},"effects":"none","data_count":0}],"last_seq":1,"terminal":true}
$ curl -s "$B/v1/requests/ses_nope/events"                                      # [HTTP 404]
{"request_id":"","trace_id":"","operation":"rivet.sessions.read","type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"not_found.session","message":"no session `ses_nope`","retryable":false},"effects":"none","data_count":0}
# nine opens of chat.echo in a row → 202 ×8, then [HTTP 429]:
{…,"operation":"chat.echo","type":"result","status":"error","data":null,"error":{"kind":"limit","code":"limit.sessions","message":"at most 8 live sessions per principal","retryable":true},"effects":"none","data_count":0}
```

The cap also applies to `rivet.sessions.open` on the same host (429 `limit.sessions`) until live sessions end,
are cancelled or idle out.

### Verified: `rivet.sessions.*` through `/v1/request` (a fresh server on `127.0.0.1:18921`)

```sh
$ curl -s -X POST $B/v1/request -d '{"operation":"rivet.sessions.open","data":{"operation":"demo.countdown"}}' -H 'content-type: application/json'
{"request_id":"req_0113229aed","trace_id":"tr_0113229aed","operation":"rivet.sessions.open","type":"result","status":"ok","data":{"session_id":"ses_0113393b2d","request_id":"req_0292e7bb7a","trace_id":"tr_0113229aed","catalog_version":"sha256:3bff5923…","input_schema":null,"emits_schema":{"type":"integer"},"next_send_seq":1,"expires_at":"2026-09-28T22:44:23Z"},"error":null,"effects":"none","data_count":0}
$ curl -s -X POST $B/v1/request -d '{"operation":"rivet.sessions.read","data":{"session_id":"ses_0113393b2d","after_seq":0,"max_events":2}}' -H 'content-type: application/json'
{"request_id":"req_030dbcec1f","trace_id":"tr_030dbcec1f","operation":"rivet.sessions.read","type":"result","status":"ok","data":{"session_id":"ses_0113393b2d","events":[{"request_id":"req_0292e7bb7a","trace_id":"tr_0113229aed","operation":"demo.countdown","type":"data","seq":1,"data":3,"error":null},{…,"type":"data","seq":2,"data":2,"error":null}],"last_seq":2,"terminal":false},"error":null,"effects":"none","data_count":0}
$ curl -s -X POST $B/v1/request -d '{"operation":"rivet.sessions.read","data":{"session_id":"ses_0113393b2d","after_seq":2}}' -H 'content-type: application/json'
{"request_id":"req_048c165bac",…,"operation":"rivet.sessions.read",…,"status":"ok","data":{"session_id":"ses_0113393b2d","events":[{…,"type":"data","seq":3,"data":1,"error":null},{"request_id":"req_0292e7bb7a","trace_id":"tr_0113229aed","operation":"demo.countdown","type":"result","seq":4,"status":"ok","data":{"done":true},"error":null,"effects":"none","data_count":3}],"last_seq":4,"terminal":true},…}
$ curl -s -i -X POST $B/v1/request -d '{"id":"rivet.sessions.open","params":{"id":"demo.countdown"}}' -H 'content-type: application/json'
HTTP/1.1 200 OK
deprecation: true                  (0.1.0 keys, still accepted through 0.2.x)
```

The built-in call has its own request ID; the session's run has another (and shares the trace ID). A streaming
operation called unary on `/v1/request` is refused with `stream.required` (422) and a hint naming SSE, polling,
WebSocket and `rivet.sessions.open`.

### Verified: WebSocket (`/v1/ws`, raw client)

```text
 101 Switching Protocols   Sec-WebSocket-Protocol: rivet.v1
 >> {"type":"request","ref":"c1","operation":"chat.echo","data":{}}
 >> {"type":"input","ref":"c1","seq":1,"data":{"text":"hello"}}
 >> {"type":"request","ref":"c1","operation":"demo.add","data":{"a":2,"b":3}}
 >> {"type":"request","ref":"a1","operation":"demo.add","data":{"a":2,"b":3}}
 >> {"type":"input","ref":"zz","seq":1,"data":{"text":"x"}}
 >> {"type":"finish_input","ref":"c1"}
 << {"ref":"c1","request_id":"","trace_id":"","operation":"demo.add","type":"result","status":"error","data":null,"error":{"kind":"conflict","code":"conflict.ref","message":"ref `c1` is already in flight","retryable":false},"effects":"none","data_count":0}
 << {"ref":"zz","request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"not_found.ref","message":"ref `zz` is not in flight","retryable":false},"effects":"none","data_count":0}
 << {"ref":"c1","request_id":"req_01e376d2c5","trace_id":"tr_01e376d2c5","operation":"chat.echo","type":"data","seq":1,"data":{"text":"hello"},"error":null}
 << {"ref":"c1","request_id":"req_01e376d2c5","trace_id":"tr_01e376d2c5","operation":"chat.echo","type":"result","seq":2,"status":"ok","data":{"count":1},"error":null,"effects":"none","data_count":1}
 << {"ref":"a1","request_id":"req_0262d45e5a","trace_id":"tr_0262d45e5a","operation":"demo.add","type":"result","seq":1,"status":"ok","data":5,"error":null,"effects":"none","data_count":0}
```

**Known issue (0.2.0-rc).** The `conflict.ref` refusal of a duplicate request frame is a `type: "result"`
record carrying the **in-flight** ref (`c1`), so the ref receives two terminal-looking records. It can be told
apart only by its empty `request_id` and its `operation` (`demo.add`, not `chat.echo`). In 0.1.0 it was a
distinct `type: "error"` frame. A client that ends a ref at its first `type: "result"`, such as
`docs/demos/01-catalog/fixtures/ws_client.py`, stops early. Reported to the plan owner.

```text
 WS ref lifecycle (setup_ws.rs)

 request frame ─▶ serve.parse_input ─▶ multiplex_ws: dup ref? conflict.ref │ 9th ref? limit.ws_refs │ require_operation
               ─▶ SessionDriver.open(connection_owned = true) ─▶ spawn pump(ref)
 pump: loop read(after, wait 5000) ─▶ data records … ─▶ one result record (any status) ─▶ ref freed
 input / finish_input refused ─▶ multiplex_ws sends that ref's terminal result record (status error) with the
                                SPECIFIC code first, then cancels the session (its own record is dropped)
 outbound: one 16-frame lane per ref (WsOutbox) merged into the socket writer
 cancel frame  ─▶ SessionDriver.cancel
 socket close  ─▶ cancel + join every in-flight ref (connection-owned), abort pumps
```

A refused input frame ends its ref with the **specific** error (`chat.text` receives and emits `text`):

```text
 >> {"type":"request","ref":"c1","operation":"chat.text","data":{}}
 >> {"type":"input","ref":"c1","seq":1,"data":"hi"}
 >> {"type":"input","ref":"c1","seq":3,"data":"skip"}
 << {"ref":"c1","request_id":"req_090151367d","trace_id":"tr_090151367d","operation":"chat.text","type":"result","status":"error","data":null,"error":{"kind":"conflict","code":"conflict.input_sequence","message":"expected send_seq 2, got 3","retryable":false},"effects":"none","data_count":0}
 >> {"type":"request","ref":"c2","operation":"chat.text","data":{}}
 >> {"type":"input","ref":"c2","seq":1,"data":5}
 << {"ref":"c2","request_id":"req_1080325e52","trace_id":"tr_1080325e52","operation":"chat.text","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.input","message":"input item 1 at $ must be text, got integer","retryable":false,"details":{"seq":1,"path":"$","expected":"text","found":"integer"}},"effects":"none","data_count":0}
```

In this run the `hi` echo of `c1` was not delivered before the refusal ended the ref (the timing varies; a data
record can precede the terminal record).

The CLI's remote duplex uses the same WebSocket path:

```sh
$ rivet --endpoint http://127.0.0.1:18921 request chat.echo --stream --input-jsonl - < chat-input.jsonl
{"request_id":"req_128ea6b094","trace_id":"tr_128ea6b094","operation":"chat.echo","type":"data","seq":1,"data":{"text":"hello"},"error":null}
{"request_id":"req_128ea6b094","trace_id":"tr_128ea6b094","operation":"chat.echo","type":"data","seq":2,"data":{"text":"goodbye"},"error":null}
{"request_id":"req_128ea6b094","trace_id":"tr_128ea6b094","operation":"chat.echo","type":"result","seq":3,"status":"ok","data":{"count":2},"error":null,"effects":"none","data_count":2}
exit=0
```

### Not verified

`docs/demos/10-grpc` (`chat.exchange` with `chat-input.jsonl`) needs a gRPC fixture server
that was not running; the gRPC adapter side of a duplex session was not exercised here (see
SYS-2026-0005). Idle expiry (60 s) and retention expiry (60 s) were not waited out; they are
described from the background sweeper in `src/infra/session_driver.rs` and its unit tests
(`cancel_wins_the_race_with_completion` and the sweeper tests in `tests/conformance_sessions.rs`). MCP `tools/call`
of `rivet.sessions.*` was not run for this document; SYS-2026-0004 shows the `accepted` envelope an MCP
streaming tool returns.

### Other session owners (0.2.0)

```text
 SessionHost (one per Runtime, shared registry = the CURRENT catalog snapshot)
   ├─ principal-owned  : polling, rivet.sessions.*, MCP streaming tools      (count toward 8 per principal)
   ├─ connection-owned : WebSocket refs, C ABI call handles (rivet_call_start) (not counted; die with the owner)
   └─ every run keeps the catalog snapshot it opened on: a module loaded later (Runtime::load, rivet_load)
      is not visible to an already-open session; receipts report that snapshot's catalog_version
```

A C ABI call handle is a connection-owned session: `rivet_call_next` reads it in ≤ 5 s slices, the 60 s idle
lease still applies, and each record is returned as its own envelope string
([SYS-2026-0010](../components/sys-2026-0010-ffi-surface-and-packaging.md#call-handle)). A module load swaps
the runtime's catalog snapshot; a session's run and its nested calls stay on the snapshot they started with
([SYS-2026-0001](../components/sys-2026-0001-compiler-and-catalog.md#run-time-module-loads-and-catalog-snapshots)).

## Data and Storage

All session state is in memory inside `SessionHost.sessions` (`HashMap<String,
Arc<Session>>`):

```text
 Session
 ├─ id "ses_NN…" · request_id · trace_id · owner (principal name) · connection_owned
 ├─ receives: Option<ValueSpec>          (input validation; None → input closed at open)
 ├─ State { InputSequencer{last_seq,last_hash,closed}, EventLog{events≤16,last_seq,acked,delivered,terminal},
 │          held (seq, Reservation)[], held_bytes, cancel_requested?, terminal_state?, last_touch, terminal_at }
 ├─ token: CancelToken                   (fired by cancel / idle / shutdown)
 ├─ input: Option<mpsc::Sender<Value>>   (capacity 16; dropped by finish_input / cancel)
 ├─ task: JoinHandle (the run)           · read_lock (one reader at a time)
 └─ changed: Notify                      (wakes readers and blocked producers)
```

Payload hashes are SHA-256 of the JSON encoding of `data`. Nothing is written to disk; a
process restart ends every session.

## Dependencies

| Dependency | Use |
|---|---|
| `tokio` (`mpsc`, `Notify`, `Mutex`, `spawn`, `time::timeout`) | input queue, wake-ups, run task, deadlines |
| `sha2` | payload hash for duplicate detection |
| `axum` (serve) | polling routes and the `/v1/ws` upgrade |
| `tokio-tungstenite` | CLI remote duplex WebSocket client |
| Shared dispatcher and interpreter (SYS-2026-0002) | the run, `incoming`, `emit` |
| `serve.authorize_operation` (SYS-2026-0004) | per-principal operation check |

## Deployment

Sessions exist wherever a `Runtime` exists: inside `rivet serve` (shared by the polling,
WebSocket, `/v1/request` and MCP surfaces) and inside library hosts. `serve --stdio` exposes
them only as MCP tools. A CLI process that is not serving only uses the local duplex path.

## Security Boundaries

```text
 caller ─▶ surface auth (none/bearer; SYS-2026-0004) ─▶ principal
 open   ─▶ require_operation(principal, id) ─▶ public catalog only (private IDs → not_found)
 every send/finish/read/cancel ─▶ SessionHost.get(id, principal): owner must match,
                                  else not_found.session (existence not disclosed)
 input  ─▶ validated against `receives` before it reaches `incoming`
 effects inside the run ─▶ same policy broker as any request (SYS-2026-0003)
```

Session IDs are routing handles, not credentials: knowing one grants nothing without the
owning principal. The CLI stdin feeder never echoes line content in errors.

## Observability

- Every record carries the run's `request_id`, `trace_id` and `operation`, and `seq` (0.2.0 envelope records).
- Effect decisions of the run are in the trace store under the run's `request_id`
  (`rivet trace show`, SYS-2026-0002).
- Terminal errors distinguish cause by code: `cancelled.session` (explicit cancel or WS
  close), `cancelled.idle` (idle lease), `cancelled.shutdown` (serve drain),
  `timeout.request` (run deadline), `limit.buffered_bytes` (host byte budget), or the
  operation's own error.
- Session requests carry the caller's W3C trace (`traceparent` on polling open and the WS
  upgrade; `rivet.sessions.open` inherits the calling request's trace).

## Known Limitations

From the [manual's Known Limitations](../../manuals/man-2026-0001-rivet-manual.md#known-limitations):

- `--timeout` is not forwarded over the WebSocket duplex path; WS refs (and MCP streaming
  tools) run with the default 30 s deadline.
- No persistence, resume or reconnection to a session after a process restart (no
  persistent store of any kind).

## Last Verified Version

`0.2.0-rc (main at 8031baa)`, 2026-09-29, macOS, `target/release/rivet` built with
`cargo build --release --features cli`. Every capture was re-run with a scratch bundle (`chat.echo`, `chat.text`,
`demo.add`, `demo.countdown`) locally and on `rivet serve --listen 127.0.0.1:18920`, `18921` and `18922`; all
servers were stopped afterwards (SIGTERM, exit 0). Session, request and trace IDs, `catalog_version` and
`expires_at` differ per run.

History: `0.1.0-dev (commit 829ca43)`, 2026-09-28, macOS, `target/debug/rivet`. First verified at
`f40d4aa` with a scratch bundle (`chat.echo`, `demo.countdown`, `demo.add`) and
`rivet serve --listen 127.0.0.1:18420`; the cancel-after-finish, WebSocket refusal frames and
SIGTERM drain were re-verified at `829ca43` on `127.0.0.1:18901`–`18904`; all servers were
stopped afterwards. Session, request and trace IDs, `catalog_version` and `expires_at`
differ per run.

## Related Documents

- [PROP-2026-0001 Rivet runtime proposal](../../proposals/implemented/prop-2026-0001-rivet-runtime.md)
- [PLAN-2026-0001 v0.1.0 implementation and release](../../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md)
- [REF-2026-0002 Language and usage](../../references/ref-2026-0002-language-and-usage.md)
- [SYS-2026-0002 Execution, scopes and DAG](sys-2026-0002-execution-scopes-and-dag.md)
- [SYS-2026-0004 Surfaces and serve](../components/sys-2026-0004-surfaces-and-serve.md)
- [SYS-2026-0005 Protocol adapters](../integrations/sys-2026-0005-protocol-adapters.md)
- [SYS-2026-0008 policy.json reference](../configuration/sys-2026-0008-policy-json-reference.md)
- [API-2026-0006 Envelopes](../../api/api-2026-0006-envelopes.md)
- [SYS-2026-0010 FFI surface and packaging](../components/sys-2026-0010-ffi-surface-and-packaging.md)
- [Demo folders](../../demos/README.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial current-state document (PLAN-2026-0001 D-21). |
| 2 | 2026-09-28 | Claude | TASK-092 drift fix for the fix batch (829ca43): configurable `deadline_ms` (cap 600 000), background sweeper, cancel wins the race, terminal state reported by cancel, `queue_bytes` and host `max_buffered_bytes` enforced, structured cancel with grace, `cancelled.shutdown`, WS specific refusal frames and per-ref lanes, `restrict`/`trace` on open; limitations reduced to the current ones. |
| 3 | 2026-09-29 | Claude | PLAN-2026-0002 D-36/D-47 (TASK-073, TASK-070): receipts in `accepted`/`ok` envelopes, events as 0.2.0 stream records (`status` on the terminal record; no `error` event), session-route refusals as envelopes, `{operation, data}` inputs on every surface; every capture re-run on the 0.2.0-rc; connection-owned C ABI call handles and catalog snapshots; WS duplicate-ref known issue. |
