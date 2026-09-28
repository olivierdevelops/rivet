---
document_id: REF-2026-0005
title: "Rivet documentation index"
document_type: reference
status: active
created_date: 2026-09-27
last_updated: 2026-09-29
document_revision: 11
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, execution, cli, http, mcp, library, policy, ffi]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, embedded, server]
audience: [maintainers, developers, reviewers]
scope: Navigation of the Rivet documentation — every directory with its current documents, for release 0.1.0 and the 0.2.0 release candidate (in progress).
reason: AGENTS.md "Directory Indexes" and PLAN-2026-0002 rows D-66, D-72 (TASK-080) — one page that points to every documentation directory and lifecycle document.
dependencies: [PROJECT.md, DOCUMENTATION.md, AGENTS.md]
related_documents: ["PROP-2026-0001", "PROP-2026-0002", "PLAN-2026-0001", "PLAN-2026-0002", "MIG-2026-0001", "INC-2026-0011", "STD-2026-0001"]
supersedes: null
superseded_by: null
tags: [rivet, index, navigation]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-29
---

# Rivet documentation index

> **Status:** Active
> **Created:** 2026-09-27
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** language, execution, cli, http, mcp, library, policy, ffi

Navigate the documentation. Current-state claims belong in [README.md](README.md); design decisions belong in
proposals and decisions. The current release is **0.1.0**; **0.2.0 is in progress** (PLAN-2026-0002).
Supported platforms: macOS and Linux.

```text
docs/
├── README.md            current state, 0.2.0 progress, risks, gates
├── index.md             this navigation page
├── decisions/           ADR-0001 approval · 0002 crates · 0003 sandbox · 0004 v0.2.0 approval · 0005 workspace/features
├── incidents/           resolved INC-2026-0001 … 0010 · active INC-2026-0011 (Windows port failures)
├── troubleshooting/     TRBL-2026-0001 … 0007
├── research/            RES-2026-0001 Capy spike · 0002 crates · 0003 sandbox · 0004 workspace/FFI/features
├── manuals/             MAN-2026-0001 root manual + 9 volumes (0009 C ABI/FFI · 0010 editors/highlighting)
├── system/              SYS-2026-0001 … 0011 (0010 FFI surface/packaging · 0011 highlighting/grammar)
├── api/                 API-2026-0001 … 0007 (HTTP, WebSocket, MCP, Rust, errors, 0006 envelopes, 0007 C ABI) + schemas/
├── migrations/          MIG-2026-0001 0.1.0 → 0.2.0 envelopes, CLI flags, Rust facade
├── architecture/        ARCH-2026-0001
├── security/            SEC-2026-0001
├── operations/          OPS-2026-0001
├── runbooks/            RUN-2026-0001 · 0002
├── onboarding/          ONB-2026-0001
├── testing/             TEST-2026-0001 … 0033 (one per 0.1.0 plan test row, latest recorded result)
├── reports/             RPT-2026-0001 validation of PLAN-2026-0001
├── releases/            REL-0.1.0 (first release, tag v0.1.0); REL-0.2.0 comes with P5
├── plans/               PLAN-2026-0001 (v0.1.0, released) · PLAN-2026-0002 (v0.2.0, in progress)
├── proposals/           design proposals by lifecycle
│   ├── approved/        PROP-2026-0002 (v0.2.0: envelopes, globals, modules, library, C ABI, highlighting)
│   ├── implemented/     PROP-2026-0001 Rivet runtime (implemented in v0.1.0, revision 9)
│   └── draft/           (empty)
├── references/          REF-2026-0001 request + evidence, REF-2026-0002 numbered examples
├── demos/               sample folders 01–13 (0.1.0, re-verified for 0.2.0 in P4); 14-globals, 15-ffi,
│                        16-editor, 17-modules and DEMO-2026-0020 are being added
└── standards/           STD-2026-0001 orchestrator and cross-package review
```

```text
 lifecycle of 0.2.0
 PROP-2026-0002 ─► ADR-0004 ─► PLAN-2026-0002 ─► RES-2026-0004 / ADR-0005 ─► code (P2) ─► INC/TRBL as found
        ─► API-0006/0007 · MAN-0009/0010 · SYS-0010/0011 · MIG-0001 · manuals and API re-captured (P4)
        ─► REL-0.2.0 (P5)
```

## Reading order and active documents

- [Current state](README.md)
- [Manuals](manuals/index.md) — start with the [root manual](manuals/man-2026-0001-rivet-manual.md) and its [What's New in 0.2.0](manuals/man-2026-0001-rivet-manual.md#whats-new-in-020)
- [API](api/index.md) — the [envelopes](api/api-2026-0006-envelopes.md) first
- [Migrations](migrations/index.md) — [MIG-2026-0001](migrations/mig-2026-0001-response-and-input-envelopes.md) for 0.1.0 clients
- [System](system/index.md) · [Architecture](architecture/index.md) · [Security](security/index.md)
- [Operations](operations/index.md) · [Runbooks](runbooks/index.md) · [Onboarding](onboarding/index.md)
- [Decisions](decisions/index.md) · [Research](research/index.md)
- [Incidents](incidents/index.md) · [Troubleshooting](troubleshooting/index.md)
- [Tests](testing/index.md) · [Reports](reports/index.md) · [Releases](releases/index.md)
- [Proposals](proposals/index.md), then [approved](proposals/approved/index.md) and [implemented](proposals/implemented/index.md) proposals
- [Plans](plans/index.md)
- [References](references/index.md)
- [Sample folders: status and reading order](demos/index.md), then the [walkthroughs](demos/README.md)
- [Standards baseline](standards/index.md) — [STD-2026-0001](standards/std-2026-0001-orchestrator-and-cross-package-review.md)

## Status and maintenance

Recently updated, 2026-09-29: 0.2.0 documentation (PLAN-2026-0002 P4) — API-2026-0006/0007, MAN-2026-0009/0010,
SYS-2026-0010/0011, MIG-2026-0001 and the new `migrations/` directory; the API documents and manuals re-captured on
the 0.2.0 release candidate. INC-2026-0010 resolved; INC-2026-0011 (Windows) active: Windows is not a supported
platform. Nothing is deprecated, superseded or archived; the 0.1.0 wire shapes are documented only as history in
MIG-2026-0001. Pending: demos 14–17, DEMO-2026-0020, RPT-2026-0015 and REL-0.2.0 (P4/P5). Update this index when a
document changes lifecycle or a new directory or document is added.

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 11 | 2026-09-29 | Claude | TASK-080 / D-66, D-72: status active (was draft) with a visible header; tree updated to the real inventory (ADR-0005, INC-0009…0011, TRBL-0004…0007, RES-0004, MAN-0009/0010, SYS-0010/0011, API-0006/0007 and schemas, new `migrations/`, STD-2026-0001, demos 14–17 in progress); 0.2.0 lifecycle diagram; reading order adds manuals, API, migrations and the other current-state directories; release 0.1.0 with 0.2.0 in progress. |
| 10 | 2026-09-28 | Claude | Added manuals, system, API, architecture, security, operations, runbooks and onboarding to navigation. |
| 9 | 2026-09-28 | Claude | Added incidents/ and troubleshooting/ to navigation. |
| 8 | 2026-09-28 | Claude | Added research/ to navigation; ADR-0002/0003 approved; P1 complete. |
| 7 | 2026-09-28 | Claude | Added decisions/ and proposals/approved/ to navigation; status after ADR-0001 approval and design revision 8. |
| 6 | 2026-09-28 | Claude | Added plans/ with PLAN-2026-0001 to navigation. |
| 5 | 2026-09-28 | Claude | Added the demos index to navigation and a directory map; status updated for design revision 5. |
| 4 | 2026-09-28 | Codex | Added twelve draft sample folders with source files, fixtures, request bodies and usage READMEs (UQ-16). |
| 3 | 2026-09-28 | Codex | Added gRPC, documented multi-operation catalogs, incoming MCP tools and duplex sessions; expanded reference to 120 examples. |
| 2 | 2026-09-28 | Codex | Added UDP, OAuth 2.0, QUIC/HTTP3 design coverage, traceability and 22 examples; updated current-state navigation. |
| 1 | 2026-09-27 | Codex | Created design corpus navigation and status. |
