---
document_id: TEST-2026-0050
title: "T-17 — Every import error with its span and exit (UC-10 / R21)"
document_type: test
status: completed
created_date: 2026-09-29
last_updated: 2026-09-29
document_revision: 1
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [language, registry]
affected_versions:
  from: "0.2.0"
  to: null
validated_plan_requirements: [PLAN-2026-0002 R21]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and recorded result for PLAN-2026-0002 test T-17 (PROP-2026-0002 T-17).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates (PLAN-2026-0002 TASK-061).
related_documents: [PLAN-2026-0002, PROP-2026-0002, ADR-0004]
supersedes: null
superseded_by: null
tags: [rivet, test, v0.2.0]
---

# T-17 — Every import error with its span and exit (UC-10 / R21)

> **Status:** Completed
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** language, registry

## Purpose

Show that every import error code is reported at the import line with its exit code, that a cycle names its path, that symlinks and paths outside the root are refused, and that both import limits apply.

```text
  import "x" wrong shape / URL  ─► syntax.import                    exit 2
  missing file / symlink         ─► not_found.import                 exit 4
  import "../../etc/x"           ─► permission.import_outside_root   exit 3
  a → b → a                      ─► check.import_cycle (a → b → a)   exit 2
  same alias twice / collision   ─► check.import_duplicate / _collision  exit 2
  257 files or depth 17          ─► limit.imports                    exit 5
```

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0002 R21 | `syntax.import`, `not_found.import`, `permission.import_outside_root`, `check.import_cycle`, `check.import_duplicate`, `check.import_collision`, `limit.imports` |

Plan row: [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md#test-and-validation-checklist) T-17 (failure, UC-10).

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first so `librivet` exists for the FFI suites (TRBL-2026-0006); Python 3 and a C compiler on `PATH`. Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

## Test Data

The fixtures, bundles and goldens defined in `tests/conformance_modules.rs`, `tests/conformance_verification_defects.rs` and the shared helpers under `tests/` (in-process servers, temporary directories, `tests/fixtures/`).

## Procedure

```sh
cargo test --all-features --test conformance_modules
cargo test --all-features --test conformance_verification_defects
```

Tests executed (9):

- `conformance_modules::t17_collision` — check.import_collision when a namespaced ID equals a local operation ID, at the import
- `conformance_modules::t17_cycle` — check.import_cycle names the cycle path at the import that starts it
- `conformance_modules::t17_duplicate_alias` — check.import_duplicate when one file reuses an alias
- `conformance_modules::t17_limits` — limit.imports for imports deeper than 16 levels and for more than 256 files (exit 5)
- `conformance_modules::t17_not_found` — not_found.import at the import (exit 4); `rivet check` renders it and exits 4
- `conformance_modules::t17_outside_root` — permission.import_outside_root for a `..` escape (in a nested module too) and for a symlink below the root (exit 3)
- `conformance_modules::t17_syntax_import` — syntax.import: malformed, after a declaration, absolute path and reserved alias, each at the import line (exit 2)
- `conformance_verification_defects::url_imports_are_refused_at_check` — INC-2026-0012 item 18: `import "https://x.com/a.rivet" as x` (any scheme://) is syntax.import at check, never read as a path
- `conformance_verification_defects::load_refusal_has_ids_and_registry_kind` — INC-2026-0012 item 13: a refused Runtime::load (check.import_duplicate) keeps the registry kind (syntax, as `rivet check` reports it) and carries minted request/trace IDs

## Expected Results

Exact codes and exits (plan T-17).

## Actual Results

The seven T-17 tests passed (syntax, not found incl. symlink, outside root, cycle with its path, duplicate alias, collision, both limits). `url_imports_are_refused_at_check` (any `scheme://` import is `syntax.import`) and `load_refusal_has_ids_and_registry_kind` passed. Linux: green in CI run 36505156729.

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

## Evidence

```text
conformance_modules                  test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.45s
conformance_verification_defects     test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.64s

GitHub Actions run 36505156729 (commit 14750b8, conclusion: success)
  test (ubuntu-latest)  success    test (macos-latest)  success
  features (none|serve|grpc|quic|oauth|cli)  success ×6    deny  success
```

## Evidence Sources

- The commands above, run at commit `14750b8`; full-suite output saved to `/tmp/p3-tests.log` (493 passed, 0 failed) and read with `grep -E "test result|FAILED|panicked"`.
- Test source: `tests/conformance_modules.rs`, `tests/conformance_verification_defects.rs`.
- CI: [run 36505156729](https://github.com/olivierdevelops/rivet/actions/runs/36505156729) at the same commit (`test (ubuntu-latest)`, `test (macos-latest)`, the six `features` jobs and `deny`, all `success`).
- Release verification cross-reference: [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md).

## Executed By

Claude (automated run, PLAN-2026-0002 P3).

## Executed At

2026-09-29T14:23:39Z

## Defects Raised

None in this run ([INC-2026-0012](../incidents/resolved/inc-2026-0012-documentation-and-demo-verification-defects.md) items 13 and 18 fixed before it).

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-17
- [PROP-2026-0002](../proposals/implemented/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md)
- [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md) · [Tests index](index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-29 | Claude | Created and executed at commit `14750b8` (PLAN-2026-0002 P3, TASK-061); result PASS. |
