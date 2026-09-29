---
document_id: TEST-2026-0038
title: "T-05 — Pretty JSON goldens and stream refusal (UC-03 / R6)"
document_type: test
status: completed
created_date: 2026-09-29
last_updated: 2026-09-29
document_revision: 1
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [cli, http, library, ffi]
affected_versions:
  from: "0.2.0"
  to: null
validated_plan_requirements: [PLAN-2026-0002 R6]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and recorded result for PLAN-2026-0002 test T-05 (PROP-2026-0002 T-05).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates (PLAN-2026-0002 TASK-061).
related_documents: [PLAN-2026-0002, PROP-2026-0002, ADR-0004]
supersedes: null
superseded_by: null
tags: [rivet, test, v0.2.0]
---

# T-05 — Pretty JSON goldens and stream refusal (UC-03 / R6)

> **Status:** Completed
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** cli, http, library, ffi

## Purpose

Show that pretty output is the golden 2-space indented envelope with unchanged key order, and that pretty is refused where a record must stay on one line.

```text
  --pretty │ ?pretty=true │ to_json_pretty() │ FFI {"pretty":true} ─► 2-space indent, same key order
  --pretty --stream  ─► validation.usage (exit 2)
  SSE + ?pretty=true ─► validation.pretty_stream (HTTP 400)
```

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0002 R6 | `--pretty`, `?pretty=true`, `to_json_pretty`, FFI `pretty`; refused on NDJSON/SSE |

Plan row: [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md#test-and-validation-checklist) T-05 (integration, UC-03).

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first so `librivet` exists for the FFI suites (TRBL-2026-0006); Python 3 and a C compiler on `PATH`. Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

## Test Data

The fixtures, bundles and goldens defined in `tests/conformance_envelope.rs`, `tests/conformance_ffi.rs` and the shared helpers under `tests/` (in-process servers, temporary directories, `tests/fixtures/`).

## Procedure

```sh
cargo test --all-features --test conformance_envelope
cargo test --all-features --test conformance_ffi
```

Tests executed (2):

- `conformance_envelope::t05_pretty_goldens_and_stream_refusal` — T-05: --pretty, ?pretty=true and to_json_pretty give the golden 2-space indented envelope with unchanged key order; --pretty with --stream is validation.usage (exit 2) and ?pretty=true with SSE is 400 validation.pretty_stream
- `conformance_ffi::t11_c_demo_shared_and_static` — T-11: the C demo linked shared and static prints envelopes, a stream (3 data + result), live input echoes, a cancelled call, highlight tokens, and refuses a double free

## Expected Results

Indent 2; `validation.usage` / HTTP 400 (plan T-05).

## Actual Results

`t05_pretty_goldens_and_stream_refusal` passed: `--pretty`, `?pretty=true` and `to_json_pretty()` match the golden; `--pretty --stream` is `validation.usage` (exit 2) and SSE with `?pretty=true` is `validation.pretty_stream` (HTTP 400). The C demo prints a pretty envelope through the FFI `pretty` option (`conformance_ffi`). Linux: green in CI run 36505156729.

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

## Evidence

```text
conformance_envelope                 test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.58s
conformance_ffi                      test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.44s

GitHub Actions run 36505156729 (commit 14750b8, conclusion: success)
  test (ubuntu-latest)  success    test (macos-latest)  success
  features (none|serve|grpc|quic|oauth|cli)  success ×6    deny  success
```

## Evidence Sources

- The commands above, run at commit `14750b8`; full-suite output saved to `/tmp/p3-tests.log` (493 passed, 0 failed) and read with `grep -E "test result|FAILED|panicked"`.
- Test source: `tests/conformance_envelope.rs`, `tests/conformance_ffi.rs`.
- CI: [run 36505156729](https://github.com/olivierdevelops/rivet/actions/runs/36505156729) at the same commit (`test (ubuntu-latest)`, `test (macos-latest)`, the six `features` jobs and `deny`, all `success`).
- Release verification cross-reference: [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md).

## Executed By

Claude (automated run, PLAN-2026-0002 P3).

## Executed At

2026-09-29T14:23:39Z

## Defects Raised

None.

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-05
- [PROP-2026-0002](../proposals/approved/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md)
- [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md) · [Tests index](index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-29 | Claude | Created and executed at commit `14750b8` (PLAN-2026-0002 P3, TASK-061); result PASS. |
