---
document_id: TEST-2026-0053
title: "T-20 — The loader's policy governs every module (UC-10, UC-11 / R24)"
document_type: test
status: completed
created_date: 2026-09-29
last_updated: 2026-09-29
document_revision: 1
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [policy, audit, registry]
affected_versions:
  from: "0.2.0"
  to: null
validated_plan_requirements: [PLAN-2026-0002 R24]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and recorded result for PLAN-2026-0002 test T-20 (PROP-2026-0002 T-20).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates (PLAN-2026-0002 TASK-061).
related_documents: [PLAN-2026-0002, PROP-2026-0002, ADR-0004]
supersedes: null
superseded_by: null
tags: [rivet, test, v0.2.0]
---

# T-20 — The loader's policy governs every module (UC-10, UC-11 / R24)

> **Status:** Completed
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** policy, audit, registry

## Purpose

Show that only the loader's policy decides for effects in every module, that a module's own `policy.json` is ignored with `warning[check.module_policy_ignored]`, and that `io`, `graph`, `policy explain` and `policy generate` cover modules with module spans and namespaced IDs.

```text
  ./policy.json (loader) ─────────────► decides for app.rivet, users.rivet, lib/billing.rivet
  lib/policy.json (module) ─► ignored ─► warning[check.module_policy_ignored] (check, request, serve)
  io · graph · policy explain · policy generate ─► lib/billing.rivet:3 · billing.invoice
```

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0002 R24 | One policy (the loader's); a module's `policy.json` ignored with a warning; manifest, graph, explain and generate cover modules |

Plan row: [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md#test-and-validation-checklist) T-20 (security, UC-10, UC-11).

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first so `librivet` exists for the FFI suites (TRBL-2026-0006); Python 3 and a C compiler on `PATH`. Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

## Test Data

The fixtures, bundles and goldens defined in `tests/conformance_modules.rs`, `tests/conformance_verification_defects.rs` and the shared helpers under `tests/` (in-process servers, temporary directories, `tests/fixtures/`).

## Procedure

```sh
cargo test --all-features --test conformance_modules
cargo test --all-features --test conformance_verification_defects
```

Tests executed (4):

- `conformance_modules::t20_loader_policy_only` — modules run under the loader's policy only: the module's own policy.json is ignored (warning check.module_policy_ignored at the import) and cannot widen access
- `conformance_modules::t20_manifest_graph_and_generate_cover_modules` — io, graph and policy generate cover every module with module spans (lib/data.rivet:3), namespaced IDs and one bootstrap row per file
- `conformance_verification_defects::request_prints_the_module_policy_warning` — INC-2026-0012 item 14: `rivet request` prints warning[check.module_policy_ignored] on stderr at load (once), like `rivet check`
- `conformance_verification_defects::policy_explain_data_denial_and_modules` — INC-2026-0012 item 12: policy explain takes --data (--params alias); a denied call with --json prints a status error / kind permission envelope and exits 3; params flow through a call into a module (report.remote → users.fetch is exact)

## Expected Results

Denials as by the loader; warning; spans (plan T-20).

## Actual Results

Both T-20 tests passed (loader policy only with the warning; `io`/`graph`/`explain`/`generate` over modules). `request_prints_the_module_policy_warning` (the warning on `rivet request`, not only `check`) and `policy_explain_data_denial_and_modules` (params reach module calls, so callee targets are filled) passed. The T-20 fixture's module `policy.json` would grant `./**`, while the loader's grants only `./data/**`; the fixture's `secret/key.json` stays unreadable, and its value never appears in the run output (RPT-2026-0015 canary scan). Linux: green in CI run 36505156729.

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

## Evidence

```text
conformance_modules                  test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.45s
conformance_verification_defects     test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.64s

GitHub Actions run 36505156729 (commit 14750b8, conclusion: success)
  test (ubuntu-latest)  success    test (macos-latest)  success
  features (none|serve|grpc|quic|oauth|cli)  success ×6    deny  success
```

## Evidence Sources

- The commands above, run at commit `14750b8`; full-suite output saved to `/tmp/p3-tests.log` (493 passed, 0 failed) and read with `grep -E "test result|FAILED|panicked"`.
- Test source: `tests/conformance_modules.rs`, `tests/conformance_verification_defects.rs`.
- CI: [run 36505156729](https://github.com/olivierdevelops/rivet/actions/runs/36505156729) at the same commit (`test (ubuntu-latest)`, `test (macos-latest)`, the six `features` jobs and `deny`, all `success`).
- Release verification cross-reference: [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md).

## Executed By

Claude (automated run, PLAN-2026-0002 P3).

## Executed At

2026-09-29T14:23:39Z

## Defects Raised

None in this run ([INC-2026-0012](../incidents/resolved/inc-2026-0012-documentation-and-demo-verification-defects.md) items 12 and 14 fixed before it).

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-20
- [PROP-2026-0002](../proposals/approved/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md)
- [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md) · [Tests index](index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-29 | Claude | Created and executed at commit `14750b8` (PLAN-2026-0002 P3, TASK-061); result PASS. |
