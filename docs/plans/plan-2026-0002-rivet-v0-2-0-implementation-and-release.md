---
document_id: PLAN-2026-0002
title: "Rivet v0.2.0 implementation, validation and release"
document_type: plan
status: approved
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 7
start_date: 2026-09-28
target_date: null           # not estimated; scope fixed by PROP-2026-0002 (ADR-0004)
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, registry, execution, audit, serve, cli, http, ws, poll, mcp, library]
affected_versions:
  from: "0.1.0"
  to: "0.2.0"
applicable_environments: [development, embedded, server]
audience: [maintainers, implementers, reviewers]
scope: >-
  Everything required to implement PROP-2026-0002 revision 3 (R1–R24, UC-01–UC-11, C-01–C-15), test and validate it, create
  and update every user, operator, system, API, migration and release document it affects, and release v0.2.0.
reason: The maintainer approved PROP-2026-0002 and asked for a plan, then implementation, tests and release, with
  every documentation deliverable included ("ensure plan contains all docs documentation to add", 2026-09-28).
dependencies: [PROP-2026-0002, ADR-0004, PLAN-2026-0001, REL-0.1.0, vhco-contract.json, AGENTS.md, DOCUMENTATION.md]
related_documents: [PROP-2026-0002, ADR-0004, REL-0.1.0, RPT-2026-0001, DEMO-2026-0015]
supersedes: null
superseded_by: null
tags: [rivet, plan, v0.2.0, envelope, globals, library, ffi, highlighting, modules, documentation]
confidentiality: internal
review_cycle: on-change
next_review_date: 2026-10-28
---

# Rivet v0.2.0 implementation, validation and release

> **Status:** Approved
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0 → 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** language, registry, execution, audit, serve, cli, http, ws, poll, mcp, library

## Summary

This is the live execution ledger for turning the approved
[PROP-2026-0002](../proposals/approved/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md)
([ADR-0004](../decisions/adr-0004-approve-envelopes-globals-library-ffi-highlighting.md)) into Rivet **v0.2.0**.
It has five gated phases, like PLAN-2026-0001:

1. contract and experiments;
2. implementation, in six increments (P2f file modules was added by PROP-2026-0002 revision 3 / ADR-0004 revision 2);
3. tests and validation;
4. documentation and demos;
5. release.

Every requirement traces to tasks, files, tests, documents and a release-verification row. The
[Documentation and Demo Checklist](#documentation-and-demo-checklist) lists **every** document to create or update
(D-01…D-72), including each of the 62 current-state files whose examples change with the new envelope.

```text
 PROP-2026-0002 ──► ADR-0004 ──► PLAN-2026-0002 (this) ──► P1 contract delta + RES-2026-0004 experiments
                                                               │
        ┌──────────────────────────────┬───────────────────────┼───────────────────────┬──────────────────────┐
        ▼                              ▼                       ▼                       ▼                      ▼
  P2a envelopes + input          P2b globals ─► P2f     P2c facade + features   P2d C ABI (librivet)   P2e highlighting
  + pretty (R1–R6, breaking)     (R7–R9)  modules       + rivet-runtime (R10–12) (R13–R15, R23)         (R16–R17)
                                          (R19–R24)
        └──────────────────────────────┴───────────────────────┴───────────────────────┴──────────────────────┘
                                                               │
                             P3 tests: TEST-2026-0034…0053 + regression re-run ─► RPT-2026-0015
                                                               │
     P4 docs: MIG-2026-0001 · API-0006/0007 · MAN-0009/0010 · SYS-0010/0011 · 62 files re-shaped · demos 14–16 · DEMO-2026-0020
                                                               │
                         P5 Cargo.toml 0.2.0 ─► release: v0.2.0 ─► tag v0.2.0 ─► REL-0.2.0 ─► §34 gate
```

## Objective, Scope and Proposal Baseline

**Objective.** Ship Rivet v0.2.0 with:
- one output envelope and one input envelope on every surface;
- opt-in pretty JSON;
- immutable `global` constants;
- Rivet usable as a Cargo dependency (package `rivet-runtime`, lib `rivet`, Cargo features, the `cli` feature);
- a C ABI shared and static library (`librivet`, `rivet.h`);
- syntax highlighting (a TextMate grammar, a VS Code extension and `rivet highlight`);
- file modules: `import "PATH" as ALIAS [public]` in `.rivet`, and `Runtime::load` / `rivet_load` module objects
  (namespaced `ALIAS.ID`, loader's policy only).

Everything ships with the current-state documentation set required by DOCUMENTATION.md.

**Proposal baseline.** PROP-2026-0002 revision 3 (approved 2026-09-28; the revision 3 modules amendment was approved in ADR-0004 revision 2), with Q-01…Q-06 resolved per its
recommendations (ADR-0004).

| Owned by this plan | IDs |
|---|---|
| Goals | G-01 – G-08 |
| Requirements | R1 – R24 |
| Use cases | UC-01 – UC-11 |
| Proposed changes | C-01 – C-15 |
| Proposal file rows | F-01 – F-24 (mapped to PF-IDs below) |
| Proposal tests | T-01 – T-20 (same IDs); plan-level checks T-27 – T-34 |

**Not owned by this plan** (per ADR-0004):
- crates.io publication (G-PUB, the owner's decision);
- tree-sitter (Q-02), JSON colourization (Q-03) and a `rivet globals` command (Q-06);
- client-supplied `request_id` (Q-01);
- Marketplace publishing;
- the v0.1.0 remote-dependent tasks, which stay with PLAN-2026-0001 (TASK-051/078/080/081).

```text
            v0.2.0 scope
 ┌───────────────────────────────────────────────────────────────────────────────┐
 │ wire      ResponseEnvelope · InputEnvelope · pretty · deprecation aliases      │
 │ language  global NAME = EXPR (immutable, load-time) · exact manifest targets   │
 │ packaging rivet-runtime (lib rivet, bin rivet[cli]) · features · facade        │
 │ C ABI     rivet-ffi → librivet .so/.dylib/.dll + .a/.lib · rivet.h · rivet.pc  │
 │ editors   keywords.json → rivet.tmLanguage.json → VS Code .vsix · highlight    │
 │ modules   import "PATH" as ALIAS [public] · rt.load(path) · rivet_load          │
 └───────────────────────────────────────────────────────────────────────────────┘
   outside: crates.io (G-PUB) · tree-sitter · JSON colour · URL imports · hot reload · run-from-path · push/CI
```

## Live Status Summary

| State | Count | Notes |
|---|---:|---|
| NOT STARTED | 23 | P3–P5 |
| IN PROGRESS | 0 | |
| BLOCKED | 2 | TASK-095/096 now unblocked by the remote (origin added 2026-09-28); updated at P5 |
| DONE | 48 | P1, P2a–P2f |
| FAILED | 0 | |
| DEFERRED | 1 | TASK-038 crates.io publish (G-PUB, owner) |

- **Current phase:** P1 and P2a–P2f done (476 tests, `vhco sync` 0); P3 next.
- **Next action:** P3 full test and validation (TASK-060…064), including the first CI run of the `features` matrix, the header check and `conformance_ffi` on Linux (the Linux static `Libs.private` list is unverified); no open incident.
- **Remote:** `origin` = https://github.com/olivierdevelops/rivet.git (added 2026-09-28); `main` and `v0.1.0` pushed; CI runs on push.
- **Last updated:** 2026-09-29.
- **Release target:** v0.2.0.

## Requirements and Use Cases

| Req / UC | Why it exists | Planned outcome (how it is satisfied) | Acceptance criteria | Owning phase | Status |
|---|---|---|---|---|---|
| R1 / UC-01, UC-02 | UQ-03/05: one output shape | `ResponseEnvelope` serializer used by every surface | T-01 schema over CLI, HTTP, SSE, NDJSON, polling, WS, MCP, library, FFI | P2a | DONE (P2a; FFI part in P2d) |
| R2 / UC-01 | UQ-05: status and error semantics | `status` ok/error/cancelled/accepted; `type` result/data; `effects` at the top level | T-01 over every API-2026-0005 code | P2a | DONE (P2a) |
| R3 / UC-01 | UQ-03: every JSON output | Built-ins, GET routes and CLI `--json` outputs enveloped | T-03 | P2a | DONE (P2a) |
| R4 / UC-01 | UQ-06: one input shape | `InputEnvelope::parse` shared by every surface; `--data`, `--input` | T-02 | P2a | DONE (P2a; FFI part in P2d) |
| R5 / UC-09 | Compatibility for 0.1.0 clients | `id`/`params` aliases in 0.2.x with deprecation signals; mixed keys refused | T-15 | P2a | DONE (P2a) |
| R6 / UC-03 | UQ-04: readable JSON | `--pretty`, `?pretty=true`, `to_json_pretty`, FFI `pretty`; refused on NDJSON/SSE | T-05 | P2a | DONE (P2a; FFI part in P2d) |
| R7 / UC-04 | UQ-07: reuse values | `global NAME = EXPR`, load-time, read-only | T-06 | P2b | DONE (P2b; docs P4) |
| R8 / UC-04 | UQ-07: safe globals | `syntax.global`, `check.global_*` codes with spans | T-07 | P2b | DONE (P2b; docs P4) |
| R9 / UC-04 | Manifest precision | Global substitution in the manifest and call graph | T-08 | P2b | DONE (P2b; see the R9 deviation; docs P4) |
| R10 / UC-05 | UQ-02: Cargo dependency | `rivet-runtime` package, `rivet` facade, hidden internals | T-09 | P2c | DONE (P2c; docs P4) |
| R11 / UC-05 | UQ-02: lean builds | Features `serve,grpc,quic,oauth,cli`; `unsupported.feature` | T-10 | P2c | DONE (P2c; docs P4) |
| R12 / UC-05 | UQ-02: publication path | Git dependency on tag `v0.2.0`; crates.io prepared, gated by G-PUB | T-10 (packaging list) | P2c | DONE (P2c: packaging prepared; publication DEFERRED to G-PUB; docs P4) |
| R13 / UC-06 | UQ-08: C ABI | `rivet-ffi` cdylib + staticlib, `rivet.h`, `rivet.pc`, `rivet_abi_version` | T-11 | P2d | DONE (P2d; docs P4) |
| R14 / UC-06 | FFI safety | Panic, null, UTF-8 and JSON guards; ownership rules; thread safety | T-12 | P2d | DONE (P2d; docs P4) |
| R15 / UC-06 | FFI streams | Pull call handle: start, next, send, finish_input, cancel, free | T-11 | P2d | DONE (P2d; docs P4) |
| R16 / UC-07 | UQ-01: editors | Generated TextMate grammar; VS Code extension `.vsix` | T-13 | P2e | DONE (P2e; docs P4) |
| R17 / UC-08 | UQ-01: terminal/HTML | `rivet highlight`, `rivet::highlight::tokens` | T-14 | P2e | DONE (P2e; FFI `rivet_highlight` in P2d; docs P4) |
| R18 / all | DOCUMENTATION §§29–31 | Every document in the Documentation and Demo Checklist | T-30, T-31, T-32 | P4 | NOT STARTED |
| R19 / UC-10 | UQ-09: files as modules | `import "PATH" as ALIAS [public]`; root-confined bootstrap reads | T-16 | P2f | DONE (P2f; docs P4) |
| R20 / UC-10 | UQ-09: namespaced by alias | `ALIAS.ID`; internal by default, `public` to expose; transitive namespacing | T-16 | P2f | DONE (P2f; docs P4) |
| R21 / UC-10 | Safe imports | `syntax.import`, `not_found.import`, `permission.import_outside_root`, `check.import_cycle`, `check.import_duplicate`, `check.import_collision`, `limit.imports` | T-17 | P2f | DONE (P2f; docs P4) |
| R22 / UC-11 | UQ-09: host object (Rust) | `Runtime::load/load_as` → `Module`; builder without an entry file; catalog snapshot swap | T-18 | P2f | DONE (P2f; Module::stream/duplex take a Scope; docs P4) |
| R23 / UC-11 | UQ-09: host object (C/Python) | `rivet_load`, `rivet_module_*`; Python wrapper example | T-19 | P2d | DONE (P2d; docs P4) |
| R24 / UC-10, UC-11 | UQ-09: loader's policy only | One policy; module `policy.json` ignored with a warning; manifest/graph/explain/generate cover modules | T-20 | P2f | DONE (P2f; docs P4) |

## Applicable Project Standards

| Rule | Source | Plan impact | Validation |
|---|---|---|---|
| Contract before code | AGENTS golden rule 1; ADR-0004 G-CONTRACT | TASK-004 applies the proposal's Contract Delta before any P2 code | `vhco sync .` lists only expected gaps until they are implemented |
| Five buckets, pure use cases | AGENTS | `compile_globals`, `highlight_source`, `parse_input` are use cases; envelope shaping is a pure domain function; `setup_ffi.rs` is the `ffi` surface | `vhco validate .` (T-28) |
| Annotate everything | AGENTS rules 6–7 | `vhco:` annotations on every new file; contract todos claimed | `vhco sync .` = 0 at P3 |
| Test each use case | AGENTS rule 8 | New conformance suites + colocated tests | T-01…T-15 |
| README and docs current | AGENTS rule 9; DOCUMENTATION §§30–31 | P4 checklist, including the 62-file envelope sweep | T-31 |
| Live ledger | DOCUMENTATION §4.5 | Seven status values; evidence on every DONE/FAILED/BLOCKED/DEFERRED | Phase exits |
| Incidents and troubleshooting | DOCUMENTATION §§25–26 | Every unexpected defect → INC-2026-0009+; reusable problems → TRBL-2026-0004+ | T-31 |
| Tests and validation recorded | DOCUMENTATION §§27–28 | TEST-2026-0034…0053 (new) + regression re-records; RPT-2026-0015 | T-32 |
| Release verification guide | DOCUMENTATION §29 | DEMO-2026-0020 + demos 01–16 executed | T-30 |
| Migrations documented | DOCUMENTATION §4.21 | MIG-2026-0001 with a rollback plan | T-31 |
| One version source, tag = commit | DOCUMENTATION §32 | Both packages share one version (workspace `version`); `check_version.py` also checks `rivet_version()` | T-33 |
| Release completion gate | DOCUMENTATION §34 | Walked in REL-0.2.0 | TASK-094 |
| Lots of ASCII visuals | CLAUDE.md | Every new manual, system, API and migration document has journey/sequence/state visuals | T-31 review |

## Measurable Claims

PROP-2026-0002 makes **no performance claim**. It states a size cost (the new envelope keys add about 60–80 bytes
per response), which is not a claim to measure. The feature split is expected to shrink `--no-default-features`
builds; this is recorded as an observation in RES-2026-0004, not an acceptance threshold.

| Measurement | Baseline method | Test | Controlled environment | Acceptance threshold | Report destination |
|---|---|---|---|---|---|
| None (no optimization claim) | — | — | — | — | — |

## Prerequisites

| # | Prerequisite | Why | Task |
|---|---|---|---|
| 1 | G-DESIGN approved | No work before approval | TASK-001 (DONE, ADR-0004) |
| 2 | G-CONTRACT: contract delta applied | AGENTS contract-first | TASK-004 |
| 3 | Disk space ≥ 15 GiB free | Release + debug + FFI builds; 4.3 GiB free at start | TASK-008 (`cargo clean`, sequential builds) |
| 4 | Toolchain: Rust 1.90, `cbindgen` (cargo install), Node.js + `npx` (dev only: `vsce`, `vscode-tmgrammar-test`), a C compiler, Python 3 | FFI, editors, tests | TASK-008 |
| 5 | v0.1.0 tag exists | Regression baseline and migration "before" | Done (f69b5b9) |

## Phases, Entry Gates and Exit Gates

```text
 P1 contract + experiments ─► P2a envelopes ─► P2c facade/features ─► P2d C ABI ─┐
                              P2b globals ─► P2f modules ──────────────▲──────────┼─► P3 test+validate ─► P4 docs+demos ─► P5 release
                              P2e highlighting ───────────────────────────────────┘
 Sequential execution by agents in the main checkout (disk); each increment ends green and committed.
```

| Phase | Purpose | Entry criteria | Exit criteria | Depends on | Status |
|---|---|---|---|---|---|
| P1 | Contract delta; experiments E1–E3; disk and tooling | This plan approved | Contract updated; RES-2026-0004 written; ADR-0005 recorded; ≥ 15 GiB free | — | DONE |
| P2a | Envelopes, input, pretty (breaking) | P1 exit | T-01–T-05, T-15 green; all 0.1.0 suites updated and green | P1 | DONE |
| P2b | Globals | P1 exit | T-06–T-08 green | P1 | DONE (`673994c`) |
| P2c | Package rename, facade, features | P2a exit | T-09, T-10 green; feature matrix builds | P2a | DONE (`10c8130`; T-09, T-10 PASS; feature matrix builds) |
| P2d | C ABI (incl. module handles, R23) | P2c and P2f exit | T-11, T-12 green (shared and static, macOS; Linux in CI when available) | P2c | DONE (`f9af92a`; T-11, T-12, T-19 PASS on macOS; Linux via CI) |
| P2e | Highlighting | P1 exit | T-13, T-14 green | P1 | DONE (`9ec2a39`, `0d6c2ea`; T-13, T-14 PASS) |
| P2f | File modules (import + host load) | P2b exit (global scoping per module) | T-16–T-18, T-20 green | P2b | DONE (`2942066`) |
| P3 | Full test and validation | P2a–P2f exit | T-01–T-20 + T-27–T-29, T-34 recorded; `vhco sync` 0; RPT-2026-0015 | P2* | NOT STARTED |
| P4 | Documentation and demos | P3 exit | Every D-row DONE or NOT APPLICABLE with a reason; T-30, T-31 | P3 | NOT STARTED |
| P5 | Version, release commit, tag, REL | P4 exit | §34 gate walked; T-33 | P4 | NOT STARTED |

## Live Work Checklist

Owners: **M** = project maintainer; **I** = implementer (agent). Each P2 task follows the AGENTS loop: claim the
contract todo, write the code, add a colocated or conformance test, then run `vhco validate .` and `vhco sync .`.

### P1 — Contract and experiments

| Task | Phase | Description / method | Req / UC / Change IDs | Production files | Test files / manual procedure | Dependencies | Owner | Status | Evidence / result |
|---|---|---|---|---|---|---|---|---|---|
| TASK-001 | P1 | Record approval: ADR-0004; move PROP-2026-0002 to `approved/` | all | `docs/decisions/adr-0004-…md`, proposal | review | — | M/I | DONE | ADR-0004 (2026-09-28) |
| TASK-002 | P1 | Create this plan; link it from `docs/plans/index.md`, `docs/README.md`, `docs/index.md` | R18 | this file, indexes | T-31 | TASK-001 | I | DONE | PLAN-2026-0002 rev 1 |
| TASK-003 | P1 | Update proposal/decision indexes for the approval | R18 | `docs/proposals/**/index.md`, `docs/decisions/index.md` | T-31 | TASK-001 | I | DONE | indexes updated |
| TASK-004 | P1 | Apply the proposal's Contract Delta to `vhco-contract.json` by hand. Domain: `ResponseEnvelope`, `InputEnvelope`, `OutputFormat`, `EnvelopeStatus`, `RecordType`, `GlobalDecl`, `GlobalScope`, `HighlightToken`, `HighlightFormat`, `FfiOptions`; `CapabilityReport.features`, `abi_version`. Use cases: `language.compile_globals`, `language.highlight_source`, `serve.parse_input`. Surface `ffi`. CLI triggers `highlight`, `--data/--input/--pretty`. Keep reviewed wording | all / C-01–C-13 | `vhco-contract.json` | `vhco sync .` shows only unimplemented pieces | TASK-001 | I | DONE | `ef8e053`: hand-edited delta (13 domain entries incl. the P2b/P2c/P2d/P2e ones, Completion/DataEvent `operation`, WsFrame `input`/`record`; use cases serve.parse_input, language.compile_globals, language.highlight_source; surface `ffi`; flow triggers renamed). Flow handling for serve.parse_input/project_polling aligned in `c3b5565`. `vhco sync .` = 12 gaps, all future phases (see findings) |
| TASK-005 | P1 | E1: a Cargo workspace (root package `rivet-runtime` + `ffi/`) is accepted by `vhco validate/sync/check`; if not, record the fallback (shims behind an `ffi` feature in the main crate) | R13 / C-10 | `Cargo.toml` (scratch branch) | `vhco validate .` | TASK-004 | I | DONE | `3e4560f`: PASS. vhco 1.6.0 validate/sync/check green on a workspace (`rivet-runtime` + `ffi/`); vhco reads only `src/`, so `ffi` annotations must live in `orchestrator/setup_ffi.rs`; fallback not needed (RES-2026-0004 E1) |
| TASK-006 | P1 | E2: `#[no_mangle] extern "C"` shims in `rivet-ffi` export from both `cdylib` and `staticlib`; `nm -gU` lists only `rivet_*`; a C program links statically on macOS; capture `--print native-static-libs` | R13 / C-10 | scratch | `nm`, `cc` | TASK-005 | I | DONE | `3e4560f`: PASS. `nm -gU librivet.dylib` = exactly 4 `rivet_*` symbols; C links shared and static on macOS; native-static-libs `-framework Security -framework CoreFoundation -liconv -lSystem -lc -lm`; the dylib install name needs `@rpath` (RES-2026-0004 E2) |
| TASK-007 | P1 | E3: cfg-gating adapter registration in `runtime.rs` compiles with `--no-default-features`, and each single feature builds | R11 / C-08 | scratch | `cargo build --no-default-features` | TASK-004 | I | DONE | `3e4560f`: PASS for `grpc` (tonic/prost optional, cfg on module + registration): `--no-default-features` 0 errors, 1 unused-import warning, 197 → 182 crates; entanglement survey for quic/serve/oauth/cli (RES-2026-0004 E3) |
| TASK-008 | P1 | Environment: `cargo clean`; confirm ≥ 15 GiB free; install `cbindgen`; confirm Node/npx and a C compiler; document the dev prerequisites | — | — | `df -h`, tool versions | — | I | DONE | No `target/` at start (32 GiB free; 23 GiB after the P2a builds); `cbindgen` 0.29.4 installed; Apple clang 21.0.0; Python 3; **Node.js absent** (`/usr/local/bin/npx` shim, no `node`) → TextMate snapshots and `vsce` unavailable until installed (finding) |
| TASK-009 | P1 | Write RES-2026-0004 (E1–E3 results) and ADR-0005 (workspace layout, package name `rivet-runtime`, feature set, fallback if needed); update the research and decision indexes | R10–R13 | docs | T-31 | TASK-005–007 | I | DONE | `3e4560f`: RES-2026-0004 (rev 2 adds the flow-handling sync finding) and ADR-0005 written; research and decision indexes updated |

### P2a — Envelopes, input and pretty (breaking)

| Task | Phase | Description / method | Req / UC / Change IDs | Production files | Test files / manual procedure | Dependencies | Owner | Status | Evidence / result |
|---|---|---|---|---|---|---|---|---|---|
| TASK-010 | P2a | `src/domain/envelope.rs`: `ResponseEnvelope`, `InputEnvelope`, `OutputFormat`, `EnvelopeStatus`, `RecordType`. `from(Completion / RivetError / DataEvent / SessionReceipt)`, `parse(json, legacy_ok)`, `to_json`, `to_json_pretty`. Key order R1. Hand-written JSON Schemas `docs/api/schemas/{response,input,stream-record}.schema.json`, validated in tests | R1, R2, R4–R6 / C-01, C-02, C-04 | PF-01 | `conformance_envelope` | TASK-004 | I | DONE | `c3b5565`: `src/domain/envelope.rs` (ResponseEnvelope, InputEnvelope, RawInput, OutputFormat, EnvelopeStatus, RecordType, `error_object`, codes); parsing is the use case `features/serve/parse_input.rs`; schemas in `f7d2907` |
| TASK-011 | P2a | CLI: `request` prints envelopes; `--data`; `--input FILE\|-`; `--params` alias with `warning[deprecated.params]`; global `--pretty`; `--pretty --stream` → validation.usage; every `--json` output (list, describe, outputs, io, graph, policy explain/generate, trace, auth, connectors, check) enveloped with `operation` = the built-in ID; error output on stdout/stderr unchanged in location | R1, R3–R6 | PF-03 | T-01–T-05 | TASK-010 | I | DONE | `c3b5565`: `--data`, `--input FILE\|-`, `--params` alias (`warning[deprecated.params]`; `--input` aliases → `warning[deprecated.input]`), global `--pretty`, `--pretty --stream` → validation.usage; every `--json` output enveloped (list, describe, outputs, io/`--format json`, graph, check, policy explain/generate, trace, connectors, auth); errors stay on stderr |
| TASK-012 | P2a | HTTP: `/v1/request` input parser; `Deprecation: true`; `?pretty=true` on JSON routes; SSE records; GET `/v1/operations[/{id}[/outputs]]`, `/v1/io`, `/v1/policy/generate` and `/v1/health` return envelopes; `validation.pretty_stream` on SSE with pretty; access log `deprecated=1` | R1, R3–R6 | PF-03 | T-01–T-05, T-15 | TASK-010 | I | DONE | `c3b5565`: body → serve.parse_input; `Deprecation: true` + access-log `deprecated: 1`; `?pretty=true` on every JSON route (the access-log layer re-renders); SSE records (`event: data`/`result`); GET routes + `/v1/health` (data adds `version`) enveloped; SSE + pretty → 400 validation.pretty_stream |
| TASK-013 | P2a | Polling (`accepted` receipt envelope, event records) and WebSocket (request frames `{type:"request",ref,operation,data}`, record frames with `ref`) | R1, R4 | PF-03 | T-01, T-02, T-04 | TASK-010 | I | DONE | `c3b5565`: POST /v1/requests → 202 envelope status accepted (data = receipt); events batch carries records; WS request frames `{type,ref,operation,data}` (aliases), server frames = records with `ref` first; replies name the ref's operation |
| TASK-014 | P2a | MCP: `structuredContent` = envelope, text content = envelope JSON, `isError` = status error; `rivet.request` tool schema `{operation,data}` (alias `id/params`); sessions tools input `data` | R1, R4 | PF-03 | T-01, T-02 | TASK-010 | I | DONE | `c3b5565`: structuredContent = envelope, text = its JSON, isError = status error or cancelled; outputSchema = envelope with `data` = declared output; `rivet.request`/`rivet.sessions.open` take `{operation,data}` (aliases, `Deprecation` header over HTTP) |
| TASK-015 | P2a | Library: `Runtime::call(InputEnvelope) -> ResponseEnvelope`; `request(...)` kept; stream `Envelope` renders as records; `ResponseEnvelope::from(Completion)` | R1, R4 | PF-03 | T-01, T-02 | TASK-010 | I | DONE | `c3b5565`: `Runtime::call(InputEnvelope) -> ResponseEnvelope`, `Runtime::call_json(&str)`; `request(...)` kept; `Envelope::record()`, `Completion::envelope()`, `to_json_pretty()` |
| TASK-016 | P2a | `--endpoint` remote client and `infra/remote_client.rs` decode 0.2 envelopes; a 0.1 server gives `protocol.endpoint` with a version hint from `/v1/health` | R1 | PF-04 | T-02 | TASK-012 | I | DONE | `c3b5565`: request body `{operation,data}`; 2xx envelopes decoded (status error/cancelled → RivetError, `effects` top-level); 0.1.0 error bodies still decode; a non-envelope 2xx → protocol.endpoint with a hint and `details.server_version` from /v1/health |
| TASK-017 | P2a | Error registry: `validation.input_envelope` (2/422), `validation.pretty_stream` (2/400) | R4–R6 | PF-02 | T-05, T-15 | TASK-010 | I | DONE | `c3b5565`: `validation.input_envelope` (kind validation → exit 2, HTTP 422) and `validation.pretty_stream` (HTTP 400 via `io::http::error_status`; the CLI uses validation.usage); API-2026-0005 rows are P4 (D-44) |
| TASK-018 | P2a | Update every existing conformance suite (27 suites + samples) to the envelope and input shapes; no behaviour change allowed | R1 | tests | full `cargo test` | TASK-011–017 | I | DONE | `c3b5565`: all suites moved to `{operation,data}`, `--data` and envelope reads (T-34); 409 tests pass (400 baseline + 3 unit + 6 conformance_envelope in `f7d2907`) |
| TASK-019 | P2a | Update demo request fixtures (`docs/demos/**/requests/*.json\|jsonl`) and fixture scripts to the input envelope so the demos stay executable | R4 | fixtures | T-30 | TASK-018 | I | DONE | `c3b5565`: `01-catalog/requests/{add,countdown}.http.json`, `ws-frames.jsonl`, `10-grpc/requests/open.http.json`, `ws-chat.jsonl` and both `fixtures/ws_client.py` docstrings; MCP request files unchanged (JSON-RPC framing) |

### P2b — Globals

| Task | Phase | Description / method | Req / UC / Change IDs | Production files | Test files / manual procedure | Dependencies | Owner | Status | Evidence / result |
|---|---|---|---|---|---|---|---|---|---|
| TASK-020 | P2b | Grammar: top-level `global NAME = EXPR` in `rivet.capy`; lowering to `GlobalDecl` (`domain/ir.rs`) | R7 / C-05 | PF-05 | T-06 | TASK-004 | I | DONE | `673994c`: `rivet.capy` top-level `global` (tail capture); `Lowerer::global_decl` splits NAME/EXPR (`syntax.global` at the line or the bad name; an effect head is `check.global_not_constant`); `GlobalDecl{name, expr, span, expr_span}` in `domain/ir.rs` |
| TASK-021 | P2b | Use case `features/language/compile_globals.rs`: constant evaluation in declaration order (literals, lists, objects, arithmetic, comparison, interpolation, pure built-ins over earlier globals); codes `syntax.global`, `check.global_not_constant`, `check.global_forward_ref`, `check.global_duplicate` | R7, R8 | PF-06 | T-06, T-07 | TASK-020 | I | DONE | `673994c`: `features/language/compile_globals.rs` — one `GlobalScope` per file, declaration order, folded by `domain/const_eval.rs` (shared with the interpreter: `binary`, pure built-ins); duplicate at the second name, forward ref at the reference, not-constant at the expression (UC-04 sample: `bad.rivet:1:16`); called from `compile_program` |
| TASK-022 | P2b | Frame lookup: locals → params → globals; `check.global_shadow` (param, local, loop variable, binding, `as NAME`) and `check.global_assign` at compile time | R7, R8 | PF-07 | T-06, T-07 | TASK-021 | I | DONE | `673994c`: `Frame::get` → locals, params, then the operation file's frozen globals (`Interpreter.globals`, `Arc` per file); `check.global_shadow` for a param (name span), loop variable, map item, `with … as`, dag node, task, secret; `check.global_assign` for `=` and `+=` |
| TASK-023 | P2b | Effect analysis and call graph substitute globals; targets built only from literals and globals are `exact`; `policy generate` emits exact grants | R9 / C-06 | PF-08 | T-08 | TASK-021 | I | DONE | `673994c`: `effect_sites` resolves globals in templates (component-aware for URLs, like `assemble_url`) and folds whole constant expressions; literal+global targets are `exact`, a param keeps `param_dependent` with the host resolved; `(request G)` with a global ID is a static call edge; `policy generate` grants exactly (T-08) |
| TASK-024 | P2b | `tests/conformance_globals.rs` (T-06–T-08), including concurrent requests reading the same values | R7–R9 | tests | T-06–T-08 | TASK-022, 023 | I | DONE | `673994c`: `tests/conformance_globals.rs` — 12 tests (T-06 values everywhere, 32 concurrent requests equal; T-07 every code with line/column/end column and exit 2, CLI render; T-08 manifest + library/CLI generate) |

### P2c — Package, facade and features

| Task | Phase | Description / method | Req / UC / Change IDs | Production files | Test files / manual procedure | Dependencies | Owner | Status | Evidence / result |
|---|---|---|---|---|---|---|---|---|---|
| TASK-030 | P2c | Workspace root; package `rivet-runtime`, `[lib] name = "rivet"`, `[[bin]] rivet` with `required-features = ["cli"]`; `workspace.package.version` shared with `rivet-ffi` | R10, R12 / C-07, C-09 | PF-10 | T-09 | TASK-009, P2a | I | DONE | `10c8130`: `[workspace] members = [".", "ffi"]`, resolver 3; `[workspace.package]` holds version (0.1.0), edition, rust-version, license, repository, homepage; root package `rivet-runtime` with `[lib] name = "rivet"` and `[[bin]] rivet` `required-features = ["cli"]`; `ffi/` (`rivet-ffi`) inherits `version.workspace` |
| TASK-031 | P2c | Facade in `src/lib.rs`: `Runtime`, `RuntimeBuilder`, `Policy`, `Value`, `InputEnvelope`, `ResponseEnvelope`, `Envelope`, `Error`, `DataSink`, `highlight`; internals `pub(crate)` or `#[doc(hidden)] pub mod internal`; tests use the facade or `internal` | R10 / C-07 | PF-09 | T-09 | TASK-030 | I | DONE | `10c8130`: facade in `src/lib.rs` — `Runtime`, `RuntimeBuilder`, `Module`, `Scope`, `StreamHandle`, `DuplexHandle`, `DuplexSender`, `Policy`, `Value`, `InputEnvelope`, `ResponseEnvelope`, `EnvelopeStatus`, `OutputFormat`, `RecordType`, `Envelope`, `Completion`, `DataEvent`, `DataSink`, `Error`, `ErrorKind`, `Result`, `highlight`, `build_features`, `ABI_VERSION`, `VERSION`, `types::{GraphQuery, Catalog, OutputReport, Principal, RegistryEntry, IoQuery, IoReport, PolicyDraft, ModuleSummary, SessionLimits, SourceSpan}`; the five buckets live under `#[doc(hidden)] pub mod internal` (`src/internal.rs`, see deviation); every test and example uses the facade or `rivet::internal::…`; `cargo doc` 0 warnings |
| TASK-032 | P2c | Cargo features `serve`, `grpc`, `quic` (+ HTTP/3), `oauth`, `cli`; default = `serve,grpc,quic,oauth`; cfg-gated dependencies and adapter registration; `unsupported.feature` (5/501, `details.feature`); `rivet.capabilities.features` | R11 / C-08 | PF-10 | T-10 | TASK-030 | I | DONE | `10c8130`: features `serve` (axum), `grpc` (tonic, prost, prost-reflect, tower), `quic` (quinn, h3, h3-quinn), `oauth` (keyring-core + platform stores), `cli` (clap); default = serve, grpc, quic, oauth; cfg on the adapter modules and their registration; stand-ins in `infra/unsupported_features.rs` (HTTP/3, OAuth); `domain::capabilities::require_build_features` refuses at load (`unsupported.feature`, exit 5, HTTP 501, `details.feature`, span); `rivet serve` without `serve` exits 5; `rivet.capabilities` gains `build_features` and `abi_version`, and its protocol rows follow the build. `--no-default-features` and each single feature build with 0 warnings; unique normal deps 237 → 172 |
| TASK-033 | P2c | `examples/embed.rs` (facade only); convert `docs/demos/12-library/embedding.rs.txt` into a compiled example (closes PLAN-2026-0001 D-13 note) | R10 | examples | T-09 | TASK-031 | I | DONE | `10c8130`: `examples/embed.rs` (facade only; `cargo run --example embed` prints the envelopes, a stream, outputs, the manifest and a draft); `examples/modules.rs` moved to the facade. `docs/demos/12-library/embedding.rs.txt` stays until D-61 (P4) points at the example |
| TASK-034 | P2c | `commands.perch` build/install/tests pass `--features cli` (or `--all-features` where right); CI feature matrix; `scripts/check_version.py` reads the workspace version | R11 | `commands.perch`, `.github/workflows/ci.yml`, `scripts/check_version.py` | `perch --check`; T-10 | TASK-032 | I | DONE | `10c8130`, `f9af92a`: `commands.perch` (`--features cli` for build/install/help, `--workspace --all-features` for check/tests/clippy, new `features`, `ffi`, `ffi_header`; `perch --check` clean); CI builds `--workspace --all-features`, adds the `features` matrix job (none, serve, grpc, quic, oauth, cli: build + `conformance_features`) and the cbindgen `--verify` step, every step through `scripts/ci_step.py`; `check_version.py` reads `[workspace.package]`, checks `version.workspace` in both manifests and librivet; `editors/vscode/package_vsix.py` reads the workspace version |
| TASK-035 | P2c | crates.io readiness: package metadata (`keywords`, `categories`, `documentation`, `include`), `cargo package --list` clean. `cargo publish --dry-run` is expected to fail on the git dependency; record it | R12 | `Cargo.toml` | T-10 | TASK-030 | I | DONE | `10c8130`: `documentation`, `homepage`, `keywords`, `categories`, `include` (src, grammar, `editors/keywords.json`, examples); `cargo package --list` checked by `conformance_features` (no tests/, docs/, ffi/). `cargo publish --dry-run -p rivet-runtime` fails as expected: "all dependencies must have a version requirement specified when publishing. dependency `capy-core` does not specify a version" (G-PUB) |
| TASK-038 | P2c | Publish Capy as `capy-lang` and `rivet-runtime` to crates.io | R12 | — | — | G-PUB | M | DEFERRED | ADR-0004: owner's decision, outside v0.2.0 |

### P2d — C ABI

| Task | Phase | Description / method | Req / UC / Change IDs | Production files | Test files / manual procedure | Dependencies | Owner | Status | Evidence / result |
|---|---|---|---|---|---|---|---|---|---|
| TASK-040 | P2d | `src/orchestrator/setup_ffi.rs` (surface `ffi`): runtime handle (owns a tokio runtime), `request` (blocking), call handles (start/next/send/finish_input/cancel/free) over the session driver, `pretty`, `highlight` passthrough, `catch_unwind`, null/UTF-8/JSON guards, handle generation checks | R13–R15 / C-10 | PF-11 | T-11, T-12 | P2c | I | DONE | `f9af92a`: `src/orchestrator/setup_ffi.rs` (surface `ffi`, 12 calls and triggers): runtime handle owning a multi-thread tokio runtime; blocking `request` (serve.parse_input → `Runtime::call`); call handles = sessions of the shared session driver (`connection_owned`), `next(timeout)` long-polls in ≤ 5 s slices, `send` keeps the sequence, `cancel` = sessions.cancel_session + execution.cancel_request, `free` while running cancels and drains (5 s grace); `pretty` per input or runtime; `highlight` passthrough; `catch_unwind` on every entry (`internal.panic`), NULL/UTF-8/JSON guards (`validation.ffi_argument`), opaque tokens `(generation << 2) \| kind` never dereferenced, tracked strings; domain `FfiOptions` (`src/domain/ffi.rs`) |
| TASK-041 | P2d | `ffi/` crate: `crate-type = ["cdylib","staticlib"]`, lib name `rivet` (→ `librivet`), `#[no_mangle] extern "C"` shims only; `cbindgen.toml`; checked-in `include/rivet.h` (CI diff check); `rivet.pc.in` with `Libs.private` per OS | R13 | PF-12 | T-11 | TASK-040 | I | DONE | `f9af92a`: `ffi/` — `crate-type = ["cdylib", "staticlib", "rlib"]` (rlib: deviation), lib `rivet`, 18 `#[unsafe(no_mangle)]` shims only; `build.rs` sets `@rpath/librivet.dylib` (soname on Linux); `cbindgen.toml` → checked-in `include/rivet.h` (CI `cbindgen --verify`, test `t11_header_matches_cbindgen`); `rivet.pc.in` + `render_pc.py` (Libs.private per OS; macOS without `-lSystem`); runtime features pass through |
| TASK-042 | P2d | Examples: `examples/c/demo.c` + `Makefile` (shared and static), `examples/python/demo.py` (ctypes); stream, input and cancel shown | R13, R15 | examples | T-11 | TASK-041 | I | DONE | `f9af92a`: `examples/c/{demo.c, Makefile}` (shared with rpath, static with Libs.private; `TARGET=`/`OUT=`), `examples/python/{rivet.py, demo.py}`; request, pretty, invalid, stream, live input, timeout marker, cancel, highlight, double free shown; shared bundle `examples/ffi/app.rivet` |
| TASK-043 | P2d | Tests: `ffi/tests/` (Rust calling the extern functions: null, bad UTF-8, bad JSON, panic probe, double free, free-while-running) and `tests/conformance_ffi.rs` driving the C example; ASan build on Linux when CI exists | R14 | tests | T-11, T-12 | TASK-041 | I | DONE | `f9af92a`: `ffi/tests/ffi_safety.rs` (8 tests: NULL/bad UTF-8 on every entry, bad JSON, panic probe = `block_on` inside a tokio worker, double free of runtime/call/module/string, wrong handle kind, free while running < 6 s, 8 threads on one runtime, module handles, versions) and `tests/conformance_ffi.rs` (7 tests: `nm` exports, C shared+static, C modules, Python demo and modules, header, pkg-config). ASan not run (no Linux host; CI job later) |
| TASK-044 | P2d | `rivet.capabilities.abi_version`; `rivet_version()` equals the crate version (T-33) | R13 | PF-11 | T-11, T-33 | TASK-040 | I | DONE | `10c8130`, `f9af92a`: `rivet.capabilities.abi_version` = 1 (`domain::capabilities::ABI_VERSION`); `rivet_version()` = `CARGO_PKG_VERSION` (`versions_agree`; `check_version.py` loads librivet and compares both) |
| TASK-045 | P2d | Module ABI (R23): `rivet_load`, `rivet_module_operations`, `rivet_module_call`, `rivet_module_call_start`, `rivet_module_free`; `examples/c/modules.c`; `examples/python/rivet.py` wrapper (module object with operation attributes) + `modules.py` | R23 / C-15 | PF-11, PF-12, PF-17 | T-19 | TASK-040, TASK-105 | I | DONE | `f9af92a`: `rivet_load` (→ `Runtime::load`/`load_as`, registry.load_module), `rivet_module_operations` (`[{id, operation, name, description, emits, receives}]`), `rivet_module_call`, `rivet_module_call_start`, `rivet_module_free` (the module stays loaded); `examples/c/modules.c`, `examples/python/rivet.py` (`Module.__getattr__` → operations) + `modules.py`; contract: ffi calls += `registry/load_module`, `rivet_load` trigger (hand edit) |

### P2e — Highlighting

| Task | Phase | Description / method | Req / UC / Change IDs | Production files | Test files / manual procedure | Dependencies | Owner | Status | Evidence / result |
|---|---|---|---|---|---|---|---|---|---|
| TASK-050 | P2e | `editors/gen_grammar.py`: extract `rivet.capy` literals + class table into `editors/keywords.json`, then generate `editors/rivet.tmLanguage.json` (classes: declaration, control, option, type, effect, string, interpolation, number, boolean/null, comment `#`, operation id, global, operator) | R16 / C-11 | PF-14 | T-13 | TASK-020 (so `global` is included) | I | DONE | `9ec2a39`: `editors/gen_grammar.py` (108 `rivet.capy` literals incl. `block_sections else`; table classes declaration, control, option (65 `opt_*` literals derived + 6 modifiers), type, effect, boolean_null, operator, builtin) → `editors/keywords.json` → `editors/rivet.tmLanguage.json` (`source.rivet`, standard scopes) + the extension copy; `--check` mode |
| TASK-051 | P2e | `editors/vscode/`: `package.json` (version = workspace version), `language-configuration.json` (`#` comments, brackets, `end` indentation), grammar link, README; `npx @vscode/vsce package` → `rivet-<ver>.vsix` | R16 | PF-14 | T-13 | TASK-050 | I | DONE | `9ec2a39`: `editors/vscode/{package.json (0.1.0 = Cargo), language-configuration.json, README.md, .vscodeignore, syntaxes/}`; `.vsix` built by `editors/vscode/package_vsix.py` (Python `zipfile`, deterministic) instead of `vsce` (deviation); `dist/` ignored; installed as `rivet.rivet@0.1.0` into an isolated `code --extensions-dir` |
| TASK-052 | P2e | Use case `features/language/highlight_source.rs` (pure, over the parser port) + ANSI/HTML/JSON renderers; CLI `rivet highlight FILE [--format …]` (ansi on a TTY, json otherwise); library `rivet::highlight::tokens` | R17 / C-12 | PF-13 | T-14 | TASK-004 | I | DONE | `0d6c2ea`: `features/language/highlight_source.rs` (pure, Parser port, embeds `editors/keywords.json`), `domain/highlight.rs` (HighlightToken, HighlightFormat, ANSI/HTML/JSON renderers), CLI `rivet highlight FILE [--format …]`, library `rivet::highlight::{tokens, tokens_of, highlight, render}`; contract: library calls + trigger (hand edit) |
| TASK-053 | P2e | Tests: keyword drift (`editors/check_keywords.py` in `cargo test` via a small Rust test or CI step); `vscode-tmgrammar-test` snapshots over REF-2026-0002 blocks and demo sources (Node optional job); `tests/conformance_highlight.rs` goldens | R16, R17 | tests | T-13, T-14 | TASK-050–052 | I | DONE | `9ec2a39`, `0d6c2ea`: `tests/conformance_highlight.rs` (8 tests: drift, TextMate corpus, `.vsix`, 3 format goldens, classes, partial tokens, usage, span corpus) shelling out to `python3 editors/check_keywords.py`, `editors/tests/check_grammar.py` (Python TextMate engine, replaces `vscode-tmgrammar-test`) and `package_vsix.py`; 455 tests pass |

### P2f — File modules (PROP-2026-0002 revision 3)

| Task | Phase | Description / method | Req / UC / Change IDs | Production files | Test files / manual procedure | Dependencies | Owner | Status | Evidence / result |
|---|---|---|---|---|---|---|---|---|---|
| TASK-100 | P2f | Contract delta for modules: domain `ImportDecl`, `ModuleRef`, `ModuleSummary`, `CatalogSnapshot`; use cases `language.resolve_imports`, `registry.load_module`; library/ffi triggers `load`/`rivet_load` | R19–R24 / C-14, C-15 | `vhco-contract.json` | `vhco sync .` | TASK-004 | I | DONE | `673994c` (hand-edited with P2b): domain `ImportDecl`, `ModuleRef`, `ModuleLoad`, `ModuleSummary`, `CatalogSnapshot`; `SourceBundle.modules`, `CompiledProgram.{globals, global_scopes, modules}`, `Operation.{module, param_spans}`; use cases `language.resolve_imports` (Parser, SourceLoader) and `registry.load_module` (CatalogStore) with todos, failures and flows; cli/library `calls`; library trigger `Runtime::load(PATH) \| Runtime::load_as(PATH, ALIAS)`. Refined in `2942066` (port parameter types only, flow step order; TRBL-2026-0005). The ffi `rivet_load` trigger is left to P2d |
| TASK-101 | P2f | Grammar: top-level `import "PATH" as ALIAS [public]` before declarations; lowering to `ImportDecl`; `syntax.import` | R19 / C-14 | PF-19 | T-16, T-17 | TASK-100 | I | DONE | `673994c`/`2942066`: `rivet.capy` `import` (tail capture); `Lowerer::imports` checks shape `import "PATH" as ALIAS [public]`, relative PATH, identifier alias (`rivet` reserved) and placement before any other top-level line (`syntax.import` at the import line; inside an operation at the statement) |
| TASK-102 | P2f | Use case `features/language/resolve_imports.rs` over the source-loader port: path relative to the importing file, confined to the runtime root (no `..`, no symlinks), canonical dedup (compile once), cycle detection with the path, limits (256 files, depth 16), alias uniqueness, namespacing `ALIAS.ID` (transitive), visibility (internal vs `public`), collisions, per-module globals/connectors/auth, bootstrap sites for each file, warning `check.module_policy_ignored` | R19–R21, R24 | PF-20 | T-16, T-17, T-20 | TASK-101, P2b | I | DONE | `2942066`: `features/language/resolve_imports.rs` — breadth-first from the roots (entry "" + host-loaded modules), path relative to the importing file and lexically inside the root (`permission.import_outside_root`, exit 3), `SourceLoader.read_module` refuses symlinks and missing files (`not_found.import`, exit 4), shallowest import names a file and it compiles once, `check.import_duplicate`, `check.import_cycle` with the path, `limit.imports` (256 files, depth 16, exit 5), public iff an all-`public` chain, `policy_ignored` → warning `check.module_policy_ignored`; namespacing in `lowering/modules.rs` (`ALIAS.ID`, transitive), `check.import_collision` |
| TASK-103 | P2f | Calls: `(ALIAS.ID {…})` and `(request "ALIAS.ID" …)` resolve to the namespaced operation; calls inside a module use its own IDs; `check.unknown_function` knows imported names | R20 | PF-20 | T-16 | TASK-102 | I | DONE | `2942066`: `namespace_modules` resolves every literal `(request "id")` and call-by-ID `(ID {…})` / `(ALIAS.ID {…})` in its own file's scope (own IDs, import aliases, own connectors, `rivet.*`) and rewrites it to `(request "canonical")`; unresolvable module calls and another module's `private true` op → `check.unknown_operation`; the expression parser accepts the file's own IDs and `ALIAS.…` (`CallScope`) |
| TASK-104 | P2f | Manifest, call graph, `policy explain`, `policy generate` and `io --include-bootstrap` cover modules with module spans (`users.rivet:12`) and namespaced IDs; replaces the `(+ imports)` placeholder (INC-2026-0008 known issue) | R24 / C-14 | PF-21 | T-20 | TASK-102 | I | DONE | `2942066`: `io --include-bootstrap` lists `./app.rivet` and each module file (the `(+ imports)` placeholder is gone); sites, graph, `policy explain`/`generate` carry module spans (`lib/data.rivet:3`) and namespaced IDs (T-20) |
| TASK-105 | P2f | Host API: `Runtime::load(path)`, `load_as(path, alias)` → `Module` (`alias`, `operations`, `describe`, `outputs`, `call`, `stream`, `duplex`); `Runtime::builder().root(dir)` without an entry file; `features/registry/load_module.rs`; immutable catalog snapshot swap (`Arc`), with in-flight requests keeping theirs; loaded modules public under their alias | R22 / C-15 | PF-22 | T-18 | TASK-102, TASK-015 | I | DONE | `2942066`: `Runtime` holds `RwLock<Arc<Snapshot>>` (catalog + registry + interpreter + MCP/OAuth/gRPC adapters); a request and its nested calls stay on their snapshot; `Runtime::builder().root(dir)`; `features/registry/load_module.rs` over the `CatalogStore` port (`RuntimeCatalog`: resolve + compile, publish = Arc swap, loads serialized); `Runtime::load/load_as` → `rivet::Module` (`alias`, `file`, `warnings`, `operations`, `describe`, `outputs`, `call`, `stream(&Scope,…)`, `duplex(&Scope,…)`); loaded modules public under their alias |
| TASK-106 | P2f | `examples/modules.rs` (Rust host loading two files) | R22 | PF-17 | T-18 | TASK-105 | I | DONE | `2942066`: `examples/modules.rs` + `examples/modules/{users.rivet, lib/billing.rivet}` — `cargo run --example modules` loads two files (billing imports users: compiled once), prints envelopes named `users.get` / `billing.invoice`, and shows the duplicate-alias refusal |
| TASK-107 | P2f | `tests/conformance_modules.rs` (T-16, T-17, T-18, T-20), including concurrent `load` during requests | R19–R24 | PF-T09 | T-16–T-18, T-20 | TASK-102–105 | I | DONE | `2942066`: `tests/conformance_modules.rs` — 16 tests: T-16 (compile once, transitive namespaces, visibility, calls, per-module globals, module-scoped connectors), T-17 (every code with file/line/column and exit; symlink; cycle path; both limits), T-18 (root-only runtime, load/load_as, Module API, streams, 6 concurrent loads during 24 running requests, in-flight snapshot kept), T-20 (loader policy only, warning, io/graph/explain/generate) |

### P3 — Tests and validation

| Task | Phase | Description / method | Req / UC / Change IDs | Production files | Test files / manual procedure | Dependencies | Owner | Status | Evidence / result |
|---|---|---|---|---|---|---|---|---|---|
| TASK-060 | P3 | Full gates: `cargo fmt --check`, `clippy --workspace --all-targets --all-features -D warnings`, `cargo test --workspace --all-features`, feature matrix, `cargo deny check`, release build of both crates, `vhco assure` | all | — | T-27, T-28, T-34 | P2* | I | NOT STARTED | |
| TASK-061 | P3 | TEST documents for new tests TEST-2026-0034…0053 (T-01…T-20) and re-record the regression suites TEST-2026-0001…0033 at the RC commit (revision bump; results for v0.2.0) | all | docs | T-32 | TASK-060 | I | NOT STARTED | |
| TASK-062 | P3 | Validation report RPT-2026-0015 (R1–R18 PASS/PARTIAL/FAIL; deviations; unintended behaviour; limitations) | all | docs | T-32 | TASK-061 | I | NOT STARTED | |
| TASK-063 | P3 | Record every unexpected defect as INC-2026-0009+ and reusable problems as TRBL-2026-0004+ | — | docs | T-31 | ongoing | I | NOT STARTED | |
| TASK-064 | P3 | Security checks: secret-canary scan of all test output (`--nocapture`), FFI misuse suite, confirmation that a global cannot hold a secret | R14, R8 | — | T-12 | TASK-060 | I | NOT STARTED | |

### P4 — Documentation and demos

| Task | Phase | Description / method | Req / UC / Change IDs | Production files | Test files / manual procedure | Dependencies | Owner | Status | Evidence / result |
|---|---|---|---|---|---|---|---|---|---|
| TASK-070 | P4 | Envelope sweep: update every example in the 62 current-state files listed in D-40…D-66 to the 0.2.0 envelopes, input keys and flags; paste real output from the RC build | R18 / C-13 | docs | T-30, T-31 | P3 | I | NOT STARTED | |
| TASK-071 | P4 | New API docs: API-2026-0006 (envelope reference + schemas), API-2026-0007 (C ABI reference) | R1–R6, R13–R15 | docs | T-31 | P3 | I | NOT STARTED | |
| TASK-072 | P4 | New manuals: MAN-2026-0009 (C ABI and FFI), MAN-2026-0010 (editor support and highlighting); chapter updates listed in D-20…D-29 | R7–R17 | docs | T-30 | P3 | I | NOT STARTED | |
| TASK-073 | P4 | New system docs: SYS-2026-0010 (FFI surface and packaging), SYS-2026-0011 (highlighting and grammar generation); updates D-30…D-39 | R10–R17 | docs | review | P3 | I | NOT STARTED | |
| TASK-074 | P4 | Migration guide MIG-2026-0001 + new `docs/migrations/index.md` | R5 | docs | T-31 | P3 | I | NOT STARTED | |
| TASK-075 | P4 | New demos 14-globals (DEMO-2026-0016), 15-ffi (DEMO-2026-0017), 16-editor (DEMO-2026-0018), 17-modules (DEMO-2026-0019), executed literally; demos index/README/manifest | R7–R17 | docs/demos | T-30 | P3 | I | NOT STARTED | |
| TASK-076 | P4 | Re-execute demos 01–13 against the RC and update their READMEs (verification records, `verified_against: "0.2.0"`) | R18 | docs/demos | T-30 | TASK-070 | I | NOT STARTED | |
| TASK-077 | P4 | Release verification guide DEMO-2026-0020 (U-01…U-24, one per requirement; per-surface success and failure; FFI and editor rows; verification record) | R18 | docs/demos | T-30 | TASK-075, 076 | I | NOT STARTED | |
| TASK-078 | P4 | REF-2026-0002: `global` syntax row and examples; CLI mapping `--data`/`--input`/`--pretty`/`highlight`; envelope examples; status lines re-verified | R7, R4 | docs | T-29 | P3 | I | NOT STARTED | |
| TASK-079 | P4 | Architecture, security, operations, runbooks, onboarding updates (D-30…D-39) | R10–R14 | docs | T-31 | P3 | I | NOT STARTED | |
| TASK-080 | P4 | README.md, `docs/README.md`, `docs/index.md`, and `AGENTS.md` project facts (workspace, `ffi` surface, features) | R18 | docs | T-31 | all P4 | I | NOT STARTED | |
| TASK-081 | P4 | Record the six documentation-impact decisions (findings table + REL-0.2.0) | R18 | docs | T-32 | all P4 | I | NOT STARTED | |
| TASK-082 | P4 | Documents with `review_cycle: on-release`: set `next_review_date`; `vhco doc` drift check | R18 | docs | T-31 | all P4 | I | NOT STARTED | |

### P5 — Version, release and rollout

| Task | Phase | Description / method | Req / UC / Change IDs | Production files | Test files / manual procedure | Dependencies | Owner | Status | Evidence / result |
|---|---|---|---|---|---|---|---|---|---|
| TASK-090 | P5 | Release candidate approval (maintainer standing approval via ADR-0004 unless revoked) | all | — | — | P4 | M | NOT STARTED | |
| TASK-091 | P5 | Workspace version `0.1.0` → `0.2.0`; `.vsix` version; `rivet_version()`; T-33 (`scripts/check_version.py --tag` extended to FFI and vsix) | release | `Cargo.toml`, `editors/vscode/package.json` | T-33 | TASK-090 | I | NOT STARTED | |
| TASK-092 | P5 | REL-0.2.0 draft (flat form) with Breaking Changes → MIG-2026-0001; release commit `release: v0.2.0`; full SHA recorded | release | `docs/releases/rel-0.2.0-release-notes.md` | T-31 | TASK-091 | I | NOT STARTED | |
| TASK-093 | P5 | Annotated tag `v0.2.0`; verify with `git rev-list -n 1`, `describe --exact-match` and a clean tree; build release artifacts locally (`rivet` binary, `librivet.{dylib,a}`, `rivet.h`, `rivet.pc`, `.vsix`) with SHA-256 checksums | release | artifacts | T-33 | TASK-092 | I | NOT STARTED | |
| TASK-094 | P5 | Finalize REL-0.2.0 (tag, SHA, `release:` block, §34 walk); PROP-2026-0002 → `implemented/`; plan status update; indexes | release | docs | `vhco docs release . REL-0.2.0` | TASK-093 | I | NOT STARTED | |
| TASK-095 | P5 | Push `main`, `v0.1.0` and `v0.2.0`; publish artifacts | release | — | remote shows tags | a git remote | M | BLOCKED | no git remote |
| TASK-096 | P5 | CI on Linux and Windows; post-release verification of DEMO-2026-0020 on the published artifacts | release | — | CI | TASK-095 | M/I | BLOCKED | no git remote |

## File and Artifact Checklist

CRUD values: CREATE, READ, UPDATE, DELETE, GENERATE. Proposal F-IDs are cited.

### Production

| ID | Category | Exact path | CRUD | Planned edit (why / what) | Req / tasks | Proposal F | Status | Verification |
|---|---|---|---|---|---|---|---|---|
| PF-01 | production | `src/domain/envelope.rs` (+ `domain/mod.rs`) | CREATE | Envelope types, parser, serializer, formats | R1–R6 / 010 | F-01 | DONE | T-01–T-05 |
| PF-02 | production | `src/domain/contracts.rs`, `src/domain/errors.rs` | UPDATE | Serialize through PF-01; new error codes | R1, R4–R6 / 010, 017 | F-02 | DONE | T-01 |
| PF-03 | production | `src/orchestrator/setup_{cli,http,ws,poll,mcp,library,serve}.rs`, `src/io/{cli,http,ws,mcp,library}/**`, `src/orchestrator/builtins.rs` | UPDATE | Input parsing, output writing, flags, pretty, Deprecation | R1, R3–R6 / 011–015 | F-03, F-04 | DONE | T-01–T-05 |
| PF-04 | production | `src/orchestrator/remote_cli.rs`, `src/infra/remote_client.rs` | UPDATE | Decode 0.2 envelopes; version hint | R1 / 016 | F-04 | DONE | T-02 |
| PF-05 | production | `src/infra/rivet.capy`, `src/features/language/lowering/lower.rs`, `src/domain/ir.rs` | UPDATE | `global` form and `GlobalDecl` | R7 / 020 | F-05, F-06 | DONE | T-06 |
| PF-06 | production | `src/features/language/compile_globals.rs` | CREATE | Constant evaluation and check codes | R7, R8 / 021 | F-06 | DONE | T-06, T-07 |
| PF-07 | production | `src/infra/execution_driver.rs` | UPDATE | Read-only global scope | R7 / 022 | F-07 | DONE | T-06 |
| PF-08 | production | `src/features/audit/inspect_effects.rs`, `src/domain/call_graph.rs`, `src/features/policy/generate_policy.rs` | UPDATE | Global substitution | R9 / 023 | F-08 | DONE | T-08 |
| PF-09 | production | `src/lib.rs`, module visibility across `src/**` | UPDATE | Facade; internals hidden | R10 / 031 | F-09 | DONE (`10c8130`; also `src/internal.rs`) | T-09 |
| PF-10 | production | `Cargo.toml` (workspace + package), `Cargo.lock`, `src/orchestrator/runtime.rs`, `src/features/registry/describe_capabilities.rs` | UPDATE | Rename, features, cfg gates, `unsupported.feature`, capabilities | R10–R12 / 030, 032, 035 | F-10 | DONE (`10c8130`; also `src/domain/capabilities.rs`, `src/domain/errors.rs`, `src/infra/unsupported_features.rs`, `src/infra/mod.rs`, `src/orchestrator/{builtins,setup_cli,setup_library}.rs`) | T-10 |
| PF-11 | production | `src/orchestrator/setup_ffi.rs` | CREATE | FFI surface logic, including module handles (R23) | R13–R15, R23 / 040, 044, 045 | F-11, F-23 | DONE (`f9af92a`; also `src/domain/ffi.rs`) | T-11, T-12, T-19 |
| PF-12 | production | `ffi/Cargo.toml`, `ffi/src/lib.rs`, `ffi/cbindgen.toml`, `ffi/include/rivet.h`, `ffi/rivet.pc.in` | CREATE | Shims, header, pkg-config | R13 / 041 | F-12 | DONE (`f9af92a`; also `ffi/build.rs`, `ffi/render_pc.py`) | T-11 |
| PF-13 | production | `src/features/language/highlight_source.rs`, `src/io/cli/**` (`highlight`) | CREATE / UPDATE | Tokenizer, renderers, command | R17 / 052 | F-13 | DONE (`0d6c2ea`; also `src/domain/highlight.rs`, `src/orchestrator/{setup_cli,setup_library,remote_cli}.rs`, `src/lib.rs`) | T-14 |
| PF-14 | production | `editors/gen_grammar.py`, `editors/check_keywords.py`, `editors/keywords.json` (GENERATE), `editors/rivet.tmLanguage.json` (GENERATE), `editors/vscode/{package.json,language-configuration.json,README.md,.vscodeignore}` | CREATE | Grammar and extension | R16 / 050, 051 | F-14 | DONE (`9ec2a39`; plus `editors/vscode/package_vsix.py`, `editors/vscode/syntaxes/rivet.tmLanguage.json` (generated copy)) | T-13 |
| PF-15 | production | `vhco-contract.json` | UPDATE | Contract Delta | all / 004 | F-16 | DONE | T-28 |
| PF-16 | tooling | `commands.perch`, `.github/workflows/ci.yml`, `scripts/check_version.py` | UPDATE | `cli` feature, feature matrix, FFI and vsix version checks | R11, R13 / 034, 091 | F-18 | IN PROGRESS (`10c8130`, `f9af92a`: `cli` feature, feature matrix, header verify, librivet version check; the vsix version check is P5 TASK-091) | T-10, T-33 |
| PF-17 | examples | `examples/embed.rs`, `examples/modules.rs`, `examples/c/{demo.c,modules.c,Makefile}`, `examples/python/{demo.py,rivet.py,modules.py}`, `docs/demos/12-library/embedding.rs` (from `.txt`) | CREATE / UPDATE | Consumer examples, including module objects | R10, R13, R22, R23 / 033, 042, 045, 106 | F-24 | IN PROGRESS (`examples/modules.rs` `2942066`; `examples/embed.rs` `10c8130`; `examples/c/`, `examples/python/`, `examples/ffi/app.rivet` `f9af92a`; `docs/demos/12-library/embedding.rs` is P4 D-61) | T-09, T-11, T-18, T-19 |
| PF-18 | agent guide | `AGENTS.md` (project facts) | UPDATE | Workspace, `ffi` surface, features | R18 / 080 | — | NOT STARTED | review |
| PF-19 | production | `src/infra/rivet.capy`, `src/features/language/lowering/lower.rs`, `src/domain/ir.rs` | UPDATE | `import` form, `ImportDecl` | R19 / 101 | F-19 | DONE | T-16 |
| PF-20 | production | `src/features/language/resolve_imports.rs` (CREATE), `compile_program.rs`, `src/infra/source_loader.rs` (UPDATE) | CREATE / UPDATE | Resolution, namespacing, visibility, errors, bootstrap sites | R19–R21, R24 / 102, 103 | F-20 | DONE | T-16, T-17 |
| PF-21 | production | `src/features/audit/inspect_effects.rs`, `src/domain/call_graph.rs`, `src/features/policy/*` | UPDATE | Module spans and namespaced IDs | R24 / 104 | F-21 | DONE | T-20 |
| PF-22 | production | `src/features/registry/load_module.rs` (CREATE), `src/orchestrator/runtime.rs`, facade `Module` | CREATE / UPDATE | load/load_as, snapshot swap, builder without an entry file | R22 / 105 | F-22 | DONE | T-18 |

### Tests and fixtures

| ID | Category | Exact path | CRUD | Planned edit | Req / tasks | Status | Verification |
|---|---|---|---|---|---|---|---|
| PF-T01 | test | `tests/conformance_envelope.rs` | CREATE | T-01–T-05, T-15 | R1–R6 / 010–018 | DONE | `cargo test` |
| PF-T02 | test | `tests/conformance_globals.rs` | CREATE | T-06–T-08 | R7–R9 / 024 | DONE | `cargo test` |
| PF-T03 | test | `tests/conformance_features.rs` + CI feature matrix | CREATE | T-10 | R11 / 032 | DONE (`10c8130`; 3–7 tests depending on the features) | CI / local |
| PF-T04 | test | `tests/conformance_ffi.rs`, `ffi/tests/*.rs` | CREATE | T-11, T-12 | R13–R15 / 043 | DONE (`f9af92a`; 7 + 8 tests) | `cargo test --workspace` |
| PF-T05 | test | `tests/conformance_highlight.rs`, `editors/tests/` (tmgrammar snapshots) | CREATE | T-13, T-14 | R16, R17 / 053 | DONE (`editors/tests/check_grammar.py`, not tmgrammar snapshots; `tests/fixtures/highlight/`) | `cargo test --test conformance_highlight` (runs the Python checks) |
| PF-T06 | test | every existing `tests/conformance_*.rs` + `tests/support/**` | UPDATE | Envelope/input assertions | R1 / 018 | DONE | T-34 |
| PF-T07 | fixture | `docs/demos/**/requests/*`, `docs/demos/**/fixtures/*` | UPDATE | Input envelopes | R4 / 019 | DONE | T-30 |
| PF-T08 | schema | `docs/api/schemas/response.schema.json`, `input.schema.json`, `stream-record.schema.json` | CREATE | JSON Schemas used by T-01 | R1, R4 / 010 | DONE | T-01 |
| PF-T09 | test | `tests/conformance_modules.rs` (+ module fixtures under a temp dir) | CREATE | T-16–T-18, T-20 | R19–R24 / 107 | DONE | `cargo test` |

### Generated and release artifacts

| ID | Category | Exact path | CRUD | Planned edit | Req / tasks | Status | Verification |
|---|---|---|---|---|---|---|---|
| PF-G01 | generated | `vhco.json`, `vhco.html` | GENERATE | `vhco spec` / `vhco doc` | R18 / 060 | NOT STARTED | T-28 |
| PF-G02 | generated | `editors/keywords.json`, `editors/rivet.tmLanguage.json` | GENERATE | from `rivet.capy` | R16 / 050 | DONE (`9ec2a39`) | T-13 |
| PF-V01 | version | workspace `Cargo.toml` `version` (single source) | UPDATE | `0.1.0` → `0.2.0` | release / 091 | NOT STARTED | T-33 |
| PF-V02 | release | commit `release: v0.2.0`, tag `v0.2.0` | CREATE | §32 | release / 092, 093 | NOT STARTED | T-33 |
| PF-V03 | release | `target/release/rivet`, `librivet.{dylib,so,dll}`, `librivet.a`/`rivet.lib`, `rivet.h`, `rivet.pc`, `rivet-0.2.0.vsix`, `SHA256SUMS` | GENERATE | Local release artifacts | release / 093 | NOT STARTED | T-33 |
| PF-V04 | release | `docs/releases/rel-0.2.0-release-notes.md` | CREATE | §33 | release / 092, 094 | NOT STARTED | T-31 |

## Test and Validation Checklist

T-01–T-20 are the proposal's tests. T-27–T-34 are plan-level gates. New TEST documents are numbered
TEST-2026-0034 (T-01) … TEST-2026-0053 (T-20). The regression suites keep TEST-2026-0001…0033, re-recorded for 0.2.0.

| Test ID | Type | UC / Req | Scenario (positive / negative / regression) | Exact test file or manual steps | Command / environment | Expected result | Status | Evidence |
|---|---|---|---|---|---|---|---|---|
| T-01 | integration | UC-01 / R1, R2 | Envelope schema on every surface for ok, error, cancelled, accepted | `tests/conformance_envelope.rs` | `cargo test --test conformance_envelope` | Every output validates; key order fixed | PASS (`conformance_envelope`) | TEST-2026-0034 |
| T-02 | integration | UC-01 / R4 | One input file on CLI `--input`, HTTP, WS, polling, MCP, library, FFI | same | same | Identical `data` | PASS (FFI leg in P2d) | TEST-2026-0035 |
| T-03 | integration | UC-01 / R3 | Built-ins, GET routes and `--json` outputs enveloped | same | same | Schema valid | PASS | TEST-2026-0036 |
| T-04 | integration | UC-02 / R1 | Stream records then exactly one result; consumer stop → cancelled | same | same | Sequence and statuses | PASS | TEST-2026-0037 |
| T-05 | integration | UC-03 / R6 | Pretty goldens; pretty + stream refused | same | same | Indent 2; validation.usage / 400 | PASS | TEST-2026-0038 |
| T-06 | integration | UC-04 / R7 | Globals visible everywhere; concurrent requests equal | `tests/conformance_globals.rs` | `cargo test --test conformance_globals` | Values equal | PASS (`conformance_globals`, 3 tests) | TEST-2026-0039 |
| T-07 | failure | UC-04 / R8 | Every `check.global_*` / `syntax.global` code with its span | same | same | Code, line, column; exit 2 | PASS (`conformance_globals`, 7 tests) | TEST-2026-0040 |
| T-08 | integration | UC-04 / R9 | Manifest exactness; exact `policy generate` grants | same | same | `exact` targets | PASS (`conformance_globals`, 2 tests) | TEST-2026-0041 |
| T-09 | build | UC-05 / R10 | An external crate uses only the facade | `examples/embed.rs`, scratch crate | `cargo run --example embed --features …` | Builds, prints an envelope | PASS (`cargo run --example embed`: envelopes, stream, outputs, manifest, draft; facade only) | TEST-2026-0042 |
| T-10 | build | UC-05 / R11, R12 | Feature matrix; `unsupported.feature`; `cargo package --list`; vhco accepts the workspace | `tests/conformance_features.rs`, CI | `cargo build --no-default-features`, per-feature, `vhco validate .` | All green | PASS (local: `--no-default-features` and each single feature build with 0 warnings; `conformance_features` under all features (3), none (6), serve (6), grpc/quic/oauth (5), cli (7); `cargo package --list`; `vhco validate`/`sync` green on the workspace) | TEST-2026-0043 |
| T-11 | integration | UC-06 / R13, R15 | C (shared and static) and Python examples; stream, input, cancel | `tests/conformance_ffi.rs`, `examples/c`, `examples/python` | `make -C examples/c test`; `python3 examples/python/demo.py` | Envelopes; correct lifecycle | PASS on macOS (`conformance_ffi` t11_*, 5 tests: 18 `rivet_*` exports only, install name `@rpath`, C shared + static, Python, header, pkg-config); Linux in CI | TEST-2026-0044 |
| T-12 | failure / security | UC-06 / R14 | Null, bad UTF-8, bad JSON, panic, double free, free-while-running | `ffi/tests/` | `cargo test -p rivet-ffi` (+ ASan on Linux CI) | Error envelopes; no crash, no leak | PASS (`ffi/tests/ffi_safety.rs`, 8 tests; ASan not run) | TEST-2026-0045 |
| T-13 | regression | UC-07 / R16 | Keyword drift; TextMate snapshots over REF blocks and demos; `.vsix` packages | `editors/tests/`, `editors/check_keywords.py` | `python3 editors/check_keywords.py`; `npx vscode-tmgrammar-test …`; `npx @vscode/vsce package` | No drift; snapshots stable; `.vsix` built | PASS (`conformance_highlight` t13_*, 3 tests: `check_keywords.py`, `check_grammar.py` over 107 samples, `package_vsix.py`; Node tools replaced, see deviation) | TEST-2026-0046 |
| T-14 | integration | UC-08 / R17 | `rivet highlight` ansi/html/json goldens; syntax error → partial tokens | `tests/conformance_highlight.rs` | `cargo test --test conformance_highlight` | Goldens match | PASS (`conformance_highlight` t14_*, 5 tests) | TEST-2026-0047 |
| T-15 | compatibility | UC-09 / R5 | Legacy `{id, params}` with deprecation signals; mixed keys refused | `tests/conformance_envelope.rs` | same | Header/warning; 422 | PASS | TEST-2026-0048 |
| T-16 | integration | UC-10 / R19, R20 | Imports compile once; `ALIAS.ID`; internal vs `public`; `(alias.id …)` and `request`; nested imports; per-module globals | `tests/conformance_modules.rs` | `cargo test --test conformance_modules` | As specified | PASS (`conformance_modules`, 4 tests) | TEST-2026-0049 |
| T-17 | failure | UC-10 / R21 | Every import error code with its span; the cycle path in the message | same | same | Exact codes and exits | PASS (`conformance_modules`, 7 tests) | TEST-2026-0050 |
| T-18 | integration | UC-11 / R22 | `load`/`load_as`, `operations`, `call`, streams; builder without an entry file; concurrent load during requests | same | same | Envelopes; no race | PASS (`conformance_modules`, 3 tests) | TEST-2026-0051 |
| T-19 | integration | UC-11 / R23 | C module example; Python module wrapper | `tests/conformance_ffi.rs`, `examples/c/modules.c`, `examples/python/modules.py` | `make -C examples/c modules`; `python3 examples/python/modules.py` | Envelopes | PASS on macOS (`conformance_ffi` t19_*, 2 tests: C shared + static, Python module attributes) | TEST-2026-0052 |
| T-20 | security | UC-10, UC-11 / R24 | Loader policy governs modules; module policy.json ignored with a warning; manifest/graph/generate cover modules with module spans | `tests/conformance_modules.rs` | same | Denials as by the loader; warning; spans | PASS (`conformance_modules`, 2 tests) | TEST-2026-0053 |
| T-27 | build / static | all | fmt, clippy (`--all-features`), deny, release build of both crates | CI or local | `cargo fmt --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo deny check && cargo build --release --workspace --all-features` | All green | NOT STARTED | TEST-2026-0027 (re-recorded) |
| T-28 | architecture | all | VHCO structure and zero drift | — | `vhco validate . && vhco sync . && vhco check .` | Green; sync 0 | NOT STARTED | TEST-2026-0028 (re-recorded) |
| T-29 | corpus | R7, R16 | REF-2026-0002 and demo sources parse (with `global`) | `tests/conformance_samples.rs` | `cargo test --test conformance_samples` | All parse | NOT STARTED | TEST-2026-0029 (re-recorded) |
| T-30 | manual / e2e | all | Demos 01–16 and DEMO-2026-0020 executed step by step against the RC | READMEs | manual | Every step matches | NOT STARTED | TEST-2026-0030 (re-recorded) |
| T-31 | documentation | R18 | `check_docs`, `vhco docs check` | `scripts/check_docs.py` | `python3 scripts/check_docs.py && vhco docs check .` | 0 problems / 0 errors | NOT STARTED | TEST-2026-0031 (re-recorded) |
| T-32 | traceability | R1–R18 | R → task → PF → T → D → U; no orphans | script over this plan, RPT, REL | — | No orphans | NOT STARTED | TEST-2026-0032 (re-recorded) |
| T-33 | release | release | Version sync across Cargo, `--version`, MCP serverInfo, capabilities, `rivet_version()`, vsix; tag = commit; clean tree | `scripts/check_version.py --tag` | — | All `0.2.0`; SHAs equal | NOT STARTED | TEST-2026-0033 (re-recorded) |
| T-34 | regression | R1 | Every 0.1.0 suite (T-01…T-26 of PLAN-2026-0001) green on the new envelopes with no behaviour change | `tests/conformance_*.rs` | `cargo test --workspace --all-features` | 400+ tests pass | PASS (409/409 at P2a) | TEST-2026-0001…0026 (re-recorded) |

## Documentation and Demo Checklist

Every document follows DOCUMENTATION §5/§20 metadata, the §7 visible header and its §12 type template. Every new
manual, system, API and migration document includes ASCII journey, sequence and state visuals (CLAUDE.md).
Real output only: each example is pasted from the RC build, and varying IDs are marked as such.

```text
 docs/
 ├── plans/          PLAN-2026-0002 (this) ······························ P1   D-01
 ├── decisions/      ADR-0004 approval · ADR-0005 workspace/features ······· P1   D-02, D-03
 ├── research/       RES-2026-0004 P1 experiments ························· P1   D-04
 ├── incidents/      INC-2026-0009+ as found ······························ P2–P3 D-05
 ├── troubleshooting/TRBL-2026-0004+ as found ····························· P2–P3 D-06
 ├── testing/        TEST-2026-0034…0053 new · 0001…0033 re-recorded ······· P3   D-07
 ├── reports/        RPT-2026-0015 validation ····························· P3   D-08
 ├── migrations/     NEW DIR · MIG-2026-0001 envelopes + index ············ P4   D-09
 ├── api/            API-2026-0006 envelopes · API-2026-0007 C ABI · schemas/ ·· P4 D-10…D-12 (+ updates D-40…D-44)
 ├── manuals/        MAN-2026-0009 C ABI/FFI · MAN-2026-0010 editors ······· P4   D-13, D-14 (+ D-20…D-29)
 ├── system/         SYS-2026-0010 FFI/packaging · SYS-2026-0011 highlighting ·· P4 D-15, D-16 (+ D-30…D-39)
 ├── architecture/ security/ operations/ runbooks/ onboarding/  updates ···· P4   D-30…D-39
 ├── references/     REF-2026-0002 (global, CLI mapping, envelopes) ········ P4   D-45
 ├── demos/          14-globals · 15-ffi · 16-editor · 17-modules · 01–13 re-verified · DEMO-2026-0020 ·· P4 D-17…D-19, D-71, D-50…D-63
 ├── proposals/      PROP-2026-0002 → implemented/ ························ P5   D-67
 └── releases/       REL-0.2.0 ··········································· P5   D-68
 editors/vscode/README.md (extension readme) ································ P2e  D-69
```

### New documents

| ID | Artifact | Exact path | Why | Required content | Related interfaces | Status | Verification |
|---|---|---|---|---|---|---|---|
| D-01 | plan | `docs/plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md` + `docs/plans/index.md` | §4.5 | This ledger | — | DONE | T-31 |
| D-02 | decision | `docs/decisions/adr-0004-approve-envelopes-globals-library-ffi-highlighting.md` + index | Approval | Gates, Q resolutions | — | DONE | T-31 |
| D-03 | decision | `docs/decisions/adr-0005-workspace-package-and-features.md` + index | Packaging decision after E1–E3 | Workspace layout, `rivet-runtime`, feature set, fallback | Cargo | DONE | T-31 |
| D-04 | research | `docs/research/res-2026-0004-workspace-ffi-and-feature-experiments.md` + index | P1 evidence | E1–E3 results, symbol lists, native static libs per OS, build-size observation | FFI, features | DONE | T-31 |
| D-05 | incidents | `docs/incidents/resolved/inc-2026-0009…` + indexes | §25 | One per unexpected defect | — | NOT STARTED | T-31 |
| D-06 | troubleshooting | `docs/troubleshooting/trbl-2026-0004…` + index | §26 | Linking, symbol export, feature-gate pitfalls as found | — | DONE | T-31 |
| D-07 | tests | `docs/testing/test-2026-0034-envelope-schema.md` … `test-2026-0048-legacy-input.md` + re-records of 0001…0033 + `docs/testing/index.md` | §27 | Definition, requirement, environment, result, evidence | all | NOT STARTED | T-32 |
| D-08 | validation | `docs/reports/rpt-2026-0015-validation-of-plan-2026-0002.md` + index | §28 | R1–R18 results | all | NOT STARTED | T-32 |
| D-09 | migration | `docs/migrations/mig-2026-0001-response-and-input-envelopes.md` + **new** `docs/migrations/index.md` (REF-2026-0037) | §4.21 | Before/after per surface (CLI, HTTP, SSE, polling, WS, MCP, library); `jq` mapping; deprecation timeline 0.2 → 0.3; client checklist; rollback (pin v0.1.0) | all surfaces | NOT STARTED | T-31 |
| D-10 | API | `docs/api/api-2026-0006-envelopes.md` | Canonical envelope reference | Response, input, stream record; status/type tables; key order; pretty; error object; per-surface wrapping (WS `ref`, MCP `structuredContent`); JSON Schemas | all | NOT STARTED | T-01, T-31 |
| D-11 | API schema | `docs/api/schemas/{response,input,stream-record}.schema.json` | Machine-readable contract | JSON Schema 2020-12 | all | DONE | T-01 |
| D-12 | API | `docs/api/api-2026-0007-c-abi.md` | C ABI reference | Every `rivet_*` function (incl. `rivet_load`, `rivet_module_*`), ownership, threading, status, error envelopes, ABI versioning, linking flags per OS | FFI | NOT STARTED | T-11, T-31 |
| D-13 | manual | `docs/manuals/man-2026-0009-c-abi-and-ffi.md` | Integrator guide | Install `librivet`, compile/link (shared/static), C walkthrough, Python ctypes, Go cgo sketch, streaming/input/cancel, **module objects (`rivet_load`, Python wrapper)**, errors and recovery, troubleshooting | FFI | NOT STARTED | T-30 |
| D-14 | manual | `docs/manuals/man-2026-0010-editor-support-and-highlighting.md` | Author guide | Install `.vsix`, other TextMate editors, `rivet highlight` (ansi/html/json), docs pipeline use, regenerating the grammar | editors, CLI | NOT STARTED | T-30 |
| D-15 | system | `docs/system/components/sys-2026-0010-ffi-surface-and-packaging.md` | §31 | Workspace, facade, features, `setup_ffi` design, runtime/module/call handle lifecycle state machines, artifacts | library, ffi | NOT STARTED | TASK-082 |
| D-16 | system | `docs/system/components/sys-2026-0011-highlighting-and-grammar-generation.md` | §31 | Token classes, parser-span tokenizer, generator pipeline, drift test | language | NOT STARTED | TASK-082 |
| D-17 | demo | `docs/demos/14-globals/` (README DEMO-2026-0016, `app.rivet`, `policy.json`) | Show globals + exact manifest | Steps, success and failure (`check.global_*`), verification record | language | NOT STARTED | T-30 |
| D-18 | demo | `docs/demos/15-ffi/` (README DEMO-2026-0017, C and Python programs) | Show the C ABI | Build, link shared and static, run, stream, cancel, misuse error | FFI | NOT STARTED | T-30 |
| D-19 | demo | `docs/demos/16-editor/` (README DEMO-2026-0018) | Show highlighting | Install `.vsix`, open samples, `rivet highlight` outputs | editors | NOT STARTED | T-30 |
| D-71 | demo | `docs/demos/17-modules/` (README DEMO-2026-0019, `app.rivet`, `users.rivet`, `billing.rivet`, `policy.json`, host programs in Rust/C/Python) | Show modules | `import … as`, internal vs `public`, `rivet list`/`io` across files, import errors, `rt.load`, `rivet_load`, Python module object; verification record | language, library, ffi | NOT STARTED | T-30 |
| D-64 | release verification | `docs/demos/demo-2026-0020-v0-2-0-release-verification.md` | §29 | Version/tag/commit; U-01…U-24; per-surface success/failure incl. FFI, editor and modules; cleanup; verification record | all | NOT STARTED | T-30 |
| D-68 | release notes | `docs/releases/rel-0.2.0-release-notes.md` + `docs/releases/index.md` | §33 | Full template; Breaking Changes; six impact decisions; §34 walk; `release:` block | all | NOT STARTED | T-31, T-33 |
| D-69 | extension readme | `editors/vscode/README.md` | Marketplace-style readme (local) | Features, install, file association | editors | NOT STARTED | review |

### Updated documents (feature chapters)

| ID | Artifact | Exact path | Why it changes | Required addition / removal | Related interfaces | Status | Verification |
|---|---|---|---|---|---|---|---|
| D-20 | manual (root) | `docs/manuals/man-2026-0001-rivet-manual.md` | §30 | What's New in 0.2.0; feature catalogue rows (envelopes, pretty, globals, modules, Cargo dependency, C ABI, highlighting) with why/when and demo links; reading path adds MAN-0009/0010; limitations update (remove L1 "no import form"; add non-goals: URL imports, hot reload) | all | NOT STARTED | T-30 |
| D-21 | manual | `docs/manuals/man-2026-0002-installation-and-quickstart.md` | Install changes | `perch install` / `cargo install rivet-runtime --features cli --git …`; installing `librivet`; installing the `.vsix`; quickstart with envelopes | CLI | NOT STARTED | T-30 |
| D-22 | manual | `docs/manuals/man-2026-0003-language-guide.md` | New forms | **Globals chapter** (syntax, allowed expressions, lookup order, errors, examples); **Modules chapter** (`import … as ALIAS [public]`, path rules, namespacing, visibility, per-module globals, calling `(alias.id …)`, errors, policy); syntax table rows | language | NOT STARTED | T-29 |
| D-23 | manual | `docs/manuals/man-2026-0004-cli-reference.md` | New flags/command | `--data`, `--input`, `--pretty`, `--params` deprecation, `rivet highlight`, envelope output for every command; `list`/`io --include-bootstrap` across modules; exit codes | CLI | NOT STARTED | T-02 |
| D-24 | manual | `docs/manuals/man-2026-0005-policy-and-io-manifest-guide.md` | Globals, modules | Globals in targets, exact grants; one policy for all modules; module `policy.json` ignored; manifest across files | policy, audit | NOT STARTED | T-08 |
| D-25 | manual | `docs/manuals/man-2026-0006-serving-and-surfaces.md` | Wire change | Envelopes per surface, `?pretty=true`, `Deprecation` monitoring, WS/poll/MCP shapes | serve | NOT STARTED | T-01 |
| D-26 | manual | `docs/manuals/man-2026-0007-embedding-library.md` | Library change | Cargo dependency (git tag), features, facade, `Runtime::call`, envelopes, **`Runtime::load`/`Module` objects**, builder without an entry file, link to MAN-0009 for non-Rust | library | NOT STARTED | T-09 |
| D-27 | manual | `docs/manuals/man-2026-0008-protocols-and-connectors.md` | Features | Which protocols need which feature; `unsupported.feature` | transports | NOT STARTED | T-10 |
| D-28 | manual index | `docs/manuals/index.md` | New manuals | MAN-0009, MAN-0010 rows; reading order | — | NOT STARTED | T-31 |
| D-29 | reference manual links | all manual cross-links to API-2026-0006/0007 | Navigation | Links | — | NOT STARTED | T-31 |
| D-30 | architecture | `docs/architecture/arch-2026-0001-rivet-runtime-architecture.md` + index | §31 | Workspace crates, facade boundary, `ffi` surface, envelope edge layer, feature gates (diagrams) | all | NOT STARTED | T-31 |
| D-31 | system | `docs/system/components/sys-2026-0001-compiler-and-catalog.md` | Globals, highlighting, modules | `global` lowering, `compile_globals`, tokenizer link; `resolve_imports` (resolution, dedup, cycles, namespacing, visibility); `load_module` and catalog snapshots | language | NOT STARTED | TASK-082 |
| D-32 | system | `docs/system/runtime/sys-2026-0002-execution-scopes-and-dag.md` | Frame | Global scope lookup order | execution | NOT STARTED | TASK-082 |
| D-33 | system | `docs/system/components/sys-2026-0003-policy-broker-and-io-manifest.md` | Manifest | Global substitution; manifest and policy across modules; bootstrap reads of imported files | audit | NOT STARTED | TASK-082 |
| D-34 | system | `docs/system/components/sys-2026-0004-surfaces-and-serve.md` | Wire | Input parser, envelope writer, pretty, Deprecation, remote client | serve | NOT STARTED | TASK-082 |
| D-35 | system | `docs/system/integrations/sys-2026-0005-protocol-adapters.md`, `sys-2026-0006-oauth-and-credentials.md`, `sys-2026-0009-mcp-client-connectors.md` | Features | Feature gates per adapter; examples re-shaped | transports, auth, connectors | NOT STARTED | TASK-082 |
| D-36 | system | `docs/system/runtime/sys-2026-0007-sessions.md`, `docs/system/configuration/sys-2026-0008-policy-json-reference.md`, `docs/system/index.md` | Wire | Receipts/records as envelopes; index rows for SYS-0010/0011 | sessions | NOT STARTED | TASK-082 |
| D-37 | security | `docs/security/sec-2026-0001-policy-and-sandbox-model.md` + index | New boundary | FFI trust boundary (host = library principal), memory-safety guarantees and non-guarantees, globals cannot hold secrets; modules: root confinement, single policy, no URL imports | ffi, language | NOT STARTED | T-12, T-31 |
| D-38 | operations / runbooks | `docs/operations/ops-2026-0001-operating-rivet-serve.md`, `docs/runbooks/run-2026-0001-…`, `run-2026-0002-…` + indexes | Wire / install | `cli` feature install; `Deprecation` and `deprecated=1` monitoring; envelope examples in procedures | serve | NOT STARTED | T-30 (dry run) |
| D-39 | onboarding | `docs/onboarding/onb-2026-0001-contributor-setup.md` + index | Dev setup | Workspace, features, `cbindgen`, C compiler, Node/npx for editors, new suites, grammar regeneration | — | NOT STARTED | T-31 |

### Updated documents (envelope sweep — every current-state file with 0.1.0 wire examples, TASK-070)

| ID | Exact path(s) | Required change | Status | Verification |
|---|---|---|---|---|
| D-40 | `docs/api/api-2026-0001-http-rest-sse-polling.md` | Every request/response/SSE/polling example → envelopes; `pretty`; `Deprecation`; new errors | NOT STARTED | T-31 |
| D-41 | `docs/api/api-2026-0002-websocket-rivet-v1.md` | Frames `{type:"request",ref,operation,data}`; record frames | NOT STARTED | T-31 |
| D-42 | `docs/api/api-2026-0003-mcp-server-tools.md` | `structuredContent` envelope; `rivet.request` `{operation,data}` | NOT STARTED | T-31 |
| D-43 | `docs/api/api-2026-0004-rust-library.md` | Facade, features, dependency snippet, `call`, envelopes, `load`/`load_as`/`Module` | NOT STARTED | T-09, T-18 |
| D-44 | `docs/api/api-2026-0005-error-registry.md` + `docs/api/index.md` | New codes: `validation.input_envelope`, `validation.pretty_stream`, `syntax.global`, `check.global_*` (5), `unsupported.feature`, `validation.ffi_argument`, `internal.panic`, `syntax.import`, `not_found.import`, `permission.import_outside_root`, `check.import_cycle`, `check.import_duplicate`, `check.import_collision`, `limit.imports`, `check.module_policy_ignored`; examples re-shaped; index rows API-0006/0007 | NOT STARTED | T-31 |
| D-45 | `docs/references/ref-2026-0002-language-and-usage.md` | `global` and `import` syntax rows + examples; CLI mapping; envelope examples; status lines re-verified | NOT STARTED | T-29 |
| D-46 | `docs/manuals/man-2026-0001…0008` (all eight) | Example outputs re-shaped (in addition to D-20…D-27 content) | NOT STARTED | T-30 |
| D-47 | `docs/system/**` (nine SYS docs + `docs/system/index.md`) | Example outputs re-shaped | NOT STARTED | TASK-082 |
| D-48 | `docs/architecture/arch-2026-0001…`, `docs/security/sec-2026-0001…`, `docs/operations/ops-2026-0001…`, `docs/runbooks/run-2026-0002…` | Example outputs re-shaped | NOT STARTED | T-31 |
| D-49 | `README.md`, `docs/README.md` | Quickstart output re-shaped; 0.2.0 state | NOT STARTED | T-31 |
| D-50 | `docs/demos/01-catalog/README.md` + `requests/{add.http,add.mcp,countdown.http,initialize.mcp,list.mcp,outputs.mcp}.json`, `requests/ws-frames.jsonl` | Re-executed; input envelopes | NOT STARTED | T-30 |
| D-51 | `docs/demos/02-file-crud/README.md` | Re-executed | NOT STARTED | T-30 |
| D-52 | `docs/demos/03-http/README.md` | Re-executed | NOT STARTED | T-30 |
| D-53 | `docs/demos/04-streaming/README.md` | Re-executed (records) | NOT STARTED | T-30 |
| D-54 | `docs/demos/05-dag/README.md` | Re-executed | NOT STARTED | T-30 |
| D-55 | `docs/demos/06-mcp-bridge/README.md`, `fixtures/crm_mcp.py`, `schemas/{search-result,tools-list}.fixture.json` | Re-executed; fixture shapes (remote MCP responses stay MCP-shaped; only Rivet output changes) | NOT STARTED | T-30 |
| D-56 | `docs/demos/07-oauth2/README.md` | Re-executed | NOT STARTED | T-30 |
| D-57 | `docs/demos/08-udp/README.md` | Re-executed | NOT STARTED | T-30 |
| D-58 | `docs/demos/09-quic/README.md` | Re-executed | NOT STARTED | T-30 |
| D-59 | `docs/demos/10-grpc/README.md` + `requests/{initialize.mcp,open.http,open.mcp,sessions-finish.mcp,sessions-read.mcp,sessions-send.mcp}.json`, `requests/ws-chat.jsonl` | Re-executed; input envelopes | NOT STARTED | T-30 |
| D-60 | `docs/demos/11-sandbox/README.md` | Re-executed | NOT STARTED | T-30 |
| D-61 | `docs/demos/12-library/README.md`, `embedding.rs` | Facade API, `call`, envelopes; compiled example | NOT STARTED | T-09, T-30 |
| D-62 | `docs/demos/13-real-world-apis/README.md` (+ its report RPT-2026-0014, historical, unchanged) | Commands and outputs re-shaped where local fixtures allow; public-API steps marked as not re-verifiable offline | NOT STARTED | T-30 |
| D-63 | `docs/demos/README.md`, `docs/demos/index.md`, `docs/demos/manifest.json` | Rows for 14–16 and DEMO-2026-0020; `verified_against: "0.2.0"` | NOT STARTED | T-31 |
| D-65 | `docs/demos/demo-2026-0015-v0-1-0-release-verification.md` | **Not changed**: historical verification of v0.1.0 (NOT APPLICABLE: records a past release) | NOT APPLICABLE | — |

### Lifecycle, navigation and agent guide

| ID | Artifact | Exact path | Required change | Status | Verification |
|---|---|---|---|---|---|
| D-66 | navigation | `docs/index.md`, `docs/README.md` | New dirs/docs (migrations/, API-0006/0007, MAN-0009/0010, SYS-0010/0011, demos 14–16, DEMO-0019, RPT-0015, REL-0.2.0); current release 0.2.0; risks | NOT STARTED | T-31 |
| D-67 | proposal lifecycle | `docs/proposals/approved/prop-2026-0002-…` → `docs/proposals/implemented/` + indexes | Status `implemented` at P5 | NOT STARTED | T-31 |
| D-70 | agent guide | `AGENTS.md` project facts | Workspace crates, `ffi` surface, features, editors dir | NOT STARTED | review |
| D-72 | navigation (modules) | `docs/demos/README.md`, `docs/demos/index.md`, `docs/demos/manifest.json`, `docs/index.md` | Rows for 17-modules (DEMO-2026-0019) and the release guide DEMO-2026-0020 | NOT STARTED | T-31 |

**Six post-release documentation-impact decisions** (§31), all expected `UPDATED`:

| Artifact | Expected decision | Covered by |
|---|---|---|
| Release verification guide / demo | UPDATED | D-17…D-19, D-71, D-50…D-64 |
| Top-level README.md | UPDATED | D-49 |
| system/ | UPDATED | D-15, D-16, D-31…D-36, D-47 |
| architecture/ | UPDATED | D-30 |
| api/ and CLI reference | UPDATED | D-10…D-12, D-23, D-40…D-44 |
| manuals/ | UPDATED | D-13, D-14, D-20…D-28, D-46 |

## Version, Release and Rollout Checklist

| Item | Source / target | Required action | Dependency | Status | Evidence |
|---|---|---|---|---|---|
| Version | workspace `Cargo.toml` (single source for `rivet-runtime` and `rivet-ffi`) | `0.1.0` → `0.2.0`; `.vsix` version synced; T-33 | TASK-090 | NOT STARTED | |
| Breaking change | MIG-2026-0001 | Linked from REL-0.2.0 "Breaking Changes" | TASK-074 | NOT STARTED | |
| Release notes | `docs/releases/rel-0.2.0-release-notes.md` | Full §33 template | TASK-092 | NOT STARTED | |
| Release commit | repository | `release: v0.2.0`; full SHA | TASK-092 | NOT STARTED | |
| Tag | repository | `git tag -a v0.2.0`; verify | TASK-093 | NOT STARTED | |
| Artifacts | local `dist/v0.2.0/` (git-ignored) | binary, `librivet` shared + static, `rivet.h`, `rivet.pc`, `.vsix`, `SHA256SUMS` | TASK-093 | NOT STARTED | |
| Push / publish | remote | Push branch and tags; GitHub release assets | a remote | BLOCKED | no git remote |
| Post-release verification | DEMO-2026-0020 | On published artifacts | TASK-095 | BLOCKED | no git remote |

```text
 workspace version 0.2.0 ─► commit "release: v0.2.0" ─► SHA ─► tag v0.2.0 ─► dist/v0.2.0/{rivet, librivet.*, rivet.h, rivet.pc, *.vsix, SHA256SUMS}
        └── T-33: Cargo == --version == serverInfo == capabilities == rivet_version() == vsix ──┘      └─► REL-0.2.0 ─► §34
```

## Decisions, Findings, Deviations and Blockers

| Timestamp | Type | Task / requirement | Finding or decision | Impact | Owner / follow-up | Linked |
|---|---|---|---|---|---|---|
| 2026-09-28 | Decision | TASK-001 | PROP-2026-0002 approved; Q-01…Q-06 resolved per recommendations | Scope fixed | Maintainer | ADR-0004 |
| 2026-09-28 | Decision | — | Agents run sequentially in the main checkout, not in parallel worktrees: 4.3 GiB free and `target/` at 28 GiB | Longer wall-clock; no disk exhaustion | Implementer | TASK-008 |
| 2026-09-28 | Deviation | IDs | The new release verification guide is DEMO-2026-0020 because DEMO-2026-0016…0018 go to the three new demos (DEMO-2026-0014/0015 are taken) | — | Implementer | D-17…D-19, D-64 |
| 2026-09-28 | Deviation | IDs | The validation report is RPT-2026-0015 (RPT-2026-0014 belongs to the 13-real-world-apis smoke tests) | — | Implementer | D-08 |
| 2026-09-28 | Blocker | TASK-095/096 | No git remote: push, CI and git-dependency consumers of the tag cannot be verified remotely | Local release only | Maintainer | — |
| 2026-09-28 | Scope | TASK-038 | crates.io publication deferred (G-PUB) | Git dependency path | Maintainer | ADR-0004 |
| 2026-09-28 | Decision | UQ-09 / P2f | Scope amendment: file modules (import in `.rivet` + host load objects in Rust/C/Python), namespaced by alias, loader's policy only; CLI run-from-path not selected | +8 tasks (TASK-045, TASK-100–107), T-16–T-20, D-71, D-72 | Maintainer | PROP-2026-0002 rev 3, ADR-0004 rev 2 |
| 2026-09-28 | Deviation | IDs | The modules demo takes DEMO-2026-0019; the release verification guide moves to DEMO-2026-0020 | — | Implementer | D-64, D-71 |
| 2026-09-28 | Finding | TASK-005 | vhco reads only `src/`: the `ffi/` crate is invisible, so all FFI logic and annotations go in `orchestrator/setup_ffi.rs` (ADR-0005). `vhco sync` also compares flow triggers (exact text) and flow handling (`layer: ref` steps), so renaming a trigger or a step needs a hand edit of the contract flow | Every later phase edits contract flows with its code | Implementer | RES-2026-0004, TRBL-2026-0004 |
| 2026-09-28 | Finding | TASK-004 | Remaining `vhco sync .` gaps after P2a (12, all future phases): domain `CapabilityReport` (P2c/P2d), `FfiOptions` (P2d), `GlobalDecl`, `GlobalScope` (P2b), `HighlightFormat`, `HighlightToken` (P2e); use cases and flows `language.compile_globals` (P2b) and `language.highlight_source` (P2e); surface `ffi` (P2d); surface `cli` calls `language/highlight_source` (P2e). These entries were written ahead of the code; the implementing phase may refine fields and step ids by hand | Expected gaps | P2b, P2c, P2d, P2e | `vhco-contract.json` |
| 2026-09-28 | Decision | TASK-004 | `rivet.capabilities` keeps its `features` key (protocol rows); compiled Cargo features go under `build_features` (contract `CapabilityReport`) | Refines R11 wording | P2c | ADR-0005 |
| 2026-09-28 | Finding | TASK-008 | Node.js is not installed (`npx` shim only): `vsce` packaging and `vscode-tmgrammar-test` snapshots (T-13) cannot run until it is; the Python keyword-drift check stays mandatory | P2e tooling | Maintainer / P2e | TASK-051, TASK-053 |
| 2026-09-28 | Deviation | TASK-013 | Polling sub-routes keep their session bodies, per the proposal's polling sample: GET …/events answers `{session_id, events: [records], last_seq, terminal}` and input/finish_input/cancel answer SessionAck/CancelReceipt; POST /v1/requests (202, status accepted) and every error are envelopes | Matches the PROP-2026-0002 sample | — | T-01 |
| 2026-09-28 | Decision | TASK-012 | SSE event names mirror the record `type`: `event: data` for items and `event: result` for the terminal record (0.1.0's `event: error` is gone; `status` says error/cancelled) | Clients switch on `type`/`status` | P4 docs (D-40) | API-2026-0001 |
| 2026-09-28 | Decision | TASK-014 | MCP `isError` is true for status error **and** cancelled (keeps 0.1.0 behaviour for cancelled calls; the proposal sample wrote `status == "error"`) | No behaviour change | — | T-01 |
| 2026-09-28 | Decision | TASK-011 | `rivet check --json` prints an envelope (operation `rivet.check`, data counts) and its failures an error envelope; without `--json` it is unchanged. `policy generate` without `--json` still prints the bare policy draft (so `> policy.json` keeps working); with `--json` it prints the `rivet.policy.generate` envelope | Consistent JSON outputs | P4 docs (D-23) | T-03 |
| 2026-09-28 | Decision | TASK-010 | Data records omit `status`, `effects` and `data_count` (they describe a finished request), per the proposal's stream sample; terminal stream records carry `seq` and `data_count` = items before them (errors too) | `stream-record.schema.json` | — | T-04 |
| 2026-09-28 | Decision | TASK-015 | The script-level `completion` property of a request stream keeps its 0.1.0 object shape (language API, not wire output) | No language change | — | — |
| 2026-09-28 | Finding | T-34 | Suites that passed unchanged (dag, files, http3, language, library, outputs, policy_generate, quic, resources, samples, streams, syntax, udp) assert through the Rust library types, not wire JSON; their wire paths are covered by conformance_envelope | Checked, not a gap | — | T-34 |
| 2026-09-28 | Finding | TASK-018 | No unexpected defect in existing code was found during P1/P2a; no incident recorded | — | — | D-05 |
| 2026-09-28 | Deviation | TASK-021 / R7 | Contract refined by hand: `GlobalScope` gains `file` (one frozen scope per file, so each module has its own globals, R20) and `language.compile_globals` returns `GlobalScope[]`; `GlobalDecl` gains `expr_span`; `Operation` gains `module` and `param_spans` (exact `check.global_shadow` span of a param) | Contract and code agree; `vhco sync` has no P2b gap | — | `vhco-contract.json`, `673994c` |
| 2026-09-28 | Deviation | TASK-023 / R9 | The UC-04 sample shows `https://api.example.com/users/{id}` as `exact`. The build keeps the 0.1.0 rule that a param placeholder makes a target `param_dependent`; globals are resolved into the template (host and path known, grantable), and a target built **only** from literals and globals is `exact` | Proposal sample differs; P4 docs show the real output | P4 (D-24, D-33) | T-08 |
| 2026-09-28 | Decision | TASK-021, TASK-022 / R8 | A global may use literals, lists, objects, operators, `${…}` and the pure built-ins (`length`, `base64.*`, `text`, `keys`, `xml.element`); `env`, `secret`, `request`, effects, params and locals are `check.global_not_constant` (a global can never hold a secret — TASK-064 input). A constant that fails to evaluate keeps its own code (`value.type`, `value.division_by_zero`, exit 2). `NAME = …`/`NAME += …` is `check.global_assign`; params, loop variables, map items, `with … as`, dag nodes, tasks and secrets are `check.global_shadow`, each at the statement's own span | Deterministic diagnostics | P4 (D-22, D-44) | T-07 |
| 2026-09-28 | Decision | TASK-102, TASK-103 / R20 | A file reached under two aliases compiles once under the shallowest (breadth-first) namespace; other aliases point at it (`ImportDecl.target`). Calls by operation ID `(ID {…})` work in every file (its own IDs) and are rewritten to `(request "ID" …)`; built-in names win. Dynamic `(request var …)` targets are catalog IDs (namespaced) | Compile once; one call model | P4 (D-22) | T-16 |
| 2026-09-28 | Decision | TASK-102 / R20, R24 | Connector and auth profile references are file-scoped (a module's connectors and profiles stay inside it), but their **names stay bundle-unique** (`check.import_collision` across files) because the loader's single policy grants them by name (`allow_mcp NAME/…`, `allow_auth`); MCP import IDs of a module connector are not prefixed, and those of an internal module are unlisted | One policy namespace | P4 (D-33, D-37) | T-16 |
| 2026-09-28 | Decision | TASK-105 / R22 | Snapshot semantics: a request **and its nested calls** stay on the catalog snapshot it started with (a dynamic nested call to a module loaded meanwhile is `not_found.operation`, tested); loads are serialized; the OAuth adapter is reused across a load when the auth profiles are unchanged, otherwise rebuilt (pending authorizations of the replaced adapter are lost); session receipts report the catalog they opened on | Documented behaviour | P4 (D-26, D-43) | T-18 |
| 2026-09-28 | Deviation | TASK-105 / R22 | `Module::stream` and `Module::duplex` take the owning `&Scope` (`module.stream(&scope, id, data)`), like `Scope::stream`, because 0.1.0 streams are scope-owned (structured concurrency); the proposal wrote `stream(id, data)`. `operations()`/`describe()` report short IDs (`get`); envelopes carry the namespaced `operation` (`users.get`) | API differs from the sketch | P4 (D-26, D-43) | T-18 |
| 2026-09-28 | Decision | TASK-102 / R21 | `limit.imports` keeps the kind registry row (exit 5, HTTP 429, retryable like every `limit.*` code). All new codes are registered in `domain::errors::LANGUAGE_CODES` with a unit test of their rows | Consistent registry | P4 (D-44) | unit test |
| 2026-09-28 | Finding | TASK-104 | The INC-2026-0008 known issue (`./app.rivet (+ imports)`) is resolved: bootstrap rows list the entry and each module file. `tests/conformance_io_manifest.rs` was updated for the new row; demos 01, 08 and 11 READMEs still show the old placeholder | P4 sweep must update them | P4 (D-47…) | T-20 |
| 2026-09-28 | Finding | TASK-101, TASK-105 | Public API changes for P2c/MIG-2026-0001: `SourceBundle` gains `modules` (three existing test literals updated); `Runtime::bundle()` returns an owned `SourceBundle` (was `&SourceBundle`); `setup_library::session_host` takes `Arc<dyn Registry>`; new `Runtime::{load, load_as, catalog}`, `RuntimeBuilder::root`, `rivet::Module` | Facade (P2c) and migration guide must list them | P2c (TASK-031), P4 (D-09) | — |
| 2026-09-28 | Finding | TASK-045 (P2d) | `rivet_load(rt, path, alias_or_null, &module, &err)` should call `Runtime::load` / `Runtime::load_as` (→ `registry.load_module`) and wrap the returned `rivet::Module`; `rivet_module_operations` = `Module::operations()` (short IDs + descriptions), `rivet_module_call` = `Module::call`, `rivet_module_call_start` = a scope-owned `Module::stream`/`duplex`. P2d must add `registry/load_module` to the `ffi` surface `calls` and the `rivet_load` flow trigger in the contract (not added here, to keep sync free of P2f gaps) | Hand-off | P2d | TRBL-2026-0005 |
| 2026-09-28 | Finding | TASK-063 | INC-2026-0009 (active, S4): numeric index paths (`xs.0`) do not parse and the message names `assign_map` — pre-existing in 0.1.0, found by the globals tests, not fixed (fix or v0.2.0 Known Issue). TRBL-2026-0005: contract entries written ahead of code must use port parameter types only, flow steps in source order, and add triggered actions to the surface `calls` | Recorded | P3/P5 | INC-2026-0009, TRBL-2026-0005 |
| 2026-09-28 | Finding | P2b, P2f | 440 tests pass (409 + 2 unit + 12 `conformance_globals` + 1 unit + 16 `conformance_modules`); fmt, clippy `-D warnings`, `vhco validate`, `vhco check` green; `vhco sync` = 8 gaps, all P2c/P2d/P2e (`CapabilityReport`, `FfiOptions`, `HighlightFormat`, `HighlightToken`, flow/use case `language.highlight_source`, cli calls `language/highlight_source`, surface `ffi`) | P2b and P2f exits met | — | `673994c`, `2942066` |
| 2026-09-28 | Deviation | TASK-051, TASK-053 / R16, T-13 | Node.js is absent, so no `vsce` and no `vscode-tmgrammar-test`: `editors/vscode/package_vsix.py` writes the `.vsix` with Python `zipfile` (`[Content_Types].xml`, `extension.vsixmanifest`, `extension/{package.json, README.md, language-configuration.json, syntaxes/rivet.tmLanguage.json}`, fixed timestamps, `.vscodeignore` honoured); `editors/tests/check_grammar.py` applies the generated TextMate regexes with a Python engine (match/captures, begin/end, includes) over the 94 REF-2026-0002 blocks and 13 demo apps and asserts every declaration, control, option and effect statement keyword is scoped. Verified: the `.vsix` installs as `rivet.rivet@0.1.0` (isolated `code --extensions-dir`). Not covered: Oniguruma-specific behaviour (the grammar uses only syntax both engines share) | T-13 passes without Node | P5 (TASK-091 builds the release `.vsix` with the same script) | `9ec2a39` |
| 2026-09-28 | Deviation | TASK-052 / R10, R17 | Contract hand edit: surface `library` calls `language/highlight_source` and the flow gains the trigger `rivet::highlight::tokens(SOURCE) \| rivet::highlight::highlight(SOURCE, FORMAT)`, because R10/R17 name the library entry and vhco only compares triggers of called actions (TRBL-2026-0005 rule 3). `rivet::highlight` is a `pub use` of `orchestrator::setup_library::highlight` (the facade itself is P2c) | Sync has no P2e gap | P2c (TASK-031 keeps `highlight` in the facade) | `vhco-contract.json`, `0d6c2ea` |
| 2026-09-28 | Decision | TASK-052 / R17 | Token model: `col`/`len` count characters (not bytes), 1-based; every token lies on one line (multi-line backtick strings split per line); brackets, commas, colons, whitespace and object keys are not tokens; `true`/`false`/`null`, built-ins after `(` and statement keywords are `keyword`; an assignment's effect head and its verb (`http get`) are `effect`; names declared by `global` are `global` wherever referenced (`retry_on.0`); `(ID …)` calls are `operation_id`; the use case returns `(tokens, Option<error>)`, cut at the earliest diagnostic | Stable goldens | P4 (D-14, D-16) | `tests/fixtures/highlight/` |
| 2026-09-28 | Finding | TASK-052 / P2c | `highlight_source.rs` embeds `editors/keywords.json` with `include_str!` (one table for the grammar and the CLI). P2c's packaging (`cargo package --list`, T-10) must keep `editors/keywords.json` in the crate if `include`/`exclude` is set | Packaging constraint | P2c (TASK-030, TASK-036) | — |
| 2026-09-28 | Finding | TASK-052 | Capy keeps no node for the `end` of a flat option line's sub-block (`transport command …` ⏎ `args …` ⏎ `end` in demo 13): the highlighter's gap scan classifies uncovered `end` and `else` words; `rivet check` accepts the form as before (not a defect of this phase) | Highlight complete | — | T-14 corpus test |
| 2026-09-28 | Finding | INC-2026-0009 | Resolved in `93388c1`: numeric path segments (`xs.0`, `m.rows.1.0`, `${xs.0}`, a global's `retry_on.0`) parse and evaluate; a keyword statement whose value fails is `syntax.expression` "the expression after `KEYWORD` does not parse" (never `assign_map`). 4 tests (3 `conformance_language` + 1 unit); moved to `docs/incidents/resolved/` (`6af5c27`). No v0.2.0 Known Issue needed; P4 should add a list-index example (REF/MAN) | Incident closed | P4 (MAN-2026-0003, REF-2026-0002) | `93388c1`, `6af5c27` |
| 2026-09-28 | Finding | P2e | 455 tests pass (444 after INC-2026-0009 + 3 unit + 8 `conformance_highlight`); fmt, clippy `-D warnings`, `vhco validate`, `vhco check` green; `vhco sync` = 3 gaps, all P2c/P2d (`CapabilityReport`, `FfiOptions`, surface `ffi`) | P2e exit met | — | `9ec2a39`, `0d6c2ea` |
| 2026-09-29 | Deviation | TASK-031 / R10 | Internals are `#[doc(hidden)] pub mod internal` in `src/internal.rs`, whose `#[path]` modules keep the five folders unchanged, and `pub(crate) use internal::{domain, …}` keeps every `crate::domain::…` path. `pub(crate)` modules alone cannot be re-exported to the tests (E0365), and an inline `mod internal { #[path = "../domain/mod.rs"] … }` resolves below a non-existent `src/internal/` | vhco validate/sync unaffected; `cargo package --list` ships `src/internal.rs` | — | `10c8130` |
| 2026-09-29 | Finding | TASK-031 / MIG-2026-0001 | Breaking Rust API changes for the migration guide: the package is `rivet-runtime` (`rivet = { package = "rivet-runtime", … }`; `use rivet::…` unchanged); `rivet::{domain, features, infra, io, orchestrator}` moved to the hidden `rivet::internal::…` (e.g. `rivet::domain::Value` → `rivet::Value`, `rivet::domain::RivetError` → `rivet::Error`, `rivet::domain::policy::Policy` → `rivet::Policy`, `rivet::domain::envelope::InputEnvelope` → `rivet::InputEnvelope`, `rivet::domain::io_manifest::IoQuery` → `rivet::types::IoQuery`); the `rivet` binary needs `--features cli` (`cargo install rivet-runtime --features cli`); `BuildProbe` gains `build_features`, `abi_version`; `rivet.capabilities` data gains `build_features`, `abi_version`; `remote_cli::receives_of` moved to `setup_library` (re-exported) | P4 (D-09, D-26, D-43) | — | `10c8130` |
| 2026-09-29 | Decision | TASK-032 / R11 | `unsupported.feature` is raised at load (before anything runs) for: a `grpc` connector or effect (`grpc`), a `quic` scope and any HTTP/3 request, including `version prefer [3, …]` (`quic`), and an `auth NAME oauth2` profile (`oauth`); every use is reported (first + suppressed, source order). `cli` does not imply `serve`: a CLI without `serve` keeps `rivet serve` and refuses it (exit 5). The protocol rows of `rivet.capabilities` and `serve.surfaces` follow the build. `rivet-ffi` passes the runtime features through (default = the runtime defaults) | Lean builds refuse instead of silently degrading | P4 (D-27, D-26) | T-10 |
| 2026-09-29 | Finding | P2c | The P2c commit (`10c8130`) left `vhco sync` at 2 gaps, both owned by P2d and present since P1 (`FfiOptions`, surface `ffi`); `CapabilityReport` closed there. P2d (`f9af92a`) brings `sync` to 0 | Sync 3 → 2 → 0 | — | — |
| 2026-09-29 | Finding | TASK-034 | `editors/vscode/package_vsix.py` read `[package].version` and failed on the workspace manifest (`t13_vsix…` red), as the P2e finding anticipated; it now reads `[workspace.package]` first. Not an incident: caught by the suite in the same change | — | — | `10c8130` |
| 2026-09-29 | Finding | TASK-035 / R12 | `cargo publish --dry-run -p rivet-runtime` fails on the git dependency: "all dependencies must have a version requirement specified when publishing. dependency `capy-core` does not specify a version" (expected until G-PUB) | Git dependency remains the supported path | Maintainer (G-PUB) | TASK-038 |
| 2026-09-29 | Decision | TASK-040 / R15 | A call handle is a session of the shared session driver opened `connection_owned` (not counted in the 8 sessions per principal); the 60 s idle lease still applies, so a call not read for 60 s is cancelled; `rivet_call_next(timeout)` long-polls in ≤ 5 s slices (negative = wait); `rivet_call_cancel` runs sessions.cancel_session and execution.cancel_request; records may be pretty (each is its own string, not NDJSON) | Documented FFI behaviour | P4 (MAN-2026-0009, API-2026-0007) | T-11 |
| 2026-09-29 | Deviation | TASK-041 / R13, R14 | Compared with the proposal's illustrative prototypes: `crate-type` adds `rlib` (so `ffi/tests` call the exported functions); `rivet_runtime_free`, `rivet_call_cancel`, `rivet_call_free`, `rivet_module_free` and `rivet_string_free` return `RivetStatus` (void in the sketch) so a double free is observable (`RIVET_ERROR`); `rivet_call_start`/`rivet_module_call_start` never return NULL (a failed start yields its error record, then NULL); a NULL `data_json` means `{}`; `rivet_module_operations` returns `[{id, operation, name, description, emits, receives}]`; `rivet_highlight` json output ends with an error-envelope line after a syntax error | ABI 1 as shipped in `ffi/include/rivet.h` | P4 (API-2026-0007) | T-11, T-12 |
| 2026-09-29 | Finding | TASK-042 | Static linking on macOS prints `ld: warning: object file … was built for newer 'macOS' version (26.4) than being linked (26.0)` for ring's assembly when librivet is built without `MACOSX_DEPLOYMENT_TARGET` (RES-2026-0004 E2); the link and the programs are correct. Release artifacts (P5) must build librivet and link C with one `MACOSX_DEPLOYMENT_TARGET` | Noise only | P5 (TASK-093) | examples/c/Makefile |
| 2026-09-29 | Finding | TASK-043 | `cargo test` leaves the cdylib and staticlib in `target/<profile>/deps` (no uplift to `target/<profile>`); `conformance_ffi` looks there first and needs `cargo test --workspace` (TRBL-2026-0006). The Linux `Libs.private` list (`-lgcc_s -lutil -lrt -lpthread -lm -ldl -lc`) and ASan are unverified locally (no Linux host): the first CI run checks the list | Linux static link verified by CI | P3 (TASK-060, TASK-064) | TRBL-2026-0006 |
| 2026-09-29 | Finding | P2c, P2d | 476 tests pass (455 + 3 `conformance_features` + 1 `FfiOptions` unit + 2 `setup_ffi` unit + 8 `ffi_safety` + 7 `conformance_ffi`); fmt, clippy `--workspace --all-targets --all-features -D warnings`, feature matrix, `vhco validate`, `vhco check` green; `vhco sync` 0; check_docs 0 problems; `vhco docs check` 0 errors. Parallel uncommitted edits by another agent (STD-2026-0001, the contract `overview.summary`, README and docs indexes) were left unstaged; the contract was staged hunk by hunk | P2c and P2d exits met | — | `10c8130`, `f9af92a` |

## Rollout Strategy

1. Local release, as with v0.1.0: tag `v0.2.0` plus `dist/v0.2.0/` artifacts with checksums.
2. Once a remote exists:
   1. Push `main`, `v0.1.0` and `v0.2.0`.
   2. Consumers add `rivet = { package = "rivet-runtime", git = …, tag = "v0.2.0" }`.
   3. C hosts download `librivet` and `rivet.h`.
   4. Authors install the `.vsix`.
3. The migration window runs through 0.2.x: legacy input is accepted with deprecation signals. Operators watch
   `deprecated=1` in the access log before upgrading to 0.3.0.

## Rollback Strategy

| Scenario | Action |
|---|---|
| A client breaks on the new output | Pin `v0.1.0` (MIG-2026-0001 rollback section); the input aliases do not help output consumers |
| An FFI defect in the field | Mark `librivet` experimental in a 0.2.x patch; Rust and CLI unaffected |
| The feature split breaks an embedder | Use the default features (all runtime features on) |
| A release defect | Patch release 0.2.1 through the same gates; never move a tag |

## Risks

| Risk | Likelihood | Impact | Mitigation | Owner |
|---|---|---|---|---|
| The envelope sweep misses a doc or surface | Medium | High | Grep gate: no `"result":` in `src/`, and D-40…D-63 lists every file; T-01 schema | Implementer |
| Disk exhaustion during builds | High | Medium | TASK-008; sequential agents; `cargo clean` between phases | Implementer |
| The `vhco` workspace is not accepted | Medium | Medium | E1 first; fallback to an `ffi` feature in the main crate (ADR-0005) | Implementer |
| Static-link system library drift per OS | Medium | Low | `native-static-libs` captured into `rivet.pc` | Implementer |
| Node tooling unavailable | Low | Low | Python keyword-drift check is mandatory; Node snapshot job optional | Implementer |
| Hidden behaviour change during the envelope work | Low | High | T-34: all 0.1.0 suites stay green with shape-only edits | Implementer |

## Completion Criteria and Final Traceability

The plan is complete when every in-scope task is DONE (or NOT APPLICABLE/DEFERRED with a reason) and the §34 gate
is walked in REL-0.2.0 (TASK-094). TASK-095/096 may stay BLOCKED on the remote, as with v0.1.0.

| Requirement / UC | Implementation tasks | File changes | Tests | Docs / demo | Release update | Final status |
|---|---|---|---|---|---|---|
| R1 / UC-01, 02 | 010–016, 018 | PF-01–PF-04 | T-01, T-04, T-34 | D-10, D-11, D-40–D-42, D-46–D-63 | U-01 | CODE + TESTS DONE (docs P4, release P5) |
| R2 / UC-01 | 010, 017 | PF-01, PF-02 | T-01 | D-10, D-44 | U-02 | CODE + TESTS DONE (docs P4, release P5) |
| R3 / UC-01 | 011, 012 | PF-03 | T-03 | D-23, D-40 | U-03 | CODE + TESTS DONE (docs P4, release P5) |
| R4 / UC-01 | 010–015, 019 | PF-01, PF-03, PF-T07 | T-02 | D-10, D-23, D-40–D-42, D-50, D-59 | U-04 | CODE + TESTS DONE (docs P4, release P5) |
| R5 / UC-09 | 010–012, 014 | PF-01, PF-03 | T-15 | D-09, D-44 | U-05 | CODE + TESTS DONE (docs P4, release P5) |
| R6 / UC-03 | 010–012 | PF-01, PF-03 | T-05 | D-10, D-23, D-25 | U-06 | CODE + TESTS DONE (docs P4, release P5) |
| R7 / UC-04 | 020–022 | PF-05–PF-07 | T-06, T-29 | D-17, D-22, D-45 | U-07 | CODE + TESTS DONE (docs P4, release P5) |
| R8 / UC-04 | 021, 022 | PF-06 | T-07 | D-22, D-44 | U-08 | CODE + TESTS DONE (docs P4, release P5) |
| R9 / UC-04 | 023 | PF-08 | T-08 | D-24, D-33 | U-09 | CODE + TESTS DONE (docs P4, release P5) |
| R10 / UC-05 | 030, 031, 033 | PF-09, PF-10, PF-17 | T-09 | D-26, D-43, D-61 | U-10 | CODE + TESTS DONE (docs P4, release P5) |
| R11 / UC-05 | 032, 034 | PF-10, PF-16 | T-10 | D-03, D-27, D-35 | U-11 | CODE + TESTS DONE (docs P4, release P5) |
| R12 / UC-05 | 030, 035, 038 | PF-10 | T-10 | D-03, D-26 | U-12 | CODE DONE; publication DEFERRED (G-PUB) |
| R13 / UC-06 | 040–042, 044 | PF-11, PF-12, PF-17 | T-11 | D-12, D-13, D-15, D-18 | U-13 | CODE + TESTS DONE (docs P4, release P5) |
| R14 / UC-06 | 040, 043 | PF-11, PF-12 | T-12 | D-12, D-37 | U-14 | CODE + TESTS DONE (docs P4, release P5) |
| R15 / UC-06 | 040, 042 | PF-11, PF-17 | T-11 | D-12, D-13 | U-15 | CODE + TESTS DONE (docs P4, release P5) |
| R16 / UC-07 | 050, 051, 053 | PF-14, PF-G02 | T-13 | D-14, D-16, D-19, D-69 | U-16 | CODE + TESTS DONE (P2e); docs P4 |
| R17 / UC-08 | 052, 053 | PF-13 | T-14 | D-14, D-16, D-23 | U-17 | CODE + TESTS DONE (P2e); docs P4 |
| R18 / all | 070–082 | docs | T-30, T-31, T-32 | D-01…D-72 | U-18 | NOT STARTED |
| R19 / UC-10 | 100, 101, 102 | PF-19, PF-20 | T-16 | D-22, D-45, D-71 | U-19 | CODE + TESTS DONE (docs P4, release P5) |
| R20 / UC-10 | 102, 103 | PF-20 | T-16 | D-22, D-31, D-71 | U-20 | CODE + TESTS DONE (docs P4, release P5) |
| R21 / UC-10 | 102 | PF-20 | T-17 | D-22, D-44 | U-21 | CODE + TESTS DONE (docs P4, release P5) |
| R22 / UC-11 | 105, 106 | PF-22, PF-17 | T-18 | D-26, D-43, D-71 | U-22 | CODE + TESTS DONE (docs P4, release P5) |
| R23 / UC-11 | 045 | PF-11, PF-12, PF-17 | T-19 | D-12, D-13, D-15, D-71 | U-23 | CODE + TESTS DONE (docs P4, release P5) |
| R24 / UC-10, UC-11 | 102, 104 | PF-21 | T-20 | D-24, D-33, D-37 | U-24 | CODE + TESTS DONE (docs P4, release P5) |

```text
 R-n ──► TASK-0xx ──► PF-xx ──► T-nn (TEST-2026-00nn) ──► D-xx (docs/demo) ──► U-nn (DEMO-2026-0020) ──► REL-0.2.0
```

## Post-Implementation Review

To be written at P5: what the experiments changed, the envelope sweep cost, incidents found, and what to carry
into 0.3.0 (alias removal, G-PUB, tree-sitter).

## Related Documents

- [PROP-2026-0002](../proposals/approved/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md) · [ADR-0004](../decisions/adr-0004-approve-envelopes-globals-library-ffi-highlighting.md)
- [PLAN-2026-0001](plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) · [REL-0.1.0](../releases/rel-0.1.0-release-notes.md) · [RPT-2026-0001](../reports/rpt-2026-0001-validation-of-plan-2026-0001.md)
- [DOCUMENTATION.md](../../DOCUMENTATION.md) · [AGENTS.md](../../AGENTS.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 7 | 2026-09-29 | Claude | P2c and P2d done: TASK-030…035 (`10c8130`) and TASK-040…045 (`f9af92a`) DONE with evidence; phase rows P2c/P2d DONE; R10–R15, R23 code and tests done (R12 publication deferred to G-PUB); PF-09–PF-12, PF-T03, PF-T04 DONE, PF-16/PF-17 in progress; T-09–T-12, T-19 PASS (476 tests); findings and deviations recorded (internal module layout, breaking API list, feature refusal rules, ABI deviations, macOS deployment target, deps-dir libraries); TRBL-2026-0006; live status refreshed. |
| 6 | 2026-09-28 | Claude | P2e done: TASK-050…053 DONE (`9ec2a39`, `0d6c2ea`); phase row P2e DONE; R16/R17 code and tests done; PF-13, PF-14, PF-T05, PF-G02 DONE; T-13, T-14 PASS (455 tests); INC-2026-0009 resolved (`93388c1`, `6af5c27`); deviations (Python `.vsix` packager and TextMate engine instead of Node tools; library trigger contract edit) and findings recorded; live status refreshed. |
| 5 | 2026-09-28 | Claude | Live status refreshed after P2b/P2f; git remote added and main + v0.1.0 pushed. |
| 4 | 2026-09-28 | Claude | P2b and P2f done: TASK-020…024 (`673994c`) and TASK-100…107 (`673994c`, `2942066`) DONE with evidence; phase rows P2b/P2f DONE; R7–R9, R19–R22, R24 code and tests done; PF-05–PF-08, PF-19–PF-22, PF-T02, PF-T09 DONE, PF-17 in progress; T-06–T-08, T-16–T-18, T-20 PASS (440 tests); findings and deviations recorded (contract refinements, R9 sample, snapshot semantics, Module stream signature, P2d hand-off, API changes, INC-2026-0009, TRBL-2026-0005). |
| 3 | 2026-09-28 | Claude | P1 and P2a done: TASK-004…TASK-019 DONE with evidence (commits `ef8e053`, `3e4560f`, `c3b5565`, `f7d2907`), phase rows P1/P2a DONE, R1–R6 code and tests done, T-01–T-05/T-15/T-34 PASS (409 tests), PF-01–PF-04/PF-15/PF-T01/PF-T06–PF-T08 and D-03/D-04/D-06/D-11 DONE; findings and deviations recorded (vhco sync gaps for later phases, polling bodies, SSE event names, MCP isError, Node.js absent). |
| 2 | 2026-09-28 | Claude | Scope amendment (PROP-2026-0002 rev 3, ADR-0004 rev 2): P2f file modules (TASK-100–107), TASK-045 module ABI, R19–R24, T-16–T-20 (TEST-2026-0049…0053), demo 17-modules (DEMO-2026-0019), release guide → DEMO-2026-0020, documentation rows updated (D-12, D-13, D-15, D-20, D-22–D-24, D-26, D-31, D-33, D-37, D-43–D-45, D-64, D-71, D-72). |
| 1 | 2026-09-28 | Claude | Created from approved PROP-2026-0002 (ADR-0004): P1–P5, 65 tasks, file/test checklists, and a complete documentation checklist D-01…D-70 including the 62-file envelope sweep. |
