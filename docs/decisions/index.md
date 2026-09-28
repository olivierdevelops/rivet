---
document_id: REF-2026-0013
title: "Rivet decisions"
document_type: reference
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
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
scope: Navigation and status for architecture decision records.
reason: AGENTS.md requires an index.md in every documentation directory; decisions/ was created for ADR-0001.
dependencies: [DOCUMENTATION.md]
related_documents: [ADR-0001, ADR-0002, ADR-0003, PROP-2026-0001, PLAN-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, decisions, index]
confidentiality: internal
review_cycle: on-change
next_review_date: 2026-10-28
---

# Rivet decisions

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** not-applicable → 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** documentation

## Purpose

Decision records (ADRs) capture approved technical and architectural decisions, with their context, rationale,
alternatives and consequences (DOCUMENTATION §4.4, template §12.3).

**Belongs here:** approved or proposed decisions that constrain implementation.
**Does not belong here:** open design questions (`../discussions/`), requests for approval (`../proposals/`),
evidence (`../research/`).

**Naming:** `adr-<NNNN>-<short-description>.md`, ID `ADR-<NNNN>` (a global sequence with no year).

## Active documents

| ADR | Status | Date | Decision | Drives |
|---|---|---|---|---|
| [ADR-0001](adr-0001-approve-rivet-runtime-design.md) | approved | 2026-09-28 | Approve PROP-2026-0001, `vhco-contract.json` and PLAN-2026-0001; close G-DESIGN, G-CONTRACT and G-LIC; approve TASK-005; adopt Git | [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md), [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) |
| [ADR-0002](adr-0002-rust-crate-selection.md) | approved | 2026-09-28 | Rust crate set for v0.1.0 (RES-2026-0002); ring-only TLS; broker-dialed hyper client; rmcp pinned to 2025-11-25; hand-rolled OAuth | TASK-014, [RES-2026-0002](../research/res-2026-0002-rust-crate-feasibility.md) |
| [ADR-0003](adr-0003-process-sandbox-backends.md) | approved | 2026-09-28 | Sandbox backends: macOS Seatbelt ships; Linux Landlock+seccomp gated on T-08; Windows/others refuse `unsupported.sandbox_backend`; children get no network | TASK-035, [RES-2026-0003](../research/res-2026-0003-process-sandbox-backends.md) |

## Pending decisions

None.

```text
 PROP-2026-0001 ──▶ ADR-0001 approved (2026-09-28) ──▶ PLAN-2026-0001 P1 ──▶ P2
                          │
                          ├── RES-2026-0002 ──▶ ADR-0002 crates   (approved) ──▶ TASK-014 Cargo.toml
                          └── RES-2026-0003 ──▶ ADR-0003 sandbox  (approved) ──▶ TASK-035 sandbox_*.rs
```

## Recently added or updated

- 2026-09-28: ADR-0001 recorded the maintainer's approval ("use git, i own capy so dont worry — everything else is
  approved").
- 2026-09-28: ADR-0002 (crates) and ADR-0003 (sandbox backends) approved from the P1 research.

## Deprecated, superseded or archived

None.

## Recommended reading order

1. [ADR-0001](adr-0001-approve-rivet-runtime-design.md): what was approved.
2. [ADR-0002](adr-0002-rust-crate-selection.md): which libraries implement it.
3. [ADR-0003](adr-0003-process-sandbox-backends.md): where child-process confinement is and is not available.

## Related directories

[proposals](../proposals/index.md) · [plans](../plans/index.md) · [research](../research/index.md) ·
[current state](../README.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 2 | 2026-09-28 | Claude | Added ADR-0002 and ADR-0003; no pending decisions. |
| 1 | 2026-09-28 | Claude | Created decisions index with ADR-0001 and the pending ADR-0002/0003. |
