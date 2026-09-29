---
document_id: REF-2026-0007
title: "Draft proposals"
document_type: reference
status: draft
created_date: 2026-09-27
last_updated: 2026-09-28
document_revision: 8
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

# Draft proposals

Proposals awaiting a maintainer decision live here with `status: draft` or `proposed`. When a decision record
approves one, it moves to [approved/](../approved/index.md) and keeps its file name.

## Reading order and active documents

| Proposal | Status | Target | Summary |
|---|---|---|---|
| [PROP-2026-0002](../implemented/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md) | approved (moved) | v0.2.0 | Standard input/output envelopes, pretty JSON, `global` constants, Rivet as a Cargo dependency, C ABI shared/static library, syntax highlighting |

- PROP-2026-0001 (Rivet runtime) was implemented in v0.1.0 and lives at
  [implemented/prop-2026-0001-rivet-runtime.md](../implemented/prop-2026-0001-rivet-runtime.md).
- [Parent proposal index](../index.md)

## Status and maintenance

```text
  draft/        (empty)
  approved/     PROP-2026-0002  ADR-0004 → PLAN-2026-0002 (v0.2.0)
  implemented/  PROP-2026-0001  v0.1.0
```

Recently updated, 2026-09-28: PROP-2026-0002 drafted, then approved (ADR-0004) and moved to approved/. Update this index when a document changes lifecycle.

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 8 | 2026-09-28 | Claude | PROP-2026-0002 approved and moved to approved/. |
| 7 | 2026-09-28 | Claude | Added PROP-2026-0002 (draft). |
| 6 | 2026-09-28 | Claude | PROP-2026-0001 approved and moved to `approved/`; this directory is now empty. |
| 5 | 2026-09-28 | Claude | Status for design revision 5 with the approval-gate checklist. |
| 4 | 2026-09-28 | Codex | Added twelve draft sample folders with source files, fixtures, request bodies and usage READMEs (UQ-16). |
| 3 | 2026-09-28 | Codex | Added gRPC, documented multi-operation catalogs, incoming MCP tools and duplex sessions; expanded reference to 120 examples. |
| 2 | 2026-09-28 | Codex | Added UDP, OAuth 2.0, QUIC/HTTP3 design coverage, traceability and 22 examples; updated current-state navigation. |
| 1 | 2026-09-27 | Codex | Created design corpus navigation and status. |
