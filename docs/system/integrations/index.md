---
document_id: REF-2026-0031
title: "Rivet system integrations"
document_type: reference
status: active
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 3
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [transports, datagrams, quic, grpc, http, auth, connectors, mcp]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, embedded, server]
audience: [maintainers, implementers, operators, reviewers]
scope: Navigation and status for docs/system/integrations/.
reason: AGENTS.md requires an index.md in every documentation directory; system/integrations/ was created for PLAN-2026-0001 phase P4.
dependencies: [DOCUMENTATION.md, AGENTS.md]
related_documents: [REF-2026-0023, SYS-2026-0005, SYS-2026-0006, SYS-2026-0009, PLAN-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, system, index]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-29
---

# Rivet system integrations

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** transports, datagrams, quic, grpc, http, auth, connectors, mcp

## Purpose

Outbound integrations: protocol adapters, OAuth 2.0 credentials and outbound MCP client connectors. Every one is reached only through the policy broker. Documents here describe the current implemented code (DOCUMENTATION §4.7, template §12.6) and cite the
modules they describe.

**Belongs here:** `SYS-…` current-state documents on this subject. **Does not belong here:** design intent
(`../../proposals/`), decisions (`../../decisions/`), how-to material or release history.

**Naming:** `sys-<YYYY>-<NNNN>-<short-description>.md`.

```text
   interpreter ──▶ policy broker ──▶ SYS-0005 adapters ──▶ network / processes
                                 ├─▶ SYS-0006 OAuth ────▶ token endpoint (bearer bound to destination)
                                 └─▶ SYS-0009 MCP client ▶ MCP server (stdio / Streamable HTTP)
```

## Active documents

| ID | Document | Covers |
|---|---|---|
| SYS-2026-0005 | [Protocol adapters](sys-2026-0005-protocol-adapters.md) | HTTP/1.1-3 client, sockets, websocket, processes and sandboxes, UDP, QUIC, gRPC, codecs, TLS |
| SYS-2026-0006 | [OAuth 2.0 and credentials](sys-2026-0006-oauth-and-credentials.md) | profiles, grant types, transactions, token store, refresh |
| SYS-2026-0009 | [MCP client connectors](sys-2026-0009-mcp-client-connectors.md) | connectors, reviewed snapshots, connectors sync |

## Recently added or updated

- 2026-09-29: SYS-2026-0005, SYS-2026-0006 and SYS-2026-0009 were revised (revision 3) and re-verified on the 0.2.0-rc. They add the Cargo feature gates per adapter (`grpc`, `quic`, `oauth`) and show every capture as an envelope (PLAN-2026-0002 D-35, D-47). Windows is not supported (INC-2026-0011).
- 2026-09-28: created for PLAN-2026-0001 phase P4, verified against `0.1.0-dev` commit `f40d4aa`.
- 2026-09-28: SYS-2026-0005, SYS-2026-0006 and SYS-2026-0009 revised (revision 2) for the post-P3 fix batch and re-verified at commit `829ca43`
  (TASK-092); limitations now point to the [manual's Known Limitations](../../manuals/man-2026-0001-rivet-manual.md#known-limitations).

## Deprecated, superseded or archived

None.

## Relationships, open work and reading order

Read in the order of the table above. Open work and known limitations are listed in each document's
*Known Limitations* section and summarised in the [system index](../index.md), which also shows how this folder
relates to the rest of the system.

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Created the index for PLAN-2026-0001 phase P4. |
| 2 | 2026-09-28 | Claude | Recorded the fix-batch revision (commit 829ca43) of this folder's documents. |
| 3 | 2026-09-29 | Claude | Recorded revision 3 of SYS-2026-0005, 0006 and 0009 (feature gates, envelope captures; PLAN-2026-0002 D-35, D-47). |
