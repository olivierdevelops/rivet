---
document_id: INC-2026-0009
title: "Numeric index paths (`xs.0`) do not parse and report a misleading assign_map error"
document_type: incident
status: resolved
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 2
authors: [Claude]
owner: Project maintainer
severity: "S4"
start_time: 2026-09-28T21:40:00Z
end_time: 2026-09-28T23:30:00Z
root_cause_status: identified
systems: [Rivet]
components: [language]
affected_versions:
  from: "0.1.0"
  to: "0.2.0-dev"
confidentiality: internal
scope: A pre-existing parser defect found while writing the PLAN-2026-0002 P2b globals tests; fixed in P2e (commit 93388c1).
reason: DOCUMENTATION §25 — unexpected defects found during implementation are recorded as incidents.
related_documents: [PLAN-2026-0002, PROP-2026-0002, REF-2026-0002, MAN-2026-0003]
supersedes: null
superseded_by: null
tags: [rivet, incident, language, parser, capy]
---

# Numeric index paths (`xs.0`) do not parse and report a misleading assign_map error

> **Status:** Resolved (fixed in `93388c1`, PLAN-2026-0002 P2e)
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 → 0.2.0-dev
> **Owner:** Project maintainer
> **Affected Components:** language

## Incident Summary

A dotted path whose segment is a number (`xs.0`, `retry_on.0`) is rejected by the parser, although the
interpreter's path lookup supports list indexes (`value.missing_key` "index N is out of range" exists for it)
and REF-2026-0002 describes "out-of-range indexes" as typed errors. The diagnostic is misleading: it names the
internal grammar function `assign_map`, not the path.

```text
 source                          what happens
 ─────────────────────────────   ──────────────────────────────────────────────────────────────────────
 return {x: xs.a}                ok
 return {x: xs.0}                error[syntax.e0001]: expected `+=`, `.`, or `=`, found "{" in `assign_map`
 return xs.0                     error[syntax.e0001]: expected `+=`, `.`, or `=`, found "xs" in `assign_map`
 y = xs.0                        error[syntax.expression]: the value assigned to `y` does not parse
 interpreter lookup(["xs","0"])  would return the first item (never reached)
```

## Severity

S4: no security or data impact; a documented-looking form is unavailable and the message does not point at
the cause. Iterating with `for item in xs` works.

## Status

Resolved on `main` in commit `93388c1` during PLAN-2026-0002 P2e, before any v0.2.0 release. No Known Issue
entry is needed in the v0.2.0 release notes; the fix is a v0.2.0 change (numeric index paths now work).

```text
 source                          before (0.1.0 … 93388c1^)                    after (93388c1)
 ─────────────────────────────   ──────────────────────────────────────────   ───────────────────────────
 return xs.0                     syntax.e0001 … in `assign_map`               1
 return {x: xs.0}                syntax.e0001 … in `assign_map`               {"x": 1}
 y = xs.0                        syntax.expression (value of `y`)             y = 1
 "${xs.1}"                       syntax.interpolation (not a dotted path)     "2"
 return xs.5  (2 items)          (never reached)                              value.missing_key "index 5 is out of range for `xs`"
 return {x: }                    syntax.e0001 … in `assign_map`               syntax.expression "the expression after `return` does not parse"
```

## Discovery Context

Writing `tests/conformance_globals.rs` (T-06): a sample returning `retry_on.0` from a `global retry_on = [429,
503]` failed to compile. Reproduced on `main` before the P2b change (`git stash`), so it is pre-existing.

## Start Time

2026-09-28T21:40:00Z

## End Time

2026-09-28T23:30:00Z (fix committed as `93388c1`).

## Affected Systems

Rivet.

## Affected Versions

0.1.0 (released) and 0.2.0-dev.

## Affected Components

- language (Capy grammar `rivet.capy` value captures; `lowering/expr.rs` path parsing)

## Customer Impact

Authors cannot index a list by position in an expression; they get a message about `assign_map`.

## Detection

Failing new conformance test during development.

## Reproduction Steps

1. Write `operation a.b` / `output json` / `xs = [1, 2]` / `return xs.0` / `end` to `t.rivet`.
2. Run `rivet --file t.rivet check`.
3. Observe `error[syntax.e0001]: expected '+=', '.', or '=', found "xs" in 'assign_map'` (exit 2).
4. Expected: the check passes and `rivet request a.b` returns `1`.

## Timeline

```text
2026-09-28T21:40Z  found by the P2b globals tests; confirmed pre-existing on d837ad9
2026-09-28T22:30Z  recorded as INC-2026-0009 (active); test rewritten to avoid `.0`
```

## Logs and Evidence

The four outputs in the table above (`rivet check`, debug build of `673994c`).

## Source Files

- `src/infra/rivet.capy` (statement shapes; `any` captures)
- `src/features/language/lowering/expr.rs` (`path()` accepts identifier segments only)
- `src/infra/execution_driver.rs` (`lookup` already supports numeric segments)

## Root Cause

Identified. Three layers each rejected the form:

```text
 xs.0 ──▶ Capy lexer: `.0` is a number ──▶ the `any` value capture fails ──▶ Capy reports its LAST
          alternative (`assign_map`, priority 20) instead of the statement it matched (`return`)
      ──▶ expr.rs lexer: `0.1` after `.` would lex as a float; path() took identifier segments only
      ──▶ expr.rs lex_string: `${xs.0}` rejected segments that do not start with a letter
 interpreter lookup / const_eval::read_path: already indexed lists by numeric segment (never reached)
```

## Contributing Factors

No test or REF example indexes a list with a numeric segment.

## Resolution

Commit `93388c1` (no Capy change):

1. `src/infra/capy_parser.rs` — `mask_numeric_segments`: on the existing retry after a Capy diagnostic, the
   leading digit of each numeric path segment is replaced by `_` (same byte length, outside strings and
   comments; runs that start with a digit such as `1.5` are numbers and stay). Capy sees `xs._`; every span
   still slices the original `xs.0`, which Rivet's own expression parser lowers. Sources that already parsed
   are untouched (the mask applies only on the retry).
2. `src/features/language/lowering/expr.rs` — digits right after a `.` token are one integer index segment
   (never a float), `path()` accepts `Ident | Int` segments, and `${…}` accepts all-digit segments after the
   first.
3. `convert_diagnostic` — when a keyword statement's value fails and Capy names an internal alternative
   (`assign`, `assign_map`, `assign_poll`, `assign_block`, `append_assign`, `member_call`, `call_stmt`), the
   diagnostic becomes `syntax.expression` "the expression after `KEYWORD` does not parse" at the statement.

```text
   text ──Capy──▶ diagnostic? ──yes──▶ mask_numeric_segments ▶ mask_infix_elements ▶ Capy (retry)
                                                   same byte length ⇒ spans slice the original text
```

## Verifying Tests

- `tests/conformance_language.rs::inc_2026_0009_numeric_index_paths_parse_and_evaluate` — `xs.0`, `m.rows.1.0`,
  a global's `retry_on.0`, and index paths in objects, lists, calls, conditions, assignments and `${…}`.
- `tests/conformance_language.rs::inc_2026_0009_out_of_range_index_is_a_typed_error` — `value.missing_key`
  at 4:12–16; `[1.5, 2]` floats still lex as numbers.
- `tests/conformance_language.rs::inc_2026_0009_bad_value_names_the_statement_not_assign_map` — `return {x: }`
  and `emit [1,` are `syntax.expression` naming the keyword.
- `src/infra/capy_parser.rs::numeric_segments_are_masked_with_the_same_length` (unit).

All pass; the full suite is 444 tests at `93388c1`.

## Corrective Actions

Fixed (see Resolution). The v0.2.0 release notes list numeric index paths as a language fix, not a Known Issue.

## Preventive Actions

Add a REF/MAN example that indexes a list so the corpus test (T-29) covers the form (PLAN-2026-0002 P4,
MAN-2026-0003 / REF-2026-0002 updates).

## Owners

Implementer (fix); project maintainer (decision: fix vs known issue).

## Remaining Risks

Low. Negative indexes (`xs.-1`) are not supported (not a path); `xs.0` inside an operation ID
(`operation a.0`) now reaches lowering and fails as `syntax.operation_id`, which is the correct code.

## Lessons Learned

Writing tests with realistic data (lists of status codes) exercises forms the corpus never used.

## Related Documents

- [PLAN-2026-0002](../../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — P2b findings
- [REF-2026-0002](../../references/ref-2026-0002-language-and-usage.md) — "out-of-range indexes are typed errors"
- [MAN-2026-0003](../../manuals/man-2026-0003-language-guide.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 2 | 2026-09-28 | Claude | Resolved in `93388c1` (P2e): root cause identified, fix, verifying tests; moved to `resolved/`. |
| 1 | 2026-09-28 | Claude | Recorded (active) during PLAN-2026-0002 P2b. |
