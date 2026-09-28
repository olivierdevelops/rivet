---
document_id: DEMO-2026-0001
title: "One file, four operations, every access point"
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

# One file, four operations, every access point

**Draft usage sample.** Rivet has no runtime or CLI implementation yet. These files make the proposed interface concrete; commands below are intended usage, not executed demos.

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
     request                        REST  SSE  poll  WS  MCP  (one listener)
           |                                        |
           +------------ same immutable catalog ----+
```

## Verified Against Version

None. Based on proposal revision 5 semantics (UQ-17: declared outputs, policy.json, one serve) and S103–S108 and S120. No parser, runtime or network fixture execution is claimed.

## Prerequisites

A future Rivet build implementing this stage. Commands assume the repository root initially, then the setup directory. Each folder is an independent bundle; do not concatenate folders with duplicate IDs. The WebSocket step uses the third-party [websocat](https://github.com/vi/websocat) client; any WebSocket client that can set a subprotocol works.

## Setup

```sh
cd docs/demos/01-catalog
```

No data files or remote fixtures are needed. The four operations are pure.

**This folder deliberately has no `policy.json`.** When no policy file sits next to the entry `.rivet` file (and no `--policy PATH` is passed), Rivet is **deny-by-default** for all new application I/O. Pure operations still run; any read, write, network, environment or credential effect would fail with `permission.denied` (exit 3). There is no implicit allow-all mode.

```text
  rivet --file app.rivet ...
        |
        +-- --policy PATH given? ----yes----> load that file (path only, never grants)
        |          no
        +-- ./policy.json next to app.rivet? --yes--> load it
        |          no
        +-- deny-by-default: pure ops run, every new application effect is denied
```

The alternate file [policies/team.json](policies/team.json) is used only in the authentication step below.

## Steps

### Command / Request

#### 1. Discover and call from the CLI

```sh
rivet --file app.rivet list --outputs
rivet --file app.rivet describe demo.add --json
rivet --file app.rivet request demo.add --params '{"a":2,"b":3}'
rivet --file app.rivet request demo.greet --params '{"person":"Ada"}'
rivet --file app.rivet request demo.health --params '{}'
rivet --file app.rivet request demo.countdown --params '{}' --stream
```

#### 2. View outputs

```sh
rivet --file app.rivet outputs demo.health
rivet --file app.rivet outputs --all --json
```

#### 3. Start one server for every surface

In a dedicated terminal:

```sh
rivet --file app.rivet serve --listen 127.0.0.1:8080
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
 GET /v1/operations/{id}  event-stream   {id}/events     rivet.v1
 GET /v1/operations/{id}/outputs          ?after_seq&wait_ms
                                      POST /v1/requests/{id}/input
                                      POST /v1/requests/{id}/finish_input
                                      POST /v1/requests/{id}/cancel
      +----------------+-----------------+------------------+-----------------+
                                         |
                   one auth -> principal -> operation authorization
                                         |
                     shared dispatcher / immutable catalog / broker
```

Run every following step from a second terminal in this folder.

#### 4. REST

```sh
curl -sS http://127.0.0.1:8080/v1/operations
curl -sS http://127.0.0.1:8080/v1/operations/demo.add
curl -sS http://127.0.0.1:8080/v1/operations/demo.add/outputs
curl -sS http://127.0.0.1:8080/v1/request -H 'Content-Type: application/json' --data-binary @requests/add.http.json
```

#### 5. SSE (same route, different `Accept`)

```sh
curl -N http://127.0.0.1:8080/v1/request \
  -H 'Content-Type: application/json' -H 'Accept: text/event-stream' \
  --data-binary @requests/countdown.http.json
```

#### 6. Polling: open → events → terminal

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

Repeat with `after_seq` set to the returned `last_seq` until `terminal` is `true`. A unary operation can be submitted the same way as an asynchronous job; its batch contains a single terminal `result` event:

```sh
curl -sS -X POST http://127.0.0.1:8080/v1/requests \
  -H 'Content-Type: application/json' --data-binary @requests/add.http.json
```

Polling sessions survive client reconnects until they expire or are cancelled (`POST /v1/requests/{id}/cancel`).

#### 7. WebSocket frames

[requests/ws-frames.jsonl](requests/ws-frames.jsonl) holds three client frames, multiplexed by client-chosen `ref`:

```sh
websocat --protocol rivet.v1 ws://127.0.0.1:8080/v1/ws < requests/ws-frames.jsonl
```

```text
 client frames (ref)                      server frames (per-ref order guaranteed)
 c1 request demo.add {a:2,b:3}   ------>  c1 result  completion.result = 5
 c2 request demo.countdown {}    ------>  c2 data seq 1..3, then c2 result {count:3}
 c3 request demo.add {b:3}       ------>  c3 error   validation.required
                                          (frames of different refs may interleave)
```

#### 8. MCP (Streamable HTTP at /mcp)

Initialize and note the returned `MCP-Session-Id` header:

```sh
curl -i http://127.0.0.1:8080/mcp \
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
    --data-binary @requests/$body.mcp.json
done
```

The bodies are [initialized](requests/initialized.mcp.json), [list](requests/list.mcp.json), [add](requests/add.mcp.json) (`tools/call {name:"demo.add"}`) and [outputs](requests/outputs.mcp.json) (`tools/call {name:"rivet.outputs"}`). `rivet.request` also exists as a built-in generic tool, but direct named tools are the canonical way to call an operation.

For an MCP client that launches Rivet as a subprocess, use stdio instead. It serves MCP only, because stdio cannot share a socket with the other surfaces:

```sh
rivet --file app.rivet serve --stdio
```

#### 9. Authentication and narrowed surfaces (alternate policy file)

Stop the server and restart it with the alternate policy:

```sh
rivet --file app.rivet --policy ./policies/team.json serve --listen 127.0.0.1:8080
```

[policies/team.json](policies/team.json) grants no I/O. It configures `serve.auth` as bearer tokens (stored only as sha256 hashes), gives principal `ada` `demo.*` and principal `ci` only `demo.health`, and mounts every surface except `ws`. The fixture-only tokens are `dev-token-ada` and `dev-token-ci`:

```sh
curl -sS http://127.0.0.1:8080/v1/request -H 'Authorization: Bearer dev-token-ada' \
  -H 'Content-Type: application/json' --data-binary @requests/add.http.json
curl -sS http://127.0.0.1:8080/v1/request -H 'Authorization: Bearer dev-token-ci' \
  -H 'Content-Type: application/json' --data-binary @requests/add.http.json
curl -sS -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8080/v1/request \
  -H 'Content-Type: application/json' --data-binary @requests/add.http.json
curl -sS -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8080/v1/ws
```

```text
  Authorization header ---> serve.auth (bearer sha256) ---> principal ---> serve.principals[p].operations
        missing/wrong  ---> 401                              not listed ---> 403 permission.denied
  surface not in serve.surfaces ---------------------------------------------> 404
```

The same principal model applies to the `Authorization` header on REST/SSE/polling, the WebSocket upgrade request and every MCP HTTP request. A non-loopback `--listen` with auth `none` refuses to start (`serve.auth_required`, exit 2).

The embedding equivalent is in [12-library](../12-library/README.md).

### Expected Output / Response

**CLI.** `request` results are `5`, `"Hello, Ada!"` and `{"ready":true}`, each exit 0. `demo.countdown --stream` prints NDJSON data items 3, 2, 1 and one terminal result `{"count":3}`. `list --outputs` adds a one-line output summary per operation; `describe` now includes an **Output** section (params, output and errors). Omitting `b` applies zero; omitting `a` is `validation.required` before execution (exit 2). An unknown ID is `not_found` (exit 4).

**View outputs.** `rivet outputs demo.health` prints:

```text
demo.health — Check availability
output  object   Readiness report for this catalog.
  ready    boolean  required  True whenever the host can run pure operations.
emits    —
receives —
errors   —
```

`rivet outputs --all --json` prints one entry per public operation (abbreviated):

```json
[
  {"id": "demo.add",
   "output": {"type": "integer", "description": "Sum of a and b."},
   "emits": null, "receives": null, "errors": []},
  {"id": "demo.countdown",
   "output": {"type": "object", "description": "Summary returned after the last item.",
              "properties": {"count": {"type": "integer", "description": "Number of items emitted."}},
              "required": ["count"], "additionalProperties": false},
   "emits": {"type": "integer", "description": "One countdown value per item."},
   "receives": null, "errors": []}
]
```

`GET /v1/operations/demo.add/outputs` and the MCP `rivet.outputs` tool return the same JSON for one ID.

**REST.** `POST /v1/request` returns HTTP 200 with a Completion such as `{"request_id":"req_01","trace_id":"tr_01","result":5,"data_count":0,"effects":"none"}`. The dispatcher validates each result against the declared output before returning; a mismatch would be `output.invalid` (HTTP 500, exit 5).

**SSE.**

```text
id: 1
event: data
data: {"request_id":"req_02","trace_id":"tr_02","seq":1,"type":"data","data":3}

id: 2
event: data
data: {"request_id":"req_02","trace_id":"tr_02","seq":2,"type":"data","data":2}

id: 3
event: data
data: {"request_id":"req_02","trace_id":"tr_02","seq":3,"type":"data","data":1}

id: 4
event: result
data: {"request_id":"req_02","trace_id":"tr_02","seq":4,"type":"result","result":{"count":3}}
```

**Polling.** The open call returns HTTP 202:

```json
{"session_id": "sess_01", "request_id": "req_03", "catalog_version": "sha256:fixture",
 "input_schema": null, "emits_schema": {"type": "integer"}, "next_send_seq": 1,
 "events_url": "/v1/requests/sess_01/events", "expires_at": "2026-09-28T12:00:30Z"}
```

Polling `events_url` returns a SessionBatch; the countdown usually fits in one batch:

```json
{"session_id": "sess_01", "last_seq": 4, "terminal": true, "events": [
  {"request_id": "req_03", "trace_id": "tr_03", "seq": 1, "type": "data", "data": 3},
  {"request_id": "req_03", "trace_id": "tr_03", "seq": 2, "type": "data", "data": 2},
  {"request_id": "req_03", "trace_id": "tr_03", "seq": 3, "type": "data", "data": 1},
  {"request_id": "req_03", "trace_id": "tr_03", "seq": 4, "type": "result", "result": {"count": 3}}]}
```

The unary `demo.add` job returns one batch with one terminal `result` event whose result is `5`.

**WebSocket.** The server sends JSON text frames. Frames for different refs may interleave; each ref gets exactly one terminal frame (`result` or `error`):

```json
{"type":"result","ref":"c1","completion":{"request_id":"req_11","trace_id":"tr_11","result":5,"data_count":0,"effects":"none"}}
{"type":"data","ref":"c2","seq":1,"data":3}
{"type":"data","ref":"c2","seq":2,"data":2}
{"type":"data","ref":"c2","seq":3,"data":1}
{"type":"error","ref":"c3","error":{"kind":"validation","code":"validation.required","message":"missing required param a","retryable":false,"operation_id":"demo.add","effects":"none"}}
{"type":"result","ref":"c2","completion":{"request_id":"req_12","trace_id":"tr_12","result":{"count":3},"data_count":3,"effects":"none"}}
```

A connection may hold at most 8 in-flight refs; each ref has a bounded 16-frame queue. Closing the socket cancels and joins all of its refs. Unlike polling sessions, WebSocket requests are owned by the connection.

**MCP.** `tools/list` contains the four direct tools with their names, descriptions, `inputSchema` and `outputSchema`. Each `outputSchema` is the Completion envelope whose `result` property is the declared output. The list also contains the built-in generic tools `rivet.request`, `rivet.list`, `rivet.describe`, `rivet.outputs` and `rivet.sessions.*`. `demo.countdown` is advertised with `_meta: {"rivet/delivery":"session"}`; calling it returns a SessionReceipt. The direct `demo.add` call returns the Completion in `structuredContent`, so `structuredContent.result` is `5`.

**Authentication step.** `ada` gets HTTP 200 with result 5; `ci` gets HTTP 403 `permission.denied`; no header gets 401; `/v1/ws` gets 404 because `ws` is not in `serve.surfaces`.

| Condition | Code | HTTP | CLI exit |
|---|---|---|---|
| Success | — | 200 (202 for polling open) | 0 |
| Missing `a` | `validation.required` | 422 | 2 |
| Malformed policy file | `policy.invalid` | — | 2 |
| Principal not authorized | `permission.denied` | 403 | 3 |
| Unknown operation ID | `not_found` | 404 | 4 |
| Result does not match declared output | `output.invalid` | 500 | 5 |

## Effects and policy

No application effects, so no policy.json is needed. The host owns bootstrap reads and the listener. MCP stdio stdout carries only protocol messages, never logs. An HTTP or MCP caller can only narrow authority for its own request, never grant it. Effective authority is the host ceiling ∩ policy file ∩ per-request restriction.

## Inspect before invoking

```sh
rivet --file app.rivet check --strict-docs
rivet --file app.rivet io --by target
rivet --file app.rivet io --check-policy
rivet --file app.rivet io --include-bootstrap --strict --format json
```

`check --strict-docs` passes. Every public operation has descriptions on its params, output and each output/emits field. None of the operations `fail`, so no `error` lines are required. Removing any `description` from an `output` or `field` line makes strict-docs fail with exit 2.

**I/O manifest.** All four operations are pure, so the manifest has no application effect sites:

```text
TARGET   ACCESS   CAPABILITY   USED BY
(no application effect sites: demo.add, demo.countdown, demo.greet, demo.health are pure)
```

`io --check-policy` finds no policy.json beside app.rivet, so the effective policy is deny-by-default. With zero sites nothing can be denied, and it exits 0. The alternate [policies/team.json](policies/team.json) holds only `limits` and `serve` settings and no grants, so `--policy ./policies/team.json io --check-policy` gives the same result. The JSON form is `{"bundle": {...}, "policy": null, "complete": true, "sites": [], "targets": [], "bootstrap": [...]}`, and `--strict` exits 0.

`--include-bootstrap` adds the fixed runtime-internal list under a separate `bootstrap` key: the bundle and imports, policy.json, the CA bundle, resolv.conf or the system resolver, tzdata, descriptor/schema files, and stdin/stdout/stderr. It is listed for transparency, never granted to scripts. Sandbox guarantees apply to **script-initiated effects through brokered adapters**. `io` performs no I/O and evaluates no source expression. Exit codes: 3 when `--check-policy` finds a reachable site denied or partial; 7 with `--strict` when any site is dynamic or opaque (`complete: false`); otherwise 0.

## Release Updates

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-01 | UQ-17 / R23 | Operations declare described outputs | `rivet outputs --all --json` | Output schema per operation | Not run — runtime does not exist |
| U-02 | UQ-17 / R24 | `--sandbox` removed; no policy.json = deny-by-default | Run without any policy flag | Pure ops succeed | Not run |
| U-03 | UQ-17 / R25 | One `serve` mounts REST, SSE, polling, WS and MCP | Steps 3–8 | Same results on every surface | Not run |
| U-04 | UQ-18 / R26 | `io` became the generated I/O manifest (targets, access verbs, capability, `--by`, `--check-policy`) | Inspect before invoking | Tables above | Not run — runtime does not exist |

## Cleanup

Stop any server you started with Ctrl-C. Stopping the server cancels open polling sessions and WebSocket refs and awaits their cleanup.

## Verification Record

| Step | Verified by | Date | Result |
|---|---|---|---|
| JSON/JSONL parse, relative links, manifest IDs vs declared public IDs, no removed flags | Claude static script | 2026-09-28 | Passed as documentation checks; not language conformance |
| Source IDs, JSON fixtures, links and documentation structure | Codex static review | 2026-09-28 | Checked as documentation; not language conformance |
| Parse/execute and validate expected output | Future implementation fixture suite | Not run | BLOCKED — runtime does not exist |

## Known Caveats

Expected values assume the declared fixture behavior. Request, trace and session IDs are generated; compare application results rather than literal IDs. The WebSocket error `message` text is illustrative. Runtime errors remain typed and retain partial-effect information.

## Related Documents

- [All sample folders](../README.md)
- [Usage reference](../../references/ref-2026-0002-language-and-usage.md)
- [Proposal](../../proposals/draft/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest: `io --by target` shows no application sites; `io --check-policy` exits 0 with no policy.json. |
| 2 | 2026-09-28 | Claude | UQ-17 rework: added `demo.countdown` and declared/described outputs; removed `--sandbox` (no policy.json = deny-by-default) and added `policies/team.json` for bearer auth; replaced `--transport`/`--mcp` with one `serve` showing REST, SSE, polling, WebSocket (`requests/ws-frames.jsonl`) and direct-tool MCP; added View outputs, strict-docs, bootstrap and exit-code expectations. |
| 1 | 2026-09-28 | Codex | Added draft source files, prerequisites, invocation examples and expected behavior. |
