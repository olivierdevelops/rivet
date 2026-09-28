---
document_id: REF-2026-0030
title: "Rivet system runtime"
document_type: reference
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 2
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [execution, files, sessions, ws, poll]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, embedded, server]
audience: [maintainers, implementers, operators, reviewers]
scope: Navigation and status for docs/system/runtime/.
reason: AGENTS.md requires an index.md in every documentation directory; system/runtime/ was created for PLAN-2026-0001 phase P4.
dependencies: [DOCUMENTATION.md, AGENTS.md]
related_documents: [REF-2026-0023, SYS-2026-0002, SYS-2026-0007, PLAN-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, system, index]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-28
---

# Rivet system runtime

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** execution, files, sessions, ws, poll

## Purpose

How a request runs: dispatch, limits, interpretation, scopes and handles, DAGs, cancellation, deadlines, and long-lived duplex sessions. Documents here describe the current implemented code (DOCUMENTATION §4.7, template §12.6) and cite the
modules they describe.

**Belongs here:** `SYS-…` current-state documents on this subject. **Does not belong here:** design intent
(`../../proposals/`), decisions (`../../decisions/`), how-to material or release history.

**Naming:** `sys-<YYYY>-<NNNN>-<short-description>.md`.

```text
   request ──▶ SYS-0002 dispatcher ──▶ interpreter ──▶ result / error envelope
                    │                       │
                    │ emits/receives        └── with-handles closed in reverse order
                    ▼
               SYS-0007 session ──▶ events · input · finish · cancel
```

## Active documents

| ID | Document | Covers |
|---|---|---|
| SYS-2026-0002 | [Execution, scopes and DAG](sys-2026-0002-execution-scopes-and-dag.md) | dispatcher, limits, interpreter, resource handles and cleanup, run_dag, cancellation, deadlines |
| SYS-2026-0007 | [Duplex sessions](sys-2026-0007-sessions.md) | session lifecycle, input, events, retention and limits |

## Recently added or updated

- 2026-09-28: created for PLAN-2026-0001 phase P4, verified against `0.1.0-dev` commit `f40d4aa`.
- 2026-09-28: SYS-2026-0002 and SYS-2026-0007 revised (revision 2) for the post-P3 fix batch and re-verified at commit `829ca43`
  (TASK-092); limitations now point to the [manual's Known Limitations](../../manuals/man-2026-0001-rivet-manual.md#known-limitations).

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
| 2 | 2026-09-28 | Claude | Recorded the fix-batch revision (commit 829ca43) of this folder's documents. |
