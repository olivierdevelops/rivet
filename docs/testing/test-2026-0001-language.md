---
document_id: TEST-2026-0001
title: "T-01 — unit + integration (UC-01 / R1, R2)"
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
validated_plan_requirements: [PLAN-2026-0001 R1, PLAN-2026-0001 R2]
environment: "macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)"
executed_by: Claude (automated)
executed_at: 2026-09-28T06:36:40Z
result: PARTIAL
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-01.
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-01 — unit + integration (UC-01 / R1, R2)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

+: all grammar shapes parse; −: invalid sources run zero effects; spans exact

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R1 | 010, 014, 016 |
| PLAN-2026-0001 R2 | 016, 017 |

## Preconditions

A clean checkout at commit `19bd7c3`; Rust toolchain from `rust-toolchain.toml`; fixtures are started in-process on `127.0.0.1:0` by the tests (no external services).

## Test Environment

macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in `tests/conformance_language.rs` and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
cargo test conformance_language
```

Tests executed:

- `compiles_the_catalog_demo`
- `duplicate_ids_fail_with_both_spans`
- `fcall_style_is_rejected_with_a_prefix_hint`
- `header_after_body_is_option_after_body`
- `literal_call_cycles_are_rejected`

## Expected Results

PASS; 0 effects on invalid

## Actual Results

5 passed, 0 failed, 0 ignored on macOS. The plan requires Linux and Windows too; those runs are pending CI.

## Result

PARTIAL

## Evidence

```text
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

## Evidence Sources

- Command above, run at commit `19bd7c3`.
- Test source: `tests/conformance_language.rs`.

## Executed By

Claude (automated run).

## Executed At

2026-09-28T06:36:40Z

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-01
- [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `19bd7c3`. |
