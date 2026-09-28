---
document_id: INC-2026-0003
title: "`x = map …` / `x = poll …` parsed as plain assignments; `until` conditions failed"
document_type: incident
status: resolved
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
severity: "S3"
start_time: 2026-09-28T12:10:00Z
end_time: 2026-09-28T12:30:00Z
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

# `x = map …` / `x = poll …` parsed as plain assignments; `until` conditions failed

> **Status:** Resolved
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0-dev (unreleased)
> **Owner:** Project maintainer
> **Affected Components:** language, capy_parser

## Incident Summary

After INC-2026-0002 raised plain assignments to priority 10, `status = poll every "1s" timeout "5s"` matched the generic assignment block (same priority, declared first) and its body lines were rejected as non-options. Separately, `until response.body.state == "done"` failed because lowering ran the argument-list parser over the condition before the expression parser.

## Severity

S3 — valid documented forms rejected (caught before release).

## Status

Resolved in commit `2d8ec7c` before any release.

## Discovery Context

A cancellation unit test used a `poll` loop and failed to compile; the new fragment-lowering corpus test then reported two more structural failures (multipart body parts, `until` with a comparison).

## Start Time

2026-09-28T12:10:00Z

## End Time

2026-09-28T12:30:00Z

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

Automated tests.

## Reproduction Steps

1. `s = poll every "1s" timeout "5s"` with an indented `until …` / `yield 1` body.
2. Compile.
3. Before the fix: syntax.option_expected on `until` and `yield`.

## Timeline

```text
2026-09-28T12:10:00Z  defect observed (automated tests)
2026-09-28T12:30:00Z  fix merged in 2d8ec7c, regression test green
```

## Logs and Evidence

The failing and passing test runs are listed under Verifying Tests; the fix commit is `2d8ec7c`.

## Source Files

- `src/infra/rivet.capy (assign_map, assign_poll, opt_body_block)`
- `src/features/language/lowering/lower.rs (opt_until, option_line)`

## Root Cause

Equal grammar priorities after the INC-2026-0002 change, and a stray `parse_args` call in the `until` lowering.

## Contributing Factors

Parallel implementation streams and a grammar driven by priorities increase the chance of interactions that only a corpus-wide test reveals.

## Resolution

`map`/`poll` assignments carry `priority 20`; `until` lowers through the expression parser only; `body multipart … end` gained a block form whose `field`/`file` part lines lower as child options.

## Verifying Tests

- `orchestrator::runtime::tests::cancel_running_request_by_id`
- `tests/conformance_samples.rs::every_reference_fragment_lowers`

## Corrective Actions

Grammar priority table documented in RES-2026-0001 findings 9.

## Preventive Actions

Every grammar priority change runs the fragment-lowering corpus (all 159 examples).

## Owners

Implementer (fix); project maintainer (review).

## Remaining Risks

None known.

## Lessons Learned

Priority fixes can shift other ties; the full corpus must be lowered after each grammar change.

## Related Documents

- [PLAN-2026-0001](../../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md)
- [PROP-2026-0001](../../proposals/implemented/prop-2026-0001-rivet-runtime.md)
- [RES-2026-0001](../../research/res-2026-0001-capy-grammar-spike.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Recorded and resolved. |
