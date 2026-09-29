---
document_id: TEST-2026-0042
title: "T-09 — An external crate uses only the facade (UC-05 / R10)"
document_type: test
status: completed
created_date: 2026-09-29
last_updated: 2026-09-29
document_revision: 1
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [library]
affected_versions:
  from: "0.2.0"
  to: null
validated_plan_requirements: [PLAN-2026-0002 R10]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and recorded result for PLAN-2026-0002 test T-09 (PROP-2026-0002 T-09).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates (PLAN-2026-0002 TASK-061).
related_documents: [PLAN-2026-0002, PROP-2026-0002, ADR-0004]
supersedes: null
superseded_by: null
tags: [rivet, test, v0.2.0]
---

# T-09 — An external crate uses only the facade (UC-05 / R10)

> **Status:** Completed
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** library

## Purpose

Show that an embedder builds and runs against the public facade only (`rivet::{Runtime, Policy, Value, InputEnvelope, ResponseEnvelope, …}`, `rivet::types`, `rivet::serve`).

```text
  [dependencies] rivet = { package = "rivet-runtime", … }
        use rivet::{Runtime, InputEnvelope, …};   ── facade only ──►  examples/embed.rs
                                                                       │ cargo run --example embed
                                                                       ▼
                         envelopes · stream records · outputs · manifest · policy draft
```

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0002 R10 | Package `rivet-runtime`, library `rivet`, public facade; internals under `#[doc(hidden)] rivet::internal` |

Plan row: [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md#test-and-validation-checklist) T-09 (build, UC-05).

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first so `librivet` exists for the FFI suites (TRBL-2026-0006); Python 3 and a C compiler on `PATH`. Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

## Test Data

The fixtures, bundles and goldens defined in `tests/conformance_verification_defects.rs`, `tests/conformance_library.rs` and the shared helpers under `tests/` (in-process servers, temporary directories, `tests/fixtures/`).

## Procedure

```sh
cargo run --example embed --all-features
cargo test --all-features --test conformance_verification_defects
cargo test --all-features --test conformance_library
```

Tests executed (13):

- `conformance_verification_defects::facade_exposes_sessions_trace_and_serve` — INC-2026-0012 item 15: the facade exposes the session methods' types (rivet::types::Session*), rivet::TraceResult and rivet::serve::{start, ServeOptions} without rivet::internal
- `conformance_library::data_sink_that_stops_early` — S70/S71: a DataSink receives ordered items; a sink that stops early ends the request with consumer_failed (500 / exit 5) and no further items are produced
- `conformance_library::data_sink_typed_stop_cancels` — G33: a DataSink that returns the typed stop (RivetError::consumer_stop) ends the request `cancelled` / consumer.stop, not consumer_failed, and no further items are produced
- `conformance_library::dropped_future_closes_the_socket` — dropping a request future mid-flight closes its open socket: the local fixture observes the client close, also for a nested child request
- `conformance_library::dropped_future_kills_the_child_process`
- `conformance_library::io_manifest_and_policy_draft_from_the_library` — rt.io lists the HTTP and exec sites of the bundle and rt.generate_policy drafts exactly those grants without writing a file
- `conformance_library::outputs_from_the_library` — S106: rt.outputs returns the declared output (integer, "Sum of a and b."); private and unknown IDs are not_found
- `conformance_library::policy_constructors_and_host_ceiling` — G10: Policy::from_file / Policy::from_json load the policy.json schema, and builder .ceiling(Policy) intersects: a target must be allowed by both, a ceiling never grants and its deny wins
- `conformance_library::policy_from_json_matches_policy_files` — policy_from_json (Policy::from_json) accepts the policy.json schema; an empty policy denies new I/O; invalid JSON or a foreign access verb is policy.invalid
- `conformance_library::runtime_lives_inside_the_host_runtime` — S106: one in-memory compilation serves many calls inside the host's current-thread Tokio runtime; building never starts a nested runtime
- `conformance_library::scope_cancels_and_joins_what_it_started` — G24: everything a scope started is cancelled and joined when the scope body returns: an in-flight HTTP call's socket is already closed when `scope` returns
- `conformance_library::scope_stream_and_duplex` — G24: rt.scope gives scope.stream (ordered Data envelopes, one terminal Result, then None) and scope.duplex (send checked against `receives`, finish_send, next; split halves)
- `conformance_library::stream_pull_in_the_host` — the host pulls a request's stream as ordered Data envelopes followed by exactly one terminal Result (the library stream API)

## Expected Results

Builds; prints an envelope (plan T-09).

## Actual Results

`cargo run --example embed --all-features` built and exited 0, printing a pretty ok envelope for `demo.add` (`data` 5), an error envelope (`validation.type`), three stream data records and the terminal result (`seq` 4, `data_count` 3), the declared outputs, the manifest and a policy draft (23 lines). `examples/embed.rs` imports only the facade. `facade_exposes_sessions_trace_and_serve` passed (session types, `TraceResult`, `rivet::serve`). The scratch-crate leg (a separate crate depending on `rivet-runtime` by path, compiling `docs/demos/12-library/embedding.rs.txt`) is recorded as PASS in DEMO-2026-0012 step 2 (commit 8031baa). CI builds all examples on Linux and macOS (`--all-targets`, run 36505156729).

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

## Evidence

```text
conformance_verification_defects     test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.64s
conformance_library                  test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s

GitHub Actions run 36505156729 (commit 14750b8, conclusion: success)
  test (ubuntu-latest)  success    test (macos-latest)  success
  features (none|serve|grpc|quic|oauth|cli)  success ×6    deny  success
```

```text
$ cargo run --example embed --all-features
{
  "request_id": "req_015dcb93cd",          (varies per run)
  "trace_id": "tr_015dcb93cd",
  "operation": "demo.add",
  "type": "result",
  "status": "ok",
  "data": 5,
  "error": null,
  "effects": "none",
  "data_count": 0
}
{…"operation":"events.count","type":"result","seq":4,"status":"ok","data":{"count":3},…,"data_count":3}
EXIT=0
```

## Evidence Sources

- The commands above, run at commit `14750b8`; full-suite output saved to `/tmp/p3-tests.log` (493 passed, 0 failed) and read with `grep -E "test result|FAILED|panicked"`.
- Test source: `tests/conformance_verification_defects.rs`, `tests/conformance_library.rs`.
- CI: [run 36505156729](https://github.com/olivierdevelops/rivet/actions/runs/36505156729) at the same commit (`test (ubuntu-latest)`, `test (macos-latest)`, the six `features` jobs and `deny`, all `success`).
- Release verification cross-reference: [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md).

## Executed By

Claude (automated run, PLAN-2026-0002 P3).

## Executed At

2026-09-29T14:23:39Z

## Defects Raised

None in this run ([INC-2026-0012](../incidents/resolved/inc-2026-0012-documentation-and-demo-verification-defects.md) item 15 fixed before it).

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-09
- [PROP-2026-0002](../proposals/implemented/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md)
- [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md) · [Tests index](index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-29 | Claude | Created and executed at commit `14750b8` (PLAN-2026-0002 P3, TASK-061); result PASS. |
