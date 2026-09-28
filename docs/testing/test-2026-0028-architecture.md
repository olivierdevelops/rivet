---
document_id: TEST-2026-0028
title: "T-28 — architecture (all / R14)"
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
validated_plan_requirements: [PLAN-2026-0001 R14]
environment: "macOS (aarch64-apple-darwin), Rust 1.90.0, cargo-deny 0.20.2, vhco CLI; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)"
executed_by: Claude (automated)
executed_at: 2026-09-28T06:38:38Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-28.
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-28 — architecture (all / R14)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

VHCO structure and drift

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R14 | 004, 006, 052–072, 083–092 |

## Preconditions

A clean checkout at commit `19bd7c3`; Rust toolchain from `rust-toolchain.toml`; fixtures are started in-process on `127.0.0.1:0` by the tests (no external services).

## Test Environment

macOS (aarch64-apple-darwin), Rust 1.90.0, cargo-deny 0.20.2, vhco CLI; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in — and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
vhco validate . && vhco sync . && vhco check . && vhco spec .
```

Tests executed:

- (manual / command procedure)

## Expected Results

validate green; sync 0; check green

## Actual Results

vhco validate: no violations; vhco sync: code matches vhco-contract.json (0 drift); vhco check: the one declared guarantee (http-routes-documented) holds; vhco spec regenerated vhco.json.

## Result

PASS

## Evidence

```text
✓ no violations — the Exhaustive Use Case Invariant holds
✓ code matches vhco-contract.json
✓ 1 guarantee(s) hold
  ✓ http-routes-documented [api.every_route_has_request_response] — all 7 routes are documented with request + response
✓ wrote /Users/oliverlaleau/Documents/projects/rivet/vhco.json
```

## Evidence Sources

- Command above, run at commit `19bd7c3`.
- Test source: —.

## Executed By

Claude (automated run).

## Executed At

2026-09-28T06:38:38Z

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-28
- [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `19bd7c3`. |
