---
document_id: TEST-2026-0027
title: "T-27 — build / static (all / R1)"
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
validated_plan_requirements: [PLAN-2026-0001 R1, PLAN-2026-0002 R10, PLAN-2026-0002 R11, PLAN-2026-0002 R13]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-27; re-recorded for v0.2.0 (PLAN-2026-0002 T-27).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001, PLAN-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-27 — build / static (all / R1)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0, 0.2.0 (re-recorded)
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

fmt, clippy `-D warnings`, `cargo deny`, release build on three OSes

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R1 | 010, 014, 016 |
| PLAN-2026-0002 (all; R10, R11, R13 packaging) | T-27 build / static: fmt, clippy `--all-features`, deny, release build of both crates (`rivet-runtime`, `rivet-ffi`) |

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`, the v0.2.0 release candidate); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first (the FFI suites need `librivet`). Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

v0.1.0 run: commit `f15a82b`.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

v0.1.0 run: macOS (aarch64-apple-darwin), Rust 1.90.0, cargo-deny 0.20.2, vhco CLI; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in CI or local and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo build --workspace --all-features
cargo deny check
cargo build --release --workspace --all-features
```

Tests executed:

- (command procedure; each command must exit 0)

v0.1.0 run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo deny check && cargo build --release`.

## Expected Results

All green; Capy licence passes deny after G-LIC

## Actual Results

### v0.2.0 run (2026-09-29, commit `14750b8`)

fmt, clippy `--workspace --all-targets --all-features -D warnings`, the debug build, `cargo deny check` (advisories, bans, licenses, sources; cargo-deny 0.20.2) and the release build of both workspace crates (`rivet` binary, `librivet.dylib`, `librivet.a`) all exit 0 on macOS, with 0 warnings in the release build. Linux: CI run 36505156729 ran fmt, clippy, the release and debug builds on ubuntu-latest and macos-latest, and the `deny` job (licenses, bans, sources) — all green. **PARTIAL → PASS.** In 0.1.0 this test was PARTIAL only because Linux and Windows had no CI runner. Linux is now covered and green; Windows is no longer a supported platform (maintainer decision, INC-2026-0011), so it is not a reason for PARTIAL.

### v0.1.0 run (2026-09-28, commit `f15a82b`)

fmt, clippy -D warnings, cargo deny (advisories, bans, licenses, sources) and the release build all succeed on macOS. The Capy licence passes cargo-deny. The plan also requires Linux and Windows release builds, which are pending CI (TASK-051), so the result is PARTIAL.

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

v0.1.0 run: PARTIAL.

## Evidence

v0.2.0 run:

```text
== fmt
fmt EXIT=0
== clippy
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.49s
clippy EXIT=0   (exit codes confirmed by a re-run with `set -o pipefail`)
== build
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.40s
build EXIT=0
== deny
advisories ok, bans ok, licenses ok, sources ok
deny EXIT=0
== release
    Finished `release` profile [optimized] target(s) in 51.69s
cargo build --release --workspace --all-features  154.99s user 3.96s system 307% cpu 51.768 total
EXIT=0
target/release: rivet (18.5 MB), librivet.dylib (15.3 MB), librivet.a (62.0 MB)

GitHub Actions run 36505156729 (commit 14750b8, conclusion: success)
  test (ubuntu-latest)  success    test (macos-latest)  success
  features (none|serve|grpc|quic|oauth|cli)  success ×6    deny  success
```

v0.1.0 run:

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

- The commands above, run at commit `14750b8`; logs `/tmp/p3-gates/static.log`, `/tmp/p3-gates/release.log`.
- CI: [run 36505156729](https://github.com/olivierdevelops/rivet/actions/runs/36505156729) at the same commit (`test (ubuntu-latest)`, `test (macos-latest)`, six `features` jobs, `deny`: all `success`).

v0.1.0 run:

- Command above, run at commit `f15a82b`.
- Test source: CI or local.

## Executed By

Claude (automated run).

## Executed At

2026-09-29T14:23:39Z (v0.2.0 run; v0.1.0 run: 2026-09-28T10:45:52Z)

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-27
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-27
- [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f15a82b`. |
| 2 | 2026-09-29 | Claude | Re-recorded for v0.2.0 at commit `14750b8` (PLAN-2026-0002 T-27, TASK-061): PASS; PARTIAL → PASS (Linux green in CI; Windows unsupported). v0.1.0 run kept as history. |
