---
document_id: TEST-2026-0043
title: "T-10 — Cargo feature matrix, unsupported.feature and packaging (UC-05 / R11, R12)"
document_type: test
status: completed
created_date: 2026-09-29
last_updated: 2026-09-29
document_revision: 1
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [library, cli, serve, grpc, quic, oauth_adapter]
affected_versions:
  from: "0.2.0"
  to: null
validated_plan_requirements: [PLAN-2026-0002 R11, PLAN-2026-0002 R12]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and recorded result for PLAN-2026-0002 test T-10 (PROP-2026-0002 T-10).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates (PLAN-2026-0002 TASK-061).
related_documents: [PLAN-2026-0002, PROP-2026-0002, ADR-0004]
supersedes: null
superseded_by: null
tags: [rivet, test, v0.2.0]
---

# T-10 — Cargo feature matrix, unsupported.feature and packaging (UC-05 / R11, R12)

> **Status:** Completed
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** library, cli, serve, grpc, quic, oauth_adapter

## Purpose

Show that `rivet-runtime` builds with no default features and with each single feature, that a compiled-out adapter is refused with `unsupported.feature`, that `cargo package --list` ships the embedded files, and that vhco accepts the workspace.

```text
  features:  none   cli   serve   grpc   quic   oauth   (default = serve,grpc,quic,oauth)
             build ✔ ✔ ✔ ✔ ✔ ✔  (0 warnings each, local + CI)
  bundle uses grpc/quic/oauth2 while compiled out ─► unsupported.feature  exit 5 · HTTP 501 · details.feature
```

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0002 R11 | Features `serve`, `grpc`, `quic`, `oauth`, `cli`; a compiled-out adapter is `unsupported.feature` |
| PLAN-2026-0002 R12 | Git dependency on tag `v0.2.0`; crates.io packaging prepared, publication gated by G-PUB |

Plan row: [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md#test-and-validation-checklist) T-10 (build, UC-05).

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first so `librivet` exists for the FFI suites (TRBL-2026-0006); Python 3 and a C compiler on `PATH`. Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

## Test Data

The fixtures, bundles and goldens defined in `tests/conformance_features.rs`, `tests/conformance_verification_defects.rs` and the shared helpers under `tests/` (in-process servers, temporary directories, `tests/fixtures/`).

## Procedure

```sh
for f in none cli serve grpc quic oauth; do cargo build -p rivet-runtime --no-default-features [--features $f] | tail -2; done
cargo test --all-features --test conformance_features
vhco validate . && vhco sync .
```

Tests executed (4):

- `conformance_features::capabilities_report_the_compiled_features` — rivet.capabilities reports the compiled Cargo features as build_features (== rivet::build_features()) and abi_version 1, after the 0.1.0 keys
- `conformance_features::compiled_out_adapters_are_refused_with_unsupported_feature` — a bundle using grpc, quic, HTTP/3 and oauth2 is refused with one unsupported.feature per use (exit 5, HTTP 501, details.feature, source span) when those features are compiled out, and accepted when they are in
- `conformance_features::package_list_ships_the_embedded_files` — R12 packaging: `cargo package --list` ships the library, the grammar and editors/keywords.json (embedded by highlight_source) and no tests or docs
- `conformance_verification_defects::compiled_out_grpc_points_at_the_call` — INC-2026-0012 item 17: without the grpc feature a gRPC call is reported at its call site (the `response = grpc …` line), not at the connector's endpoint line

## Expected Results

All green (plan T-10).

## Actual Results

Local: `--no-default-features` and each of `cli`, `serve`, `grpc`, `quic`, `oauth` built with 0 warnings. Under `--all-features`, `conformance_features` ran its 3 feature-independent tests (capabilities report `build_features` and `abi_version` 1; compiled-out adapters refused; `cargo package --list` ships `editors/keywords.json` and no tests/docs); the `without_*` tests are cfg-gated to the lean builds and run in the CI `features` jobs (all six green in run 36505156729). `compiled_out_grpc_points_at_the_call` passed. `vhco validate .` and `vhco sync .` are green on the workspace. R12's publication part is not tested here: the `v0.2.0` tag dependency exists only after P5, and crates.io publication is deferred (G-PUB).

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

## Evidence

```text
conformance_features                 test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s
conformance_verification_defects     test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.64s

GitHub Actions run 36505156729 (commit 14750b8, conclusion: success)
  test (ubuntu-latest)  success    test (macos-latest)  success
  features (none|serve|grpc|quic|oauth|cli)  success ×6    deny  success
```

```text
== none
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.90s
== cli
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 13.66s
== serve
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 17.63s
== grpc
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 17.38s
== quic
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 17.69s
== oauth
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 12.43s
warnings: none=0 cli=0 serve=0 grpc=0 quic=0 oauth=0
```

## Evidence Sources

- The commands above, run at commit `14750b8`; full-suite output saved to `/tmp/p3-tests.log` (493 passed, 0 failed) and read with `grep -E "test result|FAILED|panicked"`.
- Test source: `tests/conformance_features.rs`, `tests/conformance_verification_defects.rs`.
- CI: [run 36505156729](https://github.com/olivierdevelops/rivet/actions/runs/36505156729) at the same commit (`test (ubuntu-latest)`, `test (macos-latest)`, the six `features` jobs and `deny`, all `success`).
- Release verification cross-reference: [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md).

## Executed By

Claude (automated run, PLAN-2026-0002 P3).

## Executed At

2026-09-29T14:23:39Z

## Defects Raised

None in this run ([INC-2026-0012](../incidents/resolved/inc-2026-0012-documentation-and-demo-verification-defects.md) item 17 fixed before it).

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-10
- [PROP-2026-0002](../proposals/implemented/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md)
- [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md) · [Tests index](index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-29 | Claude | Created and executed at commit `14750b8` (PLAN-2026-0002 P3, TASK-061); result PASS. |
