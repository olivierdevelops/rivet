---
document_id: REF-2026-0021
title: "Rivet architecture documentation index"
document_type: reference
status: active
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 3
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [execution, policy, serve, library, cli]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, embedded, server]
audience: [maintainers, contributors, reviewers]
scope: Navigation and status page for docs/architecture — how the implemented Rivet runtime is structured.
reason: AGENTS.md "Directory Indexes" requires an index for every documentation directory; created with ARCH-2026-0001 (PLAN-2026-0001 D-14).
related_documents: [PLAN-2026-0001, PLAN-2026-0002, ARCH-2026-0001, ADR-0002, ADR-0003, ADR-0005, SYS-2026-0010]
supersedes: null
superseded_by: null
tags: [rivet, architecture, index]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-29
---

# Rivet architecture documentation index

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** execution, policy, serve, library, cli

## Purpose

`docs/architecture/` explains how the **implemented** Rivet runtime is structured: component boundaries, the port/adapter map, request and data flow, and deployment shapes. It is written from the `src/` tree, not from the design.

```text
   domain ◀── features ◀── orchestrator ──▶ io        (surfaces)
      ▲                         │
      └──────── infra ◀─────────┘                      (adapters satisfy domain ports)
```

## What belongs here

- Belongs: component structure, dependency direction, sequence and data-flow diagrams, deployment topology.
- Does not belong: wire contracts ([api/](../api/index.md)), threat model ([security/](../security/index.md)), design alternatives and history ([decisions/](../decisions/index.md), [proposals/](../proposals/index.md)).

## Naming rules

`arch-YYYY-NNNN-<slug>.md`, `document_type: architecture`, ID prefix `ARCH`.

## Active documents

| ID | Document | Covers |
|---|---|---|
| ARCH-2026-0001 | [Rivet runtime architecture](arch-2026-0001-rivet-runtime-architecture.md) | workspace (rivet-runtime, rivet-ffi), facade vs hidden buckets, envelope edge layer, Cargo feature gates, file modules and catalog snapshots, `ffi` surface, five buckets, request lifecycle, policy broker, interpreter/handles/DAG, serve fan-out, adapters, data flow, CLI/serve/library/C ABI shapes |

## Recently added or updated

2026-09-29 — ARCH-2026-0001 revision 3 for 0.2.0 (PLAN-2026-0002 D-30): workspace and packages, the facade boundary (`src/internal.rs`), the envelope edge layer, Cargo feature gates, file modules and catalog snapshots, and the `ffi` surface, with diagrams. The document now counts 45 use cases and names the supported platforms (macOS and Linux).

2026-09-28 — ARCH-2026-0001 created for 0.1.0 from commit `f40d4aa`.

2026-09-28 — ARCH-2026-0001 revision 2 for the fix batch (commit `829ca43`): 40 use cases, ceiling and restriction layers, structured cancellation, secret taint, serve health/access log/drain, WS lanes, library scopes; drift recorded and fixed in `2a751ab` (`rivet.trace.export` dispatch).

## Deprecated, superseded or archived

None.

## Important relationships

- Crate choices behind the adapters: [ADR-0002](../decisions/adr-0002-rust-crate-selection.md); sandbox backends: [ADR-0003](../decisions/adr-0003-process-sandbox-backends.md).
- Plan row D-14 of [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md); row D-30 of [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md).
- Packaging and the `ffi` surface in depth: [SYS-2026-0010](../system/components/sys-2026-0010-ffi-surface-and-packaging.md); decision: [ADR-0005](../decisions/adr-0005-workspace-package-and-features.md).

## Unresolved work and open questions

Update ARCH-2026-0001 when a persistent trace store, TLS listener, connection pooling, a serve-embedding facade or crates.io publication is added (all absent in 0.2.0). (The `rivet.trace.export` dispatch drift found at `829ca43` was fixed in `2a751ab`.) Current limitations: [manual's Known Limitations](../manuals/man-2026-0001-rivet-manual.md#known-limitations).

## Recommended reading order

1. [ARCH-2026-0001](arch-2026-0001-rivet-runtime-architecture.md) · 2. [API index](../api/index.md) · 3. [Security model](../security/sec-2026-0001-policy-and-sandbox-model.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Created the architecture index with ARCH-2026-0001. |
| 2 | 2026-09-28 | Claude | Recorded ARCH-2026-0001 revision 2 (fix batch, commit 829ca43) and the open drift. |
| 3 | 2026-09-29 | Claude | Recorded ARCH-2026-0001 revision 3 (0.2.0, PLAN-2026-0002 D-30) and the updated open items. |
