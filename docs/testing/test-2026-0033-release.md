---
document_id: TEST-2026-0033
title: "T-33 — release (release)"
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
executed_at: 2026-09-28T10:54:15Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-33.
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-33 — release (release)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

Version sync (`Cargo.toml` = `rivet --version` = MCP serverInfo = docs); tag = commit; clean tree

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R14 | 004, 006, 052–072, 083–092 |

## Preconditions

A clean checkout at commit `f69b5b9`; Rust toolchain from `rust-toolchain.toml`; fixtures are started in-process on `127.0.0.1:0` by the tests (no external services).

## Test Environment

macOS (aarch64-apple-darwin), Rust 1.90.0, cargo-deny 0.20.2, vhco CLI; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in Manual per §32.4 and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
git describe --tags --exact-match HEAD
```

Tests executed:

- (manual / command procedure)

## Expected Results

`v0.1.0`; SHAs equal

## Actual Results

Release commit f69b5b911f174b0198adb89c8305c8cce268fc11 carries the annotated tag v0.1.0. The tag resolves to that commit; at that commit `git describe --tags --exact-match HEAD` printed v0.1.0 and the working tree was clean. Cargo.toml, `rivet --version`, MCP serverInfo.version and rivet.capabilities all report 0.1.0 (scripts/check_version.py --tag). Documentation versions were synchronized in the release commit; historical verification notes keep the 0.1.0-dev build they ran against. Pushing waits on a git remote (TASK-078).

## Result

PASS

## Evidence

```text
$ git rev-parse HEAD
f69b5b911f174b0198adb89c8305c8cce268fc11
$ git rev-list -n 1 v0.1.0
f69b5b911f174b0198adb89c8305c8cce268fc11
$ git describe --tags --exact-match f69b5b9
v0.1.0
$ git cat-file -t v0.1.0
tag
$ python3 scripts/check_version.py --tag   (at the release commit)
    Cargo.toml version                 0.1.0
ok  rivet --version                    0.1.0
ok  MCP serverInfo.version             0.1.0
ok  rivet.capabilities version         0.1.0
ok  git describe --exact-match HEAD    0.1.0
ok  clean working tree                 yes
version sync: OK
```

## Evidence Sources

- Command above, run at commit `f69b5b9`.
- Test source: Manual per §32.4.

## Executed By

Claude (automated run).

## Executed At

2026-09-28T10:54:15Z

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-33
- [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f69b5b9`. |
