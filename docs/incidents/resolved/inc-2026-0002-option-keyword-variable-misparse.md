---
document_id: INC-2026-0002
title: "Variables named like option keywords were parsed as option lines"
document_type: incident
status: resolved
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
severity: "S3"
start_time: 2026-09-28T08:05:00Z
end_time: 2026-09-28T08:20:00Z
root_cause_status: identified
systems: [Rivet]
components: [language, capy_parser]
affected_versions:
  from: "0.1.0-dev"
  to: "0.1.0-dev"
confidentiality: internal
scope: Defect found and fixed during PLAN-2026-0001 implementation; never released.
reason: DOCUMENTATION §25 — unexpected defects found during implementation are recorded as incidents.
related_documents: [PLAN-2026-0001, PROP-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, incident, implementation]
---

# Variables named like option keywords were parsed as option lines

> **Status:** Resolved
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0-dev (unreleased)
> **Owner:** Project maintainer
> **Affected Components:** language, capy_parser

## Incident Summary

`message = socket.receive_from json timeout "5s"` and `stream.send json {…}` parsed as the `message` / `stream` option lines with a tail of `= …` / `.send …`, instead of an assignment and a method call. The Capy grammar spike reported the corpus as clean because the tree was structurally valid.

## Severity

S3 — silent misparse of valid programs (caught before release).

## Status

Resolved in commit `be4193d` before any release.

## Discovery Context

The T-29 sample corpus test (every docs/demos bundle must compile) failed on docs/demos/08-udp and 09-quic.

## Start Time

2026-09-28T08:05:00Z

## End Time

2026-09-28T08:20:00Z

## Affected Systems

Rivet.

## Affected Versions

0.1.0-dev development builds only; no release contained the defect.

## Affected Components

- language
- capy_parser

## Customer Impact

None: the defect never shipped.

## Detection

Automated test (tests/conformance_samples.rs).

## Reproduction Steps

1. `operation a.b` / `output json` / `message = socket.receive_from json` / `return message` / `end`
2. Compile.
3. Before the fix: lowering error "unexpected token in expression"; the statement was an `opt_message` node.

## Timeline

```text
2026-09-28T08:05:00Z  defect observed (automated test (tests/conformance_samples.rs))
2026-09-28T08:20:00Z  fix merged in be4193d, regression test green
```

## Logs and Evidence

The failing and passing test runs are listed under Verifying Tests; the fix commit is `be4193d`.

## Source Files

- `src/infra/rivet.capy (assign, assign_block, append_assign, member_call)`

## Root Cause

Capy picks the first complete match in priority order; the option functions and the assignment functions had equal priority, and an option line with a `tail` capture matches any remainder.

## Contributing Factors

Parallel implementation streams and a grammar driven by priorities increase the chance of interactions that only a corpus-wide test reveals.

## Resolution

Assignments, `+=` and method calls carry `priority 10`, above every option line (option lines never have `=`, `+=` or `.` as their second token).

## Verifying Tests

- `tests/conformance_samples.rs::every_demo_bundle_compiles`
- `tests/conformance_samples.rs::every_reference_fragment_lowers`

## Corrective Actions

RES-2026-0001 finding 9 records the rule.

## Preventive Actions

The corpus test now lowers (not only parses) every reference fragment inside a wrapper operation and fails on structural errors.

## Owners

Implementer (fix); project maintainer (review).

## Remaining Risks

New option keywords must be checked against common variable names; the lowering test catches regressions.

## Lessons Learned

A clean parse is not a correct parse; test the lowered meaning, not just the absence of diagnostics.

## Related Documents

- [PLAN-2026-0001](../../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md)
- [PROP-2026-0001](../../proposals/implemented/prop-2026-0001-rivet-runtime.md)
- [RES-2026-0001](../../research/res-2026-0001-capy-grammar-spike.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Recorded and resolved. |
