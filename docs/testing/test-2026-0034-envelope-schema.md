---
document_id: TEST-2026-0034
title: "T-01 — Response envelope schema on every surface (UC-01 / R1, R2)"
document_type: test
status: completed
created_date: 2026-09-29
last_updated: 2026-09-29
document_revision: 1
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [serve, cli, http, ws, poll, mcp, library]
affected_versions:
  from: "0.2.0"
  to: null
validated_plan_requirements: [PLAN-2026-0002 R1, PLAN-2026-0002 R2]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and recorded result for PLAN-2026-0002 test T-01 (PROP-2026-0002 T-01).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates (PLAN-2026-0002 TASK-061).
related_documents: [PLAN-2026-0002, PROP-2026-0002, ADR-0004]
supersedes: null
superseded_by: null
tags: [rivet, test, v0.2.0]
---

# T-01 — Response envelope schema on every surface (UC-01 / R1, R2)

> **Status:** Completed
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** serve, cli, http, ws, poll, mcp, library

## Purpose

Show that every surface answers ok, error, cancelled and accepted outcomes with one `ResponseEnvelope` that validates against `docs/api/schemas/response.schema.json` and keeps the fixed key order.

```text
  CLI ─┐   HTTP ─┐  SSE ─┐  NDJSON ─┐  polling ─┐  WS ─┐  MCP ─┐  library ─┐  C ABI ─┐
       └────────┴──────┴─────────┴───────────┴───────┴───────┴──────────┴─────────┘
                                         │
            {request_id, trace_id, operation, type, status, data, error, effects, data_count}
                                         │
                   response.schema.json ─┴─► ok · error · cancelled · accepted
```

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0002 R1 | `ResponseEnvelope` serializer used by every surface (CLI, HTTP, SSE, NDJSON, polling, WS, MCP, library, FFI) |
| PLAN-2026-0002 R2 | `status` ok / error / cancelled / accepted; `type` result / data; `effects` at the top level |

Plan row: [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md#test-and-validation-checklist) T-01 (integration, UC-01).

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first so `librivet` exists for the FFI suites (TRBL-2026-0006); Python 3 and a C compiler on `PATH`. Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

## Test Data

The fixtures, bundles and goldens defined in `tests/conformance_envelope.rs`, `tests/conformance_verification_defects.rs`, `tests/conformance_ffi.rs` and the shared helpers under `tests/` (in-process servers, temporary directories, `tests/fixtures/`).

## Procedure

```sh
cargo test --workspace --all-targets --all-features --no-fail-fast   # full run, output saved to /tmp/p3-tests.log
cargo test --all-features --test conformance_envelope
cargo test --all-features --test conformance_verification_defects
cargo test --all-features --test conformance_ffi
```

Tests executed (7):

- `conformance_envelope::t01_envelope_schema_on_every_surface` — T-01: every surface (CLI, HTTP, SSE, NDJSON, polling, WS, MCP structuredContent, library) answers ok, error, cancelled and accepted outcomes with a ResponseEnvelope that validates against docs/api/schemas and keeps the R1 key order; every error kind renders status error (cancelled for kind cancelled) with `effects` at the top level
- `conformance_verification_defects::mcp_errors_name_the_target_or_null` — INC-2026-0012 items 5 and 6: an MCP rivet.request error envelope names the target operation (like its success envelope); mcp.session_required has operation null, not the JSON-RPC method
- `conformance_verification_defects::nested_errors_have_no_effects` — INC-2026-0012 item 7: `effects` appears once, at the envelope's top level: nested suppressed[] and cause errors carry none
- `conformance_verification_defects::ws_open_refusals_carry_ids` — INC-2026-0012 item 8: a WS request (and a polling open) refused at open (validation.required, not_found.operation) carries request and trace IDs like REST, the CLI and MCP; a unary WS result keeps seq 1 (every WS ref is a session, API-2026-0002)
- `conformance_verification_defects::rejected_envelope_echoes_the_operation` — INC-2026-0012 item 4: an input envelope refused after naming an operation string (wrong-typed deadline_ms) answers with that operation on REST, WebSocket and Runtime::call_json, not null
- `conformance_ffi::t11_c_demo_shared_and_static` — T-11: the C demo linked shared and static prints envelopes, a stream (3 data + result), live input echoes, a cancelled call, highlight tokens, and refuses a double free
- `conformance_ffi::t11_python_demo` — T-11: the Python ctypes demo requests, streams, sends live input, cancels and highlights through librivet

## Expected Results

Every output validates against the schema; key order fixed (plan T-01).

## Actual Results

`t01_envelope_schema_on_every_surface` passed: the CLI, HTTP, SSE, NDJSON, polling, WebSocket, MCP `structuredContent` and library outputs for ok, error, cancelled and accepted outcomes all validate against `response.schema.json` with the fixed key order. The C ABI leg is exercised by `conformance_ffi` (C shared/static and Python print envelopes). The INC-2026-0012 regressions that tightened envelope consistency (MCP error `operation`, no nested `effects`, IDs on WS/polling refusals at open, operation echoed on rejected input) passed. Linux: green in CI run 36505156729 (`test (ubuntu-latest)`), which runs the same `cargo test --workspace --all-targets --all-features`.

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

## Evidence

```text
conformance_envelope                 test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.58s
conformance_verification_defects     test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.64s
conformance_ffi                      test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.44s

GitHub Actions run 36505156729 (commit 14750b8, conclusion: success)
  test (ubuntu-latest)  success    test (macos-latest)  success
  features (none|serve|grpc|quic|oauth|cli)  success ×6    deny  success
```

## Evidence Sources

- The commands above, run at commit `14750b8`; full-suite output saved to `/tmp/p3-tests.log` (493 passed, 0 failed) and read with `grep -E "test result|FAILED|panicked"`.
- Test source: `tests/conformance_envelope.rs`, `tests/conformance_verification_defects.rs`, `tests/conformance_ffi.rs`.
- CI: [run 36505156729](https://github.com/olivierdevelops/rivet/actions/runs/36505156729) at the same commit (`test (ubuntu-latest)`, `test (macos-latest)`, the six `features` jobs and `deny`, all `success`).
- Release verification cross-reference: [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md).

## Executed By

Claude (automated run, PLAN-2026-0002 P3).

## Executed At

2026-09-29T14:23:39Z

## Defects Raised

None in this run. The envelope defects found by documentation verification were fixed before it ([INC-2026-0012](../incidents/resolved/inc-2026-0012-documentation-and-demo-verification-defects.md) items 2–8, 16).

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-01
- [PROP-2026-0002](../proposals/implemented/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md)
- [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md) · [Tests index](index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-29 | Claude | Created and executed at commit `14750b8` (PLAN-2026-0002 P3, TASK-061); result PASS. |
