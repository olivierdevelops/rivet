---
document_id: TEST-2026-0024
title: "T-24 — unit / resource (UC-02, UC-07 / R4, R10)"
document_type: test
status: completed
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [execution]
affected_versions:
  from: "0.1.0"
  to: null
validated_plan_requirements: [PLAN-2026-0001 R4, PLAN-2026-0001 R10]
environment: "macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)"
executed_by: Claude (automated)
executed_at: 2026-09-28T10:45:05Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-24.
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-24 — unit / resource (UC-02, UC-07 / R4, R10)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

Every registry code on every surface; 65 concurrent calls; depth 17; call cycle; unguarded `.result`

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R4 | 015, 022, 036 |
| PLAN-2026-0001 R10 | 036 |

## Preconditions

A clean checkout at commit `f15a82b`; Rust toolchain from `rust-toolchain.toml`; fixtures are started in-process on `127.0.0.1:0` by the tests (no external services).

## Test Environment

macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in `tests/conformance_errors_limits_dag.rs` and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
cargo test conformance_errors_limits_dag
```

Tests executed:

- `auth_failure_on_http_and_cli`
- `call_depth_limit`
- `check_warnings_for_unguarded_result_and_undeclared_codes`
- `cli_exit_codes_for_real_failures`
- `default_body_limit_is_8_mib_and_max_body_overrides`
- `http_status_for_real_failures`
- `library_cancel_by_request_id`
- `literal_call_cycle_is_rejected_by_check`
- `nested_calls_share_the_concurrency_budget`
- `output_invalid_preserves_effects`
- `registry_table_matches_the_reference`
- `sigint_cancels_with_exit_130`
- `sixty_fifth_concurrent_request_is_refused`

## Expected Results

Registry-consistent exit/HTTP

## Actual Results

13 passed, 0 failed, 0 ignored.

## Result

PASS

## Evidence

```text
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.30s
```

## Evidence Sources

- Command above, run at commit `f15a82b`.
- Test source: `tests/conformance_errors_limits_dag.rs`.

## Executed By

Claude (automated run).

## Executed At

2026-09-28T10:45:05Z

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-24
- [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f15a82b`. |
