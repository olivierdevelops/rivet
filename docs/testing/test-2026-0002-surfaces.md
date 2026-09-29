---
document_id: TEST-2026-0002
title: "T-02 — integration / e2e (UC-02 / R8, R9)"
document_type: test
status: completed
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 2
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [execution]
affected_versions:
  from: "0.1.0"
  to: null
validated_plan_requirements: [PLAN-2026-0001 R8, PLAN-2026-0001 R9, PLAN-2026-0002 R1]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-02; re-recorded for v0.2.0 (PLAN-2026-0002 T-34).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001, PLAN-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-02 — integration / e2e (UC-02 / R8, R9)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0, 0.2.0 (re-recorded)
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

Same fixture registry over CLI/HTTP/MCP/library; auth filtering; stdout clean

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R8 | 022, 024, 025, 028, 032 |
| PLAN-2026-0001 R9 | 022, 025 |
| PLAN-2026-0002 R1 | T-34 regression: every 0.1.0 suite stays green on the 0.2.0 envelopes and input keys with no behaviour change |

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`, the v0.2.0 release candidate); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first (the FFI suites need `librivet`). Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

v0.1.0 run: commit `f15a82b`.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

v0.1.0 run: macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in `tests/conformance_surfaces.rs` and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
cargo test --workspace --all-targets --all-features --no-fail-fast   # full run → /tmp/p3-tests.log
cargo test --all-features --test conformance_surfaces
```

Tests executed (3):

- `conformance_surfaces::local_and_remote_cli_agree` — the CLI run locally (--file) and against a running serve (--endpoint) prints the same Completion, NDJSON stream, validation ErrorEnvelope, tables and JSON for request/list/describe/outputs/io/trace, with identical exit codes
- `conformance_surfaces::oauth_begin_complete_across_invocations` — OAuth begin and complete run as two separate CLI invocations against one running host (its memory transaction store), then a protected request uses the connected account; a standalone local CLI cannot complete that transaction; no token or code is printed
- `conformance_surfaces::token_file_and_sensitive_builtins` — --token-file sends the bearer (never on argv or env); without it the server answers 401 (exit 3); a network principal may not read traces or the I/O manifest unless listed explicitly (exit 3)

v0.1.0 run: `cargo test conformance_surfaces`.

## Expected Results

Identical params/result/error

## Actual Results

### v0.2.0 run (2026-09-29, commit `14750b8`)

3 passed, 0 failed, 0 ignored on macOS at commit `14750b8` (v0.2.0 release candidate). This is the PLAN-2026-0002 T-34 regression: the 0.1.0 suite passes on the 0.2.0 envelopes and input keys (TASK-018 changed only request and response shapes). Green on ubuntu-latest and macos-latest in CI run 36505156729, which runs the same `cargo test --workspace --all-targets --all-features --no-fail-fast`.

### v0.1.0 run (2026-09-28, commit `f15a82b`)

3 passed, 0 failed, 0 ignored.

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

v0.1.0 run: PASS.

## Evidence

v0.2.0 run:

```text
conformance_surfaces               test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.27s

GitHub Actions run 36505156729 (commit 14750b8, conclusion: success)
  test (ubuntu-latest)  success    test (macos-latest)  success
  features (none|serve|grpc|quic|oauth|cli)  success ×6    deny  success
```

v0.1.0 run:

```text
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.48s
```

## Evidence Sources

- The commands above, run at commit `14750b8`; full-suite output in `/tmp/p3-tests.log` (493 passed, 0 failed), read with `grep -E "test result|FAILED|panicked"`.
- Test source: `tests/conformance_surfaces.rs`.
- CI: [run 36505156729](https://github.com/olivierdevelops/rivet/actions/runs/36505156729) at the same commit (`test (ubuntu-latest)`, `test (macos-latest)`, six `features` jobs, `deny`: all `success`).

v0.1.0 run:

- Command above, run at commit `f15a82b`.
- Test source: `tests/conformance_surfaces.rs`.

## Executed By

Claude (automated run).

## Executed At

2026-09-29T14:23:39Z (v0.2.0 run; v0.1.0 run: 2026-09-28T10:44:17Z)

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-34
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-02
- [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f15a82b`. |
| 2 | 2026-09-29 | Claude | Re-recorded for v0.2.0 at commit `14750b8` (PLAN-2026-0002 T-34, TASK-061): PASS. v0.1.0 run kept as history. |
