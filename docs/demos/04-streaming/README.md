---
document_id: DEMO-2026-0004
title: "Streaming data and contextual cleanup"
document_type: demo
status: draft
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 3
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [examples]
affected_versions:
  from: not-applicable
  to: proposed-v0.1
applicable_environments: [development]
audience: [developers, reviewers]
scope: Illustrate the existing proposed interface with sample files; no runtime implementation or release claim.
reason: User requested sample files in folders with READMEs showing usage; UQ-17 (2026-09-28) adds declared outputs, policy.json-only policy and one serve for every surface; UQ-18 (2026-09-28) adds the generated I/O manifest and policy generate.
dependencies: [PROP-2026-0001, REF-2026-0002]
related_documents: [PROP-2026-0001, REF-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, examples, design]
confidentiality: internal
review_cycle: on-design-change
next_review_date: 2026-10-27
verified_against: not-implemented
---

# Streaming data and contextual cleanup

**Draft usage sample.** Rivet has no runtime or CLI implementation yet. These files make the proposed interface concrete; commands below are intended usage, not executed demos.

## Purpose

Streaming data and contextual cleanup. Delivery stage: **A; WebSocket in B**. Read [app.rivet](app.rivet) alongside this walkthrough.

| Operation ID | Output | Emits | Behavior |
|---|---|---|---|
| `events.count` | `object {count}` | `integer` | Pure finite data stream. |
| `files.chunks` | `json` (always null) | `bytes` | Read bounded binary chunks. |
| `socket.ping` | `object {type}`, open | — | Scoped WebSocket exchange. |

```text
  emit 1 ─┐
  emit 2 ─┼──> data items (seq 1..n) ──> one terminal result (validated against `output`)
  emit 3 ─┘         |                            |
               CLI: NDJSON             SSE: event: data / event: result
```

## Verified Against Version

None. Based on proposal revision 5 semantics (UQ-17) and S19, S41, S70–S73. No parser, runtime or network fixture execution is claimed.

## Prerequisites

A future Rivet build implementing this stage. External services below are controlled fixtures, not public services to contact. Commands assume the repository root initially, then the setup directory. Each folder is an independent bundle; do not concatenate folders with duplicate IDs.

## Setup

```sh
cd docs/demos/04-streaming
```

The file input [data/lines.txt](data/lines.txt) is included. `events.count` needs no fixture. `socket.ping` needs the controlled WSS `/realtime` fixture: ping → `{"type":"pong"}`.

This folder has two policy files, one per stage:

```text
  policy.json               (auto-discovered)   allow_read  ./data/**                  -> files.chunks
  policies/websocket.json   (--policy)          allow_network wss://api.example.com:443 -> socket.ping
  events.count is pure and runs under either file (or under none).
```

## Steps

### Command / Request

```sh
rivet --file app.rivet request events.count --params '{}' --stream
rivet --file app.rivet request files.chunks --params '{}' --stream
rivet --file app.rivet --policy ./policies/websocket.json request socket.ping --params '{}'
```

To consume `events.count` over SSE, start one server (it uses the auto-discovered policy.json):

```sh
rivet --file app.rivet serve --listen 127.0.0.1:8080
```

Then, from another terminal:

```sh
curl -N http://127.0.0.1:8080/v1/request \
  -H 'Content-Type: application/json' -H 'Accept: text/event-stream' \
  -d '{"id":"events.count","params":{}}'
```

The same server also offers polling (`POST /v1/requests`) and WebSocket (`/v1/ws`) delivery of the same stream. See [01-catalog](../01-catalog/README.md) steps 6–7.

View the declared outputs, including the emitted item schemas:

```sh
rivet --file app.rivet outputs events.count
rivet --file app.rivet outputs --all --json
```

### Expected Output / Response

`events.count` yields data 1, 2, 3 and exactly one terminal result `{"count":3}`. CLI frames are NDJSON; HTTP frames are SSE. `files.chunks` emits tagged byte values whose concatenation matches the input, then returns null. The chunk count is not fixed. `socket.ping` returns the pong after cleanup. Its output block is `open true`, so extra fields from the fixture are accepted. All three exit 0.

Running `socket.ping` **without** `--policy ./policies/websocket.json` fails with `permission.denied` (exit 3). The auto-discovered policy.json grants no network.

`rivet outputs events.count`:

```text
events.count — Emit a finite sequence
output  object   Summary returned after the last item.
  count    integer  required  Number of items emitted.
emits    integer  One of 1, 2, 3, in order.
receives —
errors   —
```

`rivet outputs --all --json` (abbreviated):

```json
[
  {"id": "events.count",
   "output": {"type": "object", "description": "Summary returned after the last item.",
              "properties": {"count": {"type": "integer", "description": "Number of items emitted."}},
              "required": ["count"], "additionalProperties": false},
   "emits": {"type": "integer", "description": "One of 1, 2, 3, in order."},
   "receives": null, "errors": []},
  {"id": "socket.ping",
   "output": {"type": "object", "description": "First message received after the ping.",
              "properties": {"type": {"type": "string", "description": "Message type; the fixture answers \"pong\"."}},
              "required": ["type"], "additionalProperties": true},
   "emits": null, "receives": null, "errors": []}
]
```

## Effects and policy

The pure stream needs no grants. The file stream needs a read grant, and the WebSocket needs an exact origin grant. Cancellation stops production and awaits owned cleanup. There is no explicit close method. `socket.ping` has its `timeout "20s"` option before the first body statement (leading-options rule).

## Inspect before invoking

```sh
rivet --file app.rivet check --strict-docs
rivet --file app.rivet io --by target
rivet --file app.rivet io --check-policy
rivet --file app.rivet --policy ./policies/websocket.json io socket.ping --check-policy
```

`check --strict-docs` passes: every public operation describes its output, every output field and its emitted items.

**`io --by target`**. `events.count` is pure and contributes no rows:

```text
TARGET                      ACCESS              CAPABILITY      USED BY
./data/lines.txt            read                allow_read      files.chunks
wss://api.example.com:443   connect GET (ws)    allow_network   socket.ping
```

**`io --check-policy`** with the default [policy.json](policy.json), which grants only `allow_read ./data/**`:

```text
OPERATION      KIND      ACCESS        TARGET                           KNOWLEDGE   SOURCE         DECISION
files.chunks   file      read          ./data/lines.txt                 exact       app.rivet:19   allowed
socket.ping    network   connect GET   wss://api.example.com/realtime   exact       app.rivet:35   denied
```

It exits 3, because `socket.ping` is denied. The WebSocket upgrade is recorded as `connect` with `method: GET` and `protocol: ws`. Under [policies/websocket.json](policies/websocket.json), `io socket.ping --check-policy` shows `allowed` and exits 0. Neither file covers both operations on purpose, so `io --check-policy` without an ID exits 3 under either one.

`--include-bootstrap` adds the fixed runtime-internal list under a separate `bootstrap` key: the bundle and imports, policy.json, the CA bundle, resolv.conf or the system resolver, tzdata, descriptor/schema files, and stdin/stdout/stderr. It is listed for transparency, never granted to scripts. Sandbox guarantees apply to **script-initiated effects through brokered adapters**. `io` performs no I/O and evaluates no source expression. Exit codes: 3 when `--check-policy` finds a reachable site denied or partial; 7 with `--strict` when any site is dynamic or opaque (`complete: false`); otherwise 0.

## Release Updates

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-01 | UQ-17 / R23 | Declared outputs and emits schemas | `rivet outputs events.count` | Table above | Not run — runtime does not exist |
| U-02 | UQ-17 / R24 | Three `--sandbox` variants became policy.json + `policies/websocket.json` | Commands above | Same results | Not run |
| U-03 | UQ-18 / R26 | `io` became the generated I/O manifest (targets, access verbs, capability, `--by`, `--check-policy`) | Inspect before invoking | Tables above | Not run — runtime does not exist |

## Cleanup

Stop any server you started with Ctrl-C. Scope exit owns application resources; do not add `socket.close` calls.

## Verification Record

| Step | Verified by | Date | Result |
|---|---|---|---|
| JSON/JSONL parse, relative links, manifest IDs vs declared public IDs, no removed flags or bare durations | Claude static script | 2026-09-28 | Passed as documentation checks; not language conformance |
| Source IDs, JSON fixtures, links and documentation structure | Codex static review | 2026-09-28 | Checked as documentation; not language conformance |
| Parse/execute and validate expected output | Future implementation fixture suite | Not run | BLOCKED — runtime does not exist |

## Known Caveats

A streaming operation invoked without `--stream` on the CLI, or without SSE/polling/WebSocket delivery over HTTP, returns `stream.required` (validation, HTTP 422, exit 2). MCP exposes streaming operations with bounded session delivery; see 10-grpc. No assumption is made that MCP tool results can directly carry an arbitrary data stream. Expected values assume the declared fixture behavior. Request, trace and session IDs are generated; compare application results rather than literal IDs. Runtime errors remain typed and retain partial-effect information.

## Related Documents

- [All sample folders](../README.md)
- [Usage reference](../../references/ref-2026-0002-language-and-usage.md)
- [Proposal](../../proposals/draft/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest: `io --by target` and `io --check-policy` (socket.ping denied under policy.json, allowed under policies/websocket.json). |
| 2 | 2026-09-28 | Claude | UQ-17: declared outputs/emits with descriptions (`open true` on socket.ping); quoted `timeout "20s"`; `--sandbox` variants replaced by policy.json and policies/websocket.json; serve without `--transport`; `stream.required` code; View outputs, strict-docs, bootstrap and exit codes. |
| 1 | 2026-09-28 | Codex | Added draft source files, prerequisites, invocation examples and expected behavior. |
