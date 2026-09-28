---
document_id: DEMO-2026-0004
title: "Streaming data and contextual cleanup"
document_type: demo
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 5
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, execution, files, transports, policy, cli, serve, http]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development]
audience: [developers, reviewers]
scope: Runnable streaming demo — a pure emitted sequence on the CLI and over SSE, a scoped `with file open … mode read` chunk stream, and a scoped WebSocket ping with timeout and Ctrl-C cancellation against a shipped local WebSocket fixture.
reason: User requested sample files in folders with READMEs showing usage; UQ-17 adds declared outputs and policy.json-only policy; UQ-18 adds the generated I/O manifest; TASK-067 executed every step against the 0.1.0 release candidate.
dependencies: [PROP-2026-0001, REF-2026-0002]
related_documents: [PLAN-2026-0001, DEMO-2026-0015, DEMO-2026-0013, MAN-2026-0003, MAN-2026-0006, TEST-2026-0003, TEST-2026-0005, TEST-2026-0020, TEST-2026-0025]
supersedes: null
superseded_by: null
tags: [rivet, demo, streaming, sse, files, websocket, cancellation]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-28
verified_against: "0.1.0"
---

# Streaming data and contextual cleanup

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** language, execution, files, transports, policy, cli, serve, http

## Purpose

Streaming data and contextual cleanup. Delivery stage: **A; WebSocket in B**. Read [app.rivet](app.rivet) alongside this walkthrough.

| Operation ID | Output | Emits | Behavior |
|---|---|---|---|
| `events.count` | `object {count}` | `integer` | Pure finite data stream 1, 2, 3. |
| `files.chunks` | `json` (always null) | `bytes` | `with file open "./data/lines.txt" mode read as reader`, `chunk_size 65536`, `for chunk in reader`. |
| `socket.ping` | `object {type}`, `open true` | — | Scoped WebSocket: send a ping, return the first reply, dispose the socket. |

```text
  emit 1 ─┐
  emit 2 ─┼──> data items (seq 1..n) ──> one terminal result (validated against `output`)
  emit 3 ─┘         |                            |
               CLI: NDJSON (--stream)     SSE: event: data / event: result

  with file open … as reader ──> for chunk in reader ──> emit chunk ──> scope exit closes the handle
  with websocket … as socket ──> send / receive     ──> return       ──> scope exit closes the socket
                                        │ timeout / Ctrl-C
                                        └──> cancelled or timed out; cleanup still runs
```

## Verified Against Version

0.1.0. Verified on 0.1.0-dev at commit `829ca43`, the release candidate (the version bump to 0.1.0 happens at release, P5), with `target/release/rivet` on macOS 26.4.1 (Darwin 25.4.0, arm64), 2026-09-28. Every output block below was pasted from that run. Request and trace IDs and ports vary.

## Prerequisites

```sh
cargo build --release                       # from the repository root
export PATH="$PWD/target/release:$PATH"     # `rivet --version` prints rivet 0.1.0-dev until the P5 bump
python3 -m venv "$TMPDIR/rivet-demo-venv" && "$TMPDIR/rivet-demo-venv/bin/pip" install websockets
```

The WebSocket fixture [fixtures/ws_fixture.py](fixtures/ws_fixture.py) needs the `websockets` package. `curl` is used for SSE. Loopback ports 18840 (fixture) and 18841 (serve) must be free.

## Setup

```sh
cd docs/demos/04-streaming
```

The file input [data/lines.txt](data/lines.txt) (`alpha`, `beta`, `gamma`, 17 bytes) is included. This folder has two policy files, one per stage:

```text
  policy.json               (auto-discovered)   allow_read    ./data/**                  -> files.chunks
  policies/websocket.json   (--policy)          allow_network wss://api.example.com:443  -> socket.ping
  events.count is pure and runs under either file (or under none).
```

`wss://api.example.com/realtime` is a placeholder; step 6 rewrites it to the local fixture in a scratch copy.

## Steps

### 1. Check the bundle and view outputs

#### Command / Request

```sh
rivet --file app.rivet check --strict-docs
rivet --file app.rivet outputs events.count
rivet --file app.rivet outputs --all --json
```

#### Expected Output / Response

```text
ok: 3 operations, 0 connectors, 0 auth profiles
```

```text
events.count — Emit a finite sequence
output  object   Summary returned after the last item.
  count   integer  required  Number of items emitted.
emits   integer  One of 1, 2, 3, in order.
receives —
errors   —
```

`outputs --all --json` prints one line; reformatted, the item schemas are:

```json
[
  {"id":"events.count", "output":{"type":"object","properties":{"count":{"type":"integer","description":"Number of items emitted."}},"required":["count"],"additionalProperties":false,"description":"Summary returned after the last item."},
   "emits":{"type":"integer","description":"One of 1, 2, 3, in order."}, "receives":null, "errors":[]},
  {"id":"files.chunks", "output":{"description":"Always null; the file content is delivered as emitted chunks."},
   "emits":{"type":"object","properties":{"$type":{"const":"bytes"},"base64":{"type":"string"}},"required":["$type","base64"],"description":"Up to 65536 bytes of data/lines.txt per item, in file order."}, "receives":null, "errors":[]},
  {"id":"socket.ping", "output":{"type":"object","properties":{"type":{"type":"string","description":"Message type; the fixture answers \"pong\"."}},"required":["type"],"additionalProperties":true,"description":"First message received after the ping."},
   "emits":null, "receives":null, "errors":[]}
]
```

All exit 0. `socket.ping`'s `open true` shows as `"additionalProperties":true`. The `emits … description` text is shown on the `emits` line and as the item schema's `description`.

### 2. I/O manifest under both policy files

#### Command / Request

```sh
rivet --file app.rivet io --by target
rivet --file app.rivet io --check-policy
rivet --file app.rivet --policy ./policies/websocket.json io socket.ping --check-policy
rivet --file app.rivet io --check-files
```

#### Expected Output / Response

`events.count` is pure and contributes no rows:

```text
TARGET                     ACCESS            CAPABILITY     ORIGIN          PHASE    NEEDS FILE  USED BY
./data/lines.txt           read              allow_read     with file open  body     yes         files.chunks
wss://api.example.com:443  connect GET (ws)  allow_network  with websocket  connect  —           socket.ping
```

Under the default policy.json (exit **3**, `socket.ping` denied):

```text
OPERATION     KIND     ACCESS       TARGET                          KNOWLEDGE  SOURCE        DECISION
files.chunks  file     read         ./data/lines.txt                exact      app.rivet:19  allowed
socket.ping   network  connect GET  wss://api.example.com/realtime  exact      app.rivet:35  denied
1 allowed · 1 denied
```

Under policies/websocket.json for `socket.ping` only (exit 0):

```text
OPERATION    KIND     ACCESS       TARGET                          KNOWLEDGE  SOURCE        DECISION
socket.ping  network  connect GET  wss://api.example.com/realtime  exact      app.rivet:35  allowed
1 allowed
```

Neither file covers both operations on purpose, so the whole-bundle check exits 3 under either one (under websocket.json, `files.chunks` is the denied row). `io --check-files` (exit 0):

```text
events.count needs no existing files.
files.chunks needs, before it can run:
  ./data/lines.txt  (with file open)   present
socket.ping needs no existing files.
1 file · 1 present
```

### 3. Pure stream on the CLI

#### Command / Request

```sh
rivet --file app.rivet request events.count --params '{}' --stream
rivet --file app.rivet request events.count --params '{}'
```

#### Expected Output / Response

With `--stream`, NDJSON data lines then the terminal line (exit 0):

```json
{"request_id":"req_018414bf3d","trace_id":"tr_018414bf3d","seq":1,"type":"data","data":1}
{"request_id":"req_018414bf3d","trace_id":"tr_018414bf3d","seq":2,"type":"data","data":2}
{"request_id":"req_018414bf3d","trace_id":"tr_018414bf3d","seq":3,"type":"data","data":3}
{"request_id":"req_018414bf3d","trace_id":"tr_018414bf3d","result":{"count":3},"data_count":3,"effects":"none","type":"result"}
```

Without `--stream` the CLI prints only the Completion; `data_count` still reports the three items (exit 0):

```json
{"request_id":"req_01836d6445","trace_id":"tr_01836d6445","result":{"count":3},"data_count":3,"effects":"none"}
```

### 4. Scoped file chunks

#### Command / Request

```sh
rivet --file app.rivet request files.chunks --params '{}' --stream
```

#### Expected Output / Response

The 17-byte file fits one 65536-byte chunk. Bytes are tagged base64 (`YWxwaGEKYmV0YQpnYW1tYQo=` is `alpha\nbeta\ngamma\n`); the handle closes at scope exit, then the result is `null` (exit 0):

```json
{"request_id":"req_01838ecc05","trace_id":"tr_01838ecc05","seq":1,"type":"data","data":{"$type":"bytes","base64":"YWxwaGEKYmV0YQpnYW1tYQo="}}
{"request_id":"req_01838ecc05","trace_id":"tr_01838ecc05","result":null,"data_count":1,"effects":"none","type":"result"}
```

### 5. The same streams over SSE

#### Command / Request

```sh
rivet --file app.rivet serve --listen 127.0.0.1:18841 2>serve.err & SV=$!
curl -sSN http://127.0.0.1:18841/v1/request -H 'Content-Type: application/json' \
  -H 'Accept: text/event-stream' -d '{"id":"events.count","params":{}}'
curl -sS -w ' %{http_code}\n' http://127.0.0.1:18841/v1/request -H 'Content-Type: application/json' \
  -d '{"id":"events.count","params":{}}'
kill $SV; rm -f serve.err
```

#### Expected Output / Response

```text
id: 1
event: data
data: {"request_id":"req_0143822655","trace_id":"tr_0143822655","seq":1,"type":"data","data":1}

id: 2
event: data
data: {"request_id":"req_0143822655","trace_id":"tr_0143822655","seq":2,"type":"data","data":2}

id: 3
event: data
data: {"request_id":"req_0143822655","trace_id":"tr_0143822655","seq":3,"type":"data","data":3}

id: 4
event: result
data: {"request_id":"req_0143822655","trace_id":"tr_0143822655","result":{"count":3},"data_count":3,"effects":"none","type":"result","seq":4}
```

A plain JSON POST of a streaming operation is refused with HTTP 422:

```text
{"request_id":"","trace_id":"","error":{"kind":"validation","code":"stream.required","message":"`events.count` streams; use Accept: text/event-stream, POST /v1/requests, /v1/ws or rivet.sessions.open","retryable":false,"effects":"none"}} 422
```

`files.chunks` streams the same way (`event: data` with the base64 chunk, then `event: result` with `null`). Polling and WebSocket delivery of the same stream are shown in [01-catalog](../01-catalog/README.md) steps 6–7.

### 6. Scoped WebSocket ping against the local fixture

#### Command / Request

```sh
rivet --file app.rivet request socket.ping --params '{}'          # in this folder: no network grant
WORK="$(mktemp -d)"; cp -R app.rivet policy.json policies data fixtures "$WORK/"; cd "$WORK"
sed -i.bak -e 's#wss://api\.example\.com/realtime#ws://127.0.0.1:18840/realtime#' app.rivet
sed -i.bak -e 's#wss://api\.example\.com:443#ws://127.0.0.1:18840#' policies/websocket.json
"$TMPDIR/rivet-demo-venv/bin/python" fixtures/ws_fixture.py 18840 > fixture.log 2>&1 & FX=$!
rivet --file app.rivet --policy ./policies/websocket.json request socket.ping --params '{}'
rivet --file app.rivet request socket.ping --params '{}'
```

#### Expected Output / Response

In this folder, the auto-discovered policy.json grants no network (exit 3):

```json
{"request_id":"req_0181093575","trace_id":"tr_0181093575","error":{"kind":"permission","code":"permission.denied","message":"allow_network connect wss://api.example.com:443/realtime denied: no grant for allow_network wss://api.example.com:443/realtime","retryable":false,"effects":"none","source":{"file":"app.rivet","line":35,"column":5,"end_line":40,"end_column":8},"operation_id":"socket.ping","details":{"capability":"allow_network","access":"connect","target":"wss://api.example.com:443/realtime"}}}
```

In the scratch copy under websocket.json, the fixture's extra field is accepted by `open true` (exit 0):

```json
{"request_id":"req_0100ba82bd","trace_id":"tr_0100ba82bd","result":{"type":"pong","fixture":"04-streaming"},"data_count":0,"effects":"committed"}
```

Without `--policy` the same call is denied for `ws://127.0.0.1:18840/realtime` (exit 3). `fixture.log` shows the socket closed when the scope ended:

```text
ws fixture on ws://127.0.0.1:18840/realtime
open  ('127.0.0.1', 60446) /realtime
recv  {'type': 'ping'}
close ('127.0.0.1', 60446) code=1005
```

### 7. Timeout and Ctrl-C cleanup

#### Command / Request

```sh
kill $FX
"$TMPDIR/rivet-demo-venv/bin/python" fixtures/ws_fixture.py 18840 --silent >> fixture.log 2>&1 & FX=$!
rivet --file app.rivet --policy ./policies/websocket.json request socket.ping --params '{}' --timeout 2s
rivet --file app.rivet --policy ./policies/websocket.json request socket.ping --params '{}'   # press Ctrl-C
```

#### Expected Output / Response

The silent fixture never answers. `--timeout 2s` stops the receive (exit 6):

```json
{"request_id":"req_01af20fee5","trace_id":"tr_01af20fee5","error":{"kind":"timeout","code":"timeout.receive","message":"receive did not complete before its deadline","retryable":false,"effects":"committed","source":{"file":"app.rivet","line":38,"column":9,"end_line":38,"end_column":36},"operation_id":"socket.ping"}}
```

Ctrl-C cancels (exit 130):

```json
{"request_id":"req_01378b3e5d","trace_id":"tr_01378b3e5d","error":{"kind":"cancelled","code":"cancelled.request","message":"`socket.ping` was cancelled","retryable":false,"effects":"committed","source":{"file":"app.rivet","line":38,"column":9,"end_line":38,"end_column":36},"operation_id":"socket.ping"}}
```

In both cases `fixture.log` gains an `open … recv … close` triple: the socket is closed by scope cleanup, not left open. `effects` is `committed` because the ping was already sent.

## Effects and policy

```text
  outcome table (verified in steps 3–7)
  ┌───────────────────────────────────────────┬───────────────────┬──────┬───────────┐
  │ situation                                 │ code              │ exit │ effects   │
  ├───────────────────────────────────────────┼───────────────────┼──────┼───────────┤
  │ events.count (CLI, --stream or not; SSE)  │ —                 │  0   │ none      │
  │ events.count as plain JSON over HTTP      │ stream.required   │ 422  │ none      │
  │ files.chunks                              │ —                 │  0   │ none      │
  │ socket.ping with the grant                │ —                 │  0   │ committed │
  │ socket.ping without the grant             │ permission.denied │  3   │ none      │
  │ no reply within --timeout                 │ timeout.receive   │  6   │ committed │
  │ Ctrl-C while waiting                      │ cancelled.request │ 130  │ committed │
  └───────────────────────────────────────────┴───────────────────┴──────┴───────────┘
```

The pure stream needs no grants, the file stream needs `allow_read`, and the WebSocket needs an exact origin grant. `timeout "20s"` in the `with websocket` block is a leading option. There is no explicit close method; scope exit closes handles in reverse order.

## Release Updates

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-03 | UQ-02 / R3 | `with` scopes close file and socket handles on success, timeout and cancel | Steps 4, 6, 7 | `close` logged each time; exits 0 / 6 / 130 | This README steps 4, 6, 7 (2026-09-28, 829ca43); TEST-2026-0003, TEST-2026-0005 |
| U-09 | UQ-08 / R9 | Emitted items then one terminal result | Steps 3, 5 | Items 1, 2, 3 then `{count:3}` | This README steps 3, 5; TEST-2026-0003 |
| U-22 | UQ-15/08/17 / R22 | Streams over SSE; `stream.required` for plain JSON | Step 5 | SSE frames; 422 | This README step 5; TEST-2026-0019 |
| U-23 | UQ-17 / R23 | Declared outputs and emits schemas | Step 1 | Three entries | This README step 1; TEST-2026-0020 |
| U-24 | UQ-17 / R24 | Per-stage policy files | Steps 2, 6 | Denied without, allowed with websocket.json | This README steps 2, 6; TEST-2026-0021 |

## Cleanup

```sh
kill $FX 2>/dev/null
cd - && rm -rf "$WORK"
rm -rf "$TMPDIR/rivet-demo-venv"   # optional
```

Nothing is written to this folder (step 5 removes its `serve.err`).

## Verification Record

| Step | Verified By | Verified At | Result |
|---|---|---|---|
| 1. `check --strict-docs`, `outputs` | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 2. `io --by target`; `io --check-policy` exit 3 / 0; `io --check-files` exit 0 | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 3. `events.count` with and without `--stream` | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 4. `files.chunks` scoped file handle | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 5. SSE for both streams; `stream.required` 422 | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 6. `socket.ping` denied, then pong from the fixture | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 7. `--timeout 2s` exit 6; Ctrl-C exit 130; socket closed | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |

Build: `cargo build` and `cargo build --release` at 829ca43; every command above was executed from this folder or the scratch copy and the output pasted from that run.

## Known Caveats

- The real `wss://api.example.com` is not contacted; the fixture is plain `ws://` on loopback.
- The fixture logs close code 1005 (no status code in the close frame) when Rivet disposes the socket.
- Request and trace IDs and client ports vary between runs.

## Related Documents

- [All sample folders](../README.md) · [demos index](../index.md) · [release verification guide](../demo-2026-0015-v0-1-0-release-verification.md)
- [Language guide](../../manuals/man-2026-0003-language-guide.md) · [Serving and surfaces](../../manuals/man-2026-0006-serving-and-surfaces.md)
- [Stream tests TEST-2026-0003](../../testing/test-2026-0003-streams.md) · [Resource tests TEST-2026-0005](../../testing/test-2026-0005-resources.md)
- [Usage reference](../../references/ref-2026-0002-language-and-usage.md) · [Proposal](../../proposals/approved/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 5 | 2026-09-28 | Claude | TASK-067: `files.chunks` uses `with file open … mode read` (G35); added fixtures/ws_fixture.py and a scratch-copy WebSocket run; executed every step against 0.1.0-dev (829ca43) and pasted real output. Fixes: `outputs` shows `emits integer` without the description and bytes items as a tagged base64 object; `--check-policy` summary lines; the CLI without `--stream` returns the Completion (only HTTP refuses with `stream.required`); added timeout (exit 6) and Ctrl-C (exit 130) cleanup; removed draft disclaimers; status active; verified_against 0.1.0. |
| 4 | 2026-09-28 | Claude | TASK-005/R26 (ADR-0001, proposal revision 8): `io --by target` gains ORIGIN, PHASE, NEEDS FILE (`with file open` read needs its file). |
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest: `io --by target` and `io --check-policy` (socket.ping denied under policy.json, allowed under policies/websocket.json). |
| 2 | 2026-09-28 | Claude | UQ-17: declared outputs/emits with descriptions (`open true` on socket.ping); quoted `timeout "20s"`; `--sandbox` variants replaced by policy.json and policies/websocket.json; serve without `--transport`; `stream.required` code; View outputs, strict-docs, bootstrap and exit codes. |
| 1 | 2026-09-28 | Codex | Added draft source files, prerequisites, invocation examples and expected behavior. |
