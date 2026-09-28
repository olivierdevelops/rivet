---
document_id: TEST-2026-0009
title: "T-09 — security / audit (UC-09 / R6, R13)"
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
validated_plan_requirements: [PLAN-2026-0001 R6, PLAN-2026-0001 R13]
environment: "macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)"
executed_by: Claude (automated)
executed_at: 2026-09-28T10:44:26Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-09.
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-09 — security / audit (UC-09 / R6, R13)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

Every adapter; dynamic/opaque sites; secret canaries; failed sink

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R6 | 037, 038 |
| PLAN-2026-0001 R13 | 021, 035, 037, 093 |

## Preconditions

A clean checkout at commit `f15a82b`; Rust toolchain from `rust-toolchain.toml`; fixtures are started in-process on `127.0.0.1:0` by the tests (no external services).

## Test Environment

macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in `tests/conformance_audit.rs` and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
cargo test conformance_audit
```

Tests executed:

- `every_site_is_reported`
- `graph_cli`
- `secret_values_never_printed`
- `static_call_graph_tree_and_json`
- `trace_export_builtin_dispatches`
- `trace_export_needs_write_grant_and_never_overwrites`
- `trace_records_denials_with_effect_ids`

## Expected Results

Every site reported; no canary leaks

## Actual Results

7 passed, 0 failed, 0 ignored.

## Result

PASS

## Evidence

```text
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.42s
```

## Evidence Sources

- Command above, run at commit `f15a82b`.
- Test source: `tests/conformance_audit.rs`.

## Executed By

Claude (automated run).

## Executed At

2026-09-28T10:44:26Z

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-09
- [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f15a82b`. |
