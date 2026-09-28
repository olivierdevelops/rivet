---
document_id: INC-2026-0009
title: "Numeric index paths (`xs.0`) do not parse and report a misleading assign_map error"
document_type: incident
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
severity: "S4"
start_time: 2026-09-28T21:40:00Z
end_time: null
root_cause_status: suspected
systems: [Rivet]
components: [language]
affected_versions:
  from: "0.1.0"
  to: "0.2.0-dev"
confidentiality: internal
scope: A pre-existing parser defect found while writing the PLAN-2026-0002 P2b globals tests; not fixed in P2b.
reason: DOCUMENTATION §25 — unexpected defects found during implementation are recorded as incidents.
related_documents: [PLAN-2026-0002, PROP-2026-0002, REF-2026-0002, MAN-2026-0003]
supersedes: null
superseded_by: null
tags: [rivet, incident, language, parser, capy]
---

# Numeric index paths (`xs.0`) do not parse and report a misleading assign_map error

> **Status:** Active (open, not fixed)
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

Active. Not fixed in PLAN-2026-0002 P2b/P2f (out of scope). It must be fixed or listed under Known Issues in
the v0.2.0 release notes (DOCUMENTATION §25).

## Discovery Context

Writing `tests/conformance_globals.rs` (T-06): a sample returning `retry_on.0` from a `global retry_on = [429,
503]` failed to compile. Reproduced on `main` before the P2b change (`git stash`), so it is pre-existing.

## Start Time

2026-09-28T21:40:00Z

## End Time

Open.

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

Suspected, not confirmed: Capy tokenizes `.0` as the start of a number, so the `return`/`assign` value
capture fails and Capy reports the last alternative it tried (`assign_map`); when Capy accepts the line, Rivet's
expression parser still accepts only identifier segments after `.`.

## Contributing Factors

No test or REF example indexes a list with a numeric segment.

## Resolution

None yet. Candidate fix: accept an integer segment after `.` in `expr.rs::path()` and confirm Capy's value
capture keeps `xs.0` as one token (a Capy change may be needed); map a Capy `assign_map` fallback message on a
value line to a clearer `syntax.expression` diagnostic.

## Verifying Tests

None yet (a regression test must accompany the fix).

## Corrective Actions

Track in the v0.2.0 release notes Known Issues if still open at P5.

## Preventive Actions

Add a REF/MAN example that indexes a list so the corpus test (T-29) covers the form.

## Owners

Implementer (fix); project maintainer (decision: fix vs known issue).

## Remaining Risks

Authors may assume indexing works because the error registry and REF mention out-of-range indexes.

## Lessons Learned

Writing tests with realistic data (lists of status codes) exercises forms the corpus never used.

## Related Documents

- [PLAN-2026-0002](../../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — P2b findings
- [REF-2026-0002](../../references/ref-2026-0002-language-and-usage.md) — "out-of-range indexes are typed errors"
- [MAN-2026-0003](../../manuals/man-2026-0003-language-guide.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Recorded (active) during PLAN-2026-0002 P2b. |
