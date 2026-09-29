---
document_id: TEST-2026-0024
title: "T-24 — unit / resource (UC-02, UC-07 / R4, R10)"
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
validated_plan_requirements: [PLAN-2026-0001 R4, PLAN-2026-0001 R10, PLAN-2026-0002 R1]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-24; re-recorded for v0.2.0 (PLAN-2026-0002 T-34).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001, PLAN-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-24 — unit / resource (UC-02, UC-07 / R4, R10)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0, 0.2.0 (re-recorded)
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

Every registry code on every surface; 65 concurrent calls; depth 17; call cycle; unguarded `.result`

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R4 | 015, 022, 036 |
| PLAN-2026-0001 R10 | 036 |
| PLAN-2026-0002 R1 | T-34 regression: every 0.1.0 suite stays green on the 0.2.0 envelopes and input keys with no behaviour change |

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`, the v0.2.0 release candidate); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first (the FFI suites need `librivet`). Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

v0.1.0 run: commit `f15a82b`.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

v0.1.0 run: macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in `tests/conformance_errors_limits_dag.rs` and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
cargo test --workspace --all-targets --all-features --no-fail-fast   # full run → /tmp/p3-tests.log
cargo test --all-features --test conformance_errors_limits_dag
```

Tests executed (13):

- `conformance_errors_limits_dag::auth_failure_on_http_and_cli` — auth kind: a missing bearer token is 401 over HTTP and exit 3 through `rivet --endpoint`
- `conformance_errors_limits_dag::call_depth_limit` — limits.max_call_depth: depth 16 runs, depth 17 is limit.call_depth (429 / exit 5, retryable)
- `conformance_errors_limits_dag::check_warnings_for_unguarded_result_and_undeclared_codes` — `check` warns (exit 0) on an unguarded `.result` of a fail-independent node and on an undeclared `fail` code; `--strict-docs` makes the undeclared code an error (exit 2)
- `conformance_errors_limits_dag::cli_exit_codes_for_real_failures` — CLI: real failures of every drivable kind exit with the registry code and print one ErrorEnvelope on stderr
- `conformance_errors_limits_dag::default_body_limit_is_8_mib_and_max_body_overrides` — G11 default size limits follow the proposal (8 MiB frame/body, 16-frame / 32 MiB session queues, 256 MiB host budget): a 9 MiB HTTP body is limit.http_body by default and passes with an explicit `max_body`
- `conformance_errors_limits_dag::http_status_for_real_failures` — HTTP surface: the same failures answer POST /v1/request with the registry status and the same ErrorEnvelope kind/code
- `conformance_errors_limits_dag::library_cancel_by_request_id` — a library host cancels its own running request by ID: kind cancelled (409 before headers, exit 130) and the upstream call is aborted
- `conformance_errors_limits_dag::literal_call_cycle_is_rejected_by_check` — a literal `(request "ID" …)` cycle is check.call_cycle (syntax kind, exit 2) from `rivet check`; nothing runs
- `conformance_errors_limits_dag::nested_calls_share_the_concurrency_budget` — nested calls share the host-wide budget: a parent plus 63 concurrent nested calls fit in 64; one more nested call makes the 65th concurrent request and is limit.concurrency
- `conformance_errors_limits_dag::output_invalid_preserves_effects` — output.invalid: the result is checked before Completion, exit 5 / HTTP 500, and the committed effect (appended audit line) is preserved and reported
- `conformance_errors_limits_dag::registry_table_matches_the_reference` — every registry kind maps to the documented HTTP status, CLI exit and retryability (REF "Error registry")
- `conformance_errors_limits_dag::sigint_cancels_with_exit_130` — SIGINT during `rivet request` cancels the running request: one `cancelled` ErrorEnvelope, exit 130, the in-flight call is aborted
- `conformance_errors_limits_dag::sixty_fifth_concurrent_request_is_refused` — limits.max_concurrent_requests (64): with 64 requests in flight the 65th top-level request is limit.concurrency; capacity returns afterwards

v0.1.0 run: `cargo test conformance_errors_limits_dag`.

## Expected Results

Registry-consistent exit/HTTP

## Actual Results

### v0.2.0 run (2026-09-29, commit `14750b8`)

13 passed, 0 failed, 0 ignored on macOS at commit `14750b8` (v0.2.0 release candidate). This is the PLAN-2026-0002 T-34 regression: the 0.1.0 suite passes on the 0.2.0 envelopes and input keys (TASK-018 changed only request and response shapes). Green on ubuntu-latest and macos-latest in CI run 36505156729, which runs the same `cargo test --workspace --all-targets --all-features --no-fail-fast`.

### v0.1.0 run (2026-09-28, commit `f15a82b`)

13 passed, 0 failed, 0 ignored.

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

v0.1.0 run: PASS.

## Evidence

v0.2.0 run:

```text
conformance_errors_limits_dag      test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.60s

GitHub Actions run 36505156729 (commit 14750b8, conclusion: success)
  test (ubuntu-latest)  success    test (macos-latest)  success
  features (none|serve|grpc|quic|oauth|cli)  success ×6    deny  success
```

v0.1.0 run:

```text
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.30s
```

## Evidence Sources

- The commands above, run at commit `14750b8`; full-suite output in `/tmp/p3-tests.log` (493 passed, 0 failed), read with `grep -E "test result|FAILED|panicked"`.
- Test source: `tests/conformance_errors_limits_dag.rs`.
- CI: [run 36505156729](https://github.com/olivierdevelops/rivet/actions/runs/36505156729) at the same commit (`test (ubuntu-latest)`, `test (macos-latest)`, six `features` jobs, `deny`: all `success`).

v0.1.0 run:

- Command above, run at commit `f15a82b`.
- Test source: `tests/conformance_errors_limits_dag.rs`.

## Executed By

Claude (automated run).

## Executed At

2026-09-29T14:23:39Z (v0.2.0 run; v0.1.0 run: 2026-09-28T10:45:05Z)

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-34
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-24
- [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f15a82b`. |
| 2 | 2026-09-29 | Claude | Re-recorded for v0.2.0 at commit `14750b8` (PLAN-2026-0002 T-34, TASK-061): PASS. v0.1.0 run kept as history. |
