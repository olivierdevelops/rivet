---
document_id: REF-2026-0011
title: "Rivet plans"
document_type: reference
status: draft
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 2
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [documentation]
affected_versions:
  from: not-applicable
  to: "0.1.0"
applicable_environments: [development]
audience: [maintainers, implementers, reviewers]
scope: Navigation and status for implementation, validation and release plans.
reason: AGENTS.md requires an index.md in every documentation directory; plans/ was created for PLAN-2026-0001.
dependencies: [DOCUMENTATION.md]
related_documents: [PLAN-2026-0001, PROP-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, plans, index]
confidentiality: internal
review_cycle: on-change
next_review_date: 2026-10-28
---

# Rivet plans

> **Status:** Draft
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** not-applicable → 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** documentation

## Purpose

Plans turn an approved proposal into executable, traceable work. Each plan is a **live ledger**: its task, file,
test, documentation and release tables are updated at every status change (DOCUMENTATION §4.5, template §12.4).
Every release originates from a plan with `status: approved` (§24).

**Belongs here:** implementation, testing, rollout, release, migration and deprecation plans.
**Does not belong here:** design proposals (`../proposals/`), decisions (`../decisions/`), completed results
(`../reports/`), test records (`../testing/`).

**Naming:** `plan-<YYYY>-<NNNN>-<short-description>.md`, ID `PLAN-<YYYY>-<NNNN>`.

## Active documents

| Plan | Status | Baseline | Target | Summary |
|---|---|---|---|---|
| [PLAN-2026-0001](plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) | approved | PROP-2026-0001 rev 8 | v0.1.0 | P1 approval/discovery → P2 implementation (Stage A + B) → P3 tests/validation → P4 docs/demos → P5 release. 96 tasks; P1 in progress |

```text
 PROP-2026-0001 ──approve──► PLAN-2026-0001 ──► v0.1.0 (Stage A + B)
                                   │
                                   └── Stage C (pipes, watch, mTLS TCP, codecs, reconnect) ──► PLAN-2026-0002 (not written)
```

## Recently added or updated

- 2026-09-28: PLAN-2026-0001 created (draft).

## Deprecated, superseded or archived

None.

## Relationships and open work

- G-DESIGN, G-CONTRACT, G-LIC and G-SPIKE are closed and the Git repository exists. PLAN-2026-0001 becomes
  `approved` once ADR-0002/0003 are accepted (TASK-012/013).
- Option-derived file sites / `io --needs` were approved (ADR-0001) and added in proposal revision 8.

## Recommended reading order

1. [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md): what is being built and why.
2. [PLAN-2026-0001](plan-2026-0001-rivet-v0-1-0-implementation-and-release.md): how it is built, validated, documented and released.

## Related directories

[proposals](../proposals/index.md) · [references](../references/index.md) · [demos](../demos/index.md) ·
[standards](../standards/index.md) · [current state](../README.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 2 | 2026-09-28 | Claude | P1 progress: gates closed, TASK-005 approved. |
| 1 | 2026-09-28 | Claude | Created plans index with PLAN-2026-0001. |
