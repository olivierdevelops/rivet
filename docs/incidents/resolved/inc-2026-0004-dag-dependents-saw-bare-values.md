---
document_id: INC-2026-0004
title: "DAG dependent nodes saw bare values instead of node envelopes"
document_type: incident
status: resolved
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
severity: "S3"
start_time: 2026-09-28T08:40:00Z
end_time: 2026-09-28T08:45:00Z
root_cause_status: identified
systems: [Rivet]
components: [execution, execution_driver]
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

# DAG dependent nodes saw bare values instead of node envelopes

> **Status:** Resolved
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0-dev (unreleased)
> **Owner:** Project maintainer
> **Affected Components:** execution, execution_driver

## Incident Summary

Inside a dag, a dependent node reading `first.result` failed with "`first` is integer; cannot read `.result`": completed dependencies were bound as their raw values, while the design (and the 05-dag demo) specify the `{status, result, error}` envelope.

## Severity

S3 — documented `first.result` access failed (caught before release).

## Status

Resolved in commit `be4193d` before any release.

## Discovery Context

Runtime test driving docs/demos/05-dag `report.total`.

## Start Time

2026-09-28T08:40:00Z

## End Time

2026-09-28T08:45:00Z

## Affected Systems

Rivet.

## Affected Versions

0.1.0-dev development builds only; no release contained the defect.

## Affected Components

- execution
- execution_driver

## Customer Impact

None: the defect never shipped.

## Detection

Automated test.

## Reproduction Steps

1. Run `report.total` from docs/demos/05-dag with a=2, b=5.
2. Before the fix: value.missing_key error on node `total`.

## Timeline

```text
2026-09-28T08:40:00Z  defect observed (automated test)
2026-09-28T08:45:00Z  fix merged in be4193d, regression test green
```

## Logs and Evidence

The failing and passing test runs are listed under Verifying Tests; the fix commit is `be4193d`.

## Source Files

- `src/infra/execution_driver.rs (dag node frames; now src/features/execution/run_dag.rs)`

## Root Cause

The scheduler defined succeeded dependencies with their result value rather than the envelope.

## Contributing Factors

Parallel implementation streams and a grammar driven by priorities increase the chance of interactions that only a corpus-wide test reveals.

## Resolution

Dependencies are bound as envelopes; the scheduler later moved into the `execution.run_dag` use case (e1456cf) with unit tests of the envelope.

## Verifying Tests

- `orchestrator::runtime::tests::dag_fail_independent_reports_blocked_descendants`
- `features::execution::run_dag::tests::*`

## Corrective Actions

Envelope construction lives in one place (`NodeStatus::envelope`).

## Preventive Actions

Demo bundles are executed by tests, not only compiled.

## Owners

Implementer (fix); project maintainer (review).

## Remaining Risks

None known.

## Lessons Learned

Run the documented examples, not only compile them.

## Related Documents

- [PLAN-2026-0001](../../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md)
- [PROP-2026-0001](../../proposals/approved/prop-2026-0001-rivet-runtime.md)
- [RES-2026-0001](../../research/res-2026-0001-capy-grammar-spike.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Recorded and resolved. |
