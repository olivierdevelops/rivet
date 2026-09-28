---
document_id: DEMO-2026-0010
title: "All four gRPC call modes"
document_type: demo
status: draft
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 4
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [registry]
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

# All four gRPC call modes

**Draft usage sample.** Rivet has no runtime or CLI implementation yet. These files make the proposed interface concrete; commands below are intended usage, not executed demos.

## Purpose

All four gRPC call modes. Delivery stage: **B**. Read [app.rivet](app.rivet) alongside this walkthrough.

| Operation ID | Mode | Output | Emits / receives | Behavior |
|---|---|---|---|---|
| `users.grpc_get` | unary | `object {id, name}` | — | GetUser. Renamed from `users.get` so it never collides with the HTTP `users.get` in 03-http. |
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

None. Based on proposal revision 8 semantics (UQ-17, E12) and S110–S118. No parser, runtime or network fixture execution is claimed.

## Prerequisites

A future Rivet build implementing this stage. External services below are controlled fixtures, not public services to contact. Commands assume the repository root initially, then the setup directory. Each folder is an independent bundle; do not concatenate folders with duplicate IDs. The WebSocket step uses the third-party [websocat](https://github.com/vi/websocat) client.

## Setup

```sh
cd docs/demos/10-grpc
```

The [schemas/users.proto](schemas/users.proto) file declares all four methods. With an installed protoc, explicitly generate the descriptor before loading the bundle:

```sh
protoc --proto_path=schemas --include_imports --descriptor_set_out=schemas/users.pb schemas/users.proto
```

This command is host preparation, not an implicit subprocess inside Rivet. No generated binary descriptor is claimed to be present. Provide a controlled native gRPC HTTP/2 server at users.example.com:443 with a trusted matching TLS certificate. Fixture contracts:

- `GetUser("42")` returns Ada.
- `Watch` emits two ChatMessage values, then OK. Topic `fail_after_one` emits one, then UNAVAILABLE.
- `Upload` counts messages.
- `Chat` echoes each message and finishes after the client half-closes.

[policy.json](policy.json) is auto-discovered. It grants the exact origin and the four methods, and its `serve` block mounts all five surfaces with loopback auth `none`.

## Steps

### Command / Request

#### CLI

```sh
rivet --file app.rivet request users.grpc_get --params '{"id":"42"}'
rivet --file app.rivet request users.watch --params '{"topic":"changes"}' --stream
rivet --file app.rivet request users.upload --params '{"items":[{"text":"a"},{"text":"b"}]}'
rivet --file app.rivet request chat.exchange --params '{}' --input-jsonl - --stream < chat-input.jsonl
```

#### One server, every surface

In a dedicated terminal:

```sh
rivet --file app.rivet serve --listen 127.0.0.1:8080
```

**Polling (live input).** Open the session. The body is [requests/open.http.json](requests/open.http.json):

```sh
curl -sS -X POST http://127.0.0.1:8080/v1/requests \
  -H 'Content-Type: application/json' --data-binary @requests/open.http.json
```

Set `S` to the returned `session_id` (it also appears in `events_url`), then send input, finish input and read events:

```sh
S='REPLACE_WITH_SESSION_ID'
curl -sS -X POST http://127.0.0.1:8080/v1/requests/$S/input \
  -H 'Content-Type: application/json' --data-binary @requests/send.http.json
curl -sS -X POST http://127.0.0.1:8080/v1/requests/$S/finish_input \
  -H 'Content-Type: application/json' --data-binary @requests/finish_input.http.json
curl -sS "http://127.0.0.1:8080/v1/requests/$S/events?after_seq=0&wait_ms=1000"
```

Repeat the events read with `after_seq` set to the last `last_seq` until `terminal` is `true`. To stop early, post [requests/cancel.http.json](requests/cancel.http.json) to `/v1/requests/$S/cancel`. For sustained conversations, send and read concurrently rather than accumulating all input first. Finish within the operation's `timeout "30s"`.

```text
  POST /v1/requests ----> 202 SessionReceipt {session_id, events_url, next_send_seq:1}
  POST .../input  {send_seq:1, data}  ----> SessionAck {accepted_seq:1, input_closed:false}
  POST .../finish_input {}            ----> SessionAck {accepted_seq:null, input_closed:true}
  GET  .../events?after_seq=0         ----> SessionBatch {events:[data..., result], terminal:true}
```

**WebSocket (live input, connection-owned).** [requests/ws-chat.jsonl](requests/ws-chat.jsonl) holds a request frame, two input frames and a finish_input frame for ref `chat1`:

```sh
websocat --protocol rivet.v1 ws://127.0.0.1:8080/v1/ws < requests/ws-chat.jsonl
```

**MCP.** MCP initialization is shown in [01-catalog](../01-catalog/README.md). Once initialized on this host, post [requests/open.mcp.json](requests/open.mcp.json) to `/mcp`. This direct named tool call `tools/call {name:"chat.exchange"}` is the canonical form, and it returns a SessionReceipt in `structuredContent`. Continue with the built-in generic tools `rivet.sessions.send`, `rivet.sessions.read`, `rivet.sessions.finish_input` and `rivet.sessions.cancel`. Unary `users.grpc_get` and finite `users.upload` are ordinary direct tools that return a Completion.

#### View outputs

```sh
rivet --file app.rivet outputs chat.exchange
rivet --file app.rivet outputs --all --json
curl -sS http://127.0.0.1:8080/v1/operations/users.grpc_get/outputs
```

### Expected Output / Response

The unary result is `{"id":"42","name":"Ada"}` and the upload result is `{"count":2}`, each exit 0. Watch and Chat emit data, then one terminal result after OK trailers. A late UNAVAILABLE becomes a terminal `grpc.unavailable` error (exit 5) and keeps the data already emitted. A send acknowledgment means queue acceptance, not remote processing. Calling `chat.exchange` on the CLI without `--input-jsonl` is `stream.input_required` (validation, HTTP 422, exit 2).

WebSocket server frames for `chat1`:

```json
{"type":"data","ref":"chat1","seq":1,"data":{"text":"hello"}}
{"type":"data","ref":"chat1","seq":2,"data":{"text":"goodbye"}}
{"type":"result","ref":"chat1","completion":{"request_id":"req_21","trace_id":"tr_21","result":{"status":"OK"},"data_count":2,"effects":"unknown"}}
```

The completion `result` shape is illustrative, because the output is declared as opaque `json`. If the socket closes before the result frame, Rivet cancels `chat1` and joins its cleanup; unlike a polling session, it cannot be resumed.

`rivet outputs chat.exchange`:

```text
chat.exchange — Chat with the service
output  json     gRPC completion (final OK status and bounded trailers); opaque, not structurally checked.
emits    object   One echoed example.ChatMessage per server message.
  text     text     required  Echoed message text.
receives object   One example.ChatMessage per caller input item.
  text     text     required  Message text to send.
errors   —
```

`rivet outputs --all --json` (first entry shown):

```json
[
  {"id": "chat.exchange",
   "output": {"description": "gRPC completion (final OK status and bounded trailers); opaque, not structurally checked."},
   "emits": {"type": "object", "description": "One echoed example.ChatMessage per server message.",
             "properties": {"text": {"type": "string", "description": "Echoed message text."}},
             "required": ["text"], "additionalProperties": false},
   "receives": {"type": "object", "description": "One example.ChatMessage per caller input item.",
                "properties": {"text": {"type": "string", "description": "Message text to send."}},
                "required": ["text"], "additionalProperties": false},
   "errors": []}
]
```

An input item without `text` is rejected against the `receives` schema before it is sent.

## Effects and policy

[policy.json](policy.json) grants the exact origin and four specific methods. Descriptor bootstrap, auth and credentials remain independent effects. No OAuth is used here; see 07-oauth2 and S118 for profile attachment. All channels and streams belong to the request scope.

## Inspect before invoking

```sh
rivet --file app.rivet check --strict-docs
rivet --file app.rivet io --by target
rivet --file app.rivet io --check-policy
rivet --file app.rivet io --needs --include-bootstrap
```

`check --strict-docs` passes: every public operation describes its params, output and each output, emits and receives field. The IDs are unique in this bundle, and a duplicate would be `registry.duplicate_id` (exit 2).

**`io --by target`**. Each call produces a network site for the connector endpoint and a gRPC call site that records the call mode:

```text
TARGET                          ACCESS                     CAPABILITY      ORIGIN      PHASE     NEEDS FILE   USED BY
https://users.example.com:443   connect POST (grpc)        allow_network   endpoint    connect   —            chat.exchange, users.grpc_get, users.upload, users.watch
users/example.Users/Chat        call bidi                  allow_grpc      with grpc   body      —            chat.exchange
users/example.Users/GetUser     call unary                 allow_grpc      grpc        body      —            users.grpc_get
users/example.Users/Upload      call client_stream         allow_grpc      with grpc   body      —            users.upload
users/example.Users/Watch       call server_stream         allow_grpc      with grpc   body      —            users.watch
```

**`io --check-policy`** with the auto-discovered [policy.json](policy.json). All eight sites are allowed, so it exits 0:

```text
OPERATION        KIND      ACCESS               TARGET                          KNOWLEDGE   SOURCE         DECISION
chat.exchange    network   connect POST         https://users.example.com:443   exact       app.rivet:2    allowed
chat.exchange    grpc      call bidi            users/example.Users/Chat        exact       app.rivet:68   allowed
users.grpc_get   network   connect POST         https://users.example.com:443   exact       app.rivet:2    allowed
users.grpc_get   grpc      call unary           users/example.Users/GetUser     exact       app.rivet:15   allowed
users.upload     network   connect POST         https://users.example.com:443   exact       app.rivet:2    allowed
users.upload     grpc      call client_stream   users/example.Users/Upload      exact       app.rivet:47   allowed
users.watch      network   connect POST         https://users.example.com:443   exact       app.rivet:2    allowed
users.watch      grpc      call server_stream   users/example.Users/Watch       exact       app.rivet:30   allowed
```

Network rows point at the connector's `endpoint` line (2), where the connection is declared; call rows point at the `grpc` statement. Removing one method from the `allow_grpc` targets denies only that operation's call row, and `io --check-policy` exits 3. The generated `schemas/users.pb` appears only under `--include-bootstrap`, as a descriptor file.

**The connector `descriptor` file is a bootstrap read, not an operation site.** `descriptor "./schemas/users.pb"` (app.rivet:3) is a file-valued option at connector level, so it is read when the bundle is assembled. It carries the same site fields as every other site, but it is never granted by policy.json or by `policy generate`:

```text
KEY         SITE                              ORIGIN                     PHASE   REQUIRES_EXISTING   SECRET
bootstrap   file read ./schemas/users.pb      option descriptor          load    yes                 no
            (app.rivet:3)                     — never granted; a missing file fails the bundle load (not_found, exit 4)
```

`rivet --file app.rivet io --needs --include-bootstrap` lists it under its own heading:

```text
bundle load needs:
  ./schemas/users.pb    (descriptor)
chat.exchange needs no existing files.
users.grpc_get needs no existing files.
users.upload needs no existing files.
users.watch needs no existing files.
```

`io --needs` without `--include-bootstrap` prints only the four `needs no existing files.` lines: the operations need no existing files of their own. If `users.pb` were missing (for example, `protoc` was not run), the bundle would not load at all, so it can never be a per-operation need.

`--include-bootstrap` adds the fixed runtime-internal list under a separate `bootstrap` key: the bundle and imports, policy.json, the CA bundle, resolv.conf or the system resolver, tzdata, descriptor/schema files, and stdin/stdout/stderr. It is listed for transparency, never granted to scripts. Sandbox guarantees apply to **script-initiated effects through brokered adapters**. `io` performs no I/O and evaluates no source expression. Exit codes: 3 when `--check-policy` finds a reachable site denied or partial; 7 with `--strict` when any site is dynamic or opaque (`complete: false`); otherwise 0.

## Release Updates

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-01 | UQ-17 / R23 | Declared outputs, emits and receives | `rivet outputs chat.exchange` | Table above | Not run — runtime does not exist |
| U-02 | UQ-17 / R25 | One serve; polling routes and WebSocket frames for live input | Steps above | Same chat results on every surface | Not run |
| U-03 | Review E12 | `users.get` renamed `users.grpc_get` | `request users.grpc_get` | `{"id":"42","name":"Ada"}` | Not run |
| U-04 | UQ-18 / R26 | `io` became the generated I/O manifest (targets, access verbs, capability, `--by`, `--check-policy`) | Inspect before invoking | Tables above | Not run — runtime does not exist |

## Cleanup

Cancel any open polling sessions, then stop the host and the fixture server. WebSocket refs are cancelled when their socket closes. Remove `schemas/users.pb` only if you generated it for this walkthrough, and keep the source `.proto`.

## Verification Record

| Step | Verified by | Date | Result |
|---|---|---|---|
| JSON/JSONL parse, relative links, manifest IDs vs declared public IDs, no removed flags or bare durations | Claude static script | 2026-09-28 | Passed as documentation checks; not language conformance |
| Source IDs, JSON fixtures, links and documentation structure | Codex static review | 2026-09-28 | Checked as documentation; not language conformance |
| Parse/execute and validate expected output | Future implementation fixture suite | Not run | BLOCKED — runtime does not exist |

## Known Caveats

Expected values assume the declared fixture behavior. Request, trace and session IDs are generated; compare application results rather than literal IDs. Runtime errors remain typed and retain partial-effect information.

## Related Documents

- [All sample folders](../README.md)
- [Usage reference](../../references/ref-2026-0002-language-and-usage.md)
- [Proposal](../../proposals/approved/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 4 | 2026-09-28 | Claude | TASK-005/R26 (ADR-0001, proposal revision 8): `io --by target` gains ORIGIN (`endpoint`, `grpc`, `with grpc`), PHASE and NEEDS FILE; connector `descriptor` shown as a bootstrap `load` read with new fields; `io --needs --include-bootstrap` excerpt. |
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest: endpoint + four call-mode sites by target and `io --check-policy` (all eight allowed). |
| 2 | 2026-09-28 | Claude | UQ-17: renamed `users.get` → `users.grpc_get` (E12); declared outputs/emits/receives; quoted durations with options first; `--policy policy.json` dropped (auto-discovered, with `serve` block); one `serve` replaces `--transport … --mcp`; polling routes replace generic session request files (removed `requests/read.http.json`); added `requests/ws-chat.jsonl`; View outputs, strict-docs, bootstrap and exit codes. |
| 1 | 2026-09-28 | Codex | Added draft source files, prerequisites, invocation examples and expected behavior. |
