---
document_id: TEST-2026-0025
title: "T-25 — integration (UC-21, UC-09 / R6, R26)"
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
validated_plan_requirements: [PLAN-2026-0001 R6, PLAN-2026-0001 R26]
environment: "macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)"
executed_by: Claude (automated)
executed_at: 2026-09-28T10:45:07Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-25.
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-25 — integration (UC-21, UC-09 / R6, R26)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

exact/bounded/param_dependent/dynamic/opaque sites of every kind; views; formats; `--check-policy` exit 3; `--strict` exit 7; `--trace` (+ option-derived sites if TASK-005 is approved)

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R6 | 037, 038 |
| PLAN-2026-0001 R26 | 038, 039 |

## Preconditions

A clean checkout at commit `f15a82b`; Rust toolchain from `rust-toolchain.toml`; fixtures are started in-process on `127.0.0.1:0` by the tests (no external services).

## Test Environment

macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in `tests/conformance_io_manifest.rs` and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
cargo test conformance_io_manifest
```

Tests executed:

- `demo02_by_target_and_check_policy`
- `demo02_needs_and_access_filter`
- `demo03_http_rows`
- `demo11_json_manifest`
- `demo11_needs_and_check_files`
- `demo11_views`
- `fixtures_exist`
- `s141_s143_by_target`
- `s144_check_policy_partial_denied_unknown`
- `s146_access_narrowing`
- `s153_trace_join`
- `s154_s155_option_sites_and_needs`
- `s156_s157_check_files`
- `s62_json_sites`
- `s63_s64_entry_and_bootstrap`
- `s65_strict_incomplete_exits_7`

## Expected Results

Manifest equals the golden JSON

## Actual Results

16 passed, 0 failed, 0 ignored.

## Result

PASS

## Evidence

```text
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.72s
```

## Evidence Sources

- Command above, run at commit `f15a82b`.
- Test source: `tests/conformance_io_manifest.rs`.

## Executed By

Claude (automated run).

## Executed At

2026-09-28T10:45:07Z

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-25
- [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f15a82b`. |
