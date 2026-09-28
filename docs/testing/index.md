---
document_id: REF-2026-0033
title: "Rivet tests"
document_type: reference
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [language, execution, files, policy, audit, serve, transports]
affected_versions:
  from: "0.1.0"
  to: null
confidentiality: internal
scope: Navigation and status page for TEST documents.
reason: AGENTS.md requires an index.md in every documentation directory.
related_documents: [PLAN-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, index, test]
---

# Rivet tests

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** language, execution, files, policy, audit, serve, transports

## Purpose

This page lists the TEST documents (DOCUMENTATION §27). Each one maps to one row of the PLAN-2026-0001 Test and Validation Checklist and records the most recent run.

```text
 plan row T-nn ──► TEST-2026-00nn ──► cargo test --test conformance_<suite>
      │                  │
      │                  └── result: PASS | PARTIAL (macOS only; Linux/Windows pending CI)
      └── validated requirement R-n  ──► RPT-2026-0001 (validation report)
```

## Active documents

| Test | Title | Result |
|---|---|---|
| [TEST-2026-0001](test-2026-0001-language.md) | T-01 — unit + integration (UC-01 / R1, R2) | PARTIAL |
| [TEST-2026-0002](test-2026-0002-surfaces.md) | T-02 — integration / e2e (UC-02 / R8, R9) | PASS |
| [TEST-2026-0003](test-2026-0003-streams.md) | T-03 — fault / resource (UC-03 / R3, R4, R9) | PASS |
| [TEST-2026-0004](test-2026-0004-files.md) | T-04 — security / edge (UC-04 / R5, R11) | PASS |
| [TEST-2026-0005](test-2026-0005-resources.md) | T-05 — integration / fault (UC-05 / R3, R12, R13) | PASS |
| [TEST-2026-0006](test-2026-0006-mcp.md) | T-06 — integration (UC-06 / R7) | PASS |
| [TEST-2026-0007](test-2026-0007-dag.md) | T-07 — unit / integration (UC-07 / R10) | PASS |
| [TEST-2026-0008](test-2026-0008-sandbox.md) | T-08 — security / compatibility (UC-08 / R11, R13) | PARTIAL |
| [TEST-2026-0009](test-2026-0009-audit.md) | T-09 — security / audit (UC-09 / R6, R13) | PASS |
| [TEST-2026-0010](test-2026-0010-library.md) | T-10 — integration (UC-10 / R1, R3, R9) | PASS |
| [TEST-2026-0011](test-2026-0011-oauth.md) | T-11 — integration / security (UC-11 / R16) | PASS |
| [TEST-2026-0012](test-2026-0012-udp.md) | T-12 — integration (UC-12 / R15) | PASS |
| [TEST-2026-0013](test-2026-0013-quic.md) | T-13 — integration / security (UC-13 / R17) | PASS |
| [TEST-2026-0014](test-2026-0014-http3.md) | T-14 — integration (UC-14 / R18) | PASS |
| [TEST-2026-0015](test-2026-0015-auth-transport-policy.md) | T-15 — cross-surface / security (UC-11–14 / R15–R18) | PASS |
| [TEST-2026-0016](test-2026-0016-operation-catalog.md) | T-16 — unit / integration (UC-15 / R20) | PASS |
| [TEST-2026-0017](test-2026-0017-grpc.md) | T-17 — integration (UC-16 / R19) | PASS |
| [TEST-2026-0018](test-2026-0018-mcp-catalog.md) | T-18 — e2e (UC-17 / R21) | PASS |
| [TEST-2026-0019](test-2026-0019-sessions.md) | T-19 — e2e / fault (UC-18 / R22) | PASS |
| [TEST-2026-0020](test-2026-0020-outputs.md) | T-20 — unit / e2e (UC-19 / R23) | PASS |
| [TEST-2026-0021](test-2026-0021-policy-file.md) | T-21 — security (UC-08 / R24) | PASS |
| [TEST-2026-0022](test-2026-0022-serve.md) | T-22 — e2e / security (UC-20 / R25) | PASS |
| [TEST-2026-0023](test-2026-0023-syntax.md) | T-23 — unit (UC-01 / R1, R2) | PASS |
| [TEST-2026-0024](test-2026-0024-errors-limits-dag.md) | T-24 — unit / resource (UC-02, UC-07 / R4, R10) | PASS |
| [TEST-2026-0025](test-2026-0025-io-manifest.md) | T-25 — integration (UC-21, UC-09 / R6, R26) | PASS |
| [TEST-2026-0026](test-2026-0026-policy-generate.md) | T-26 — integration (UC-22 / R26) | PASS |
| [TEST-2026-0027](test-2026-0027-build-static.md) | T-27 — build / static (all / R1) | PARTIAL |
| [TEST-2026-0028](test-2026-0028-architecture.md) | T-28 — architecture (all / R14) | PASS |
| [TEST-2026-0029](test-2026-0029-samples.md) | T-29 — regression / corpus (UC-01 / R2, R14) | PASS |
| [TEST-2026-0030](test-2026-0030-demos-e2e.md) | T-30 — manual / e2e (all UCs / R1–R26) | PARTIAL |
| [TEST-2026-0031](test-2026-0031-documentation.md) | T-31 — documentation (R14) | PASS |
| [TEST-2026-0032](test-2026-0032-traceability.md) | T-32 — traceability (R1–R26) | PASS |

The table has 28 PASS and 4 PARTIAL results. Every PARTIAL result is PASS on macOS; the plan also requires Linux and Windows runs (and, for T-30, a run against the published artifact), which wait on CI and a git remote (TASK-051).

## Recently added or updated

- 2026-09-28: created during PLAN-2026-0001 P3.

## Deprecated, superseded or archived

None.

## Related directories

[plans](../plans/index.md) · [incidents](../incidents/index.md) · [current state](../README.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Created. |
