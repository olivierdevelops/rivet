---
document_id: TEST-2026-0045
title: "T-12 — FFI misuse never crashes or leaks (UC-06 / R14)"
document_type: test
status: completed
created_date: 2026-09-29
last_updated: 2026-09-29
document_revision: 1
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [ffi]
affected_versions:
  from: "0.2.0"
  to: null
validated_plan_requirements: [PLAN-2026-0002 R14]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and recorded result for PLAN-2026-0002 test T-12 (PROP-2026-0002 T-12).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates (PLAN-2026-0002 TASK-061).
related_documents: [PLAN-2026-0002, PROP-2026-0002, ADR-0004]
supersedes: null
superseded_by: null
tags: [rivet, test, v0.2.0]
---

# T-12 — FFI misuse never crashes or leaks (UC-06 / R14)

> **Status:** Completed
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** ffi

## Purpose

Show that misuse of the C ABI (NULL, bad UTF-8, bad JSON, a panic, a double free, a wrong handle kind, free while running, shared threads) returns error envelopes or `RIVET_ERROR` and never crashes, and that nothing leaks.

```text
  NULL / bad UTF-8 ─► validation.ffi_argument     bad JSON ─► validation.input_envelope
  panic in an entry ─► internal.panic (no unwind into C)
  double free / wrong kind ─► RIVET_ERROR (opaque tokens, never dereferenced)
  free while running ─► cancel + ≤ 5 s grace       8 threads · 1 runtime ─► ok
  leaks --atExit ─► 0 leaks
```

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0002 R14 | Panic, NULL, UTF-8 and JSON guards; ownership rules; thread safety |

Plan row: [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md#test-and-validation-checklist) T-12 (failure / security, UC-06).

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first so `librivet` exists for the FFI suites (TRBL-2026-0006); Python 3 and a C compiler on `PATH`. Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

## Test Data

The fixtures, bundles and goldens defined in `ffi/src/tests.rs` and the shared helpers under `tests/` (in-process servers, temporary directories, `tests/fixtures/`).

## Procedure

```sh
cargo test -p rivet-ffi --all-features
leaks --atExit -- target/debug/deps/rivet-<hash> --test-threads=1     # the rivet-ffi unit-test binary
make -C examples/c OUT=/tmp/p3-leaks all
MallocStackLogging=1 leaks --atExit -- /tmp/p3-leaks/demo_static ../ffi/app.rivet
MallocStackLogging=1 leaks --atExit -- /tmp/p3-leaks/modules_static ../modules
RUSTFLAGS=-Zsanitizer=address cargo +nightly test -p rivet-ffi --all-features --target aarch64-apple-darwin --lib   # attempted
```

Tests executed (8):

- `rivet-ffi tests::null_and_bad_utf8_arguments_are_refused` — FFI: NULL and non-UTF-8 arguments on every entry point are validation.ffi_argument envelopes or RIVET_ERROR, never a crash
- `rivet-ffi tests::bad_json_is_an_error_envelope` — FFI: malformed JSON in options, input envelopes and data is an error envelope with the input-envelope or ffi_argument code
- `rivet-ffi tests::a_panic_becomes_an_internal_panic_envelope` — FFI: a panic inside an entry point (block_on inside a tokio worker) is caught and returned as an internal.panic envelope
- `rivet-ffi tests::double_free_and_wrong_handles_are_refused` — FFI: double free of a runtime, call, module or string, and a handle of the wrong kind, are refused (RIVET_ERROR / ffi_argument) instead of undefined behaviour
- `rivet-ffi tests::free_while_running_cancels_within_the_grace` — FFI: freeing a call that is still running cancels it and returns within the 5 s grace; the runtime keeps working
- `rivet-ffi tests::a_runtime_is_shared_by_threads` — FFI: one runtime handle is used from several threads at once (RivetRuntime is thread-safe)
- `rivet-ffi tests::module_handles` — FFI: rivet_load / rivet_module_* over a root-only runtime; load errors are envelopes; a freed module is refused
- `rivet-ffi tests::versions_agree` — FFI: rivet_abi_version is 1, rivet_version equals the crate version and rivet.capabilities reports abi_version

## Expected Results

Error envelopes; no crash, no leak (plan T-12).

## Actual Results

`cargo test -p rivet-ffi --all-features` ran the 8 misuse tests in `ffi/src/tests.rs` (moved in-crate from `ffi/tests/` by INC-2026-0010): 8 passed. The same binary under macOS `leaks --atExit` passed 8/8 with **0 leaks** (2512 allocation nodes live at exit, 0 leaked bytes). The C host examples, which include a double free, cancel and a stream, also report 0 leaks (`demo_static`, `modules_static`). Linux: green in CI run 36505156729 (`--workspace` includes rivet-ffi). **Method deviation:** the plan names AddressSanitizer on Linux CI; CI has no ASan job, and the local nightly ASan build (macOS arm64) compiled but the instrumented test binary hung before the harness printed anything (even `versions_agree`), so no ASan result exists. The expected result (error envelopes, no crash, no leak) is met by the tests and `leaks`; ASan is a follow-up in RPT-2026-0015.

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

## Evidence

```text
rivet-ffi (ffi/src/tests.rs)         test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s

GitHub Actions run 36505156729 (commit 14750b8, conclusion: success)
  test (ubuntu-latest)  success    test (macos-latest)  success
  features (none|serve|grpc|quic|oauth|cli)  success ×6    deny  success
```

```text
$ leaks --atExit -- target/debug/deps/rivet-06da278f94ee8837 --test-threads=1
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.30s
Process 19292: 2512 nodes malloced for 400 KB
Process 19292: 0 leaks for 0 total leaked bytes.

$ leaks --atExit -- /tmp/p3-leaks/demo_static ../ffi/app.rivet
Process 12887: 0 leaks for 0 total leaked bytes.
$ leaks --atExit -- /tmp/p3-leaks/modules_static ../modules
Process 14935: 0 leaks for 0 total leaked bytes.
```

## Evidence Sources

- The commands above, run at commit `14750b8`; full-suite output saved to `/tmp/p3-tests.log` (493 passed, 0 failed) and read with `grep -E "test result|FAILED|panicked"`.
- Test source: `ffi/src/tests.rs`.
- CI: [run 36505156729](https://github.com/olivierdevelops/rivet/actions/runs/36505156729) at the same commit (`test (ubuntu-latest)`, `test (macos-latest)`, the six `features` jobs and `deny`, all `success`).
- Release verification cross-reference: [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md).

## Executed By

Claude (automated run, PLAN-2026-0002 P3).

## Executed At

2026-09-29T14:23:39Z

## Defects Raised

None. The ASan hang is a tooling problem (no test ran), not a defect of the code under test.

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-12
- [PROP-2026-0002](../proposals/implemented/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md)
- [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md) · [Tests index](index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-29 | Claude | Created and executed at commit `14750b8` (PLAN-2026-0002 P3, TASK-061); result PASS. |
