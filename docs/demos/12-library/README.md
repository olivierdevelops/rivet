---
document_id: DEMO-2026-0012
title: "Embed Rivet as a Rust library"
document_type: demo
status: active
created_date: 2026-09-28
last_updated: 2026-09-30
document_revision: 9
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [library, registry, execution, policy, audit, cli]
affected_versions:
  from: "0.2.0"
  to: null
applicable_environments: [development, embedded]
audience: [developers, reviewers]
scope: Runnable library demo — a complete host program (embedding.rs.txt) that builds a Runtime from source, policy.json and a host ceiling, makes a unary request, consumes a scoped stream, reads declared outputs, the I/O manifest and a policy draft, compiled as a scratch crate against the `rivet` crate and compared with the CLI.
reason: User requested sample files in folders with READMEs showing usage; UQ-09/10 ask for a Rust library; UQ-17 adds declared outputs and policy.json-only policy; TASK-067 compiled and executed the embedding against the 0.1.0 release candidate. TASK-076 (PLAN-2026-0002 D-61) rewrote the embedding against the 0.2.0 facade (package rivet-runtime, `Runtime::call`, ResponseEnvelope) and re-executed it against the 0.2.0 release candidate.
dependencies: [PROP-2026-0001, REF-2026-0002]
related_documents: [PLAN-2026-0001, DEMO-2026-0015, DEMO-2026-0013, DEMO-2026-0001, MAN-2026-0007, API-2026-0004, TEST-2026-0010, TEST-2026-0020, PLAN-2026-0002, DEMO-2026-0020, MIG-2026-0001, DEMO-2026-0019]
supersedes: null
superseded_by: null
tags: [rivet, demo, library, embedding, rust]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-28
verified_against: "0.2.0"
---

# Embed Rivet as a Rust library

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-30
> **Affected Versions:** 0.2.0 and later
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
  | "app.rivet" ──────────────────────┴──> Runtime::builder()  <──────┘  |
  |                                          .file .policy .ceiling      |
  |                                          .build()                    |
  |   rt.call(InputEnvelope)               -> ResponseEnvelope {data: 5} |
  |   rt.request("demo.greet", …)          -> Completion (Result, `?`)   |
  |   rt.scope(|scope| scope.stream("events.count", …))                  |
  |        s.next() -> Ok(Some(Envelope)) x4, then Ok(None)              |
  |   rt.outputs(Some("demo.add"), false)  -> OutputSpec                 |
  |   rt.io(&IoQuery) / rt.generate_policy -> manifest / draft           |
  +---------------------------------------------------------------------+
     no CLI subprocess, no listener; host code stays outside the sandbox
```

## Verified Against Version

0.2.0. Verified on 0.2.0-dev at commit `8031baa`, the release candidate (the version string is bumped to 0.2.0 at release, P5), on macOS 26.4.1 (Darwin 25.4.0, arm64), 2026-09-29: embedding.rs.txt compiled as a scratch binary crate against the repository's `rivet-runtime` package (library `rivet`) and run from this folder; the CLI comparison used `target/release/rivet`. Every output block below was pasted from that run. Request and trace IDs and hashes vary.

What changed for embedders in 0.2.0 ([migration guide](../../migrations/mig-2026-0001-response-and-input-envelopes.md), [Rust library API](../../api/api-2026-0004-rust-library.md)):

```text
  0.1.0                                          0.2.0
  ─────────────────────────────────────────────  ─────────────────────────────────────────────────────────
  rivet = { path = … }                           rivet = { package = "rivet-runtime", git/path = … }
  use rivet::domain::Value                       use rivet::Value            (internals: rivet::internal, hidden)
  use rivet::domain::policy::Policy              use rivet::Policy
  use rivet::domain::io_manifest::IoQuery        use rivet::types::IoQuery
  RivetResult<T>                                 rivet::Result<T>  (error type rivet::Error)
  rt.request(id, Value, sink) only               + rt.call(InputEnvelope) -> ResponseEnvelope (wire shape)
                                                 + rt.load(path) -> Module   (see 17-modules)
```

## Prerequisites

```sh
cargo build --release --features cli       # from the repository root
export PATH="$PWD/target/release:$PATH"     # rivet --version prints rivet 0.2.0
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
rivet --file app.rivet request demo.add --data '{"a":2,"b":3}'
rivet --file app.rivet request events.count --stream
rivet --file app.rivet outputs demo.add
```

#### Expected Output / Response

```text
ok: 4 operations, 0 connectors, 0 auth profiles
```

```json
{"request_id":"req_01b78ee965","trace_id":"tr_01b78ee965","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}
{"request_id":"req_01b449cc15","trace_id":"tr_01b449cc15","operation":"events.count","type":"data","seq":1,"data":1,"error":null}
{"request_id":"req_01b449cc15","trace_id":"tr_01b449cc15","operation":"events.count","type":"data","seq":2,"data":2,"error":null}
{"request_id":"req_01b449cc15","trace_id":"tr_01b449cc15","operation":"events.count","type":"data","seq":3,"data":3,"error":null}
{"request_id":"req_01b449cc15","trace_id":"tr_01b449cc15","operation":"events.count","type":"result","seq":4,"status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}
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
rivet = { package = "rivet-runtime", path = "$RIVET_ROOT" }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
serde_json = "1"

[workspace]
TOML
cargo build --manifest-path "$EMBED/Cargo.toml"
"$EMBED/target/debug/embed"          # run from this folder so policy.json resolves
```

#### Expected Output / Response

The program's assertions pass (`demo.add` → `data` 5 and `status` ok; `{"a":"two"}` → `status` error; `demo.greet` → `Value::text("Hello, Ada!")`; the stream yields 1, 2, 3 then a result with `data_count` 3). It prints, then exits 0:

```text
{
  "request_id": "req_0134df92ad",
  "trace_id": "tr_0134df92ad",
  "operation": "demo.add",
  "type": "result",
  "status": "ok",
  "data": 5,
  "error": null,
  "effects": "none",
  "data_count": 0
}
{"request_id":"req_02b538cdf2","trace_id":"tr_02b538cdf2","operation":"demo.add","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.type","message":"parameter `a` must be an integer, got text","retryable":false,"operation_id":"demo.add","details":{"field":"a"}},"effects":"none","data_count":0}
{"request_id":"req_04b69b4984","trace_id":"tr_04b69b4984","operation":"events.count","type":"data","seq":1,"data":1,"error":null}
{"request_id":"req_04b69b4984","trace_id":"tr_04b69b4984","operation":"events.count","type":"data","seq":2,"data":2,"error":null}
{"request_id":"req_04b69b4984","trace_id":"tr_04b69b4984","operation":"events.count","type":"data","seq":3,"data":3,"error":null}
{"request_id":"req_04b69b4984","trace_id":"tr_04b69b4984","operation":"events.count","type":"result","seq":4,"status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}
{"id":"demo.add","output":{"type":"integer","description":"Sum of a and b."},"emits":null,"receives":null,"errors":[]}
{"bundle":{"file":"app.rivet","sha256":"36aa9cf790e354a2500efabe5ae70e12e7b27944ea296c7e5a7fc46f0308e045"},"policy":{"file":"policy.json","sha256":"sha256:55326b2ffb3d392f99d424e3ddda1d91dffd0a8e307b616d947c1069f9995d38"},"complete":true,"sites":[],"targets":[],"needs":[],"bootstrap":[]}
{"version":1,"grants":[],"network":{"deny_private_ranges":true}}
build features: ["serve", "grpc", "quic", "oauth"]
```

The first two blocks are `rt.call(InputEnvelope)`: the same ResponseEnvelope, key for key, as `rivet request` prints (step 1), `to_json_pretty()` for the success and `to_json_string()` for the failure. The four stream lines are `env.record().to_json_string()`, the same records as NDJSON; the terminal record carries `"seq":4` like the CLI's (fixed in INC-2026-0012). Then `rt.outputs(Some("demo.add"), false)` (the `data` entry of `rivet outputs --all --json`), `rt.io(&IoQuery{ids: ["demo.add"], format: "json"})` (the bare IoManifest, no sites, `complete: true`; the CLI wraps it in a `rivet.io` envelope), `rt.generate_policy(&["events.count"])` (identical to `rivet --file app.rivet policy generate events.count`, whose stderr adds `policy generate: 0 grants, 0 review items`) and `rivet::build_features()`: this dependency uses the default features (`serve, grpc, quic, oauth`); the CLI build adds `cli`.

```text
  library call                                   CLI equivalent
  ─────────────────────────────────────────────  ─────────────────────────────────────────────
  rt.call(InputEnvelope::new("demo.add")…)       rivet request demo.add --data '{"a":2,"b":3}'
  rt.request("demo.greet", Value, None)          rivet request demo.greet --data '{"person":"Ada"}'
  rt.scope(… scope.stream("events.count") …)     rivet request events.count --stream
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

The four operations are pure. Host code stays outside the library's sandbox boundary: an arbitrary host callback or `DataSink` that performs native file or network I/O must be controlled by the host. A `DataSink` passed to `rt.request(ID, params, Some(sink))` receives each emitted item; returning `rivet::Error::consumer_stop()` ends the request `cancelled`, not failed. Dropping a request future runs its cleanup; `rt.scope` joins everything it started. `Runtime` is `Clone + Send + Sync`; build it once and share it.

## Release Updates

0.2.0 updates shown here (numbering of the [v0.2.0 release verification guide](../demo-2026-0020-v0-2-0-release-verification.md)):

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-01 / U-04 | UQ-03/05/06 / R1, R4 | `Runtime::call(InputEnvelope)` returns the wire ResponseEnvelope | Step 2 | Same envelope as `rivet request` | This README step 2 (2026-09-29, 8031baa) |
| U-06 | UQ-04 / R6 | `to_json_pretty()` | Step 2 | Indented envelope | This README step 2 |
| U-10 | UQ-02 / R10 | Package `rivet-runtime`, library `rivet`, facade only (`rivet::{Runtime, Policy, Value, InputEnvelope, …}`, `rivet::types`) | Step 2 | Scratch crate compiles with `package = "rivet-runtime"` and no `rivet::domain` path | This README step 2; `examples/embed.rs` |
| U-11 | UQ-02 / R11 | `rivet::build_features()` | Step 2 last line | `["serve", "grpc", "quic", "oauth"]` for a default dependency | This README step 2 |

Still verified from 0.1.0 (numbering of [DEMO-2026-0015](../demo-2026-0015-v0-1-0-release-verification.md)):

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-01 | UQ-09/10 / R1 | Rust library over the Capy parser; no CLI subprocess | Step 2 | Program builds and exits 0 | This README step 2 (re-run 2026-09-29, 8031baa); TEST-2026-0010 |
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
| 1. CLI `check --strict-docs`, request with `--data`, stream, outputs | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 2. embedding.rs.txt (0.2.0 facade) compiled against `rivet-runtime` and run from this folder | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 3. `io --by target`, `io --check-policy` (0), `io --check-files` (0) | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 2. (re-run after INC-2026-0012) scratch crate built against the repository (`cargo build`, rivet-runtime path dependency) and run from this folder: assertions pass, exit 0; library terminal record `"seq":4,…,"data_count":3`; rest of the output unchanged except IDs | Claude | 2026-09-29, commit 7c25175, macOS 26.4.1 arm64 | PASS |

Verified on 0.2.0-dev at commit `8031baa`, the release candidate (`cargo build --release --workspace --all-features` for the CLI); the embedding was built with `cargo build` in a scratch crate depending on the repository by path (`package = "rivet-runtime"`) and run from this folder. The 0.1.0 verification (TASK-067, commit 829ca43) is recorded in revision 5 below. Step 2 was re-run on 2026-09-29 at commit `7c25175` (source = `14750b8`) after the INC-2026-0012 fixes; its output above is from that run.

## Known Caveats

- The embedding is kept as `embedding.rs.txt` and compiled in a scratch crate. Its compiled twin is `examples/embed.rs` (`cargo run --example embed`), which takes the demo directory as an argument.
- `scope.duplex` (live input) is described in the program's comments; it is exercised against the gRPC fixture by TEST-2026-0010, not here.

## Related Documents

- [All sample folders](../README.md) · [demos index](../index.md) · [v0.2.0 release verification guide](../demo-2026-0020-v0-2-0-release-verification.md) · [v0.1.0 guide](../demo-2026-0015-v0-1-0-release-verification.md) · [Modules from Rust (17-modules)](../17-modules/README.md)
- [Same catalog on every surface (01-catalog)](../01-catalog/README.md)
- [Embedding manual MAN-2026-0007](../../manuals/man-2026-0007-embedding-library.md) · [Rust library API](../../api/api-2026-0004-rust-library.md)
- [Library tests TEST-2026-0010](../../testing/test-2026-0010-library.md)
- [Usage reference](../../references/ref-2026-0002-language-and-usage.md) · [Proposal](../../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 9 | 2026-09-30 | Claude | v0.2.0 release (PLAN-2026-0002 TASK-091): version strings and current-release wording updated to 0.2.0. |
| 8 | 2026-09-29 | Claude | INC-2026-0012 re-verification (T-30) at 7c25175: the scratch embedding crate was compiled and run again; step 2 output replaced by that run (terminal record `"seq":4` confirmed by the program itself); one Verification Record row. |
| 7 | 2026-09-29 | Claude | INC-2026-0012: the library terminal record carries `"seq":4` like the CLI's; the step-2 terminal line is updated to the fixed shape (IDs from the TASK-076 run; the `seq` key is verified by `conformance_verification_defects::library_terminal_records_carry_seq`, the scratch program was not re-run); `seq` caveat removed. |
| 6 | 2026-09-29 | Claude | TASK-076 (PLAN-2026-0002 D-61): embedding.rs.txt rewritten against the 0.2.0 facade (`rivet::{Runtime, Policy, Value, InputEnvelope, Completion, Envelope}`, `rivet::types::IoQuery`, `rivet::Result`, `.file`, `Runtime::call`, `to_json_pretty`, `build_features`), matching `examples/embed.rs`; scratch crate depends on `package = "rivet-runtime"`; CLI step uses `--data` and 0.2.0 envelopes; 0.1.0→0.2.0 migration table; 0.2.0 Release Updates; terminal-record `seq` caveat; verified_against 0.2.0 |
| 5 | 2026-09-28 | Claude | TASK-067: compiled embedding.rs.txt as a scratch crate against 0.1.0-dev (829ca43) and ran it from this folder; pasted the real output (outputs, manifest, draft) and the CLI comparison; replaced the outdated sketch (`.source(src)`, `json!` params, `rt.outputs("demo.add")`) with the real API (`.source(path, src, root)`, `Value::from_json`, `.ceiling`, `rt.outputs(Some(id), false)`); removed draft disclaimers; status active; verified_against 0.1.0. |
| 4 | 2026-09-28 | Claude | TASK-005/R26 (ADR-0001, proposal revision 8): `io --by target` header gains ORIGIN, PHASE, NEEDS FILE (still no sites); verified-against revision 8. |
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest: no sites; `io --check-policy` exits 0; library `rt.io` / `rt.generate_policy` equivalents. |
| 2 | 2026-09-28 | Claude | UQ-17/E11: embedding sketch replaced by the canonical builder (`Policy::from_file`, `rt.request`, `rt.scope`/`scope.stream`, `rt.outputs`); added grant-free policy.json; declared outputs; removed `--sandbox`; View outputs, strict-docs, bootstrap and exit codes. |
| 1 | 2026-09-28 | Codex | Added draft source files, prerequisites, invocation examples and expected behavior. |
