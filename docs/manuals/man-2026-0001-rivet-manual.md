---
document_id: MAN-2026-0001
title: "Rivet manual"
document_type: manual
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, registry, execution, files, transports, http, datagrams, quic, grpc, connectors, auth, policy, audit, sessions, serve, poll, ws, mcp, cli, library]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, server, embedded]
audience: [developers, operators, integrators, reviewers]
scope: Root book of the Rivet 0.1.0 manual set — purpose, reading order, what is new, goals and boundaries, concepts, mental model, the feature catalogue (every feature, why to use it, where the instructions are), configuration, limitations, glossary and version applicability.
reason: PLAN-2026-0001 row D-34 (DOCUMENTATION.md §30) — the manual is the canonical current-state book; this root answers "what can I do, why, and where are the instructions?" for the implemented 0.1.0 build.
related_documents: [PLAN-2026-0001, PROP-2026-0001, REF-2026-0002, MAN-2026-0002, MAN-2026-0003, MAN-2026-0004, MAN-2026-0005, MAN-2026-0006, MAN-2026-0007, MAN-2026-0008, API-2026-0001, API-2026-0002, API-2026-0003, OPS-2026-0001, DEMO-2026-0001, DEMO-2026-0011, DEMO-2026-0012]
supersedes: null
superseded_by: null
tags: [rivet, manual, feature-catalogue, current-state]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.1.0-dev (commit f40d4aa)"
---

# Rivet manual

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** language, registry, execution, files, transports, http, datagrams, quic, grpc, connectors, auth, policy, audit, sessions, serve, poll, ws, mcp, cli, library

## Purpose

Rivet runs **described operations** written in `.rivet` files. You declare an operation once — its ID, parameters,
declared output, streamed items and errors — and the same operation is callable from the `rivet` CLI, over HTTP
(REST, SSE, polling), over WebSocket, over MCP and from Rust. Every effect the operation performs (files,
network, processes, credentials, MCP and gRPC calls) goes through one policy broker that reads `policy.json`
and denies anything not granted.

This book is the **current-state manual for the implemented 0.1.0 build** (verified against
`rivet 0.1.0-dev`, commit `f40d4aa`). It explains what you can do, why you would do it, and where the exact,
copy-pasteable instructions live. Volumes MAN-2026-0002 … MAN-2026-0008 hold the task procedures.

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
     MCP  ──────┤              └─────────────────────┘              └─► remote MCP servers
     Rust lib ──┘                 same result, same errors, same exit/status codes everywhere
```

## Reading Order

```text
 new user          ──► MAN-0002 install + quickstart ──► MAN-0003 language ──► MAN-0004 CLI reference
 administrator     ──► MAN-0005 policy.json + I/O manifest ──► MAN-0006 serving (auth, principals)
 integrator        ──► MAN-0008 protocols and connectors (HTTP/1-3, WS, TCP, UDP, QUIC, gRPC, OAuth, MCP)
 Rust developer    ──► MAN-0007 embedding the library
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
| [Protocols and connectors](man-2026-0008-protocols-and-connectors.md) | MAN-2026-0008 | pick and configure a transport, know the grants it needs and its errors |

## What's New in 0.1.0

0.1.0 is the **first implemented release**; everything below is new. Detailed history belongs to the release
document `REL-0.1.0` (created in PLAN-2026-0001 P5).

| Addition | Task instructions | Demo |
|---|---|---|
| One `.rivet` file declares many described operations with typed params, declared outputs, `emits`, `receives` and declared errors | [MAN-2026-0003](man-2026-0003-language-guide.md) | [01-catalog](../demos/01-catalog/README.md) |
| CLI: `request`, `list`, `describe`, `outputs`, `check`, `io`, `policy explain/generate`, `trace show`, `auth …`, `connectors sync`, `serve` | [MAN-2026-0004](man-2026-0004-cli-reference.md) | [01-catalog](../demos/01-catalog/README.md) |
| `policy.json` only (beside the entry file, or `--policy PATH`); absent = deny-by-default; `access` verbs narrow grants | [MAN-2026-0005](man-2026-0005-policy-and-io-manifest-guide.md) | [11-sandbox](../demos/11-sandbox/README.md) |
| Generated I/O manifest (`rivet io`), `--check-policy`, `--needs`, `--check-files`, and `policy generate` | [MAN-2026-0005](man-2026-0005-policy-and-io-manifest-guide.md) | [11-sandbox](../demos/11-sandbox/README.md) |
| One `rivet serve` mounts REST, SSE, polling, WebSocket (`rivet.v1`) and MCP (`/mcp`); `--stdio` for MCP only | [MAN-2026-0006](man-2026-0006-serving-and-surfaces.md) | [01-catalog](../demos/01-catalog/README.md) |
| Bearer authentication, per-principal operation lists, `serve.surfaces` | [MAN-2026-0006](man-2026-0006-serving-and-surfaces.md) | [01-catalog](../demos/01-catalog/README.md) |
| `--endpoint URL` thin client with `--token-file` | [MAN-2026-0006](man-2026-0006-serving-and-surfaces.md) | [01-catalog](../demos/01-catalog/README.md) |
| Rust embedding: `rivet::Runtime::builder()` | [MAN-2026-0007](man-2026-0007-embedding-library.md) | [12-library](../demos/12-library/README.md) |
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
   GOALS (what Rivet is for)                          NON-GOALS / BOUNDARIES (0.1.0)
   ───────────────────────────────────────            ─────────────────────────────────────────────
   one declared operation, every surface              not a general-purpose programming language
   typed params + declared, validated outputs         no implicit I/O: every effect is a named form
   deny-by-default effects from one policy.json       no command-line or environment grants
   reviewable I/O before running (rivet io)           no shell strings, no PATH lookup for commands
   explicit protocol behaviour (no silent             no silent HTTP redirect/version fallback,
     redirects, fallbacks or replays)                   no automatic replay of mutations
   bounded concurrency, deadlines and cleanup         no mTLS on `serve`, no persistent trace store
   same errors / exit codes / HTTP statuses           no graphical UI
```

Rivet guarantees that **script-initiated effects** pass the broker. It does not sandbox the host that embeds it,
and it cannot see what a spawned child process does except through the OS sandbox (see MAN-2026-0008).

## Concepts

| Concept | Meaning |
|---|---|
| Bundle | The entry `.rivet` file named with `--file` plus its `policy.json`. The CLI requires `--file` for local commands. |
| Operation / pipeline | A named, typed callable (`operation demo.add … end`). `pipeline` is the same thing, used for composition. |
| Header | Lines before the first body statement, in the fixed order `name, description, private, param, output, emits, receives, error`. |
| Declared output | The `output` type (and `field`s) the result must match; a mismatch fails `output.invalid` (exit 5). |
| Emits / receives | `emits T` declares streamed data items; `receives T` declares live input read with `for item in incoming`. |
| Completion | The terminal result object `{request_id, trace_id, result, data_count, effects}`. |
| Envelope | One streamed item `{request_id, trace_id, seq, type: "data", data}` or the terminal `type: "result"`. |
| Effect site | One place in source that performs I/O; listed by `rivet io` with kind, access verb, target and capability. |
| Capability / access verb | The grant family (`allow_read`, `allow_network`, …) and the verb within it (`read`, `create`, `connect`, …). |
| Policy broker | The single component that authorizes every effect against `policy.json`; deny entries win. |
| Principal | The authenticated caller of a serve surface (`local` for CLI and library). |
| Session | A live request with an ID you can poll, feed input to, finish or cancel (polling, WebSocket refs, `rivet.sessions.*`). |
| Connector | A declared MCP or gRPC endpoint (`connector NAME mcp|grpc … end`). |
| Auth profile | A declared OAuth 2.0 client (`auth NAME oauth2 … end`), used by `auth PROFILE account "A"`. |

## Architecture and Mental Model

```text
  app.rivet ──parse (Capy)──► lower + check ──► CompiledProgram ──► Registry (catalog, schemas)
                                  │ errors: syntax.*, check.*, registry.duplicate_id  (exit 2)
  policy.json ──strict schema v1──► Policy ─────────────────────────► Policy broker
                                  │ errors: policy.invalid (exit 2, JSON pointer named)

  request(ID, params, principal)
      │
      ├─ principal allowed? (serve.principals) ── no ─► permission.denied (403 / exit 3)
      ├─ params valid? (types, required, min/max/enum, unknown fields) ── no ─► validation.* (422 / exit 2)
      ├─ run body in a scope with a deadline (default 30 s)
      │     each effect ─► broker: grant? deny? private range? ── no ─► permission.denied
      │                            └─ yes ─► adapter (file/http/udp/…) ─► trace event
      ├─ result matches declared output? ── no ─► output.invalid (500 / exit 5)
      └─ Completion {request_id, trace_id, result, data_count, effects}
```

Four facts to keep in mind:

1. **Nothing runs until it compiles.** `rivet check` compiles without running anything; every other local command
   compiles first too, so a syntax error blocks `request`, `list` and `serve` alike.
2. **Policy is a file, never a flag.** No `policy.json` beside the entry file means every new effect is denied
   while pure operations still run.
3. **One dispatcher.** The CLI, every serve surface and the library call the same `request` path, so results,
   error codes and exit codes/HTTP statuses match.
4. **Scopes clean up.** A `with` block, a `scope`, a DAG or a request deadline closes its handles (in reverse
   order, within 5 s) whether the body returns, fails or is cancelled.

## Installation and Setup

Build from source with Cargo (Rust 1.90.0, pinned by `rust-toolchain.toml`); see
[MAN-2026-0002](man-2026-0002-installation-and-quickstart.md). There is no published binary or crate in 0.1.0.

```bash
cargo build                          # produces target/debug/rivet
target/debug/rivet --version         # rivet 0.1.0-dev
```

## Feature Catalogue

"Since" is the first version that ships the feature. Surfaces: **CLI** = local `rivet` with `--file`;
**Remote** = `rivet --endpoint`; **HTTP** = REST/SSE/polling; **WS** = WebSocket `rivet.v1`; **MCP** = `/mcp`
or `--stdio`; **Lib** = Rust `Runtime`.

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
| Control flow: `if`, `for`, `while`, `break`, `iterate max N`, `return`, `+=` | Ordinary logic with bounded loops | language | 0.1.0 | [MAN-0003 §Control flow](man-2026-0003-language-guide.md#control-flow) | [01-catalog](../demos/01-catalog/README.md) |
| `try` / `catch error [kind K|code "C"]` | Recover from a typed failure | language | 0.1.0 | [MAN-0003 §try/catch](man-2026-0003-language-guide.md#recover-with-try-and-catch) | [03-http](../demos/03-http/README.md) |
| `map … limit N … yield`, `poll every … timeout … until … yield` | Bounded fan-out; wait for a remote job | language | 0.1.0 | [MAN-0003 §map and poll](man-2026-0003-language-guide.md#map-poll-and-iterate) | [05-dag](../demos/05-dag/README.md) |
| `dag` with `node … after […]`, `fail fast`/`fail independent` | Run a dependency graph and inspect per-node status | language | 0.1.0 | [MAN-0003 §DAG](man-2026-0003-language-guide.md#run-a-dag) | [05-dag](../demos/05-dag/README.md) |
| `concurrent` / `task`, `scope timeout` | Structured parallel work joined before exit; group deadlines | language | 0.1.0 | [MAN-0003 §Concurrency](man-2026-0003-language-guide.md#concurrent-tasks-and-scopes) | [10-grpc](../demos/10-grpc/README.md) |
| `with` resource blocks | Own a socket/stream/process for exactly one block | language | 0.1.0 | [MAN-0003 §Resources](man-2026-0003-language-guide.md#own-resources-with-with) | [04-streaming](../demos/04-streaming/README.md) |
| `secret NAME from env "VAR" for "ORIGIN"` | Use a credential bound to one destination origin | language | 0.1.0 | [MAN-0003 §Secrets](man-2026-0003-language-guide.md#use-secrets) | [03-http](../demos/03-http/README.md) |
| Pure helpers `length`, `keys`, `text`, `base64.encode/decode`, `xml.element` | Compute values without I/O | language | 0.1.0 | [MAN-0003 §Expressions](man-2026-0003-language-guide.md#values-and-expressions) | — |
| `rivet check [--strict-docs]` | Compile and lint without running; enforce descriptions in CI | CLI | 0.1.0 | [MAN-0004 §check](man-2026-0004-cli-reference.md#rivet-check) | [01-catalog](../demos/01-catalog/README.md) |

### Effects (what an operation can touch)

| Feature | Why / When to Use It | Supported Surfaces | Since Version | Instructions | Demo |
|---|---|---|---|---|---|
| File verbs: `read`, `list`, `stat`, `create`, `update`, `write`, `append`, `delete` (`missing ok`), `copy`, `move` | Explicit intent per write; `create` never overwrites, `update` never creates | all | 0.1.0 | [MAN-0003 §Files](man-2026-0003-language-guide.md#files) | [02-file-crud](../demos/02-file-crud/README.md) |
| Hard-link refusal | Stop writes through a second name of a file | all | 0.1.0 | [MAN-0003 §Files](man-2026-0003-language-guide.md#files) | [02-file-crud](../demos/02-file-crud/README.md) |
| HTTP/1.1 and HTTP/2 client | Call REST APIs with explicit methods, bodies, decoding, retries and redirects | all | 0.1.0 | [MAN-0008 §HTTP](man-2026-0008-protocols-and-connectors.md#http11-and-http2) | [03-http](../demos/03-http/README.md) |
| HTTP response streams: `stream sse|jsonl|lines|bytes` | Consume model/log streams item by item | all | 0.1.0 | [MAN-0008 §Streams](man-2026-0008-protocols-and-connectors.md#http-response-streams) | [04-streaming](../demos/04-streaming/README.md) |
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
| `rivet policy explain [ID]` | See the effective policy and each site's decision | CLI | 0.1.0 | [MAN-0005 §Explain](man-2026-0005-policy-and-io-manifest-guide.md#explain-the-effective-policy) | [11-sandbox](../demos/11-sandbox/README.md) |
| `rivet io` manifest (`--by`, `--kind`, `--access`, `--format`, `--include-bootstrap`) | Review every URL, path and verb before running anything | CLI, Remote, HTTP `/v1/io`, MCP `rivet.io`, Lib | 0.1.0 | [MAN-0005 §I/O manifest](man-2026-0005-policy-and-io-manifest-guide.md#review-io-with-rivet-io) | [11-sandbox](../demos/11-sandbox/README.md) |
| `io --check-policy`, `--strict` | Gate CI on "every site allowed" and "nothing dynamic" | CLI, Remote | 0.1.0 | [MAN-0005 §Check](man-2026-0005-policy-and-io-manifest-guide.md#check-the-manifest-against-policy) | [11-sandbox](../demos/11-sandbox/README.md) |
| `io --needs`, `--check-files` | Know which files must exist before running; probe them | CLI (`--check-files` local only) | 0.1.0 | [MAN-0005 §Needs](man-2026-0005-policy-and-io-manifest-guide.md#files-an-operation-needs) | [02-file-crud](../demos/02-file-crud/README.md) |
| `rivet policy generate [--output]` | Draft a least-privilege policy from the manifest | CLI, HTTP `/v1/policy/generate`, Lib | 0.1.0 | [MAN-0005 §Generate](man-2026-0005-policy-and-io-manifest-guide.md#generate-a-least-privilege-draft) | [11-sandbox](../demos/11-sandbox/README.md) |
| `rivet trace show REQ`, `io --trace REQ` | See broker decisions for one request on a running server | Remote, MCP `rivet.trace.show`, Lib | 0.1.0 | [MAN-0005 §Trace](man-2026-0005-policy-and-io-manifest-guide.md#trace-one-request) | [11-sandbox](../demos/11-sandbox/README.md) |

### Surfaces

| Feature | Why / When to Use It | Supported Surfaces | Since Version | Instructions | Demo |
|---|---|---|---|---|---|
| CLI `request` (`--params`, `--stream`, `--timeout`, `--input-jsonl -`, Ctrl-C) | Run one operation from a shell or script | CLI, Remote | 0.1.0 | [MAN-0004 §request](man-2026-0004-cli-reference.md#rivet-request) | [01-catalog](../demos/01-catalog/README.md) |
| `list`, `describe`, `outputs` | Discover the catalog and its schemas | CLI, Remote, HTTP, MCP, Lib | 0.1.0 | [MAN-0004](man-2026-0004-cli-reference.md#rivet-list) | [01-catalog](../demos/01-catalog/README.md) |
| `rivet serve` one listener | Expose every surface on one port | serve | 0.1.0 | [MAN-0006 §Start](man-2026-0006-serving-and-surfaces.md#start-a-server) | [01-catalog](../demos/01-catalog/README.md) |
| REST `/v1/request`, `/v1/operations…`, `/v1/io`, `/v1/policy/generate` | Call from any HTTP client | HTTP | 0.1.0 | [MAN-0006 §REST](man-2026-0006-serving-and-surfaces.md#call-over-rest) | [01-catalog](../demos/01-catalog/README.md) |
| SSE (`Accept: text/event-stream`) | Stream items to browsers and HTTP clients | HTTP | 0.1.0 | [MAN-0006 §SSE](man-2026-0006-serving-and-surfaces.md#stream-over-sse) | [01-catalog](../demos/01-catalog/README.md) |
| Polling sessions `/v1/requests…` | Long-poll events; send input; cancel | HTTP | 0.1.0 | [MAN-0006 §Polling](man-2026-0006-serving-and-surfaces.md#poll-a-session) | [01-catalog](../demos/01-catalog/README.md) |
| WebSocket `/v1/ws` (`rivet.v1`) | Multiplex up to 8 requests per connection | WS | 0.1.0 | [MAN-0006 §WebSocket](man-2026-0006-serving-and-surfaces.md#multiplex-over-websocket) | [01-catalog](../demos/01-catalog/README.md) |
| MCP server (`/mcp` Streamable HTTP, `serve --stdio`) | Expose operations as MCP tools to agents | MCP | 0.1.0 | [MAN-0006 §MCP](man-2026-0006-serving-and-surfaces.md#expose-tools-over-mcp) | [01-catalog](../demos/01-catalog/README.md) |
| Serve auth (`none` loopback-only, `bearer`), principals, `serve.surfaces` | Authenticate callers; limit operations and surfaces | serve | 0.1.0 | [MAN-0006 §Auth](man-2026-0006-serving-and-surfaces.md#authenticate-callers-and-authorize-operations) | [01-catalog](../demos/01-catalog/README.md) |
| `--endpoint URL --token-file PATH` | Use the same CLI against a running server | Remote | 0.1.0 | [MAN-0006 §Endpoint](man-2026-0006-serving-and-surfaces.md#use-the-cli-against-a-server) | [01-catalog](../demos/01-catalog/README.md) |
| Built-in `rivet.*` operations | Discovery, sessions, I/O, policy draft, traces and OAuth through any surface | all | 0.1.0 | [MAN-0006 §Built-ins](man-2026-0006-serving-and-surfaces.md#built-in-operations) | [01-catalog](../demos/01-catalog/README.md) |
| Rust library `rivet::Runtime` | Embed the catalog in a Rust host | Lib | 0.1.0 | [MAN-0007](man-2026-0007-embedding-library.md) | [12-library](../demos/12-library/README.md) |

## Configuration and Environment Variables

Rivet reads **no environment variable for its own configuration** and has no configuration file other than
`policy.json`. Environment variables are read only when an operation asks for one through a granted form.

| Name | Kind | Type / Allowed Values | Default | Required When | Scope | Effect | Security Notes | Example |
|---|---|---|---|---|---|---|---|---|
| `--file PATH` | CLI flag | path to entry `.rivet` | none | every local command | invocation | selects the bundle; `policy.json` beside it is discovered | the bundle root anchors relative paths | `--file app.rivet` |
| `--policy PATH` | CLI flag | path to a policy JSON file | discovered `policy.json` | selecting another file | invocation | replaces discovery; a missing file is `policy.invalid` (exit 2) | a path, never grant text | `--policy policies/read-only.json` |
| `policy.json` | file | schema v1 (MAN-2026-0005) | absent → deny-by-default | any effect | bundle | grants, deny, network, limits, serve, approved | deny wins; private ranges denied unless named literally | `{"version": 1}` |
| `limits.max_concurrent_requests` | policy key | positive integer | 64 | — | host | top-level requests in flight | exceeding → `limit.concurrency` | `64` |
| `limits.max_call_depth` | policy key | positive integer | 16 | — | host | nested `(request …)` depth | — | `16` |
| `limits.max_buffered_bytes` | policy key | positive integer | 268435456 | — | host | buffered response budget | — | `268435456` |
| `--listen HOST:PORT` | serve flag | IP or `localhost` + port | `127.0.0.1:8080` | serve | process | listener address | non-loopback needs `serve.auth` | `--listen 127.0.0.1:18080` |
| `--token-file PATH` | CLI flag | file with the bearer token | none | `--endpoint` to a bearer server | invocation | sends `Authorization: Bearer …` | never pass tokens in argv or env | `--token-file ~/.rivet/ada.token` |
| `--timeout D` | CLI flag | digits + `ms`/`s`/`m`/`h` | 30s | long operations | request | request deadline | — | `--timeout 2m` |
| `deadline_ms` | HTTP body field | integer ms | 30000 | long HTTP requests | request | request deadline, capped at 600000 | — | `{"deadline_ms": 5000}` |
| `secret … from env "VAR"` | env read by an operation | any env var name | — | operation uses it | request | value bound to the `for` origin | needs `allow_env` grant | `FIXTURE_TOKEN` |
| `client_secret env "VAR"` | env read by OAuth | env var name | — | confidential OAuth client | profile | client secret for the token endpoint | needs `allow_env` grant | `CRM_CLIENT_SECRET` |

## Task-Oriented Workflows

The volumes hold full procedures. The three journeys most readers need first:

```text
 A. FIRST RUN (MAN-2026-0002)
 [write app.rivet] -> rivet check -> "ok: N operations" -> rivet request ID --params '{…}' -> Completion JSON
                         |                                        |
                         +-> error[syntax.*] exit 2 -> fix line    +-> error JSON exit 2/3/4/5/6 -> read code

 B. GRANT EFFECTS SAFELY (MAN-2026-0005)
 rivet io (review) -> rivet policy generate --output policy.json (draft) -> edit/review -> rivet io --check-policy
        exit 0 "N allowed" -> rivet request …          exit 3 "k denied" -> add grant or accept denial

 C. SERVE (MAN-2026-0006)
 rivet serve --file app.rivet --listen 127.0.0.1:8080 -> stderr receipt JSON -> curl /v1/request | SSE | /v1/ws | /mcp
                         |
                         +-> non-loopback without serve.auth -> serve.auth_required exit 2 -> add bearer tokens
```

### UI Procedure

Not applicable: Rivet 0.1.0 has no graphical user interface. Every interaction is a CLI command, an HTTP/WS/MCP
request or a Rust call, each shown with exact input and output in the volumes.

## Complete CLI Reference

See [MAN-2026-0004](man-2026-0004-cli-reference.md): every command, flag, default, exit code and a success and
failure example for each.

## Complete API and Event Reference

Routes, frames and tools are summarized in [MAN-2026-0006](man-2026-0006-serving-and-surfaces.md) and
specified in [API-2026-0001](../api/api-2026-0001-http-rest-sse-polling.md) (REST, SSE, polling),
[API-2026-0002](../api/api-2026-0002-websocket-rivet-v1.md) (WebSocket) and
[API-2026-0003](../api/api-2026-0003-mcp-server-tools.md) (MCP tools).

## Errors and Recovery Reference

Every error, on every surface, is one JSON envelope:

```json
{"request_id":"req_…","trace_id":"tr_…","error":{"kind":"validation","code":"validation.type",
 "message":"parameter `a` must be an integer, got text","retryable":false,"effects":"none",
 "operation_id":"demo.add","details":{"field":"a"}}}
```

| Error kind / code | Surface | Cause | User-Visible Result | Recovery | Retry Safe | Related Feature |
|---|---|---|---|---|---|---|
| `syntax.*`, `check.*`, `registry.duplicate_id`, `docs.*` | CLI | source does not parse or check | `error[code]` with a caret at the line, exit 2 | fix the line shown | no | language |
| `validation.*` (`validation.type`, `.required`, `.enum`, `.max`, `.unknown_field`, `.usage`), `policy.invalid`, `stream.*`, `serve.auth_required` | all | bad params, usage or configuration | HTTP 422 (400 for malformed JSON), exit 2 | correct the input | no | params, policy, serve |
| `auth.required`, `auth.invalid` | serve | missing/unknown bearer token | HTTP 401 + `www-authenticate: Bearer`, exit 3 | send the right token | no | serve auth |
| `permission.denied`, `file.hardlink_refused` | all | policy or principal denies the effect/operation | HTTP 403, exit 3 | add a grant or principal entry, or accept | no | policy |
| `not_found.*` | all | unknown operation, file, session, trace, profile | HTTP 404, exit 4 | check the ID/path | no | all |
| `conflict.*` (`conflict.already_exists`, `conflict.exists`, `conflict.ref`) | all | state conflict (e.g. `file create` target exists) | HTTP 409, exit 4 | choose another target or `update` | no | files, generate |
| `limit.*` | all | budget exceeded (`limit.concurrency`, WS refs) | HTTP 429, exit 5 | back off and retry | yes, after backoff | limits |
| `timeout.*` (`timeout.request`, `.poll`, `.scope`) | all | deadline reached | HTTP 504, exit 6 | raise `--timeout`/`deadline_ms` or fix the dependency | caller decides | deadlines |
| `http.status`, `connection.*`, `dns.*`, `tls.*`, `protocol.*`, `process.*`, `udp.*`, `quic.*`, `grpc.*`, application codes | all | dependency failed or declared `fail` | HTTP 502 (500 for some), exit 5 | inspect `details`; retry only if replay-safe | only if replay-safe | protocols |
| `unsupported.*` | all | feature/platform not available in this build | HTTP 501, exit 5 | use the supported alternative named in the message | no | limitations |
| `output.invalid` | all | result violates the declared output | HTTP 500, exit 5 | fix the operation or its declaration | no | outputs |
| `cancelled.*` | all | caller cancelled (Ctrl-C, cancel frame) | exit 130; terminal cancel event | re-run if wanted | no | sessions |
| `io --check-policy` result | CLI | a reachable site is denied/unknown | table printed, exit 3 | add grants or remove the site | — | I/O manifest |
| inspection incomplete | CLI | `io --strict` found dynamic sites; `policy generate` left review items | exit 7 | review the dynamic targets by hand | — | I/O manifest |
| `io --check-files` result | CLI | a needed file is not permitted (3) or missing (4) | listing printed, exit 3/4 | create the file or grant `stat` | — | needs |

```text
exit codes:  0 ok   2 syntax/validation/config   3 permission/auth   4 not_found/conflict
             5 dependency/runtime/unsupported/output_invalid/limit   6 timeout   7 inspection incomplete
             130 cancelled
```

## Examples and Demos

The twelve sample folders under [`docs/demos/`](../demos/README.md) are the executable demos. Every command in
this manual set was run against `target/debug/rivet` (0.1.0-dev, commit `f40d4aa`) from those folders or from a
scratch copy when the command writes files. Request and trace IDs (`req_…`, `tr_…`, `ses_…`, `mcp_…`, `auth_…`)
are generated per run and will differ on your machine.

## Operations, Observability and Maintenance

- `rivet serve` prints one startup **receipt** line (JSON) on stderr: listen address, mounted surfaces, auth
  type, `catalog_version` and `policy_hash`. Keep it in your logs to know exactly what was served.
- Broker decisions are kept in an **in-memory trace store per process**; read them with
  `rivet --endpoint URL trace show REQ` while the server runs. A local `rivet trace show` starts a new process and
  therefore finds nothing (`not_found.trace`).
- Operating guidance (bind, auth, limits, upgrade) is in [OPS-2026-0001](../operations/ops-2026-0001-operating-rivet-serve.md);
  token rotation and policy roll-out are in [RUN-2026-0001](../runbooks/run-2026-0001-rotate-serve-bearer-tokens.md) and
  [RUN-2026-0002](../runbooks/run-2026-0002-roll-out-policy-change.md).

## Edge Cases

- `file delete` with `missing ok` succeeds on an absent file and still reports `effects: "committed"`.
- Cancelling a polling session after it already finished returns `"state":"cancelled"` with HTTP 200.
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
| Server stop | SIGINT/SIGTERM ends `rivet serve`; in-flight requests are cancelled and cleaned up | restart; sessions are in memory and do not survive |

## Security and Compatibility

- Deny-by-default; `deny` entries override `grants`; private, loopback and link-local addresses (including
  `169.254.169.254`) are denied for every scheme (`http`, `https`, `ws`, `tcp`, `udp`, `quic`) unless a grant
  names that address literally. A `"*"` target does not lift this.
- Commands are argv-only: no shell strings (`/bin/sh -c` → `unsupported.shell`) and no PATH lookup.
- Bearer tokens are stored in `policy.json` as SHA-256 hashes; the CLI reads the client token only from
  `--token-file`.
- OAuth tokens are never printed or returned; `auth status` is sanitized.
- Compatibility: the wire contracts (Completion, Envelope, error envelope, WS `rivet.v1`, MCP protocol
  `2025-11-25`) are new in 0.1.0; there is nothing earlier to be compatible with.

## Limitations

Known limitations of 0.1.0 (each volume repeats the ones that affect its tasks):

| Area | Limitation | Consequence / workaround |
|---|---|---|
| serve | `serve.auth` type `mtls` refuses to start (`unsupported.serve_mtls`, exit 5) | use `bearer` behind a TLS-terminating proxy |
| sandbox | Only macOS Seatbelt is active. Linux Landlock+seccomp is built but gated (refuses until verified on kernel ≥ 6.12); other platforms refuse (`unsupported.sandbox_backend`) | on those hosts, run processes only with no `policy.json`, or not at all |
| sandbox | A policy with `access`-narrowed or non-`DIR/**` globbed read/write grants cannot be expressed to the sandbox; process starts fail `unsupported.sandbox_backend` | use `DIR/**`, exact paths or `*` targets in bundles that run processes |
| language | No `finally` (and no `else`) in 0.1.0 | use a second `if` with the negated condition; put cleanup in `with` blocks |
| language | `with file open` / `with file watch` resources fail `unsupported.adapter` in this build | use one-shot `file read` / `file list` |
| language | Secret taint is not enforced on `return`/`emit`: an operation can return or emit a secret value | never return or emit secrets; review operations that declare `secret` |
| HTTP/3 | No Alt-Svc discovery; HTTP/3 only when requested with `version 3` or `version prefer [3, 2]` | request it explicitly |
| transports | No connection pooling; every request opens its own connections | expect per-request handshakes |
| audit | Trace store is in memory, per process, bounded | read traces from the running server; nothing persists |
| MCP server | No resources/templates or prompts exposed by `rivet serve` (`resources/list` → method not found); no legacy HTTP+SSE MCP transport | expose operations as tools |
| WebSocket | `--timeout` is not applied over the WebSocket duplex path | set deadlines in the operation (`timeout`, `scope timeout`) |
| built-ins | No `rivet.capabilities` operation | read this manual's catalogue and the refusal codes |

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
| CLI commands and exit codes | 0.1.0 | — | — | macOS, Linux (Windows untested) |
| `policy.json` schema v1 | 0.1.0 | — | — | all |
| `rivet io`, `policy generate` | 0.1.0 | — | — | all |
| `rivet serve` REST/SSE/polling/WS/MCP, bearer auth | 0.1.0 | — | — | server |
| Process sandbox (Seatbelt) | 0.1.0 | — | — | macOS only |
| Rust library `Runtime` | 0.1.0 | — | — | embedded |

## Related Features

Every feature above links to its task section. Cross-cutting: policy (MAN-2026-0005) governs every effect;
surfaces (MAN-2026-0006) expose every operation.

## Related Documents

- Plan: [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) (rows D-34 … D-41)
- Approved design: [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md)
- Numbered examples: [REF-2026-0002](../references/ref-2026-0002-language-and-usage.md)
- System documents: [SYS-2026-0001](../system/components/sys-2026-0001-compiler-and-catalog.md),
  [SYS-2026-0002](../system/runtime/sys-2026-0002-execution-scopes-and-dag.md),
  [SYS-2026-0003](../system/components/sys-2026-0003-policy-broker-and-io-manifest.md),
  [SYS-2026-0004](../system/components/sys-2026-0004-surfaces-and-serve.md)
- Manual index: [index.md](index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial root manual for the implemented 0.1.0 build: catalogue, concepts, limitations, glossary; every command verified against 0.1.0-dev commit f40d4aa. |
