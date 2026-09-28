---
document_id: TEST-2026-0016
title: "T-16 — unit / integration (UC-15 / R20)"
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
validated_plan_requirements: [PLAN-2026-0001 R20]
environment: "macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)"
executed_by: Claude (automated)
executed_at: 2026-09-28T10:44:46Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-16.
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-16 — unit / integration (UC-15 / R20)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

Multi-op file; duplicate in second declaration/import; private helper

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R20 | 017, 019 |

## Preconditions

A clean checkout at commit `f15a82b`; Rust toolchain from `rust-toolchain.toml`; fixtures are started in-process on `127.0.0.1:0` by the tests (no external services).

## Test Environment

macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in `tests/conformance_operation_catalog.rs` and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
cargo test conformance_operation_catalog
```

Tests executed:

- `capabilities_builtin_reports_this_build`
- `duplicate_id_rejects_the_whole_candidate`
- `metadata_is_identical_on_every_surface`
- `private_helper_is_hidden_but_callable_in_bundle`
- `reserved_rivet_ids_are_rejected`
- `strict_docs_findings`

## Expected Results

Atomic load; both spans shown

## Actual Results

6 passed, 0 failed, 0 ignored.

## Result

PASS

## Evidence

```text
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.56s
```

## Evidence Sources

- Command above, run at commit `f15a82b`.
- Test source: `tests/conformance_operation_catalog.rs`.

## Executed By

Claude (automated run).

## Executed At

2026-09-28T10:44:46Z

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-16
- [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f15a82b`. |
