---
document_id: TEST-2026-0029
title: "T-29 — regression / corpus (UC-01 / R2, R14)"
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
validated_plan_requirements: [PLAN-2026-0001 R2, PLAN-2026-0001 R14]
environment: "macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)"
executed_by: Claude (automated)
executed_at: 2026-09-28T10:45:11Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-29.
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-29 — regression / corpus (UC-01 / R2, R14)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

Every Stage A/B example and demo parses; demo ops run on fixtures

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R2 | 016, 017 |
| PLAN-2026-0001 R14 | 004, 006, 052–072, 083–092 |

## Preconditions

A clean checkout at commit `f15a82b`; Rust toolchain from `rust-toolchain.toml`; fixtures are started in-process on `127.0.0.1:0` by the tests (no external services).

## Test Environment

macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in `tests/conformance_samples.rs` and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
cargo test conformance_samples
```

Tests executed:

- `every_demo_bundle_compiles`
- `every_reference_block_parses`
- `every_reference_fragment_lowers`

## Expected Results

100% of in-scope samples pass; Stage C samples refuse

## Actual Results

3 passed, 0 failed, 0 ignored.

## Result

PASS

## Evidence

```text
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.28s
```

## Evidence Sources

- Command above, run at commit `f15a82b`.
- Test source: `tests/conformance_samples.rs`.

## Executed By

Claude (automated run).

## Executed At

2026-09-28T10:45:11Z

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-29
- [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f15a82b`. |
