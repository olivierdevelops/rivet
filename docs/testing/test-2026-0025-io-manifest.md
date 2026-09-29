---
document_id: TEST-2026-0025
title: "T-25 — integration (UC-21, UC-09 / R6, R26)"
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
validated_plan_requirements: [PLAN-2026-0001 R6, PLAN-2026-0001 R26, PLAN-2026-0002 R1]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-25; re-recorded for v0.2.0 (PLAN-2026-0002 T-34).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001, PLAN-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-25 — integration (UC-21, UC-09 / R6, R26)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0, 0.2.0 (re-recorded)
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

exact/bounded/param_dependent/dynamic/opaque sites of every kind; views; formats; `--check-policy` exit 3; `--strict` exit 7; `--trace` (+ option-derived sites if TASK-005 is approved)

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R6 | 037, 038 |
| PLAN-2026-0001 R26 | 038, 039 |
| PLAN-2026-0002 R1 | T-34 regression: every 0.1.0 suite stays green on the 0.2.0 envelopes and input keys with no behaviour change |

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`, the v0.2.0 release candidate); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first (the FFI suites need `librivet`). Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

v0.1.0 run: commit `f15a82b`.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

v0.1.0 run: macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in `tests/conformance_io_manifest.rs` and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
cargo test --workspace --all-targets --all-features --no-fail-fast   # full run → /tmp/p3-tests.log
cargo test --all-features --test conformance_io_manifest
```

Tests executed (16):

- `conformance_io_manifest::demo02_by_target_and_check_policy` — 02-file-crud `io --by target` rows (README table, one row per capability) and `io --check-policy` table exit 0
- `conformance_io_manifest::demo02_needs_and_access_filter` — 02-file-crud `io --needs` listing and `--access create,update,delete` keeps only mutating rows
- `conformance_io_manifest::demo03_http_rows` — 03-http origin-grouped `--by target` row and per-path `--check-policy` rows (query template included), exit 0
- `conformance_io_manifest::demo11_json_manifest` — 11-sandbox JSON manifest: effect ids, call chain through data.read, origin/phase/requires_existing, policy digest, complete=true and --strict exit 0
- `conformance_io_manifest::demo11_needs_and_check_files` — 11-sandbox --needs (callee via) and --check-files under policy.json (exit 3), for allowed ops (exit 0) and create-only.json (all not_permitted, exit 3)
- `conformance_io_manifest::demo11_views` — 11-sandbox check-policy (deny overrides grant, exit 3), by target, by capability and an empty delete filter
- `conformance_io_manifest::fixtures_exist`
- `conformance_io_manifest::s141_s143_by_target` — S141/S143 by-target rows: origin grouping, glob rows per capability, NEEDS FILE yes/no/`yes: update`, private helper labelled
- `conformance_io_manifest::s144_check_policy_partial_denied_unknown` — S144 check-policy against the fixture policy: partial for 2026-* globs, denied without allow_delete, unknown for dynamic, exit 3; S145 CSV row shape
- `conformance_io_manifest::s146_access_narrowing` — S146 create-only narrowing: stat allowed, update denied, exit 3
- `conformance_io_manifest::s153_trace_join` — S153 planned vs actual: a real request's broker decisions carry effect_ids that --trace joins into attempts; unknown request is not_found; the one-shot CLI trace store is empty (exit 4)
- `conformance_io_manifest::s154_s155_option_sites_and_needs` — S154/S155: tls options are before_connect file sites; key_file is secret; body file of a file created earlier is not a need
- `conformance_io_manifest::s156_s157_check_files` — S156/S157 --check-files: all present exit 0, missing key exit 4, read-only policy without stat = not_permitted exit 3
- `conformance_io_manifest::s62_json_sites` — S62 JSON: deterministic effect ids, param templates/globs, longest call chain, secrets by name, complete=false, policy null
- `conformance_io_manifest::s63_s64_entry_and_bootstrap` — S63/S64: one entry follows its literal call; --kind network with --include-bootstrap lists the fixed bootstrap list
- `conformance_io_manifest::s65_strict_incomplete_exits_7` — S65 every site of --all (13) with call row, dynamic config.url reported by expression, --strict exits 7 and plain run exits 0

v0.1.0 run: `cargo test conformance_io_manifest`.

## Expected Results

Manifest equals the golden JSON

## Actual Results

### v0.2.0 run (2026-09-29, commit `14750b8`)

16 passed, 0 failed, 0 ignored on macOS at commit `14750b8` (v0.2.0 release candidate). This is the PLAN-2026-0002 T-34 regression: the 0.1.0 suite passes on the 0.2.0 envelopes and input keys (TASK-018 changed only request and response shapes). Green on ubuntu-latest and macos-latest in CI run 36505156729, which runs the same `cargo test --workspace --all-targets --all-features --no-fail-fast`. The bootstrap rows now list the entry file and each module file (the `(+ imports)` placeholder of the INC-2026-0008 known issue is gone, TASK-104); the golden was updated for that row only.

### v0.1.0 run (2026-09-28, commit `f15a82b`)

16 passed, 0 failed, 0 ignored.

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

v0.1.0 run: PASS.

## Evidence

v0.2.0 run:

```text
conformance_io_manifest            test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.30s

GitHub Actions run 36505156729 (commit 14750b8, conclusion: success)
  test (ubuntu-latest)  success    test (macos-latest)  success
  features (none|serve|grpc|quic|oauth|cli)  success ×6    deny  success
```

v0.1.0 run:

```text
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.72s
```

## Evidence Sources

- The commands above, run at commit `14750b8`; full-suite output in `/tmp/p3-tests.log` (493 passed, 0 failed), read with `grep -E "test result|FAILED|panicked"`.
- Test source: `tests/conformance_io_manifest.rs`.
- CI: [run 36505156729](https://github.com/olivierdevelops/rivet/actions/runs/36505156729) at the same commit (`test (ubuntu-latest)`, `test (macos-latest)`, six `features` jobs, `deny`: all `success`).

v0.1.0 run:

- Command above, run at commit `f15a82b`.
- Test source: `tests/conformance_io_manifest.rs`.

## Executed By

Claude (automated run).

## Executed At

2026-09-29T14:23:39Z (v0.2.0 run; v0.1.0 run: 2026-09-28T10:45:07Z)

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-34
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-25
- [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f15a82b`. |
| 2 | 2026-09-29 | Claude | Re-recorded for v0.2.0 at commit `14750b8` (PLAN-2026-0002 T-34, TASK-061): PASS. v0.1.0 run kept as history. |
