---
document_id: REF-2026-0024
title: "Rivet system components"
document_type: reference
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, registry, policy, audit, cli, library, serve, http, ws, poll, mcp]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, embedded, server]
audience: [maintainers, implementers, operators, reviewers]
scope: Navigation and status for docs/system/components/.
reason: AGENTS.md requires an index.md in every documentation directory; system/components/ was created for PLAN-2026-0001 phase P4.
dependencies: [DOCUMENTATION.md, AGENTS.md]
related_documents: [REF-2026-0023, SYS-2026-0001, SYS-2026-0003, SYS-2026-0004, PLAN-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, system, index]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-28
---

# Rivet system components

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** language, registry, policy, audit, cli, library, serve, http, ws, poll, mcp

## Purpose

Compile-time and control-plane parts of Rivet: the compiler and catalog, the policy broker and I/O manifest, and the surfaces that expose operations. Documents here describe the current implemented code (DOCUMENTATION §4.7, template §12.6) and cite the
modules they describe.

**Belongs here:** `SYS-…` current-state documents on this subject. **Does not belong here:** design intent
(`../../proposals/`), decisions (`../../decisions/`), how-to material or release history.

**Naming:** `sys-<YYYY>-<NNNN>-<short-description>.md`.

```text
   app.rivet ──▶ SYS-0001 compile ──▶ registry ──▶ SYS-0004 surfaces (CLI · library · serve)
                        │                                   │
                        └── effect sites ──▶ SYS-0003 IoManifest / policy generate
                                              SYS-0003 broker ◀── every effect attempt at run time
```

## Active documents

| ID | Document | Covers |
|---|---|---|
| SYS-2026-0001 | [Compiler and operation catalog](sys-2026-0001-compiler-and-catalog.md) | rivet.capy grammar, AST JSON boundary, lowering, expression parser, checks, registry, declared outputs |
| SYS-2026-0003 | [Policy broker and I/O manifest](sys-2026-0003-policy-broker-and-io-manifest.md) | load_policy, authorize_effect, traced broker, effect analysis, IoManifest, policy generate, trace store |
| SYS-2026-0004 | [Surfaces and serve](sys-2026-0004-surfaces-and-serve.md) | CLI and --endpoint client, library, serve listener, REST/SSE/poll/WS/MCP, principals |

## Recently added or updated

- 2026-09-28: created for PLAN-2026-0001 phase P4, verified against `0.1.0-dev` commit `f40d4aa`.

## Deprecated, superseded or archived

None.

## Relationships, open work and reading order

Read in the order of the table above. Open work and known 0.1.0 limitations are listed in each document's
*Known Limitations* section and summarised in the [system index](../index.md), which also shows how this folder
relates to the rest of the system.

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Created the index for PLAN-2026-0001 phase P4. |
