---
document_id: MAN-2026-0002
title: "Rivet installation and quickstart"
document_type: manual
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [cli, language, registry, policy, audit, serve, http]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, server]
audience: [developers, operators, new-users]
scope: Build rivet from source with Cargo and run the first check, request, outputs, io and serve against the 01-catalog demo, with success and failure examples.
reason: PLAN-2026-0001 row D-35 — installation manual and first-run journey for the implemented 0.1.0 build.
related_documents: [MAN-2026-0001, MAN-2026-0003, MAN-2026-0004, MAN-2026-0005, MAN-2026-0006, DEMO-2026-0001, PLAN-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, manual, installation, quickstart]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.1.0-dev (commit f40d4aa)"
---

# Rivet installation and quickstart

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** cli, language, registry, policy, audit, serve, http

## Purpose

Get from a clean checkout to a running operation in about ten minutes: build the binary, compile a bundle, call
an operation, read its declared output, inspect its I/O and serve it over HTTP. Part of the
[Rivet manual](man-2026-0001-rivet-manual.md).

## Reading Order

```text
 1 Prerequisites ─► 2 Build ─► 3 check ─► 4 request ─► 5 outputs ─► 6 io ─► 7 serve ─► 8 next steps
```

## Prerequisites

| Requirement | Detail |
|---|---|
| Rust toolchain | 1.90.0 (pinned by `rust-toolchain.toml`; `rustup` installs it on first `cargo` use) |
| Platform | macOS or Linux. The process sandbox is active only on macOS (see MAN-2026-0008). |
| Network | Only to fetch crates on the first build (including the pinned Capy git dependency). |
| Disk | A debug build needs several GB under `target/`; see TRBL-2026-0003 if the linker reports "no space left on device". |
| Tools for the examples | `curl`; `shasum` (macOS) or `sha256sum` (Linux) for bearer-token hashes. |

## Installation and Setup

### Journey Overview

```text
 [git checkout] -> cargo build -> target/debug/rivet --version -> "rivet 0.1.0-dev"
                        |
                        +-> compile error / linker "no space left" -> free disk (TRBL-2026-0003) -> retry
```

### CLI Procedure

```bash
cd /path/to/rivet            # the repository root (contains Cargo.toml)
cargo build                  # debug build → target/debug/rivet
target/debug/rivet --version
```

Expected output (the version string of the unreleased tree):

```text
rivet 0.1.0-dev
```

Optimized build (slower to compile, faster to run):

```bash
cargo build --release        # → target/release/rivet
target/release/rivet --version
```

Put the binary on your `PATH` if you like (the rest of this manual writes plain `rivet`):

```bash
export PATH="$PWD/target/debug:$PATH"
rivet --help
```

`rivet --help` lists the commands:

```text
Commands:
  request     Invoke one operation
  list        List public operations
  describe    Describe operations: params, output, errors, source
  outputs     Show declared outputs, emits, receives and errors
  check       Compile and check the bundle without running anything
  io          Generate the I/O manifest (every I/O site, target and access verb)
  policy      Policy tools
  serve       Serve every surface (REST, SSE, polling, WebSocket, MCP) on one listener
  trace       Request traces recorded by this host
  connectors  Outbound MCP connectors
  auth        OAuth account management (rivet.auth.* built-ins); tokens are never printed
  help        Print this message or the help of the given subcommand(s)
```

There is no published crate, installer or prebuilt binary for 0.1.0. To use Rivet as a library, depend on the
repository by path or git (MAN-2026-0007).

## Task-Oriented Workflows

All commands below run from `docs/demos/01-catalog/`, whose `app.rivet` declares four pure operations
(`demo.greet`, `demo.add`, `demo.health`, `demo.countdown`) and has **no** `policy.json`. IDs such as
`req_01fe6f6e2d` are generated per run and differ on your machine.

```bash
cd docs/demos/01-catalog
```

### 1. Compile and check the bundle

Why: catch syntax and composition errors without running anything.

```bash
rivet check --file app.rivet
echo $?
```

```text
ok: 4 operations, 0 connectors, 0 auth profiles
0
```

Failure example — `--file` is required for every local command:

```bash
rivet check
```

```text
error[validation.usage]: --file PATH is required (the entry .rivet file)
```

Exit code 2. Failure example — a syntax error (here an `if … else`, which 0.1.0 does not have) is reported with
its line and a caret; exit code 2:

```text
error[syntax.unknown_statement]: unknown statement `else`
  --> app.rivet:22:5
   |
 22|     else
   |     ^^^^
```

### 2. List and call an operation

```bash
rivet list --file app.rivet
rivet request --file app.rivet demo.add --params '{"a":2,"b":3}'
```

```text
ID              NAME                DESCRIPTION
demo.greet      Greet a person      Return a greeting for the supplied person.
demo.add        Add two integers    Add two signed integers and return their sum.
demo.health     Check availability  Return a constant readiness response without I/O.
demo.countdown  Count down          Emit 3, 2, 1 as data items and then return a summary.
{"request_id":"req_01fcf2a8bd","trace_id":"tr_01fcf2a8bd","result":5,"data_count":0,"effects":"none"}
```

`result` is the operation's value; `effects: "none"` says nothing outside Rivet changed.

Failure examples (stdout carries the error envelope; exit code in brackets):

```text
$ rivet request --file app.rivet demo.add --params '{"a":"x"}'                          [2]
{"request_id":"req_01fbbafdad","trace_id":"tr_01fbbafdad","error":{"kind":"validation","code":"validation.type","message":"parameter `a` must be an integer, got text","retryable":false,"effects":"none","operation_id":"demo.add","details":{"field":"a"}}}

$ rivet request --file app.rivet demo.nope                                               [4]
{"request_id":"req_01f6376a9d","trace_id":"tr_01f6376a9d","error":{"kind":"not_found","code":"not_found.operation","message":"no operation `demo.nope`","retryable":false,"effects":"none","operation_id":"demo.nope"}}
```

Streaming operation, printed as NDJSON envelopes with `--stream`:

```bash
rivet request --file app.rivet demo.countdown --stream
```

```text
{"request_id":"req_01f871eedd","trace_id":"tr_01f871eedd","seq":1,"type":"data","data":3}
{"request_id":"req_01f871eedd","trace_id":"tr_01f871eedd","seq":2,"type":"data","data":2}
{"request_id":"req_01f871eedd","trace_id":"tr_01f871eedd","seq":3,"type":"data","data":1}
{"request_id":"req_01f871eedd","trace_id":"tr_01f871eedd","result":{"count":3},"data_count":3,"effects":"none","type":"result"}
```

### 3. Read the declared outputs

```bash
rivet outputs --file app.rivet demo.countdown
```

```text
demo.countdown — Count down
output  object   Summary returned after the last item.
  count   integer  required  Number of items emitted.
emits   integer
receives —
errors   —
```

`rivet outputs --file app.rivet demo.countdown --json` prints the same as JSON Schema; `--all` shows every
operation.

### 4. Inspect I/O before running anything

```bash
rivet io --file app.rivet
rivet policy --file app.rivet explain
```

```text
OPERATION  KIND  ACCESS  TARGET  KNOWLEDGE  SOURCE
(no I/O sites)
policy   none — no policy.json: every new application effect is denied (pure operations still run)
base     .
network  deny_private_ranges true
limits   64 concurrent, depth 16, 268435456 buffered bytes
```

This bundle is pure, so nothing needs a grant. For a bundle that touches files, run the same commands in
`docs/demos/02-file-crud/` or `docs/demos/11-sandbox/` and continue with MAN-2026-0005.

### 5. Serve every surface

```text
 terminal A                                   terminal B
 ──────────                                   ──────────
 rivet serve --file app.rivet \
   --listen 127.0.0.1:18080
   stderr: {"listen_addr":…,"surfaces":[…]}
                                   ◄───────── curl -X POST …/v1/request
   (serves until Ctrl-C)           ─────────► {"result":5,…}
```

Terminal A:

```bash
rivet serve --file app.rivet --listen 127.0.0.1:18080
```

stderr receipt (one line):

```text
{"listen_addr":"127.0.0.1:18080","stdio":false,"surfaces":["http","sse","poll","ws","mcp"],"auth_type":"none","catalog_version":"sha256:67104f0e7faaeafee253db758a668a9b24aa4263677e9d5ea2451b32019f9730","policy_hash":null}
```

Terminal B:

```bash
curl -sS -X POST http://127.0.0.1:18080/v1/request \
  -H 'content-type: application/json' \
  -d '{"id":"demo.add","params":{"a":2,"b":3}}'
rivet --endpoint http://127.0.0.1:18080 request demo.add --params '{"a":4,"b":5}'
```

```text
{"request_id":"req_010c731ded","trace_id":"tr_010c731ded","result":5,"data_count":0,"effects":"none"}
{"request_id":"req_046718b54c","trace_id":"tr_046718b54c","result":9,"data_count":0,"effects":"none"}
```

Stop the server with Ctrl-C in terminal A (exit code 0).

Failure example — listening on a non-loopback address without authentication is refused before binding:

```bash
rivet serve --file app.rivet --listen 0.0.0.0:18080
```

```text
{"request_id":"","trace_id":"","error":{"kind":"validation","code":"serve.auth_required","message":"non-loopback listener requires serve.auth in policy.json","retryable":false,"effects":"none"}}
```

Exit code 2. Recovery: add bearer tokens to `policy.json` (MAN-2026-0006).

### Expected Result and Side Effects

| Step | Side effects |
|---|---|
| `cargo build` | writes `target/`; downloads crates on first run |
| `check`, `list`, `describe`, `outputs`, `io`, `policy explain` | none (read the bundle and policy only) |
| `request` of a pure operation | none (`effects: "none"`) |
| `serve` | binds the TCP port until stopped; keeps sessions and traces in memory only |

### Verified Demo

[01-catalog (DEMO-2026-0001)](../demos/01-catalog/README.md). Every command above was run against
`rivet 0.1.0-dev` (commit `f40d4aa`) from that folder on macOS; `cargo build --release` was also run.

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `error[validation.usage]: --file PATH is required` | local command without a bundle | add `--file app.rivet` |
| `error[not_found.source]: cannot read missing.rivet` (exit 4) | wrong path | check the path relative to your shell's directory |
| `error[policy.invalid]: --policy /x.json: no such file` (exit 2) | `--policy` names a missing file | fix the path; omit `--policy` to use discovery |
| `serve.auth_required` (exit 2) | non-loopback `--listen` with no `serve.auth` | bind `127.0.0.1` or configure bearer auth |
| `--listen nothost: use HOST:PORT with an IP address or localhost` (exit 2) | malformed listen address | use `127.0.0.1:PORT` |

## Limitations

- Build from source only; no packages. Windows builds are untested.
- See the [root manual's limitations](man-2026-0001-rivet-manual.md#limitations).

## Version Applicability

| Feature / Interface | Introduced | Changed | Deprecated / Removed | Applicable Environment |
|---|---|---|---|---|
| Source build with Cargo | 0.1.0 | — | — | development |
| Quickstart commands | 0.1.0 | — | — | development, server |

## Related Documents

- [Rivet manual](man-2026-0001-rivet-manual.md) · [Language guide](man-2026-0003-language-guide.md) ·
  [CLI reference](man-2026-0004-cli-reference.md) · [Policy guide](man-2026-0005-policy-and-io-manifest-guide.md) ·
  [Serving](man-2026-0006-serving-and-surfaces.md)
- [Plan PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial installation and quickstart for 0.1.0, verified against 0.1.0-dev commit f40d4aa. |
