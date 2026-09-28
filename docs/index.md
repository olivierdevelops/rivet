---
document_id: REF-2026-0005
title: "Rivet documentation index"
document_type: reference
status: draft
created_date: 2026-09-27
last_updated: 2026-09-28
document_revision: 10
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, execution, cli, http, mcp, library, policy]
affected_versions:
  from: not-applicable
  to: proposed-v0.1
applicable_environments: [development, embedded, server]
audience: [maintainers, developers, reviewers]
scope: Proposed Rivet behavior and design review; no implementation or release claim.
reason: Record the project brief and requested changes as reviewable contracts and examples.
dependencies: [PROJECT.md, DOCUMENTATION.md, AGENTS.md]
related_documents: ["PROP-2026-0001"]
supersedes: null
superseded_by: null
tags: [rivet, rust, capy, design]
confidentiality: internal
review_cycle: on-design-change
next_review_date: 2026-10-27
---

# Rivet documentation index

Navigate the design corpus. Current-state claims belong in README.md; design decisions belong in proposals.

```text
docs/
├── README.md            current state, risks, gates
├── index.md             this navigation page
├── decisions/           ADR-0001 approval · ADR-0002 crates · ADR-0003 sandbox · ADR-0004 v0.2.0 approval
├── incidents/           INC-2026-0001 … 0008 (all resolved; found during implementation)
├── troubleshooting/     TRBL-2026-0001 … 0003
├── research/            RES-2026-0001 Capy spike · 0002 crates · 0003 sandbox (completed)
├── manuals/             MAN-2026-0001 manual + 7 volumes (current-state book)
├── system/              SYS-2026-0001 … 0009 (implemented components)
├── api/                 API-2026-0001 … 0005 (HTTP, WebSocket, MCP, Rust library, error registry)
├── architecture/        ARCH-2026-0001
├── security/            SEC-2026-0001
├── operations/          OPS-2026-0001
├── runbooks/            RUN-2026-0001 · 0002
├── onboarding/          ONB-2026-0001
├── testing/             TEST-2026-0001 … 0033 (one per plan test row, latest recorded result)
├── reports/             RPT-2026-0001 validation of PLAN-2026-0001
├── releases/            REL-0.1.0 (first release, tag v0.1.0)
├── plans/               PLAN-2026-0001 (v0.1.0, released) · PLAN-2026-0002 (v0.2.0, in progress)
├── proposals/           design proposals by lifecycle
│   ├── approved/        PROP-2026-0002 (v0.2.0: envelopes, globals, library, C ABI, highlighting)
│   ├── implemented/     PROP-2026-0001 Rivet runtime (implemented in v0.1.0, revision 9)
│   └── draft/           (empty)
├── references/          REF-2026-0001 request + evidence, REF-2026-0002 numbered examples S01–S159
├── demos/               twelve draft sample folders (index.md = status, README.md = walkthroughs)
└── standards/           supplied rules baseline
```

## Reading order and active documents

- [Current state](README.md)
- [Decisions](decisions/index.md)
- [Research](research/index.md)
- [Incidents](incidents/index.md)
- [Tests](testing/index.md)
- [Reports](reports/index.md)
- [Releases](releases/index.md)
- [Troubleshooting](troubleshooting/index.md)
- [Proposals](proposals/index.md), then [implemented proposals](proposals/implemented/index.md)
- [Plans](plans/index.md)
- [References](references/index.md)
- [Sample folders: status and reading order](demos/index.md), then the [walkthroughs](demos/README.md)
- [Standards baseline](standards/index.md)

## Status and maintenance

Recently updated, 2026-09-28: the maintainer approved the design, contract and plan
([ADR-0001](decisions/adr-0001-approve-rivet-runtime-design.md)). Added [decisions/](decisions/index.md) and
[proposals/approved/](proposals/approved/index.md) to navigation. PROP-2026-0001 is at design revision 8
(option-derived file sites, `io --needs`, `--check-files`). No runtime capability or release was added. Nothing
is deprecated, superseded or archived. Pending work: the Capy parse spike (G-SPIKE), research spikes and
ADR-0002/0003, then P2 implementation. Update this index when a document changes lifecycle or a new related
document is added. The references explain the proposal; the standards describe how it must be reviewed.

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
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
