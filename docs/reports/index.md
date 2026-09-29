---
document_id: REF-2026-0034
title: "Rivet reports"
document_type: reference
status: active
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 2
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [language, execution, files, policy, audit, serve, transports]
affected_versions:
  from: "0.1.0"
  to: null
confidentiality: internal
scope: Navigation and status page for reports.
reason: AGENTS.md requires an index.md in every documentation directory.
related_documents: [PLAN-2026-0001, PLAN-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, index, report]
---

# Rivet reports

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0, 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** language, execution, files, policy, audit, serve, transports

## Purpose

This page lists findings, assessments and validation reports (DOCUMENTATION §4.18, §28).

```text
 TEST-2026-0001…0033 (v0.1.0) ──► RPT-2026-0001 (R1–R26: 21 PASS, 5 PARTIAL) ──► REL-0.1.0
 TEST-2026-0001…0053 (v0.2.0) ──► RPT-2026-0015 (R1–R24: 22 PASS, 2 PARTIAL) ──► REL-0.2.0 (P5)
```

## Active documents

| Report | Status | Summary |
|---|---|---|
| [RPT-2026-0015](rpt-2026-0015-validation-of-plan-2026-0002.md) | completed | Validation of PLAN-2026-0002 (R1–R24) for v0.2.0 at `14750b8`: 22 PASS, 2 PARTIAL (R12 tag/G-PUB after P5; R18 P4 not exited), 0 FAIL; canary scan 0 hits |
| [RPT-2026-0001](rpt-2026-0001-validation-of-plan-2026-0001.md) | completed | Validation of PLAN-2026-0001 (R1–R26) for v0.1.0 |

## Recently added or updated

- 2026-09-29: RPT-2026-0015 written at PLAN-2026-0002 P3 (TASK-062).
- 2026-09-28: RPT-2026-0001 drafted at P3.

## Deprecated, superseded or archived

None.

## Related directories

[testing](../testing/index.md) · [plans](../plans/index.md) · [current state](../README.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 2 | 2026-09-29 | Claude | RPT-2026-0015 (validation of PLAN-2026-0002) added. |
| 1 | 2026-09-28 | Claude | Created. |
