---
document_id: REF-2026-0004
title: "Rivet documentation current state"
document_type: reference
status: active
created_date: 2026-09-27
last_updated: 2026-09-30
document_revision: 16
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, execution, cli, http, mcp, library, policy, ffi]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, embedded, server]
audience: [maintainers, developers, reviewers]
scope: Current runtime documentation and developer workflows — release 0.2.0 (current) and 0.1.0 (previous).
reason: Record the project brief and requested changes as reviewable contracts and examples.
dependencies: [PROJECT.md, DOCUMENTATION.md, AGENTS.md]
related_documents: ["PROP-2026-0001", "PROP-2026-0002", "PLAN-2026-0002", "MIG-2026-0001", "API-2026-0006", "API-2026-0007", "MAN-2026-0009", "MAN-2026-0010", "SYS-2026-0010", "SYS-2026-0011", "INC-2026-0011", "STD-2026-0001", "REF-2026-0002"]
supersedes: null
superseded_by: null
tags: [rivet, rust, capy, design]
confidentiality: internal
review_cycle: on-design-change
next_review_date: 2026-10-27
---

# Rivet documentation current state

Rivet is implemented as a Rust library and `rivet` binary. The **current release is 0.2.0** (git tag `v0.2.0`, pushed to
`origin` with `main`; release artifacts are built locally, see REL-0.2.0). The previous release is 0.1.0 (tag `v0.1.0`). The source tree contains the runtime, the C
ABI library `librivet` (`ffi/`), editor support (`editors/`), conformance tests, protocol fixtures and manuals.
Supported platforms: **macOS and Linux** (CI green on both); Windows is not supported
([INC-2026-0011](incidents/active/inc-2026-0011-windows-port-failures.md)).
The original design approval is recorded in [ADR-0001](decisions/adr-0001-approve-rivet-runtime-design.md);
[PLAN-2026-0001](plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) records implementation work.

## Build, install and verify

From the repository root with Perch on `PATH`:

```sh
perch --help
perch build        # cargo build --locked --release --features cli --bin rivet
perch install      # the same, then bman add
perch ffi          # librivet + rivet.h check + C and Python examples
perch tests
perch gates
```

Without Perch: `cargo install rivet-runtime --git https://github.com/olivierdevelops/rivet --tag v0.2.0 --features cli`
(the binary needs the `cli` feature; the `v0.2.0` tag is created in P5).

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

## In progress: 0.2.0

[PLAN-2026-0002](plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) (approved in
[ADR-0004](decisions/adr-0004-approve-envelopes-globals-library-ffi-highlighting.md)) implements
[PROP-2026-0002](proposals/implemented/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md) for v0.2.0.
P1 and P2a–P2f are done on `main` (476 tests; CI green on macOS and Linux); P4 documentation is under way; P5
releases.

```text
 PROP-2026-0002 ─► ADR-0004 ─► PLAN-2026-0002 ─► P1 ✓ ─► P2a–P2f ✓ ─► P3 ─► P4 docs (now) ─► P5 v0.2.0 ─► REL-0.2.0
```

| 0.2.0 feature | Read |
|---|---|
| One response and input envelope on every surface (**breaking**), `--pretty`, `--data`, `--input` | [API-2026-0006](api/api-2026-0006-envelopes.md), [MIG-2026-0001](migrations/mig-2026-0001-response-and-input-envelopes.md) |
| `global` constants and file modules (`import … as`, `rt.load`) | [MAN-2026-0003](manuals/man-2026-0003-language-guide.md#globals), [MAN-2026-0005](manuals/man-2026-0005-policy-and-io-manifest-guide.md#one-policy-across-modules) |
| Cargo dependency `rivet-runtime`, features, facade | [MAN-2026-0007](manuals/man-2026-0007-embedding-library.md), [API-2026-0004](api/api-2026-0004-rust-library.md), [ADR-0005](decisions/adr-0005-workspace-package-and-features.md) |
| C ABI `librivet` | [MAN-2026-0009](manuals/man-2026-0009-c-abi-and-ffi.md), [API-2026-0007](api/api-2026-0007-c-abi.md), [SYS-2026-0010](system/components/sys-2026-0010-ffi-surface-and-packaging.md) |
| Highlighting and editors | [MAN-2026-0010](manuals/man-2026-0010-editor-support-and-highlighting.md), [SYS-2026-0011](system/components/sys-2026-0011-highlighting-and-grammar-generation.md) |
| Everything new, with demos | [What's New in 0.2.0](manuals/man-2026-0001-rivet-manual.md#whats-new-in-020) |

INC-2026-0009 (numeric index paths) and INC-2026-0010 (ffi rlib collision) are resolved; INC-2026-0011 (Windows
port failures) is active and records why Windows is not supported.

## Open risks and limitations

- Platforms: macOS and Linux are supported and green in CI; Windows is not supported
  ([INC-2026-0011](incidents/active/inc-2026-0011-windows-port-failures.md)).
- Linux process sandbox gated until verified on kernel ≥ 6.12 ([ADR-0003](decisions/adr-0003-process-sandbox-backends.md)):
  sandboxed spawns are refused with `unsupported.sandbox_backend` (exit 5 / HTTP 501).
- 0.2.0 output is a breaking change for 0.1.0 clients ([MIG-2026-0001](migrations/mig-2026-0001-response-and-input-envelopes.md));
  the `id`/`params` inputs are deprecated and removed in 0.3.0.
- Not supported: mTLS serve, `finally`, Stage C adapters, URL imports, a persistent trace store (0.1.0 also had no
  `import`; 0.2.0 adds file modules). See the [manual's Known Limitations](manuals/man-2026-0001-rivet-manual.md#known-limitations).

## Current documentation

| Area | Source of truth |
|---|---|
| Runtime and usage | [Rivet manual](manuals/man-2026-0001-rivet-manual.md), [CLI reference](manuals/man-2026-0004-cli-reference.md), [manuals index](manuals/index.md) (ten volumes) |
| Wire contracts | [API index](api/index.md): envelopes (API-2026-0006), HTTP, WebSocket, MCP, Rust, C ABI (API-2026-0007), error registry |
| Migrations | [Migrations index](migrations/index.md): [MIG-2026-0001](migrations/mig-2026-0001-response-and-input-envelopes.md) (0.1.0 → 0.2.0) |
| Architecture | Five Rust code buckets, 14 features and seven surfaces (`cli`, `http`, `library`, `mcp`, `ws`, `poll`, `ffi`) in the [hand-authored contract](../vhco-contract.json); workspace crates `rivet-runtime` and `rivet-ffi` |
| Developer workflow | [Contributor setup](onboarding/onb-2026-0001-contributor-setup.md), [AGENTS.md](../AGENTS.md) |
| Code review standards | [Orchestrator and cross-package implementation standard](standards/std-2026-0001-orchestrator-and-cross-package-review.md) (STD-2026-0001) |
| Design and decisions | [Implemented proposal](proposals/implemented/prop-2026-0001-rivet-runtime.md), [decisions index](decisions/index.md) |
| Test evidence | [Testing index](testing/index.md), [validation report](reports/rpt-2026-0001-validation-of-plan-2026-0001.md) |
| Protocol and sandbox limits | [Protocols and connectors](manuals/man-2026-0008-protocols-and-connectors.md), [ADR-0003](decisions/adr-0003-process-sandbox-backends.md) |
| Examples | [Sample folders](demos/README.md) (twelve verified release demos plus the 13-real-world-apis cookbook; 0.2.0 adds `14-globals`, `15-ffi`, `16-editor`, `17-modules`, being written), [numbered-example reference](references/ref-2026-0002-language-and-usage.md); fixture prerequisites are documented per demo |
| Incidents | [Incident index](incidents/index.md) |

The [navigation index](index.md) links the documentation directories. Historical design examples and recorded
validation results describe their stated versions; use the current manuals and command help for the source
tree. Platform support beyond the locally verified environment requires the corresponding CI evidence.

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 16 | 2026-09-30 | Claude | v0.2.0 release (PLAN-2026-0002 TASK-091): version strings and current-release wording updated to 0.2.0. |
| 15 | 2026-09-29 | Claude | TASK-080 / D-49, D-66: current release 0.1.0 (remote `origin`, no published artifacts) with 0.2.0 in progress; 0.2.0 feature table linking API-0006/0007, MIG-0001, MAN-0009/0010, SYS-0010/0011, globals/modules, facade; Perch `ffi` and the Cargo install with `--features cli`; platforms macOS and Linux, Windows unsupported (INC-2026-0011); seven surfaces and two crates; STD-2026-0001 and Perch content kept. |
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
