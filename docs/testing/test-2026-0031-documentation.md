---
document_id: TEST-2026-0031
title: "T-31 — documentation (R14)"
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
validated_plan_requirements: [PLAN-2026-0001 R14, PLAN-2026-0002 R18]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-31; re-recorded for v0.2.0 (PLAN-2026-0002 T-31).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001, PLAN-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-31 — documentation (R14)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0, 0.2.0 (re-recorded)
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

Front matter, IDs, links, anchors, fences, headers, revisions, index membership; ASCII visual presence in manuals/system/API

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R14 | 004, 006, 052–072, 083–092 |
| PLAN-2026-0002 R18 | T-31 documentation: `check_docs` 0 problems and `vhco docs check` 0 errors on the P3 documentation set |

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`, the v0.2.0 release candidate); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first (the FFI suites need `librivet`). Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

v0.1.0 run: commit `f15a82b`.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

v0.1.0 run: macOS (aarch64-apple-darwin), Rust 1.90.0, cargo-deny 0.20.2, vhco CLI; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in `scripts/check_docs.py` and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
python3 scripts/check_docs.py
vhco docs check .
```

Tests executed:

- (command procedure; 0 problems / 0 errors required)

v0.1.0 run: `python3 scripts/check_docs.py`.

## Expected Results

0 errors

## Actual Results

### v0.2.0 run (2026-09-29, commit `14750b8`)

On the working tree with the P3 TEST documents and the updated testing index (parent commit `14750b8`): `check_docs: 180 files, 0 problem(s)`; `vhco docs check .` → `66 findings (0 error, 66 warning)` — the warnings are index-page and folder-placement naming findings (DOC-DIR-001 and STD-2026-0002 §5 rename hints), none blocking. Both commands are re-run before every P3 commit (0 problems / 0 errors each time). This covers the P3 documents (TEST, RPT, plan). The P4 documentation checklist (D-rows) is validated by T-31 again at P4 exit.

### v0.1.0 run (2026-09-28, commit `f15a82b`)

scripts/check_docs.py found 0 problems in 128 files. vhco docs check found 0 errors. Its 59 warnings are all DOC-DIR-001 and DOC-NAME-001 on the directory index.md pages that AGENTS.md (Directory Indexes) requires. vhco's naming rule asks to rename or move them, which conflicts with the project rule, so the warnings are accepted. None of them block.

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

v0.1.0 run: PASS.

## Evidence

v0.2.0 run:

```text
$ python3 scripts/check_docs.py
check_docs: 180 files, 0 problem(s)
$ vhco docs check .
66 findings (0 error, 66 warning)
```

v0.1.0 run:

```text
check_docs: 128 files, 0 problem(s)
59 findings (0 error, 59 warning)
warnings by rule:
  30 DOC-DIR-001
  29 DOC-NAME-001
```

## Evidence Sources

- The commands above; `scripts/check_docs.py` (DOCUMENTATION §19); vhco 1.6.0.
- CI (the `docs (T-31)` step runs `check_docs.py`): [run 36505156729](https://github.com/olivierdevelops/rivet/actions/runs/36505156729) at the same commit (`test (ubuntu-latest)`, `test (macos-latest)`, six `features` jobs, `deny`: all `success`).

v0.1.0 run:

- Command above, run at commit `f15a82b`.
- Test source: `scripts/check_docs.py`.

## Executed By

Claude (automated run).

## Executed At

2026-09-29T14:23:39Z (v0.2.0 run; v0.1.0 run: 2026-09-28T10:45:54Z)

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-31
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-31
- [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f15a82b`. |
| 2 | 2026-09-29 | Claude | Re-recorded for v0.2.0 at commit `14750b8` (PLAN-2026-0002 T-31, TASK-061): PASS. v0.1.0 run kept as history. |
