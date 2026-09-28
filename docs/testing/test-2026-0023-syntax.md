---
document_id: TEST-2026-0023
title: "T-23 — unit (UC-01 / R1, R2)"
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
validated_plan_requirements: [PLAN-2026-0001 R1, PLAN-2026-0001 R2]
environment: "macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)"
executed_by: Claude (automated)
executed_at: 2026-09-28T06:37:25Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-23.
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-23 — unit (UC-01 / R1, R2)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

Prefix calls, quoted durations, `${a.b}` vs `${a + b}`, escapes, URL encoding, option after body, yield/return

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R1 | 010, 014, 016 |
| PLAN-2026-0001 R2 | 016, 017 |

## Preconditions

A clean checkout at commit `19bd7c3`; Rust toolchain from `rust-toolchain.toml`; fixtures are started in-process on `127.0.0.1:0` by the tests (no external services).

## Test Environment

macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in `tests/conformance_syntax.rs` and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
cargo test conformance_syntax
```

Tests executed:

- `allow_block_after_a_dynamic_call`
- `cli_renders_the_exact_location`
- `did_you_mean_for_keyword_typos`
- `fcall_style_is_rejected`
- `header_order_and_late_header`
- `if_else_pairs_into_otherwise`
- `if_else_runs_one_branch`
- `infix_inside_objects_lists_and_call_arguments_evaluates`
- `interpolation_takes_dotted_paths_only`
- `multibyte_characters_in_diagnostics_do_not_panic`
- `option_after_body`
- `orphan_and_duplicate_else_are_rejected`
- `prefix_calls_with_trailing_objects`
- `quoted_versus_unquoted_durations`
- `string_escapes`
- `two_space_indentation_is_rejected`
- `unknown_function_fails_compile_with_did_you_mean`
- `url_interpolation_is_component_aware`
- `yield_versus_return`

## Expected Results

Exact diagnostics

## Actual Results

19 passed, 0 failed, 0 ignored.

## Result

PASS

## Evidence

```text
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.29s
```

## Evidence Sources

- Command above, run at commit `19bd7c3`.
- Test source: `tests/conformance_syntax.rs`.

## Executed By

Claude (automated run).

## Executed At

2026-09-28T06:37:25Z

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-23
- [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `19bd7c3`. |
