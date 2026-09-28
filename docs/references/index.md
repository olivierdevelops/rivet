---
document_id: REF-2026-0008
title: "Rivet references"
document_type: reference
status: draft
created_date: 2026-09-27
last_updated: 2026-09-28
document_revision: 6
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

# Rivet references

Source records and proposed syntax references, named ref-YYYY-NNNN-topic.md. These are not release manuals or executed demos.

```text
  REF-2026-0001  request + evidence  ──(UQ-01..UQ-18 feed)──>  PROP-2026-0001 requirements
  REF-2026-0002  numbered examples   ──(S-ids cited by)─────>  PROP-2026-0001 + docs/demos
```

## Reading order and active documents

- [Request and evidence](ref-2026-0001-request-and-evidence.md) — user requests UQ-01 to UQ-18 and the licence evidence (G-LIC closed by the Capy owner, ADR-0001); UQ-17 supersedes UQ-13's `--sandbox` flag syntax while keeping its deny-by-default behaviour
- [Numbered usage examples](ref-2026-0002-language-and-usage.md) — S01–S159: Capy prefix-call syntax, declared outputs, policy.json-only commands, unified `serve`, the generated I/O manifest, option-derived file sites and `rivet io --needs` / `--check-files` (S154–S159)
- [Sample folders: status](../demos/index.md) and [walkthroughs](../demos/README.md)
- [Proposal](../proposals/approved/prop-2026-0001-rivet-runtime.md)

## Status and maintenance

Recently updated, 2026-09-28: REF-2026-0001 records the closure of G-LIC; REF-2026-0002 adds option-derived file sites and S154–S159 for design revision 8 (TASK-005, approved by [ADR-0001](../decisions/adr-0001-approve-rivet-runtime-design.md)). Earlier the same day both were revised for design revisions 5 (UQ-17) and 6 (UQ-18). Nothing deprecated, superseded or archived. Open question: example syntax is unverified until the Capy parser spike parses every numbered example. Update this index when a document changes lifecycle or a new related document is added.

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 6 | 2026-09-28 | Claude | Status for design revision 8: G-LIC closed in REF-2026-0001; S154–S159 (option-derived file sites, `io --needs`, `--check-files`) in REF-2026-0002. |
| 5 | 2026-09-28 | Claude | Status for design revision 5: UQ-17 recorded, examples moved to prefix calls/policy.json/unified serve; linked demos index; parser spike noted as open. |
| 4 | 2026-09-28 | Codex | Added twelve draft sample folders with source files, fixtures, request bodies and usage READMEs (UQ-16). |
| 3 | 2026-09-28 | Codex | Added gRPC, documented multi-operation catalogs, incoming MCP tools and duplex sessions; expanded reference to 120 examples. |
| 2 | 2026-09-28 | Codex | Added UDP, OAuth 2.0, QUIC/HTTP3 design coverage, traceability and 22 examples; updated current-state navigation. |
| 1 | 2026-09-27 | Codex | Created design corpus navigation and status. |
