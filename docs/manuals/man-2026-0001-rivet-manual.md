---
document_id: MAN-2026-0001
title: "Rivet manual"
document_type: manual
status: active
created_date: 2026-09-28
last_updated: 2026-09-30
document_revision: 6
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, registry, execution, files, transports, http, datagrams, quic, grpc, connectors, auth, policy, audit, sessions, serve, poll, ws, mcp, cli, library, ffi]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, server, embedded]
audience: [developers, operators, integrators, reviewers]
scope: Root book of the Rivet manual set (0.1.0 and the 0.2.0 release candidate) — purpose, reading order, what is new in 0.2.0, goals and boundaries, concepts, mental model, the feature catalogue (every feature, why to use it, where the instructions are), configuration, limitations and supported platforms, glossary and version applicability.
reason: PLAN-2026-0001 row D-34 and PLAN-2026-0002 rows D-20, D-46 (DOCUMENTATION.md §30) — the manual is the canonical current-state book; this root answers "what can I do, why, and where are the instructions?" for the implemented build.
related_documents: [PLAN-2026-0001, PLAN-2026-0002, PROP-2026-0001, PROP-2026-0002, MIG-2026-0001, API-2026-0006, API-2026-0007, MAN-2026-0009, MAN-2026-0010, INC-2026-0011, REF-2026-0002, MAN-2026-0002, MAN-2026-0003, MAN-2026-0004, MAN-2026-0005, MAN-2026-0006, MAN-2026-0007, MAN-2026-0008, API-2026-0001, API-2026-0002, API-2026-0003, OPS-2026-0001, DEMO-2026-0001, DEMO-2026-0011, DEMO-2026-0012]
supersedes: null
superseded_by: null
tags: [rivet, manual, feature-catalogue, current-state]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.2.0-rc (source at 6f9943f)"
next_review_date: 2026-10-29
---

# Rivet manual

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-30
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** language, registry, execution, files, transports, http, datagrams, quic, grpc, connectors, auth, policy, audit, sessions, serve, poll, ws, mcp, cli, library, ffi

## Purpose

Rivet runs **described operations** written in `.rivet` files. You declare an operation once — its ID, parameters,
declared output, streamed items and errors — and the same operation is callable from the `rivet` CLI, over HTTP
(REST, SSE, polling), over WebSocket, over MCP, from Rust and (0.2.0) from C, Python or Go through `librivet`. Every effect the operation performs (files,
network, processes, credentials, MCP and gRPC calls) goes through one policy broker that reads `policy.json`
and denies anything not granted.

This book is the **current-state manual for the implemented build**. The current release is **0.2.0**
(PLAN-2026-0002) and this book describes it; examples were captured on the 0.2.0 release candidate (source at `6f9943f`, re-verified 2026-09-29), marking what changed since 0.1.0. It explains
what you can do, why you would do it, and where the exact, copy-pasteable instructions live. Volumes MAN-2026-0002
… MAN-2026-0010 hold the task procedures. **Supported platforms: macOS and Linux** (CI green on both); Windows is
not supported ([INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)).

```text
                 ┌──────────────── one app.rivet ────────────────┐
                 │ operation demo.add                            │
                 │     param a integer required                  │
                 │     output integer description "Sum…"         │
                 │     return a + b                              │
                 └───────────────────────┬───────────────────────┘
                                         │ compiled once (Capy grammar)
                                         ▼
     rivet CLI ─┐              ┌─────────────────────┐              ┌─► files / HTTP / WS / TCP / Unix
     REST/SSE ──┤              │  shared dispatcher  │   brokered   ├─► processes (sandboxed)
     polling  ──┼─ request ───►│  + typed catalog    ├─── effects ──┼─► UDP / QUIC / HTTP/3 / gRPC
     WebSocket ─┤  (ID,params) │  + policy broker    │ (policy.json)├─► OAuth tokens / credentials
     MCP  ──────┤  (envelope   └─────────────────────┘              └─► remote MCP servers
     Rust lib ──┤   {operation,
     C ABI ─────┘    data})       one ResponseEnvelope, same errors, same exit/status codes everywhere
```

## Reading Order

```text
 new user          ──► MAN-0002 install + quickstart ──► MAN-0003 language ──► MAN-0004 CLI reference
 administrator     ──► MAN-0005 policy.json + I/O manifest ──► MAN-0006 serving (auth, principals)
 integrator        ──► MAN-0008 protocols and connectors (HTTP/1-3, WS, TCP, UDP, QUIC, gRPC, OAuth, MCP)
 Rust developer    ──► MAN-0007 embedding the library (Cargo dependency, features, facade, modules)
 C / Python / Go   ──► MAN-0009 C ABI and FFI (librivet)
 editor user       ──► MAN-0010 editor support and highlighting (.vsix, rivet highlight)
 0.1.0 client      ──► MIG-2026-0001 migrating to the 0.2.0 envelopes
 everyone          ──► this book: catalogue, concepts, limitations, glossary
```

| Volume | ID | Read it when you want to… |
|---|---|---|
| [Installation and quickstart](man-2026-0002-installation-and-quickstart.md) | MAN-2026-0002 | build `rivet`, run your first `check`, `request`, `outputs`, `io` and `serve` |
| [Language guide](man-2026-0003-language-guide.md) | MAN-2026-0003 | write `.rivet` operations: params, outputs, errors, control flow, DAGs, resources, secrets, connectors |
| [CLI reference](man-2026-0004-cli-reference.md) | MAN-2026-0004 | look up every command, flag and exit code with success and failure examples |
| [Policy and I/O manifest guide](man-2026-0005-policy-and-io-manifest-guide.md) | MAN-2026-0005 | write `policy.json`, narrow by access verbs, review I/O with `rivet io`, generate a least-privilege draft |
| [Serving and surfaces](man-2026-0006-serving-and-surfaces.md) | MAN-2026-0006 | run `rivet serve`, authenticate callers, drive REST/SSE/polling/WebSocket/MCP, use `--endpoint` |
| [Embedding the library](man-2026-0007-embedding-library.md) | MAN-2026-0007 | call the same catalog from Rust with `rivet::Runtime` |
| [Protocols and connectors](man-2026-0008-protocols-and-connectors.md) | MAN-2026-0008 | pick and configure a transport, know the grants it needs, its errors and the Cargo feature it needs |
| [C ABI and FFI](man-2026-0009-c-abi-and-ffi.md) | MAN-2026-0009 | build and link `librivet`, call operations from C, Python or Go, stream, send live input, load modules |
| [Editor support and highlighting](man-2026-0010-editor-support-and-highlighting.md) | MAN-2026-0010 | install the VS Code `.vsix`, use the TextMate grammar elsewhere, run `rivet highlight` |

## What's New in 0.2.0

0.2.0 is the current release (PLAN-2026-0002; release notes
[REL-0.2.0](../releases/rel-0.2.0-release-notes.md)); the previous release is 0.1.0. migrating an existing client is
[MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md). Demos 14–17 in `docs/demos/` show each addition end to end.

```text
 0.1.0 ───────────────────────────────────────────▶ 0.2.0 (release candidate)
 {"result":…} / {…,"error"} per surface              one ResponseEnvelope {…, status, data, error, …} everywhere  (breaking)
 {id, params} · --params                             {operation, data} · --data / --input  (old keys deprecated)
 compact JSON only                                   --pretty · ?pretty=true
 constants repeated in every operation               global NAME = constant (exact I/O targets)
 one file per bundle                                 import "./x.rivet" as x [public] · rt.load / rivet_load
 package rivet, everything public                    package rivet-runtime + facade + Cargo features
 Rust only                                           C ABI librivet (C, Python ctypes, Go cgo)
 no editor support                                   rivet highlight · TextMate grammar · VS Code .vsix
```

| Addition or change | Task instructions | Demo |
|---|---|---|
| **Envelopes** (breaking): every surface answers `{request_id, trace_id, operation, type, status, data, error, effects, data_count}`; streams are `type: data` records then one `type: result`; SSE `event: data`/`result`; WS records with `ref`; MCP `structuredContent` = envelope | [API-2026-0006](../api/api-2026-0006-envelopes.md), [MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md), [MAN-0006](man-2026-0006-serving-and-surfaces.md) | [01-catalog](../demos/01-catalog/README.md) (re-verified in P4) |
| **Input envelope** `{operation, data, deadline_ms?, restrict?}`; CLI `--data`, `--input FILE\|-`; `id`/`params`/`--params` deprecated with warnings and the HTTP `deprecation: true` header | [MAN-0004 §request](man-2026-0004-cli-reference.md#rivet-request), [API-2026-0006](../api/api-2026-0006-envelopes.md#inputenvelope) | [01-catalog](../demos/01-catalog/README.md) |
| **Pretty output**: `--pretty` on the CLI, `?pretty=true` on HTTP (refused with streams) | [MAN-0004](man-2026-0004-cli-reference.md#global-options), [MAN-0006](man-2026-0006-serving-and-surfaces.md) | [01-catalog](../demos/01-catalog/README.md) |
| **Globals**: `global NAME = EXPR` load-time constants shared by every operation of a file; targets built from literals and globals are `exact` in `rivet io` | [MAN-0003 §Globals](man-2026-0003-language-guide.md#globals), [MAN-0005](man-2026-0005-policy-and-io-manifest-guide.md#globals-in-targets) | `docs/demos/14-globals/` |
| **Modules**: `import "./users.rivet" as users [public]`, namespaced IDs, one policy for every module; hosts load files as module objects (`rt.load`, `rivet_load`) | [MAN-0003 §Modules](man-2026-0003-language-guide.md#modules-import), [MAN-0005](man-2026-0005-policy-and-io-manifest-guide.md#one-policy-across-modules), [MAN-0007](man-2026-0007-embedding-library.md) | `docs/demos/17-modules/` |
| **Cargo dependency with features**: package `rivet-runtime` from the git tag; features `serve`, `grpc`, `quic`, `oauth` (default) and `cli`; a compiled-out adapter is `unsupported.feature` | [MAN-0007](man-2026-0007-embedding-library.md), [MAN-0008](man-2026-0008-protocols-and-connectors.md#cargo-features-per-protocol) | [12-library](../demos/12-library/README.md) |
| **Rust facade**: `rivet::{Runtime, InputEnvelope, ResponseEnvelope, Module, …}`, `Runtime::call`, `load`/`load_as` | [MAN-0007](man-2026-0007-embedding-library.md), [API-2026-0004](../api/api-2026-0004-rust-library.md) | [12-library](../demos/12-library/README.md) |
| **C ABI** `librivet` (shared and static, `rivet.h`, `rivet.pc`): 18 `rivet_*` functions, call handles, module objects; Python and Go examples | [MAN-0009](man-2026-0009-c-abi-and-ffi.md), [API-2026-0007](../api/api-2026-0007-c-abi.md) | `docs/demos/15-ffi/` |
| **Highlighting**: `rivet highlight FILE --format ansi\|html\|json`, `rivet::highlight`, `rivet_highlight`, the generated TextMate grammar and the VS Code `.vsix` | [MAN-0010](man-2026-0010-editor-support-and-highlighting.md) | `docs/demos/16-editor/` |
| **Platforms**: macOS and Linux supported (CI green on both); Windows dropped ([INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)) | [MAN-0002](man-2026-0002-installation-and-quickstart.md) | — |

### Introduced in 0.1.0

0.1.0 was the **first implemented release**; everything below was new then. Detailed history belongs to the release
document `REL-0.1.0` (created in PLAN-2026-0001 P5).

| Addition | Task instructions | Demo |
|---|---|---|
| One `.rivet` file declares many described operations with typed params, declared outputs, `emits`, `receives` and declared errors | [MAN-2026-0003](man-2026-0003-language-guide.md) | [01-catalog](../demos/01-catalog/README.md) |
| CLI: `request`, `list`, `describe`, `outputs`, `check`, `io`, `graph`, `policy explain/generate`, `trace show/export`, `auth …`, `connectors sync`, `serve` | [MAN-2026-0004](man-2026-0004-cli-reference.md) | [01-catalog](../demos/01-catalog/README.md) |
| `check` warnings (undeclared `fail` codes, unguarded DAG results) and `check.unknown_function` with a did-you-mean hint | [MAN-2026-0004 §check](man-2026-0004-cli-reference.md#rivet-check) | [05-dag](../demos/05-dag/README.md) |
| `if … else … end`; infix expressions inside objects, lists and call arguments | [MAN-2026-0003 §Control flow](man-2026-0003-language-guide.md#control-flow) | [01-catalog](../demos/01-catalog/README.md) |
| Scoped file handles `with file open PATH mode read\|write\|append as NAME` (+ `chunk_size N`) | [MAN-2026-0003 §Files](man-2026-0003-language-guide.md#files) | [04-streaming](../demos/04-streaming/README.md) |
| `rivet graph ID` static call graph; `rivet trace export`; `rivet.capabilities` | [MAN-2026-0004](man-2026-0004-cli-reference.md#rivet-graph) | [05-dag](../demos/05-dag/README.md) |
| Per-request `restrict {grants}` (narrow-only) on HTTP, MCP, WebSocket, polling and the library; library host ceiling | [MAN-2026-0005 §Narrow one request](man-2026-0005-policy-and-io-manifest-guide.md#narrow-one-request-with-restrict) | [12-library](../demos/12-library/README.md) |
| `serve`: `GET /v1/health`, one access-log line per request, W3C `traceparent`, SIGTERM drain | [MAN-2026-0006](man-2026-0006-serving-and-surfaces.md#health-access-log-and-shutdown) | [01-catalog](../demos/01-catalog/README.md) |
| `policy.json` only (beside the entry file, or `--policy PATH`); absent = deny-by-default; `access` verbs narrow grants | [MAN-2026-0005](man-2026-0005-policy-and-io-manifest-guide.md) | [11-sandbox](../demos/11-sandbox/README.md) |
| Generated I/O manifest (`rivet io`), `--check-policy`, `--needs`, `--check-files`, and `policy generate` | [MAN-2026-0005](man-2026-0005-policy-and-io-manifest-guide.md) | [11-sandbox](../demos/11-sandbox/README.md) |
| One `rivet serve` mounts REST, SSE, polling, WebSocket (`rivet.v1`) and MCP (`/mcp`); `--stdio` for MCP only | [MAN-2026-0006](man-2026-0006-serving-and-surfaces.md) | [01-catalog](../demos/01-catalog/README.md) |
| Bearer authentication, per-principal operation lists, `serve.surfaces` | [MAN-2026-0006](man-2026-0006-serving-and-surfaces.md) | [01-catalog](../demos/01-catalog/README.md) |
| `--endpoint URL` thin client with `--token-file` | [MAN-2026-0006](man-2026-0006-serving-and-surfaces.md) | [01-catalog](../demos/01-catalog/README.md) |
| Rust embedding: `rivet::Runtime::builder()`, `Runtime::scope` stream/duplex, `Policy::from_file/from_json`, `.ceiling(Policy)`, typed `DataSink` stop | [MAN-2026-0007](man-2026-0007-embedding-library.md) | [12-library](../demos/12-library/README.md) |
| Files with explicit verbs and hard-link refusal | [MAN-2026-0003](man-2026-0003-language-guide.md) | [02-file-crud](../demos/02-file-crud/README.md) |
| HTTP/1.1 + HTTP/2 client, SSE/JSONL/lines/bytes streams, explicit redirects and retries | [MAN-2026-0008](man-2026-0008-protocols-and-connectors.md) | [03-http](../demos/03-http/README.md), [04-streaming](../demos/04-streaming/README.md) |
| DAGs, `map`, `poll`, `iterate`, `concurrent`, `try`/`catch` | [MAN-2026-0003](man-2026-0003-language-guide.md) | [05-dag](../demos/05-dag/README.md) |
| MCP client connectors with reviewed snapshots and `connectors sync` | [MAN-2026-0008](man-2026-0008-protocols-and-connectors.md) | [06-mcp-bridge](../demos/06-mcp-bridge/README.md) |
| OAuth 2.0: client_credentials, authorization_code + PKCE S256, device_code; `auth` commands | [MAN-2026-0008](man-2026-0008-protocols-and-connectors.md) | [07-oauth2](../demos/07-oauth2/README.md) |
| UDP unicast, bind and multicast | [MAN-2026-0008](man-2026-0008-protocols-and-connectors.md) | [08-udp](../demos/08-udp/README.md) |
| QUIC v1 and HTTP/3 (`version 3` strict, `version prefer [3, 2]`) | [MAN-2026-0008](man-2026-0008-protocols-and-connectors.md) | [09-quic](../demos/09-quic/README.md) |
| gRPC unary, server/client streaming and bidi from a pinned `FileDescriptorSet` | [MAN-2026-0008](man-2026-0008-protocols-and-connectors.md) | [10-grpc](../demos/10-grpc/README.md) |
| argv-only processes, macOS Seatbelt sandbox | [MAN-2026-0008](man-2026-0008-protocols-and-connectors.md) | [11-sandbox](../demos/11-sandbox/README.md) |

## Project Goals and Boundaries

```text
   GOALS (what Rivet is for)                          NON-GOALS / BOUNDARIES (0.2.0)
   ───────────────────────────────────────            ─────────────────────────────────────────────
   one declared operation, every surface              not a general-purpose programming language
   typed params + declared, validated outputs         no implicit I/O: every effect is a named form
   deny-by-default effects from one policy.json       no command-line or environment grants
   reviewable I/O before running (rivet io)           no shell strings, no PATH lookup for commands
   explicit protocol behaviour (no silent             no silent HTTP redirect/version fallback,
     redirects, fallbacks or replays)                   no automatic replay of mutations
   bounded concurrency, memory, deadlines, cleanup    no mTLS on `serve`, no persistent trace store
   same envelope / errors / exit codes / statuses     no graphical UI (editor highlighting only)
   modules from local files under one root            no URL imports, no hot reload of imports
   one policy for the whole bundle and its modules    no per-module policy files (ignored, with a warning)
```

Rivet guarantees that **script-initiated effects** pass the broker. It does not sandbox the host that embeds it,
and it cannot see what a spawned child process does except through the OS sandbox (see MAN-2026-0008).

## Concepts

| Concept | Meaning |
|---|---|
| Bundle | The entry `.rivet` file named with `--file`, the files it `import`s (0.2.0) and the one `policy.json` beside the entry file. The CLI requires `--file` for local commands. |
| Global | (0.2.0) `global NAME = EXPR` — a constant evaluated once at load, visible to every operation of its file; never a secret or an effect. |
| Module | (0.2.0) A `.rivet` file imported `as ALIAS` (or loaded with `rt.load`): its operations are namespaced `ALIAS.ID`; `public` imports are callable from outside, internal ones only from the importing file. |
| Input envelope | (0.2.0) `{operation, data, deadline_ms?, restrict?}` — what every surface accepts (`id`/`params` are deprecated aliases). |
| Response envelope | (0.2.0) `{request_id, trace_id, operation, type, status, data, error, effects, data_count}` — what every surface answers; `status` is `ok`, `error`, `cancelled` or `accepted`. |
| Operation / pipeline | A named, typed callable (`operation demo.add … end`). `pipeline` is the same thing, used for composition. |
| Header | Lines before the first body statement, in the fixed order `name, description, private, param, output, emits, receives, error`. |
| Declared output | The `output` type (and `field`s) the result must match; a mismatch fails `output.invalid` (exit 5). |
| Emits / receives | `emits T` declares streamed data items; `receives T` declares live input read with `for item in incoming`. |
| Completion | The Rust result of `Runtime::request` (`result`, `data_count`, `effects`); on the wire it is rendered as a response envelope. |
| Stream record | One streamed item `{…, type: "data", seq, data, error: null}`, then exactly one terminal `type: "result"` record with `status`. |
| Cargo feature | (0.2.0) A compile-time switch of `rivet-runtime` (`serve`, `grpc`, `quic`, `oauth`, `cli`); a bundle needing a compiled-out one fails `unsupported.feature`. |
| Effect site | One place in source that performs I/O; listed by `rivet io` with kind, access verb, target and capability. |
| Capability / access verb | The grant family (`allow_read`, `allow_network`, …) and the verb within it (`read`, `create`, `connect`, …). |
| Policy broker | The single component that authorizes every effect against `policy.json`; deny entries win. |
| Principal | The authenticated caller of a serve surface (`local` for CLI and library). |
| Session | A live request with an ID you can poll, feed input to, finish or cancel (polling, WebSocket refs, `rivet.sessions.*`). |
| Connector | A declared MCP or gRPC endpoint (`connector NAME mcp\|grpc … end`). |
| Auth profile | A declared OAuth 2.0 client (`auth NAME oauth2 … end`), used by `auth PROFILE account "A"`. |

## Architecture and Mental Model

```text
  app.rivet ──parse (Capy)──► lower + check ──► CompiledProgram ──► Registry (catalog, schemas)
                                  │ errors: syntax.*, check.*, registry.duplicate_id  (exit 2)
  policy.json ──strict schema v1──► Policy ─────────────────────────► Policy broker
                                  │ errors: policy.invalid (exit 2, JSON pointer named)

  request(ID, params, principal, restrict?)
      │
      ├─ principal allowed? (serve.principals) ── no ─► permission.denied (403 / exit 3)
      ├─ params valid? (types, required, min/max/enum, unknown fields) ── no ─► validation.* (422 / exit 2)
      ├─ run body in a scope with a deadline (default 30 s)
      │     each effect ─► broker: policy.json ∩ host ceiling ∩ restrict — grant? deny? private range?
      │                            ├─ no ─► permission.denied
      │                            └─ yes ─► secret-taint check ─► adapter (file/http/udp/…) ─► trace event
      ├─ result matches declared output? ── no ─► output.invalid (500 / exit 5)
      └─ ResponseEnvelope {request_id, trace_id, operation, type:"result", status:"ok", data, error:null, effects, data_count}
         (every failure above is the same envelope with status "error", data null and the error object)
```

Four facts to keep in mind:

1. **Nothing runs until it compiles.** `rivet check` compiles without running anything; every other local command
   compiles first too, so a syntax error blocks `request`, `list` and `serve` alike.
2. **Policy is a file, never a flag.** No `policy.json` beside the entry file means every new effect is denied
   while pure operations still run.
3. **One dispatcher.** The CLI, every serve surface and the library call the same `request` path, so results,
   error codes and exit codes/HTTP statuses match.
4. **Scopes clean up.** A `with` block, a `scope`, a DAG or a request deadline closes its handles (in reverse
   order, within 5 s) whether the body returns, fails or is cancelled. Cancellation is structured: a cancel,
   deadline or server shutdown fires a token that the run observes at its next await point.

## Installation and Setup

Build from source with Cargo (Rust 1.90.0, pinned by `rust-toolchain.toml`) on macOS or Linux; see
[MAN-2026-0002](man-2026-0002-installation-and-quickstart.md). There is no crates.io crate; install from the git tag
(0.2.0: the binary needs the `cli` feature) or with `perch install`:

```bash
cargo install rivet-runtime --git https://github.com/olivierdevelops/rivet --tag v0.2.1 --features cli
rivet --version                      # rivet 0.2.1
```

## Feature Catalogue

"Since" is the first version that ships the feature. Surfaces: **CLI** = local `rivet` with `--file`;
**Remote** = `rivet --endpoint`; **HTTP** = REST/SSE/polling; **WS** = WebSocket `rivet.v1`; **MCP** = `/mcp`
or `--stdio`; **Lib** = Rust `Runtime`; **C** = the C ABI (`librivet`).

### New in 0.2.0

| Feature | Why / When to Use It | Supported Surfaces | Since Version | Instructions | Demo |
|---|---|---|---|---|---|
| Response envelopes | One parser for every surface: switch on `status`, read `data` or `error`; streams end with one `type: result` record. Use it whenever you write a client. | all | 0.2.0 | [API-2026-0006](../api/api-2026-0006-envelopes.md), [MAN-0006 §Envelopes](man-2026-0006-serving-and-surfaces.md#envelopes-on-every-surface) | [01-catalog](../demos/01-catalog/README.md) |
| Input envelope, `--data`, `--input FILE\|-` | Send the same `{operation, data}` document from a shell, a file, HTTP, WS, MCP, Rust or C; `--input` when a script builds the whole request | all | 0.2.0 | [MAN-0004 §request](man-2026-0004-cli-reference.md#rivet-request) | [01-catalog](../demos/01-catalog/README.md) |
| Deprecated `id`/`params`/`--params` | Keep 0.1.0 clients running through 0.2.x while you migrate; watch `deprecated=1` in the access log | all | 0.2.0 (removed 0.3.0) | [MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md), [MAN-0006](man-2026-0006-serving-and-surfaces.md#deprecation-monitoring) | — |
| Pretty output (`--pretty`, `?pretty=true`) | Read envelopes by eye while developing; keep compact JSON for machines and streams | CLI, HTTP, Lib, C | 0.2.0 | [MAN-0004](man-2026-0004-cli-reference.md#global-options) | [01-catalog](../demos/01-catalog/README.md) |
| Globals (`global NAME = EXPR`) | Name a base URL, a limit or a path once per file; targets built from literals and globals stay `exact`, so policy grants stay exact | language | 0.2.0 | [MAN-0003 §Globals](man-2026-0003-language-guide.md#globals), [MAN-0005](man-2026-0005-policy-and-io-manifest-guide.md#globals-in-targets) | `docs/demos/14-globals/` |
| Modules (`import … as ALIAS [public]`, `rt.load`) | Split a large bundle into files, reuse a library of operations, or load a file into a running host; one policy governs all of them | language, all surfaces, Lib, C | 0.2.0 | [MAN-0003 §Modules](man-2026-0003-language-guide.md#modules-import), [MAN-0005](man-2026-0005-policy-and-io-manifest-guide.md#one-policy-across-modules), [MAN-0007](man-2026-0007-embedding-library.md#load-files-as-module-objects) | `docs/demos/17-modules/` |
| Cargo dependency with features | Depend on Rivet from another Rust project by git tag; drop `serve`/`grpc`/`quic`/`oauth` for a lean build | Lib | 0.2.0 | [MAN-0007](man-2026-0007-embedding-library.md#add-the-dependency-and-choose-features), [MAN-0008](man-2026-0008-protocols-and-connectors.md#cargo-features-per-protocol) | [12-library](../demos/12-library/README.md) |
| Rust facade and `Runtime::call` | A small, stable import surface; `call` returns the same envelope as every other surface | Lib | 0.2.0 | [MAN-0007](man-2026-0007-embedding-library.md), [API-2026-0004](../api/api-2026-0004-rust-library.md) | [12-library](../demos/12-library/README.md) |
| C ABI (`librivet`) | Embed Rivet in C, C++, Python, Go or any language with a C FFI, with the same envelopes and policy | C | 0.2.0 | [MAN-0009](man-2026-0009-c-abi-and-ffi.md), [API-2026-0007](../api/api-2026-0007-c-abi.md) | `docs/demos/15-ffi/` |
| Highlighting (`rivet highlight`, grammar, `.vsix`) | Read `.rivet` code with colours in a terminal, docs pipeline or VS Code; token classes come from the parser | CLI, Lib, C, editors | 0.2.0 | [MAN-0010](man-2026-0010-editor-support-and-highlighting.md) | `docs/demos/16-editor/` |

### Authoring operations

| Feature | Why / When to Use It | Supported Surfaces | Since Version | Instructions | Demo |
|---|---|---|---|---|---|
| Operations and pipelines | Give one callable a stable ID, a name and a description shown everywhere | all | 0.1.0 | [MAN-0003 §Operations](man-2026-0003-language-guide.md#write-an-operation) | [01-catalog](../demos/01-catalog/README.md) |
| Typed parameters (`required`, `default`, `min`, `max`, `enum`) | Reject bad input before any effect runs | all | 0.1.0 | [MAN-0003 §Parameters](man-2026-0003-language-guide.md#declare-parameters) | [01-catalog](../demos/01-catalog/README.md) |
| Declared outputs and fields (`open true`) | Promise callers a result shape; violations fail `output.invalid` | all; `rivet outputs` | 0.1.0 | [MAN-0003 §Outputs](man-2026-0003-language-guide.md#declare-outputs-emits-receives-and-errors) | [01-catalog](../demos/01-catalog/README.md) |
| Declared errors and `fail` | Give callers stable, documented failure codes | all | 0.1.0 | [MAN-0003 §Errors](man-2026-0003-language-guide.md#declare-outputs-emits-receives-and-errors) | [03-http](../demos/03-http/README.md) |
| `emits` streaming | Deliver items as they are produced (NDJSON, SSE, polling, WS) | all | 0.1.0 | [MAN-0003 §Streams](man-2026-0003-language-guide.md#stream-data-and-receive-live-input) | [04-streaming](../demos/04-streaming/README.md) |
| `receives` live input (`incoming`) | Feed items into a running request (duplex) | CLI `--input-jsonl -`, polling, WS, MCP sessions, Lib | 0.1.0 | [MAN-0003 §Streams](man-2026-0003-language-guide.md#stream-data-and-receive-live-input) | [10-grpc](../demos/10-grpc/README.md) |
| `private true` helpers | Keep helper operations callable only from other operations | language | 0.1.0 | [MAN-0003 §Operations](man-2026-0003-language-guide.md#write-an-operation) | [05-dag](../demos/05-dag/README.md) |
| Composition `(request "id" {…})`, dynamic IDs with `allow […]` | Reuse operations; literal calls are checked at compile time, cycles rejected | language | 0.1.0 | [MAN-0003 §Calls](man-2026-0003-language-guide.md#call-other-operations) | [11-sandbox](../demos/11-sandbox/README.md) |
| Control flow: `if … else … end`, `for`, `while`, `break`, `iterate max N`, `return`, `+=` | Ordinary logic with bounded loops (no `else if`: nest an `if` in the `else`) | language | 0.1.0 | [MAN-0003 §Control flow](man-2026-0003-language-guide.md#control-flow) | [01-catalog](../demos/01-catalog/README.md) |
| `try` / `catch error [kind K\|code "C"]` | Recover from a typed failure | language | 0.1.0 | [MAN-0003 §try/catch](man-2026-0003-language-guide.md#recover-with-try-and-catch) | [03-http](../demos/03-http/README.md) |
| `map … limit N … yield`, `poll every … timeout … until … yield` | Bounded fan-out; wait for a remote job | language | 0.1.0 | [MAN-0003 §map and poll](man-2026-0003-language-guide.md#map-poll-and-iterate) | [05-dag](../demos/05-dag/README.md) |
| `dag` with `node … after […]`, `fail fast`/`fail independent` | Run a dependency graph and inspect per-node status | language | 0.1.0 | [MAN-0003 §DAG](man-2026-0003-language-guide.md#run-a-dag) | [05-dag](../demos/05-dag/README.md) |
| `concurrent` / `task`, `scope timeout` | Structured parallel work joined before exit; group deadlines | language | 0.1.0 | [MAN-0003 §Concurrency](man-2026-0003-language-guide.md#concurrent-tasks-and-scopes) | [10-grpc](../demos/10-grpc/README.md) |
| `with` resource blocks | Own a socket/stream/process/file handle for exactly one block | language | 0.1.0 | [MAN-0003 §Resources](man-2026-0003-language-guide.md#own-resources-with-with) | [04-streaming](../demos/04-streaming/README.md) |
| `secret NAME from env "VAR" for "ORIGIN"` | Use a credential bound to one destination origin; returning, emitting or sending it anywhere else (files, processes, other origins, nested calls, derived encodings) is refused | language | 0.1.0 | [MAN-0003 §Secrets](man-2026-0003-language-guide.md#use-secrets) | [03-http](../demos/03-http/README.md) |
| Pure helpers `length`, `keys`, `text`, `base64.encode/decode`, `xml.element` | Compute values without I/O | language | 0.1.0 | [MAN-0003 §Expressions](man-2026-0003-language-guide.md#values-and-expressions) | — |
| `rivet check [--strict-docs]` | Compile and lint without running (warnings for undeclared `fail` codes and unguarded DAG results; unknown functions fail with a did-you-mean); enforce descriptions in CI | CLI | 0.1.0 | [MAN-0004 §check](man-2026-0004-cli-reference.md#rivet-check) | [01-catalog](../demos/01-catalog/README.md) |
| `rivet graph ID [--all] [--json]` | See the static call graph: literal calls, connector calls, effect sites, DAG nodes and `after` edges, both `if` arms | CLI, Lib | 0.1.0 | [MAN-0004 §graph](man-2026-0004-cli-reference.md#rivet-graph) | [05-dag](../demos/05-dag/README.md) |

### Effects (what an operation can touch)

| Feature | Why / When to Use It | Supported Surfaces | Since Version | Instructions | Demo |
|---|---|---|---|---|---|
| File verbs: `read`, `list`, `stat`, `create`, `update`, `write`, `append`, `delete` (`missing ok`), `copy`, `move` | Explicit intent per write; `create` never overwrites, `update` never creates; `update … if_version V` is a locked compare-and-replace | all | 0.1.0 | [MAN-0003 §Files](man-2026-0003-language-guide.md#files) | [02-file-crud](../demos/02-file-crud/README.md) |
| Scoped file handles `with file open … mode read\|write\|append` | Read a large file in bounded byte chunks, or write/append through one handle | all | 0.1.0 | [MAN-0003 §Files](man-2026-0003-language-guide.md#files) | [04-streaming](../demos/04-streaming/README.md) |
| Hard-link refusal | Stop writes through a second name of a file | all | 0.1.0 | [MAN-0003 §Files](man-2026-0003-language-guide.md#files) | [02-file-crud](../demos/02-file-crud/README.md) |
| HTTP/1.1 and HTTP/2 client | Call REST APIs with explicit methods, bodies, decoding, retries and redirects | all | 0.1.0 | [MAN-0008 §HTTP](man-2026-0008-protocols-and-connectors.md#http11-and-http2) | [03-http](../demos/03-http/README.md) |
| HTTP response streams: `stream sse\|jsonl\|lines\|bytes` | Consume model/log streams item by item | all | 0.1.0 | [MAN-0008 §Streams](man-2026-0008-protocols-and-connectors.md#http-response-streams) | [04-streaming](../demos/04-streaming/README.md) |
| HTTP/3 (`version 3`, `version prefer [3, 2]`) | Require or prefer QUIC-based HTTP without silent fallback | all | 0.1.0 | [MAN-0008 §HTTP/3](man-2026-0008-protocols-and-connectors.md#http3) | [09-quic](../demos/09-quic/README.md) |
| WebSocket client | Scoped request/response or streaming over `ws://`/`wss://` | all | 0.1.0 | [MAN-0008 §WebSocket](man-2026-0008-protocols-and-connectors.md#websocket-client) | [04-streaming](../demos/04-streaming/README.md) |
| TCP and Unix sockets with framing | Talk to line/length-prefixed services | all | 0.1.0 | [MAN-0008 §TCP/Unix](man-2026-0008-protocols-and-connectors.md#tcp-and-unix-sockets) | — |
| Processes (argv only) and the OS sandbox | Run a local tool without a shell; confine it by policy | all | 0.1.0 | [MAN-0008 §Processes](man-2026-0008-protocols-and-connectors.md#processes-and-the-sandbox) | [11-sandbox](../demos/11-sandbox/README.md) |
| UDP unicast, bind, multicast | Datagram telemetry and discovery | all | 0.1.0 | [MAN-0008 §UDP](man-2026-0008-protocols-and-connectors.md#udp) | [08-udp](../demos/08-udp/README.md) |
| QUIC v1 streams and datagrams | Multiplexed reliable streams with ALPN | all | 0.1.0 | [MAN-0008 §QUIC](man-2026-0008-protocols-and-connectors.md#quic) | [09-quic](../demos/09-quic/README.md) |
| gRPC (unary, server stream, client stream, bidi) | Call typed services from a pinned descriptor | all | 0.1.0 | [MAN-0008 §gRPC](man-2026-0008-protocols-and-connectors.md#grpc) | [10-grpc](../demos/10-grpc/README.md) |
| OAuth 2.0 profiles and `rivet auth …` | Acquire and manage tokens; tokens are never printed | all; `auth` CLI | 0.1.0 | [MAN-0008 §OAuth](man-2026-0008-protocols-and-connectors.md#oauth-20) | [07-oauth2](../demos/07-oauth2/README.md) |
| MCP client connectors, `connectors sync`, approved snapshots | Call remote MCP tools/resources/prompts as operations after review | all; `connectors` CLI | 0.1.0 | [MAN-0008 §MCP connectors](man-2026-0008-protocols-and-connectors.md#mcp-client-connectors) | [06-mcp-bridge](../demos/06-mcp-bridge/README.md) |

### Policy, inspection and audit

| Feature | Why / When to Use It | Supported Surfaces | Since Version | Instructions | Demo |
|---|---|---|---|---|---|
| `policy.json` (grants, deny, access, network, limits, serve, approved) | Decide exactly what effects are allowed | all | 0.1.0 | [MAN-0005 §Write a policy](man-2026-0005-policy-and-io-manifest-guide.md#write-a-policyjson) | [11-sandbox](../demos/11-sandbox/README.md) |
| Deny-by-default and private-range protection | Safe by default; block SSRF to loopback/RFC1918/metadata | all | 0.1.0 | [MAN-0005 §Network](man-2026-0005-policy-and-io-manifest-guide.md#network-targets-and-private-ranges) | [03-http](../demos/03-http/README.md) |
| `rivet policy explain [ID] [--params JSON]` | See the effective policy and each site's decision; with `--params`, the concrete targets of one call (exit 3 when denied) | CLI | 0.1.0 | [MAN-0005 §Explain](man-2026-0005-policy-and-io-manifest-guide.md#explain-the-effective-policy) | [11-sandbox](../demos/11-sandbox/README.md) |
| Per-request `restrict {grants}` | Let a caller narrow one request's authority (never widen) | HTTP, polling, WS, MCP, Lib | 0.1.0 | [MAN-0005 §restrict](man-2026-0005-policy-and-io-manifest-guide.md#narrow-one-request-with-restrict) | — |
| `rivet io` manifest (`--by`, `--kind`, `--access`, `--format`, `--include-bootstrap`) | Review every URL, path and verb before running anything | CLI, Remote, HTTP `/v1/io`, MCP `rivet.io`, Lib | 0.1.0 | [MAN-0005 §I/O manifest](man-2026-0005-policy-and-io-manifest-guide.md#review-io-with-rivet-io) | [11-sandbox](../demos/11-sandbox/README.md) |
| `io --check-policy`, `--strict` | Gate CI on "every site allowed" and "nothing dynamic" | CLI, Remote | 0.1.0 | [MAN-0005 §Check](man-2026-0005-policy-and-io-manifest-guide.md#check-the-manifest-against-policy) | [11-sandbox](../demos/11-sandbox/README.md) |
| `io --needs`, `--check-files` | Know which files must exist before running; probe them | CLI (`--check-files` local only) | 0.1.0 | [MAN-0005 §Needs](man-2026-0005-policy-and-io-manifest-guide.md#files-an-operation-needs) | [02-file-crud](../demos/02-file-crud/README.md) |
| `rivet policy generate [--output]` | Draft a least-privilege policy from the manifest | CLI, HTTP `/v1/policy/generate`, Lib | 0.1.0 | [MAN-0005 §Generate](man-2026-0005-policy-and-io-manifest-guide.md#generate-a-least-privilege-draft) | [11-sandbox](../demos/11-sandbox/README.md) |
| `rivet trace show REQ`, `io --trace REQ` | See broker decisions for one request on a running server | Remote, MCP `rivet.trace.show`, Lib | 0.1.0 | [MAN-0005 §Trace](man-2026-0005-policy-and-io-manifest-guide.md#trace-one-request) | [11-sandbox](../demos/11-sandbox/README.md) |
| `rivet trace export REQ --output PATH`, `Runtime::export_trace` | Save one request's sanitized trace to a new file through the broker | Remote, HTTP/MCP `rivet.trace.export`, Lib | 0.1.0 | [MAN-0004 §trace](man-2026-0004-cli-reference.md#rivet-trace-show-and-export) | — |
| `rivet.capabilities` | Ask what this build supports (Stage A/B/C, HTTP versions, sandbox status) | CLI `request`, HTTP, MCP `tools/call`, Lib | 0.1.0 | [MAN-0006 §Built-ins](man-2026-0006-serving-and-surfaces.md#built-in-operations) | — |

### Surfaces

| Feature | Why / When to Use It | Supported Surfaces | Since Version | Instructions | Demo |
|---|---|---|---|---|---|
| CLI `request` (`--data`, `--input`, `--pretty`, `--stream`, `--timeout`, `--input-jsonl -`, Ctrl-C) | Run one operation from a shell or script | CLI, Remote | 0.1.0 (`--data`/`--input`/`--pretty` 0.2.0) | [MAN-0004 §request](man-2026-0004-cli-reference.md#rivet-request) | [01-catalog](../demos/01-catalog/README.md) |
| `list`, `describe`, `outputs` | Discover the catalog and its schemas | CLI, Remote, HTTP, MCP, Lib | 0.1.0 | [MAN-0004](man-2026-0004-cli-reference.md#rivet-list) | [01-catalog](../demos/01-catalog/README.md) |
| `rivet serve` one listener | Expose every surface on one port | serve | 0.1.0 | [MAN-0006 §Start](man-2026-0006-serving-and-surfaces.md#start-a-server) | [01-catalog](../demos/01-catalog/README.md) |
| `GET /v1/health`, access log, `traceparent`, SIGTERM drain | Probe liveness, audit each request, correlate traces, stop cleanly | serve | 0.1.0 | [MAN-0006 §Health](man-2026-0006-serving-and-surfaces.md#health-access-log-and-shutdown) | [01-catalog](../demos/01-catalog/README.md) |
| REST `/v1/request`, `/v1/operations…`, `/v1/io`, `/v1/policy/generate` | Call from any HTTP client | HTTP | 0.1.0 | [MAN-0006 §REST](man-2026-0006-serving-and-surfaces.md#call-over-rest) | [01-catalog](../demos/01-catalog/README.md) |
| SSE (`Accept: text/event-stream`) | Stream items to browsers and HTTP clients | HTTP | 0.1.0 | [MAN-0006 §SSE](man-2026-0006-serving-and-surfaces.md#stream-over-sse) | [01-catalog](../demos/01-catalog/README.md) |
| Polling sessions `/v1/requests…` | Long-poll events; send input; cancel | HTTP | 0.1.0 | [MAN-0006 §Polling](man-2026-0006-serving-and-surfaces.md#poll-a-session) | [01-catalog](../demos/01-catalog/README.md) |
| WebSocket `/v1/ws` (`rivet.v1`) | Multiplex up to 8 requests per connection | WS | 0.1.0 | [MAN-0006 §WebSocket](man-2026-0006-serving-and-surfaces.md#multiplex-over-websocket) | [01-catalog](../demos/01-catalog/README.md) |
| MCP server (`/mcp` Streamable HTTP, `serve --stdio`) | Expose operations as MCP tools to agents | MCP | 0.1.0 | [MAN-0006 §MCP](man-2026-0006-serving-and-surfaces.md#expose-tools-over-mcp) | [01-catalog](../demos/01-catalog/README.md) |
| Serve auth (`none` loopback-only, `bearer`), principals, `serve.surfaces` | Authenticate callers; limit operations and surfaces | serve | 0.1.0 | [MAN-0006 §Auth](man-2026-0006-serving-and-surfaces.md#authenticate-callers-and-authorize-operations) | [01-catalog](../demos/01-catalog/README.md) |
| `--endpoint URL --token-file PATH` | Use the same CLI against a running server | Remote | 0.1.0 | [MAN-0006 §Endpoint](man-2026-0006-serving-and-surfaces.md#use-the-cli-against-a-server) | [01-catalog](../demos/01-catalog/README.md) |
| Built-in `rivet.*` operations | Discovery, sessions, I/O, policy draft, traces and OAuth through any surface | all | 0.1.0 | [MAN-0006 §Built-ins](man-2026-0006-serving-and-surfaces.md#built-in-operations) | [01-catalog](../demos/01-catalog/README.md) |
| Rust library `rivet::Runtime` (scopes, ceiling, restrict, typed sink stop) | Embed the catalog in a Rust host | Lib | 0.1.0 (facade 0.2.0) | [MAN-0007](man-2026-0007-embedding-library.md) | [12-library](../demos/12-library/README.md) |

## Configuration and Environment Variables

Rivet reads **no environment variable for its own configuration** and has no configuration file other than
`policy.json`. Environment variables are read only when an operation asks for one through a granted form.

| Name | Kind | Type / Allowed Values | Default | Required When | Scope | Effect | Security Notes | Example |
|---|---|---|---|---|---|---|---|---|
| `--file PATH` | CLI flag | path to entry `.rivet` | none | every local command | invocation | selects the bundle; `policy.json` beside it is discovered | the bundle root anchors relative paths | `--file app.rivet` |
| `--data JSON` | CLI flag (0.2.0) | JSON object | `{}` | operation has params | request | the envelope's `data`; `--params` is its deprecated alias (`warning[deprecated.params]`) | never put secrets in argv | `--data '{"a":2}'` |
| `--input FILE\|-` | CLI flag (0.2.0) | path or `-` (stdin) | none | a script builds the whole request | request | reads a complete InputEnvelope; legacy keys warn `deprecated.input` | — | `--input req.json` |
| `--pretty` / `?pretty=true` | CLI flag / HTTP query (0.2.0) | boolean | off | reading by eye | output | 2-space JSON, same key order; refused with `--stream` / SSE | — | `--pretty` |
| Cargo features | build (0.2.0) | `serve`, `grpc`, `quic`, `oauth` (default), `cli` | defaults | building the binary / depending on the crate | build | compiled-out adapters fail `unsupported.feature` | fewer features = smaller attack surface | `--features cli` |
| `--policy PATH` | CLI flag | path to a policy JSON file | discovered `policy.json` | selecting another file | invocation | replaces discovery; a missing file is `policy.invalid` (exit 2) | a path, never grant text | `--policy policies/read-only.json` |
| `policy.json` | file | schema v1 (MAN-2026-0005) | absent → deny-by-default | any effect | bundle | grants, deny, network, limits, serve, approved | deny wins; private ranges denied unless named literally | `{"version": 1}` |
| `limits.max_concurrent_requests` | policy key | positive integer | 64 | — | host | top-level requests in flight | exceeding → `limit.concurrency` | `64` |
| `limits.max_call_depth` | policy key | positive integer | 16 | — | host | nested `(request …)` depth | — | `16` |
| `limits.max_buffered_bytes` | policy key | positive integer ≤ 9223372036854775807 | 268435456 | — | host | bytes all session/stream queues of the host may hold | exceeding → `limit.buffered_bytes` | `268435456` |
| `--listen HOST:PORT` | serve flag | IP or `localhost` + port | `127.0.0.1:8080` | serve | process | listener address | non-loopback needs `serve.auth` | `--listen 127.0.0.1:18080` |
| `--token-file PATH` | CLI flag | file with the bearer token | none | `--endpoint` to a bearer server | invocation | sends `Authorization: Bearer …` | never pass tokens in argv or env | `--token-file ~/.rivet/ada.token` |
| `--timeout D` | CLI flag | digits + `ms`/`s`/`m`/`h`, at most `10m` | 30s | long operations | request | request deadline; above 600000 ms → `validation.usage` (exit 2) | — | `--timeout 2m` |
| `deadline_ms` | input-envelope field | integer ms | 30000 | long requests / sessions | request | request, session or WS-ref deadline, capped at 600000 | — | `{"deadline_ms": 5000}` |
| `restrict` | input-envelope field (HTTP / MCP / WS / Lib / C) | `{"grants": [...]}` | none | narrowing one request | request | intersected with policy.json for that request | narrows only; `policy.invalid` on other keys | `{"restrict":{"grants":[…]}}` |
| `traceparent` | HTTP header | W3C `00-<trace>-<span>-<flags>` | none | correlating traces | request | the request's `trace_id`; echoed on the response | invalid values are ignored | `00-4bf9…4736-00f0…02b7-01` |
| `secret … from env "VAR"` | env read by an operation | any env var name | — | operation uses it | request | value bound to the `for` origin | needs `allow_env` grant | `FIXTURE_TOKEN` |
| `client_secret env "VAR"` | env read by OAuth | env var name | — | confidential OAuth client | profile | client secret for the token endpoint | needs `allow_env` grant | `CRM_CLIENT_SECRET` |

## Task-Oriented Workflows

The volumes hold full procedures. The three journeys most readers need first:

```text
 A. FIRST RUN (MAN-2026-0002)
 [write app.rivet] -> rivet check -> "ok: N operations" -> rivet request ID --data '{…}' -> envelope, status "ok" (stdout)
                         |                                        |
                         +-> error[syntax.*] exit 2 -> fix line    +-> envelope, status "error" (stderr), exit 2/3/4/5/6 -> error.code

 B. GRANT EFFECTS SAFELY (MAN-2026-0005)
 rivet io (review) -> rivet policy generate --output policy.json (draft) -> edit/review -> rivet io --check-policy
        exit 0 "N allowed" -> rivet request …          exit 3 "k denied" -> add grant or accept denial

 C. SERVE (MAN-2026-0006)
 rivet serve --file app.rivet --listen 127.0.0.1:8080 -> stderr receipt JSON -> curl /v1/request | SSE | /v1/ws | /mcp
                         |
                         +-> non-loopback without serve.auth -> serve.auth_required exit 2 -> add bearer tokens
```

### UI Procedure

Rivet has no graphical user interface. Every interaction is a CLI command, an HTTP/WS/MCP request, a Rust call or a
C call, each shown with exact input and output in the volumes. The only editor integration (0.2.0) is syntax
highlighting in VS Code and other TextMate editors ([MAN-2026-0010](man-2026-0010-editor-support-and-highlighting.md)).

## Complete CLI Reference

See [MAN-2026-0004](man-2026-0004-cli-reference.md): every command, flag, default, exit code and a success and
failure example for each.

## Complete API and Event Reference

Routes, frames and tools are summarized in [MAN-2026-0006](man-2026-0006-serving-and-surfaces.md) and
specified in [API-2026-0001](../api/api-2026-0001-http-rest-sse-polling.md) (REST, SSE, polling),
[API-2026-0002](../api/api-2026-0002-websocket-rivet-v1.md) (WebSocket),
[API-2026-0003](../api/api-2026-0003-mcp-server-tools.md) (MCP tools),
[API-2026-0004](../api/api-2026-0004-rust-library.md) (Rust) and [API-2026-0007](../api/api-2026-0007-c-abi.md) (C ABI).
The shared shapes are [API-2026-0006](../api/api-2026-0006-envelopes.md); the error registry is
[API-2026-0005](../api/api-2026-0005-error-registry.md).

## Errors and Recovery Reference

Every error, on every surface, is the same response envelope with `status: "error"` (captured 2026-09-29 from the
0.2.0-rc; IDs differ on every run):

```text
$ rivet --file docs/demos/01-catalog/app.rivet request demo.add --data '{"a":"two"}'        # stderr, exit 2
{"request_id":"req_101e9dfdd2","trace_id":"tr_101e9dfdd2","operation":"demo.add","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.type","message":"parameter `a` must be an integer, got text","retryable":false,"operation_id":"demo.add","details":{"field":"a"}},"effects":"none","data_count":0}
```

| Error kind / code | Surface | Cause | User-Visible Result | Recovery | Retry Safe | Related Feature |
|---|---|---|---|---|---|---|
| `syntax.*`, `check.*`, `registry.duplicate_id`, `docs.*` | CLI | source does not parse or check (e.g. `syntax.else_if`, `check.unknown_function`) | `error[code]` with a caret at the line and a hint, exit 2 | fix the line shown | no | language |
| warnings `docs.undeclared_error`, `check.unguarded_result`, `check.module_policy_ignored` | CLI `check` | likely mistakes that still compile; a module's own `policy.json` | `warning[…]` on stderr, exit 0 | declare the code / guard with `if NODE.status == "succeeded"` / grant in the entry policy | — | language, modules |
| warnings `deprecated.params`, `deprecated.input`; HTTP `deprecation: true` | CLI, HTTP, MCP | 0.1.0 input keys or `--params` | request still succeeds | send `{operation, data}` / `--data` | — | envelopes |
| `syntax.global`, `check.global_*`, `syntax.import`, `check.import_*` | CLI, load | malformed or non-constant global; bad, cyclic, duplicate or colliding import | `error[code]` with caret, exit 2 | fix the line | no | globals, modules |
| `not_found.import`, `permission.import_outside_root`, `limit.imports` | CLI, load, `rt.load` | missing module file; path escapes the root; > 256 files or depth > 16 | exit 4 / 3 / 5 | fix the path; flatten | no | modules |
| `validation.input_envelope`, `validation.pretty_stream` | all / HTTP | input not an object, mixed key and alias, bad field type; `?pretty=true` with SSE | HTTP 422 / 400, exit 2 | send a valid envelope; drop `pretty` for streams | no | envelopes |
| `unsupported.feature` | all | the bundle (or `rivet serve`) needs a Cargo feature this build lacks (`details.feature`) | HTTP 501, exit 5, at load | rebuild with the feature | no | features |
| `validation.ffi_argument`, `internal.panic` | C | NULL/invalid/freed argument; a panic caught at the boundary | error envelope string, `RIVET_ERROR` | fix the call ([MAN-0009](man-2026-0009-c-abi-and-ffi.md)) | no | C ABI |
| `validation.*` (`validation.type`, `.required`, `.enum`, `.max`, `.unknown_field`, `.usage`), `policy.invalid`, `stream.*`, `serve.auth_required` | all | bad params, usage or configuration | HTTP 422 (400 for malformed JSON), exit 2 | correct the input | no | params, policy, serve |
| `auth.required`, `auth.invalid` | serve | missing/unknown bearer token | HTTP 401 + `www-authenticate: Bearer`, exit 3 | send the right token | no | serve auth |
| `permission.denied`, `file.hardlink_refused` | all | policy, host ceiling, `restrict`, principal or secret binding denies the effect/operation | HTTP 403, exit 3 | add a grant or principal entry, or accept | no | policy |
| `not_found.*` | all | unknown operation, file, session, trace, profile | HTTP 404, exit 4 | check the ID/path | no | all |
| `conflict.*` (`conflict.already_exists`, `conflict.exists`, `conflict.ref`) | all | state conflict (e.g. `file create` target exists) | HTTP 409, exit 4 | choose another target or `update` | no | files, generate |
| `limit.*` | all | budget exceeded (`limit.concurrency`, `limit.buffered_bytes`, WS refs) | HTTP 429, exit 5 | back off and retry | yes, after backoff | limits |
| `timeout.*` (`timeout.request`, `.poll`, `.scope`) | all | deadline reached | HTTP 504, exit 6 | raise `--timeout`/`deadline_ms` or fix the dependency | caller decides | deadlines |
| `http.status`, `connection.*`, `dns.*`, `tls.*`, `protocol.*`, `process.*`, `udp.*`, `quic.*`, `grpc.*`, application codes | all | dependency failed or declared `fail` | HTTP 502 (500 for some), exit 5 | inspect `details`; retry only if replay-safe | only if replay-safe | protocols |
| `unsupported.*` (e.g. `unsupported.sandbox_backend` on Linux) | all | feature/platform not available in this build | HTTP 501, exit 5 | use the supported alternative named in the message | no | limitations |
| `output.invalid` | all | result violates the declared output | HTTP 500, exit 5 | fix the operation or its declaration | no | outputs |
| `cancelled.*`, `consumer.stop` | all | caller cancelled (Ctrl-C, cancel frame, idle lease, server shutdown) or a library sink stopped | exit 130; terminal cancel event | re-run if wanted | no | sessions |
| `io --check-policy` / `policy explain ID --params` result | CLI | a reachable site (or a concrete target) is denied | table printed, exit 3 | add grants or remove the site | — | I/O manifest |
| inspection incomplete | CLI | `io --strict` found dynamic sites; `policy generate` left review items | exit 7 | review the dynamic targets by hand | — | I/O manifest |
| `io --check-files` result | CLI | a needed file is not permitted (3) or missing (4) | listing printed, exit 3/4 | create the file or grant `stat` | — | needs |

```text
exit codes:  0 ok   2 syntax/validation/config   3 permission/auth   4 not_found/conflict
             5 dependency/runtime/unsupported/output_invalid/limit   6 timeout   7 inspection incomplete
             130 cancelled
```

## Examples and Demos

The thirteen sample folders under [`docs/demos/`](../demos/README.md) are the executable demos; demos
`14-globals`, `15-ffi`, `16-editor` and `17-modules` (DEMO-2026-0016…0019) are being added for 0.2.0. The 0.2.0
examples in this manual set were captured on 2026-09-29 from `cargo build --release --features cli`
(`target/release/rivet`, source at `6f9943f`, macOS 26.4) in those folders or in a scratch copy when the command writes
files; 0.1.0-only material was verified at commits `f40d4aa`/`829ca43`. Request, trace, session and MCP-session IDs
(`req_…`, `tr_…`, `ses_…`, `mcp_…`, `auth_…`) are generated per run and will differ on your machine. Version fields
read `0.1.0` until the 0.2.0 release commit.

## Operations, Observability and Maintenance

- `rivet serve` prints one startup **receipt** line (JSON) on stderr: listen address, mounted surfaces, auth
  type, `catalog_version` and `policy_hash`. Keep it in your logs to know exactly what was served.
- After the receipt, every request adds one **access-log line** on stderr
  (`{time, surface, method, route, principal, operation, status, duration_ms}`, plus `"deprecated":1` for 0.1.0
  input keys; never params, bodies or tokens). Count `"deprecated":1` to see which clients still need migrating.
- `GET /v1/health` answers the `rivet.health` envelope, `data` = `{"status":"ok","catalog_version":…,"version":…}`
  (unauthenticated on loopback).
- SIGINT and SIGTERM **drain**: stop accepting, cancel in-flight requests and sessions (handles close within 5 s),
  exit 0.
- Broker decisions are kept in an **in-memory trace store per process**; read them with
  `rivet --endpoint URL trace show REQ` while the server runs. A local `rivet trace show` starts a new process and
  therefore finds nothing (`not_found.trace`).
- Operating guidance (bind, auth, limits, upgrade) is in [OPS-2026-0001](../operations/ops-2026-0001-operating-rivet-serve.md);
  token rotation and policy roll-out are in [RUN-2026-0001](../runbooks/run-2026-0001-rotate-serve-bearer-tokens.md) and
  [RUN-2026-0002](../runbooks/run-2026-0002-roll-out-policy-change.md).

## Edge Cases

- `file delete` with `missing ok` succeeds on an absent file and reports `effects: "none"` (nothing changed; since
  commit `2a751ab`).
- `rivet outputs` / `describe` show the `emits` and `receives` descriptions, and the item JSON Schemas carry them
  as `description` (REST, MCP, library).
- Cancelling a polling session after it already finished returns its terminal state (for example
  `"state":"succeeded"`) with HTTP 200; a cancel that arrives before a racing completion wins (`cancelled`).
- A variable named like a codec (`text`, `json`, `bytes`) never replaces the codec keyword:
  `file append P text text` appends the variable `text`.
- `for` loop variables, `catch` `error` and `map` items are block-local; ordinary assignments inside a block are
  visible after it (assignments are operation-scoped).
- `version prefer [3, 2]` on an `http://` URL skips HTTP/3 (it needs `https://`) and speaks HTTP/2 cleartext, which
  fails against HTTP/1-only servers (`connection.http`).

## Failure Modes, Recovery and Rollback

| Failure | What you see | Recovery / rollback |
|---|---|---|
| Bad `policy.json` | `error[policy.invalid]: policy.json /grants/0/access/0: …`, exit 2; nothing runs, `serve` does not start | fix the key at the JSON pointer; keep the previous file for rollback |
| Effect denied at run time | `permission.denied`, exit 3, `effects: "none"` (nothing happened) | add the grant shown in `details`, re-run |
| Dependency fails mid-operation | exit 5 with `effects` = `committed`, `partial` or `unknown` | read `effects` before retrying; Rivet never replays a mutation for you |
| Server stop | SIGINT/SIGTERM drains `rivet serve` (exit 0); in-flight requests and sessions end `cancelled` and are cleaned up | restart; sessions are in memory and do not survive |

## Security and Compatibility

- Deny-by-default; `deny` entries override `grants`; private, loopback and link-local addresses (including
  `169.254.169.254`) are denied for every scheme (`http`, `https`, `ws`, `tcp`, `udp`, `quic`) unless a grant
  names that address literally. A `"*"` target does not lift this.
- Commands are argv-only: no shell strings (`/bin/sh -c` → `unsupported.shell`) and no PATH lookup.
- Bearer tokens are stored in `policy.json` as SHA-256 hashes; the CLI reads the client token only from
  `--token-file`.
- OAuth tokens are never printed or returned; `auth status` is sanitized.
- Compatibility: 0.2.0 changes every **output** to the response envelope (breaking for readers); **inputs** stay
  compatible through 0.2.x (`id`/`params` deprecated, refused in 0.3.0). Follow
  [MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md); roll back by pinning `v0.1.0`.
  WS `rivet.v1` and MCP protocol `2025-11-25` keep their names.
- The C ABI host is a library principal: it has the same authority as a Rust host (MAN-2026-0009). Globals can
  never hold secrets (`check.global_not_constant`). Modules are confined to the runtime root and run under the
  loader's single policy.

## Known Limitations

These are **all** the known limitations of the 0.2.0 release candidate (each volume repeats the ones that affect its
tasks). Each row was checked against the source at `6f9943f`.

```text
  what 0.2.0 does NOT do                                   what you do instead
  ─────────────────────────────────────────────            ────────────────────────────────────────────
  run on Windows                                      ──►   macOS or Linux (INC-2026-0011)
  sandbox processes on Linux (gated)                  ──►   run process-spawning operations on macOS,
                                                            or without a policy.json on Linux
  import from a URL, or hot-reload an import           ──►   local files under the root; rt.load at run time
  serve over mTLS                                     ──►   bearer auth behind a TLS-terminating proxy
  `finally`                                           ──►   `with` blocks for cleanup; explicit catch paths
  Stage C forms (watch, pipes, TCP TLS, reconnect …)  ──►   the Stage A/B alternatives named in the error
```

| Area | Limitation | Consequence / workaround |
|---|---|---|
| platforms | **macOS and Linux are supported; Windows is not** (0.2.0 dropped it from CI after thirteen Windows-only failures, W-01…W-13, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)). | Build and run on macOS or Linux. |
| sandbox | **The process sandbox is active on macOS only.** macOS uses Seatbelt (`rivet.capabilities` → `"sandbox":{"backend":"macos-seatbelt","status":"active"}`). On Linux, Landlock + seccomp is implemented but **gated** ("gated until the T-08 conformance suite passes on Linux CI (kernel >= 6.12, Landlock ABI 6)"): a sandboxed spawn is refused with `unsupported.sandbox_backend` (exit 5, HTTP 501). | Run process-spawning operations on macOS, or on Linux without a `policy.json` (no sandbox is requested then — the child is unconfined). |
| language | **No URL imports and no hot reload**: `import` takes a local path under the runtime root (`permission.import_outside_root` otherwise); a changed module is picked up only by reloading the bundle or by `rt.load` under a new alias. | Vendor shared modules into the bundle root. |
| language | **No `finally`**: `try … catch … finally` is `syntax.unknown_statement` (exit 2). | Put cleanup in `with` blocks (they always close their handles) or repeat it on both paths. |
| language | **Stage C forms are refused** at run time with `unsupported.*` (exit 5): `with file watch` (`unsupported.stage_c`), `with pipe` / named pipes (`unsupported.adapter`), `tls` on TCP/Unix (`unsupported.tcp_tls`), `reconnect` (`unsupported.reconnect`), `interactive true` processes (`unsupported.interactive`), serve mTLS, custom (non-gRPC) protobuf codecs. `rivet.capabilities` lists them with `"stage":"C"`. | Use one-shot `file` verbs or `with file open`, `http`/`websocket`/`tcp` without TLS on raw sockets, argv processes. |
| build | **Compiled-out features refuse, never degrade**: a build without `grpc`, `quic` or `oauth` fails any bundle that uses them at load (`unsupported.feature`), and a binary without `serve` refuses `rivet serve`. | Build with the default features, or check `rivet.capabilities` → `build_features`. |
| distribution | **No crates.io crate** (the `capy-core` git dependency blocks `cargo publish`). | Depend on the git tag `v0.2.0`. |
| serve | **mTLS is not supported**: `serve.auth` type `mtls` refuses to start (`unsupported.serve_mtls`, exit 5). | Use `bearer` behind a TLS-terminating proxy. |
| policy | **`approved.overlaps` is unused**: the key is accepted and validated as a string list but nothing reads it. | Leave it empty. |
| serve | **The `*` principal pattern matches `rivet.auth.*`**: `serve.principals` `["*"]` lets that principal call `rivet.auth.begin/complete/status/disconnect/cancel`; what they may do is then governed only by `allow_auth` grants. (Sensitive built-ins still need an exact entry.) | List operations explicitly (`demo.*`) instead of `*` for principals that must not manage OAuth accounts, and keep `allow_auth` narrow. |
| MCP client | **An MCP 401 invalidates the lease without retry**: when an HTTP MCP connector answers 401 to a bearer, Rivet drops the cached token and returns `http.status` (401); the *next* call reacquires. | Retry the call once at the caller. |
| HTTP/3 | **No Alt-Svc discovery**: HTTP/3 is used only when requested (`version 3` or `version prefer [3, 2]`). | Request it explicitly. |
| transports | **No connection pooling**: every attempt opens its own connection (HTTP one connection per attempt). | Expect a handshake per request. |
| audit | **No persistent trace store**: traces live in memory in the process that ran the request (bounded). A local `rivet trace show/export` starts a new process and finds nothing. | Read traces from the running server (`--endpoint … trace show`) or the embedding `Runtime`. |
| MCP server | **No resources, resource templates or prompts, and no legacy HTTP+SSE MCP transport** on `rivet serve` (`resources/list` → `-32601`). | Expose operations as tools; use Streamable HTTP `/mcp` or `--stdio`. |

0.1.0's limitation "no `import` form" is gone: 0.2.0 has file modules ([MAN-0003 §Modules](man-2026-0003-language-guide.md#modules-import)).
The defects found while verifying the 0.1.0 fix batch were fixed in commit `2a751ab`
([INC-2026-0007](../incidents/resolved/inc-2026-0007-demo-verification-defects.md)); the Windows port failures are
catalogued in the active [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md).

## Troubleshooting References

- Troubleshooting records: [docs/troubleshooting/index.md](../troubleshooting/index.md).
- Error code reference: the table above, and each volume's "Errors and Recovery" section.

## Glossary

| Term | Definition |
|---|---|
| access verb | The action a site performs within a capability (`read`, `create`, `connect`, `bind`, `exec`, `call`, `use`, …). |
| bootstrap I/O | Runtime-internal reads (the bundle, `policy.json`, CA bundle, resolver, tzdata, connector descriptors) listed by `io --include-bootstrap`, not governed by policy. |
| capability | A grant family: `allow_read`, `allow_write`, `allow_delete`, `allow_network`, `allow_listen`, `allow_exec`, `allow_env`, `allow_pipe`, `allow_unix`, `allow_mcp`, `allow_grpc`, `allow_auth`, `allow_credentials`. |
| catalog_version | `sha256:` of the compiled bundle, shown in the serve receipt and session receipts. |
| Completion | Terminal success object of a request. |
| effects | `none`, `committed`, `partial` or `unknown` — whether the request changed anything outside Rivet. |
| effect_id | Stable ID of one I/O site, `OPERATION#N`. |
| knowledge | How well a site's target is known statically: `exact`, `glob`, `dynamic` or opaque. |
| principal | Authenticated caller name; `local` for CLI and library. |
| ref | Client-chosen request handle on a WebSocket connection. |
| snapshot | A reviewed MCP server description file (`format "rivet.mcp.snapshot/1"`) approved by sha256 in `policy.json`. |

## Version Applicability

| Feature / Interface | Introduced | Changed | Deprecated / Removed | Applicable Environment |
|---|---|---|---|---|
| Language (operations, params, outputs, control flow, DAG, resources) | 0.1.0 | — | — | all |
| CLI commands and exit codes | 0.1.0 | 0.2.0: envelopes, `--data`, `--input`, `--pretty`, `highlight` | `--params` deprecated 0.2.0, removed 0.3.0 | macOS, Linux |
| Wire output (every surface) | 0.1.0 | 0.2.0: response envelope (breaking) | 0.1.0 shapes removed in 0.2.0 | all |
| Input keys `id`/`params` | 0.1.0 | 0.2.0: `operation`/`data` canonical | deprecated 0.2.0, removed 0.3.0 | all |
| Globals, modules | 0.2.0 | — | — | all |
| Cargo package `rivet-runtime`, features, facade | 0.2.0 | package `rivet` renamed | `rivet::domain::…` paths removed | embedded |
| C ABI `librivet` (ABI 1) | 0.2.0 | — | — | macOS, Linux |
| `rivet highlight`, grammar, `.vsix` | 0.2.0 | — | — | editors, CLI |
| Windows | 0.1.0 (partial) | — | unsupported from 0.2.0 (INC-2026-0011) | — |
| `policy.json` schema v1 | 0.1.0 | — | — | all |
| `rivet io`, `policy generate` | 0.1.0 | — | — | all |
| `rivet serve` REST/SSE/polling/WS/MCP, bearer auth | 0.1.0 | — | — | server |
| Process sandbox (Seatbelt active; Linux gated) | 0.1.0 | — | — | active on macOS; gated on Linux |
| Rust library `Runtime` | 0.1.0 | 0.2.0: `call`, `load`, facade | — | embedded |

## Related Features

Every feature above links to its task section. Cross-cutting: policy (MAN-2026-0005) governs every effect;
surfaces (MAN-2026-0006) expose every operation.

## Related Documents

- Plans: [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) (rows D-34 … D-41), [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) (rows D-20, D-46)
- 0.2.0: [PROP-2026-0002](../proposals/implemented/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md), [MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md), [API-2026-0006](../api/api-2026-0006-envelopes.md), [API-2026-0007](../api/api-2026-0007-c-abi.md), [MAN-2026-0009](man-2026-0009-c-abi-and-ffi.md), [MAN-2026-0010](man-2026-0010-editor-support-and-highlighting.md), [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)
- Approved design: [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)
- Numbered examples: [REF-2026-0002](../references/ref-2026-0002-language-and-usage.md)
- System documents: [SYS-2026-0001](../system/components/sys-2026-0001-compiler-and-catalog.md),
  [SYS-2026-0002](../system/runtime/sys-2026-0002-execution-scopes-and-dag.md),
  [SYS-2026-0003](../system/components/sys-2026-0003-policy-broker-and-io-manifest.md),
  [SYS-2026-0004](../system/components/sys-2026-0004-surfaces-and-serve.md)
- Manual index: [index.md](index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 6 | 2026-09-30 | Claude | v0.2.1 patch (PLAN-2026-0002 TASK-097): version strings, install tag v0.2.1; INC-2026-0013 behaviour where described. |
| 5 | 2026-09-30 | Claude | v0.2.0 release (PLAN-2026-0002 TASK-091): version strings and current-release wording updated to 0.2.0. |
| 1 | 2026-09-28 | Claude | Initial root manual for the implemented 0.1.0 build: catalogue, concepts, limitations, glossary; every command verified against 0.1.0-dev commit f40d4aa. |
| 2 | 2026-09-28 | Claude | Fix batch through 829ca43 and 2a751ab (TASK-095 doc part): What's New and catalogue rows for `else`, `with file open`, `check` warnings, `graph`, `trace export`, `rivet.capabilities`, `restrict`, ceiling, health/access log/drain, `traceparent`, secret taint on every sink, `if_version`, buffered-bytes budget; configuration and error rows; new **Known Limitations** chapter listing exactly the current limitations; known defects linked. |
| 3 | 2026-09-29 | Claude | 0.2.0 (D-20, D-46): **What's New in 0.2.0** (0.1.0 table kept as "Introduced in 0.1.0"; 0.2.0 marked in progress); catalogue rows for envelopes, input envelope and `--data`/`--input`, deprecation, pretty output, globals, modules, the Cargo dependency with features, the facade, the C ABI and highlighting, with why/when and demo paths 14–17; reading path adds MAN-0009/0010 and MIG-0001; concepts, mental model and configuration for envelopes, globals, modules and features; error rows for the new codes; Known Limitations: removed "no `import`", added macOS/Linux support and Windows unsupported (INC-2026-0011), Linux sandbox gated (`unsupported.sandbox_backend`), no URL imports / hot reload, compiled-out features, envelope deviations; version applicability. |
| 4 | 2026-09-29 | Claude | INC-2026-0012: removed the WS `--timeout` limitation and the envelope known-deviation row (both fixed). |
