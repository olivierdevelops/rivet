---
document_id: REF-2026-0004
title: "Rivet documentation current state"
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
related_documents: ["PROP-2026-0001", "REF-2026-0002"]
supersedes: null
superseded_by: null
tags: [rivet, rust, capy, design]
confidentiality: internal
review_cycle: on-design-change
next_review_date: 2026-10-27
---

# Rivet documentation current state

Rivet is in design review (design revision 5). The proposed system is a Rust/Capy library and runtime with scoped protocol resources, a shared request registry, MCP bridging, file CRUD, auditable effects, policy-file restrictions and bounded DAGs. All protocol work (UDP, OAuth 2.0, QUIC/HTTP3, native gRPC in all four modes, sessions) is required scope. Revision 5 (UQ-17) adds declared, described operation outputs viewable from every surface, makes `policy.json` the only policy source (absent file = deny-by-default), and serves HTTP, SSE, polling, WebSocket and MCP from one `rivet serve` listener.

```text
  PROJECT.md (brief)
      |
      v
  REF-2026-0001  request + evidence (UQ-01 .. UQ-17)
      |
      v
  PROP-2026-0001  proposal (status: proposed)  <----->  vhco-contract.json (hand-authored)
      |                                                     |
      v                                                     v
  REF-2026-0002  numbered examples               docs/demos/  twelve sample folders
      |
      v
  [gates]  G-LIC Capy licence  +  Capy parser spike  -->  PLAN-2026-0001 (draft)  -->  v0.1.0 (not started)
```

| Area | Current state |
|---|---|
| Latest release | None; no runtime code or Cargo project |
| Active implementation | None started. [PLAN-2026-0001](plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) (draft) plans v0.1.0: approval/discovery → implementation → tests/validation → docs/demos → release; all 96 tasks NOT STARTED |
| Pending proposal | [PROP-2026-0001](proposals/draft/prop-2026-0001-rivet-runtime.md), design revision 5 |
| Current architecture | Proposed five-bucket Rust design in PROP-2026-0001; contract adds `outputs`, `policy.load_policy`, `serve` and `transports` use cases and WS/polling surfaces |
| Important decisions | None approved. User decisions recorded: Capy prefix calls, all protocol scope required, demos in `docs/demos/`, policy only via `policy.json`, one `serve` for every surface |
| Open incidents | No incident records; runtime does not exist |
| Recent research | Capy public parser API and supplied Go dispatcher inspected; [evidence](references/ref-2026-0001-request-and-evidence.md) |
| Known limitations | Examples are proposed, not parser-tested; VHCO runtime gates require future `src/` |
| Current risks | **G-LIC:** Capy LICENSE is source-available while Cargo.toml says MIT; the user owns Capy and will relicense before implementation approval (approval gate, not a design blocker). **Parser spike:** every numbered example and every `docs/demos` `.rivet` file must parse cleanly with Capy `Library::parse` before implementation. Process sandbox platform support and conditional file updates remain unproven. |
| Documentation requiring updates | Proposal/contract after human review; examples and demos after the parser spike and implementation tests. PLAN-2026-0001 lists every document still to be written (ADRs, research, TEST, validation report, release verification demo, manuals, system, API, architecture, security, operations, runbooks, onboarding, release notes) |

For concrete files, start with the [twelve sample folders](demos/README.md) ([status and reading order](demos/index.md)), each with a full bundle and a usage README. They illustrate the draft and are not executed demos.

Start with the [proposal](proposals/draft/prop-2026-0001-rivet-runtime.md), then the [numbered-example reference](references/ref-2026-0002-language-and-usage.md). The [navigation index](index.md) links every documentation directory. [Standards](standards/index.md) index the supplied rules; no new project policy was silently approved.

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 6 | 2026-09-28 | Claude | Linked PLAN-2026-0001 (v0.1.0 implementation, validation and release plan) as the active planning document. |
| 5 | 2026-09-28 | Claude | Revision 5 current state: declared outputs, policy.json-only, unified serve, prefix calls; recorded G-LIC and parser spike risks; linked demos index. |
| 4 | 2026-09-28 | Codex | Added twelve draft sample folders with source files, fixtures, request bodies and usage READMEs (UQ-16). |
| 3 | 2026-09-28 | Codex | Added gRPC, documented multi-operation catalogs, incoming MCP tools and duplex sessions; expanded reference to 120 examples. |
| 2 | 2026-09-28 | Codex | Added UDP, OAuth 2.0, QUIC/HTTP3 design coverage, traceability and 22 examples; updated current-state navigation. |
| 1 | 2026-09-27 | Codex | Recorded current design state and limitations. |
