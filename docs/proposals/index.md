---
document_id: REF-2026-0006
title: "Rivet proposals"
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

# Rivet proposals

Decision requests awaiting review. Documents are named prop-YYYY-NNNN-topic.md and live under their lifecycle directory. No approved or implemented proposal exists.

```text
  draft/ ──(human approval + G-LIC + parser spike)──> implemented/   (none yet)
         └─(rejection)──────────────────────────────> rejected/      (none)
```

## Reading order and active documents

- [Draft proposals](draft/index.md)
- [Runtime proposal](draft/prop-2026-0001-rivet-runtime.md) — design revision 5, requirements R1–R25
- [Reference samples](../references/ref-2026-0002-language-and-usage.md)

## Status and maintenance

Recently updated, 2026-09-28: PROP-2026-0001 revised for UQ-17 (R23 declared outputs, R24 policy.json only, R25 one `serve` for every surface) plus the review fixes. Nothing deprecated, superseded or archived. Unresolved: human design review; approval gates G-LIC (Capy owner relicenses to MIT) and the Capy parser spike. Update this index when a document changes lifecycle or a new related document is added.

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 5 | 2026-09-28 | Claude | Status for design revision 5 (R23–R25) and the two approval gates; added lifecycle diagram. |
| 4 | 2026-09-28 | Codex | Added twelve draft sample folders with source files, fixtures, request bodies and usage READMEs (UQ-16). |
| 3 | 2026-09-28 | Codex | Added gRPC, documented multi-operation catalogs, incoming MCP tools and duplex sessions; expanded reference to 120 examples. |
| 2 | 2026-09-28 | Codex | Added UDP, OAuth 2.0, QUIC/HTTP3 design coverage, traceability and 22 examples; updated current-state navigation. |
| 1 | 2026-09-27 | Codex | Created design corpus navigation and status. |
