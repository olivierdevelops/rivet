---
document_id: TEST-2026-0028
title: "T-28 — architecture (all / R14)"
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
validated_plan_requirements: [PLAN-2026-0001 R14, PLAN-2026-0002 R1, PLAN-2026-0002 R24]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-28; re-recorded for v0.2.0 (PLAN-2026-0002 T-28).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001, PLAN-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-28 — architecture (all / R14)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0, 0.2.0 (re-recorded)
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

VHCO structure and drift

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R14 | 004, 006, 052–072, 083–092 |
| PLAN-2026-0002 (all) | T-28 architecture: VHCO structure and zero contract drift after P2a–P2f and INC-2026-0012 |

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`, the v0.2.0 release candidate); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first (the FFI suites need `librivet`). Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

v0.1.0 run: commit `f15a82b`.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

v0.1.0 run: macOS (aarch64-apple-darwin), Rust 1.90.0, cargo-deny 0.20.2, vhco CLI; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in — and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
vhco validate .
vhco sync .
vhco check .
vhco assure .
```

Tests executed:

- (command procedure; `vhco sync` must report 0 gaps)

v0.1.0 run: `vhco validate . && vhco sync . && vhco check . && vhco spec .`.

## Expected Results

validate green; sync 0; check green

## Actual Results

### v0.2.0 run (2026-09-29, commit `14750b8`)

`vhco validate .` — no violations (Exhaustive Use Case Invariant holds); `vhco sync .` — code matches `vhco-contract.json` (0 gaps, including the P2c–P2f and INC-2026-0012 hand-edited entries); `vhco check .` — 1 guarantee holds (`http-routes-documented`: all 7 routes documented); `vhco assure .` — validate, sync, guarantees, tests (3 commands), docs (0 blocking) and release (0.1.0 tag) green. All exit 0 (vhco 1.6.0). `vhco spec .` is not part of the PLAN-2026-0002 T-28 command; the generated `vhco.json` is refreshed separately (PF-G01).

### v0.1.0 run (2026-09-28, commit `f15a82b`)

vhco validate: no violations; vhco sync: code matches vhco-contract.json (0 drift); vhco check: the one declared guarantee (http-routes-documented) holds; vhco spec regenerated vhco.json.

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

v0.1.0 run: PASS.

## Evidence

v0.2.0 run:

```text
== validate
✓ no violations — the Exhaustive Use Case Invariant holds
EXIT=0
== sync
✓ code matches vhco-contract.json
EXIT=0
== check
✓ 1 guarantee(s) hold
  ✓ http-routes-documented [api.every_route_has_request_response] — all 7 routes are documented with request + response
EXIT=0

vhco 1.6.0
$ vhco assure .
  ✓ validate    architecture rules pass
  ✓ sync        code matches the design contract
  ✓ guarantees  1 guarantee(s) hold
  ✓ tests       3 test command(s) passed
  ✓ docs        no blocking documentation findings (66 warning(s))
  ✓ release     release 0.1.0 matches tag and commit
exit codes: validate=0 sync=0 check=0 assure=0
```

v0.1.0 run:

```text
✓ no violations — the Exhaustive Use Case Invariant holds
✓ code matches vhco-contract.json
✓ 1 guarantee(s) hold
  ✓ http-routes-documented [api.every_route_has_request_response] — all 7 routes are documented with request + response
```

## Evidence Sources

- The commands above, run at commit `14750b8`; log `/tmp/p3-gates/vhco.log`.
- Contract: `vhco-contract.json`.

v0.1.0 run:

- Command above, run at commit `f15a82b`.
- Test source: —.

## Executed By

Claude (automated run).

## Executed At

2026-09-29T14:23:39Z (v0.2.0 run; v0.1.0 run: 2026-09-28T10:45:52Z)

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-28
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-28
- [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f15a82b`. |
| 2 | 2026-09-29 | Claude | Re-recorded for v0.2.0 at commit `14750b8` (PLAN-2026-0002 T-28, TASK-061): PASS. v0.1.0 run kept as history. |
