---
document_id: TEST-2026-0023
title: "T-23 — unit (UC-01 / R1, R2)"
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
validated_plan_requirements: [PLAN-2026-0001 R1, PLAN-2026-0001 R2, PLAN-2026-0002 R1]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-23; re-recorded for v0.2.0 (PLAN-2026-0002 T-34).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001, PLAN-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-23 — unit (UC-01 / R1, R2)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0, 0.2.0 (re-recorded)
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

Prefix calls, quoted durations, `${a.b}` vs `${a + b}`, escapes, URL encoding, option after body, yield/return

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R1 | 010, 014, 016 |
| PLAN-2026-0001 R2 | 016, 017 |
| PLAN-2026-0002 R1 | T-34 regression: every 0.1.0 suite stays green on the 0.2.0 envelopes and input keys with no behaviour change |

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`, the v0.2.0 release candidate); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first (the FFI suites need `librivet`). Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

v0.1.0 run: commit `f15a82b`.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

v0.1.0 run: macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in `tests/conformance_syntax.rs` and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
cargo test --workspace --all-targets --all-features --no-fail-fast   # full run → /tmp/p3-tests.log
cargo test --all-features --test conformance_syntax
```

Tests executed (19):

- `conformance_syntax::allow_block_after_a_dynamic_call` — a call followed by an `allow [...]` block keeps the block after the closing paren; the dynamic target must be listed
- `conformance_syntax::cli_renders_the_exact_location` — the CLI renders a syntax diagnostic as FILE:LINE:COL with a caret and exits 2; --json prints the ErrorEnvelope with the same span
- `conformance_syntax::did_you_mean_for_keyword_typos` — an unknown statement keyword is syntax.unknown_statement at the keyword with a did-you-mean hint
- `conformance_syntax::fcall_style_is_rejected` — `request("id", {…})` is syntax.fcall_style at the argument list with a prefix-call hint
- `conformance_syntax::header_order_and_late_header` — header lines keep the order name, description, private, param, output, emits, receives, error; a header line after the body is syntax.option_after_body
- `conformance_syntax::if_else_pairs_into_otherwise` — G34: `if COND … else … end` pairs the else section with its `if` (nested pairs stay with their own `if`) and lowers into Stmt::If.otherwise
- `conformance_syntax::if_else_runs_one_branch` — G34: the interpreter runs exactly one branch of `if … else … end` (nested else included)
- `conformance_syntax::infix_inside_objects_lists_and_call_arguments_evaluates` — G30: infix inside objects, lists and call arguments parses AND evaluates (`{n: n - 1}`, `[a + 1]`, `(request "x" {v: a * 2})`)
- `conformance_syntax::interpolation_takes_dotted_paths_only` — `${a.b}` interpolates a dotted path; `${a + b}` is syntax.interpolation at the placeholder
- `conformance_syntax::multibyte_characters_in_diagnostics_do_not_panic` — regression: an escape or number followed by a multi-byte character reports a diagnostic instead of panicking on a char boundary
- `conformance_syntax::option_after_body` — the leading-options rule: an option line after a body statement is syntax.option_after_body at that option line
- `conformance_syntax::orphan_and_duplicate_else_are_rejected` — G34: an orphan `else`, a second `else` and `else if COND` are syntax errors at the `else` line
- `conformance_syntax::prefix_calls_with_trailing_objects` — a prefix call keeps named arguments as one trailing object and a literal `(request "ID" {…})` joins the call graph
- `conformance_syntax::quoted_versus_unquoted_durations` — durations are quoted: `timeout 10s` / `dag timeout 5s` are syntax.duration_unquoted at the bare token with a quoted hint; a malformed quoted duration is syntax.duration
- `conformance_syntax::string_escapes` — escapes: \x00, \t, \", \\ and \u00e9 decode; \0 is syntax.escape with a `write \x00` hint
- `conformance_syntax::two_space_indentation_is_rejected` — two-space indentation is syntax.indent on every offending line; continuation lines inside brackets are exempt
- `conformance_syntax::unknown_function_fails_compile_with_did_you_mean` — G3: `(len x)` fails at compile time with check.unknown_function at the name and a did-you-mean
- `conformance_syntax::url_interpolation_is_component_aware` — `${x}` in a path segment stays ONE encoded segment (`/ ? # @ ..` never add structure); in a query value it is one encoded query component; a bare `..` is refused
- `conformance_syntax::yield_versus_return` — `yield` is only valid inside map/poll (syntax.yield); a map body must yield (syntax.map_yield); `return` inside map exits the whole operation

v0.1.0 run: `cargo test conformance_syntax`.

## Expected Results

Exact diagnostics

## Actual Results

### v0.2.0 run (2026-09-29, commit `14750b8`)

19 passed, 0 failed, 0 ignored on macOS at commit `14750b8` (v0.2.0 release candidate). This is the PLAN-2026-0002 T-34 regression: the 0.1.0 suite passes on the 0.2.0 envelopes and input keys (TASK-018 changed only request and response shapes). Green on ubuntu-latest and macos-latest in CI run 36505156729, which runs the same `cargo test --workspace --all-targets --all-features --no-fail-fast`.

### v0.1.0 run (2026-09-28, commit `f15a82b`)

19 passed, 0 failed, 0 ignored.

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

v0.1.0 run: PASS.

## Evidence

v0.2.0 run:

```text
conformance_syntax                 test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s

GitHub Actions run 36505156729 (commit 14750b8, conclusion: success)
  test (ubuntu-latest)  success    test (macos-latest)  success
  features (none|serve|grpc|quic|oauth|cli)  success ×6    deny  success
```

v0.1.0 run:

```text
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.52s
```

## Evidence Sources

- The commands above, run at commit `14750b8`; full-suite output in `/tmp/p3-tests.log` (493 passed, 0 failed), read with `grep -E "test result|FAILED|panicked"`.
- Test source: `tests/conformance_syntax.rs`.
- CI: [run 36505156729](https://github.com/olivierdevelops/rivet/actions/runs/36505156729) at the same commit (`test (ubuntu-latest)`, `test (macos-latest)`, six `features` jobs, `deny`: all `success`).

v0.1.0 run:

- Command above, run at commit `f15a82b`.
- Test source: `tests/conformance_syntax.rs`.

## Executed By

Claude (automated run).

## Executed At

2026-09-29T14:23:39Z (v0.2.0 run; v0.1.0 run: 2026-09-28T10:45:02Z)

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-34
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-23
- [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f15a82b`. |
| 2 | 2026-09-29 | Claude | Re-recorded for v0.2.0 at commit `14750b8` (PLAN-2026-0002 T-34, TASK-061): PASS. v0.1.0 run kept as history. |
