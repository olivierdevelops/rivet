---
document_id: TEST-2026-0047
title: "T-14 — rivet highlight goldens and partial tokens (UC-08 / R17)"
document_type: test
status: completed
created_date: 2026-09-29
last_updated: 2026-09-29
document_revision: 1
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [language, cli, library]
affected_versions:
  from: "0.2.0"
  to: null
validated_plan_requirements: [PLAN-2026-0002 R17]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and recorded result for PLAN-2026-0002 test T-14 (PROP-2026-0002 T-14).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates (PLAN-2026-0002 TASK-061).
related_documents: [PLAN-2026-0002, PROP-2026-0002, ADR-0004]
supersedes: null
superseded_by: null
tags: [rivet, test, v0.2.0]
---

# T-14 — rivet highlight goldens and partial tokens (UC-08 / R17)

> **Status:** Completed
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** language, cli, library

## Purpose

Show that `rivet highlight` matches the ansi, html and json goldens, that every token class appears where expected and lies inside the source, and that a syntax error gives the tokens before it plus exit 2.

```text
  app.rivet ─► highlight_source (parser spans) ─► tokens ─► ansi │ html │ json
  broken.rivet ─► tokens up to the error + error envelope ─► exit 2
```

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0002 R17 | `rivet highlight` (ansi, html, json), `rivet::highlight::tokens`, `rivet_highlight` |

Plan row: [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md#test-and-validation-checklist) T-14 (integration, UC-08).

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first so `librivet` exists for the FFI suites (TRBL-2026-0006); Python 3 and a C compiler on `PATH`. Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

## Test Data

The fixtures, bundles and goldens defined in `tests/conformance_highlight.rs`, `tests/conformance_verification_defects.rs` and the shared helpers under `tests/` (in-process servers, temporary directories, `tests/fixtures/`).

## Procedure

```sh
cargo test --all-features --test conformance_highlight
cargo test --all-features --test conformance_verification_defects
```

Tests executed (6):

- `conformance_highlight::t14_cli_goldens_for_ansi_html_and_json` — T-14 goldens: `rivet highlight app.rivet --format ansi|html|json` match tests/fixtures/highlight/app.{ansi,html,jsonl}; the library renders the same bytes; json is the default when stdout is not a terminal
- `conformance_highlight::t14_every_reference_and_demo_token_lies_inside_the_source` — T-14 corpus: every REF-2026-0002 rivet block and every demo app.rivet tokenizes; parses cleanly; every token lies inside the source (line/col/len slice exactly its text), tokens are sorted and non-overlapping, and each statement's first word is a keyword, option or effect token
- `conformance_highlight::t14_every_token_class_appears_where_expected` — T-14 classes: the golden source yields every R17 class (keyword, option, type, effect, string, interpolation, number, comment, operation_id, global, variable, operator) at the expected positions, e.g. global api at 3:8 and the effect verb `get`
- `conformance_highlight::t14_syntax_error_gives_partial_tokens_and_exit_2` — T-14 partial tokens: a syntax error prints the tokens that start before it (none after), the syntax.expression diagnostic at 5:5 on stderr, exit 2; the library returns the same tokens with the error
- `conformance_highlight::t14_usage_errors` — T-14 usage: an unreadable FILE is validation.usage (exit 2) and an unknown --format is refused by the CLI
- `conformance_verification_defects::highlight_keeps_tokens_of_an_unclosed_block` — INC-2026-0012 item 9: an unclosed block keeps every token before the error (`operation`, `x.y` on line 1, `name "X"` on line 2)

## Expected Results

Goldens match (plan T-14).

## Actual Results

The five T-14 tests passed (format goldens, token classes, the span corpus over reference and demo sources, partial tokens with exit 2, usage errors), and `highlight_keeps_tokens_of_an_unclosed_block` passed. `rivet_highlight` from C and Python is exercised in `conformance_ffi` (T-11). Linux: green in CI run 36505156729.

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

## Evidence

```text
conformance_highlight                test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.45s
conformance_verification_defects     test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.64s

GitHub Actions run 36505156729 (commit 14750b8, conclusion: success)
  test (ubuntu-latest)  success    test (macos-latest)  success
  features (none|serve|grpc|quic|oauth|cli)  success ×6    deny  success
```

## Evidence Sources

- The commands above, run at commit `14750b8`; full-suite output saved to `/tmp/p3-tests.log` (493 passed, 0 failed) and read with `grep -E "test result|FAILED|panicked"`.
- Test source: `tests/conformance_highlight.rs`, `tests/conformance_verification_defects.rs`.
- CI: [run 36505156729](https://github.com/olivierdevelops/rivet/actions/runs/36505156729) at the same commit (`test (ubuntu-latest)`, `test (macos-latest)`, the six `features` jobs and `deny`, all `success`).
- Release verification cross-reference: [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md).

## Executed By

Claude (automated run, PLAN-2026-0002 P3).

## Executed At

2026-09-29T14:23:39Z

## Defects Raised

None in this run ([INC-2026-0012](../incidents/resolved/inc-2026-0012-documentation-and-demo-verification-defects.md) item 9 fixed before it).

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-14
- [PROP-2026-0002](../proposals/implemented/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md)
- [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md) · [Tests index](index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-29 | Claude | Created and executed at commit `14750b8` (PLAN-2026-0002 P3, TASK-061); result PASS. |
