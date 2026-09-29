---
document_id: TEST-2026-0029
title: "T-29 — regression / corpus (UC-01 / R2, R14)"
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
validated_plan_requirements: [PLAN-2026-0001 R2, PLAN-2026-0001 R14, PLAN-2026-0002 R7, PLAN-2026-0002 R16, PLAN-2026-0002 R19]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-29; re-recorded for v0.2.0 (PLAN-2026-0002 T-29).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001, PLAN-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-29 — regression / corpus (UC-01 / R2, R14)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0, 0.2.0 (re-recorded)
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

Every Stage A/B example and demo parses; demo ops run on fixtures

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R2 | 016, 017 |
| PLAN-2026-0001 R14 | 004, 006, 052–072, 083–092 |
| PLAN-2026-0002 R7, R16, R19 | T-29 corpus: REF-2026-0002 blocks and demo sources parse, including `global` and `import` blocks and the 14–17 demos |

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`, the v0.2.0 release candidate); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first (the FFI suites need `librivet`). Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

v0.1.0 run: commit `f15a82b`.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

v0.1.0 run: macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in `tests/conformance_samples.rs` and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
cargo test --workspace --all-targets --all-features --no-fail-fast   # full run → /tmp/p3-tests.log
cargo test --all-features --test conformance_samples
```

Tests executed (3):

- `conformance_samples::every_demo_bundle_compiles` — each demo is loaded like `rivet check` (source loader, demo folder as root, imports resolved), so 17-modules compiles with its modules (INC-2026-0012 item 1)
- `conformance_samples::every_reference_block_parses` — every rivet block in REF-2026-0002 parses with the embedded grammar
- `conformance_samples::every_reference_fragment_lowers` — every reference body fragment lowers without structural errors inside a wrapper operation; blocks that start with a declaration (including `global` and `import`) are compiled as whole files (INC-2026-0012 item 19)

v0.1.0 run: `cargo test conformance_samples`.

## Expected Results

100% of in-scope samples pass; Stage C samples refuse

## Actual Results

### v0.2.0 run (2026-09-29, commit `14750b8`)

3 passed, 0 failed on macOS at commit `14750b8`: every demo bundle under `docs/demos/` compiles (17-modules resolved through `resolve_imports` with the demo folder as root); every REF-2026-0002 block parses; every reference fragment lowers, with blocks that start with `global` or `import` treated as whole files. Both fixes come from [INC-2026-0012](../incidents/resolved/inc-2026-0012-documentation-and-demo-verification-defects.md) items 1 and 19 (`fad2940`). Green on ubuntu-latest and macos-latest in CI run 36505156729.

### v0.1.0 run (2026-09-28, commit `f15a82b`)

3 passed, 0 failed, 0 ignored.

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

v0.1.0 run: PASS.

## Evidence

v0.2.0 run:

```text
conformance_samples               test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.37s

GitHub Actions run 36505156729 (commit 14750b8, conclusion: success)
  test (ubuntu-latest)  success    test (macos-latest)  success
  features (none|serve|grpc|quic|oauth|cli)  success ×6    deny  success
```

v0.1.0 run:

```text
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.28s
```

## Evidence Sources

- The commands above, run at commit `14750b8`; `/tmp/p3-tests.log`.
- Test source: `tests/conformance_samples.rs`.
- CI: [run 36505156729](https://github.com/olivierdevelops/rivet/actions/runs/36505156729) at the same commit (`test (ubuntu-latest)`, `test (macos-latest)`, six `features` jobs, `deny`: all `success`).

v0.1.0 run:

- Command above, run at commit `f15a82b`.
- Test source: `tests/conformance_samples.rs`.

## Executed By

Claude (automated run).

## Executed At

2026-09-29T14:23:39Z (v0.2.0 run; v0.1.0 run: 2026-09-28T10:45:11Z)

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-29
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-29
- [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f15a82b`. |
| 2 | 2026-09-29 | Claude | Re-recorded for v0.2.0 at commit `14750b8` (PLAN-2026-0002 T-29, TASK-061): PASS. v0.1.0 run kept as history. |
