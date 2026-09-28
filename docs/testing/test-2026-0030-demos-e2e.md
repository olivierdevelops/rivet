---
document_id: TEST-2026-0030
title: "T-30 — manual / e2e (all UCs / R1–R26)"
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
validated_plan_requirements: [PLAN-2026-0001 R1, PLAN-2026-0001 R26]
environment: "macOS (aarch64-apple-darwin), Rust 1.90.0, cargo-deny 0.20.2, vhco CLI; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)"
executed_by: Claude (automated)
executed_at: 2026-09-28T10:28:03Z
result: PARTIAL
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-30.
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-30 — manual / e2e (all UCs / R1–R26)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

Execute the 12 sample READMEs and DEMO-2026-0014 step by step against the v0.1.0 build, then the release artifact

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R1 | 010, 014, 016 |
| PLAN-2026-0001 R26 | 038, 039 |

## Preconditions

A clean checkout at commit `64c93d8`; Rust toolchain from `rust-toolchain.toml`; fixtures are started in-process on `127.0.0.1:0` by the tests (no external services).

## Test Environment

macOS (aarch64-apple-darwin), Rust 1.90.0, cargo-deny 0.20.2, vhco CLI; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in Manual: follow each README; compare expected output and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
Clean checkout at the release commit
```

Tests executed:

- (manual / command procedure)

## Expected Results

Every step matches; verification records filled

## Actual Results

All 12 release sample folders (01-catalog … 12-library) were executed step by step from their READMEs against the release candidate (commit 829ca43, target/release/rivet, macOS 26.4.1 arm64) with local fixtures. Every step matches, and each folder is now status active, verified_against 0.1.0, runtime_verified true. The run found six defects; they were fixed in 2a751ab (INC-2026-0007), and the affected READMEs (02, 03, 04, 05, 10) were corrected against the fixed build. DEMO-2026-0015 (release verification guide) records U-01…U-26. Result PARTIAL, for two reasons: the Linux/Windows sandbox rows and the published release artifact (TASK-081) cannot be run here, and U-16 demonstrates only client_credentials (the other OAuth flows are covered by T-11).

## Result

PARTIAL

## Evidence

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

- Command above, run at commit `64c93d8`.
- Test source: Manual: follow each README; compare expected output.

## Executed By

Claude (automated run).

## Executed At

2026-09-28T10:28:03Z

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-30
- [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `64c93d8`. |
