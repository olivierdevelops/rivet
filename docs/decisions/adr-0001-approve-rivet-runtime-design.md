---
document_id: ADR-0001
title: "Approve the Rivet runtime design and implementation plan"
document_type: decision
status: approved
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
approval:
  decision_date: 2026-09-28
  approvers: [Project maintainer]
systems: [Rivet]
components: [language, runtime, interfaces, sandbox, documentation]
affected_versions:
  from: not-applicable
  to: "0.1.0"
reason: Record the maintainer's approval of PROP-2026-0001, the hand-authored contract and PLAN-2026-0001, and the closure of the Capy licence gate.
related_documents: [PROP-2026-0001, PLAN-2026-0001, REF-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, approval, decision]
confidentiality: internal
review_cycle: on-change
next_review_date: 2026-12-28
---

# Approve the Rivet runtime design and implementation plan

> **Status:** Approved
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** not-applicable → 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** language, runtime, interfaces, sandbox, documentation

## Status

Approved on 2026-09-28 by the project maintainer.

## Context

The following were presented for approval together:
- [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md) revision 6, covering R1–R26,
  UC-01–UC-22 and C-01–C-24;
- the hand-authored `vhco-contract.json`;
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) revision 1.

Four gates were open: G-DESIGN, G-CONTRACT, G-LIC and G-SPIKE. One decision was pending (PLAN-2026-0001
TASK-005): option-derived file sites and `rivet io --needs`.

The maintainer's words (2026-09-28): *"use git, i own capy so dont worry — everything else is approved"*.

## Decision

```text
 G-DESIGN   closed ── proposal approved as written (revision 6 → 7 records approval)
 G-CONTRACT closed ── vhco-contract.json approved as the implementation contract
 G-LIC      closed ── maintainer owns Capy; Rivet may depend on and ship it
 TASK-005   approved ─ option-derived file sites (tls ca_file/cert_file/key_file…), phase,
                       requires_existing, secret, `io --needs`, `--check-files` join R26
 PLAN       approved ─ PLAN-2026-0001 is the execution ledger for v0.1.0
 Git        adopted ── repository initialised on branch main
 G-SPIKE    open ───── entry gate for code, owned by the implementer (TASK-010)
```

## Rationale

The maintainer reviewed the design and plan, and owns the only third-party dependency whose licence text
conflicted with redistribution. No outstanding design question remained besides TASK-005, which is included in
the approval.

## Alternatives Considered

- **Keep G-LIC open until the upstream `LICENSE` file changes.** Rejected: the licence holder has authorized
  the use directly. Updating the upstream text is the owner's own housekeeping.
- **Defer TASK-005.** Rejected: it is covered by "everything else is approved".

## Consequences

### Positive Consequences

- P1 can close as soon as the spike and the research ADRs are done. Implementation (P2) can then start.

### Negative Consequences

- The design is frozen as written. Later changes must go through the contract first, then the proposal (AGENTS
  loop).

### Risks

- G-SPIKE may still force grammar changes. Any such change goes back through the contract and the proposal
  before code is written.

## Implementation Impact

- PROP-2026-0001 moved to `docs/proposals/approved/` with status `approved`.
- PLAN-2026-0001 status becomes `approved` once P1 exits.
- TASK-005 design text is added to the proposal, reference, demos and contract.

## Superseded Decisions

None.

## Related Documents

- [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md)
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md)
- [REF-2026-0001](../references/ref-2026-0001-request-and-evidence.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Recorded the maintainer's approval of design, contract, plan and TASK-005, and the closure of G-LIC. |
