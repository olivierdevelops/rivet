---
document_id: REF-2026-0037
title: "Rivet migrations index"
document_type: reference
status: active
created_date: 2026-09-29
last_updated: 2026-09-29
document_revision: 1
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [cli, http, poll, ws, mcp, library]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, embedded, server]
audience: [developers, integrators, operators]
scope: Navigation and status page for docs/migrations — guides for moving clients, hosts and deployments between Rivet versions.
reason: AGENTS.md "Directory Indexes" requires an index for every documentation directory; created with MIG-2026-0001 (PLAN-2026-0002 D-09).
related_documents: [MIG-2026-0001, PLAN-2026-0002, API-2026-0006]
supersedes: null
superseded_by: null
tags: [rivet, migrations, index]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-29
---

# Rivet migrations index

> **Status:** Active
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** envelope, cli, http, sse, poll, ws, mcp, library, packaging

## Purpose

`docs/migrations/` holds one guide per breaking move between Rivet versions (DOCUMENTATION §4.21). Each guide shows
the before and after of every affected surface, a checklist and a rollback plan.

```text
 0.1.0 ───────────── MIG-2026-0001 ─────────────▶ 0.2.0 ─────── (0.3.0: aliases removed) ───────▶
       outputs → envelopes · inputs {operation,data} · --data/--input · --features cli · rivet-runtime facade
```

## What belongs here

- Belongs: version-to-version client, host, configuration and packaging migrations with rollback.
- Does not belong: the current contracts themselves ([api/](../api/index.md)), release history ([releases/](../releases/index.md)).

## Naming rules

`mig-YYYY-NNNN-<slug>.md`, `document_type: migration`, ID prefix `MIG`.

## Active documents

| ID | Document | From → to | Covers |
|---|---|---|---|
| MIG-2026-0001 | [Response and input envelopes, CLI flags and the Rust facade](mig-2026-0001-response-and-input-envelopes.md) | 0.1.0 → 0.2.x | CLI, HTTP, SSE, polling, WebSocket, MCP, Rust library, install; jq mapping; 0.2 → 0.3 timeline; rollback to v0.1.0 |

## Recently added or updated

2026-09-29 — directory created with MIG-2026-0001 for the 0.2.0 release.

## Deprecated, superseded or archived

None.

## Important relationships

- The target contract: [API-2026-0006](../api/api-2026-0006-envelopes.md).
- Requirement R5 of [PROP-2026-0002](../proposals/approved/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md),
  executed under [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md).

## Unresolved work and open questions

- A migration guide for 0.3.0 (alias removal) is due with that release.

## Related directories

[api/](../api/index.md) · [manuals/](../manuals/index.md) · [releases/](../releases/index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-29 | Claude | Created the migrations index with MIG-2026-0001. |
