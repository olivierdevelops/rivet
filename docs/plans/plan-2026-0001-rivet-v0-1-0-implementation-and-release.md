---
document_id: PLAN-2026-0001
title: "Rivet v0.1.0 implementation, validation and release"
document_type: plan
status: draft
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
start_date: null            # set when P1 exits (all approval gates closed)
target_date: null           # estimated after G-SPIKE and the sandbox spike (P1), per PROP-2026-0001 "Plan Strategy"
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, registry, execution, files, connectors, audit, policy, auth, datagrams, quic, grpc, sessions, serve, transports, cli, http, ws, poll, mcp, library, documentation]
affected_versions:
  from: not-applicable
  to: "0.1.0"
applicable_environments: [development, embedded, server]
audience: [maintainers, implementers, reviewers]
scope: Everything required to implement PROP-2026-0001 revision 6 (Stages A and B, R1–R26, UC-01–UC-22, C-01–C-24), test and validate it, write every user, operator, system and release document, and release v0.1.0.
reason: The maintainer asked for a plan to implement, validate and release Rivet following DOCUMENTATION.md, including every documentation deliverable (demos, manuals, system docs and the rest).
dependencies: [PROP-2026-0001, REF-2026-0001, REF-2026-0002, vhco-contract.json, AGENTS.md, DOCUMENTATION.md, PROJECT.md]
related_documents: [PROP-2026-0001, REF-2026-0001, REF-2026-0002, DEMO-2026-0013]
supersedes: null
superseded_by: null
tags: [rivet, plan, implementation, validation, release, documentation]
confidentiality: internal
review_cycle: on-change
next_review_date: 2026-10-28
---

# Rivet v0.1.0 implementation, validation and release

> **Status:** Draft — cannot become `approved` until the P1 gates close (DOCUMENTATION §24)
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** not-applicable → 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** every Rivet feature, surface and adapter; all documentation directories

## Summary

This is the live execution ledger for turning the approved design in
[PROP-2026-0001](../proposals/draft/prop-2026-0001-rivet-runtime.md) (revision 6) into the released Rust crate and
`rivet` binary **v0.1.0**. It covers five gated phases:

1. approval and discovery;
2. implementation;
3. tests and validation;
4. documentation and demos;
5. release.

Every requirement is traced to tasks, files, tests, documentation and a release verification row. The plan
follows the lifecycle chain of [DOCUMENTATION.md §3.2](../../DOCUMENTATION.md#32-canonical-lifecycle-chain) and the
release workflow of [§23–§34](../../DOCUMENTATION.md#23-release-workflow). It also follows the VHCO loop in
[AGENTS.md](../../AGENTS.md#the-loop--do-this-for-every-feature-or-change): contract, then evaluation, code, tests,
validation and docs.

All work in this plan is **NOT STARTED**. No code exists and no gate has passed. The repository is not yet a Git
repository, which the release needs (TASK-007).

```text
 PROP-2026-0001 rev 6 ──► ADR-0001 (approval) ──► PLAN-2026-0001 (this) ──► src/ + tests/
        │                                                                     │
        │     RES spikes ──► ADR-0002/0003                                    ▼
        │                                                    TEST-2026-0001…0030 (testing/)
        │                                                                     │
        │                                                                     ▼
        │                                                    RPT-2026-0001 validation (reports/)
        │                                                                     │
        ▼                                                                     ▼
 REF-2026-0002 (153 examples) ─────────────────────────────► DEMO-2026-0014 release verification
 docs/demos/01…12 samples ──────────────────────────────────► (samples re-verified, status active)
                                                                              │
                                                                              ▼
                                           MAN-2026-0001…0008 manuals · SYS · ARCH · API · SEC · OPS · RUN · ONB
                                                                              │
                                                                              ▼
                                  Cargo.toml 0.1.0 ─► release commit ─► tag v0.1.0 ─► REL-0.1.0 release notes
```

## Objective, Scope and Proposal Baseline

**Objective.** Ship Rivet v0.1.0. It is a Rust library and `rivet` binary that:
- parses `.rivet` bundles with Capy;
- publishes one described operation catalog over CLI, HTTP (REST/SSE/polling), WebSocket, MCP and the Rust
  library;
- enforces `policy.json`;
- generates I/O manifests and policy drafts;
- runs every Stage A and Stage B protocol required by the proposal.

It ships with the full current-state documentation set required by DOCUMENTATION.md.

**Proposal baseline.** PROP-2026-0001 **revision 6** (2026-09-28), with the hand-authored `vhco-contract.json`
matching it. Revision 7 would add option-derived file sites and `io --needs`, and only if TASK-005 approves it.

| Owned by this plan | IDs |
|---|---|
| Goals | G-01 – G-06 |
| Requirements | R1 – R26 |
| Use cases | UC-01 – UC-22 |
| Proposed changes | C-01 – C-24 |
| Proposal file rows | F-01 – F-37 (mapped to this plan's PF-IDs below) |
| Proposal tests | T-01 – T-26 (kept with the same IDs; plan-level checks are T-27 – T-33) |

**Not owned by this plan.** These are deferred to a future plan (PLAN-2026-0002, not yet written). The proposal's
delivery matrix labels them Stage C, which is optional:
- named pipes/FIFO and file watching;
- mTLS TCP (S24);
- interactive binary processes;
- generic custom codecs;
- reconnect/resumable delivery.

The design does not require a native gRPC *server* surface, gRPC-Web or legacy MCP HTTP+SSE, and they are not
planned. Any Stage C example among S01–S153 stays documented as Stage C and is excluded from release verification.

```text
            v0.1.0 scope
 ┌──────────────────────────────────────────────────────────────────────────┐
 │ Stage A  language · catalog · outputs · policy.json · broker · files     │
 │          HTTP client · SSE/JSONL · TCP/Unix/WS client · processes · DAG  │
 │          audit/trace · I/O manifest · policy generate · MCP client       │
 │          CLI · library · serve (REST/SSE/poll/WS/MCP) · sessions         │
 │ Stage B  OAuth 2.0 (3 flows) · UDP unicast/bind/multicast · QUIC v1      │
 │          HTTP/3 · gRPC (4 modes) · duplex sessions on every surface      │
 └──────────────────────────────────────────────────────────────────────────┘
   outside: Stage C adapters → PLAN-2026-0002 (future)
```

## Live Status Summary

| State | Count | Notes |
|---|---:|---|
| NOT STARTED | 96 | Every task |
| IN PROGRESS | 0 | |
| BLOCKED | 0 | P2+ is gated on P1 by entry criteria, not recorded as BLOCKED |
| DONE | 0 | |
| FAILED | 0 | |
| DEFERRED | 0 | Stage C is out of scope, not deferred inside this plan |

- **Current phase:** P1 (approval and discovery), not started.
- **Next action:** maintainer design review for G-DESIGN (TASK-001) and the decision on option-derived file sites
  (TASK-005).
- **Blockers:** G-LIC (Capy relicensing by its owner). No Git repository (TASK-007).
- **Last updated:** 2026-09-28.
- **Release target:** v0.1.0; date set at P1 exit.

## Requirements and Use Cases

Every row restates the proposal's requirement with *why it exists* and *how the plan satisfies it*. Acceptance
criteria are the proposal's, made testable here.

| Req / UC | Why it exists | Planned outcome (how it is satisfied) | Acceptance criteria | Owning phase | Status |
|---|---|---|---|---|---|
| R1 / UC-01, UC-10 | UQ-09/10: Rust library using Capy | `capy_parser.rs` is the only Capy importer. It converts `ParseResult` into Rivet's `SyntaxTree`, and the library builds without a Capy executable | T-01, T-23 green. `cargo tree` shows `capy-core` pinned to the recorded commit | P2a | NOT STARTED |
| R2 / UC-01 | UQ-03: simple, protocol-visible syntax | `rivet.capy` grammar for every statement shape in REF-2026-0002; diagnostics carry file/line/column | All S01–S153 and demo `.rivet` files parse; invalid samples produce exact spans | P2a | NOT STARTED |
| R3 / UC-03, UC-05, UC-10 | UQ-02: context closes resources | `scope_supervisor` owns every handle and joins cleanup on success, error, break and cancel | T-03, T-05, T-10: zero live handles/tasks/processes after the grace period | P2a, P2c | NOT STARTED |
| R4 / UC-02, UC-03, UC-05, UC-07 | UQ-03: good errors | `domain/errors.rs` registry (code→kind→HTTP→exit→retryable); one terminal event | T-24: every registry code checked on every surface | P2a | NOT STARTED |
| R5 / UC-04 | UQ-04: file CRUD | `apply_file_operation` + `file_access` with no-follow handles, version guards and hard-link refusal | T-04 | P2a | NOT STARTED |
| R6 / UC-09, UC-21 | UQ-05/18: list all I/O and trace attempts | Effect graph → IoManifest (targets and access verbs) → views/formats/check-policy; `effect_id` on trace attempts | T-09, T-25 | P2c | NOT STARTED |
| R7 / UC-06 | UQ-06: MCP bridge | `invoke_mcp` + `mcp_client` (stdio and Streamable HTTP), pinned snapshots | T-06 | P2c | NOT STARTED |
| R8 / UC-02, UC-10, UC-21 | UQ-07/09: same ops on every surface | One dispatcher; the surface adapters only encode and decode | T-02: identical results/errors from CLI, HTTP, MCP and library | P2a, P2b | NOT STARTED |
| R9 / UC-02, UC-03, UC-10 | UQ-08: request(ID, params, on_data) | `Runtime::request`, `scope.stream`, `next() -> Result<Option<Envelope>>` | T-03, T-10 | P2a | NOT STARTED |
| R10 / UC-07 | UQ-11: DAGs | `run_dag` state machine and `DagCompletion`; default fail fast; depth and concurrency limits | T-07, T-24 | P2c | NOT STARTED |
| R11 / UC-04, 06, 08, 09, 21, 22 | UQ-13/17: deny-by-default | `load_policy` + `policy_broker`: no file means every effect is denied; the host ceiling is intersected with policy.json | T-08, T-21 | P2a | NOT STARTED |
| R12 / UC-05 | UQ-01/12: protocol families | Availability matrix; transports feature (HTTP/socket/process); unsupported stages refuse | T-05, T-08 | P2c | NOT STARTED |
| R13 / UC-05, 08, 09 | PROJECT §§6, 79–80: secrets, no shell | argv-only processes; secrets bound to destinations; redaction | T-05, T-08, T-09 (secret canaries) | P2a, P2c | NOT STARTED |
| R14 / UC-01–22 | UQ-12, AGENTS/DOCUMENTATION | All documents in the Documentation and Demo Checklist; indexes; traceability | T-31, T-32; §34 gate | P4 | NOT STARTED |
| R15 / UC-12 | UQ-14: UDP | `exchange_datagrams` + `udp_adapter` | T-12, T-15 | P2d | NOT STARTED |
| R16 / UC-11 | UQ-14: OAuth 2.0 | auth feature (begin/complete/status/disconnect/acquire/cancel) + `oauth_adapter` + `credential_store` | T-11, T-15 | P2d | NOT STARTED |
| R17 / UC-13 | UQ-14: QUIC | `exchange_quic` + `quic_adapter` | T-13, T-15 | P2d | NOT STARTED |
| R18 / UC-14 | UQ-14: HTTP/3 | `http_adapter` version selection and safe fallback | T-14, T-15 | P2d | NOT STARTED |
| R19 / UC-16 | UQ-15: gRPC | `invoke_rpc` + `grpc_adapter`, all four modes | T-17 | P2d | NOT STARTED |
| R20 / UC-15 | UQ-15: many described operations | Multi-declaration compile; atomic duplicate detection | T-16 | P2a | NOT STARTED |
| R21 / UC-17 | UQ-15: incoming MCP | `io/mcp` direct named tools plus built-ins | T-18 | P2b | NOT STARTED |
| R22 / UC-18, UC-20 | UQ-15/08/17: live streams everywhere | sessions feature + `session_driver`; polling and WS projections | T-19, T-22 | P2b | NOT STARTED |
| R23 / UC-19 | UQ-17: declared outputs | `compile_output_spec`, `validate_output`, `inspect_outputs`; `rivet outputs` and its projections | T-20 | P2a, P2b | NOT STARTED |
| R24 / UC-08, UC-22 | UQ-17: policy.json only | `load_policy` (discovery, `--policy PATH`, schema v1, `access`); `--sandbox` never implemented | T-21 | P2a | NOT STARTED |
| R25 / UC-20 | UQ-17: one serve, all surfaces | `start_serve`, `authenticate_principal`, `authorize_operation`, `multiplex_ws`, `project_polling` | T-22 | P2b | NOT STARTED |
| R26 / UC-21, UC-22 | UQ-18: generated I/O manifest and policy draft | `inspect_effects` (manifest), `generate_policy`, `policy_draft_writer` | T-25, T-26 | P2c | NOT STARTED |

## Applicable Project Standards

| Rule | Source | Plan impact | Validation |
|---|---|---|---|
| Contract before code; human evaluation | AGENTS golden rule 1, loop steps 1–2 | P1 exit requires G-CONTRACT; any design change during P2 goes back to the contract first | TASK-002 evidence; `vhco sync .` |
| Five folders, closed-world imports | AGENTS "The five folders" | All code in `src/{domain,features,io,infra,orchestrator}`; only `orchestrator` composes | `vhco validate .` green after every task (T-28) |
| Pure, surface-blind use cases; every I/O through a port | AGENTS golden rules 4–5 | Adapters live only in `infra/`; use cases take ports as parameters | Code review + `vhco validate .` |
| Annotate everything; todos and steps spell out the logic | AGENTS golden rules 6–7 | Every file carries `// vhco:` annotations; every contract todo is claimed | `vhco sync .` = 0 (T-28) |
| Test each use case; iterate until green | AGENTS golden rule 8 | Colocated `<use_case>_test.rs` with `// vhco:test` plus conformance suites | T-01–T-27 |
| README and docs/ current | AGENTS golden rule 9; DOCUMENTATION §31 | P4 documentation tasks; the six impact decisions are recorded | T-31, REL-0.1.0 |
| Plan is a live ledger | DOCUMENTATION §4.5 | Only the seven status values; evidence column filled on DONE/FAILED/BLOCKED/DEFERRED | Plan review each phase exit |
| Tests and validation recorded | DOCUMENTATION §27–§28 | TEST-2026-0001…0033; RPT-2026-0001 | T-32 |
| Release verification guide and demo executed | DOCUMENTATION §29 | DEMO-2026-0014 + 12 sample folders executed against v0.1.0 | T-30 |
| Manual as the current-state book | DOCUMENTATION §30 | MAN-2026-0001…0008 with a feature catalogue | T-31 |
| One version source; tag = commit | DOCUMENTATION §32 | `Cargo.toml` is canonical; tag `v0.1.0` | T-33 |
| Release completion gate | DOCUMENTATION §34 | Checked item by item in TASK-096 | TASK-096 evidence |
| Lots of ASCII visuals | CLAUDE.md; AGENTS "Document Categories" | Every manual, system, API and architecture document includes journey/sequence/state diagrams | Documentation review (T-31) |

## Measurable Claims

PROP-2026-0001 makes **no performance or optimization claim**; its defaults are stated as "proposed safety
defaults, not performance claims". Resource *bounds* are correctness properties, tested as pass/fail and not
benchmarked:

| Measurement | Baseline method | Test | Controlled environment | Acceptance threshold | Report destination |
|---|---|---|---|---|---|
| Stream queue bound | Not applicable — bound, not optimization | T-03 slow-consumer fixture | CI Linux runner, fixed seed | Never more than 16 buffered frames per request | TEST-2026-0003 |
| Global buffered bytes | Not applicable | T-24 | CI Linux runner | Never more than `limits.max_buffered_bytes` (default 256 MiB); excess gives `limit.*` | TEST-2026-0024 |
| Concurrency and depth limits | Not applicable | T-24 | CI Linux runner | 65th concurrent nested call → `limit.*`; depth 17 → `limit.call_depth` | TEST-2026-0024 |
| Cleanup grace | Not applicable | T-05 | CI Linux/macOS/Windows | All owned resources closed within 5 s grace | TEST-2026-0005 |

Any future throughput or latency claim requires a proposal amendment with a baseline, threshold and report
destination before measuring (DOCUMENTATION §21.3).

## Prerequisites

| # | Prerequisite | Why | Task |
|---|---|---|---|
| 1 | G-DESIGN: the maintainer accepts PROP-2026-0001 rev 6 (or a revised revision) | No code before approval | TASK-001 |
| 2 | G-CONTRACT: the maintainer reviews `vhco live vhco-contract.json` | AGENTS loop step 2 | TASK-002 |
| 3 | G-LIC: Capy `LICENSE` relicensed to match `Cargo.toml` MIT (owner = project user) | Bundling a source-available crate is prohibited | TASK-003 |
| 4 | G-SPIKE: pinned Capy parses every S01–S153 example and every demo `.rivet` file | Every other design decision depends on it | TASK-010 |
| 5 | Git repository initialized; remote chosen | §32 release commit and tag | TASK-007 |
| 6 | Rust toolchain ≥ 1.85 (edition 2024 async closures per PROP E11), `vhco` CLI installed, `protoc` for gRPC fixtures | Build, validation, fixture generation | TASK-008 |
| 7 | Contract ↔ proposal reconciliation (see Decisions table) | The contract is the code's source of truth | TASK-006 |

## Phases, Entry Gates and Exit Gates

```text
 P1 approve+discover ─► P2a foundation ─► P2b surfaces ─┬─► P3 test+validate ─► P4 docs+demos ─► P5 release
                         (Stage A core)   + serve        │
                                   └────► P2c Stage A ───┤
                                          effects        │
                                          P2d Stage B ───┘
                                          protocols
 Every P2 task: contract first → code → colocated test → vhco validate → vhco sync (AGENTS loop)
```

| Phase | Purpose | Entry criteria | Exit criteria | Depends on | Status |
|---|---|---|---|---|---|
| P1 | Approvals, discovery spikes, decisions, repo setup | This plan exists | G-DESIGN, G-CONTRACT, G-LIC, G-SPIKE closed. ADR-0001–0003 accepted. Git initialized. Plan status → `approved` with start/target dates | — | NOT STARTED |
| P2a | Foundation: crate, domain, compiler, registry, outputs, policy, broker, execution, files, CLI, library | P1 exit | `cargo build`/`test` green for P2a suites. `vhco validate .` green. `sync` shrinking | P1 | NOT STARTED |
| P2b | Surfaces: HTTP/SSE/polling/WS/MCP server, unified serve, principals, sessions | P2a exit | T-02, T-18, T-19, T-20, T-22 green | P2a | NOT STARTED |
| P2c | Stage A effects: transports, DAG, audit/trace, I/O manifest, policy generate, MCP client | P2a exit (runs alongside P2b) | T-03–T-07, T-09, T-24–T-26 green | P2a | NOT STARTED |
| P2d | Stage B: OAuth, UDP, QUIC, HTTP/3, gRPC | P2c transports done | T-11–T-17 green | P2c | NOT STARTED |
| P3 | Full test and validation pass; TEST docs; validation report | P2b, P2c and P2d exit | T-01–T-29 recorded PASS (or documented PARTIAL/FAIL); `vhco sync .` = 0; RPT-2026-0001 written | P2* | NOT STARTED |
| P4 | Documentation and demos | P3 exit (stable behaviour) | Every Documentation and Demo Checklist row DONE or NOT APPLICABLE with a reason; demos executed (T-30); docs checks (T-31) | P3 | NOT STARTED |
| P5 | Version, commit, tag, release document, post-release verification | P4 exit; release candidate approved by the maintainer | §34 gate fully checked (TASK-096); tag verified (T-33) | P4 | NOT STARTED |

## Live Work Checklist

Owners: **M** = project maintainer (the human; approvals and reviews). **I** = implementer (agent or engineer).
Every P2 task follows the AGENTS loop: contract todo claimed with `// vhco:todo`, then a colocated test, then
`vhco validate .` and `vhco sync .`.

### P1 — Approval and discovery

| Task | Phase | Description / method | Req / UC / Change IDs | Production files | Test files / manual procedure | Dependencies | Owner | Status | Evidence / result |
|---|---|---|---|---|---|---|---|---|---|
| TASK-001 | P1 | Design review of PROP-2026-0001 rev 6; record G-DESIGN in its Approval table and in ADR-0001 | all | `docs/proposals/draft/prop-2026-0001-rivet-runtime.md` (UPDATE), `docs/decisions/adr-0001-approve-rivet-runtime-design.md` (CREATE) | Manual: maintainer sign-off | — | M | NOT STARTED | |
| TASK-002 | P1 | `vhco live vhco-contract.json --port 7787`; the maintainer audits; changes loop until approved; record G-CONTRACT | all / C-09 | `vhco-contract.json` (UPDATE if changes) | Manual: live review | TASK-006 | M | NOT STARTED | |
| TASK-003 | P1 | Relicense Capy at the pinned commit (or a new pinned commit) so `LICENSE` matches MIT; record the commit hash and licence text | R1 / G-LIC | `docs/references/ref-2026-0001-request-and-evidence.md` (UPDATE licence evidence) | Manual: read the upstream LICENSE at the pinned commit | — | M | NOT STARTED | |
| TASK-004 | P1 | Move the proposal to `docs/proposals/approved/` with status `approved`; fix inbound links; update indexes | R14 | proposal (MOVE), `docs/proposals/index.md`, `docs/proposals/draft/index.md`, `docs/proposals/approved/index.md` (CREATE) | T-31 link check | TASK-001 | I | NOT STARTED | |
| TASK-005 | P1 | Decide on the 2026-09-28 suggestion of option-derived file sites (`tls ca_file`…), `phase`/`requires_existing`/`secret` fields, `io --needs`, `--check-files`. If approved: proposal rev 7, REF-2026-0002 examples, contract, T-25 cases. If declined: NOT APPLICABLE | R6, R26 / UC-21 / C-23 | proposal, REF-2026-0002, `vhco-contract.json`, docs/demos READMEs (UPDATE if approved) | Manual: maintainer decision | TASK-001 | M | NOT STARTED | |
| TASK-006 | P1 | Reconcile contract ↔ proposal. (a) `rivet.auth.cancel` → add `cancel_authorization` to the contract auth feature. (b) Proposal F-35 names `build_io_manifest.rs` but the contract extends `inspect_effects` → align the proposal to the contract. (c) Add the `transports` feature to proposal F-13. (d) Update AGENTS.md "Project facts", which still describe a Go `email_provider` | R14 / C-09 | `vhco-contract.json`, proposal, `AGENTS.md` (UPDATE) | `python -m json.tool`; T-31 | — | I | NOT STARTED | |
| TASK-007 | P1 | `git init`; `.gitignore` (target/, scratch); first commit of the design state; choose a remote (GitHub assumed for CI) | release / §32 | `.gitignore` (CREATE) | `git status` clean | — | M | NOT STARTED | |
| TASK-008 | P1 | Record toolchain: Rust ≥ 1.85 edition 2024, `rustfmt`, `clippy`, `cargo-deny`, `vhco`, `protoc`; write `rust-toolchain.toml` | R1 | `rust-toolchain.toml` (CREATE) | `cargo --version`, `vhco --version` | TASK-007 | I | NOT STARTED | |
| TASK-009 | P1 | Research: crate feasibility. Candidates include `tokio`, `hyper`/`axum`, `reqwest` or a `hyper` client, `tokio-tungstenite`, `quinn`, `h3`/`h3-quinn`, `tonic` + `prost-reflect`, `oauth2`, `rustls`, an MCP SDK (or hand-rolled JSON-RPC), `jsonschema`, `serde_json`, `cap-std` (confined file handles). Check licences and the MSRV | R12, R15–R19 / C-08, C-10–C-14 | `docs/research/res-2026-0002-rust-crate-feasibility.md` (CREATE) | Manual: prototype each crate against its fixture | TASK-008 | I | NOT STARTED | |
| TASK-010 | P1 | **G-SPIKE**: write `src/infra/rivet.capy` v0 and a throwaway harness that parses every `rivet` code block in REF-2026-0002 and every `docs/demos/**/app.rivet`. Record parse failures and grammar changes | R1, R2 / UC-01 / C-01, C-21 | `docs/research/res-2026-0001-capy-grammar-spike.md` (CREATE); spike harness in scratch (not committed) | Harness output: 153 examples + 12 files, 0 failures | TASK-003, TASK-008 | I | NOT STARTED | |
| TASK-011 | P1 | Research: OS sandbox backends for sandboxed `allow_exec`/stdio MCP (Linux landlock+seccomp; macOS sandbox-exec/Seatbelt; Windows AppContainer/Job objects). Choose exact `src/infra/sandbox_<platform>.rs` paths (proposal F-14, OQ-1) | R11, R12 / C-05, C-08 | `docs/research/res-2026-0003-process-sandbox-backends.md` (CREATE) | Manual: prototype confinement tests per OS | TASK-008 | I | NOT STARTED | |
| TASK-012 | P1 | ADRs from the research: ADR-0002 crate selection; ADR-0003 sandbox backends per platform (a platform without a passing backend refuses sandboxed exec with `unsupported.sandbox_backend`) | R11, R12 | `docs/decisions/adr-0002-rust-crate-selection.md`, `docs/decisions/adr-0003-process-sandbox-backends.md`, `docs/decisions/index.md` (CREATE) | M review | TASK-009, TASK-011 | M | NOT STARTED | |
| TASK-013 | P1 | Plan approval: set `start_date`/`target_date` from the spike results; status `approved`; add the plan map to proposal "Plan Strategy" | all | this plan, proposal (UPDATE) | M sign-off | TASK-001–012 | M | NOT STARTED | |

### P2a — Foundation (Stage A core)

| Task | Phase | Description / method | Req / UC / Change IDs | Production files | Test files / manual procedure | Dependencies | Owner | Status | Evidence / result |
|---|---|---|---|---|---|---|---|---|---|
| TASK-014 | P2a | Crate skeleton: `[lib] path = "src/orchestrator/lib.rs"`, `[[bin]] rivet = src/orchestrator/main.rs`, `version = "0.1.0-dev"`, pinned `capy-core` git rev, crates from ADR-0002 | R1 / C-01 | `Cargo.toml`, `Cargo.lock`, `src/orchestrator/lib.rs`, `src/orchestrator/main.rs` (CREATE) | T-27 build | TASK-013 | I | NOT STARTED | |
| TASK-015 | P2a | Domain data: contracts, error registry, syntax tree, outputs, policy, IoManifest, sessions, serve, DAG. No internal imports | R1–R26 / C-01–C-24 | `src/domain/mod.rs`, `contracts.rs`, `errors.rs`, `syntax_tree.rs`, `outputs.rs`, `policy.rs`, `io_manifest.rs`, `sessions.rs`, `serve.rs`, `dag.rs` (CREATE) | colocated `src/domain/*_test.rs` (serde round-trips; error-registry table completeness) | TASK-014 | I | NOT STARTED | |
| TASK-016 | P2a | Grammar and parser adapter: `rivet.capy` (every statement shape); `capy_parser.rs` is the only Capy import and returns a `SyntaxTree` with spans | R1, R2 / UC-01 / C-01, C-21 | `src/infra/rivet.capy`, `src/infra/capy_parser.rs`, `src/features/language/ports.rs` (CREATE) | `src/infra/capy_parser_test.rs`; T-01, T-23 | TASK-015 | I | NOT STARTED | |
| TASK-017 | P2a | `compile_program`: lower the SyntaxTree to typed IR. Covers prefix calls, quoted durations, interpolation, component-aware URL templates, the leading-options rule, `yield`/`return`, multi-declaration, atomic duplicate IDs, the static call-cycle check | R1, R2, R20 / UC-01, UC-15 / C-01, C-15, C-21 | `src/features/language/compile_program.rs` | `compile_program_test.rs`; T-01, T-16, T-23 | TASK-016 | I | NOT STARTED | |
| TASK-018 | P2a | `compile_output_spec`: `output`/`field`/`error` blocks → OutputSpec (closed objects, `list T`, `open true`); `check --strict-docs` rules | R23 / UC-19 / C-18 | `src/features/language/compile_output_spec.rs` | `compile_output_spec_test.rs`; T-20 | TASK-017 | I | NOT STARTED | |
| TASK-019 | P2a | Registry: `describe_operations`, `inspect_outputs`; immutable catalog; private filtering | R8, R20, R23 / UC-15, UC-19 / C-15, C-18 | `src/features/registry/describe_operations.rs`, `inspect_outputs.rs`, `ports.rs`; `src/infra/registry.rs` | colocated tests; T-16, T-20 | TASK-018 | I | NOT STARTED | |
| TASK-020 | P2a | `load_policy`: discovery beside entry, `--policy PATH`, schema v1 (grants/deny/access/network/limits/serve/approved), deny-by-default when absent, `policy.invalid` | R11, R24 / UC-08 / C-19 | `src/features/policy/load_policy.rs`, `ports.rs`; `src/infra/policy_file_reader.rs` | `load_policy_test.rs`; T-21 | TASK-015 | I | NOT STARTED | |
| TASK-021 | P2a | `authorize_effect` + `policy_broker`: host ceiling ∩ policy.json ∩ request restriction; access-verb check; SSRF private-range denial after DNS; hard links; case folding; per-attempt recheck | R11, R13 / UC-08 / C-05, C-19 | `src/features/policy/authorize_effect.rs`; `src/infra/policy_broker.rs` | `authorize_effect_test.rs`; T-08, T-21 | TASK-020 | I | NOT STARTED | |
| TASK-022 | P2a | Execution core: `request_operation`, `cancel_request`, `validate_output`; `execution_driver`; `scope_supervisor` (reverse-order cleanup, grace, suppressed errors); global limits | R3, R4, R9, R23 / UC-02, UC-03 / C-02, C-03, C-18, C-22 | `src/features/execution/request_operation.rs`, `cancel_request.rs`, `validate_output.rs`, `ports.rs`; `src/infra/execution_driver.rs`, `scope_supervisor.rs` | colocated tests; T-02, T-03, T-24 | TASK-019, TASK-021 | I | NOT STARTED | |
| TASK-023 | P2a | Files: `apply_file_operation` + `file_access` (create exclusive, read, update existing, append, delete, list, stat; no-follow; `file.hardlink_refused`) | R5, R11 / UC-04 / C-04 | `src/features/files/apply_file_operation.rs`, `ports.rs`; `src/infra/file_access.rs` | colocated test; T-04 | TASK-022 | I | NOT STARTED | |
| TASK-024 | P2a | CLI surface: global `--file`, `--policy`, `--json`; subcommands `request`, `list`, `describe`, `outputs`, `check [--strict-docs]`, `io`, `graph`, `policy explain`, `policy generate`, `trace show/export`, `connectors sync`, `serve`; exit codes from the registry; stdout carries only results | R8, R9, R23, R24 / UC-02, UC-19 / C-02, C-18, C-19 | `src/io/cli/mod.rs`; `src/orchestrator/setup_cli.rs` | CLI tests in `tests/conformance_surfaces.rs`; T-02 | TASK-022 | I | NOT STARTED | |
| TASK-025 | P2a | Library surface: `Runtime::builder().source().policy(Policy::from_file)`, `request`, `scope`/`stream`/`duplex`, `outputs`, `io`, `generate_policy`; the runtime owns cleanup of dropped futures | R8, R9 / UC-10 / C-02 | `src/io/library/mod.rs`; `src/orchestrator/setup_library.rs` | T-10 | TASK-022 | I | NOT STARTED | |

### P2b — Surfaces, unified serve and sessions

| Task | Phase | Description / method | Req / UC / Change IDs | Production files | Test files / manual procedure | Dependencies | Owner | Status | Evidence / result |
|---|---|---|---|---|---|---|---|---|---|
| TASK-026 | P2b | Serve core: `start_serve` (single listener, `--listen`, `--stdio`, `serve.surfaces`, `serve.auth_required` on non-loopback), `serve_listener` | R25 / UC-20 / C-20 | `src/features/serve/start_serve.rs`, `ports.rs`; `src/infra/serve_listener.rs`; `src/orchestrator/setup_serve.rs` | colocated test; T-22 | TASK-024 | I | NOT STARTED | |
| TASK-027 | P2b | `authenticate_principal` (none/bearer sha256/mTLS; library `Authenticator`) and `authorize_operation` (`serve.principals`; `rivet.io` and `rivet.policy.generate` only when explicitly listed) | R25, R26 / UC-20 / C-20, C-23 | `src/features/serve/authenticate_principal.rs`, `authorize_operation.rs`; `src/infra/policy_authenticator.rs` | colocated tests; T-22 | TASK-026 | I | NOT STARTED | |
| TASK-028 | P2b | HTTP REST + SSE: `POST /v1/request` (JSON or `Accept: text/event-stream`), `GET /v1/operations[/{id}[/outputs]]`, `GET /v1/io`, `POST /v1/policy/generate`, `traceparent` | R8, R23, R26 / UC-02, UC-19, UC-21 / C-02, C-18, C-23 | `src/io/http/mod.rs`; `src/orchestrator/setup_http.rs` | T-02, T-20, T-22 | TASK-027 | I | NOT STARTED | |
| TASK-029 | P2b | Sessions feature: open/send/finish_input/read/cancel; principal-bound; sequence rules; retention and limits | R22 / UC-18 / C-17 | `src/features/sessions/open_session.rs`, `send_input.rs`, `finish_input.rs`, `read_events.rs`, `cancel_session.rs`, `ports.rs`; `src/infra/session_driver.rs` | colocated tests; T-19 | TASK-022 | I | NOT STARTED | |
| TASK-030 | P2b | Polling projection: `POST /v1/requests`, `GET /v1/requests/{id}/events`, `/input`, `/finish_input`, `/cancel` → sessions | R22, R25 / UC-18, UC-20 / C-17, C-20 | `src/features/serve/project_polling.rs`; `src/io/http/poll.rs` | T-19, T-22 | TASK-028, TASK-029 | I | NOT STARTED | |
| TASK-031 | P2b | WebSocket `/v1/ws` (`rivet.v1`): `multiplex_ws` refs, eight in-flight refs, one terminal frame per ref, cancel on close | R22, R25 / UC-18, UC-20 / C-17, C-20 | `src/features/serve/multiplex_ws.rs`; `src/io/ws/mod.rs`; `src/orchestrator/setup_ws.rs` | T-22 | TASK-029 | I | NOT STARTED | |
| TASK-032 | P2b | MCP server: stdio + Streamable HTTP `/mcp`; direct named tools (title/description/inputSchema/outputSchema); built-ins `rivet.request/list/describe/outputs/io/policy.generate/sessions.*/auth.*`; session delivery `_meta` | R7, R21, R23 / UC-17 / C-16, C-18 | `src/io/mcp/mod.rs`; `src/orchestrator/setup_mcp.rs` | T-18 | TASK-027, TASK-029 | I | NOT STARTED | |

### P2c — Stage A effects, DAG, audit and the I/O manifest

| Task | Phase | Description / method | Req / UC / Change IDs | Production files | Test files / manual procedure | Dependencies | Owner | Status | Evidence / result |
|---|---|---|---|---|---|---|---|---|---|
| TASK-033 | P2c | HTTP client (`exchange_http`, `http_adapter` h1/h2): redirects off by default, `accept status`, decode, component-aware URLs, bound address, safe retries | R12, R4 / UC-05 / C-08 | `src/features/transports/exchange_http.rs`, `ports.rs`; `src/infra/http_adapter.rs` | colocated test; T-05 | TASK-022 | I | NOT STARTED | |
| TASK-034 | P2c | Sockets: TCP (framing), Unix, WebSocket client; SSE/JSONL stream decoding; `codec` | R12 / UC-05 / C-08 | `src/features/transports/exchange_socket.rs`; `src/infra/socket_adapter.rs`, `codec.rs` | T-03, T-05 | TASK-033 | I | NOT STARTED | |
| TASK-035 | P2c | Processes: argv-only `run_process`, env allow-list, reaping; sandboxed exec per ADR-0003 or `unsupported.sandbox_backend` | R12, R13, R11 / UC-05 / C-08 | `src/features/transports/run_process.rs`; `src/infra/process_adapter.rs`; `src/infra/sandbox_<platform>.rs` (exact names from ADR-0003) | T-05, T-08 | TASK-012, TASK-022 | I | NOT STARTED | |
| TASK-036 | P2c | DAG: `run_dag` state machine, fail fast default/fail independent, `map`/`poll`/`iterate`, `DagCompletion`, call-depth/concurrency limits | R10, R4 / UC-07 / C-07, C-22 | `src/features/execution/run_dag.rs` | `run_dag_test.rs`; T-07, T-24 | TASK-022 | I | NOT STARTED | |
| TASK-037 | P2c | Audit/trace: `read_trace`, `trace_store` (bounded, redacted, `effect_id` on attempts), `trace export` needing a write grant | R6, R13 / UC-09 / C-05 | `src/features/audit/read_trace.rs`, `ports.rs`; `src/infra/trace_store.rs` | colocated test; T-09 | TASK-022 | I | NOT STARTED | |
| TASK-038 | P2c | I/O manifest: `inspect_effects` builds the IoManifest (sites, access verbs, capability, normalized targets, knowledge classes, bootstrap list). Adds views `--by`, filters, formats table/json/markdown/csv, `--check-policy`, `--strict`, `--trace` | R6, R26 / UC-09, UC-21 / C-05, C-23 | `src/features/audit/inspect_effects.rs` | `inspect_effects_test.rs`; T-25 | TASK-021, TASK-037 | I | NOT STARTED | |
| TASK-039 | P2c | `generate_policy` + `policy_draft_writer`: one grant per (capability, target), narrowed `access`, origin/glob widening, review list, exit 7, `--output` never overwrites | R26, R24 / UC-22 / C-24 | `src/features/policy/generate_policy.rs`; `src/infra/policy_draft_writer.rs` | `generate_policy_test.rs`; T-26 | TASK-038 | I | NOT STARTED | |
| TASK-040 | P2c | MCP client connectors: `invoke_mcp`, `mcp_client` (stdio, Streamable HTTP), snapshots approved by hash in policy.json, `connectors sync`, recursion hop limits | R7, R11 / UC-06 / C-06 | `src/features/connectors/invoke_mcp.rs`, `ports.rs`; `src/infra/mcp_client.rs` | colocated test; T-06 | TASK-035 | I | NOT STARTED | |

### P2d — Stage B protocols

| Task | Phase | Description / method | Req / UC / Change IDs | Production files | Test files / manual procedure | Dependencies | Owner | Status | Evidence / result |
|---|---|---|---|---|---|---|---|---|---|
| TASK-041 | P2d | OAuth 2.0: begin/complete/status/disconnect/acquire/cancel; client credentials, code+PKCE, device flow (`pending` on deadline); one refresh per key; `refresh_uncertain` | R16 / UC-11 / C-10 | `src/features/auth/begin_authorization.rs`, `complete_authorization.rs`, `credential_status.rs`, `disconnect_account.rs`, `acquire_credential.rs`, `cancel_authorization.rs`, `ports.rs`; `src/infra/oauth_adapter.rs`, `credential_store.rs` | colocated tests; T-11, T-15 | TASK-033, TASK-006 | I | NOT STARTED | |
| TASK-042 | P2d | UDP: unicast, explicit bind, multicast join/leave, truncation detection | R15 / UC-12 / C-11 | `src/features/datagrams/exchange_datagrams.rs`, `ports.rs`; `src/infra/udp_adapter.rs` | colocated test; T-12, T-15 | TASK-022 | I | NOT STARTED | |
| TASK-043 | P2d | QUIC v1: TLS/ALPN, uni/bidi streams, DATAGRAM negotiation, migration disabled unless authorized, no 0-RTT | R17 / UC-13 / C-12 | `src/features/quic/exchange_quic.rs`, `ports.rs`; `src/infra/quic_adapter.rs` | colocated test; T-13, T-15 | TASK-022 | I | NOT STARTED | |
| TASK-044 | P2d | HTTP/3 in `http_adapter`: `version 3` strict; safe fallback only before any request bytes are sent; `response.version` | R18 / UC-14 / C-13 | `src/infra/http_adapter.rs` (UPDATE), `src/features/transports/exchange_http.rs` (UPDATE) | T-14, T-15 | TASK-033, TASK-043 | I | NOT STARTED | |
| TASK-045 | P2d | gRPC: descriptor-driven, four modes, ProtoJSON, metadata/trailers, status mapping, auth metadata | R19 / UC-16 / C-14 | `src/features/grpc/invoke_rpc.rs`, `ports.rs`; `src/infra/grpc_adapter.rs` | colocated test; T-17 | TASK-033, TASK-029 | I | NOT STARTED | |

### P3 — Tests and validation

| Task | Phase | Description / method | Req / UC / Change IDs | Production files | Test files / manual procedure | Dependencies | Owner | Status | Evidence / result |
|---|---|---|---|---|---|---|---|---|---|
| TASK-046 | P3 | Fixture servers: HTTP/h2/h3, WS, TCP/Unix, MCP stdio+HTTP, gRPC (from `docs/demos/10-grpc/schemas/users.proto`), QUIC, UDP multicast, OAuth provider, slow consumer | T-02–T-19 | — | `tests/fixtures/mod.rs`, `http.rs`, `ws.rs`, `socket.rs`, `mcp.rs`, `grpc.rs`, `quic.rs`, `udp.rs`, `oauth.rs`, `consumer.rs` (CREATE) | P2 tasks | I | NOT STARTED | |
| TASK-047 | P3 | Conformance suites T-01–T-26, one file per suite (see Test and Validation Checklist) | R1–R26 | — | `tests/conformance_*.rs` (26 files) | TASK-046 | I | NOT STARTED | |
| TASK-048 | P3 | Sample corpus test: every REF-2026-0002 Stage A/B example and every `docs/demos/*/app.rivet` parses; demo operations run against fixtures | R2, R14 | — | `tests/conformance_samples.rs` (T-29) | TASK-047 | I | NOT STARTED | |
| TASK-049 | P3 | Build/static: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo deny check` (licences, including Capy after G-LIC), `cargo build --release` on Linux/macOS/Windows | R1, R12 | `deny.toml` (CREATE) | T-27 | TASK-047 | I | NOT STARTED | |
| TASK-050 | P3 | VHCO gates: `vhco validate .` green, `vhco sync .` = 0, `vhco check .`, then `vhco spec .` → `vhco.json` (generated; never onto the contract) | R14 / C-09 | `vhco.json`, `vhco.html` (GENERATE) | T-28 | TASK-047 | I | NOT STARTED | |
| TASK-051 | P3 | CI workflow running T-27–T-29 on three OSes | R14 | `.github/workflows/ci.yml` (CREATE; only if the remote is GitHub) | CI run link | TASK-007, TASK-049 | I | NOT STARTED | |
| TASK-052 | P3 | TEST documents (one per suite, definition plus latest result) | R1–R26 | — | `docs/testing/test-2026-00NN-*.md` ×33 (see checklist), `docs/testing/index.md` | TASK-047–051 | I | NOT STARTED | |
| TASK-053 | P3 | Record incidents for unexpected defects found during P2/P3 (template 12.13) | as found | — | `docs/incidents/active/inc-2026-NNNN-*.md`, `docs/incidents/index.md` (CREATE dir + index at first incident; index created now) | — | I | NOT STARTED | |
| TASK-054 | P3 | Record troubleshooting for hard or reusable problems (template 12.9) | as found | — | `docs/troubleshooting/trbl-2026-NNNN-*.md`, `docs/troubleshooting/index.md` | — | I | NOT STARTED | |
| TASK-055 | P3 | Validation report: PASS/PARTIAL/FAIL/NOT APPLICABLE for R1–R26; deviations; unintended behaviour; open incidents; limitations | R1–R26 | — | `docs/reports/rpt-2026-0001-v0-1-0-validation.md`, `docs/reports/index.md` (CREATE) | TASK-052 | I | NOT STARTED | |
| TASK-056 | P3 | Maintainer review of the validation report; any FAIL goes back to P2 | all | — | Manual | TASK-055 | M | NOT STARTED | |

### P4 — Documentation and demos

| Task | Phase | Description / method | Req / UC / Change IDs | Production files | Test files / manual procedure | Dependencies | Owner | Status | Evidence / result |
|---|---|---|---|---|---|---|---|---|---|
| TASK-057 | P4 | Inventory interfaces from code before writing: `vhco spec . --stdout`, `vhco visualize .`, `vhco flow .`, `vhco query . "actions"`, `vhco query . "effects"`, `vhco coverage .`, `vhco doc . --format html`; compare with `rivet --help`, routes and tests (§30) | R14 | — | Coverage gaps list recorded in this plan's Findings | TASK-050 | I | NOT STARTED | |
| TASK-058 | P4 | Architecture document: five folders, ports and adapters, dispatcher, broker, scope supervisor, serve fan-out, data flow | R14 | — | `docs/architecture/arch-2026-0001-rivet-runtime-architecture.md`, `docs/architecture/index.md` | TASK-057 | I | NOT STARTED | |
| TASK-059 | P4 | System documents (template 12.6), current implemented behaviour only (see checklist SYS-0001…0009) | R14 | — | `docs/system/components/…`, `docs/system/configuration/…`, `docs/system/runtime/…`, `docs/system/integrations/…`, `docs/system/index.md` | TASK-057 | I | NOT STARTED | |
| TASK-060 | P4 | API documents (template 12.7): REST/SSE/polling, WebSocket `rivet.v1`, MCP server tools, Rust library, error registry | R8, R21–R26 | — | `docs/api/api-2026-0001…0005-*.md`, `docs/api/index.md` | TASK-057 | I | NOT STARTED | |
| TASK-061 | P4 | Security document: threat model, what the sandbox guarantees (script-initiated brokered effects), bootstrap I/O, SSRF defaults, secrets, process backends, remote MCP trust boundary | R11, R13, R24 | — | `docs/security/sec-2026-0001-policy-and-sandbox-model.md`, `docs/security/index.md` | TASK-057 | I | NOT STARTED | |
| TASK-062 | P4 | Operations document: deploying `rivet serve` (bind, auth, principals, surfaces, limits), trace storage, logs, health, upgrade | R25 | — | `docs/operations/ops-2026-0001-operating-rivet-serve.md`, `docs/operations/index.md` | TASK-057 | I | NOT STARTED | |
| TASK-063 | P4 | Runbooks (template 12.8): rotate bearer tokens; roll out a policy.json change and verify with `io --check-policy` | R24, R25 | — | `docs/runbooks/run-2026-0001-rotate-serve-bearer-tokens.md`, `run-2026-0002-roll-out-policy-change.md`, `docs/runbooks/index.md` | TASK-062 | I | NOT STARTED | |
| TASK-064 | P4 | Onboarding: contributor setup (toolchain, vhco loop, fixtures, running conformance suites, writing an adapter) | R14 | — | `docs/onboarding/onb-2026-0001-contributor-setup.md`, `docs/onboarding/index.md` | TASK-051 | I | NOT STARTED | |
| TASK-065 | P4 | Manual root plus volumes (template 12.15), see checklist MAN-0001…0008. Includes a feature catalogue, task workflows with CLI/HTTP/WS/MCP/library procedures, success and failure examples, errors and recovery, ASCII journeys, verified-demo links and "What's New in 0.1.0" | R1–R26 | — | `docs/manuals/man-2026-0001…0008-*.md`, `docs/manuals/index.md` | TASK-059, TASK-060, TASK-068 | I | NOT STARTED | |
| TASK-066 | P4 | Update REF-2026-0002: mark each example verified or Stage C; fix anything implementation changed; the proposal sample CLI section agrees with the code | R14 | — | `docs/references/ref-2026-0002-language-and-usage.md` (UPDATE) | TASK-048 | I | NOT STARTED | |
| TASK-067 | P4 | Execute and verify the 12 sample folders against v0.1.0 builds: fill each Verification Record; `verified_against: 0.1.0`; status draft → active; update `manifest.json` (`runtime_verified: true`), demos README and index | R14 / all UCs | — | `docs/demos/01-catalog/README.md` … `12-library/README.md`, `docs/demos/README.md`, `docs/demos/index.md`, `docs/demos/manifest.json` (UPDATE); `12-library/embedding.rs.txt` → compiled example (see PF-D13) | TASK-048 | I | NOT STARTED | |
| TASK-068 | P4 | Release verification guide DEMO-2026-0014 (§29): version/tag/commit header; one U-NN row per released update (U-01…U-26, one per requirement) with inciting UQ, action, expected result and evidence; success and failure examples per surface; cleanup; verification record | R1–R26 | — | `docs/demos/demo-2026-0014-v0-1-0-release-verification.md` (CREATE) | TASK-067 | I | NOT STARTED | |
| TASK-069 | P4 | README (root) → current implemented state: install, quickstart, surfaces, links to the manual; remove "design only" once true | R14 | — | `README.md` (UPDATE) | TASK-065 | I | NOT STARTED | |
| TASK-070 | P4 | docs current-state and indexes: `docs/README.md` (status, releases, implementations, incidents, proposals, architecture, decisions, limitations, risks), `docs/index.md`; each new directory's `index.md` | R14 | — | `docs/README.md`, `docs/index.md`, all new `index.md` (UPDATE/CREATE) | TASK-058–068 | I | NOT STARTED | |
| TASK-071 | P4 | Documentation checker script run in CI (§19): front matter, IDs unique, prefix↔type↔dir, status values, filename↔ID, header↔front matter, revision↔last history row, links/anchors, fences, index membership | R14 | `scripts/check_docs.py` (CREATE) | T-31 | TASK-070 | I | NOT STARTED | |
| TASK-072 | P4 | Record the six documentation-impact decisions (demo, README, system, architecture, API/CLI, manual) as UPDATED or NOT APPLICABLE with a reason | R14 | — | Findings table + REL-0.1.0 | TASK-058–070 | I | NOT STARTED | |

### P5 — Version, release and rollout

| Task | Phase | Description / method | Req / UC / Change IDs | Production files | Test files / manual procedure | Dependencies | Owner | Status | Evidence / result |
|---|---|---|---|---|---|---|---|---|---|
| TASK-073 | P5 | Maintainer approves the release candidate (validation report, demos, manuals) | all | — | Manual | P4 exit | M | NOT STARTED | |
| TASK-074 | P5 | Set the canonical version `0.1.0` in `Cargo.toml`; check every version reference (`rivet --version`, MCP `serverInfo.version`, API docs, manuals, DEMO-2026-0014, README) | release | `Cargo.toml`, `Cargo.lock` (UPDATE) | T-33 (version sync script step) | TASK-073 | I | NOT STARTED | |
| TASK-075 | P5 | Draft the release document in the flat form: version, plan, standards baseline, requirements, Added/Changed/Fixed/Removed, U-NN verification, tests, validation, incidents, troubleshooting, demo, manual, system, the six impact decisions, source changes, known issues, limitations, follow-up (Stage C → PLAN-2026-0002) | R1–R26 | — | `docs/releases/rel-0.1.0-release-notes.md`, `docs/releases/index.md` (CREATE) | TASK-074 | I | NOT STARTED | |
| TASK-076 | P5 | Final release commit `release: v0.1.0`; record the full SHA | §32.2 | repository | `git rev-parse HEAD` | TASK-075 | I | NOT STARTED | |
| TASK-077 | P5 | Annotated tag `v0.1.0`; verify `git rev-list -n 1 v0.1.0` == recorded SHA; `git describe --tags --exact-match HEAD` = `v0.1.0`; clean tree | §32.3–32.4 | repository | T-33 | TASK-076 | I | NOT STARTED | |
| TASK-078 | P5 | Push the commit and tag to the remote (if configured) | §32.3 | repository | remote shows the tag | TASK-077 | M | NOT STARTED | |
| TASK-079 | P5 | Finalize REL-0.1.0 with tag, full SHA and release date; set status. The finalizing commit is a follow-up doc commit; the tag stays on the release commit | §33 | — | `docs/releases/rel-0.1.0-release-notes.md` (UPDATE) | TASK-077 | I | NOT STARTED | |
| TASK-080 | P5 | Rollout: publish the binary/crate artifacts (GitHub release assets; crates.io only if the maintainer chooses and G-LIC permits) | release | release artifacts | Manual: download and run `rivet --version` | TASK-078 | M | NOT STARTED | |
| TASK-081 | P5 | Post-release verification: run DEMO-2026-0014 against the published artifact; record it in the demo's verification record | R1–R26 | — | T-30 on release artifacts | TASK-080 | I | NOT STARTED | |
| TASK-082 | P5 | Mark this plan `completed`, the proposal `implemented`; move documents as the lifecycle requires; regenerate indexes | R14 | this plan, proposal, indexes | T-31 | TASK-081 | I | NOT STARTED | |

### Cross-phase documentation upkeep

| Task | Phase | Description / method | Req / UC / Change IDs | Production files | Test files / manual procedure | Dependencies | Owner | Status | Evidence / result |
|---|---|---|---|---|---|---|---|---|---|
| TASK-083 | P1 | Create `docs/plans/index.md` and link this plan from `docs/README.md`, `docs/index.md` and the proposal | R14 | — | `docs/plans/index.md` (CREATE), `docs/README.md`, `docs/index.md` (UPDATE) | — | I | NOT STARTED | |
| TASK-084 | P1 | Create `docs/research/index.md` and `docs/decisions/index.md` | R14 | — | as named | TASK-009 | I | NOT STARTED | |
| TASK-085 | P2a | Create `docs/incidents/index.md` (with `active/`, `resolved/`, `postmortems/`) and `docs/troubleshooting/index.md` before code starts, so defects have a home | R14 | — | as named | TASK-013 | I | NOT STARTED | |
| TASK-086 | P2* | Every design change discovered during P2 updates the contract first, then the proposal/reference, then code (AGENTS loop); log it in Decisions | all | `vhco-contract.json`, proposal | `vhco sync .` | — | I | NOT STARTED | |
| TASK-087 | P2* | Update this plan's statuses and evidence at every task transition; recompute the Live Status Summary | all | this plan | Review at each phase exit | — | I | NOT STARTED | |
| TASK-088 | P3 | Create `docs/testing/index.md`, `docs/reports/index.md` | R14 | — | as named | TASK-052 | I | NOT STARTED | |
| TASK-089 | P4 | Create `docs/manuals/index.md`, `docs/system/index.md`, `docs/architecture/index.md`, `docs/api/index.md`, `docs/security/index.md`, `docs/operations/index.md`, `docs/runbooks/index.md`, `docs/onboarding/index.md` | R14 | — | as named | TASK-058–065 | I | NOT STARTED | |
| TASK-090 | P5 | Create `docs/releases/index.md` | R14 | — | as named | TASK-075 | I | NOT STARTED | |
| TASK-091 | P4 | Review documents with `review_cycle: on-release` (§34) and bump `next_review_date` | R14 | — | all docs | TASK-070 | M | NOT STARTED | |
| TASK-092 | P4 | Code ≈ system docs drift check (`vhco doc`); fix or record as a known limitation | R14 | — | Findings | TASK-059 | I | NOT STARTED | |
| TASK-093 | P3 | Secret-canary scan over every test log, trace export and error body (no token/secret/key contents) | R13, R16 | — | T-09, T-11 artifacts | TASK-047 | I | NOT STARTED | |
| TASK-094 | P3 | Stage C refusal check: Stage C syntax (FIFO, watch, mTLS TCP, custom codec, reconnect) fails with typed `unsupported.*`, never partially runs | R12 | — | part of T-05 | TASK-047 | I | NOT STARTED | |
| TASK-095 | P5 | Record limitations and known issues (platform sandbox gaps, remote MCP opacity, no persistence/resume) in REL-0.1.0, MAN limitations chapter and docs/README | R11, R12 | — | as named | TASK-075 | I | NOT STARTED | |
| TASK-096 | P5 | Walk the §34 Release Completion Gate item by item; attach evidence for each; the release is complete only when every box is checked | all | — | §34 checklist copied into REL-0.1.0 | TASK-082 | M | NOT STARTED | |

## File and Artifact Checklist

CRUD values: CREATE, READ, UPDATE, DELETE, GENERATE (tool output, never hand-edited). The proposal's F-IDs are
cited so both documents trace together.

### Production (Rust)

| ID | Category | Exact path | CRUD | Planned edit (why / what) | Req / tasks | Proposal F | Status | Verification |
|---|---|---|---|---|---|---|---|---|
| PF-01 | production | `Cargo.toml`, `Cargo.lock` | CREATE | Package `rivet`, lib + bin targets, pinned `capy-core` rev, ADR-0002 crates, version source | R1 / 014, 074 | F-02 | NOT STARTED | T-27 |
| PF-02 | production | `rust-toolchain.toml` | CREATE | Pin the toolchain channel | R1 / 008 | — | NOT STARTED | T-27 |
| PF-03 | production | `src/orchestrator/lib.rs`, `src/orchestrator/main.rs` | CREATE | Module wiring; reexports `Runtime`, `Policy`, `Completion`, `OutputSpec`, `IoManifest`; binary entry | R8, R9 / 014 | F-02 | NOT STARTED | T-10, T-27 |
| PF-04 | production | `src/orchestrator/setup_cli.rs`, `setup_http.rs`, `setup_library.rs`, `setup_mcp.rs`, `setup_ws.rs`, `setup_serve.rs` | CREATE | One `setup_<surface>` per surface (VHCO rule); `poll` registered in `setup_http.rs` or its own `setup_poll.rs` if `vhco validate` requires it | R8, R25 / 024–032 | F-12, F-31 | NOT STARTED | T-28 |
| PF-05 | production | `src/domain/mod.rs`, `contracts.rs`, `errors.rs`, `syntax_tree.rs`, `outputs.rs`, `policy.rs`, `io_manifest.rs`, `sessions.rs`, `serve.rs`, `dag.rs` | CREATE | Pure data (split of proposal F-03 by topic); error registry table | R1–R26 / 015 | F-03, F-32, F-33 | NOT STARTED | domain tests |
| PF-06 | production | `src/features/language/{compile_program,compile_output_spec,ports}.rs` | CREATE | Compile and output spec use cases | R1, R2, R20, R23 / 016–018 | F-04, F-25, F-29 | NOT STARTED | T-01, T-16, T-20, T-23 |
| PF-07 | production | `src/features/registry/{describe_operations,inspect_outputs,ports}.rs` | CREATE | Catalog and outputs | R8, R20, R23 / 019 | F-05, F-29 | NOT STARTED | T-16, T-20 |
| PF-08 | production | `src/features/execution/{request_operation,cancel_request,run_dag,validate_output,ports}.rs` | CREATE | Dispatcher, cancellation, DAG, output validation | R3, R4, R9, R10, R23 / 022, 036 | F-06, F-29, F-33 | NOT STARTED | T-02, T-03, T-07, T-24 |
| PF-09 | production | `src/features/files/{apply_file_operation,ports}.rs` | CREATE | File CRUD | R5 / 023 | F-08 | NOT STARTED | T-04 |
| PF-10 | production | `src/features/connectors/{invoke_mcp,ports}.rs` | CREATE | MCP client use case | R7 / 040 | F-11 | NOT STARTED | T-06 |
| PF-11 | production | `src/features/audit/{inspect_effects,read_trace,ports}.rs` | CREATE | IoManifest and trace read | R6, R26 / 037, 038 | F-10, F-35 | NOT STARTED | T-09, T-25 |
| PF-12 | production | `src/features/policy/{authorize_effect,load_policy,generate_policy,ports}.rs` | CREATE | Policy load, authorization, draft | R11, R24, R26 / 020, 021, 039 | F-09, F-30, F-36 | NOT STARTED | T-08, T-21, T-26 |
| PF-13 | production | `src/features/auth/{begin_authorization,complete_authorization,credential_status,disconnect_account,acquire_credential,cancel_authorization,ports}.rs` | CREATE | OAuth use cases (`cancel_authorization` per TASK-006) | R16 / 041 | F-19 | NOT STARTED | T-11 |
| PF-14 | production | `src/features/datagrams/{exchange_datagrams,ports}.rs` | CREATE | UDP | R15 / 042 | F-20 | NOT STARTED | T-12 |
| PF-15 | production | `src/features/quic/{exchange_quic,ports}.rs` | CREATE | QUIC | R17 / 043 | F-21 | NOT STARTED | T-13 |
| PF-16 | production | `src/features/grpc/{invoke_rpc,ports}.rs` | CREATE | gRPC | R19 / 045 | F-24 | NOT STARTED | T-17 |
| PF-17 | production | `src/features/sessions/{open_session,send_input,finish_input,read_events,cancel_session,ports}.rs` | CREATE | Sessions | R22 / 029 | F-27 | NOT STARTED | T-19 |
| PF-18 | production | `src/features/serve/{start_serve,authenticate_principal,authorize_operation,multiplex_ws,project_polling,ports}.rs` | CREATE | Unified serve | R25 / 026–031 | F-31 | NOT STARTED | T-22 |
| PF-19 | production | `src/features/transports/{exchange_http,exchange_socket,run_process,ports}.rs` | CREATE | Transport use cases (contract `transports` feature) | R12 / 033–035, 044 | F-13 | NOT STARTED | T-05, T-14 |
| PF-20 | production | `src/io/cli/mod.rs`, `src/io/http/mod.rs`, `src/io/http/poll.rs`, `src/io/ws/mod.rs`, `src/io/mcp/mod.rs`, `src/io/library/mod.rs` | CREATE | Surface encoders/decoders only | R8, R21, R22, R25 / 024–032 | F-12, F-26, F-31 | NOT STARTED | T-02, T-18, T-22 |
| PF-21 | production | `src/infra/rivet.capy`, `capy_parser.rs` | CREATE | Grammar; only Capy importer | R1, R2 / 010, 016 | F-04, F-32 | NOT STARTED | T-01, T-23 |
| PF-22 | production | `src/infra/{registry,execution_driver,scope_supervisor,file_access,policy_broker,policy_file_reader,policy_draft_writer,trace_store,mcp_client}.rs` | CREATE | Core adapters | R3–R7, R11, R24, R26 | F-05, F-07–F-11, F-30, F-36 | NOT STARTED | T-03–T-09, T-21, T-25, T-26 |
| PF-23 | production | `src/infra/{http_adapter,socket_adapter,process_adapter,codec}.rs` | CREATE | Transports (H3 added in TASK-044) | R12, R18 / 033–035, 044 | F-13, F-22 | NOT STARTED | T-05, T-14 |
| PF-24 | production | `src/infra/{oauth_adapter,credential_store,udp_adapter,quic_adapter,grpc_adapter,session_driver,serve_listener,policy_authenticator}.rs` | CREATE | Stage B and serve adapters | R15–R19, R22, R25 | F-19–F-21, F-24, F-27, F-31 | NOT STARTED | T-11–T-17, T-19, T-22 |
| PF-25 | production | `src/infra/sandbox_<platform>.rs` (names fixed by ADR-0003) | CREATE | Process confinement backends | R11 / 035 | F-14 | NOT STARTED | T-08 |
| PF-26 | production | `vhco-contract.json` | UPDATE | TASK-006 reconciliation; any P2 design change | R14 / 002, 006, 086 | F-15 | NOT STARTED | T-28 |
| PF-27 | production | `AGENTS.md` | UPDATE | "Project facts" → Rust, Rivet features and surfaces | R14 / 006 | — | NOT STARTED | review |
| PF-28 | production | `.gitignore`, `deny.toml` | CREATE | Repo hygiene; licence policy | release / 007, 049 | — | NOT STARTED | T-27 |

### Tests and fixtures

| ID | Category | Exact path | CRUD | Planned edit | Req / tasks | Status | Verification |
|---|---|---|---|---|---|---|---|
| PF-T01 | test | colocated `src/**/<use_case>_test.rs` (one per contract use case: 47 files) with `// vhco:test <feature.use_case> -- …` | CREATE | Unit tests per use case (AGENTS rule 8) | R1–R26 / P2 tasks | NOT STARTED | `cargo test`; vhco Tests view |
| PF-T02 | test | `tests/conformance_language.rs`, `_surfaces`, `_streams`, `_files`, `_resources`, `_mcp`, `_dag`, `_sandbox`, `_audit`, `_library`, `_oauth`, `_udp`, `_quic`, `_http3`, `_auth_transport_policy`, `_operation_catalog`, `_grpc`, `_mcp_catalog`, `_sessions`, `_outputs`, `_policy_file`, `_serve`, `_syntax`, `_errors_limits_dag`, `_io_manifest`, `_policy_generate` | CREATE | T-01–T-26 suites (names per proposal commands) | R1–R26 / 047 | NOT STARTED | T-01–T-26 |
| PF-T03 | test | `tests/conformance_samples.rs` | CREATE | T-29 corpus: REF-2026-0002 + demos | R2, R14 / 048 | NOT STARTED | T-29 |
| PF-T04 | test | `tests/fixtures/{mod,http,ws,socket,mcp,grpc,quic,udp,oauth,consumer}.rs` | CREATE | Deterministic local fixtures; no external network | T-02–T-19 / 046 | NOT STARTED | used by suites |
| PF-T05 | test | `tests/fixtures/data/` (certs from a generated test CA, descriptor `users.pb`, MCP snapshots) | CREATE / GENERATE | Test-only material; generated by `tests/fixtures/generate.sh` | 046 | NOT STARTED | fixture self-test |

### Generated artifacts

| ID | Category | Exact path | CRUD | Planned edit | Req / tasks | Status | Verification |
|---|---|---|---|---|---|---|---|
| PF-G01 | generated | `vhco.json`, `vhco.html` | GENERATE | `vhco spec .`; never hand-edited; never written onto the contract | R14 / 050 | NOT STARTED | T-28 |
| PF-G02 | generated | `target/release/rivet` (+ `.exe`), release archives | GENERATE | Release binaries per OS | release / 080 | NOT STARTED | T-33, TASK-081 |

### Version and release artifacts

| ID | Category | Exact path | CRUD | Planned edit | Req / tasks | Status | Verification |
|---|---|---|---|---|---|---|---|
| PF-V01 | version | `Cargo.toml` `package.version` | UPDATE | `0.1.0-dev` → `0.1.0` (the only version source) | release / 074 | NOT STARTED | T-33 |
| PF-V02 | release | Git commit `release: v0.1.0`, tag `v0.1.0` | CREATE | §32 | release / 076–078 | NOT STARTED | T-33 |
| PF-V03 | release | `docs/releases/rel-0.1.0-release-notes.md` | CREATE | §33 | release / 075, 079 | NOT STARTED | T-31 |
| PF-V04 | CI | `.github/workflows/ci.yml` | CREATE (if GitHub) | T-27–T-29, T-31 on three OSes | 051 | NOT STARTED | CI run |
| PF-V05 | tooling | `scripts/check_docs.py` | CREATE | §19 documentation checks | 071 | NOT STARTED | T-31 |

## Test and Validation Checklist

T-01–T-26 are the proposal's tests with unchanged meaning. T-27–T-33 are plan-level gates. Each has a TEST
document `docs/testing/test-2026-00NN-<slug>.md` (NN = the T number) holding its definition and latest result.

| Test ID | Type | UC / Req | Scenario (positive / negative / regression) | Exact test file or manual steps | Command / environment | Expected result | Status | Evidence |
|---|---|---|---|---|---|---|---|---|
| T-01 | unit + integration | UC-01 / R1, R2 | +: all grammar shapes parse; −: invalid sources run zero effects; spans exact | `tests/conformance_language.rs` | `cargo test conformance_language`; Linux/macOS/Windows | PASS; 0 effects on invalid | NOT STARTED | TEST-2026-0001 |
| T-02 | integration / e2e | UC-02 / R8, R9 | Same fixture registry over CLI/HTTP/MCP/library; auth filtering; stdout clean | `tests/conformance_surfaces.rs` | `cargo test conformance_surfaces` | Identical params/result/error | NOT STARTED | TEST-2026-0002 |
| T-03 | fault / resource | UC-03 / R3, R4, R9 | Slow consumer, emit then fail, invalid frame, disconnect | `tests/conformance_streams.rs` | `cargo test conformance_streams` | Bounded queue (≤16), one terminal, cancel observed | NOT STARTED | TEST-2026-0003 |
| T-04 | security / edge | UC-04 / R5, R11 | Symlink escape, version race, hard link, missing/full disk | `tests/conformance_files.rs` | `cargo test conformance_files` (temp root) | Correct guards; nothing mutated outside the root | NOT STARTED | TEST-2026-0004 |
| T-05 | integration / fault | UC-05 / R3, R12, R13 | HTTP/socket/process fixtures; early return; EOF; forced cleanup faults; Stage C refusal (TASK-094) | `tests/conformance_resources.rs` | `cargo test conformance_resources` | No live handles after 5 s; primary error kept; `unsupported.*` for Stage C | NOT STARTED | TEST-2026-0005 |
| T-06 | integration | UC-06 / R7 | MCP stdio/HTTP fixtures; tools/resources/prompts; drift; recursion | `tests/conformance_mcp.rs` | `cargo test conformance_mcp` | Typed faults; hop limit enforced | NOT STARTED | TEST-2026-0006 |
| T-07 | unit / integration | UC-07 / R10 | Diamond, cycle, fan-out, sibling failure, fail-fast default | `tests/conformance_dag.rs` | `cargo test conformance_dag` | Correct node states and `DagCompletion` | NOT STARTED | TEST-2026-0007 |
| T-08 | security / compatibility | UC-08 / R11, R13 | No policy.json; empty grants; hard links; junctions; case variants; DNS rebinding; process descendants | `tests/conformance_sandbox.rs` | `cargo test conformance_sandbox` per OS | Zero prohibited brokered effects; unsupported backend refuses before spawn | NOT STARTED | TEST-2026-0008 |
| T-09 | security / audit | UC-09 / R6, R13 | Every adapter; dynamic/opaque sites; secret canaries; failed sink | `tests/conformance_audit.rs` | `cargo test conformance_audit` | Every site reported; no canary leaks | NOT STARTED | TEST-2026-0009 |
| T-10 | integration | UC-10 / R1, R3, R9 | Host Tokio runtime; abandoned future; scope exit | `tests/conformance_library.rs` | `cargo test conformance_library` | No nested runtime; cleanup joined | NOT STARTED | TEST-2026-0010 |
| T-11 | integration / security | UC-11 / R16 | Three flows; state/issuer mismatch; slow_down; device `pending`; rotation race; invalid_grant | `tests/conformance_oauth.rs` | `cargo test conformance_oauth` | Tokens never escape; one refresh per key | NOT STARTED | TEST-2026-0011 |
| T-12 | integration | UC-12 / R15 | IPv4/IPv6/multicast; drop/reorder/dup; oversized/truncated; denied bind | `tests/conformance_udp.rs` | `cargo test conformance_udp` | Boundaries kept; typed errors | NOT STARTED | TEST-2026-0012 |
| T-13 | integration / security | UC-13 / R17 | Wrong cert/ALPN; blocked window; FIN/reset; DATAGRAM absent; migration | `tests/conformance_quic.rs` | `cargo test conformance_quic` | TLS checked; no 0-RTT; denied path sends 0 packets | NOT STARTED | TEST-2026-0013 |
| T-14 | integration | UC-14 / R18 | H3/H2 fixtures; strict vs preference; POST accepted then response lost | `tests/conformance_http3.rs` | `cargo test conformance_http3` | No unsafe downgrade; one mutation | NOT STARTED | TEST-2026-0014 |
| T-15 | cross-surface / security | UC-11–14 / R15–R18 | Same fixtures over all surfaces; absent/empty/restricted policy.json | `tests/conformance_auth_transport_policy.rs` | `cargo test conformance_auth_transport_policy` | Same values/errors | NOT STARTED | TEST-2026-0015 |
| T-16 | unit / integration | UC-15 / R20 | Multi-op file; duplicate in second declaration/import; private helper | `tests/conformance_operation_catalog.rs` | `cargo test conformance_operation_catalog` | Atomic load; both spans shown | NOT STARTED | TEST-2026-0016 |
| T-17 | integration | UC-16 / R19 | Four modes; map/oneof/int64/bytes/Any; late non-OK trailers | `tests/conformance_grpc.rs` | `cargo test conformance_grpc` | Status governs success | NOT STARTED | TEST-2026-0017 |
| T-18 | e2e | UC-17 / R21 | stdio and HTTP MCP clients; list/call; unauthorized; streaming tools | `tests/conformance_mcp_catalog.rs` | `cargo test conformance_mcp_catalog` | Schemas match other surfaces | NOT STARTED | TEST-2026-0018 |
| T-19 | e2e / fault | UC-18 / R22 | open/send/read/finish/cancel across CLI/HTTP/poll/WS/MCP/library; duplicate send; stale cursor | `tests/conformance_sessions.rs` | `cargo test conformance_sessions` | Ordered, bounded, principal-bound | NOT STARTED | TEST-2026-0019 |
| T-20 | unit / e2e | UC-19 / R23 | Scalar/nested/list/open outputs; violation after a committed write | `tests/conformance_outputs.rs` | `cargo test conformance_outputs` | Same schema on every surface; `output.invalid` exit 5 | NOT STARTED | TEST-2026-0020 |
| T-21 | security | UC-08 / R24 | No file; `{"version":1}`; `--policy`; unknown key; deny > grants; `access` verbs; private ranges | `tests/conformance_policy_file.rs` | `cargo test conformance_policy_file` | Deny-by-default; `policy.invalid` exit 2 | NOT STARTED | TEST-2026-0021 |
| T-22 | e2e / security | UC-20 / R25 | One serve over REST/SSE/poll/WS/MCP; none/bearer/mTLS; non-loopback+none; ninth WS ref; `rivet.io` exposure rule | `tests/conformance_serve.rs` | `cargo test conformance_serve` | Identical Completion; `serve.auth_required` exit 2 | NOT STARTED | TEST-2026-0022 |
| T-23 | unit | UC-01 / R1, R2 | Prefix calls, quoted durations, `${a.b}` vs `${a + b}`, escapes, URL encoding, option after body, yield/return | `tests/conformance_syntax.rs` | `cargo test conformance_syntax` | Exact diagnostics | NOT STARTED | TEST-2026-0023 |
| T-24 | unit / resource | UC-02, UC-07 / R4, R10 | Every registry code on every surface; 65 concurrent calls; depth 17; call cycle; unguarded `.result` | `tests/conformance_errors_limits_dag.rs` | `cargo test conformance_errors_limits_dag` | Registry-consistent exit/HTTP | NOT STARTED | TEST-2026-0024 |
| T-25 | integration | UC-21, UC-09 / R6, R26 | exact/bounded/param_dependent/dynamic/opaque sites of every kind; views; formats; `--check-policy` exit 3; `--strict` exit 7; `--trace` (+ option-derived sites if TASK-005 is approved) | `tests/conformance_io_manifest.rs` | `cargo test conformance_io_manifest` | Manifest equals the golden JSON | NOT STARTED | TEST-2026-0025 |
| T-26 | integration | UC-22 / R26 | Draft to stdout; `--output` new/existing; dynamic site | `tests/conformance_policy_generate.rs` | `cargo test conformance_policy_generate` | Least-privilege draft; exit 7 / exit 4 | NOT STARTED | TEST-2026-0026 |
| T-27 | build / static | all / R1 | fmt, clippy `-D warnings`, `cargo deny`, release build on three OSes | CI or local | `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo deny check && cargo build --release` | All green; Capy licence passes deny after G-LIC | NOT STARTED | TEST-2026-0027 |
| T-28 | architecture | all / R14 | VHCO structure and drift | — | `vhco validate . && vhco sync . && vhco check . && vhco spec .` | validate green; sync 0; check green | NOT STARTED | TEST-2026-0028 |
| T-29 | regression / corpus | UC-01 / R2, R14 | Every Stage A/B example and demo parses; demo ops run on fixtures | `tests/conformance_samples.rs` | `cargo test conformance_samples` | 100% of in-scope samples pass; Stage C samples refuse | NOT STARTED | TEST-2026-0029 |
| T-30 | manual / e2e | all UCs / R1–R26 | Execute the 12 sample READMEs and DEMO-2026-0014 step by step against the v0.1.0 build, then the release artifact | Manual: follow each README; compare expected output | Clean checkout at the release commit; fixtures running | Every step matches; verification records filled | NOT STARTED | TEST-2026-0030 |
| T-31 | documentation | R14 | Front matter, IDs, links, anchors, fences, headers, revisions, index membership; ASCII visual presence in manuals/system/API | `scripts/check_docs.py` | `python3 scripts/check_docs.py` | 0 errors | NOT STARTED | TEST-2026-0031 |
| T-32 | traceability | R1–R26 | Every R → task → file → test → doc → U-NN row; no orphan either way | Manual + script over this plan, RPT and REL | — | No orphans | NOT STARTED | TEST-2026-0032 |
| T-33 | release | release | Version sync (`Cargo.toml` = `rivet --version` = MCP serverInfo = docs); tag = commit; clean tree | Manual per §32.4 | `git describe --tags --exact-match HEAD` | `v0.1.0`; SHAs equal | NOT STARTED | TEST-2026-0033 |

Categories with no dedicated test: **performance** — NOT APPLICABLE, the proposal makes no performance claim
(resource bounds are covered by T-03/T-24). **UI** — NOT APPLICABLE, the proposal has no UI.

## Documentation and Demo Checklist

Every document follows the metadata of DOCUMENTATION §5/§20, the visible header of §7 and its type template in
§12. Every manual, system, API and architecture document includes ASCII journey, sequence and state visuals.

```text
 docs/
 ├── plans/          PLAN-2026-0001 (this) ··················· P1
 ├── decisions/      ADR-0001 approval · ADR-0002 crates · ADR-0003 sandbox ··· P1
 ├── research/       RES-2026-0001 Capy spike · 0002 crates · 0003 sandbox ···· P1
 ├── incidents/      INC-* as found (active/ resolved/ postmortems/) ········· P2–P3
 ├── troubleshooting/TRBL-* as found ·········································· P2–P3
 ├── testing/        TEST-2026-0001 … 0033 ···································· P3
 ├── reports/        RPT-2026-0001 validation ································· P3
 ├── demos/          01…12 re-verified · DEMO-2026-0014 release verification ·· P4
 ├── architecture/   ARCH-2026-0001 ··········································· P4
 ├── system/         SYS-2026-0001 … 0009 ····································· P4
 ├── api/            API-2026-0001 … 0005 ····································· P4
 ├── security/       SEC-2026-0001 ············································ P4
 ├── operations/     OPS-2026-0001 ············································ P4
 ├── runbooks/       RUN-2026-0001 · 0002 ····································· P4
 ├── onboarding/     ONB-2026-0001 ············································ P4
 ├── manuals/        MAN-2026-0001 … 0008 ····································· P4
 ├── references/     REF-2026-0002 updated (verified / Stage C marks) ········· P4
 └── releases/       REL-0.1.0 ················································ P5
```

| ID | Artifact | Exact path | Why it changes | Required addition / removal | Related interfaces | Status | Verification |
|---|---|---|---|---|---|---|---|
| D-01 | plan index | `docs/plans/index.md` | New directory (AGENTS: every dir has index.md) | Purpose, active PLAN-2026-0001, reading order | — | NOT STARTED | T-31 |
| D-02 | decision | `docs/decisions/adr-0001-approve-rivet-runtime-design.md` + `index.md` | G-DESIGN recorded as a decision (§3.2 chain) | Approved revision, approvers, date, gates | all | NOT STARTED | T-31 |
| D-03 | decision | `docs/decisions/adr-0002-rust-crate-selection.md` | Crate choices from RES-2026-0002 | Crate, version, licence, rejected alternatives | transports, Stage B | NOT STARTED | T-31 |
| D-04 | decision | `docs/decisions/adr-0003-process-sandbox-backends.md` | Per-OS process confinement | Backend per OS or refusal | `allow_exec`, stdio MCP | NOT STARTED | T-31 |
| D-05 | research | `docs/research/res-2026-0001-capy-grammar-spike.md` | G-SPIKE evidence | Parse results for 153 examples + 12 demos; grammar changes | language | NOT STARTED | T-31 |
| D-06 | research | `docs/research/res-2026-0002-rust-crate-feasibility.md` | Crate evidence | Prototype results, licences, MSRV | transports | NOT STARTED | T-31 |
| D-07 | research | `docs/research/res-2026-0003-process-sandbox-backends.md` + `docs/research/index.md` | OQ-1 evidence | Per-OS prototype outcomes | sandbox | NOT STARTED | T-31 |
| D-08 | incidents | `docs/incidents/index.md` (+ `active/`, `resolved/`, `postmortems/`) and INC-* as found | §25 | Template 12.13 per defect | — | NOT STARTED | T-31 |
| D-09 | troubleshooting | `docs/troubleshooting/index.md` and TRBL-* as found | §26 | Template 12.9 | — | NOT STARTED | T-31 |
| D-10 | tests | `docs/testing/test-2026-0001-language.md` … `test-2026-0033-release-integrity.md` (33 files, slug = suite) + `docs/testing/index.md` | §27 | Definition, requirements, environment, executed by/at, result, evidence | all | NOT STARTED | T-31, T-32 |
| D-11 | validation report | `docs/reports/rpt-2026-0001-v0-1-0-validation.md` + `docs/reports/index.md` | §28 | Result per R1–R26; deviations; limitations | all | NOT STARTED | T-32 |
| D-12 | release verification demo | `docs/demos/demo-2026-0014-v0-1-0-release-verification.md` | §29 mandatory | Version/tag/commit; U-01…U-26 table; CLI+HTTP+WS+MCP+library success and failure examples; cleanup; verification record | every surface | NOT STARTED | T-30 |
| D-13 | sample demos | `docs/demos/01-catalog/README.md` … `12-library/README.md`; `docs/demos/README.md`; `docs/demos/index.md`; `docs/demos/manifest.json`; `12-library/embedding.rs.txt` → `12-library/embedding.rs`, compiled as a crate example (`[[example]]` in Cargo.toml) | §29: demos are executed against the release | Fill Verification Record; `verified_against: 0.1.0`; status active; remove "not runnable" disclaimers; `runtime_verified: true` | every surface | NOT STARTED | T-30 |
| D-14 | architecture | `docs/architecture/arch-2026-0001-rivet-runtime-architecture.md` + `index.md` | §31 architecture impact | Folder/port/adapter map; dispatcher; broker; serve fan-out; data and control flow; deployment | all | NOT STARTED | T-31, TASK-092 |
| D-15 | system | `docs/system/components/sys-2026-0001-compiler-and-catalog.md` | §31 system impact | Grammar, SyntaxTree, lowering, registry, outputs | language, registry | NOT STARTED | TASK-092 |
| D-16 | system | `docs/system/runtime/sys-2026-0002-execution-scopes-and-dag.md` | same | Dispatcher, scope supervisor, cancellation, limits, DAG states | execution | NOT STARTED | TASK-092 |
| D-17 | system | `docs/system/components/sys-2026-0003-policy-broker-and-io-manifest.md` | same | Policy load and intersection, per-attempt checks, IoManifest, policy generate | policy, audit | NOT STARTED | TASK-092 |
| D-18 | system | `docs/system/components/sys-2026-0004-surfaces-and-serve.md` | same | CLI, library, HTTP/SSE/poll/WS/MCP, principals | serve, io/* | NOT STARTED | TASK-092 |
| D-19 | system | `docs/system/integrations/sys-2026-0005-protocol-adapters.md` | same | HTTP/1–3, sockets, processes, UDP, QUIC, gRPC, codec | transports, Stage B | NOT STARTED | TASK-092 |
| D-20 | system | `docs/system/integrations/sys-2026-0006-oauth-and-credentials.md` | same | Profiles, transactions, store, refresh | auth | NOT STARTED | TASK-092 |
| D-21 | system | `docs/system/runtime/sys-2026-0007-sessions.md` | same | Session lifecycle, retention, limits | sessions | NOT STARTED | TASK-092 |
| D-22 | system | `docs/system/configuration/sys-2026-0008-policy-json-reference.md` | same | Full schema v1 incl. `access`, `network`, `limits`, `serve`, `approved`; defaults | policy.json | NOT STARTED | TASK-092 |
| D-23 | system | `docs/system/integrations/sys-2026-0009-mcp-client-connectors.md` + `docs/system/index.md` | same | Connectors, snapshots, sync, recursion | connectors | NOT STARTED | TASK-092 |
| D-24 | API | `docs/api/api-2026-0001-http-rest-sse-polling.md` | §31 API impact | Every route, auth, request/response, error format, status codes, examples | http, poll | NOT STARTED | T-31 |
| D-25 | API | `docs/api/api-2026-0002-websocket-rivet-v1.md` | same | Frame types, refs, limits, close semantics | ws | NOT STARTED | T-31 |
| D-26 | API | `docs/api/api-2026-0003-mcp-server-tools.md` | same | Named tools, built-ins, schemas, session delivery | mcp | NOT STARTED | T-31 |
| D-27 | API | `docs/api/api-2026-0004-rust-library.md` | same | `Runtime`, `Policy`, `scope`, `stream`, `duplex`, `outputs`, `io`, `generate_policy` | library | NOT STARTED | T-31 |
| D-28 | API | `docs/api/api-2026-0005-error-registry.md` + `docs/api/index.md` | same | Every code → kind → HTTP → exit → retryable | all | NOT STARTED | T-24, T-31 |
| D-29 | security | `docs/security/sec-2026-0001-policy-and-sandbox-model.md` + `index.md` | Security boundaries changed | Threat model; guarantees and non-guarantees; bootstrap I/O; SSRF; secrets; platform matrix | policy | NOT STARTED | T-31 |
| D-30 | operations | `docs/operations/ops-2026-0001-operating-rivet-serve.md` + `index.md` | New server surface | Bind/auth/principals/surfaces, limits, trace storage, observability, upgrade/rollback | serve | NOT STARTED | T-31 |
| D-31 | runbook | `docs/runbooks/run-2026-0001-rotate-serve-bearer-tokens.md` | Operational procedure | Preconditions, steps, verification, rollback | serve.auth | NOT STARTED | T-30 (dry run) |
| D-32 | runbook | `docs/runbooks/run-2026-0002-roll-out-policy-change.md` + `index.md` | Operational procedure | Edit → `io --check-policy` → `policy explain` → deploy → verify → rollback | policy.json | NOT STARTED | T-30 (dry run) |
| D-33 | onboarding | `docs/onboarding/onb-2026-0001-contributor-setup.md` + `index.md` | New contributors | Toolchain, vhco loop, fixtures, suites, adding an adapter | — | NOT STARTED | T-31 |
| D-34 | manual (root) | `docs/manuals/man-2026-0001-rivet-manual.md` + `docs/manuals/index.md` | §30 canonical book | Purpose, reading order, What's New in 0.1.0, goals/boundaries, concepts, mental model, **feature catalogue** (every feature: what / why / where), limitations, glossary, version applicability | all | NOT STARTED | T-31 |
| D-35 | manual | `docs/manuals/man-2026-0002-installation-and-quickstart.md` | Installation | Install binary/crate, first operation, first serve | CLI | NOT STARTED | T-30 |
| D-36 | manual | `docs/manuals/man-2026-0003-language-guide.md` | Writing `.rivet` | Operations, params, outputs, errors, resources, streams, DAGs, connectors; links to REF-2026-0002 | language | NOT STARTED | T-29 |
| D-37 | manual | `docs/manuals/man-2026-0004-cli-reference.md` | §31 CLI reference | Every command/flag/exit code with success and failure examples | CLI | NOT STARTED | T-02 |
| D-38 | manual | `docs/manuals/man-2026-0005-policy-and-io-manifest-guide.md` | Administrator guide | Writing policy.json, `access`, `io` views, `--check-policy`, `policy generate`, review workflow | policy, audit | NOT STARTED | T-21, T-25 |
| D-39 | manual | `docs/manuals/man-2026-0006-serving-and-surfaces.md` | Operator guide | `serve`, auth, principals, REST/SSE/poll/WS/MCP task workflows with ASCII sequences | serve | NOT STARTED | T-22 |
| D-40 | manual | `docs/manuals/man-2026-0007-embedding-library.md` | Developer guide | Library workflows; links to API-2026-0004 and demo 12 | library | NOT STARTED | T-10 |
| D-41 | manual | `docs/manuals/man-2026-0008-protocols-and-connectors.md` | Integrator guide | HTTP/1–3, WS, TCP/Unix, processes, UDP, QUIC, gRPC, OAuth, MCP connectors; errors and recovery | transports, Stage B | NOT STARTED | T-05, T-11–T-17 |
| D-42 | reference | `docs/references/ref-2026-0002-language-and-usage.md` | Examples now verified | Verified / Stage C markers; corrections from implementation | language | NOT STARTED | T-29 |
| D-43 | proposal | `docs/proposals/…/prop-2026-0001-rivet-runtime.md` | Lifecycle | Approved (P1) → implemented (P5); plan map; reconciliation | — | NOT STARTED | T-31 |
| D-44 | README | `README.md` | §31 README impact | Current implemented state, install, quickstart, links | all | NOT STARTED | T-31 |
| D-45 | current-state | `docs/README.md`, `docs/index.md` | §14, AGENTS indexes | Status, releases, implementations, incidents, proposals, architecture, decisions, limitations, risks | — | NOT STARTED | T-31 |
| D-46 | release notes | `docs/releases/rel-0.1.0-release-notes.md` + `docs/releases/index.md` | §33 | Full release template; six impact decisions; §34 gate | all | NOT STARTED | T-31, T-33 |
| D-47 | agent guide | `AGENTS.md` project facts | Stale Go facts | Rust module, features, surfaces | — | NOT STARTED | review |

**Six post-release documentation-impact decisions** (§31), all expected `UPDATED`:

| Artifact | Expected decision | Covered by |
|---|---|---|
| Release verification guide / demo | UPDATED | D-12, D-13 |
| Top-level README.md | UPDATED | D-44 |
| system/ | UPDATED | D-15 – D-23 |
| architecture/ | UPDATED | D-14 |
| api/ and CLI reference | UPDATED | D-24 – D-28, D-37 |
| manuals/ | UPDATED | D-34 – D-41 |

## Version, Release and Rollout Checklist

| Item | Source / target | Required action | Dependency | Status | Evidence |
|---|---|---|---|---|---|
| Version | `Cargo.toml` `package.version` (only source) | `0.1.0-dev` → `0.1.0`; sync check T-33 | TASK-073 | NOT STARTED | |
| Release notes | `docs/releases/rel-0.1.0-release-notes.md` (flat form, §33) | Full template | TASK-074 | NOT STARTED | |
| Release commit | repository | `git commit -m "release: v0.1.0"`; record the full SHA | TASK-075 | NOT STARTED | |
| Tag | repository | `git tag -a v0.1.0 -m "Release v0.1.0"`; verify it resolves to the commit | TASK-076 | NOT STARTED | |
| Push | remote | Push the branch and tag | TASK-077 | NOT STARTED | |
| Artifacts | GitHub release (crates.io optional) | Linux/macOS/Windows binaries + checksums | TASK-078 | NOT STARTED | |
| Rollout | users | Announce; link the manual and DEMO-2026-0014 | TASK-080 | NOT STARTED | |
| Post-release verification | DEMO-2026-0014 on the published artifact | Execute; fill the verification record | TASK-080 | NOT STARTED | |

```text
 Cargo.toml 0.1.0 ─► commit "release: v0.1.0" ─► SHA 40-hex recorded ─► tag v0.1.0
        │                                                                   │
        └──────── T-33: rivet --version == serverInfo.version == docs ──────┘
                                                                            ▼
                         REL-0.1.0 {version 0.1.0, tag v0.1.0, commit <SHA>} ─► §34 gate
```

## Decisions, Findings, Deviations and Blockers

| Timestamp | Type | Task / requirement | Finding or decision | Impact | Owner / follow-up | Linked |
|---|---|---|---|---|---|---|
| 2026-09-28 | Blocker | TASK-003 / R1 | Capy `LICENSE` is source-available (forbids bundling and derivatives); `Cargo.toml` says MIT | No P2 code until relicensed | Maintainer (Capy owner) | G-LIC |
| 2026-09-28 | Blocker | TASK-007 | Repository is not a Git repository | §32 release impossible until initialized | Maintainer | — |
| 2026-09-28 | Finding | TASK-006 | Contract lacks `cancel_authorization` although the proposal defines `rivet.auth.cancel` | Contract ↔ proposal drift | Implementer, P1 | PROP §Increment 9 |
| 2026-09-28 | Finding | TASK-006 | Proposal F-35 names `build_io_manifest.rs`; the contract extends `inspect_effects` | Choose the contract (code source of truth) | Implementer, P1 | PROP F-35 |
| 2026-09-28 | Finding | TASK-006 | AGENTS.md "Project facts" describe a Go `email_provider` | Misleads agents | Implementer, P1 | — |
| 2026-09-28 | Decision (pending) | TASK-005 | Option-derived file sites / `io --needs` suggested in conversation, not yet approved | May extend R26/T-25 | Maintainer | — |
| 2026-09-28 | Deviation | D-10 | Proposal F-18 named one TEST file; this plan uses one TEST document per suite (33) | Finer evidence; same tests | Implementer | PROP F-18 |
| 2026-09-28 | Deviation | PF-05 | Proposal F-03 named `mod.rs` + `contracts.rs`; the plan splits the domain by topic | Same types, smaller files | Implementer | PROP F-03 |
| 2026-09-28 | Scope | — | Stage C adapters excluded; they need a future PLAN-2026-0002 | Not in v0.1.0 | Maintainer | PROP delivery matrix |

## Rollout Strategy

v0.1.0 is the first release, so there are no existing users or data to migrate.
1. Publish the tagged binaries for Linux, macOS and Windows as GitHub release assets with SHA-256 checksums.
   Publishing the crate to crates.io is optional; the maintainer decides after G-LIC confirms the dependency
   licence permits it.
2. Recommended adoption path, documented in MAN-2026-0002:
   1. `rivet check --strict-docs`.
   2. `rivet io --by target`.
   3. `rivet policy generate`, then human review.
   4. `rivet io --check-policy`.
   5. `rivet serve` on loopback.
   6. Only then a non-loopback bind with bearer or mTLS auth.
3. Post-release verification (TASK-081) must pass before the rollout is announced.

## Rollback Strategy

- **Artifacts.** A defective v0.1.0 artifact is withdrawn from the release page. The tag is never moved or reused.
  Fixes ship as v0.1.1 through a new plan revision or plan.
- **Users.** No persistent state exists: no data migration, sessions do not survive restart, and credential
  stores are user-owned. Rollback means running the previous binary, or none.
- **Policy.** A `policy.json` edit is rolled back by restoring the previous file. RUN-2026-0002 records the
  procedure and the `io --check-policy` verification.
- **During P2.** A failing increment is reverted to the last commit where `vhco validate` is green. Record an
  incident if the failure was unexpected.

## Risks

| Risk | Likelihood | Impact | Mitigation | Owner |
|---|---|---|---|---|
| Capy cannot express some statement shapes (G-SPIKE fails) | Medium | High | The spike runs first; grammar changes go back through the contract and proposal before code; fallback = Rivet-owned parser behind the same `Parser` port (PROP Alternatives) | I |
| Process sandbox backend missing on a platform | High (Windows/macOS) | Medium | ADR-0003 fixes the refusal behaviour; documented limitation; no false guarantee | I / M |
| Stage B breadth (QUIC, H3, gRPC, OAuth) delays the release | High | Medium | P2d runs alongside P2b once transports land; the scope is fixed by the maintainer, so the date moves rather than the scope | M |
| Crate licence or MSRV conflict | Low | Medium | `cargo deny` in T-27; ADR-0002 | I |
| Documentation drifts from code | Medium | Medium | TASK-092 `vhco doc`; T-31 in CI; the plan is updated every task | I |
| Tests pass only on Linux | Medium | Medium | CI on three OSes (TASK-051) | I |
| Secrets leak through traces or errors | Low | High | Canary scan TASK-093; T-09/T-11 | I |

## Completion Criteria and Final Traceability

The plan is complete when every in-scope task is DONE (or NOT APPLICABLE with a reason) and the §34 gate is fully
checked (TASK-096).

| Requirement / UC | Implementation tasks | File changes | Tests | Docs / demo | Release update | Final status |
|---|---|---|---|---|---|---|
| R1 / UC-01, UC-10 | 010, 014, 016 | PF-01, PF-03, PF-21 | T-01, T-10, T-23, T-27 | D-05, D-15, D-36 | U-01 | NOT STARTED |
| R2 / UC-01 | 016, 017 | PF-06, PF-21 | T-01, T-23, T-29 | D-36, D-42 | U-02 | NOT STARTED |
| R3 / UC-03, 05, 10 | 022, 033–035 | PF-08, PF-22 | T-03, T-05, T-10 | D-16 | U-03 | NOT STARTED |
| R4 / UC-02, 03, 05, 07 | 015, 022, 036 | PF-05, PF-08 | T-03, T-24 | D-28 | U-04 | NOT STARTED |
| R5 / UC-04 | 023 | PF-09, PF-22 | T-04 | D-36, D-38 | U-05 | NOT STARTED |
| R6 / UC-09, 21 | 037, 038 | PF-11, PF-22 | T-09, T-25 | D-17, D-38 | U-06 | NOT STARTED |
| R7 / UC-06 | 040 | PF-10, PF-22 | T-06 | D-23, D-41 | U-07 | NOT STARTED |
| R8 / UC-02, 10, 21 | 022, 024, 025, 028, 032 | PF-03, PF-04, PF-20 | T-02, T-10 | D-18, D-24–D-27, D-37 | U-08 | NOT STARTED |
| R9 / UC-02, 03, 10 | 022, 025 | PF-08, PF-20 | T-02, T-03, T-10 | D-27, D-40 | U-09 | NOT STARTED |
| R10 / UC-07 | 036 | PF-08 | T-07, T-24 | D-16, D-36 | U-10 | NOT STARTED |
| R11 / UC-04, 06, 08, 09, 21, 22 | 020, 021, 035 | PF-12, PF-22, PF-25 | T-08, T-21 | D-04, D-17, D-29 | U-11 | NOT STARTED |
| R12 / UC-05 | 033–035, 094 | PF-19, PF-23 | T-05, T-08 | D-19, D-41 | U-12 | NOT STARTED |
| R13 / UC-05, 08, 09 | 021, 035, 037, 093 | PF-12, PF-22, PF-23 | T-05, T-08, T-09 | D-29 | U-13 | NOT STARTED |
| R14 / UC-01–22 | 004, 006, 052–072, 083–092 | PF-26, PF-27, PF-G01 | T-28, T-31, T-32 | D-01–D-47 | U-14 | NOT STARTED |
| R15 / UC-12 | 042 | PF-14, PF-24 | T-12, T-15 | D-19, D-41 | U-15 | NOT STARTED |
| R16 / UC-11 | 041 | PF-13, PF-24 | T-11, T-15 | D-20, D-41 | U-16 | NOT STARTED |
| R17 / UC-13 | 043 | PF-15, PF-24 | T-13, T-15 | D-19, D-41 | U-17 | NOT STARTED |
| R18 / UC-14 | 044 | PF-19, PF-23 | T-14, T-15 | D-19, D-41 | U-18 | NOT STARTED |
| R19 / UC-16 | 045 | PF-16, PF-24 | T-17 | D-19, D-41 | U-19 | NOT STARTED |
| R20 / UC-15 | 017, 019 | PF-06, PF-07 | T-16 | D-15, D-36 | U-20 | NOT STARTED |
| R21 / UC-17 | 032 | PF-20 | T-18 | D-26, D-39 | U-21 | NOT STARTED |
| R22 / UC-18, 20 | 029–031 | PF-17, PF-18, PF-20, PF-24 | T-19, T-22 | D-21, D-24, D-25, D-39 | U-22 | NOT STARTED |
| R23 / UC-19 | 018, 019, 022 | PF-06, PF-07, PF-08 | T-20 | D-15, D-36, D-37 | U-23 | NOT STARTED |
| R24 / UC-08, 22 | 020 | PF-12, PF-22 | T-21 | D-22, D-38 | U-24 | NOT STARTED |
| R25 / UC-20 | 026–032 | PF-18, PF-20, PF-24 | T-22 | D-18, D-30, D-39 | U-25 | NOT STARTED |
| R26 / UC-21, 22 | 038, 039 | PF-11, PF-12, PF-22 | T-25, T-26 | D-17, D-38 | U-26 | NOT STARTED |

```text
 R-n ──► TASK-xxx ──► PF-xx (src/…) ──► T-n (tests/… + TEST-2026-00nn) ──► D-xx (manual/system/api)
   ▲                                                                            │
   └──────────── U-n row in DEMO-2026-0014 ◄── REL-0.1.0 ◄── RPT-2026-0001 ◄────┘
```

## Post-Implementation Review

To be completed at P5 exit. It records what went as planned, the deviations (from the Findings table), incidents
and their preventive actions, estimate accuracy, and inputs to PLAN-2026-0002 (Stage C).

## Related Documents

- [PROP-2026-0001 — Rivet runtime proposal](../proposals/draft/prop-2026-0001-rivet-runtime.md) (baseline revision 6)
- [REF-2026-0001 — Request and evidence](../references/ref-2026-0001-request-and-evidence.md) (UQ-01–UQ-18)
- [REF-2026-0002 — Language and usage reference](../references/ref-2026-0002-language-and-usage.md) (S01–S153)
- [Sample folders](../demos/README.md) · [demos index](../demos/index.md)
- [Hand-authored contract](../../vhco-contract.json)
- [AGENTS.md](../../AGENTS.md) · [DOCUMENTATION.md](../../DOCUMENTATION.md) · [PROJECT.md](../../PROJECT.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial draft: P1–P5 phases, 96 tasks, file/test/documentation/release checklists, full R1–R26 traceability; awaiting P1 gates. |
