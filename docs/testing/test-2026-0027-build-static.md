---
document_id: TEST-2026-0027
title: "T-27 — build / static (all / R1)"
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
validated_plan_requirements: [PLAN-2026-0001 R1]
environment: "macOS (aarch64-apple-darwin), Rust 1.90.0, cargo-deny 0.20.2, vhco CLI; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)"
executed_by: Claude (automated)
executed_at: 2026-09-28T10:45:52Z
result: PARTIAL
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-27.
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-27 — build / static (all / R1)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

fmt, clippy `-D warnings`, `cargo deny`, release build on three OSes

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R1 | 010, 014, 016 |

## Preconditions

A clean checkout at commit `f15a82b`; Rust toolchain from `rust-toolchain.toml`; fixtures are started in-process on `127.0.0.1:0` by the tests (no external services).

## Test Environment

macOS (aarch64-apple-darwin), Rust 1.90.0, cargo-deny 0.20.2, vhco CLI; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in CI or local and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo deny check && cargo build --release
```

Tests executed:

- (manual / command procedure)

## Expected Results

All green; Capy licence passes deny after G-LIC

## Actual Results

fmt, clippy -D warnings, cargo deny (advisories, bans, licenses, sources) and the release build all succeed on macOS. The Capy licence passes cargo-deny. The plan also requires Linux and Windows release builds, which are pending CI (TASK-051), so the result is PARTIAL.

## Result

PARTIAL

## Evidence

```text
FMT_OK
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.40s
CLIPPY_RC=0
advisories ok, bans ok, licenses ok, sources ok
DENY_RC=0
    Finished `release` profile [optimized] target(s) in 29.55s
REL_RC=0
```

## Evidence Sources

- Command above, run at commit `f15a82b`.
- Test source: CI or local.

## Executed By

Claude (automated run).

## Executed At

2026-09-28T10:45:52Z

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-27
- [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f15a82b`. |
