---
document_id: TEST-2026-0033
title: "T-33 — release (release)"
document_type: test
status: completed
created_date: 2026-09-28
last_updated: 2026-09-30
document_revision: 2
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [execution]
affected_versions:
  from: "0.1.0"
  to: null
validated_plan_requirements: [PLAN-2026-0001 R14, PLAN-2026-0002 R12]
environment: "macOS (aarch64-apple-darwin), Rust 1.90.0, cargo-deny 0.20.2, vhco CLI; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)"
executed_by: Claude (automated)
executed_at: 2026-09-30T05:40:00Z
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
> **Last Updated:** 2026-09-30
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

**v0.2.0 run (2026-09-30).** Release commit `21bb2e94dfa476856495997c4f510f5537c65225` has the annotated tag `v0.2.0`. The tag resolves to that commit, `git describe --tags --exact-match HEAD` prints `v0.2.0`, and the working tree was clean. `scripts/check_version.py --tag` reports 0.2.0 everywhere: the workspace `Cargo.toml` (both members inherit it), `ffi/Cargo.toml`'s rivet-runtime requirement, `rivet --version`, MCP `serverInfo.version`, `rivet.capabilities`, and `librivet` `rivet_version()`, with `rivet_abi_version()` equal to the capabilities (1). `main` and `v0.2.0` are pushed to `origin`. CI on `main` (run 36635293035) is green on the release commit. The tag run (36635295229) failed one macOS test through INC-2026-0013 (fixed in v0.2.1).


Release commit f69b5b911f174b0198adb89c8305c8cce268fc11 carries the annotated tag v0.1.0. The tag resolves to that commit; at that commit `git describe --tags --exact-match HEAD` printed v0.1.0 and the working tree was clean. Cargo.toml, `rivet --version`, MCP serverInfo.version and rivet.capabilities all report 0.1.0 (scripts/check_version.py --tag). Documentation versions were synchronized in the release commit; historical verification notes keep the 0.1.0-dev build they ran against. Pushing waits on a git remote (TASK-078).

## Result

PASS

## Evidence

```text
$ python3 scripts/check_version.py --tag        (at 21bb2e9, tag v0.2.0)
    Cargo.toml version                 0.2.0
ok  Cargo.toml version.workspace       yes
ok  ffi/Cargo.toml version.workspace   yes
ok  ffi/Cargo.toml rivet-runtime requirement 0.2.0
ok  rivet --version                    0.2.0
ok  MCP serverInfo.version             0.2.0
ok  rivet.capabilities version         0.2.0
ok  librivet rivet_version()           0.2.0
ok  rivet_abi_version() == capabilities 1 / 1
ok  git describe --exact-match HEAD    0.2.0
ok  clean working tree                 yes
version sync: OK
$ git ls-remote origin | grep -E "main|v0.2.0"
21bb2e94dfa476856495997c4f510f5537c65225	refs/heads/main
21bb2e94dfa476856495997c4f510f5537c65225	refs/tags/v0.2.0^{}
```


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
| 2 | 2026-09-30 | Claude | Re-recorded for v0.2.0 (tag v0.2.0 → 21bb2e9, version sync OK, pushed): PASS. |
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f69b5b9`. |
