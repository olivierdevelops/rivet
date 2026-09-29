---
document_id: TEST-2026-0010
title: "T-10 — integration (UC-10 / R1, R3, R9)"
document_type: test
status: completed
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 2
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [execution]
affected_versions:
  from: "0.1.0"
  to: null
validated_plan_requirements: [PLAN-2026-0001 R1, PLAN-2026-0001 R3, PLAN-2026-0001 R9, PLAN-2026-0002 R1]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-10; re-recorded for v0.2.0 (PLAN-2026-0002 T-34).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001, PLAN-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-10 — integration (UC-10 / R1, R3, R9)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0, 0.2.0 (re-recorded)
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

Host Tokio runtime; abandoned future; scope exit

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R1 | 010, 014, 016 |
| PLAN-2026-0001 R3 | 022, 033–035 |
| PLAN-2026-0001 R9 | 022, 025 |
| PLAN-2026-0002 R1 | T-34 regression: every 0.1.0 suite stays green on the 0.2.0 envelopes and input keys with no behaviour change |

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`, the v0.2.0 release candidate); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first (the FFI suites need `librivet`). Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

v0.1.0 run: commit `f15a82b`.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

v0.1.0 run: macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in `tests/conformance_library.rs` and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
cargo test --workspace --all-targets --all-features --no-fail-fast   # full run → /tmp/p3-tests.log
cargo test --all-features --test conformance_library
```

Tests executed (12):

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

v0.1.0 run: `cargo test conformance_library`.

## Expected Results

No nested runtime; cleanup joined

## Actual Results

### v0.2.0 run (2026-09-29, commit `14750b8`)

12 passed, 0 failed, 0 ignored on macOS at commit `14750b8` (v0.2.0 release candidate). This is the PLAN-2026-0002 T-34 regression: the 0.1.0 suite passes on the 0.2.0 envelopes and input keys (TASK-018 changed only request and response shapes). Green on ubuntu-latest and macos-latest in CI run 36505156729, which runs the same `cargo test --workspace --all-targets --all-features --no-fail-fast`. Since P2c the suite reaches internals through the facade or `rivet::internal::…` (the hidden module), with no behaviour change.

### v0.1.0 run (2026-09-28, commit `f15a82b`)

12 passed, 0 failed, 0 ignored.

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

v0.1.0 run: PASS.

## Evidence

v0.2.0 run:

```text
conformance_library                test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s

GitHub Actions run 36505156729 (commit 14750b8, conclusion: success)
  test (ubuntu-latest)  success    test (macos-latest)  success
  features (none|serve|grpc|quic|oauth|cli)  success ×6    deny  success
```

v0.1.0 run:

```text
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
```

## Evidence Sources

- The commands above, run at commit `14750b8`; full-suite output in `/tmp/p3-tests.log` (493 passed, 0 failed), read with `grep -E "test result|FAILED|panicked"`.
- Test source: `tests/conformance_library.rs`.
- CI: [run 36505156729](https://github.com/olivierdevelops/rivet/actions/runs/36505156729) at the same commit (`test (ubuntu-latest)`, `test (macos-latest)`, six `features` jobs, `deny`: all `success`).

v0.1.0 run:

- Command above, run at commit `f15a82b`.
- Test source: `tests/conformance_library.rs`.

## Executed By

Claude (automated run).

## Executed At

2026-09-29T14:23:39Z (v0.2.0 run; v0.1.0 run: 2026-09-28T10:44:27Z)

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-34
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-10
- [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f15a82b`. |
| 2 | 2026-09-29 | Claude | Re-recorded for v0.2.0 at commit `14750b8` (PLAN-2026-0002 T-34, TASK-061): PASS. v0.1.0 run kept as history. |
