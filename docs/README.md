---
document_id: REF-2026-0004
title: "Rivet documentation current state"
document_type: reference
status: active
created_date: 2026-09-27
last_updated: 2026-09-28
document_revision: 14
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, execution, cli, http, mcp, library, policy]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, embedded, server]
audience: [maintainers, developers, reviewers]
scope: Current runtime documentation and developer workflows for the unreleased source tree.
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

Rivet is implemented as a Rust library and `rivet` binary, currently version **0.1.0** (git tag `v0.1.0`, the first release). The source tree
contains the runtime, conformance tests, protocol fixtures and manuals. The release is local: no git remote is
configured yet, so nothing has been pushed or published.
The original design approval is recorded in [ADR-0001](decisions/adr-0001-approve-rivet-runtime-design.md);
[PLAN-2026-0001](plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) records implementation work.

## Build, install and verify

From the repository root with Perch on `PATH`:

```sh
perch --help
perch build
perch install
perch tests
perch gates
```

The approved [`commands.perch`](../commands.perch) wraps Cargo, Python documentation checks and VHCO.
`build` produces the release binary; `install` builds into this checkout's `target/`, then runs
`bman add "<absolute release-binary path>"` to install into the global bin directory managed by bman.
`gates` stops at the first failure in formatting, type-checking, Clippy, Rust tests, docs and architecture.
The [installation guide](manuals/man-2026-0002-installation-and-quickstart.md) covers setup and `PATH`;
the [contributor guide](onboarding/onb-2026-0001-contributor-setup.md#perch-development-commands) lists all tasks.

```text
commands.perch -> Cargo build/install/test/lint
              -> Python documentation checks
              -> VHCO validate/sync/check
                         |
                         v
                README + manuals + contributor guide
```

## Current release

| Release | Plan | Validation | Verification guide | Status |
|---|---|---|---|---|
| [REL-0.1.0](releases/rel-0.1.0-release-notes.md) | [PLAN-2026-0001](plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) | [RPT-2026-0001](reports/rpt-2026-0001-validation-of-plan-2026-0001.md): 21 PASS, 5 PARTIAL, 0 FAIL | [DEMO-2026-0015](demos/demo-2026-0015-v0-1-0-release-verification.md) | released (tag `v0.1.0`) |

```text
 PROP-2026-0001 (implemented) ─► PLAN-2026-0001 ─► v0.1.0 ─► REL-0.1.0
                                                     │
                     8 incidents, all resolved ◄─────┤
      21 PASS / 5 PARTIAL (Linux+Windows pending) ◄──┘
```

## In progress

[PLAN-2026-0002](plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) (approved in
[ADR-0004](decisions/adr-0004-approve-envelopes-globals-library-ffi-highlighting.md)) implements
[PROP-2026-0002](proposals/approved/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md) for v0.2.0:
standard input/output envelopes (a breaking change), pretty JSON, `global` constants, Rivet as a Cargo
dependency, a C ABI library and syntax highlighting. P1 (contract delta, RES-2026-0004, ADR-0005), P2a
(the envelope wire change), P2b (`global` constants), P2f (file modules) and P2e (syntax highlighting:
`rivet highlight`, the generated TextMate grammar and VS Code extension under `editors/`) are done on `main`, not
yet released; the manuals and API documents still describe 0.1.0 until P4 (TASK-070). INC-2026-0009 (numeric
index paths) is resolved; there is no open incident.

## Open risks and limitations

- Linux and Windows are not validated: CI needs a git remote (T-01, T-08, T-27 PARTIAL).
- Linux process sandbox gated until verified on kernel ≥ 6.12 ([ADR-0003](decisions/adr-0003-process-sandbox-backends.md)); Windows unsupported.
- Not in 0.1.0: mTLS serve, `import`, `finally`, Stage C adapters, persistent trace store. See the
  [manual's Known Limitations](manuals/man-2026-0001-rivet-manual.md).

## Current documentation

| Area | Source of truth |
|---|---|
| Runtime and usage | [Rivet manual](manuals/man-2026-0001-rivet-manual.md), [CLI reference](manuals/man-2026-0004-cli-reference.md) |
| Architecture | Five Rust code buckets, 14 features and six surfaces in the [hand-authored contract](../vhco-contract.json) |
| Developer workflow | [Contributor setup](onboarding/onb-2026-0001-contributor-setup.md), [AGENTS.md](../AGENTS.md) |
| Code review standards | [Orchestrator and cross-package implementation standard](standards/std-2026-0001-orchestrator-and-cross-package-review.md) |
| Design and decisions | [Implemented proposal](proposals/implemented/prop-2026-0001-rivet-runtime.md), [decisions index](decisions/index.md) |
| Test evidence | [Testing index](testing/index.md), [validation report](reports/rpt-2026-0001-validation-of-plan-2026-0001.md) |
| Protocol and sandbox limits | [Protocols and connectors](manuals/man-2026-0008-protocols-and-connectors.md), [ADR-0003](decisions/adr-0003-process-sandbox-backends.md) |
| Examples | [Sample folders](demos/README.md) (twelve verified release demos plus the 13-real-world-apis cookbook), [numbered-example reference](references/ref-2026-0002-language-and-usage.md); fixture prerequisites are documented per demo |
| Incidents | [Incident index](incidents/index.md) |

The [navigation index](index.md) links the documentation directories. Historical design examples and recorded
validation results describe their stated versions; use the current manuals and command help for the source
tree. Platform support beyond the locally verified environment requires the corresponding CI evidence.

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 14 | 2026-09-29 | Codex | Added the orchestrator and cross-package implementation standard to current documentation. |
| 13 | 2026-09-28 | Claude | PLAN-2026-0002 P2b, P2f and P2e done (globals, file modules, highlighting); INC-2026-0009 resolved. |
| 12 | 2026-09-28 | Claude | PLAN-2026-0002 P1 and P2a done (envelopes on `main`; docs follow in P4). |
| 11 | 2026-09-28 | Claude | Linked PLAN-2026-0002 (v0.2.0 in progress). |
| 10 | 2026-09-28 | Claude | TASK-070: v0.1.0 release state — current release table, open risks and limitations, proposal implemented. |
| 9 | 2026-09-28 | Codex | Changed Perch installation to a release build followed by bman add, as requested by the maintainer. |
| 8 | 2026-09-28 | Codex | Replaced obsolete pre-implementation status with runtime/manual navigation and approved Perch development workflows. |
| 7 | 2026-09-28 | Claude | Current state after ADR-0001: design approved (revision 8), contract approved, G-LIC closed, Git adopted, PLAN-2026-0001 in P1; linked decisions and approved-proposal indexes. |
| 6 | 2026-09-28 | Claude | Linked PLAN-2026-0001 (v0.1.0 implementation, validation and release plan) as the active planning document. |
| 5 | 2026-09-28 | Claude | Revision 5 current state: declared outputs, policy.json-only, unified serve, prefix calls; recorded G-LIC and parser spike risks; linked demos index. |
| 4 | 2026-09-28 | Codex | Added twelve draft sample folders with source files, fixtures, request bodies and usage READMEs (UQ-16). |
| 3 | 2026-09-28 | Codex | Added gRPC, documented multi-operation catalogs, incoming MCP tools and duplex sessions; expanded reference to 120 examples. |
| 2 | 2026-09-28 | Codex | Added UDP, OAuth 2.0, QUIC/HTTP3 design coverage, traceability and 22 examples; updated current-state navigation. |
| 1 | 2026-09-27 | Codex | Recorded current design state and limitations. |
