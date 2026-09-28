---
document_id: REF-2026-0004
title: "Rivet documentation current state"
document_type: reference
status: draft
created_date: 2026-09-27
last_updated: 2026-09-28
document_revision: 7
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
related_documents: ["PROP-2026-0001", "REF-2026-0002"]
supersedes: null
superseded_by: null
tags: [rivet, rust, capy, design]
confidentiality: internal
review_cycle: on-design-change
next_review_date: 2026-10-27
---

# Rivet documentation current state

Rivet's design is **approved** and implementation has not started. On 2026-09-28 the maintainer approved
[PROP-2026-0001](proposals/approved/prop-2026-0001-rivet-runtime.md), the hand-authored `vhco-contract.json` and
[PLAN-2026-0001](plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md), recorded in
[ADR-0001](decisions/adr-0001-approve-rivet-runtime-design.md). The repository is under Git on branch `main`.
PLAN-2026-0001 is executing phase **P1** (approval and discovery).

The system is a Rust/Capy library and runtime with scoped protocol resources, one described operation catalog
served on every surface (CLI, REST/SSE, polling, WebSocket, MCP, Rust library), `policy.json`-only policy,
auditable effects and a generated I/O manifest. All protocol work (UDP, OAuth 2.0, QUIC/HTTP3, native gRPC in
all four modes, sessions) is required scope. Design revision 8 adds option-derived file sites (`tls ca_file`,
`tls cert_file`, `tls key_file`, `body file`), per-site `origin`, `phase`, `requires_existing` and `secret`, and
`rivet io --needs` / `--check-files`.

```text
  PROJECT.md (brief)
      |
      v
  REF-2026-0001  request + evidence (UQ-01 .. UQ-18)
      |
      v
  PROP-2026-0001  proposal rev 8 (status: approved)  <----->  vhco-contract.json (approved)
      |                 |                                          |
      |            ADR-0001 (approval, G-LIC closed)               v
      v                 |                                  docs/demos/  twelve sample folders
  REF-2026-0002         v
  S01-S159        PLAN-2026-0001  P1 in execution ──[G-SPIKE: Capy parse spike]──▶ P2 implementation ──▶ v0.1.0
```

| Area | Current state |
|---|---|
| Latest release | None; no runtime code or Cargo project yet |
| Active implementation | [PLAN-2026-0001](plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) phase P1: approval, the Capy parse spike (G-SPIKE, TASK-010), research spikes and ADR-0002/0003. P2 implementation starts at P1 exit |
| Approved proposal | [PROP-2026-0001](proposals/approved/prop-2026-0001-rivet-runtime.md), design revision 8 ([approved proposals](proposals/approved/index.md)); no proposal is pending in [draft/](proposals/draft/index.md) |
| Current architecture | Five-bucket Rust design in PROP-2026-0001; the contract has 14 features (`language`, `registry`, `execution`, `files`, `connectors`, `audit`, `policy`, `auth`, `datagrams`, `quic`, `grpc`, `sessions`, `serve`, `transports`) and six surfaces (`cli`, `http`, `library`, `mcp`, `ws`, `poll`) |
| Important decisions | [ADR-0001](decisions/adr-0001-approve-rivet-runtime-design.md): design, contract and plan approved; G-DESIGN, G-CONTRACT and G-LIC closed; TASK-005 approved; Git adopted. ADR-0002 (crates) and ADR-0003 (sandbox backends) are pending ([decisions index](decisions/index.md)) |
| Open incidents | No incident records; runtime does not exist |
| Recent research | Capy public parser API and supplied Go dispatcher inspected ([evidence](references/ref-2026-0001-request-and-evidence.md)); the Capy grammar spike is P1 work |
| Known limitations | Examples are proposed, not parser-tested; VHCO runtime gates require future `src/`; Stage C adapters (pipes, file watching, mTLS TCP, codecs, reconnect) are outside v0.1.0 |
| Current risks | **G-SPIKE:** every numbered example and every `docs/demos` `.rivet` file must parse cleanly with Capy `Library::parse` before implementation; a failure sends grammar changes back through the contract and the proposal. Process sandbox platform support (ADR-0003) and conditional file updates remain unproven. G-LIC is closed: the maintainer owns Capy |
| Documentation requiring updates | Examples and demos after the parse spike and implementation tests. PLAN-2026-0001 lists every document still to be written (ADR-0002/0003, research, TEST, validation report, release verification demo, manuals, system, API, architecture, security, operations, runbooks, onboarding, release notes) |

For concrete files, start with the [twelve sample folders](demos/README.md) ([status and reading order](demos/index.md)), each with a full bundle and a usage README. They illustrate the draft and are not executed demos.

Start with the [proposal](proposals/approved/prop-2026-0001-rivet-runtime.md), then the [numbered-example reference](references/ref-2026-0002-language-and-usage.md). The [navigation index](index.md) links every documentation directory. [Standards](standards/index.md) index the supplied rules; no new project policy was silently approved.

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 7 | 2026-09-28 | Claude | Current state after ADR-0001: design approved (revision 8), contract approved, G-LIC closed, Git adopted, PLAN-2026-0001 in P1; linked decisions and approved-proposal indexes. |
| 6 | 2026-09-28 | Claude | Linked PLAN-2026-0001 (v0.1.0 implementation, validation and release plan) as the active planning document. |
| 5 | 2026-09-28 | Claude | Revision 5 current state: declared outputs, policy.json-only, unified serve, prefix calls; recorded G-LIC and parser spike risks; linked demos index. |
| 4 | 2026-09-28 | Codex | Added twelve draft sample folders with source files, fixtures, request bodies and usage READMEs (UQ-16). |
| 3 | 2026-09-28 | Codex | Added gRPC, documented multi-operation catalogs, incoming MCP tools and duplex sessions; expanded reference to 120 examples. |
| 2 | 2026-09-28 | Codex | Added UDP, OAuth 2.0, QUIC/HTTP3 design coverage, traceability and 22 examples; updated current-state navigation. |
| 1 | 2026-09-27 | Codex | Recorded current design state and limitations. |
