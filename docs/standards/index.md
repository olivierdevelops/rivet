---
document_id: REF-2026-0009
title: "Rivet standards baseline"
document_type: reference
status: draft
created_date: 2026-09-27
last_updated: 2026-09-28
document_revision: 5
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

# Rivet standards baseline

This index lists existing supplied requirements; it does not activate new standards. The explicit user Rust request supersedes inherited Go/email_provider project-facts text. DOCUMENTATION §§4.2, 12.1 and 21 govern proposal coverage; AGENTS adds the required complexity, reference-engine and forecast sections.

## Reading order and active documents

- [Supplied AGENTS instructions](../../AGENTS.md)
- [Documentation standard, revision 4](../../DOCUMENTATION.md)
- [Original brief](../../PROJECT.md)
- [Proposal validation](../proposals/approved/prop-2026-0001-rivet-runtime.md#project-validation)

## Status and maintenance

No standard changed in design revision 5. Nothing deprecated, superseded or archived. Update this index when a document changes lifecycle or a new related document is added.

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 5 | 2026-09-28 | Claude | Removed a stale proposal-status paragraph copied from other indexes; recorded that no standard changed in revision 5. |
| 4 | 2026-09-28 | Codex | Added twelve draft sample folders with source files, fixtures, request bodies and usage READMEs (UQ-16). |
| 3 | 2026-09-28 | Codex | Added gRPC, documented multi-operation catalogs, incoming MCP tools and duplex sessions; expanded reference to 120 examples. |
| 2 | 2026-09-28 | Codex | Added UDP, OAuth 2.0, QUIC/HTTP3 design coverage, traceability and 22 examples; updated current-state navigation. |
| 1 | 2026-09-27 | Codex | Created design corpus navigation and status. |
