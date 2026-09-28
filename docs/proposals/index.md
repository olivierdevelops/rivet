---
document_id: REF-2026-0006
title: "Rivet proposals"
document_type: reference
status: draft
created_date: 2026-09-27
last_updated: 2026-09-28
document_revision: 9
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

# Rivet proposals

Design proposals for Rivet, one directory per lifecycle state. Documents are named
`prop-YYYY-NNNN-topic.md` and keep their name when they move between directories. Only the directory and the
`status` field change. An approved proposal has a decision record in [decisions](../decisions/index.md).

```text
  draft/ ──(ADR approval)──▶ approved/ ──(plan released)──▶ implemented/
     │                          │
     └─(rejection)─▶ rejected/  └─(replaced)─▶ superseded/                  (none)

  today:  draft/     (empty)
          approved/  PROP-2026-0002  ── ADR-0004 ──▶ PLAN-2026-0002 (v0.2.0)
          implemented/  PROP-2026-0001  rev 9  ── ADR-0001 ──▶ PLAN-2026-0001 ──▶ v0.1.0
```

## Reading order and active documents

| Directory | Contents | Index |
|---|---|---|
| `implemented/` | [PROP-2026-0001 — Rivet runtime](implemented/prop-2026-0001-rivet-runtime.md), implemented in v0.1.0, R1–R26 | [implemented/index.md](implemented/index.md) |
| `approved/` | [PROP-2026-0002 — envelopes, globals, library, C ABI, highlighting](approved/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md), approved (ADR-0004), in PLAN-2026-0002 | [approved/index.md](approved/index.md) |
| `draft/` | Empty | [draft/index.md](draft/index.md) |

1. [Implemented proposals](implemented/index.md), then [PROP-2026-0001](implemented/prop-2026-0001-rivet-runtime.md).
2. [ADR-0001](../decisions/adr-0001-approve-rivet-runtime-design.md): the approval record.
3. [Reference samples](../references/ref-2026-0002-language-and-usage.md) (S01–S159).

## Status and maintenance

Recently updated, 2026-09-28: the maintainer approved PROP-2026-0001 (ADR-0001), closing G-DESIGN, G-CONTRACT
and G-LIC. The proposal moved from `draft/` to `approved/` (revision 7). Revision 8 added the approved TASK-005
extension (option-derived file sites, `io --needs`, `--check-files`) and the TASK-006 contract reconciliation.
Nothing is deprecated, superseded or archived. Still open: G-SPIKE, the Capy parse spike (PLAN-2026-0001
TASK-010). Update this index when a document changes lifecycle or a new related document is added.

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 9 | 2026-09-28 | Claude | PROP-2026-0002 approved (ADR-0004). |
| 8 | 2026-09-28 | Claude | PROP-2026-0002 drafted. |
| 7 | 2026-09-28 | Claude | PROP-2026-0001 implemented in v0.1.0; moved to implemented/. |
| 6 | 2026-09-28 | Claude | PROP-2026-0001 approved (ADR-0001) and moved to `approved/`; revision 8; draft/ now empty; linked approved/index.md and decisions. |
| 5 | 2026-09-28 | Claude | Status for design revision 5 (R23–R25) and the two approval gates; added lifecycle diagram. |
| 4 | 2026-09-28 | Codex | Added twelve draft sample folders with source files, fixtures, request bodies and usage READMEs (UQ-16). |
| 3 | 2026-09-28 | Codex | Added gRPC, documented multi-operation catalogs, incoming MCP tools and duplex sessions; expanded reference to 120 examples. |
| 2 | 2026-09-28 | Codex | Added UDP, OAuth 2.0, QUIC/HTTP3 design coverage, traceability and 22 examples; updated current-state navigation. |
| 1 | 2026-09-27 | Codex | Created design corpus navigation and status. |
