---
document_id: REF-2026-0036
title: "Rivet releases"
document_type: reference
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
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

# Rivet releases

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** not-applicable → 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** documentation

## Purpose

Release documents (DOCUMENTATION §4.20, §33), filed flat as `rel-<version>-release-notes.md`. Each one is the traceability record of one release.

```text
 PLAN ─► tests ─► RPT (validation) ─► DEMO (verification guide) ─► release commit + tag ─► REL-<version>
```

## Active documents

| Release | Tag | Date | Plan | Status |
|---|---|---|---|---|
| [REL-0.1.0](rel-0.1.0-release-notes.md) | `v0.1.0` | 2026-09-28 | [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) | first release |

## Recently added or updated

- 2026-09-28: REL-0.1.0.

## Deprecated, superseded or archived

None.

## Related directories

[reports](../reports/index.md) · [testing](../testing/index.md) · [demos](../demos/index.md) · [current state](../README.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Created. |
