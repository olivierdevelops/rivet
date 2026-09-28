---
document_id: REF-2026-0012
title: "Approved proposals"
document_type: reference
status: approved
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

# Approved proposals

> **Status:** Approved
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** not-applicable → 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** documentation

## Purpose

This directory holds proposals the maintainer has approved but that are **not implemented yet**. An approved
proposal is the design source of truth for its plan. Its hand-authored `vhco-contract.json` is the source of
truth for code. When the implementing plan releases, the proposal moves to `../implemented/`.

**Belongs here:** proposals with `status: approved`, each with a decision record in `../../decisions/`.
**Does not belong here:** drafts under review (`../draft/`), rejected proposals, plans (`../../plans/`) or
decisions themselves (`../../decisions/`).

**Naming:** `prop-<YYYY>-<NNNN>-<short-description>.md`, ID `PROP-<YYYY>-<NNNN>`. The file keeps its name when it
moves between lifecycle directories. Only its directory and `status` change.

```text
  draft/ ──(ADR approval)──▶ approved/ ──(plan released)──▶ implemented/
                                  │
                                  └──(superseded / withdrawn)──▶ superseded/ · rejected/
```

## Active documents

| Proposal | Status | Design revision | Decision | Plan | Summary |
|---|---|---|---|---|---|
| [PROP-2026-0001](prop-2026-0001-rivet-runtime.md) | approved | 8 | [ADR-0001](../../decisions/adr-0001-approve-rivet-runtime-design.md) | [PLAN-2026-0001](../../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) (P1) | Rivet scoped connection runtime: R1–R26, UC-01–UC-22, C-01–C-24, Increments 1–18 |

```text
  PROP-2026-0001 rev 8 ──▶ vhco-contract.json (approved) ──▶ PLAN-2026-0001 (P1 in execution) ──▶ v0.1.0
        │
        └── gates: G-DESIGN ✔  G-CONTRACT ✔  G-LIC ✔  G-SPIKE ☐ (TASK-010)
```

## Recently added or updated

- 2026-09-28: PROP-2026-0001 moved here from `draft/` with status `approved` (revision 7, ADR-0001).
- 2026-09-28: revision 8 applied the approved TASK-005 extension (option-derived file sites; `origin`, `phase`,
  `requires_existing`, `secret`; `rivet io --needs` and `--check-files`) and the TASK-006 contract reconciliation
  (`cancel_authorization`, `inspect_effects`, the `transports` feature).

## Deprecated, superseded or archived

None.

## Relationships and open work

- [ADR-0001](../../decisions/adr-0001-approve-rivet-runtime-design.md) records the approval and the closure of G-LIC.
- The numbered examples live in [REF-2026-0002](../../references/ref-2026-0002-language-and-usage.md) (S01–S159).
  The sample bundles live in [docs/demos](../../demos/index.md).
- Open: G-SPIKE, the Capy parse spike over every numbered example and demo bundle (PLAN-2026-0001 TASK-010).
  A grammar change forced by the spike goes back through the contract and this proposal first.

## Recommended reading order

1. [PROP-2026-0001](prop-2026-0001-rivet-runtime.md) — start with the Summary, then Requirements and Increments.
2. [ADR-0001](../../decisions/adr-0001-approve-rivet-runtime-design.md) — what was approved and when.
3. [PLAN-2026-0001](../../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — how it is built.

## Related directories

[proposals](../index.md) · [draft proposals](../draft/index.md) · [decisions](../../decisions/index.md) ·
[plans](../../plans/index.md) · [references](../../references/index.md) · [current state](../../README.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Created the approved-proposals index with PROP-2026-0001 (revision 8, ADR-0001). |
