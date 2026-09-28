---
document_id: DEMO-2026-0012
title: "Embed Rivet as a Rust library"
document_type: demo
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 5
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [library, registry, execution, policy, audit, cli]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, embedded]
audience: [developers, reviewers]
scope: Runnable library demo — a complete host program (embedding.rs.txt) that builds a Runtime from source, policy.json and a host ceiling, makes a unary request, consumes a scoped stream, reads declared outputs, the I/O manifest and a policy draft, compiled as a scratch crate against the `rivet` crate and compared with the CLI.
reason: User requested sample files in folders with READMEs showing usage; UQ-09/10 ask for a Rust library; UQ-17 adds declared outputs and policy.json-only policy; TASK-067 compiled and executed the embedding against the 0.1.0 release candidate.
dependencies: [PROP-2026-0001, REF-2026-0002]
related_documents: [PLAN-2026-0001, DEMO-2026-0015, DEMO-2026-0013, DEMO-2026-0001, MAN-2026-0007, API-2026-0004, TEST-2026-0010, TEST-2026-0020]
supersedes: null
superseded_by: null
tags: [rivet, demo, library, embedding, rust]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-28
verified_against: "0.1.0"
---

# Embed Rivet as a Rust library

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** library, registry, execution, policy, audit, cli

## Purpose

Embed Rivet as a Rust library. Delivery stage: **A**. Read [app.rivet](app.rivet) and [embedding.rs.txt](embedding.rs.txt) alongside this walkthrough.

| Operation ID | Output | Behavior |
|---|---|---|
| `demo.greet` | `text` | Same pure operation as the catalog sample. |
| `demo.add` | `integer` | Typed library request. |
| `demo.health` | `object {ready}` | Pure readiness call. |
| `events.count` | `object {count}`, emits `integer` | Scoped stream with pull-based backpressure. |

```text
  host process (Tokio)
  +---------------------------------------------------------------------+
  | Policy::from_file("policy.json") ─┐   Policy::from_json(ceiling) ─┐  |
  | include_str!("app.rivet") ────────┴──> Runtime::builder()  <──────┘  |
  |                                          .source .policy .ceiling    |
  |                                          .build()                    |
  |   rt.request("demo.add", …)            -> Completion {result: 5}     |
  |   rt.scope(|scope| scope.stream("events.count", …))                  |
  |        s.next() -> Ok(Some(Envelope)) x4, then Ok(None)              |
  |   rt.outputs(Some("demo.add"), false)  -> OutputSpec                 |
  |   rt.io(&IoQuery) / rt.generate_policy -> manifest / draft           |
  +---------------------------------------------------------------------+
     no CLI subprocess, no listener; host code stays outside the sandbox
```

## Verified Against Version

0.1.0. Verified on 0.1.0-dev at commit `829ca43`, the release candidate (the version bump to 0.1.0 happens at release, P5), on macOS 26.4.1 (Darwin 25.4.0, arm64), 2026-09-28: embedding.rs.txt compiled as a scratch binary crate against the repository's `rivet` crate and run from this folder; the CLI comparison used `target/release/rivet`. Every output block below was pasted from that run. Request and trace IDs and hashes vary.

## Prerequisites

```sh
cargo build --release                       # from the repository root
export PATH="$PWD/target/release:$PATH"     # `rivet --version` prints rivet 0.1.0
```

A Rust toolchain matching the repository's `rust-toolchain.toml` (edition 2024) for step 2.

## Setup

```sh
cd docs/demos/12-library
```

[policy.json](policy.json) contains no grants, only `limits` (`max_concurrent_requests` 64, `max_call_depth` 16, `max_buffered_bytes` 256 MiB). A grant-free file behaves like having no file (deny-by-default); the four operations are pure, so nothing is denied. The host ceiling in the program (`max_concurrent_requests` 8) can only narrow it.

## Steps

### 1. The same catalog through the CLI

#### Command / Request

```sh
rivet --file app.rivet check --strict-docs
rivet --file app.rivet request demo.add --params '{"a":2,"b":3}'
rivet --file app.rivet request events.count --params '{}' --stream
rivet --file app.rivet outputs demo.add
```

#### Expected Output / Response

```text
ok: 4 operations, 0 connectors, 0 auth profiles
```

```json
{"request_id":"req_01cdd3207d","trace_id":"tr_01cdd3207d","result":5,"data_count":0,"effects":"none"}
{"request_id":"req_01cc066a45","trace_id":"tr_01cc066a45","seq":1,"type":"data","data":1}
{"request_id":"req_01cc066a45","trace_id":"tr_01cc066a45","seq":2,"type":"data","data":2}
{"request_id":"req_01cc066a45","trace_id":"tr_01cc066a45","seq":3,"type":"data","data":3}
{"request_id":"req_01cc066a45","trace_id":"tr_01cc066a45","result":{"count":3},"data_count":3,"effects":"none","type":"result"}
```

```text
demo.add — Add two integers
output  integer  Sum of a and b.
emits    —
receives —
errors   —
```

All exit 0.

### 2. Compile and run the embedding

#### Command / Request

Create a scratch crate outside the repository, with the program and the bundle side by side (`include_str!("app.rivet")` is relative to the source file):

```sh
DEMO="$PWD"; RIVET_ROOT="$(cd ../../.. && pwd)"
EMBED="$(mktemp -d)/embed"; mkdir -p "$EMBED/src"
cp embedding.rs.txt "$EMBED/src/main.rs"; cp app.rivet "$EMBED/src/app.rivet"
cp "$RIVET_ROOT/rust-toolchain.toml" "$EMBED/"
cat > "$EMBED/Cargo.toml" <<TOML
[package]
name = "embed"
version = "0.1.0"
edition = "2024"

[dependencies]
rivet = { path = "$RIVET_ROOT" }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
serde_json = "1"
TOML
cargo build --manifest-path "$EMBED/Cargo.toml"
"$EMBED/target/debug/embed"          # run from this folder so policy.json resolves
```

#### Expected Output / Response

The program's `assert_eq!`s pass (`demo.add` → `Value::Int(5)`; the stream yields 1, 2, 3 then a result with `data_count` 3) and it prints three lines, then exits 0:

```text
{"id":"demo.add","output":{"type":"integer","description":"Sum of a and b."},"emits":null,"receives":null,"errors":[]}
{"bundle":{"file":"app.rivet","sha256":"36aa9cf790e354a2500efabe5ae70e12e7b27944ea296c7e5a7fc46f0308e045"},"policy":{"file":"policy.json","sha256":"sha256:55326b2ffb3d392f99d424e3ddda1d91dffd0a8e307b616d947c1069f9995d38"},"complete":true,"sites":[],"targets":[],"needs":[],"bootstrap":[]}
{"version":1,"grants":[],"network":{"deny_private_ranges":true}}
```

Line 1 is `rt.outputs(Some("demo.add"), false)`, identical to the `demo.add` entry of `rivet outputs --all --json`. Line 2 is `rt.io(&IoQuery{ids: ["demo.add"], format: "json"})`: no sites, `complete: true`. Line 3 is `rt.generate_policy(&["events.count"])`, identical to `rivet --file app.rivet policy generate events.count` (whose stderr adds `policy generate: 0 grants, 0 review items`).

```text
  library call                                   CLI equivalent
  ─────────────────────────────────────────────  ─────────────────────────────────────────────
  rt.request("demo.add", {a:2,b:3}, None)        rivet request demo.add --params '{"a":2,"b":3}'
  rt.scope(… scope.stream("events.count") …)     rivet request events.count --params '{}' --stream
  rt.outputs(Some("demo.add"), false)            rivet outputs demo.add --json
  rt.io(&IoQuery{…})                             rivet io demo.add --format json
  rt.generate_policy(&["events.count"])          rivet policy generate events.count
```

### 3. Inspect before invoking

#### Command / Request

```sh
rivet --file app.rivet io --by target
rivet --file app.rivet io --check-policy
rivet --file app.rivet io --check-files
```

#### Expected Output / Response

All four operations are pure; each command exits 0:

```text
TARGET  ACCESS  CAPABILITY  ORIGIN  PHASE  NEEDS FILE  USED BY
(no I/O sites)
```

```text
OPERATION  KIND  ACCESS  TARGET  KNOWLEDGE  SOURCE  DECISION
(no I/O sites)
```

```text
demo.add needs no existing files.
demo.greet needs no existing files.
demo.health needs no existing files.
events.count needs no existing files.
0 files
```

## Effects and policy

The four operations are pure. Host code stays outside the library's sandbox boundary: an arbitrary host callback or `DataSink` that performs native file or network I/O must be controlled by the host. A `DataSink` passed to `rt.request(ID, params, Some(sink))` receives each emitted item; returning `RivetError::consumer_stop()` ends the request `cancelled`, not failed. Dropping a request future runs its cleanup; `rt.scope` joins everything it started. `Runtime` is `Clone + Send + Sync`; build it once and share it.

## Release Updates

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-01 | UQ-09/10 / R1 | Rust library over the Capy parser; no CLI subprocess | Step 2 | Program builds and exits 0 | This README step 2 (2026-09-28, 829ca43); TEST-2026-0010 |
| U-08 | UQ-07/09 / R8 | Library and CLI share one dispatcher | Steps 1–2 | Same result 5, same stream, same outputs JSON | This README steps 1–2; TEST-2026-0002 |
| U-09 | UQ-08 / R9 | `request`, `scope.stream`, `next() -> Result<Option<Envelope>>` | Step 2 | Items 1, 2, 3 then the result | This README step 2; TEST-2026-0010 |
| U-23 | UQ-17 / R23 | `Runtime::outputs` | Step 2 line 1 | Integer output spec | This README step 2; TEST-2026-0020 |

## Cleanup

```sh
rm -rf "$(dirname "$EMBED")"
```

Nothing is written to this folder.

## Verification Record

| Step | Verified By | Verified At | Result |
|---|---|---|---|
| 1. CLI `check --strict-docs`, request, stream, outputs | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 2. embedding.rs.txt compiled against `rivet` and run from this folder | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 3. `io --by target`, `io --check-policy` (0), `io --check-files` (0) | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |

Build: `cargo build` and `cargo build --release` at 829ca43; the embedding was built with `cargo build` in a scratch crate depending on the repository by path.

## Known Caveats

- The embedding is kept as `embedding.rs.txt` and compiled in a scratch crate; it is not yet a `[[example]]` target of the repository (plan item D-13).
- `scope.duplex` (live input) is described in the program's comments; it is exercised against the gRPC fixture by TEST-2026-0010, not here.

## Related Documents

- [All sample folders](../README.md) · [demos index](../index.md) · [release verification guide](../demo-2026-0015-v0-1-0-release-verification.md)
- [Same catalog on every surface (01-catalog)](../01-catalog/README.md)
- [Embedding manual MAN-2026-0007](../../manuals/man-2026-0007-embedding-library.md) · [Rust library API](../../api/api-2026-0004-rust-library.md)
- [Library tests TEST-2026-0010](../../testing/test-2026-0010-library.md)
- [Usage reference](../../references/ref-2026-0002-language-and-usage.md) · [Proposal](../../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 5 | 2026-09-28 | Claude | TASK-067: compiled embedding.rs.txt as a scratch crate against 0.1.0-dev (829ca43) and ran it from this folder; pasted the real output (outputs, manifest, draft) and the CLI comparison; replaced the outdated sketch (`.source(src)`, `json!` params, `rt.outputs("demo.add")`) with the real API (`.source(path, src, root)`, `Value::from_json`, `.ceiling`, `rt.outputs(Some(id), false)`); removed draft disclaimers; status active; verified_against 0.1.0. |
| 4 | 2026-09-28 | Claude | TASK-005/R26 (ADR-0001, proposal revision 8): `io --by target` header gains ORIGIN, PHASE, NEEDS FILE (still no sites); verified-against revision 8. |
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest: no sites; `io --check-policy` exits 0; library `rt.io` / `rt.generate_policy` equivalents. |
| 2 | 2026-09-28 | Claude | UQ-17/E11: embedding sketch replaced by the canonical builder (`Policy::from_file`, `rt.request`, `rt.scope`/`scope.stream`, `rt.outputs`); added grant-free policy.json; declared outputs; removed `--sandbox`; View outputs, strict-docs, bootstrap and exit codes. |
| 1 | 2026-09-28 | Codex | Added draft source files, prerequisites, invocation examples and expected behavior. |
