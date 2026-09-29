---
document_id: TEST-2026-0015
title: "T-15 — cross-surface / security (UC-11–14 / R15–R18)"
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
validated_plan_requirements: [PLAN-2026-0001 R15, PLAN-2026-0001 R18, PLAN-2026-0002 R1]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-15; re-recorded for v0.2.0 (PLAN-2026-0002 T-34).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001, PLAN-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-15 — cross-surface / security (UC-11–14 / R15–R18)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0, 0.2.0 (re-recorded)
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

Same fixtures over all surfaces; absent/empty/restricted policy.json

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R15 | 042 |
| PLAN-2026-0001 R18 | 044 |
| PLAN-2026-0002 R1 | T-34 regression: every 0.1.0 suite stays green on the 0.2.0 envelopes and input keys with no behaviour change |

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`, the v0.2.0 release candidate); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first (the FFI suites need `librivet`). Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

v0.1.0 run: commit `f15a82b`.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

v0.1.0 run: macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in `tests/conformance_auth_transport_policy.rs` and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
cargo test --workspace --all-targets --all-features --no-fail-fast   # full run → /tmp/p3-tests.log
cargo test --all-features --test conformance_auth_transport_policy
```

Tests executed (4):

- `conformance_auth_transport_policy::absent_policy_same_on_every_surface` — absent policy.json: every surface runs the pure operation and denies HTTP, UDP and OAuth-attached calls with the same permission.denied, contacting nothing
- `conformance_auth_transport_policy::empty_policy_same_on_every_surface` — `{"version":1}` behaves like an absent file on every surface: pure runs, every effect is permission.denied
- `conformance_auth_transport_policy::granted_udp_same_on_every_surface` — granting the UDP peer literally makes the UDP fixture succeed with the same value on every surface
- `conformance_auth_transport_policy::restricted_policy_same_on_every_surface` — restricted policy: granted HTTP and OAuth-attached calls succeed identically everywhere, the ungranted UDP peer is denied, and the bearer never goes to an origin outside resource_origins

v0.1.0 run: `cargo test conformance_auth_transport_policy`.

## Expected Results

Same values/errors

## Actual Results

### v0.2.0 run (2026-09-29, commit `14750b8`)

4 passed, 0 failed, 0 ignored on macOS at commit `14750b8` (v0.2.0 release candidate). This is the PLAN-2026-0002 T-34 regression: the 0.1.0 suite passes on the 0.2.0 envelopes and input keys (TASK-018 changed only request and response shapes). Green on ubuntu-latest and macos-latest in CI run 36505156729, which runs the same `cargo test --workspace --all-targets --all-features --no-fail-fast`.

### v0.1.0 run (2026-09-28, commit `f15a82b`)

4 passed, 0 failed, 0 ignored.

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

v0.1.0 run: PASS.

## Evidence

v0.2.0 run:

```text
conformance_auth_transport_policy  test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.34s

GitHub Actions run 36505156729 (commit 14750b8, conclusion: success)
  test (ubuntu-latest)  success    test (macos-latest)  success
  features (none|serve|grpc|quic|oauth|cli)  success ×6    deny  success
```

v0.1.0 run:

```text
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.39s
```

## Evidence Sources

- The commands above, run at commit `14750b8`; full-suite output in `/tmp/p3-tests.log` (493 passed, 0 failed), read with `grep -E "test result|FAILED|panicked"`.
- Test source: `tests/conformance_auth_transport_policy.rs`.
- CI: [run 36505156729](https://github.com/olivierdevelops/rivet/actions/runs/36505156729) at the same commit (`test (ubuntu-latest)`, `test (macos-latest)`, six `features` jobs, `deny`: all `success`).

v0.1.0 run:

- Command above, run at commit `f15a82b`.
- Test source: `tests/conformance_auth_transport_policy.rs`.

## Executed By

Claude (automated run).

## Executed At

2026-09-29T14:23:39Z (v0.2.0 run; v0.1.0 run: 2026-09-28T10:44:44Z)

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-34
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-15
- [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f15a82b`. |
| 2 | 2026-09-29 | Claude | Re-recorded for v0.2.0 at commit `14750b8` (PLAN-2026-0002 T-34, TASK-061): PASS. v0.1.0 run kept as history. |
