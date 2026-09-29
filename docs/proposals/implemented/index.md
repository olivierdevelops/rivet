---
document_id: REF-2026-0035
title: "Implemented proposals"
document_type: reference
status: active
created_date: 2026-09-28
last_updated: 2026-09-30
document_revision: 2
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, execution, policy, audit, serve]
affected_versions:
  from: not-applicable
  to: "0.1.0"
applicable_environments: [development]
audience: [maintainers, implementers, reviewers]
scope: Navigation and status for proposals approved by the maintainer and not yet implemented.
reason: AGENTS.md requires an index.md in every documentation directory; approved/ was created when PROP-2026-0001 was approved (ADR-0001, PLAN-2026-0001 TASK-004).
dependencies: [DOCUMENTATION.md, AGENTS.md]
related_documents: [PROP-2026-0001, ADR-0001, PLAN-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, proposals, index]
confidentiality: internal
review_cycle: on-change
next_review_date: 2026-10-28
---

# Implemented proposals

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** not-applicable → 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** documentation

## Purpose

Proposals whose plan was released. The document keeps its file name; its `status` is `implemented`.

```text
 approved/ ──(plan released)──▶ implemented/
                                   └── PROP-2026-0001 ──▶ PLAN-2026-0001 ──▶ v0.1.0 (REL-0.1.0)
```

## Active documents

| Proposal | Status | Design revision | Decision | Plan | Release |
|---|---|---|---|---|---|
| [PROP-2026-0001](prop-2026-0001-rivet-runtime.md) | implemented | 9 | [ADR-0001](../../decisions/adr-0001-approve-rivet-runtime-design.md) | [PLAN-2026-0001](../../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) | [REL-0.1.0](../../releases/rel-0.1.0-release-notes.md) |
| [PROP-2026-0002](prop-2026-0002-envelopes-globals-library-ffi-highlighting.md) | implemented | 3 | [ADR-0004](../../decisions/adr-0004-approve-envelopes-globals-library-ffi-highlighting.md) | [PLAN-2026-0002](../../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) | [REL-0.2.0](../../releases/rel-0.2.0-release-notes.md) |

## Recently added or updated

- 2026-09-28: PROP-2026-0001 implemented in v0.1.0.

## Deprecated, superseded or archived

None.

## Related directories

[approved](../approved/index.md) · [proposals](../index.md) · [releases](../../releases/index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 2 | 2026-09-30 | Claude | PROP-2026-0002 implemented in v0.2.0. |
| 1 | 2026-09-28 | Claude | Created. |
