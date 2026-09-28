---
document_id: REF-2026-0005
title: "Rivet documentation index"
document_type: reference
status: draft
created_date: 2026-09-27
last_updated: 2026-09-28
document_revision: 6
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, runtime, interfaces, sandbox]
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
├── plans/               PLAN-2026-0001 v0.1.0 implementation, validation and release (draft)
├── proposals/           decision requests
│   └── draft/           PROP-2026-0001 Rivet runtime (proposed)
├── references/          REF-2026-0001 request + evidence, REF-2026-0002 numbered examples
├── demos/               twelve draft sample folders (index.md = status, README.md = walkthroughs)
└── standards/           supplied rules baseline
```

## Reading order and active documents

- [Current state](README.md)
- [Proposals](proposals/index.md)
- [Plans](plans/index.md)
- [References](references/index.md)
- [Sample folders: status and reading order](demos/index.md), then the [walkthroughs](demos/README.md)
- [Standards baseline](standards/index.md)

## Status and maintenance

Recently updated, 2026-09-28: design revision 5 (UQ-17) — declared outputs and `rivet outputs`, `policy.json` as the only policy source, one `rivet serve` for HTTP/SSE/polling/WebSocket/MCP, Capy prefix-call syntax and the review fixes. Added [demos/index.md](demos/index.md) so every documentation directory has an index. No runtime capability or release was added. Nothing deprecated, superseded or archived. Pending work: human design review, the G-LIC Capy licence gate, the Capy parser spike and platform enforcement evidence. Update this index when a document changes lifecycle or a new related document is added. The references explain the proposal; the standards describe how it must be reviewed.

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 6 | 2026-09-28 | Claude | Added plans/ with PLAN-2026-0001 to navigation. |
| 5 | 2026-09-28 | Claude | Added the demos index to navigation and a directory map; status updated for design revision 5. |
| 4 | 2026-09-28 | Codex | Added twelve draft sample folders with source files, fixtures, request bodies and usage READMEs (UQ-16). |
| 3 | 2026-09-28 | Codex | Added gRPC, documented multi-operation catalogs, incoming MCP tools and duplex sessions; expanded reference to 120 examples. |
| 2 | 2026-09-28 | Codex | Added UDP, OAuth 2.0, QUIC/HTTP3 design coverage, traceability and 22 examples; updated current-state navigation. |
| 1 | 2026-09-27 | Codex | Created design corpus navigation and status. |
