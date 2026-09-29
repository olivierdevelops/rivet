---
document_id: TEST-2026-0048
title: "T-15 — Legacy {id, params} input with deprecation signals (UC-09 / R5)"
document_type: test
status: completed
created_date: 2026-09-29
last_updated: 2026-09-29
document_revision: 1
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [serve, cli, http, ws, mcp]
affected_versions:
  from: "0.2.0"
  to: null
validated_plan_requirements: [PLAN-2026-0002 R5]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and recorded result for PLAN-2026-0002 test T-15 (PROP-2026-0002 T-15).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates (PLAN-2026-0002 TASK-061).
related_documents: [PLAN-2026-0002, PROP-2026-0002, ADR-0004]
supersedes: null
superseded_by: null
tags: [rivet, test, v0.2.0]
---

# T-15 — Legacy {id, params} input with deprecation signals (UC-09 / R5)

> **Status:** Completed
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** serve, cli, http, ws, mcp

## Purpose

Show that 0.1.0 clients keep working in 0.2.x with a deprecation signal on every surface, and that mixing a key with its alias is refused.

```text
  {"id":…, "params":…} ─► accepted ─► HTTP Deprecation: true + access log deprecated=1
                                     CLI warning[deprecated.params] · WS trace note (phase input)
  {"operation":…, "id":…}  ─► validation.input_envelope  (HTTP 422 · exit 2)
```

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0002 R5 | `id`/`params` aliases accepted in 0.2.x with deprecation signals; mixed keys refused |

Plan row: [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md#test-and-validation-checklist) T-15 (compatibility, UC-09).

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

Tests executed (2):

- `conformance_envelope::t15_legacy_aliases_and_refusals` — T-15: 0.1.0 {id, params} still works in 0.2.x with deprecation signals (HTTP Deprecation: true + access log deprecated=1, CLI warning[deprecated.params]/[deprecated.input], a trace note) on HTTP, polling, WS, MCP, CLI and the library; mixing a key with its alias or sending `error` is validation.input_envelope (422, exit 2)
- `conformance_verification_defects::ws_legacy_frame_leaves_a_deprecation_note` — INC-2026-0012 (coordinator): a legacy WebSocket request frame ({id, params}) is accepted and leaves the deprecation trace note (phase input, decision deprecated) on its request, like HTTP and MCP

## Expected Results

Header/warning; 422 (plan T-15).

## Actual Results

`t15_legacy_aliases_and_refusals` passed: legacy input works with `Deprecation: true` and `deprecated=1` over HTTP, `warning[deprecated.params]` on the CLI, and mixed keys are `validation.input_envelope` (422 / exit 2). `ws_legacy_frame_leaves_a_deprecation_note` passed. Linux: green in CI run 36505156729.

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

None.

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-15
- [PROP-2026-0002](../proposals/implemented/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md)
- [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md) · [Tests index](index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-29 | Claude | Created and executed at commit `14750b8` (PLAN-2026-0002 P3, TASK-061); result PASS. |
