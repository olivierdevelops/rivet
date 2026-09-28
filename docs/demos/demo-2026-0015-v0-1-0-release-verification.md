---
document_id: DEMO-2026-0015
title: "Rivet v0.1.0 release verification guide"
document_type: demo
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 2
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, registry, execution, files, connectors, audit, policy, auth, datagrams, quic, grpc, sessions, serve, transports, cli, http, ws, poll, mcp, library]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, embedded, server]
audience: [maintainers, reviewers, release-verifiers]
scope: Executable verification procedure for every update released in Rivet 0.1.0 (U-01…U-26, one per requirement R1…R26), with success and failure examples per surface (CLI, HTTP/SSE, polling, WebSocket, MCP, library), cleanup and the verification record. It links the twelve sample folders instead of repeating them.
reason: DOCUMENTATION.md §29 requires a release verification guide and demo for every release; PLAN-2026-0001 TASK-068 (D-12, T-30).
release_version: "0.1.0"
release_tag: v0.1.0
release_commit: f69b5b911f174b0198adb89c8305c8cce268fc11
dependencies: [PLAN-2026-0001, PROP-2026-0001]
related_documents: [PLAN-2026-0001, PROP-2026-0001, RPT-2026-0001, DEMO-2026-0001, DEMO-2026-0002, DEMO-2026-0003, DEMO-2026-0004, DEMO-2026-0005, DEMO-2026-0006, DEMO-2026-0007, DEMO-2026-0008, DEMO-2026-0009, DEMO-2026-0010, DEMO-2026-0011, DEMO-2026-0012, DEMO-2026-0013, MAN-2026-0001, API-2026-0005]
supersedes: null
superseded_by: null
tags: [rivet, release, verification, demo, v0.1.0]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-28
verified_against: "0.1.0"
---

# Rivet v0.1.0 release verification guide

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** every Rivet feature and surface (language through library)

```text
  ┌──────────────────────────────────────────────────────────────────────┐
  │  RELEASE      Rivet 0.1.0                                            │
  │  TAG          v0.1.0 (annotated)                                   │
  │  COMMIT       f69b5b911f174b0198adb89c8305c8cce268fc11             │
  │  VERIFIED ON  0.1.0-dev at commit 829ca43, the release candidate     │
  │  PLAN         PLAN-2026-0001   PROPOSAL  PROP-2026-0001 rev 8        │
  └──────────────────────────────────────────────────────────────────────┘
```

## Purpose

A reader who did not implement Rivet can use this page to decide, update by update, whether the 0.1.0 build does what the approved proposal promised. Each `U-NN` row names the user question (UQ) that incited the requirement, the exact command, the expected result and the evidence recorded for this release candidate. The long walkthroughs live in the twelve [sample folders](README.md); this page links to them.

```text
   UQ (user question) ──▶ R-NN (proposal requirement) ──▶ U-NN (this page) ──▶ command ──▶ PASS / FAIL
                                                              │
                                     evidence: demo step (docs/demos/NN-*) + TEST-2026-00NN suite
```

## Verified Against Version

0.1.0. Verified on 0.1.0-dev at commit `829ca43`, the release candidate. The version string is bumped to 0.1.0 at release (P5); rerun this guide against the tagged artifact (TASK-081) and add a row to the Verification Record. Host: macOS 26.4.1 (Darwin 25.4.0, arm64), 2026-09-28, `target/release/rivet`. Request, trace and session IDs, timestamps, hashes and ports vary between runs.

## Prerequisites

```sh
cargo build && cargo build --release            # from the repository root
export PATH="$PWD/target/release:$PATH"
rivet --version                                  # rivet 0.1.0
```

| Needed for | Tool |
|---|---|
| every step | `curl`, `python3` (stdlib fixtures: 03, 06, 07, 08) |
| WebSocket, QUIC/HTTP3, gRPC fixtures | a venv with `websockets aioquic grpcio protobuf` |
| 09-quic certificates, 10-grpc descriptor | `openssl`, `protoc` |
| library step | a Rust toolchain (edition 2024) |

Fixtures and servers use loopback ports 18800–18899 only. Nothing contacts the internet.

## Setup

```sh
python3 -m venv "$TMPDIR/rivet-verify-venv"
"$TMPDIR/rivet-verify-venv/bin/pip" install websockets aioquic grpcio protobuf
```

Every folder is independent: `cd docs/demos/NN-topic` and follow its README. Folders that need a service run a shipped fixture in a scratch copy (`WORK="$(mktemp -d)"`), so nothing is written into the repository.

```text
   docs/demos/
   ├── 01-catalog      every surface from one serve       ├── 07-oauth2    OAuth fixture
   ├── 02-file-crud    files                              ├── 08-udp       UDP fixture
   ├── 03-http         HTTP fixture                       ├── 09-quic      QUIC + HTTP/3 fixture
   ├── 04-streaming    SSE, file handles, WS fixture      ├── 10-grpc      gRPC fixture, sessions
   ├── 05-dag          DAG                                ├── 11-sandbox   policy, manifest, generate
   ├── 06-mcp-bridge   MCP fixture                        └── 12-library   Rust embedding
```

## Steps

### Release Updates

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-01 | UQ-09/10 → R1 | Rust library that embeds the Capy parser; no Capy executable | [12-library](12-library/README.md) step 2; `cargo tree -i capy-core --depth 0` | Program builds and exits 0; `capy-core v0.22.0 (…?rev=84f984c6…)` | Recorded 2026-09-28 at 829ca43; TEST-2026-0001, TEST-2026-0010 |
| U-02 | UQ-03 → R2 | One grammar for every statement; diagnostics with file/line/column | `rivet --file app.rivet check --strict-docs` in each folder; a file ending in `return 1 +` | `ok: N operations …` (exit 0); `error[syntax.e0001] … --> bad.rivet:5:5` (exit 2) | All 12 folders parse (06 after its snapshot step); TEST-2026-0023, TEST-2026-0029 |
| U-03 | UQ-02 → R3 | `with` scopes close handles on success, timeout and cancel | [04-streaming](04-streaming/README.md) steps 4, 6, 7 | `close` logged each time; exits 0 / 6 / 130 | Recorded; TEST-2026-0003, TEST-2026-0005 |
| U-04 | UQ-03 → R4 | One error registry: code → kind → HTTP → exit | [03-http](03-http/README.md) steps 5, 7 | `users.not_found` 502/5, `http.status` 502/5, `output.invalid` 500/5, `validation.min` 422/2 | Recorded; TEST-2026-0024, [API-2026-0005](../api/api-2026-0005-error-registry.md) |
| U-05 | UQ-04 → R5 | File create/read/update/delete/list with guards | [02-file-crud](02-file-crud/README.md) steps 5–7 | `conflict.already_exists` 4; `not_found.file` 4; `file.hardlink_refused` 3 | Recorded; TEST-2026-0004 |
| U-06 | UQ-05/18 → R6 | Trace attempts carry `effect_id`; `io --trace` joins planned and actual | [03-http](03-http/README.md) step 6; [11-sandbox](11-sandbox/README.md) step 8 | `2 allowed` for a retried GET; `1 allowed` for a read | Recorded; TEST-2026-0009 |
| U-07 | UQ-06 → R7 | Outbound MCP connectors with reviewed snapshots, sync and drift check | [06-mcp-bridge](06-mcp-bridge/README.md) steps 1–3, 6–7 | `not_found.mcp_snapshot` 4 → sync → `mcp.snapshot_unapproved` 2 → Ada; `mcp.tool_failed`; `mcp.schema_drift` | Recorded; TEST-2026-0006 |
| U-08 | UQ-07/09 → R8 | One dispatcher behind CLI, REST, SSE, polling, WS, MCP and library | [01-catalog](01-catalog/README.md) steps 1, 4–8; [12-library](12-library/README.md) | `demo.add` returns 5 on every surface | Recorded; TEST-2026-0002 |
| U-09 | UQ-08 → R9 | `request(ID, params, on_data)`; streams end with one terminal event | [04-streaming](04-streaming/README.md) step 3; [12-library](12-library/README.md) step 2 | Data 1, 2, 3 then `{count:3}` | Recorded; TEST-2026-0003, TEST-2026-0010 |
| U-10 | UQ-11 → R10 | DAG with `after`, limits, `fail fast` / `fail independent` | [05-dag](05-dag/README.md) steps 2–3 | `{total:10}`; `succeeded/failed/blocked` | Recorded; TEST-2026-0007 |
| U-11 | UQ-13/17 → R11 | Deny-by-default; deny beats grant; host ceiling only narrows | [11-sandbox](11-sandbox/README.md) step 2; [12-library](12-library/README.md) step 2 (`.ceiling`) | `permission.denied` exit 3 under `empty.json` and for `data.private` | Recorded on macOS; Linux/Windows process sandbox not verifiable here (Known Caveats); TEST-2026-0008 |
| U-12 | UQ-01/12 → R12 | Protocol availability matrix; unsupported stages refuse | `request rivet.capabilities`; a `with file watch` operation | `stages … "C":"unsupported"`; `unsupported.stage_c` exit 5 | Recorded ([01-catalog](01-catalog/README.md) step 1 and failure example below); TEST-2026-0005 |
| U-13 | PROJECT §§6, 79–80 → R13 | argv-only processes; secrets bound to origins and never returned or logged | `command "/usr/bin/printf" args ["%s", "hello; echo this stays data"]`; [07-oauth2](07-oauth2/README.md) step 5 | Result `"hello; echo this stays data"`; `grep -c DEMO-AT` → 0 | Recorded; TEST-2026-0008, TEST-2026-0009 |
| U-14 | UQ-12 / AGENTS → R14 | Documentation set, indexes and executed demos | `python3 scripts/check_docs.py`; `vhco docs check .` | `0 problem(s)`; `0 error` | `check_docs` 0 problems; `vhco docs check` 0 errors (index-page naming warnings only) — re-run after the component names in 13-real-world-apis were corrected |
| U-15 | UQ-14 → R15 | UDP unicast, bind, per-peer reply authorization | [08-udp](08-udp/README.md) steps 7–13 | `{"state":"ready"}`; reply only to a granted peer; `udp.truncated` 5 / HTTP 502 | Recorded; TEST-2026-0012 |
| U-16 | UQ-14 → R16 | OAuth 2.0 client credentials, credential store, typed failures | [07-oauth2](07-oauth2/README.md) steps 4–7 | Contacts returned; one token reused; `auth.client_secret_missing`, `auth.token_endpoint_failed`, `timeout.auth_token` | Recorded (client_credentials only); PKCE/device in TEST-2026-0011 |
| U-17 | UQ-14 → R17 | QUIC v1 streams with ALPN and TLS trust | [09-quic](09-quic/README.md) steps 3–6 | `{"state":"ready"}`; `timeout` 6; `quic.tls` 5 | Recorded; TEST-2026-0013 |
| U-18 | UQ-14 → R18 | HTTP/3 with no silent fallback | [09-quic](09-quic/README.md) steps 4, 6 | `{"items":[],"version":3}`; `tls.handshake` with `request_sent:false` | Recorded; TEST-2026-0014 |
| U-19 | UQ-15 → R19 | gRPC unary, server, client and bidi streaming | [10-grpc](10-grpc/README.md) steps 4–5 | Ada; two changes; `{count:2}`; echo; `grpc.not_found` 4, `grpc.unavailable` 5 | Recorded; TEST-2026-0017 |
| U-20 | UQ-15 → R20 | Many described operations per file; private helpers hidden | [01-catalog](01-catalog/README.md) step 1; [05-dag](05-dag/README.md) steps 1, 4 | Four IDs listed; helper `not_found.operation` 4 | Recorded; TEST-2026-0016 |
| U-21 | UQ-15 → R21 | Incoming MCP: direct named tools plus `rivet.*` built-ins | [01-catalog](01-catalog/README.md) step 8; [06-mcp-bridge](06-mcp-bridge/README.md) step 8 | `tools/list` then `tools/call demo.add` → 5; `isError` mapping | Recorded; TEST-2026-0018 |
| U-22 | UQ-15/08/17 → R22 | Live streams and live input on every surface | [01-catalog](01-catalog/README.md) steps 5–7; [10-grpc](10-grpc/README.md) steps 4, 6–8 | Same items over SSE, polling, WS, MCP sessions | Recorded; TEST-2026-0019 |
| U-23 | UQ-17 → R23 | Declared, described outputs validated at return | `rivet outputs --all --json` in any folder; [03-http](03-http/README.md) step 5 | Schemas per operation; `output.invalid` for an extra field | Recorded (see Known Caveats for `emits` descriptions); TEST-2026-0020 |
| U-24 | UQ-17 → R24 | Policy only from policy.json or `--policy PATH`; strict schema v1; `access` narrowing | [11-sandbox](11-sandbox/README.md) steps 2, 7; [02-file-crud](02-file-crud/README.md) step 7 | Results per policy file; `policy.invalid` exit 2 | Recorded; TEST-2026-0021 |
| U-25 | UQ-17 → R25 | One `serve` for REST, SSE, polling, WS and MCP with one auth model | [01-catalog](01-catalog/README.md) steps 3–9 | 200 / 401 / 403 / 404 as shown; `serve.auth_required` 2 | Recorded; TEST-2026-0022 |
| U-26 | UQ-18 → R26 | Generated I/O manifest, `--needs`, `--check-files`, `policy generate` | [11-sandbox](11-sandbox/README.md) steps 4–7 | Tables; exits 3 / 0 / 3; draft with 3 or 2 grants | Recorded; TEST-2026-0025, TEST-2026-0026 |

### Command / Request

The quickest end-to-end pass touches every surface with one bundle, then one protocol folder per family:

```text
  1. 01-catalog  ── CLI ─ REST ─ SSE ─ polling ─ WebSocket ─ MCP ─ bearer auth     (U-08, 20–25)
  2. 12-library  ── the same catalog from Rust                                    (U-01, 08, 09)
  3. 11-sandbox  ── policy files, manifest, needs, generate, trace                (U-06, 11, 24, 26)
  4. 02 · 03 · 04 · 05                                                            (U-03–05, 09, 10)
  5. 06 · 07 · 08 · 09 · 10  ── MCP, OAuth, UDP, QUIC/H3, gRPC fixtures           (U-07, 13, 15–19)
  6. docs checks                                                                  (U-14)
```

The per-surface examples below are the minimum a verifier should see pass **and** fail.

### Expected Output / Response

#### CLI

```sh
cd docs/demos/01-catalog
rivet --file app.rivet request demo.add --params '{"a":2,"b":3}'
rivet --file app.rivet request demo.add --params '{"b":3}'
```

```json
{"request_id":"req_0171ee835d","trace_id":"tr_0171ee835d","result":5,"data_count":0,"effects":"none"}
{"request_id":"req_016fb2d76d","trace_id":"tr_016fb2d76d","error":{"kind":"validation","code":"validation.required","message":"missing required parameter `a`","retryable":false,"effects":"none","operation_id":"demo.add","details":{"field":"a"}}}
```

Exit 0, then exit 2. A malformed bundle fails before anything runs (exit 2):

```text
error[syntax.e0001]: expected `+=`, `.`, or `=`, found "1" in `assign_map`
  --> bad.rivet:5:5
   |
  5|     return 1 +
   |     ^^^^^^^^^^
```

A Stage C statement is refused at run time (exit 5):

```json
{"request_id":"req_010740f765","trace_id":"tr_010740f765","error":{"kind":"unsupported","code":"unsupported.stage_c","message":"`with file watch` is Stage C and not available in this build (see rivet.capabilities)","retryable":false,"effects":"none","source":{"file":"watch.rivet","line":5,"column":5,"end_line":7,"end_column":8},"operation_id":"w.watch"}}
```

#### HTTP and SSE

```sh
rivet --file app.rivet serve --listen 127.0.0.1:18800 2>serve.err & SV=$!
curl -sS -w ' %{http_code}\n' http://127.0.0.1:18800/v1/request -H 'Content-Type: application/json' --data-binary @requests/add.http.json
curl -sS -w ' %{http_code}\n' http://127.0.0.1:18800/v1/request -H 'Content-Type: application/json' -d '{"id":"demo.nope","params":{}}'
curl -sSN http://127.0.0.1:18800/v1/request -H 'Content-Type: application/json' -H 'Accept: text/event-stream' --data-binary @requests/countdown.http.json
```

```text
{"request_id":"req_014a7c081d","trace_id":"tr_014a7c081d","result":5,"data_count":0,"effects":"none"} 200
{"request_id":"req_034ac7b08f","trace_id":"tr_034ac7b08f","error":{"kind":"not_found","code":"not_found.operation","message":"no operation `demo.nope`","retryable":false,"effects":"none","operation_id":"demo.nope"}} 404
```

SSE ends with exactly one terminal event:

```text
id: 3
event: data
data: {"request_id":"req_054565ae61","trace_id":"tr_054565ae61","seq":3,"type":"data","data":1}

id: 4
event: result
data: {"request_id":"req_054565ae61","trace_id":"tr_054565ae61","result":{"count":3},"data_count":3,"effects":"none","type":"result","seq":4}
```

A streaming operation posted as plain JSON is refused with HTTP 422 `stream.required` ([04-streaming](04-streaming/README.md) step 5).

#### Polling

```sh
curl -sS -w ' %{http_code}\n' -X POST http://127.0.0.1:18800/v1/requests -H 'Content-Type: application/json' --data-binary @requests/countdown.http.json
curl -sS "http://127.0.0.1:18800/v1/requests/SESSION_ID/events?after_seq=0&wait_ms=1000"
```

The receipt returns `202`; the batch ends with `"last_seq":4,"terminal":true`. Failure: an unknown session ID returns `{"request_id":"","trace_id":"","error":{"kind":"not_found","code":"not_found.session","message":"no session `ses_nope`","retryable":false,"effects":"none"}}` with `404`; cancelling an open live session yields one `cancelled.session` error event ([10-grpc](10-grpc/README.md) step 6).

#### WebSocket

```sh
"$TMPDIR/rivet-verify-venv/bin/python" fixtures/ws_client.py ws://127.0.0.1:18800/v1/ws < requests/ws-frames.jsonl
```

```json
{"type":"result","ref":"c1","completion":{"request_id":"req_087220b2f0","trace_id":"tr_087220b2f0","result":5,"data_count":0,"effects":"none"}}
{"type":"error","ref":"c3","error":{"kind":"validation","code":"validation.required","message":"missing required parameter `a`","retryable":false,"effects":"none","operation_id":"demo.add","details":{"field":"a"}}}
```

Ref `c2` streams three `data` frames and one `result`. Failure on the transport: under [policies/team.json](01-catalog/policies/team.json) the `ws` surface is not mounted and `GET /v1/ws` returns `404`.

#### MCP

```sh
curl -si http://127.0.0.1:18800/mcp -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' --data-binary @requests/initialize.mcp.json
```

Then, with the returned `mcp-session-id`, `tools/call demo.add` returns `"structuredContent":{…,"result":5,…},"isError":false` ([01-catalog](01-catalog/README.md) step 8). Failure: a remote tool error surfaces as `"isError":true` with `mcp.tool_failed` in `structuredContent` ([06-mcp-bridge](06-mcp-bridge/README.md) step 8).

#### Authentication across surfaces

```sh
kill $SV; rivet --file app.rivet --policy ./policies/team.json serve --listen 127.0.0.1:18800 2>serve.err & SV=$!
curl -sS -w ' %{http_code}\n' http://127.0.0.1:18800/v1/request -H 'Authorization: Bearer dev-token-ci' -H 'Content-Type: application/json' --data-binary @requests/add.http.json
curl -sS -w ' %{http_code}\n' http://127.0.0.1:18800/v1/request -H 'Content-Type: application/json' --data-binary @requests/add.http.json
kill $SV; rm -f serve.err
```

```text
{"request_id":"req_0233746362","trace_id":"tr_0233746362","error":{"kind":"permission","code":"permission.denied","message":"principal `ci` may not call `demo.add`","retryable":false,"effects":"none"}} 403
{"request_id":"","trace_id":"","error":{"kind":"auth","code":"auth.required","message":"missing bearer token","retryable":false,"effects":"none"}} 401
```

`serve --listen 0.0.0.0:18800` with auth `none` refuses to start with `serve.auth_required` (exit 2); `serve.auth` type `mtls` refuses with `unsupported.serve_mtls` (exit 5).

#### Library

```text
  rt.request("demo.add", {a:2,b:3}, None)   ──▶  Completion.result == Value::Int(5)
  rt.scope(|s| s.stream("events.count"))    ──▶  Data 1, Data 2, Data 3, Result(data_count 3), None
  rt.outputs(Some("demo.add"), false)       ──▶  {"id":"demo.add","output":{"type":"integer",…}}
```

Success: [12-library](12-library/README.md) step 2 prints the outputs JSON, an empty manifest and `{"version":1,"grants":[],"network":{"deny_private_ranges":true}}`, exit 0. Failure: `Policy::from_json(br#"{"version":1,"grantz":[]}"#)` returns an error before `build()` with code `policy.invalid`, `exit_code()` 2 and message ``policy.json /grantz: unknown key `grantz` (allowed: version, grants, deny, network, limits, serve, approved)``, the same error the CLI prints ([11-sandbox](11-sandbox/README.md) step 7).

## Cleanup

```sh
kill $SV 2>/dev/null                       # any serve you started
rm -rf "$TMPDIR/rivet-verify-venv"         # optional
rm -f docs/demos/10-grpc/schemas/users.pb  # generated by 10-grpc Setup
```

Each folder README ends with its own Cleanup; scratch copies (`$WORK`) are removed there. After a full pass, `git status` shows no changes under `docs/demos/`.

## Verification Record

| Step | Verified By | Verified At | Result |
|---|---|---|---|
| Build: `cargo build`, `cargo build --release` | Claude (TASK-068) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| Test suite: `cargo test --release` (397 passed, 0 failed, 32 suites) | Claude (TASK-068) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| U-01, U-08, U-09 (library) | Claude (TASK-068) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| U-02, U-12 (syntax diagnostic, `unsupported.stage_c`, `rivet.capabilities`) | Claude (TASK-068) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| U-03 … U-07, U-10, U-15 … U-26 through the twelve folders | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS (see each folder's record) |
| U-11 deny-by-default and host ceiling | Claude (TASK-068) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS on macOS (Seatbelt); Linux Landlock/seccomp NOT APPLICABLE on this host |
| U-13 argv-only process, token never logged | Claude (TASK-068) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| U-14 `check_docs.py` 0 problems; `vhco docs check .` | Claude (coordinator) | 2026-09-28, after commit 2a751ab, macOS 26.4.1 arm64 | PASS (0 errors) |
| Per-surface success and failure examples above | Claude (TASK-068) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| Tagged release build (`v0.1.0`, local `target/release/rivet`): `--version`, quickstart, 11-sandbox check and io | Claude (P5) | 2026-09-28, tag v0.1.0 (f69b5b9), macOS 26.4.1 arm64 | PASS; a published artifact does not exist yet (TASK-080/081 blocked: no git remote) |

## Known Caveats

- **Tag and commit.** The steps were recorded on the release candidate 829ca43 (`--version` 0.1.0-dev). The tagged build `v0.1.0` (f69b5b9) prints `rivet 0.1.0` and passed the smoke re-run in the last verification row.
- **Document ID.** The plan reserved DEMO-2026-0014 for this guide; that ID is used by the [13-real-world-apis](13-real-world-apis/README.md) cookbook, so this guide is DEMO-2026-0015.
- **Platform coverage.** Only macOS arm64 was available. The Linux Landlock + seccomp process sandbox is built but gated and refuses until verified on kernel ≥ 6.12; Windows refuses with `unsupported.sandbox_backend`. Those rows need a Linux host.
- **Defects found here.** Six defects found while running the demos were fixed before release ([INC-2026-0007](../incidents/resolved/inc-2026-0007-demo-verification-defects.md)); the affected READMEs (02, 03, 04, 05, 10) were re-run against commit 2a751ab.
- **Known 0.1.0 limitations** (from the plan): no mTLS serve, no `finally`, no Alt-Svc discovery, no connection pooling, in-memory trace store only, no MCP resource templates or legacy HTTP+SSE, `--timeout` not applied on the WebSocket duplex path.

## Related Documents

- [Sample folders (DEMO-2026-0013)](README.md) · [demos index](index.md)
- [Implementation plan PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) · [Validation report RPT-2026-0001](../reports/rpt-2026-0001-validation-of-plan-2026-0001.md)
- [Rivet manual](../manuals/man-2026-0001-rivet-manual.md) · [Error registry](../api/api-2026-0005-error-registry.md) · [Testing index](../testing/index.md)
- [Approved proposal](../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 2 | 2026-09-28 | Claude | P5: tag v0.1.0 and commit f69b5b9 recorded; tagged-build smoke verification row. |
| 1 | 2026-09-28 | Claude | TASK-068: created the 0.1.0 release verification guide: header with tag/commit placeholders, U-01…U-26 (one per R1…R26) with inciting UQ, command, expected result and recorded evidence, per-surface success and failure examples, cleanup and verification record at release candidate 829ca43. |
