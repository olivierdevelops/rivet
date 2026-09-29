---
document_id: TEST-2026-0046
title: "T-13 — TextMate grammar, keyword drift and .vsix packaging (UC-07 / R16)"
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
validated_plan_requirements: [PLAN-2026-0002 R16]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and recorded result for PLAN-2026-0002 test T-13 (PROP-2026-0002 T-13).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates (PLAN-2026-0002 TASK-061).
related_documents: [PLAN-2026-0002, PROP-2026-0002, ADR-0004]
supersedes: null
superseded_by: null
tags: [rivet, test, v0.2.0]
---

# T-13 — TextMate grammar, keyword drift and .vsix packaging (UC-07 / R16)

> **Status:** Completed
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** language

## Purpose

Show that the keyword table has no drift from `rivet.capy`, that the generated TextMate grammar scopes every reference and demo sample, and that the `.vsix` packages with the workspace version.

```text
  src/infra/rivet.capy ─► editors/gen_grammar.py ─► editors/keywords.json ─► rivet.tmLanguage.json ─► .vsix
          check_keywords.py (drift) ✔     check_grammar.py (107 samples) ✔     package_vsix.py ✔
```

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0002 R16 | Generated TextMate grammar; VS Code extension packaged as `.vsix` |

Plan row: [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md#test-and-validation-checklist) T-13 (regression, UC-07).

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first so `librivet` exists for the FFI suites (TRBL-2026-0006); Python 3 and a C compiler on `PATH`. Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

## Test Data

The fixtures, bundles and goldens defined in `tests/conformance_highlight.rs` and the shared helpers under `tests/` (in-process servers, temporary directories, `tests/fixtures/`).

## Procedure

```sh
cargo test --all-features --test conformance_highlight     # shells out to the Python checks below
python3 editors/check_keywords.py
python3 editors/tests/check_grammar.py
python3 editors/vscode/package_vsix.py
```

Tests executed (3):

- `conformance_highlight::t13_keyword_table_has_no_drift` — T-13 drift: every rivet.capy literal is classified in editors/keywords.json, the class table mirrors lowering's effect words, value types and built-ins, and keywords.json + rivet.tmLanguage.json are exactly what gen_grammar.py generates
- `conformance_highlight::t13_textmate_grammar_scopes_every_sample` — T-13 grammar: the generated TextMate regexes run (Python re) over every REF-2026-0002 rivet block and docs/demos/**/app.rivet without crashing, and every declaration, control, option and effect statement keyword is scoped
- `conformance_highlight::t13_vsix_packages_with_the_workspace_version` — T-13 packaging: package_vsix.py builds rivet-<Cargo version>.vsix with [Content_Types].xml, extension.vsixmanifest and extension/{package.json, README.md, language-configuration.json, syntaxes/rivet.tmLanguage.json}, deterministically, and nothing from .vscodeignore

## Expected Results

No drift; snapshots stable; `.vsix` built (plan T-13).

## Actual Results

The three T-13 tests passed: no keyword drift, every declaration, control, option and effect keyword in the REF-2026-0002 blocks and demo apps is scoped by the generated grammar, and the `.vsix` is built with the workspace version. As recorded in the plan's deviation, Node tooling (`vsce`, `vscode-tmgrammar-test`) is replaced by Python equivalents; the `.vsix` was installed in VS Code 1.108.1 during DEMO-2026-0018. Linux: green in CI run 36505156729.

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

## Evidence

```text
conformance_highlight                test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.45s

GitHub Actions run 36505156729 (commit 14750b8, conclusion: success)
  test (ubuntu-latest)  success    test (macos-latest)  success
  features (none|serve|grpc|quic|oauth|cli)  success ×6    deny  success
```

## Evidence Sources

- The commands above, run at commit `14750b8`; full-suite output saved to `/tmp/p3-tests.log` (493 passed, 0 failed) and read with `grep -E "test result|FAILED|panicked"`.
- Test source: `tests/conformance_highlight.rs`.
- CI: [run 36505156729](https://github.com/olivierdevelops/rivet/actions/runs/36505156729) at the same commit (`test (ubuntu-latest)`, `test (macos-latest)`, the six `features` jobs and `deny`, all `success`).
- Release verification cross-reference: [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md).

## Executed By

Claude (automated run, PLAN-2026-0002 P3).

## Executed At

2026-09-29T14:23:39Z

## Defects Raised

None.

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-13
- [PROP-2026-0002](../proposals/approved/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md)
- [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md) · [Tests index](index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-29 | Claude | Created and executed at commit `14750b8` (PLAN-2026-0002 P3, TASK-061); result PASS. |
