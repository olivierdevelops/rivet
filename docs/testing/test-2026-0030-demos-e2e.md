---
document_id: TEST-2026-0030
title: "T-30 — manual / e2e (all UCs / R1–R26)"
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
validated_plan_requirements: [PLAN-2026-0001 R1, PLAN-2026-0001 R26, PLAN-2026-0002 R18]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PARTIAL
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-30; re-recorded for v0.2.0 (PLAN-2026-0002 T-30).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001, PLAN-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-30 — manual / e2e (all UCs / R1–R26)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0, 0.2.0 (re-recorded)
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

Execute the 12 sample READMEs and DEMO-2026-0014 step by step against the v0.1.0 build, then the release artifact

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R1 | 010, 014, 016 |
| PLAN-2026-0001 R26 | 038, 039 |
| PLAN-2026-0002 R18 (all UCs) | T-30 manual / e2e: demos 01–17 and DEMO-2026-0020 executed step by step against the release candidate |

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`, the v0.2.0 release candidate); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first (the FFI suites need `librivet`). Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

v0.1.0 run: commit `64c93d8`.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

v0.1.0 run: macOS (aarch64-apple-darwin), Rust 1.90.0, cargo-deny 0.20.2, vhco CLI; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in Manual: follow each README; compare expected output and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
# Follow each docs/demos/NN-*/README.md and DEMO-2026-0020 step by step against the release candidate
# (target/release/rivet, librivet, the .vsix), compare the output, fill the Verification Record.
```

Tests executed:

- (manual procedure; records live in each demo README's Verification Record)

v0.1.0 run: `Clean checkout at the release commit`.

## Expected Results

Every step matches; verification records filled

## Actual Results

### v0.2.0 run (2026-09-29, commit `14750b8`)

This run summarizes the demo verification records written during P4 (TASK-075…077); the demos were not re-executed for this document. On macOS 26.4.1 arm64 every step of folders 01–12 and 14–17 is recorded PASS against the release candidate (commits `8031baa` and `166a98b`), each folder `verified_against: "0.2.0"`; 13-real-world-apis depends on third-party services and is not part of the pass/fail decision, as in 0.1.0. DEMO-2026-0020 records U-01…U-24 PASS, with three items evidenced only by conformance suites (`unsupported.feature`, `limit.imports`, the `v0.2.0` git-tag dependency). Linux: the suites behind the demos are green in CI (runs 36483001760 and 36505156729); the demos themselves were not executed on a Linux host. **Result PARTIAL**, for concrete reasons: (1) the plan's criterion is "executed … against the RC" and "every step matches", but the demo records predate the INC-2026-0012 fix commits (`4122353`, `1034636`, `4a34537`); the documentation was updated to the fixed behaviour (`55b9e73`) and the regressions are tested, but the changed steps have not been re-executed; (2) DEMO-2026-0020's `Tagged release build v0.2.0` row is NOT STARTED until P5 (TASK-093), and the U-12 git-tag dependency cannot be checked before the tag exists. Windows is not a reason for PARTIAL (unsupported, INC-2026-0011).

### v0.1.0 run (2026-09-28, commit `64c93d8`)

All 12 release sample folders (01-catalog … 12-library) were executed step by step from their READMEs against the release candidate (commit 829ca43, target/release/rivet, macOS 26.4.1 arm64) with local fixtures. Every step matches, and each folder is now status active, verified_against 0.1.0, runtime_verified true. The run found six defects; they were fixed in 2a751ab (INC-2026-0007), and the affected READMEs (02, 03, 04, 05, 10) were corrected against the fixed build. DEMO-2026-0015 (release verification guide) records U-01…U-26. Result PARTIAL, for two reasons: the Linux/Windows sandbox rows and the published release artifact (TASK-081) cannot be run here, and U-16 demonstrates only client_credentials (the other OAuth flows are covered by T-11).

## Result

PARTIAL

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)) and is not among the reasons above.

v0.1.0 run: PARTIAL.

## Evidence

v0.2.0 run:

```text
folder / guide                         id              verified_against  steps  recorded at            result
01-catalog … 12-library (12 folders)   DEMO-2026-0001…0012  "0.2.0"     5–8 each  8031baa, macOS arm64   PASS (every step)
13-real-world-apis                     DEMO-2026-0014  "0.2.0" (draft)   —      public APIs            not part of pass/fail
14-globals                             DEMO-2026-0016  "0.2.0"           7      8031baa               PASS
15-ffi                                 DEMO-2026-0017  "0.2.0"           6+Linux 8031baa/166a98b; CI 36483001760  PASS
16-editor                              DEMO-2026-0018  "0.2.0"           6      166a98b (VS Code 1.108.1)  PASS
17-modules                             DEMO-2026-0019  "0.2.0"           8      166a98b               PASS
release guide U-01…U-24                DEMO-2026-0020  "0.2.0"           24     166a98b / 8031baa     PASS; tagged build NOT STARTED (P5)
Linux                                  CI run 36483001760 (tests only; demos not executed on a Linux host)
Windows                                unsupported in 0.2.0 (INC-2026-0011)
```

v0.1.0 run:

```text
13-real-world-apis     runtime_verified=None
01-catalog             runtime_verified=True
02-file-crud           runtime_verified=True
03-http                runtime_verified=True
04-streaming           runtime_verified=True
05-dag                 runtime_verified=True
06-mcp-bridge          runtime_verified=True
07-oauth2              runtime_verified=True
08-udp                 runtime_verified=True
09-quic                runtime_verified=True
10-grpc                runtime_verified=True
11-sandbox             runtime_verified=True
12-library             runtime_verified=True
01-catalog             status=active verified_against="0.1.0"
02-file-crud           status=active verified_against="0.1.0"
03-http                status=active verified_against="0.1.0"
04-streaming           status=active verified_against="0.1.0"
05-dag                 status=active verified_against="0.1.0"
06-mcp-bridge          status=active verified_against="0.1.0"
07-oauth2              status=active verified_against="0.1.0"
08-udp                 status=active verified_against="0.1.0"
09-quic                status=active verified_against="0.1.0"
10-grpc                status=active verified_against="0.1.0"
11-sandbox             status=active verified_against="0.1.0"
12-library             status=active verified_against="0.1.0"
13-real-world-apis     status=draft verified_against=not-verified
ALL CHECKS PASSED
```

## Evidence Sources

- Verification Record sections of `docs/demos/01-catalog/README.md` … `docs/demos/17-modules/README.md` and [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md), read at commit `14750b8`.
- Front matter summary: `status`, `verified_against` of every `docs/demos/*/README.md`.

v0.1.0 run:

- Command above, run at commit `64c93d8`.
- Test source: Manual: follow each README; compare expected output.

## Executed By

Claude (automated run).

## Executed At

2026-09-29T14:23:39Z (v0.2.0 run; v0.1.0 run: 2026-09-28T10:28:03Z)

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-30
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-30
- [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `64c93d8`. |
| 2 | 2026-09-29 | Claude | Re-recorded for v0.2.0 at commit `14750b8` (PLAN-2026-0002 T-30, TASK-061): PARTIAL; still PARTIAL (records predate the INC-2026-0012 fixes; tagged-build run is P5). v0.1.0 run kept as history. |
