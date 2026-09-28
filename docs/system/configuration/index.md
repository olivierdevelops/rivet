---
document_id: REF-2026-0032
title: "Rivet system configuration"
document_type: reference
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [policy, serve, connectors, auth]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, embedded, server]
audience: [maintainers, implementers, operators, reviewers]
scope: Navigation and status for docs/system/configuration/.
reason: AGENTS.md requires an index.md in every documentation directory; system/configuration/ was created for PLAN-2026-0001 phase P4.
dependencies: [DOCUMENTATION.md, AGENTS.md]
related_documents: [REF-2026-0023, SYS-2026-0008, PLAN-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, system, index]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-28
---

# Rivet system configuration

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** policy, serve, connectors, auth

## Purpose

Configuration files Rivet reads. In 0.1.0 the only configuration file is policy.json (schema v1). Documents here describe the current implemented code (DOCUMENTATION §4.7, template §12.6) and cite the
modules they describe.

**Belongs here:** `SYS-…` current-state documents on this subject. **Does not belong here:** design intent
(`../../proposals/`), decisions (`../../decisions/`), how-to material or release history.

**Naming:** `sys-<YYYY>-<NNNN>-<short-description>.md`.

```text
   app.rivet + policy.json (beside it, or --policy PATH) ──▶ load_policy ──▶ Policy
                         absent ──▶ deny-by-default           invalid ──▶ exit 2
```

## Active documents

| ID | Document | Covers |
|---|---|---|
| SYS-2026-0008 | [policy.json schema v1 reference](sys-2026-0008-policy-json-reference.md) | every key, default and error: version, grants, deny, network, limits, serve, approved |

## Recently added or updated

- 2026-09-28: created for PLAN-2026-0001 phase P4, verified against `0.1.0-dev` commit `f40d4aa`.

## Deprecated, superseded or archived

None.

## Relationships, open work and reading order

Read in the order of the table above. Open work and known 0.1.0 limitations are listed in each document's
*Known Limitations* section and summarised in the [system index](../index.md), which also shows how this folder
relates to the rest of the system.

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Created the index for PLAN-2026-0001 phase P4. |
