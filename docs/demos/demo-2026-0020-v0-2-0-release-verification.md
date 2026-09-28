---
document_id: DEMO-2026-0020
title: "Rivet v0.2.0 release verification guide"
document_type: demo
status: active
created_date: 2026-09-29
last_updated: 2026-09-29
document_revision: 1
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, registry, execution, files, connectors, audit, policy, auth, datagrams, quic, grpc, sessions, serve, transports, cli, http, ws, poll, mcp, library, ffi, highlight_source]
affected_versions:
  from: "0.2.0"
  to: null
applicable_environments: [development, embedded, server]
audience: [maintainers, reviewers, release-verifiers]
scope: Executable verification procedure for every update released in Rivet 0.2.0 (U-01…U-24, one per requirement R1…R24 of PLAN-2026-0002), with success and failure examples per surface (CLI, HTTP/SSE, polling, WebSocket, MCP, Rust library, C ABI, Python, editor), platform coverage, cleanup and the verification record. It links the seventeen sample folders instead of repeating them.
reason: DOCUMENTATION.md §29 requires a release verification guide and demo for every release; PLAN-2026-0002 TASK-077 (D-64, T-30).
release_version: "0.2.0"
release_tag: v0.2.0
release_commit: TBD at P5
dependencies: [PLAN-2026-0002, PROP-2026-0002]
related_documents: [PLAN-2026-0002, PROP-2026-0002, DEMO-2026-0015, DEMO-2026-0001, DEMO-2026-0002, DEMO-2026-0003, DEMO-2026-0004, DEMO-2026-0005, DEMO-2026-0006, DEMO-2026-0007, DEMO-2026-0008, DEMO-2026-0009, DEMO-2026-0010, DEMO-2026-0011, DEMO-2026-0012, DEMO-2026-0013, DEMO-2026-0014, DEMO-2026-0016, DEMO-2026-0017, DEMO-2026-0018, DEMO-2026-0019, API-2026-0005, API-2026-0006, INC-2026-0011]
supersedes: null
superseded_by: null
tags: [rivet, release, verification, demo, v0.2.0]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-29
verified_against: "0.2.0"
---

# Rivet v0.2.0 release verification guide

> **Status:** Active
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0 and later
> **Owner:** Project maintainer
> **Affected Components:** every Rivet feature and surface (language through library), plus the C ABI (`librivet`), modules and the editor grammar

```text
  ┌──────────────────────────────────────────────────────────────────────────┐
  │  RELEASE      Rivet 0.2.0                                                │
  │  TAG          v0.2.0 (annotated)                    TBD at P5            │
  │  COMMIT       TBD at P5                                                  │
  │  VERIFIED ON  0.2.0-dev release candidate: source 8031baa,               │
  │               re-run at 166a98b (documentation-only commits after it)    │
  │  PLAN         PLAN-2026-0002   PROPOSAL  PROP-2026-0002                  │
  │  PLATFORMS    macOS arm64 (here) · Linux (CI run 36483001760)            │
  │               Windows: unsupported in 0.2.0 (INC-2026-0011)              │
  └──────────────────────────────────────────────────────────────────────────┘
```

## Purpose

A reader who did not implement Rivet 0.2.0 can use this page to decide, update by update, whether the build does what the approved proposal promised. Each `U-NN` row names the user question (UQ) that incited the requirement, the exact command (in one of the seventeen [sample folders](README.md)), the expected result and the evidence recorded for this release candidate.

```text
   UQ-01…UQ-09 (PROP-2026-0002) ──▶ R1…R24 (PLAN-2026-0002) ──▶ U-01…U-24 (this page) ──▶ command ──▶ PASS / FAIL
                                                                     │
                                  evidence: demo step (docs/demos/NN-*) + tests/conformance_*.rs (T-NN)
```

What 0.2.0 adds, and where each theme is verified:

```text
  ┌──────────────── envelopes (R1–R6) ────────────────┐  ┌──── globals (R7–R9) ────┐  ┌── package (R10–R12) ──┐
  │ one ResponseEnvelope on every surface             │  │ global NAME = EXPR      │  │ rivet-runtime + facade │
  │ one InputEnvelope {operation, data}; --data       │  │ check.global_* codes    │  │ Cargo features         │
  │ id/params aliases (deprecated) · --pretty         │  │ exact manifest targets  │  │ git dep; crates.io G-PUB│
  │ 01 · 02–13 re-verified                            │  │ 14-globals              │  │ 12-library             │
  └───────────────────────────────────────────────────┘  └─────────────────────────┘  └────────────────────────┘
  ┌──── C ABI (R13–R15) ────┐  ┌── highlighting (R16–R17) ──┐  ┌─ docs (R18) ─┐  ┌──── modules (R19–R24) ────┐
  │ librivet .dylib/.so/.a  │  │ TextMate grammar · .vsix   │  │ check_docs   │  │ import "P" as A [public]  │
  │ misuse → envelopes      │  │ rivet highlight ansi/html/ │  │ vhco docs    │  │ import errors · rt.load   │
  │ pull call handle        │  │ json                       │  │ check        │  │ rivet_load · one policy   │
  │ 15-ffi                  │  │ 16-editor                  │  │              │  │ 17-modules · 15-ffi       │
  └─────────────────────────┘  └────────────────────────────┘  └──────────────┘  └───────────────────────────┘
```

## Verified Against Version

0.2.0. Recorded on the 0.2.0-dev release candidate. Demos 01–15 were executed at commit `8031baa` (15-ffi steps 3–5 at `166a98b`), and demos 16, 17 and the per-surface examples below at `166a98b`, whose source is identical to `8031baa` (`git diff 8031baa 166a98b -- src ffi examples editors tests Cargo.toml` is empty). The version string is bumped from 0.1.0 to 0.2.0 at release (P5), so `rivet --version`, `rivet.capabilities`, MCP `serverInfo.version` and `rivet_version()` still print `0.1.0`, and the `.vsix` is `rivet-0.1.0.vsix`. Rerun this guide against the tagged artifact at P5 (TASK-096) and add a row to the Verification Record. Host: macOS 26.4.1 (Darwin 25.4.0, arm64), 2026-09-29, `target/release/rivet`. Request, trace and session IDs, timestamps, hashes and ports vary between runs.

## Prerequisites

```sh
cargo build --release --workspace --all-features   # from the repository root: rivet + librivet
export PATH="$PWD/target/release:$PATH"             # put this build first on PATH
rivet --version                                      # rivet 0.1.0 until the P5 bump, then rivet 0.2.0
```

| Needed for | Tool |
|---|---|
| every step | `curl`, `python3` (stdlib fixtures: 03, 06, 07, 08, 14, 17) |
| WebSocket, QUIC/HTTP3, gRPC fixtures | a venv with `websockets aioquic grpcio protobuf` |
| 09-quic certificates, 10-grpc descriptor | `openssl`, `protoc` |
| library and modules steps (12, 17) | a Rust toolchain (edition 2024, rust ≥ 1.90) |
| C ABI (15) | a C compiler (`cc`), `make` |
| editor (16) | the VS Code `code` launcher (optional; install into a scratch `--extensions-dir`) |

Fixtures and servers use loopback ports 18800–18899 only. Nothing contacts the internet except the optional [13-real-world-apis](13-real-world-apis/README.md) cookbook.

## Setup

```sh
python3 -m venv "$TMPDIR/rivet-verify-venv"
"$TMPDIR/rivet-verify-venv/bin/pip" install websockets aioquic grpcio protobuf
```

Every folder is independent: `cd docs/demos/NN-topic` and follow its README. Folders that need a service run a shipped fixture in a scratch copy (`WORK="$(mktemp -d)"`), so nothing is written into the repository.

```text
   docs/demos/
   ├── 01-catalog      every surface from one serve        ├── 10-grpc      gRPC fixture, sessions
   ├── 02-file-crud    files                               ├── 11-sandbox   policy, manifest, generate
   ├── 03-http         HTTP fixture                        ├── 12-library   Rust embedding (facade)
   ├── 04-streaming    SSE, file handles, WS fixture       ├── 13-real-world-apis  cookbook (internet)
   ├── 05-dag          DAG                                 ├── 14-globals   NEW: global constants
   ├── 06-mcp-bridge   MCP fixture                         ├── 15-ffi       NEW: C ABI + Python
   ├── 07-oauth2       OAuth fixture                       ├── 16-editor    NEW: .vsix + rivet highlight
   ├── 08-udp          UDP fixture                         └── 17-modules   NEW: import, rt.load
   └── 09-quic         QUIC + HTTP/3 fixture
```

Folders 01–12 carry two Release Updates tables: the first lists the 0.2.0 updates (numbered as on this page); the second keeps the 0.1.0 rows, numbered as in [DEMO-2026-0015](demo-2026-0015-v0-1-0-release-verification.md), and re-run on 0.2.0. The 13-real-world-apis cookbook has its own verification record instead.

## Steps

### Release Updates

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-01 | UQ-03/05 → R1 | Every outcome on every surface is a `ResponseEnvelope`: `request_id`, `trace_id`, `operation`, `type`, `status`, `data`, `error`, `effects`, `data_count` (+ `seq` on stream records, `ref` on WS frames) | [01-catalog](01-catalog/README.md) steps 1, 4–8; [10-grpc](10-grpc/README.md) steps 4–8; [12-library](12-library/README.md) step 2; [15-ffi](15-ffi/README.md) step 2 | Same nine keys in the same order on CLI, REST, SSE, polling, WS, MCP `structuredContent`, library and C | Recorded (per-surface examples below, 166a98b; folders at 8031baa); `conformance_envelope` (T-01) |
| U-02 | UQ-05 → R2 | `status` ok / error / cancelled / accepted; `type` result / data; `effects` at the top level | [01-catalog](01-catalog/README.md) steps 1, 4, 6; [04-streaming](04-streaming/README.md) steps 6–7 | `status:error` + `data:null` on failures; `accepted` (202) for a polling open; `cancelled` on Ctrl-C (exit 130) | Recorded; `conformance_envelope` (T-01, T-04) |
| U-03 | UQ-03 → R3 | Built-ins, GET routes and every CLI `--json` output are envelopes whose `data` is the payload | [01-catalog](01-catalog/README.md) steps 1, 2, 4; [11-sandbox](11-sandbox/README.md) steps 3–5, 7, 8 | `rivet.describe`, `rivet.outputs`, `rivet.io`, `rivet.policy.*`, `rivet.trace.show`, `rivet.check` envelopes | Recorded; `conformance_envelope` (T-03) |
| U-04 | UQ-06 → R4 | One `InputEnvelope {operation, data, …}`; CLI `--data` and `--input FILE\|-` | [01-catalog](01-catalog/README.md) steps 1, 4, 6, 7; [12-library](12-library/README.md) step 2; [15-ffi](15-ffi/README.md) step 2 | The same body accepted by `--input -`, REST, polling, WS, `Runtime::call` and `rivet_request` | Recorded; `conformance_envelope` (T-02) |
| U-05 | UQ-06 (0.1.0 clients) → R5 | `id`/`params` and `--params` still accepted with a deprecation signal; mixing a key with its alias refused | [01-catalog](01-catalog/README.md) steps 1, 3, 4 | `warning[deprecated.params]`; `"deprecation":true`; `validation.input_envelope` 422 / exit 2 | Recorded; `conformance_envelope` (T-15) |
| U-06 | UQ-04 → R6 | `--pretty`, `?pretty=true`, `to_json_pretty()`, FFI `"pretty"`; refused with NDJSON and SSE | [01-catalog](01-catalog/README.md) steps 1, 4, 5; [12-library](12-library/README.md) step 2; SSE example below | Indented envelope; `validation.usage` exit 2 (`--pretty --stream`); `validation.pretty_stream` HTTP 400 | Recorded; `conformance_envelope` (T-05) |
| U-07 | UQ-07 → R7 | `global NAME = EXPR`: load-time, read-only constants shared by every operation of a file | [14-globals](14-globals/README.md) step 2 | `settings.show` returns the seven computed globals | Recorded; `conformance_globals` (T-06) |
| U-08 | UQ-07 → R8 | `syntax.global` and `check.global_not_constant`, `_forward_ref`, `_assign`, `_duplicate`, `_shadow` with spans | [14-globals](14-globals/README.md) step 6 | Six diagnostics, each exit 2 | Recorded; `conformance_globals` (T-07) |
| U-09 | UQ-07 → R9 | Globals substituted in the manifest and call graph; literal-and-global targets are `exact`; exact grants from `policy generate` | [14-globals](14-globals/README.md) steps 3–5 | `…/users/index.json?limit=50` `exact`; draft with 2 exact grants; one-line retarget | Recorded; `conformance_globals` (T-08). Deviation: a target with a param stays `param_dependent` (Known Caveats) |
| U-10 | UQ-02 → R10 | Package `rivet-runtime`, library `rivet`, public facade only (`rivet::{Runtime, Policy, Value, InputEnvelope, …}`, `rivet::types`) | [12-library](12-library/README.md) step 2 | A scratch crate with `package = "rivet-runtime"` compiles against the facade and exits 0 | Recorded; `examples/embed.rs`; `conformance_library` |
| U-11 | UQ-02 → R11 | Cargo features `serve`, `grpc`, `quic`, `oauth`, `cli`; a compiled-out adapter is `unsupported.feature` | [12-library](12-library/README.md) step 2; [01-catalog](01-catalog/README.md) step 1 (`rivet.capabilities`) | `build_features` `["serve","grpc","quic","oauth"]` (library) / plus `cli` (binary) | Success recorded. The failure half (`unsupported.feature`, exit 5, HTTP 501) is evidenced by `conformance_features` (T-10) only: no lean binary is built in the demos |
| U-12 | UQ-02 → R12 | Git dependency on tag `v0.2.0`; crates.io publication prepared and gated (G-PUB) | `cargo package --list -p rivet-runtime --allow-dirty`; after P5, `rivet-runtime = { git = "…/rivet", tag = "v0.2.0" }` | 177 files: `src/**`, `rivet.capy`, `editors/keywords.json`, the two examples; no `tests/`, `docs/`, `ffi/` | Package list recorded (166a98b). **Not verifiable yet**: the tag `v0.2.0` does not exist until P5; `cargo publish --dry-run` fails on the git `capy-core` dependency, as expected until G-PUB (PLAN-2026-0002 finding R12) |
| U-13 | UQ-08 → R13 | `librivet` as `cdylib` + `staticlib`, `include/rivet.h`, `rivet.pc`, `rivet_abi_version` | [15-ffi](15-ffi/README.md) Setup, step 1 | 18 `rivet_*` symbols; shared and static links run | Recorded; `conformance_ffi` (T-11); Linux in CI run 36483001760 |
| U-14 | UQ-08 → R14 | Null, UTF-8, JSON, panic and double-free guards; ownership and thread-safety rules | [15-ffi](15-ffi/README.md) step 5 | `validation.ffi_argument`, `validation.input_envelope`, `not_found.source` envelopes; second free returns 1 | Recorded; `conformance_ffi` (T-12) |
| U-15 | UQ-08 → R15 | Pull call handle: `rivet_call_start`, `_next`, `_send`, `_finish_input`, `_cancel`, `_free` | [15-ffi](15-ffi/README.md) step 2 | Items then one terminal record; echoed input; `cancelled` | Recorded; `conformance_ffi` (T-11) |
| U-16 | UQ-01 → R16 | Generated TextMate grammar; VS Code extension packaged as `.vsix` with Python only; drift check | [16-editor](16-editor/README.md) steps 1, 2, 5 | `rivet.rivet@0.1.0` installed in a scratch profile (0.2.0 after the bump); hand edit caught (exit 1) | Recorded (VS Code 1.108.1); `conformance_highlight` (T-13) |
| U-17 | UQ-01 → R17 | `rivet highlight` (ansi, html, json), `rivet::highlight::tokens`, `rivet_highlight` from parser spans | [16-editor](16-editor/README.md) steps 4, 6; [14-globals](14-globals/README.md) step 7 | 12 classes in 65 tokens; broken file: 11 tokens + `syntax.expression`, exit 2 | Recorded; `conformance_highlight` (T-14) |
| U-18 | DOCUMENTATION §§29–31 → R18 | Every document in the plan's Documentation and Demo Checklist; demos executed and re-verified | `python3 scripts/check_docs.py`; `vhco docs check .` | 0 problems in `docs/demos`; 0 errors | Recorded for `docs/demos` at this commit (Verification Record). The rest of the P4 checklist is owned by other tasks and closes at P4 exit |
| U-19 | UQ-09 → R19 | `import "PATH" as ALIAS [public]`, relative to the importing file, root-confined; bootstrap reads name every file | [17-modules](17-modules/README.md) steps 1, 4; [08-udp](08-udp/README.md) step 5; [11-sandbox](11-sandbox/README.md) step 4 | `ok: 7 operations`; bootstrap `./app.rivet`, `./users.rivet`, `./billing.rivet` | Recorded; `conformance_modules` (T-16) |
| U-20 | UQ-09 → R20 | `ALIAS.ID` namespaces; internal by default, `public` to expose; transitive; a file reached twice compiles once | [17-modules](17-modules/README.md) steps 1–3 | Four IDs on CLI, REST and MCP; `users.get` 404 / exit 4 | Recorded; `conformance_modules` (T-16) |
| U-21 | UQ-09 → R21 | `syntax.import`, `not_found.import`, `permission.import_outside_root`, `check.import_cycle`, `check.import_duplicate`, `check.import_collision`, `limit.imports` | [17-modules](17-modules/README.md) step 6 | Exits 2, 4, 3, 2, 2, 2 at the `import` line | Recorded except `limit.imports` (`conformance_modules`, T-17, only) |
| U-22 | UQ-09 → R22 | `Runtime::builder()` without an entry file; `rt.load` / `load_as` → `Module`; catalog snapshot swap | [17-modules](17-modules/README.md) step 7 (`cargo run --release --example modules`) | `users.get`, `billing.invoice`, `users.list` envelopes; duplicate alias refused | Recorded; `conformance_modules` (T-18) |
| U-23 | UQ-09 → R23 | `rivet_load`, `rivet_module_*`; Python wrapper module object | [15-ffi](15-ffi/README.md) steps 3–4 | Same envelopes from C and Python; `check.import_duplicate` on reload | Recorded (166a98b); `conformance_ffi` (T-19) |
| U-24 | UQ-09 → R24 | The loader's policy only; a module's `policy.json` ignored with a warning; `io`, `graph`, `policy explain`, `policy generate` cover modules | [17-modules](17-modules/README.md) steps 4–6 | `2 allowed` for sites in two modules; draft with 2 grants; `warning[check.module_policy_ignored]` | Recorded; `conformance_modules` (T-20) |

### Command / Request

The quickest end-to-end pass touches every surface with one bundle, then each new feature folder, then one protocol folder per family:

```text
  1. 01-catalog   ── CLI ─ REST ─ SSE ─ polling ─ WebSocket ─ MCP ─ bearer auth    (U-01…U-06)
  2. 12-library   ── the same catalog from Rust (facade, features, pretty)          (U-01, U-04, U-06, U-10, U-11)
  3. 14-globals   ── globals, exact manifest, global diagnostics                    (U-07…U-09, U-17)
  4. 15-ffi       ── C shared + static, streams, misuse, modules, Python            (U-13…U-15, U-23)
  5. 16-editor    ── .vsix, rivet highlight, grammar drift                          (U-16, U-17)
  6. 17-modules   ── import, internal/public, io/graph across files, import errors  (U-19…U-22, U-24)
  7. 02 … 11      ── files, HTTP, streaming, DAG, MCP, OAuth, UDP, QUIC, gRPC, sandbox on 0.2.0 envelopes
  8. docs checks + cargo package --list                                            (U-12, U-18)
```

The per-surface examples below are the minimum a verifier should see pass **and** fail. They were re-run for this page at `166a98b` against [01-catalog](01-catalog/README.md) (`serve --listen 127.0.0.1:18860`).

### Expected Output / Response

```text
  surface         success (this page)                         failure (this page)                       exit / HTTP
  ─────────────── ─────────────────────────────────────────── ───────────────────────────────────────── ────────────
  CLI             demo.add → data 5                           validation.required                       0 / 2
  HTTP            POST /v1/request → data 5                   not_found.operation                       200 / 404
  SSE             data 3,2,1 then one result {count:3}        ?pretty=true + SSE: validation.pretty_stream  200 / 400
  polling         202 accepted → batch terminal:true          not_found.session                         202 / 404
  WebSocket       c1 data 5; c2 items + result                c3 validation.required (per ref)          frames
  MCP             tools/call demo.add isError:false           isError:true + validation.required         200
  Rust library    12-library step 2; 17-modules step 7        validation.type; duplicate rt.load        exit 0
  C ABI           15-ffi steps 1–3                            15-ffi step 5 misuse envelopes            exit 0
  Python          15-ffi step 4                               validation.ffi_argument / module errors   exit 0
  editor          16-editor steps 1–4                         16-editor steps 5–6 (drift, syntax error) 0 / 1 / 2
```

#### CLI

```sh
cd docs/demos/01-catalog
rivet --file app.rivet request demo.add --data '{"a":2,"b":3}'
rivet --file app.rivet request demo.add --data '{"b":3}'
```

```json
{"request_id":"req_0118e18275","trace_id":"tr_0118e18275","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}
{"request_id":"req_01164c6be5","trace_id":"tr_01164c6be5","operation":"demo.add","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.required","message":"missing required parameter `a`","retryable":false,"operation_id":"demo.add","details":{"field":"a"}},"effects":"none","data_count":0}
```

Exit 0, then exit 2. New in 0.2.0 on the CLI: `--data`/`--input`/`--pretty` ([01-catalog](01-catalog/README.md) step 1), global diagnostics ([14-globals](14-globals/README.md) step 6), import errors ([17-modules](17-modules/README.md) step 6) and `rivet highlight` ([16-editor](16-editor/README.md) step 6). All of them exit 2, 3 or 4 before anything runs.

#### HTTP and SSE

```sh
rivet --file app.rivet serve --listen 127.0.0.1:18860 2>serve.err & SV=$!
curl -sS -w ' %{http_code}\n' http://127.0.0.1:18860/v1/request -H 'Content-Type: application/json' --data-binary @requests/add.http.json
curl -sS -w ' %{http_code}\n' http://127.0.0.1:18860/v1/request -H 'Content-Type: application/json' -d '{"operation":"demo.nope","data":{}}'
curl -sSN http://127.0.0.1:18860/v1/request -H 'Content-Type: application/json' -H 'Accept: text/event-stream' --data-binary @requests/countdown.http.json | tail -8
curl -sS -w ' %{http_code}\n' 'http://127.0.0.1:18860/v1/request?pretty=true' -H 'Content-Type: application/json' -H 'Accept: text/event-stream' --data-binary @requests/countdown.http.json
```

```text
{"request_id":"req_01d66847d5","trace_id":"tr_01d66847d5","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0} 200
{"request_id":"req_025403763a","trace_id":"tr_025403763a","operation":"demo.nope","type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"not_found.operation","message":"no operation `demo.nope`","retryable":false,"operation_id":"demo.nope"},"effects":"none","data_count":0} 404
```

SSE ends with exactly one terminal event, and every event is a record with the same keys:

```text
id: 3
event: data
data: {"request_id":"req_03d1f8fca7","trace_id":"tr_03d1f8fca7","operation":"demo.countdown","type":"data","seq":3,"data":1,"error":null}

id: 4
event: result
data: {"request_id":"req_03d1f8fca7","trace_id":"tr_03d1f8fca7","operation":"demo.countdown","type":"result","seq":4,"status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}
```

Pretty output cannot be combined with an event stream (HTTP 400; the error itself is pretty-printed):

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

#### Polling

```sh
curl -sS -w ' %{http_code}\n' -X POST http://127.0.0.1:18860/v1/requests -H 'Content-Type: application/json' --data-binary @requests/countdown.http.json
curl -sS "http://127.0.0.1:18860/v1/requests/SESSION_ID/events?after_seq=0&wait_ms=1000"
curl -sS -w ' %{http_code}\n' "http://127.0.0.1:18860/v1/requests/ses_nope/events?after_seq=0"
```

The open is an envelope with `status` `accepted` and the receipt in `data` (HTTP 202); the batch holds records and ends with `"last_seq":4,"terminal":true`:

```text
{"request_id":"req_0452ad1c6c","trace_id":"tr_0452ad1c6c","operation":"demo.countdown","type":"result","status":"accepted","data":{"session_id":"ses_01d0ce881d","request_id":"req_0452ad1c6c","trace_id":"tr_0452ad1c6c","catalog_version":"sha256:67104f0e…9730","input_schema":null,"emits_schema":{"type":"integer"},"next_send_seq":1,"expires_at":"2026-09-28T23:13:27Z","events_url":"/v1/requests/ses_01d0ce881d/events"},"error":null,"effects":"none","data_count":0} 202
{"session_id":"ses_01d0ce881d","events":[{…"type":"data","seq":1,"data":3,"error":null},{…"type":"data","seq":2,"data":2,"error":null},{…"type":"data","seq":3,"data":1,"error":null},{"request_id":"req_0452ad1c6c","trace_id":"tr_0452ad1c6c","operation":"demo.countdown","type":"result","seq":4,"status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}],"last_seq":4,"terminal":true}
{"request_id":"","trace_id":"","operation":"rivet.sessions.read","type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"not_found.session","message":"no session `ses_nope`","retryable":false},"effects":"none","data_count":0} 404
```

Cancelling an open live session yields one `cancelled.session` record with `status` `cancelled` ([10-grpc](10-grpc/README.md) step 6).

#### WebSocket

```sh
"$TMPDIR/rivet-verify-venv/bin/python" fixtures/ws_client.py ws://127.0.0.1:18860/v1/ws < requests/ws-frames.jsonl
```

Every frame is the envelope plus `ref`. The invalid `c3` fails on its own ref while `c1` and `c2` succeed:

```json
{"ref":"c3","request_id":"","trace_id":"","operation":"demo.add","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.required","message":"missing required parameter `a`","retryable":false,"operation_id":"demo.add","details":{"field":"a"}},"effects":"none","data_count":0}
{"ref":"c1","request_id":"req_0584dcb419","trace_id":"tr_0584dcb419","operation":"demo.add","type":"result","seq":1,"status":"ok","data":5,"error":null,"effects":"none","data_count":0}
{"ref":"c2","request_id":"req_06030c4826","trace_id":"tr_06030c4826","operation":"demo.countdown","type":"data","seq":1,"data":3,"error":null}
{"ref":"c2","request_id":"req_06030c4826","trace_id":"tr_06030c4826","operation":"demo.countdown","type":"data","seq":2,"data":2,"error":null}
{"ref":"c2","request_id":"req_06030c4826","trace_id":"tr_06030c4826","operation":"demo.countdown","type":"data","seq":3,"data":1,"error":null}
{"ref":"c2","request_id":"req_06030c4826","trace_id":"tr_06030c4826","operation":"demo.countdown","type":"result","seq":4,"status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}
```

Failure on the transport: under [policies/team.json](01-catalog/policies/team.json) the `ws` surface is not mounted and `GET /v1/ws` returns 404 ([01-catalog](01-catalog/README.md) step 9).

#### MCP

```sh
H=$(curl -si http://127.0.0.1:18860/mcp -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' \
      --data-binary @requests/initialize.mcp.json | grep -i mcp-session-id | tr -d '\r' | cut -d' ' -f2)
for body in initialized add; do
  curl -sS http://127.0.0.1:18860/mcp -H "MCP-Session-Id: $H" -H 'MCP-Protocol-Version: 2025-11-25' \
    -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' --data-binary @requests/$body.mcp.json; echo
done
curl -sS http://127.0.0.1:18860/mcp -H "MCP-Session-Id: $H" -H 'MCP-Protocol-Version: 2025-11-25' \
  -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' \
  -d '{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"demo.add","arguments":{"b":3}}}'
kill $SV; rm -f serve.err
```

`notifications/initialized` returns 202 with an empty body. `structuredContent` is the ResponseEnvelope, and `isError` follows its `status`:

```json
{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"{\"request_id\":\"req_0785f5017b\",…,\"data\":5,…}"}],"structuredContent":{"request_id":"req_0785f5017b","trace_id":"tr_0785f5017b","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0},"isError":false}}
{"jsonrpc":"2.0","id":9,"result":{"content":[{"type":"text","text":"{…\"status\":\"error\",…}"}],"structuredContent":{"request_id":"req_08faf6f068","trace_id":"tr_08faf6f068","operation":"demo.add","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.required","message":"missing required parameter `a`","retryable":false,"operation_id":"demo.add","details":{"field":"a"}},"effects":"none","data_count":0},"isError":true}}
```

Modules on MCP: only public imports are tools ([17-modules](17-modules/README.md) step 3). A remote tool error surfaces as `isError: true` with `mcp.tool_failed` ([06-mcp-bridge](06-mcp-bridge/README.md) step 8).

#### Rust library

```text
  rt.call(InputEnvelope::new("demo.add").data({a:2,b:3}))  ──▶ ResponseEnvelope, same keys as the CLI; to_json_pretty()
  rt.scope(|s| s.stream(...)) · env.record()               ──▶ data records then one result record
  rivet::build_features()                                  ──▶ ["serve","grpc","quic","oauth"]
  rt.load("./users.rivet") / load_as(path, alias)          ──▶ Module; module.call("get", {id:42}) ──▶ "users.get"
```

Success: [12-library](12-library/README.md) step 2 (scratch crate against the `rivet-runtime` facade, exit 0) and [17-modules](17-modules/README.md) step 7 (`cargo run --release --example modules`). Failure: `demo.add` with `{"a":"two"}` returns the same `validation.type` error envelope as the CLI ([12-library](12-library/README.md) step 2); a second `rt.load` of the same alias returns `check.import_duplicate (syntax): a module named `users` is already loaded; use load_as(path, alias)` and leaves the catalog unchanged.

#### C ABI

```text
  rivet_runtime_new(json) ─▶ rivet_request(rt, input_json) ─▶ char* envelope ─▶ rivet_string_free
                          ─▶ rivet_call_start ─▶ rivet_call_next(timeout) … NULL ─▶ rivet_call_free
                          ─▶ rivet_load(rt, path, alias) ─▶ RivetModule* ─▶ rivet_module_call ─▶ rivet_module_free
```

Success: [15-ffi](15-ffi/README.md) steps 1–3 (shared and static links, request, stream, live input, cancel, modules). Failure: step 5, where a NULL handle, invalid UTF-8, malformed JSON and a missing bundle each return an error envelope (`validation.ffi_argument`, `validation.input_envelope`, `not_found.source`) and a second free returns `RIVET_ERROR` (1). Nothing crashes.

#### Python

Success: [15-ffi](15-ffi/README.md) step 4 (`examples/python/demo.py` and `modules.py` over ctypes: request `data` 5, stream `[1, 2, 3, {'count': 3}]`, module attributes `users.get`, `billing.invoice`). Failure: `options-error validation.ffi_argument`, `invalid error validation.type`, `duplicate check.import_duplicate`, `no-such-operation module 'users' has no operation 'missing'`.

#### Editor

Success: [16-editor](16-editor/README.md) steps 1–4 (`.vsix` built with Python only, installed into a scratch `--extensions-dir`, 12 token classes in ansi, html and json). Failure: step 5 (a hand edit to a generated grammar fails `gen_grammar.py --check` and `check_keywords.py`, exit 1) and step 6 (a syntax error prints the tokens before it and `syntax.expression`, exit 2).

### Platform coverage

```text
  platform          build + tests                         demos (this page)          status in 0.2.0
  ───────────────── ───────────────────────────────────── ────────────────────────── ─────────────────────────
  macOS arm64       cargo test --release --workspace      01–17 executed here        supported, verified here
                    --all-features: 475 passed, 1 failed*
  Linux x86_64      CI run 36483001760 (ubuntu-latest,    not executed here          supported, verified in CI
                    feature matrix, conformance_ffi)
  Windows           dropped from CI                       —                          unsupported (INC-2026-0011)
  * conformance_samples, caused by the new 17-modules app.rivet (Known Caveats)
```

- **macOS** (26.4.1, arm64): every demo and every example on this page, Seatbelt process sandbox, `librivet.dylib` and `librivet.a`, VS Code 1.108.1.
- **Linux**: verified by CI run 36483001760 at commit 8045343 (green on `ubuntu-latest` and `macos-latest`, the feature matrix and `cargo deny`), which includes `conformance_ffi` (shared and static `librivet.so`/`.a`). The Landlock + seccomp process sandbox stays gated as in 0.1.0. The demos were not executed on a Linux host for this page.
- **Windows**: not supported in 0.2.0. The maintainer dropped Windows from CI and from the supported platforms on 2026-09-29; the known failures W-01…W-13 are catalogued in [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md) for a future port.

## Cleanup

```sh
kill $SV $HS 2>/dev/null                     # any serve or http.server you started
rm -rf "$TMPDIR/rivet-verify-venv"           # optional
rm -f docs/demos/10-grpc/schemas/users.pb    # generated by 10-grpc Setup
make -C examples/c clean                     # 15-ffi
rm -f dist/rivet-0.1.0.vsix                  # 16-editor (dist/ is git-ignored); rivet-0.2.0.vsix after P5
```

Each folder README ends with its own Cleanup; scratch copies (`$WORK`) and scratch VS Code profiles are removed there. After a full pass, `git status` shows no changes under `docs/demos/`.

## Verification Record

| Step | Verified By | Verified At | Result |
|---|---|---|---|
| Build: `cargo build --release --workspace --all-features` | Claude (TASK-077) | 2026-09-29, commit 166a98b (source = 8031baa), macOS 26.4.1 arm64 | PASS |
| Test suite: `cargo test --release --workspace --all-features --no-fail-fast` (37 targets: 475 passed, 1 failed) | Claude (TASK-077) | 2026-09-29, commit 166a98b, macOS 26.4.1 arm64 | PASS except `conformance_samples::every_demo_bundle_compiles`, which fails on the new 17-modules app.rivet (test compiles demos without resolving imports; Known Caveats) |
| `conformance_highlight` (8 passed) | Claude (TASK-077) | 2026-09-29, commit 166a98b, macOS 26.4.1 arm64 | PASS |
| U-01…U-06 envelopes: per-surface examples above (CLI, HTTP, SSE, polling, WS, MCP) | Claude (TASK-077) | 2026-09-29, commit 166a98b, macOS 26.4.1 arm64 | PASS |
| U-01…U-06 through folders 01–13 (re-verified) | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS (see each folder's record) |
| U-07…U-09 globals (14-globals) | Claude (TASK-075) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| U-10, U-11 facade and features (12-library; `rivet.capabilities`) | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS; `unsupported.feature` by `conformance_features` only |
| U-12 `cargo package --list -p rivet-runtime` (177 files, no tests/docs/ffi) | Claude (TASK-077) | 2026-09-29, commit 166a98b, macOS 26.4.1 arm64 | PASS for packaging; git-tag dependency NOT VERIFIABLE until P5; crates.io DEFERRED (G-PUB) |
| U-13…U-15 C ABI (15-ffi steps 1–2) | Claude (TASK-075) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| U-14, U-23 misuse, C and Python modules (15-ffi steps 3–5) | Claude (coordinator) | 2026-09-29, commit 166a98b, macOS 26.4.1 arm64 | PASS |
| U-16, U-17 editor and highlighting (16-editor) | Claude (TASK-075) | 2026-09-29, commit 166a98b, macOS 26.4.1 arm64, VS Code 1.108.1 | PASS |
| U-18 `python3 scripts/check_docs.py`; `vhco docs check .` | Claude (TASK-077) | 2026-09-29, commit 166a98b, macOS 26.4.1 arm64 | PASS: `check_docs` 0 problems in `docs/demos` (1 problem elsewhere, in `docs/references/`, owned by another task); `vhco docs check .` 0 errors, 66 warnings (index-page and folder-placement naming only) |
| U-19…U-22, U-24 modules (17-modules) | Claude (TASK-075) | 2026-09-29, commit 166a98b, macOS 26.4.1 arm64 | PASS (`limit.imports` by `conformance_modules` only) |
| Linux | CI | run 36483001760 | PASS |
| Windows | — | — | NOT APPLICABLE: unsupported in 0.2.0 (INC-2026-0011) |
| Tagged release build `v0.2.0` | — | TBD at P5 | NOT STARTED |

## Known Caveats

- **Tag and commit.** Both are `TBD at P5`. The steps were recorded on the release candidate (`--version` prints `rivet 0.1.0` until the bump). Rerun at least the per-surface examples, 15-ffi step 1 and 16-editor step 1 against the tagged build and add a row to the Verification Record.
- **`conformance_samples` and modules.** `tests/conformance_samples.rs` compiles every `docs/demos/*/app.rivet` on its own without resolving imports, so [17-modules](17-modules/README.md)'s app.rivet fails it with `check.unknown_function` for `users.get`, `billing.invoice` and `users.fetch`. `rivet check` on the same file exits 0. The test must resolve imports (coordinator); the demo is unchanged.
- **Known product findings, documented as current behaviour, to be fixed in a later pass:**
  - `check.import_duplicate` from a host load (`rt.load`, `rivet_load`) reports kind `syntax` with empty `request_id` and `trace_id` ([17-modules](17-modules/README.md) step 7, [15-ffi](15-ffi/README.md) step 3).
  - The MCP `rivet.request` built-in's error envelope names `rivet.request` in `operation`, not the requested operation.
  - `policy explain` has no `--data` flag; it still takes `--params` (without a deprecation warning), unlike `request`.
  - Library stream terminal records (`env.record()`) lack `seq`, while the CLI, SSE, polling, WS and C records carry it ([12-library](12-library/README.md) step 2).
- **Observed while writing this page** (recorded, not fixed):
  - A WebSocket request rejected by validation (`c3` above) carries empty `request_id`/`trace_id`, while the same request on the CLI, REST and MCP gets IDs; a unary WS result carries `"seq":1`.
  - `check.module_policy_ignored` is printed by `rivet check` only; `request` and `serve` load the bundle silently.
  - `policy explain CALLER --params …` does not propagate params through a call into a module, so `report.remote` keeps `users.fetch`'s target `param_dependent` while `policy explain users.fetch --params '{"id":3}'` is `exact` ([17-modules](17-modules/README.md) Known Caveats).
- **R9 deviation.** A target that contains a param stays `param_dependent` even when the rest comes from globals ([14-globals](14-globals/README.md) Known Caveats).
- **Not reproduced in the demos:** `unsupported.feature` (needs a lean build), `limit.imports` (257 files), the git-tag dependency (needs the P5 tag). Each is covered by its conformance suite.
- **Public APIs.** [13-real-world-apis](13-real-world-apis/README.md) depends on third-party services and is not part of the pass/fail decision.

## Related Documents

- [Sample folders](README.md) · [demos index](index.md) · [v0.1.0 release verification guide DEMO-2026-0015](demo-2026-0015-v0-1-0-release-verification.md)
- [Implementation plan PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) · [Proposal PROP-2026-0002](../proposals/approved/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md)
- [Envelopes API-2026-0006](../api/api-2026-0006-envelopes.md) · [C ABI API-2026-0007](../api/api-2026-0007-c-abi.md) · [Error registry API-2026-0005](../api/api-2026-0005-error-registry.md)
- [Language guide MAN-2026-0003](../manuals/man-2026-0003-language-guide.md) · [C ABI and FFI MAN-2026-0009](../manuals/man-2026-0009-c-abi-and-ffi.md) · [Editor support MAN-2026-0010](../manuals/man-2026-0010-editor-support-and-highlighting.md)
- [Windows port failures INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-29 | Claude | TASK-077 (PLAN-2026-0002 D-64): created the 0.2.0 release verification guide: header with tag/commit `TBD at P5`; U-01…U-24 (one per R1…R24) with inciting UQ, command, expected result and recorded evidence; per-surface success and failure (CLI, HTTP/SSE, polling, WebSocket, MCP, Rust library, C ABI, Python, editor) re-run at 166a98b; platform coverage (macOS here, Linux CI run 36483001760, Windows unsupported per INC-2026-0011); cleanup; verification record; caveats including the `conformance_samples` import gap. |
