---
document_id: TEST-2026-0035
title: "T-02 — One input envelope on every surface (UC-01 / R4)"
document_type: test
status: completed
created_date: 2026-09-29
last_updated: 2026-09-29
document_revision: 1
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [serve, cli, http, ws, poll, mcp, library, ffi]
affected_versions:
  from: "0.2.0"
  to: null
validated_plan_requirements: [PLAN-2026-0002 R4]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and recorded result for PLAN-2026-0002 test T-02 (PROP-2026-0002 T-02).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates (PLAN-2026-0002 TASK-061).
related_documents: [PLAN-2026-0002, PROP-2026-0002, ADR-0004]
supersedes: null
superseded_by: null
tags: [rivet, test, v0.2.0]
---

# T-02 — One input envelope on every surface (UC-01 / R4)

> **Status:** Completed
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** serve, cli, http, ws, poll, mcp, library, ffi

## Purpose

Show that one `{operation, data}` input file gives identical `data` through the CLI (`--input FILE`, `--input -`, `--endpoint`), HTTP, WebSocket, polling, MCP, the library and the C ABI.

```text
                 input.json  {"operation":"demo.add","data":{"a":2,"b":3}}
   ┌──────────────┬──────────┬──────┬─────────┬──────┬─────────┬──────────────┐
   --input FILE   --input -  POST   WS frame  poll   MCP       Runtime::call   rivet_request
   └──────────────┴──────────┴──────┴─────────┴──────┴─────────┴──────────────┘
                        serve.parse_input ─► identical `data` (5)
```

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0002 R4 | `InputEnvelope::parse` shared by every surface; CLI `--data` and `--input FILE|-` |

Plan row: [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md#test-and-validation-checklist) T-02 (integration, UC-01).

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first so `librivet` exists for the FFI suites (TRBL-2026-0006); Python 3 and a C compiler on `PATH`. Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

## Test Data

The fixtures, bundles and goldens defined in `tests/conformance_envelope.rs`, `tests/conformance_verification_defects.rs`, `tests/conformance_ffi.rs` and the shared helpers under `tests/` (in-process servers, temporary directories, `tests/fixtures/`).

## Procedure

```sh
cargo test --all-features --test conformance_envelope
cargo test --all-features --test conformance_verification_defects
cargo test --all-features --test conformance_ffi
```

Tests executed (5):

- `conformance_envelope::t02_one_input_file_on_every_surface` — T-02: one input envelope file {operation, data} gives identical `data` through `rivet request --input FILE`, `--input -`, `--endpoint … --input`, POST /v1/request, POST /v1/requests, a WS request frame, MCP rivet.request and Runtime::call_json
- `conformance_verification_defects::rejected_envelope_echoes_the_operation` — INC-2026-0012 item 4: an input envelope refused after naming an operation string (wrong-typed deadline_ms) answers with that operation on REST, WebSocket and Runtime::call_json, not null
- `conformance_verification_defects::remote_ws_sends_the_timeout` — INC-2026-0012 item 11: `rivet --endpoint URL request ID --stream --input-jsonl - --timeout 300ms` sends deadline_ms on the WebSocket request frame, so the duplex ref times out (timeout.request) instead of running under the 30 s default
- `conformance_ffi::t11_c_demo_shared_and_static` — T-11: the C demo linked shared and static prints envelopes, a stream (3 data + result), live input echoes, a cancelled call, highlight tokens, and refuses a double free
- `conformance_ffi::t11_python_demo` — T-11: the Python ctypes demo requests, streams, sends live input, cancels and highlights through librivet

## Expected Results

Identical `data` on every surface (plan T-02).

## Actual Results

`t02_one_input_file_on_every_surface` passed: the same input envelope gives the same `data` through `rivet request --input FILE`, `--input -`, the remote `--endpoint` client, POST `/v1/request`, polling, WebSocket, MCP `rivet.request` and `Runtime::call`. The FFI leg (`rivet_request` from C and Python) passed in `conformance_ffi`. INC-2026-0012 regressions for input handling (operation echoed on a rejected envelope; WS `deadline_ms` from `--timeout`) passed. Linux: green in CI run 36505156729.

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

None in this run ([INC-2026-0012](../incidents/resolved/inc-2026-0012-documentation-and-demo-verification-defects.md) items 4 and 11 fixed before it).

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-02
- [PROP-2026-0002](../proposals/approved/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md)
- [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md) · [Tests index](index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-29 | Claude | Created and executed at commit `14750b8` (PLAN-2026-0002 P3, TASK-061); result PASS. |
