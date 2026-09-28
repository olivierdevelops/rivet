---
document_id: REF-2026-0007
title: "Draft proposals"
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

# Draft proposals

Unapproved design proposals. PROP-2026-0001 uses status proposed to distinguish a decision request from implementation. Approval remains unrecorded.

## Reading order and active documents

- [PROP-2026-0001 — Rivet runtime](prop-2026-0001-rivet-runtime.md)
- [Parent proposal index](../index.md)

## Status and maintenance

```text
  PROP-2026-0001  status: proposed   design revision 5
    gates:  [ ] human design review
            [ ] G-LIC  Capy relicensed to MIT (owner: project user)
            [ ] Capy parser spike: every numbered example + docs/demos .rivet parses
```

Recently updated, 2026-09-28: design revision 5 (UQ-17) — declared outputs, policy.json-only configuration, one `serve` for every surface, Capy prefix calls and the review fixes. Nothing deprecated, superseded or archived. Update this index when a document changes lifecycle or a new related document is added.

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 5 | 2026-09-28 | Claude | Status for design revision 5 with the approval-gate checklist. |
| 4 | 2026-09-28 | Codex | Added twelve draft sample folders with source files, fixtures, request bodies and usage READMEs (UQ-16). |
| 3 | 2026-09-28 | Codex | Added gRPC, documented multi-operation catalogs, incoming MCP tools and duplex sessions; expanded reference to 120 examples. |
| 2 | 2026-09-28 | Codex | Added UDP, OAuth 2.0, QUIC/HTTP3 design coverage, traceability and 22 examples; updated current-state navigation. |
| 1 | 2026-09-27 | Codex | Created design corpus navigation and status. |
