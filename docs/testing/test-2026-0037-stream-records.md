---
document_id: TEST-2026-0037
title: "T-04 — Stream records then exactly one result (UC-02 / R1)"
document_type: test
status: completed
created_date: 2026-09-29
last_updated: 2026-09-29
document_revision: 1
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [serve, cli, http, ws, poll, library]
affected_versions:
  from: "0.2.0"
  to: null
validated_plan_requirements: [PLAN-2026-0002 R1]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and recorded result for PLAN-2026-0002 test T-04 (PROP-2026-0002 T-04).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates (PLAN-2026-0002 TASK-061).
related_documents: [PLAN-2026-0002, PROP-2026-0002, ADR-0004]
supersedes: null
superseded_by: null
tags: [rivet, test, v0.2.0]
---

# T-04 — Stream records then exactly one result (UC-02 / R1)

> **Status:** Completed
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** serve, cli, http, ws, poll, library

## Purpose

Show that a stream is a sequence of `type: data` records (`seq` 1…n) followed by exactly one terminal record (`seq` n+1, `data_count` n), and that a consumer stop yields `cancelled`.

```text
  data seq=1 ─► data seq=2 ─► … ─► data seq=n ─► result seq=n+1 status=ok|error|cancelled data_count=n
                                                      ▲
                                  consumer stop ──────┘ (status: cancelled)
```

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0002 R1 | `ResponseEnvelope` records on every streaming surface: `type: data` records numbered by `seq`, then one terminal `type: result` record |

Plan row: [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md#test-and-validation-checklist) T-04 (integration, UC-02).

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first so `librivet` exists for the FFI suites (TRBL-2026-0006); Python 3 and a C compiler on `PATH`. Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

## Test Data

The fixtures, bundles and goldens defined in `tests/conformance_envelope.rs`, `tests/conformance_verification_defects.rs` and the shared helpers under `tests/` (in-process servers, temporary directories, `tests/fixtures/`).

## Procedure

```sh
cargo test --all-features --test conformance_envelope
cargo test --all-features --test conformance_verification_defects
```

Tests executed (4):

- `conformance_envelope::t04_stream_records_then_one_result` — T-04: streams are type data records (seq 1..n) followed by exactly one type result record carrying seq n+1 and data_count n on NDJSON, SSE, WS and the library; a consumer stop ends status cancelled
- `conformance_verification_defects::library_terminal_records_carry_seq` — INC-2026-0012 item 3: a library stream's terminal Envelope::Result record carries seq = data_count + 1, like the CLI, SSE, WS, polling and C records
- `conformance_verification_defects::ws_refused_input_terminal_is_numbered` — INC-2026-0012 item 2: a ref ended by a refused input (seq 3 after seq 1) gets a terminal record with the next seq (2) and the real data_count (1), as stream-record.schema.json requires
- `conformance_verification_defects::ws_duplicate_ref_refusal_is_detached` — INC-2026-0012 item 16: a request frame reusing an in-flight ref is refused with ref "" and details.ref (never a type:"result" record for that ref); the in-flight ref still ends with its own single terminal record

## Expected Results

Sequence and statuses as specified (plan T-04).

## Actual Results

`t04_stream_records_then_one_result` passed on NDJSON, SSE, WebSocket and polling, including the cancelled terminal on a consumer stop. The INC-2026-0012 regressions passed: library terminal records carry `seq`, a WS ref ended by a refused input gets the next `seq` and the real `data_count`, and a duplicate-ref refusal is detached (`ref: ""`). Linux: green in CI run 36505156729.

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

## Evidence

```text
conformance_envelope                 test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.58s
conformance_verification_defects     test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.64s

GitHub Actions run 36505156729 (commit 14750b8, conclusion: success)
  test (ubuntu-latest)  success    test (macos-latest)  success
  features (none|serve|grpc|quic|oauth|cli)  success ×6    deny  success
```

## Evidence Sources

- The commands above, run at commit `14750b8`; full-suite output saved to `/tmp/p3-tests.log` (493 passed, 0 failed) and read with `grep -E "test result|FAILED|panicked"`.
- Test source: `tests/conformance_envelope.rs`, `tests/conformance_verification_defects.rs`.
- CI: [run 36505156729](https://github.com/olivierdevelops/rivet/actions/runs/36505156729) at the same commit (`test (ubuntu-latest)`, `test (macos-latest)`, the six `features` jobs and `deny`, all `success`).
- Release verification cross-reference: [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md).

## Executed By

Claude (automated run, PLAN-2026-0002 P3).

## Executed At

2026-09-29T14:23:39Z

## Defects Raised

None in this run ([INC-2026-0012](../incidents/resolved/inc-2026-0012-documentation-and-demo-verification-defects.md) items 2, 3, 16 fixed before it).

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-04
- [PROP-2026-0002](../proposals/implemented/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md)
- [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md) · [Tests index](index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-29 | Claude | Created and executed at commit `14750b8` (PLAN-2026-0002 P3, TASK-061); result PASS. |
