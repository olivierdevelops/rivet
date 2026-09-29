---
document_id: TEST-2026-0032
title: "T-32 — traceability (R1–R26)"
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
validated_plan_requirements: [PLAN-2026-0001 R1, PLAN-2026-0001 R26, PLAN-2026-0002 R1…R24]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-32; re-recorded for v0.2.0 (PLAN-2026-0002 T-32).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001, PLAN-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-32 — traceability (R1–R26)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0, 0.2.0 (re-recorded)
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

Every R → task → file → test → doc → U-NN row; no orphan either way

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R1 | 010, 014, 016 |
| PLAN-2026-0001 R26 | 038, 039 |
| PLAN-2026-0002 R1–R24 | T-32 traceability: R → TASK → PF → T → D → U (DEMO-2026-0020); no orphans |

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`, the v0.2.0 release candidate); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first (the FFI suites need `librivet`). Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

v0.1.0 run: commit `f15a82b`.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

v0.1.0 run: macOS (aarch64-apple-darwin), Rust 1.90.0, cargo-deny 0.20.2, vhco CLI; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in Manual + script over this plan, RPT and REL and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
python3 trace_check.py      # scratch script, run from the repository root
```

The script reads the plan's Final Traceability table and checks, for every R1–R24: tasks exist in the Live Work Checklist; PF-IDs exist in the File and Artifact Checklist; T-IDs exist in the Test and Validation Checklist and T-01…T-20 name an existing TEST document; D-IDs exist in the Documentation and Demo Checklist; each U-NN exists in DEMO-2026-0020 and maps back to the same requirement. Reverse direction: every requirement is traced, every T-01…T-20 and every U row has a requirement, and every production PF row is used (PF-15 contract and PF-18 AGENTS.md are cross-cutting by design).

Tests executed:

- (script procedure)

v0.1.0 run: `—`.

## Expected Results

No orphans

## Actual Results

### v0.2.0 run (2026-09-29, commit `14750b8`)

No orphans in either direction: 24 requirements, 24 traced rows, 51 tasks, 22 PF rows, 25 plan tests, 72 D-rows and 24 U-rows (DEMO-2026-0020 has exactly U-01…U-24, each mapping back to its requirement), and all 20 TEST documents TEST-2026-0034…0053 exist. Before the TEST documents were written the same script reported 20 problems (the missing TEST-2026-0034…0053 files), which this change resolves. The REL-0.2.0 release-updates table is generated from the same U rows at P5.

### v0.1.0 run (2026-09-28, commit `f15a82b`)

Every requirement R1–R26 traces through the plan's Final Traceability table to existing TASK rows, TEST documents, D-rows in the Documentation and Demo Checklist, and a U-NN row in DEMO-2026-0015. The only reported miss is T-32, which this document creates. Every suite TEST document traces back to at least one requirement, so there are no orphans in either direction. The REL-0.1.0 Released Updates table is generated from the same U-NN rows at release time.

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

v0.1.0 run: PASS.

## Evidence

v0.2.0 run:

```text
$ python3 trace_check.py
requirements: 24  traced rows: 24
tasks: 51  PF: 22  tests: 25  D: 72  U: 24 (DEMO-2026-0020 rows: 24)
PF rows not tied to a requirement by design: PF-15 (contract, all), PF-18 (AGENTS.md, R18 via D-70)
trace_check: 0 problem(s)
```

v0.1.0 run:

```text
REQ   TASKS  TESTS  DOCS   U-ROW 
R1    ok     ok     ok     ok
R2    ok     ok     ok     ok
R3    ok     ok     ok     ok
R4    ok     ok     ok     ok
R5    ok     ok     ok     ok
R6    ok     ok     ok     ok
R7    ok     ok     ok     ok
R8    ok     ok     ok     ok
R9    ok     ok     ok     ok
R10   ok     ok     ok     ok
R11   ok     ok     ok     ok
R12   ok     ok     ok     ok
R13   ok     ok     ok     ok
R14   ok     MISS:T-32 ok     ok
R15   ok     ok     ok     ok
R16   ok     ok     ok     ok
R17   ok     ok     ok     ok
R18   ok     ok     ok     ok
R19   ok     ok     ok     ok
R20   ok     ok     ok     ok
R21   ok     ok     ok     ok
R22   ok     ok     ok     ok
R23   ok     ok     ok     ok
R24   ok     ok     ok     ok
R25   ok     ok     ok     ok
R26   ok     ok     ok     ok
tests not traced to a requirement: none
orphans: 1
```

## Evidence Sources

- `trace_check.py` (session scratch script) over `docs/plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md` and [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md) at commit `14750b8` plus the new TEST documents.

v0.1.0 run:

- Command above, run at commit `f15a82b`.
- Test source: Manual + script over this plan, RPT and REL.

## Executed By

Claude (automated run).

## Executed At

2026-09-29T14:23:39Z (v0.2.0 run; v0.1.0 run: 2026-09-28T10:46:18Z)

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-32
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-32
- [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f15a82b`. |
| 2 | 2026-09-29 | Claude | Re-recorded for v0.2.0 at commit `14750b8` (PLAN-2026-0002 T-32, TASK-061): PASS. v0.1.0 run kept as history. |
