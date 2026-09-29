---
document_id: TEST-2026-0049
title: "T-16 — File modules: import, namespaces and visibility (UC-10 / R19, R20)"
document_type: test
status: completed
created_date: 2026-09-29
last_updated: 2026-09-29
document_revision: 1
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [language, registry]
affected_versions:
  from: "0.2.0"
  to: null
validated_plan_requirements: [PLAN-2026-0002 R19, PLAN-2026-0002 R20]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and recorded result for PLAN-2026-0002 test T-16 (PROP-2026-0002 T-16).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates (PLAN-2026-0002 TASK-061).
related_documents: [PLAN-2026-0002, PROP-2026-0002, ADR-0004]
supersedes: null
superseded_by: null
tags: [rivet, test, v0.2.0]
---

# T-16 — File modules: import, namespaces and visibility (UC-10 / R19, R20)

> **Status:** Completed
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** language, registry

## Purpose

Show that imports compile each file once, namespace operations as `ALIAS.ID` (transitively), keep modules internal unless `public`, resolve `(alias.id …)` and `request` calls, and give each module its own globals and connectors.

```text
  app.rivet ── import "users.rivet" as users public ──► users.get, users.list   (listed)
      │     └─ import "lib/billing.rivet" as billing ──► billing.invoice          (internal)
      │                     └─ import "../users.rivet" as users ─► same file, compiled once
      └─ (users.get {id: 1}) · (request "billing.invoice" …)
```

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0002 R19 | `import "PATH" as ALIAS [public]`; root-confined bootstrap reads |
| PLAN-2026-0002 R20 | `ALIAS.ID` namespaces; internal by default, `public` to expose; transitive namespacing |

Plan row: [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md#test-and-validation-checklist) T-16 (integration, UC-10).

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first so `librivet` exists for the FFI suites (TRBL-2026-0006); Python 3 and a C compiler on `PATH`. Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

## Test Data

The fixtures, bundles and goldens defined in `tests/conformance_modules.rs` and the shared helpers under `tests/` (in-process servers, temporary directories, `tests/fixtures/`).

## Procedure

```sh
cargo test --all-features --test conformance_modules
```

Tests executed (4):

- `conformance_modules::t16_imports_compile_once_and_namespace` — each file compiles once (users.rivet imported by app and billing), namespaces are ALIAS.ID and transitive (billing.tax.rate), bootstrap files listed
- `conformance_modules::t16_module_calls_are_checked` — a module call that resolves nowhere and a call to another module's private operation are check.unknown_operation
- `conformance_modules::t16_module_connectors_stay_inside` — connectors stay inside their module: the importer cannot reference a module's connector, and a connector name declared in two files collides
- `conformance_modules::t16_visibility_calls_and_module_globals` — internal imports are callable but not listed; `public` (transitively) lists them; `(alias.id …)`, `(request "alias.id")`, own-ID calls and per-module globals all work

## Expected Results

As specified (plan T-16).

## Actual Results

The four T-16 tests passed: a file reached twice compiles once under the shallowest namespace; transitive `ALIAS.ID` namespaces; internal vs `public` visibility; module calls are checked; per-module globals; module-scoped connectors stay inside. Linux: green in CI run 36505156729.

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

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-16
- [PROP-2026-0002](../proposals/approved/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md)
- [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md) · [Tests index](index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-29 | Claude | Created and executed at commit `14750b8` (PLAN-2026-0002 P3, TASK-061); result PASS. |
