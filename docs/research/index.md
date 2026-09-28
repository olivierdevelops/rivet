---
document_id: REF-2026-0014
title: "Rivet research"
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
scope: Navigation and status for research, feasibility studies and prototype findings (RES documents).
reason: AGENTS.md requires an index.md in every documentation directory; PLAN-2026-0001 TASK-084 creates docs/research/index.md once the P1 research exists.
dependencies: [DOCUMENTATION.md, AGENTS.md]
related_documents: [RES-2026-0001, RES-2026-0002, RES-2026-0003, ADR-0002, ADR-0003, PLAN-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, research, index]
confidentiality: internal
review_cycle: on-change
next_review_date: 2026-10-28
---

# Rivet research

> **Status:** Approved
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** not-applicable → 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** documentation

## Purpose

Research records answer a feasibility question with evidence: prototypes, crate and platform surveys, and
spikes. Each one keeps verified findings apart from assumptions and recommendations (DOCUMENTATION §4.22).
A research record does not decide anything. The decision it supports is an ADR in [decisions](../decisions/index.md).

**Belongs here:** spikes, technology comparisons, proof-of-concept results, feasibility studies.
**Does not belong here:**
- decisions (`../decisions/`);
- designs (`../proposals/`);
- test results and validation reports (`../testing/`, `../reports/`);
- prototype source trees beyond small reproduction files. Scratch prototypes stay out of the repository.

**Naming:** `res-<YYYY>-<NNNN>-<short-description>.md`, ID `RES-<YYYY>-<NNNN>`, never reused.

## Active documents

| ID | Title | Status | Plan task | Headline result | Feeds |
|---|---|---|---|---|---|
| [RES-2026-0001](res-2026-0001-capy-grammar-spike.md) | Capy grammar spike (G-SPIKE) | completed | TASK-010 | Capy `84f984c6` parses 119/119 Rivet source blocks. **G-SPIKE PASS** | TASK-016 grammar and parser adapter |
| [RES-2026-0002](res-2026-0002-rust-crate-feasibility.md) | Rust crate feasibility | completed | TASK-009 | 20/20 live probes PASS. The dependency block builds on macOS, Linux and Windows. Graph is all permissive; MSRV ≤ 1.90 | [ADR-0002](../decisions/adr-0002-rust-crate-selection.md) → TASK-014 |
| [RES-2026-0003](res-2026-0003-process-sandbox-backends.md) | Process sandbox backends | completed | TASK-011 | macOS Seatbelt 27/27 PASS. Linux and Windows build-only. No OS can allow-list child network by host | [ADR-0003](../decisions/adr-0003-process-sandbox-backends.md) → TASK-035 |

```text
 PROP-2026-0001 feasibility table ("experiment needed" / "blocked")
        │
        ├── RES-2026-0001  Capy grammar spike ─────────► G-SPIKE PASS ─────────► TASK-016
        ├── RES-2026-0002  crate feasibility ──────────► ADR-0002 crates ──────► TASK-014 Cargo.toml
        └── RES-2026-0003  sandbox backends ───────────► ADR-0003 sandbox ─────► TASK-035 sandbox_*.rs
                                                                  │
                                           P1 exit (TASK-013) ◄───┘ ──► P2 implementation
```

## What each study proved, and what it did not

| ID | Runtime-verified | Build-verified only | Still open (owner task) |
|---|---|---|---|
| RES-2026-0001 | Every corpus block parses; invalid input rejected with spans | — | Lowering and semantic checks (TASK-016/017) |
| RES-2026-0002 | Checked-IP HTTP h1/h2, no redirects, one listener for REST/SSE/WS/MCP, MCP `2025-11-25` over HTTP and stdio, dynamic gRPC unary + ProtoJSON, QUIC + DATAGRAM, HTTP/3, cap-std no-follow, multicast, argv processes, platform TLS verify (all on macOS) | The full block on Linux and Windows | gRPC streaming modes, TLS/mTLS serve listener, OAuth provider flows, real keychain writes, cross-process credential locking |
| RES-2026-0003 | Seatbelt: file/exec/network/UNIX/signal confinement, escapes, nesting, descendants, no-fork profile | Landlock V6 + seccomp; AppContainer + Job Object | Linux T-08 in CI on kernel ≥ 6.12; Windows backend (future ADR) |

## Recently added or updated

- 2026-09-28: RES-2026-0002 and RES-2026-0003 added (P1 research). ADR-0002 and ADR-0003 recorded from them.
- 2026-09-28: RES-2026-0001 added (G-SPIKE PASS).

## Deprecated, superseded or archived

None.

## Relationships and open work

- The three P1 studies are complete. The remaining P1 exit item is plan approval (TASK-013).
- The open items in the table above are carried by their plan tasks. None blocks P2 entry.

## Recommended reading order

1. [RES-2026-0001](res-2026-0001-capy-grammar-spike.md): how Rivet source becomes a syntax tree.
2. [RES-2026-0002](res-2026-0002-rust-crate-feasibility.md): the runtime's dependency stack.
3. [RES-2026-0003](res-2026-0003-process-sandbox-backends.md): what a sandboxed child can and cannot do per OS.

## Related directories

[decisions](../decisions/index.md) · [proposals](../proposals/index.md) · [plans](../plans/index.md) ·
[references](../references/index.md) · [current state](../README.md) · [navigation index](../index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Created the research index with RES-2026-0001, RES-2026-0002 and RES-2026-0003. |
