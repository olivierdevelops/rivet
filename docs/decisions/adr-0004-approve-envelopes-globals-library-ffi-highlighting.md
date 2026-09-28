---
document_id: ADR-0004
title: "Approve PROP-2026-0002 (envelopes, globals, Cargo library, C ABI, highlighting) for v0.2.0"
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
components: [language, registry, execution, serve, cli, http, mcp, ws, poll, library]
affected_versions:
  from: "0.2.0"
  to: null
scope: Approval of PROP-2026-0002 revision 1 and its Contract Delta, with the recommended answers to Q-01…Q-06, as the scope of PLAN-2026-0002 (v0.2.0).
reason: Record the maintainer's approval ("approved, now plan then get an agent to implement the test and release", 2026-09-28).
related_documents: [PROP-2026-0002, PLAN-2026-0002, ADR-0001, REL-0.1.0]
supersedes: null
superseded_by: null
tags: [rivet, approval, decision, v0.2.0]
confidentiality: internal
review_cycle: on-change
next_review_date: 2026-12-28
---

# Approve PROP-2026-0002 (envelopes, globals, Cargo library, C ABI, highlighting) for v0.2.0

> **Status:** Approved
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** language, registry, execution, serve, cli, http, mcp, ws, poll, library

## Status

Approved on 2026-09-28 by the project maintainer.

## Context

[PROP-2026-0002](../proposals/approved/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md) revision 1
proposes the following for v0.2.0:
- one output envelope and one input envelope on every surface;
- opt-in pretty JSON;
- immutable `global` constants;
- Rivet as a Cargo dependency (package `rivet-runtime`, lib `rivet`, Cargo features);
- a C ABI shared and static library;
- syntax highlighting.

The maintainer's words (2026-09-28): *"approved, now plan then get an agent to implement the test and release;
ensure plan contains all docs documentation to add"*.

## Decision

```text
 G-DESIGN    closed ── PROP-2026-0002 approved as written (R1–R18, UC-01–UC-09, C-01–C-13)
 G-CONTRACT  closed ── the proposal's Contract Delta is the approved contract change; vhco-contract.json is
                       edited to match it before code (AGENTS contract-first), as P1 of PLAN-2026-0002
 Q-01…Q-06   resolved with the proposal's recommendations:
               Q-01 request_id stays server-assigned        Q-04 default features: runtime on, cli off
               Q-02 tree-sitter deferred                    Q-05 G-PUB (capy-lang on crates.io) — owner's call, not in scope
               Q-03 JSON colourization deferred             Q-06 no `rivet globals` command in 0.2.0
 G-PUB       open (optional) ── crates.io publication waits for Capy on crates.io; git dependency is the path
 PLAN        PLAN-2026-0002 is the execution ledger for v0.2.0
```

## Rationale

The design reuses the 0.1.0 dispatcher, policy and adapters. The only breaking change (the envelope) is allowed
under 0.x versioning, and 0.1.0 has no external consumers because it has never been pushed to a remote.

## Alternatives Considered

- **A dual-format output switch.** Rejected in the proposal (two formats forever).
- **Wait for G-PUB before releasing.** Rejected: the git dependency satisfies "import it in another rust project
  as a dep".

## Consequences

### Positive Consequences

- Clients parse one shape everywhere. Rivet can be embedded from Rust and from any C-FFI language.

### Negative Consequences

- Every 0.1.0 client, demo and documented output changes. A migration guide is required (MIG-2026-0001).

### Risks

- Symbol export through a second workspace crate, the static-link system library lists, and VHCO acceptance of
  a workspace are experiments in PLAN-2026-0002 P1.

## Implementation Impact

- PROP-2026-0002 moves to `docs/proposals/approved/` with status `approved`.
- PLAN-2026-0002 is created and approved.
- `vhco-contract.json` gets the Contract Delta in P1.

## Superseded Decisions

None.

## Related Documents

- [PROP-2026-0002](../proposals/approved/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md)
- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md)
- [ADR-0001](adr-0001-approve-rivet-runtime-design.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Recorded the approval of PROP-2026-0002, its contract delta and the resolutions of Q-01…Q-06. |
