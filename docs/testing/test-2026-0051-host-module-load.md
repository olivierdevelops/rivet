---
document_id: TEST-2026-0051
title: "T-18 — Host module objects: Runtime::load and Module (UC-11 / R22)"
document_type: test
status: completed
created_date: 2026-09-29
last_updated: 2026-09-29
document_revision: 1
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [library, registry]
affected_versions:
  from: "0.2.0"
  to: null
validated_plan_requirements: [PLAN-2026-0002 R22]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and recorded result for PLAN-2026-0002 test T-18 (PROP-2026-0002 T-18).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates (PLAN-2026-0002 TASK-061).
related_documents: [PLAN-2026-0002, PROP-2026-0002, ADR-0004]
supersedes: null
superseded_by: null
tags: [rivet, test, v0.2.0]
---

# T-18 — Host module objects: Runtime::load and Module (UC-11 / R22)

> **Status:** Completed
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** library, registry

## Purpose

Show that a root-only runtime loads modules as `Module` objects (`operations`, `describe`, `outputs`, `call`, `stream`, `duplex`), and that loads during running requests are safe (requests keep their snapshot).

```text
  Runtime::builder().root(dir) ─► rt.load("users.rivet") ─► Module{alias: users}
                                  rt.load_as("lib/billing.rivet", "billing")
     24 running requests ──── snapshot A ────────────► finish on A
     6 concurrent loads  ──── Arc swap ─► snapshot B ─► new requests
```

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0002 R22 | `Runtime::load/load_as` → `Module`; builder without an entry file; catalog snapshot swap |

Plan row: [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md#test-and-validation-checklist) T-18 (integration, UC-11).

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first so `librivet` exists for the FFI suites (TRBL-2026-0006); Python 3 and a C compiler on `PATH`. Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

## Test Data

The fixtures, bundles and goldens defined in `tests/conformance_modules.rs` and the shared helpers under `tests/` (in-process servers, temporary directories, `tests/fixtures/`).

## Procedure

```sh
cargo test --all-features --test conformance_modules
cargo run --release --example modules      # DEMO-2026-0019 step 7
```

Tests executed (3):

- `conformance_modules::t18_concurrent_loads_during_requests` — loads run while requests are in flight: running requests finish on their snapshot, new ones see every loaded module
- `conformance_modules::t18_load_and_call_modules` — a runtime without an entry file loads two modules; Module operations/describe/outputs/call use short IDs and answer envelopes named ALIAS.ID
- `conformance_modules::t18_module_streams` — a module operation streams items through a scope (Module::stream)

## Expected Results

Envelopes; no race (plan T-18).

## Actual Results

The three T-18 tests passed: root-only runtime, `load`/`load_as`, the `Module` API, module streams (scope-owned, see the plan's `Module::stream` deviation), and 6 concurrent loads during 24 running requests with the in-flight snapshot kept. Linux: green in CI run 36505156729.

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

## Evidence

```text
conformance_modules                  test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.45s

GitHub Actions run 36505156729 (commit 14750b8, conclusion: success)
  test (ubuntu-latest)  success    test (macos-latest)  success
  features (none|serve|grpc|quic|oauth|cli)  success ×6    deny  success
```

## Evidence Sources

- The commands above, run at commit `14750b8`; full-suite output saved to `/tmp/p3-tests.log` (493 passed, 0 failed) and read with `grep -E "test result|FAILED|panicked"`.
- Test source: `tests/conformance_modules.rs`.
- CI: [run 36505156729](https://github.com/olivierdevelops/rivet/actions/runs/36505156729) at the same commit (`test (ubuntu-latest)`, `test (macos-latest)`, the six `features` jobs and `deny`, all `success`).
- Release verification cross-reference: [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md).

## Executed By

Claude (automated run, PLAN-2026-0002 P3).

## Executed At

2026-09-29T14:23:39Z

## Defects Raised

None.

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-18
- [PROP-2026-0002](../proposals/approved/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md)
- [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md) · [Tests index](index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-29 | Claude | Created and executed at commit `14750b8` (PLAN-2026-0002 P3, TASK-061); result PASS. |
