---
document_id: REF-2026-0033
title: "Rivet tests"
document_type: reference
status: active
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 2
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [language, execution, files, policy, audit, serve, transports, library, ffi]
affected_versions:
  from: "0.1.0"
  to: null
confidentiality: internal
scope: Navigation and status page for TEST documents.
reason: AGENTS.md requires an index.md in every documentation directory.
related_documents: [PLAN-2026-0001, PLAN-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, index, test]
---

# Rivet tests

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0, 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** language, execution, files, policy, audit, serve, transports, library, ffi

## Purpose

This page lists the TEST documents (DOCUMENTATION §27). Each one maps to one row of a plan's Test and Validation Checklist and records the most recent run. TEST-2026-0001…0033 belong to [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) (v0.1.0). They were re-recorded for v0.2.0, and each keeps its v0.1.0 run as history. TEST-2026-0034…0053 are new for [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) (v0.2.0).

```text
 PLAN-2026-0002 row                     TEST document                    command / source
 ─────────────────────────────────────  ───────────────────────────────  ─────────────────────────────────────────
 T-01 … T-20 (proposal tests) ────────► TEST-2026-0034 … 0053 (new) ───► cargo test --test conformance_<suite>
 T-27 build · T-28 vhco · T-29 corpus ─► TEST-2026-0027 · 0028 · 0029 ──► gates, vhco, conformance_samples
 T-30 demos · T-31 docs · T-32 trace ──► TEST-2026-0030 · 0031 · 0032 ──► demo records, check_docs, trace script
 T-33 release (P5) ───────────────────► TEST-2026-0033 (still the v0.1.0 record until P5)
 T-34 regression ─────────────────────► TEST-2026-0001 … 0026 (re-recorded, each keeps its v0.1.0 run)
                │
                └──► result per requirement R1–R24 ──► RPT-2026-0015 (validation report) ──► REL-0.2.0
```

Environment of the v0.2.0 runs: macOS 26.4.1 arm64 locally at commit `14750b8`, plus CI run 36505156729 at the same commit (ubuntu-latest and macos-latest, the six-entry feature matrix, `cargo deny`). Windows is not a supported platform in 0.2.0 ([INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)).

## Active documents

### PLAN-2026-0002 (v0.2.0): new tests

| Test | Title | Result |
|---|---|---|
| [TEST-2026-0034](test-2026-0034-envelope-schema.md) | T-01 — Response envelope schema on every surface (UC-01 / R1, R2) | PASS |
| [TEST-2026-0035](test-2026-0035-input-envelope.md) | T-02 — One input envelope on every surface (UC-01 / R4) | PASS |
| [TEST-2026-0036](test-2026-0036-builtin-json-envelopes.md) | T-03 — Built-ins, GET routes and CLI --json outputs are envelopes (UC-01 / R3) | PASS |
| [TEST-2026-0037](test-2026-0037-stream-records.md) | T-04 — Stream records then exactly one result (UC-02 / R1) | PASS |
| [TEST-2026-0038](test-2026-0038-pretty-json.md) | T-05 — Pretty JSON goldens and stream refusal (UC-03 / R6) | PASS |
| [TEST-2026-0039](test-2026-0039-globals-values.md) | T-06 — Globals are visible everywhere and equal under concurrency (UC-04 / R7) | PASS |
| [TEST-2026-0040](test-2026-0040-globals-diagnostics.md) | T-07 — Every global diagnostic with its span (UC-04 / R8) | PASS |
| [TEST-2026-0041](test-2026-0041-globals-manifest.md) | T-08 — Globals in the I/O manifest and exact policy grants (UC-04 / R9) | PASS |
| [TEST-2026-0042](test-2026-0042-library-facade.md) | T-09 — An external crate uses only the facade (UC-05 / R10) | PASS |
| [TEST-2026-0043](test-2026-0043-cargo-features.md) | T-10 — Cargo feature matrix, unsupported.feature and packaging (UC-05 / R11, R12) | PASS |
| [TEST-2026-0044](test-2026-0044-c-abi.md) | T-11 — C ABI: shared and static librivet from C and Python (UC-06 / R13, R15) | PASS |
| [TEST-2026-0045](test-2026-0045-ffi-safety.md) | T-12 — FFI misuse never crashes or leaks (UC-06 / R14) | PASS |
| [TEST-2026-0046](test-2026-0046-editor-grammar.md) | T-13 — TextMate grammar, keyword drift and .vsix packaging (UC-07 / R16) | PASS |
| [TEST-2026-0047](test-2026-0047-highlight-cli.md) | T-14 — rivet highlight goldens and partial tokens (UC-08 / R17) | PASS |
| [TEST-2026-0048](test-2026-0048-legacy-input.md) | T-15 — Legacy {id, params} input with deprecation signals (UC-09 / R5) | PASS |
| [TEST-2026-0049](test-2026-0049-file-modules.md) | T-16 — File modules: import, namespaces and visibility (UC-10 / R19, R20) | PASS |
| [TEST-2026-0050](test-2026-0050-import-errors.md) | T-17 — Every import error with its span and exit (UC-10 / R21) | PASS |
| [TEST-2026-0051](test-2026-0051-host-module-load.md) | T-18 — Host module objects: Runtime::load and Module (UC-11 / R22) | PASS |
| [TEST-2026-0052](test-2026-0052-ffi-modules.md) | T-19 — Module objects from C and Python (UC-11 / R23) | PASS |
| [TEST-2026-0053](test-2026-0053-module-policy.md) | T-20 — The loader's policy governs every module (UC-10, UC-11 / R24) | PASS |

### PLAN-2026-0002 plan-level checks (re-recorded documents)

| Test | PLAN-2026-0002 row | Title (from PLAN-2026-0001) | v0.1.0 | v0.2.0 |
|---|---|---|---|---|
| [TEST-2026-0027](test-2026-0027-build-static.md) | T-27 | T-27 — build / static (all / R1) | PARTIAL | PASS |
| [TEST-2026-0028](test-2026-0028-architecture.md) | T-28 | T-28 — architecture (all / R14) | PASS | PASS |
| [TEST-2026-0029](test-2026-0029-samples.md) | T-29 | T-29 — regression / corpus (UC-01 / R2, R14) | PASS | PASS |
| [TEST-2026-0030](test-2026-0030-demos-e2e.md) | T-30 | T-30 — manual / e2e (all UCs / R1–R26) | PARTIAL | PARTIAL |
| [TEST-2026-0031](test-2026-0031-documentation.md) | T-31 | T-31 — documentation (R14) | PASS | PASS |
| [TEST-2026-0032](test-2026-0032-traceability.md) | T-32 | T-32 — traceability (R1–R26) | PASS | PASS |
| [TEST-2026-0033](test-2026-0033-release.md) | T-33 | T-33 — release (release) | PASS | P5 (not yet re-recorded) |

### PLAN-2026-0002 T-34 regression (the PLAN-2026-0001 suites, re-recorded)

| Test | Title (from PLAN-2026-0001) | v0.1.0 | v0.2.0 |
|---|---|---|---|
| [TEST-2026-0001](test-2026-0001-language.md) | T-01 — unit + integration (UC-01 / R1, R2) | PARTIAL | PASS |
| [TEST-2026-0002](test-2026-0002-surfaces.md) | T-02 — integration / e2e (UC-02 / R8, R9) | PASS | PASS |
| [TEST-2026-0003](test-2026-0003-streams.md) | T-03 — fault / resource (UC-03 / R3, R4, R9) | PASS | PASS |
| [TEST-2026-0004](test-2026-0004-files.md) | T-04 — security / edge (UC-04 / R5, R11) | PASS | PASS |
| [TEST-2026-0005](test-2026-0005-resources.md) | T-05 — integration / fault (UC-05 / R3, R12, R13) | PASS | PASS |
| [TEST-2026-0006](test-2026-0006-mcp.md) | T-06 — integration (UC-06 / R7) | PASS | PASS |
| [TEST-2026-0007](test-2026-0007-dag.md) | T-07 — unit / integration (UC-07 / R10) | PASS | PASS |
| [TEST-2026-0008](test-2026-0008-sandbox.md) | T-08 — security / compatibility (UC-08 / R11, R13) | PARTIAL | PASS |
| [TEST-2026-0009](test-2026-0009-audit.md) | T-09 — security / audit (UC-09 / R6, R13) | PASS | PASS |
| [TEST-2026-0010](test-2026-0010-library.md) | T-10 — integration (UC-10 / R1, R3, R9) | PASS | PASS |
| [TEST-2026-0011](test-2026-0011-oauth.md) | T-11 — integration / security (UC-11 / R16) | PASS | PASS |
| [TEST-2026-0012](test-2026-0012-udp.md) | T-12 — integration (UC-12 / R15) | PASS | PASS |
| [TEST-2026-0013](test-2026-0013-quic.md) | T-13 — integration / security (UC-13 / R17) | PASS | PASS |
| [TEST-2026-0014](test-2026-0014-http3.md) | T-14 — integration (UC-14 / R18) | PASS | PASS |
| [TEST-2026-0015](test-2026-0015-auth-transport-policy.md) | T-15 — cross-surface / security (UC-11–14 / R15–R18) | PASS | PASS |
| [TEST-2026-0016](test-2026-0016-operation-catalog.md) | T-16 — unit / integration (UC-15 / R20) | PASS | PASS |
| [TEST-2026-0017](test-2026-0017-grpc.md) | T-17 — integration (UC-16 / R19) | PASS | PASS |
| [TEST-2026-0018](test-2026-0018-mcp-catalog.md) | T-18 — e2e (UC-17 / R21) | PASS | PASS |
| [TEST-2026-0019](test-2026-0019-sessions.md) | T-19 — e2e / fault (UC-18 / R22) | PASS | PASS |
| [TEST-2026-0020](test-2026-0020-outputs.md) | T-20 — unit / e2e (UC-19 / R23) | PASS | PASS |
| [TEST-2026-0021](test-2026-0021-policy-file.md) | T-21 — security (UC-08 / R24) | PASS | PASS |
| [TEST-2026-0022](test-2026-0022-serve.md) | T-22 — e2e / security (UC-20 / R25) | PASS | PASS |
| [TEST-2026-0023](test-2026-0023-syntax.md) | T-23 — unit (UC-01 / R1, R2) | PASS | PASS |
| [TEST-2026-0024](test-2026-0024-errors-limits-dag.md) | T-24 — unit / resource (UC-02, UC-07 / R4, R10) | PASS | PASS |
| [TEST-2026-0025](test-2026-0025-io-manifest.md) | T-25 — integration (UC-21, UC-09 / R6, R26) | PASS | PASS |
| [TEST-2026-0026](test-2026-0026-policy-generate.md) | T-26 — integration (UC-22 / R26) | PASS | PASS |

```text
 v0.2.0 results        PASS  PARTIAL  FAIL
 ───────────────────── ────  ───────  ────
 new (0034–0053)         20        0     0
 plan checks (27–32)      5        1     0      T-30: demo records predate the INC-2026-0012 fixes; tagged-build run is P5
 T-34 (0001–0026)        26        0     0
 T-33 (0033)            — re-recorded at P5 —
```

The v0.1.0 PARTIAL results of TEST-2026-0001, 0008 and 0027 existed only because Linux and Windows had no CI runner. Linux is now covered and green. Windows is no longer a supported platform, so these tests are now PASS. For TEST-2026-0008, the Linux sandbox stays gated by design ([ADR-0003](../decisions/adr-0003-process-sandbox-backends.md)); the typed refusal before spawn is the outcome the criterion specifies (see that document). TEST-2026-0030 remains PARTIAL for the reasons it states.

## Recently added or updated

- 2026-09-29: PLAN-2026-0002 P3 (TASK-061). TEST-2026-0034…0053 created. TEST-2026-0001…0032 re-recorded for v0.2.0 at commit `14750b8` (revision 2), each keeping its v0.1.0 run.
- 2026-09-28: created during PLAN-2026-0001 P3.

## Deprecated, superseded or archived

None.

## Related directories

[plans](../plans/index.md) · [incidents](../incidents/index.md) · [demos](../demos/index.md) · [current state](../README.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 2 | 2026-09-29 | Claude | PLAN-2026-0002 P3: TEST-2026-0034…0053 added; 0001…0032 re-recorded for v0.2.0; tables split by plan row; results summary. |
| 1 | 2026-09-28 | Claude | Created. |
