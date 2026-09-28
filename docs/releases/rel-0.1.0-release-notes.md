---
document_id: REL-0.1.0
title: "Rivet 0.1.0 release notes"
document_type: release
status: completed
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 2
authors: [Claude]
owner: Project maintainer
version: "0.1.0"
git_tag: v0.1.0
git_commit: f69b5b911f174b0198adb89c8305c8cce268fc11
release_date: 2026-09-28
source_branch: main
previous_version: null
previous_tag: null
systems: [Rivet]
components: [language, registry, execution, files, policy, audit, auth, connectors, datagrams, quic, grpc, sessions, serve, transports]
affected_versions:
  from: "0.1.0"
  to: null
confidentiality: internal
scope: First release of the Rivet runtime (PLAN-2026-0001, PROP-2026-0001 Stages A and B).
reason: DOCUMENTATION §33 — the release document is the traceability record of the release.
related_documents: [PLAN-2026-0001, PROP-2026-0001, RPT-2026-0001, DEMO-2026-0015, ADR-0001, ADR-0002, ADR-0003]
supersedes: null
superseded_by: null
tags: [rivet, release]
---

# Release 0.1.0

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** language, registry, execution, files, policy, audit, auth, connectors, datagrams, quic, grpc, sessions, serve, transports

## Release Identity

```text
Release Version:          0.1.0
Git Tag:                  v0.1.0
Git Commit:               f69b5b911f174b0198adb89c8305c8cce268fc11
Release Date:             2026-09-28
Source Branch:            main
Previous Version:         none (first release)
Previous Tag:             none
Previous Release Commit:  none (history starts at aa37459ea7c1)
Repository:               local; no git remote configured (push and CI pending, TASK-051/078)
Build:                    cargo build --release (Rust 1.90.0, aarch64-apple-darwin)
```

## Plan

[PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) implements [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md) (approved in [ADR-0001](../decisions/adr-0001-approve-rivet-runtime-design.md)).

```text
 PROP-2026-0001 ─► PLAN-2026-0001 ─► code (82+ commits) ─► TEST-2026-0001…0033 ─► RPT-2026-0001 ─► DEMO-2026-0015 ─► REL-0.1.0
```

## Project Standards Baseline

| Standards Index | Revision | Applicable Rules | Proposal Validation |
|---|---|---|---|
| [`docs/standards/index.md`](../standards/index.md) (REF-2026-0009) | 5 | AGENTS.md, DOCUMENTATION.md, VHCO five-folder architecture | PASS |

## User Requirements

| Requirement | Source | Released Update | Result |
|---|---|---|---|
| PROP-2026-0001 R1 | UQ-09/10: Rust library using Capy | U-01 | PARTIAL |
| PROP-2026-0001 R2 | UQ-03: simple, protocol-visible syntax | U-02 | PARTIAL |
| PROP-2026-0001 R3 | UQ-02: context closes resources | U-03 | PASS |
| PROP-2026-0001 R4 | UQ-03: good errors | U-04 | PASS |
| PROP-2026-0001 R5 | UQ-04: file CRUD | U-05 | PASS |
| PROP-2026-0001 R6 | UQ-05/18: list all I/O and trace attempts | U-06 | PASS |
| PROP-2026-0001 R7 | UQ-06: MCP bridge | U-07 | PASS |
| PROP-2026-0001 R8 | UQ-07/09: same ops on every surface | U-08 | PASS |
| PROP-2026-0001 R9 | UQ-08: request(ID, params, on_data) | U-09 | PASS |
| PROP-2026-0001 R10 | UQ-11: DAGs | U-10 | PASS |
| PROP-2026-0001 R11 | UQ-13/17: deny-by-default | U-11 | PARTIAL |
| PROP-2026-0001 R12 | UQ-01/12: protocol families | U-12 | PARTIAL |
| PROP-2026-0001 R13 | PROJECT §§6, 79–80: secrets, no shell | U-13 | PARTIAL |
| PROP-2026-0001 R14 | UQ-12, AGENTS/DOCUMENTATION | U-14 | PASS |
| PROP-2026-0001 R15 | UQ-14: UDP | U-15 | PASS |
| PROP-2026-0001 R16 | UQ-14: OAuth 2.0 | U-16 | PASS |
| PROP-2026-0001 R17 | UQ-14: QUIC | U-17 | PASS |
| PROP-2026-0001 R18 | UQ-14: HTTP/3 | U-18 | PASS |
| PROP-2026-0001 R19 | UQ-15: gRPC | U-19 | PASS |
| PROP-2026-0001 R20 | UQ-15: many described operations | U-20 | PASS |
| PROP-2026-0001 R21 | UQ-15: incoming MCP | U-21 | PASS |
| PROP-2026-0001 R22 | UQ-15/08/17: live streams everywhere | U-22 | PASS |
| PROP-2026-0001 R23 | UQ-17: declared outputs | U-23 | PASS |
| PROP-2026-0001 R24 | UQ-17: policy.json only | U-24 | PASS |
| PROP-2026-0001 R25 | UQ-17: one serve, all surfaces | U-25 | PASS |
| PROP-2026-0001 R26 | UQ-18: generated I/O manifest and policy draft | U-26 | PASS |

## Added

- **Language:** Capy-parsed `.rivet` bundles with many described operations per file. Declared inputs, outputs and errors; prefix calls; `if … else … end`; `try … catch`; `for`, `map`, `poll`, `dag`; `with` scopes that close their handles in reverse order; `with file open`.
- **Surfaces:** one dispatcher behind every surface:
  - the CLI (`request`, `list`, `describe`, `outputs`, `check`, `io`, `graph`, `policy explain|generate`, `trace show|export`, `auth …`, `connectors sync`, `serve`), plus `--endpoint` remote mode;
  - REST/SSE, polling, WebSocket `rivet.v1` and MCP (Streamable HTTP and stdio) from one `rivet serve`;
  - the Rust library (`Runtime::request`, `scope.stream/duplex`, `Policy::from_file/from_json`, host ceiling).
- **Policy:** `policy.json`-only authority, deny-by-default. `access` verbs, deny entries, the private-range rule, per-request `restrict`, limits and buffering budget.
- **I/O inspection:** the I/O manifest (`rivet io`, `--needs`, `--check-policy`, `--check-files`, `--trace`), least-privilege drafts (`rivet policy generate`) and the static call graph (`rivet graph`).
- **Protocols:** file CRUD with no-follow confinement; HTTP/1.1, 2 and 3; SSE/JSONL streams; TCP/Unix/WebSocket; processes (argv only, macOS Seatbelt sandbox); UDP (unicast/bind/multicast); QUIC v1; gRPC in all four modes; OAuth 2.0 (client credentials, PKCE, device code); MCP client connectors with reviewed snapshots and drift checks.
- **Operations:** sessions, traces with W3C `traceparent`, `rivet trace export`, `rivet.capabilities`, `/v1/health`, graceful drain on SIGINT/SIGTERM.

## Changed

Nothing: this is the first release.

## Fixed

Defects found during implementation and validation were fixed before release (see [Incidents](#incidents)): the eight incident records (13 of their defects found by executing the demos and documentation), 18 defects found by the suites, and the fix-batch gaps G1–G36 and B1–B3 recorded in the plan's findings table.

## Removed

Nothing. The proposal's `--sandbox` CLI flag was never implemented; policy comes from `policy.json` only (R24).

## Released Updates and Verification

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-01 | PROP-2026-0001 R1 | `capy_parser.rs` is the only Capy importer. It converts `ParseResult` into Rivet's `SyntaxTree`, and the library builds without a Capy executable | [12-library](../demos/12-library/README.md) step 2; `cargo tree -i capy-core --depth 0` | Program builds and exits 0; `capy-core v0.22.0 (…?rev=84f984c6…)` | Recorded 2026-09-28 at 829ca43; TEST-2026-0001, TEST-2026-0010; [DEMO-2026-0015](../demos/demo-2026-0015-v0-1-0-release-verification.md) |
| U-02 | PROP-2026-0001 R2 | `rivet.capy` grammar for every statement shape in REF-2026-0002; diagnostics carry file/line/column | `rivet --file app.rivet check --strict-docs` in each folder; a file ending in `return 1 +` | `ok: N operations …` (exit 0); `error[syntax.e0001] … --> bad.rivet:5:5` (exit 2) | All 12 folders parse (06 after its snapshot step); TEST-2026-0023, TEST-2026-0029; [DEMO-2026-0015](../demos/demo-2026-0015-v0-1-0-release-verification.md) |
| U-03 | PROP-2026-0001 R3 | `scope_supervisor` owns every handle and joins cleanup on success, error, break and cancel | [04-streaming](../demos/04-streaming/README.md) steps 4, 6, 7 | `close` logged each time; exits 0 / 6 / 130 | Recorded; TEST-2026-0003, TEST-2026-0005; [DEMO-2026-0015](../demos/demo-2026-0015-v0-1-0-release-verification.md) |
| U-04 | PROP-2026-0001 R4 | `domain/errors.rs` registry (code→kind→HTTP→exit→retryable); one terminal event | [03-http](../demos/03-http/README.md) steps 5, 7 | `users.not_found` 502/5, `http.status` 502/5, `output.invalid` 500/5, `validation.min` 422/2 | Recorded; TEST-2026-0024, [API-2026-0005](../api/api-2026-0005-error-registry.md); [DEMO-2026-0015](../demos/demo-2026-0015-v0-1-0-release-verification.md) |
| U-05 | PROP-2026-0001 R5 | `apply_file_operation` + `file_access` with no-follow handles, version guards and hard-link refusal | [02-file-crud](../demos/02-file-crud/README.md) steps 5–7 | `conflict.already_exists` 4; `not_found.file` 4; `file.hardlink_refused` 3 | Recorded; TEST-2026-0004; [DEMO-2026-0015](../demos/demo-2026-0015-v0-1-0-release-verification.md) |
| U-06 | PROP-2026-0001 R6 | Effect graph → IoManifest (targets and access verbs) → views/formats/check-policy; `effect_id` on trace attempts | [03-http](../demos/03-http/README.md) step 6; [11-sandbox](../demos/11-sandbox/README.md) step 8 | `2 allowed` for a retried GET; `1 allowed` for a read | Recorded; TEST-2026-0009; [DEMO-2026-0015](../demos/demo-2026-0015-v0-1-0-release-verification.md) |
| U-07 | PROP-2026-0001 R7 | `invoke_mcp` + `mcp_client` (stdio and Streamable HTTP), pinned snapshots | [06-mcp-bridge](../demos/06-mcp-bridge/README.md) steps 1–3, 6–7 | `not_found.mcp_snapshot` 4 → sync → `mcp.snapshot_unapproved` 2 → Ada; `mcp.tool_failed`; `mcp.schema_drift` | Recorded; TEST-2026-0006; [DEMO-2026-0015](../demos/demo-2026-0015-v0-1-0-release-verification.md) |
| U-08 | PROP-2026-0001 R8 | One dispatcher; the surface adapters only encode and decode | [01-catalog](../demos/01-catalog/README.md) steps 1, 4–8; [12-library](../demos/12-library/README.md) | `demo.add` returns 5 on every surface | Recorded; TEST-2026-0002; [DEMO-2026-0015](../demos/demo-2026-0015-v0-1-0-release-verification.md) |
| U-09 | PROP-2026-0001 R9 | `Runtime::request`, `scope.stream`, `next() -> Result<Option<Envelope>>` | [04-streaming](../demos/04-streaming/README.md) step 3; [12-library](../demos/12-library/README.md) step 2 | Data 1, 2, 3 then `{count:3}` | Recorded; TEST-2026-0003, TEST-2026-0010; [DEMO-2026-0015](../demos/demo-2026-0015-v0-1-0-release-verification.md) |
| U-10 | PROP-2026-0001 R10 | `run_dag` state machine and `DagCompletion`; default fail fast; depth and concurrency limits | [05-dag](../demos/05-dag/README.md) steps 2–3 | `{total:10}`; `succeeded/failed/blocked` | Recorded; TEST-2026-0007; [DEMO-2026-0015](../demos/demo-2026-0015-v0-1-0-release-verification.md) |
| U-11 | PROP-2026-0001 R11 | `load_policy` + `policy_broker`: no file means every effect is denied; the host ceiling is intersected with policy.json | [11-sandbox](../demos/11-sandbox/README.md) step 2; [12-library](../demos/12-library/README.md) step 2 (`.ceiling`) | `permission.denied` exit 3 under `empty.json` and for `data.private` | Recorded on macOS; Linux/Windows process sandbox not verifiable here (Known Caveats); TEST-2026-0008; [DEMO-2026-0015](../demos/demo-2026-0015-v0-1-0-release-verification.md) |
| U-12 | PROP-2026-0001 R12 | Availability matrix; transports feature (HTTP/socket/process); unsupported stages refuse | `request rivet.capabilities`; a `with file watch` operation | `stages … "C":"unsupported"`; `unsupported.stage_c` exit 5 | Recorded ([01-catalog](../demos/01-catalog/README.md) step 1 and failure example below); TEST-2026-0005; [DEMO-2026-0015](../demos/demo-2026-0015-v0-1-0-release-verification.md) |
| U-13 | PROP-2026-0001 R13 | argv-only processes; secrets bound to destinations; redaction | `command "/usr/bin/printf" args ["%s", "hello; echo this stays data"]`; [07-oauth2](../demos/07-oauth2/README.md) step 5 | Result `"hello; echo this stays data"`; `grep -c DEMO-AT` → 0 | Recorded; TEST-2026-0008, TEST-2026-0009; [DEMO-2026-0015](../demos/demo-2026-0015-v0-1-0-release-verification.md) |
| U-14 | PROP-2026-0001 R14 | All documents in the Documentation and Demo Checklist; indexes; traceability | `python3 scripts/check_docs.py`; `vhco docs check .` | `0 problem(s)`; `0 error` | `check_docs` 0 problems; `vhco docs check` 0 errors (index-page naming warnings only) — re-run after the component names in 13-real-world-apis were corrected; [DEMO-2026-0015](../demos/demo-2026-0015-v0-1-0-release-verification.md) |
| U-15 | PROP-2026-0001 R15 | `exchange_datagrams` + `udp_adapter` | [08-udp](../demos/08-udp/README.md) steps 7–13 | `{"state":"ready"}`; reply only to a granted peer; `udp.truncated` 5 / HTTP 502 | Recorded; TEST-2026-0012; [DEMO-2026-0015](../demos/demo-2026-0015-v0-1-0-release-verification.md) |
| U-16 | PROP-2026-0001 R16 | auth feature (begin/complete/status/disconnect/acquire/cancel) + `oauth_adapter` + `credential_store` | [07-oauth2](../demos/07-oauth2/README.md) steps 4–7 | Contacts returned; one token reused; `auth.client_secret_missing`, `auth.token_endpoint_failed`, `timeout.auth_token` | Recorded (client_credentials only); PKCE/device in TEST-2026-0011; [DEMO-2026-0015](../demos/demo-2026-0015-v0-1-0-release-verification.md) |
| U-17 | PROP-2026-0001 R17 | `exchange_quic` + `quic_adapter` | [09-quic](../demos/09-quic/README.md) steps 3–6 | `{"state":"ready"}`; `timeout` 6; `quic.tls` 5 | Recorded; TEST-2026-0013; [DEMO-2026-0015](../demos/demo-2026-0015-v0-1-0-release-verification.md) |
| U-18 | PROP-2026-0001 R18 | `http_adapter` version selection and safe fallback | [09-quic](../demos/09-quic/README.md) steps 4, 6 | `{"items":[],"version":3}`; `tls.handshake` with `request_sent:false` | Recorded; TEST-2026-0014; [DEMO-2026-0015](../demos/demo-2026-0015-v0-1-0-release-verification.md) |
| U-19 | PROP-2026-0001 R19 | `invoke_rpc` + `grpc_adapter`, all four modes | [10-grpc](../demos/10-grpc/README.md) steps 4–5 | Ada; two changes; `{count:2}`; echo; `grpc.not_found` 4, `grpc.unavailable` 5 | Recorded; TEST-2026-0017; [DEMO-2026-0015](../demos/demo-2026-0015-v0-1-0-release-verification.md) |
| U-20 | PROP-2026-0001 R20 | Multi-declaration compile; atomic duplicate detection | [01-catalog](../demos/01-catalog/README.md) step 1; [05-dag](../demos/05-dag/README.md) steps 1, 4 | Four IDs listed; helper `not_found.operation` 4 | Recorded; TEST-2026-0016; [DEMO-2026-0015](../demos/demo-2026-0015-v0-1-0-release-verification.md) |
| U-21 | PROP-2026-0001 R21 | `io/mcp` direct named tools plus built-ins | [01-catalog](../demos/01-catalog/README.md) step 8; [06-mcp-bridge](../demos/06-mcp-bridge/README.md) step 8 | `tools/list` then `tools/call demo.add` → 5; `isError` mapping | Recorded; TEST-2026-0018; [DEMO-2026-0015](../demos/demo-2026-0015-v0-1-0-release-verification.md) |
| U-22 | PROP-2026-0001 R22 | sessions feature + `session_driver`; polling and WS projections | [01-catalog](../demos/01-catalog/README.md) steps 5–7; [10-grpc](../demos/10-grpc/README.md) steps 4, 6–8 | Same items over SSE, polling, WS, MCP sessions | Recorded; TEST-2026-0019; [DEMO-2026-0015](../demos/demo-2026-0015-v0-1-0-release-verification.md) |
| U-23 | PROP-2026-0001 R23 | `compile_output_spec`, `validate_output`, `inspect_outputs`; `rivet outputs` and its projections | `rivet outputs --all --json` in any folder; [03-http](../demos/03-http/README.md) step 5 | Schemas per operation; `output.invalid` for an extra field | Recorded (see Known Caveats for `emits` descriptions); TEST-2026-0020; [DEMO-2026-0015](../demos/demo-2026-0015-v0-1-0-release-verification.md) |
| U-24 | PROP-2026-0001 R24 | `load_policy` (discovery, `--policy PATH`, schema v1, `access`); `--sandbox` never implemented | [11-sandbox](../demos/11-sandbox/README.md) steps 2, 7; [02-file-crud](../demos/02-file-crud/README.md) step 7 | Results per policy file; `policy.invalid` exit 2 | Recorded; TEST-2026-0021; [DEMO-2026-0015](../demos/demo-2026-0015-v0-1-0-release-verification.md) |
| U-25 | PROP-2026-0001 R25 | `start_serve`, `authenticate_principal`, `authorize_operation`, `multiplex_ws`, `project_polling` | [01-catalog](../demos/01-catalog/README.md) steps 3–9 | 200 / 401 / 403 / 404 as shown; `serve.auth_required` 2 | Recorded; TEST-2026-0022; [DEMO-2026-0015](../demos/demo-2026-0015-v0-1-0-release-verification.md) |
| U-26 | PROP-2026-0001 R26 | `inspect_effects` (manifest), `generate_policy`, `policy_draft_writer` | [11-sandbox](../demos/11-sandbox/README.md) steps 4–7 | Tables; exits 3 / 0 / 3; draft with 3 or 2 grants | Recorded; TEST-2026-0025, TEST-2026-0026; [DEMO-2026-0015](../demos/demo-2026-0015-v0-1-0-release-verification.md) |

## Tests

| Test | Requirement | Result |
|---|---|---|
| [TEST-2026-0001](../testing/test-2026-0001-language.md) | PLAN-2026-0001 R1, PLAN-2026-0001 R2 | PARTIAL |
| [TEST-2026-0002](../testing/test-2026-0002-surfaces.md) | PLAN-2026-0001 R8, PLAN-2026-0001 R9 | PASS |
| [TEST-2026-0003](../testing/test-2026-0003-streams.md) | PLAN-2026-0001 R3, PLAN-2026-0001 R4, PLAN-2026-0001 R9 | PASS |
| [TEST-2026-0004](../testing/test-2026-0004-files.md) | PLAN-2026-0001 R5, PLAN-2026-0001 R11 | PASS |
| [TEST-2026-0005](../testing/test-2026-0005-resources.md) | PLAN-2026-0001 R3, PLAN-2026-0001 R12, PLAN-2026-0001 R13 | PASS |
| [TEST-2026-0006](../testing/test-2026-0006-mcp.md) | PLAN-2026-0001 R7 | PASS |
| [TEST-2026-0007](../testing/test-2026-0007-dag.md) | PLAN-2026-0001 R10 | PASS |
| [TEST-2026-0008](../testing/test-2026-0008-sandbox.md) | PLAN-2026-0001 R11, PLAN-2026-0001 R13 | PARTIAL |
| [TEST-2026-0009](../testing/test-2026-0009-audit.md) | PLAN-2026-0001 R6, PLAN-2026-0001 R13 | PASS |
| [TEST-2026-0010](../testing/test-2026-0010-library.md) | PLAN-2026-0001 R1, PLAN-2026-0001 R3, PLAN-2026-0001 R9 | PASS |
| [TEST-2026-0011](../testing/test-2026-0011-oauth.md) | PLAN-2026-0001 R16 | PASS |
| [TEST-2026-0012](../testing/test-2026-0012-udp.md) | PLAN-2026-0001 R15 | PASS |
| [TEST-2026-0013](../testing/test-2026-0013-quic.md) | PLAN-2026-0001 R17 | PASS |
| [TEST-2026-0014](../testing/test-2026-0014-http3.md) | PLAN-2026-0001 R18 | PASS |
| [TEST-2026-0015](../testing/test-2026-0015-auth-transport-policy.md) | PLAN-2026-0001 R15, PLAN-2026-0001 R18 | PASS |
| [TEST-2026-0016](../testing/test-2026-0016-operation-catalog.md) | PLAN-2026-0001 R20 | PASS |
| [TEST-2026-0017](../testing/test-2026-0017-grpc.md) | PLAN-2026-0001 R19 | PASS |
| [TEST-2026-0018](../testing/test-2026-0018-mcp-catalog.md) | PLAN-2026-0001 R21 | PASS |
| [TEST-2026-0019](../testing/test-2026-0019-sessions.md) | PLAN-2026-0001 R22 | PASS |
| [TEST-2026-0020](../testing/test-2026-0020-outputs.md) | PLAN-2026-0001 R23 | PASS |
| [TEST-2026-0021](../testing/test-2026-0021-policy-file.md) | PLAN-2026-0001 R24 | PASS |
| [TEST-2026-0022](../testing/test-2026-0022-serve.md) | PLAN-2026-0001 R25 | PASS |
| [TEST-2026-0023](../testing/test-2026-0023-syntax.md) | PLAN-2026-0001 R1, PLAN-2026-0001 R2 | PASS |
| [TEST-2026-0024](../testing/test-2026-0024-errors-limits-dag.md) | PLAN-2026-0001 R4, PLAN-2026-0001 R10 | PASS |
| [TEST-2026-0025](../testing/test-2026-0025-io-manifest.md) | PLAN-2026-0001 R6, PLAN-2026-0001 R26 | PASS |
| [TEST-2026-0026](../testing/test-2026-0026-policy-generate.md) | PLAN-2026-0001 R26 | PASS |
| [TEST-2026-0027](../testing/test-2026-0027-build-static.md) | PLAN-2026-0001 R1 | PARTIAL |
| [TEST-2026-0028](../testing/test-2026-0028-architecture.md) | PLAN-2026-0001 R14 | PASS |
| [TEST-2026-0029](../testing/test-2026-0029-samples.md) | PLAN-2026-0001 R2, PLAN-2026-0001 R14 | PASS |
| [TEST-2026-0030](../testing/test-2026-0030-demos-e2e.md) | PLAN-2026-0001 R1, PLAN-2026-0001 R26 | PARTIAL |
| [TEST-2026-0031](../testing/test-2026-0031-documentation.md) | PLAN-2026-0001 R14 | PASS |
| [TEST-2026-0032](../testing/test-2026-0032-traceability.md) | PLAN-2026-0001 R1, PLAN-2026-0001 R26 | PASS |
| [TEST-2026-0033](../testing/test-2026-0033-release.md) | PLAN-2026-0001 R14 | PASS |

## Validation

[RPT-2026-0001](../reports/rpt-2026-0001-validation-of-plan-2026-0001.md) records 21 requirements PASS, 5 PARTIAL and 0 FAIL. Every PARTIAL comes from platform coverage: T-01, T-08 and T-27 ran on macOS only, and CI for Linux and Windows needs a git remote.

## Measured Results

NOT APPLICABLE: neither the proposal nor the plan made an optimization or measurable performance claim (plan §Measurable Claims covers conformance only).

## Incidents

| Incident | Severity | Title | Status |
|---|---|---|---|
| [INC-2026-0001](../incidents/resolved/inc-2026-0001-private-range-bypass-opaque-url-hosts.md) | S2 | Private-range rule skipped for udp://, quic:// and tcp:// IP literals | resolved |
| [INC-2026-0002](../incidents/resolved/inc-2026-0002-option-keyword-variable-misparse.md) | S3 | Variables named like option keywords were parsed as option lines | resolved |
| [INC-2026-0003](../incidents/resolved/inc-2026-0003-map-poll-assignment-and-until-misparse.md) | S3 | `x = map …` / `x = poll …` parsed as plain assignments; `until` conditions failed | resolved |
| [INC-2026-0004](../incidents/resolved/inc-2026-0004-dag-dependents-saw-bare-values.md) | S3 | DAG dependent nodes saw bare values instead of node envelopes | resolved |
| [INC-2026-0005](../incidents/resolved/inc-2026-0005-url-grant-path-prefix-match.md) | S2 | URL grant paths matched as raw string prefixes | resolved |
| [INC-2026-0006](../incidents/resolved/inc-2026-0006-codec-keyword-variable-collision.md) | S3 | Codec keywords replaced by same-named variables at run time | resolved |
| [INC-2026-0007](../incidents/resolved/inc-2026-0007-demo-verification-defects.md) | S3 | Defects found by release demo verification | resolved |
| [INC-2026-0008](../incidents/resolved/inc-2026-0008-documentation-verification-defects.md) | S3 | Defects found by documentation verification | resolved |

## Troubleshooting

- [TRBL-2026-0001](../troubleshooting/trbl-2026-0001-vhco-helper-files-counted-as-use-cases.md) — vhco counts helper files in a feature folder as use cases
- [TRBL-2026-0002](../troubleshooting/trbl-2026-0002-capy-section-headers-cannot-take-arguments.md) — Capy block sections cannot carry arguments (try/catch filters)
- [TRBL-2026-0003](../troubleshooting/trbl-2026-0003-linker-fails-with-no-space-left-on-device.md) — Build fails at link time with "No space left on device"

## Release Verification Guide and Demo

[DEMO-2026-0015](../demos/demo-2026-0015-v0-1-0-release-verification.md) — Rivet v0.1.0 release verification guide. It is executed against the release build, together with the twelve sample folders under [docs/demos](../demos/index.md).

## Manual

- [MAN-2026-0001](../manuals/man-2026-0001-rivet-manual.md) — Rivet manual
- [MAN-2026-0002](../manuals/man-2026-0002-installation-and-quickstart.md) — Rivet installation and quickstart
- [MAN-2026-0003](../manuals/man-2026-0003-language-guide.md) — Rivet language guide
- [MAN-2026-0004](../manuals/man-2026-0004-cli-reference.md) — Rivet CLI reference
- [MAN-2026-0005](../manuals/man-2026-0005-policy-and-io-manifest-guide.md) — Rivet policy and I/O manifest guide
- [MAN-2026-0006](../manuals/man-2026-0006-serving-and-surfaces.md) — Rivet serving and surfaces
- [MAN-2026-0007](../manuals/man-2026-0007-embedding-library.md) — Embedding Rivet as a Rust library
- [MAN-2026-0008](../manuals/man-2026-0008-protocols-and-connectors.md) — Rivet protocols and connectors

## System

- [API-2026-0001](../api/api-2026-0001-http-rest-sse-polling.md) — Rivet HTTP API: REST, SSE and polling
- [API-2026-0002](../api/api-2026-0002-websocket-rivet-v1.md) — Rivet WebSocket API: subprotocol rivet.v1
- [API-2026-0003](../api/api-2026-0003-mcp-server-tools.md) — Rivet MCP server: tools over Streamable HTTP and stdio
- [API-2026-0004](../api/api-2026-0004-rust-library.md) — Rivet Rust library API
- [API-2026-0005](../api/api-2026-0005-error-registry.md) — Rivet error registry
- [ARCH-2026-0001](../architecture/arch-2026-0001-rivet-runtime-architecture.md) — Rivet runtime architecture
- [ONB-2026-0001](../onboarding/onb-2026-0001-contributor-setup.md) — Rivet contributor setup
- [OPS-2026-0001](../operations/ops-2026-0001-operating-rivet-serve.md) — Operating rivet serve
- [RUN-2026-0001](../runbooks/run-2026-0001-rotate-serve-bearer-tokens.md) — Rotate rivet serve bearer tokens
- [RUN-2026-0002](../runbooks/run-2026-0002-roll-out-policy-change.md) — Roll out a policy.json change to rivet serve
- [SEC-2026-0001](../security/sec-2026-0001-policy-and-sandbox-model.md) — Rivet policy and sandbox security model
- [SYS-2026-0001](../system/components/sys-2026-0001-compiler-and-catalog.md) — Rivet compiler and operation catalog
- [SYS-2026-0003](../system/components/sys-2026-0003-policy-broker-and-io-manifest.md) — Rivet policy broker and I/O manifest
- [SYS-2026-0004](../system/components/sys-2026-0004-surfaces-and-serve.md) — Rivet surfaces and the serve listener
- [SYS-2026-0008](../system/configuration/sys-2026-0008-policy-json-reference.md) — policy.json schema v1 reference
- [SYS-2026-0005](../system/integrations/sys-2026-0005-protocol-adapters.md) — Rivet protocol adapters
- [SYS-2026-0006](../system/integrations/sys-2026-0006-oauth-and-credentials.md) — Rivet OAuth 2.0 and credentials
- [SYS-2026-0009](../system/integrations/sys-2026-0009-mcp-client-connectors.md) — Rivet MCP client connectors
- [SYS-2026-0002](../system/runtime/sys-2026-0002-execution-scopes-and-dag.md) — Rivet execution, scopes and DAG runtime
- [SYS-2026-0007](../system/runtime/sys-2026-0007-sessions.md) — Rivet duplex sessions

## Documentation Impact

| Artifact | Decision | Updated Document or Reason |
|---|---|---|
| Release verification guide / demo | UPDATED | DEMO-2026-0015 and 12 sample folders verified |
| README.md | UPDATED | Current state, Perch build/install, verified quickstart, limitations |
| System documentation | UPDATED | SYS-2026-0001…0009 |
| Architecture documentation | UPDATED | ARCH-2026-0001 |
| API and CLI reference | UPDATED | API-2026-0001…0005, MAN-2026-0004 |
| Manual | UPDATED | MAN-2026-0001…0008 |

## Source Changes

| Source | Change | Reason |
|---|---|---|
| `src/domain/` | Added | Value, error registry, IR, policy, manifest, contracts and port traits (R1–R26) |
| `src/features/` (language, registry, execution, files, policy, audit, auth, connectors, datagrams, quic, grpc, sessions, serve, transports) | Added | One use case per file (VHCO) |
| `src/infra/` | Added | Capy parser + `rivet.capy` grammar, interpreter, adapters, sandboxes, broker |
| `src/io/`, `src/orchestrator/` | Added | CLI, HTTP/SSE/poll/WS/MCP surfaces, library, runtime assembly |
| `tests/conformance_*.rs` | Added | T-01…T-29 suites (397 tests) |
| `Cargo.toml`, `Cargo.lock`, `deny.toml`, `rust-toolchain.toml` | Added | Build, pinned `capy-core`, licence/advisory policy |
| `scripts/check_docs.py`, `scripts/check_version.py`, `.github/workflows/ci.yml` | Added | T-31, T-33, CI (needs a remote) |
| `commands.perch` | Added | Developer build/install/gate commands (maintainer-approved) |
| `vhco-contract.json`, `vhco.json`, `vhco.html` | Modified | Contract reconciled with the implementation (`vhco sync` 0) |

## Known Issues

- Linux and Windows are unvalidated because there is no CI runner (T-01, T-08, T-27 PARTIAL).
- Push, CI and artifact publication wait on a git remote (TASK-051, TASK-078, TASK-080).
- `rivet io --include-bootstrap` labels the entry file `./app.rivet (+ imports)` although 0.1.0 has no import form (cosmetic; INC-2026-0008).
- `12-library/embedding.rs.txt` is compiled by hand, not as a Cargo `[[example]]` target (plan D-13).

## Limitations

```text
 not in 0.1.0                          behaviour
 ────────────────────────────────────  ─────────────────────────────────────────────
 mTLS serve authentication             refuses to start: unsupported.serve_mtls (exit 5)
 Linux / Windows process sandbox       unsupported.sandbox_backend (Linux gated, ADR-0003)
 import form                           single-file bundles
 finally                               try/catch only
 Stage C adapters                      unsupported.* (FIFO, watch, TCP TLS, interactive, reconnect)
 persistent trace store                process lifetime; export with rivet trace export
 Alt-Svc discovery, pooling            explicit HTTP/3 selection; one connection per request
 remote MCP effects                    reported as "unknown" (opaque trust boundary)
```

See the manual's limitations chapter for the complete list.

## Follow-up Work

- Configure a git remote, then run CI on three OSes and publish artifacts (TASK-051, 078, 080).
- Verify and ungate the Linux Landlock + seccomp backend on kernel ≥ 6.12 (ADR-0003).
- Stage C adapters, `import`, `finally` and mTLS serve go in a future plan (PLAN-2026-0002).

## Release Completion Gate (DOCUMENTATION §34)

Walked item by item on 2026-09-28 against tag `v0.1.0` (commit `f69b5b911f174b0198adb89c8305c8cce268fc11`).

| # | Condition | State | Evidence |
|---|---|---|---|
| 1 | An approved plan exists in plans/ | ✔ | PLAN-2026-0001 (approved) |
| 2 | Proposal preserves the request and traces problems → goals → requirements | ✔ | PROP-2026-0001 (R1–R26), REF-2026-0001 |
| 3 | Every user-facing requirement has a complete UC-NN | ✔ | PROP-2026-0001 UC-01…UC-22 |
| 4 | Every change / file CRUD row maps to a requirement and forward to validation | ✔ | T-32 (TEST-2026-0032): no orphans |
| 5 | Proposal records the standards index revision and passes applicable rules | ✔ | Standards baseline above (REF-2026-0009 rev 5) |
| 6 | Implementation corresponds to the plan; deviations documented | ✔ | Plan Decisions/Deviations table; RPT-2026-0001 |
| 7 | No unexplained open item in the live plan | ✔ | Only TASK-051/078/080/081 BLOCKED (no git remote) and TASK-082 waiting on them; all explained |
| 8 | Unexpected bugs documented in incidents/ | ✔ | INC-2026-0001…0008, all resolved |
| 9 | Significant technical problems in troubleshooting/ | ✔ | TRBL-2026-0001…0003 |
| 10 | Tests completed and recorded in testing/ | ✔ | TEST-2026-0001…0033 |
| 11 | Every test links its plan requirement | ✔ | `validated_plan_requirements` in every TEST document |
| 12–14 | Measurable claims: baseline, execution, summary | N/A | No optimization or measurable claim was made (Measured Results) |
| 15 | Validation report with a result per requirement | ✔ | RPT-2026-0001 (completed) |
| 16 | Every PARTIAL/FAIL documented and referenced | ✔ | Validation, Known Issues above |
| 17 | Release verification guide with one U-NN row per update | ✔ | DEMO-2026-0015, U-01…U-26 |
| 18 | U-NN rows link requirement, action, expected result, evidence | ✔ | Released Updates table above |
| 19 | Demo commands actually executed and verified | ✔ | TEST-2026-0030; smoke re-run of the tagged release binary |
| 20 | Manual changes integrated into the correct chapters | ✔ | MAN-2026-0001…0008 |
| 21 | Feature catalogue covers every supported addition | ✔ | MAN-2026-0001 feature catalogue |
| 22 | Every CLI command, route, config item and error documented | ✔ | MAN-2026-0004, API-2026-0001…0005 |
| 23–27 | README, system, architecture, API/CLI, manual impact recorded | ✔ | Documentation Impact table above (all UPDATED) |
| 28 | Code and system documentation consistent | ✔ | TASK-092; `vhco sync` 0; `vhco docs check` 0 errors |
| 29 | on-release documents reviewed | ✔ | TASK-091 (next_review_date 2026-10-28) |
| 30 | Canonical project version updated | ✔ | Cargo.toml `0.1.0` |
| 31 | All version references synchronized | ✔ | T-33, `scripts/check_version.py --tag` |
| 32 | Final release state committed | ✔ | `f69b5b9 release: v0.1.0` |
| 33 | Working tree clean | ✔ | At the release commit (T-33) |
| 34 | Full release commit SHA recorded | ✔ | Release Identity above |
| 35 | Tag created and matches the version | ✔ | annotated `v0.1.0` |
| 36 | Tag resolves to the recorded commit | ✔ | `git rev-list -n 1 v0.1.0` = f69b5b9… |
| 37 | Commit and tag pushed, when applicable | N/A | No remote is configured; push once one exists (TASK-078) |
| 38 | Known issues and limitations documented | ✔ | Known Issues, Limitations above; MAN-2026-0001 |
| 39 | Final release document in releases/ | ✔ | This document |
| 40 | Version, tag and full commit hash present | ✔ | Front matter and Release Identity |
| 41 | Requirement traceability, update verification, measured results | ✔ | User Requirements, Released Updates, Measured Results |
| 42 | Six documentation-impact decisions recorded | ✔ | Documentation Impact |
| 43 | All relevant artifacts linked | ✔ | Sections above |
| 44 | Accurately represents the tagged source state | ✔ | Generated from the tagged tree; this finalizing commit changes documentation only |
| 45 | Indexes regenerated | ✔ | docs/index.md, releases/, testing/, reports/, proposals/ |

## Related Documents

- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) · [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)
- [RPT-2026-0001](../reports/rpt-2026-0001-validation-of-plan-2026-0001.md) · [Tests](../testing/index.md) · [Incidents](../incidents/index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 2 | 2026-09-28 | Claude | Finalized with tag v0.1.0 and commit f69b5b911f174b0198adb89c8305c8cce268fc11. |
| 1 | 2026-09-28 | Claude | Drafted for the release commit (tag and commit recorded after tagging). |
