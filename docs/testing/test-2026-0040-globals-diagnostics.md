---
document_id: TEST-2026-0040
title: "T-07 — Every global diagnostic with its span (UC-04 / R8)"
document_type: test
status: completed
created_date: 2026-09-29
last_updated: 2026-09-29
document_revision: 1
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [language]
affected_versions:
  from: "0.2.0"
  to: null
validated_plan_requirements: [PLAN-2026-0002 R8]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and recorded result for PLAN-2026-0002 test T-07 (PROP-2026-0002 T-07).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates (PLAN-2026-0002 TASK-061).
related_documents: [PLAN-2026-0002, PROP-2026-0002, ADR-0004]
supersedes: null
superseded_by: null
tags: [rivet, test, v0.2.0]
---

# T-07 — Every global diagnostic with its span (UC-04 / R8)

> **Status:** Completed
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** language

## Purpose

Show that every global diagnostic is reported with its exact code, line, column and end column, and that `rivet check` exits 2. It also covers the rule that a global can never hold a secret (`env`, `secret`, effects, requests are `check.global_not_constant`).

```text
  global token = (env "API_TOKEN")      ─► check.global_not_constant  app.rivet:1:16  exit 2
  global a = b * 2 ⏎ global b = 1       ─► check.global_forward_ref   1:12
  global x = 1 ⏎ global x = 2           ─► check.global_duplicate      (second name)
  operation … (x) / for x in … / as x   ─► check.global_shadow
  x = 3 / x += 1                        ─► check.global_assign
  global 1x = …                         ─► syntax.global
```

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0002 R8 | `syntax.global` and the `check.global_*` codes (`not_constant`, `forward_ref`, `duplicate`, `shadow`, `assign`) with spans |

Plan row: [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md#test-and-validation-checklist) T-07 (failure, UC-04).

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first so `librivet` exists for the FFI suites (TRBL-2026-0006); Python 3 and a C compiler on `PATH`. Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

## Test Data

The fixtures, bundles and goldens defined in `tests/conformance_globals.rs` and the shared helpers under `tests/` (in-process servers, temporary directories, `tests/fixtures/`).

## Procedure

```sh
cargo test --all-features --test conformance_globals
```

Tests executed (7):

- `conformance_globals::t07_assign` — check.global_assign: `NAME = …` and `NAME += …` on a global, at the statement
- `conformance_globals::t07_cli_check_exits_2` — `rivet check` renders check.global_not_constant with the UC-04 span and hint and exits 2
- `conformance_globals::t07_duplicate` — check.global_duplicate at the second declaration's name
- `conformance_globals::t07_forward_ref` — check.global_forward_ref: a later global or the global itself, at the reference
- `conformance_globals::t07_not_constant` — check.global_not_constant: env, request, an effect, and a param/local name, each at its span with a hint
- `conformance_globals::t07_shadow` — check.global_shadow: a param, loop variable, `with … as`, map item, dag node and secret reusing a global name
- `conformance_globals::t07_syntax_global` — syntax.global: a line without `=`, a bad name, or `global` inside an operation, each at its span (exit 2)

## Expected Results

Code, line, column; exit 2 (plan T-07).

## Actual Results

The seven T-07 tests passed: `syntax.global`, `check.global_not_constant`, `check.global_forward_ref`, `check.global_duplicate`, `check.global_shadow` and `check.global_assign`, each with the exact span, and the CLI render exits 2. `t07_not_constant` asserts that `global token = (env "API_TOKEN")` is refused with a hint towards `secret token from env`. A manual check on the debug build (RPT-2026-0015, TASK-064) confirmed the `secret` statement form and a `(secret …)` call are refused too, and a `secret` reusing a global's name is `check.global_shadow`. Linux: green in CI run 36505156729.

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

## Evidence

```text
conformance_globals                  test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s

GitHub Actions run 36505156729 (commit 14750b8, conclusion: success)
  test (ubuntu-latest)  success    test (macos-latest)  success
  features (none|serve|grpc|quic|oauth|cli)  success ×6    deny  success
```

## Evidence Sources

- The commands above, run at commit `14750b8`; full-suite output saved to `/tmp/p3-tests.log` (493 passed, 0 failed) and read with `grep -E "test result|FAILED|panicked"`.
- Test source: `tests/conformance_globals.rs`.
- CI: [run 36505156729](https://github.com/olivierdevelops/rivet/actions/runs/36505156729) at the same commit (`test (ubuntu-latest)`, `test (macos-latest)`, the six `features` jobs and `deny`, all `success`).
- Release verification cross-reference: [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md).

## Executed By

Claude (automated run, PLAN-2026-0002 P3).

## Executed At

2026-09-29T14:23:39Z

## Defects Raised

None.

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-07
- [PROP-2026-0002](../proposals/approved/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md)
- [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md) · [Tests index](index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-29 | Claude | Created and executed at commit `14750b8` (PLAN-2026-0002 P3, TASK-061); result PASS. |
