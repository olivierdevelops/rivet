---
document_id: TEST-2026-0008
title: "T-08 — security / compatibility (UC-08 / R11, R13)"
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
validated_plan_requirements: [PLAN-2026-0001 R11, PLAN-2026-0001 R13, PLAN-2026-0002 R1]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-08; re-recorded for v0.2.0 (PLAN-2026-0002 T-34).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001, PLAN-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-08 — security / compatibility (UC-08 / R11, R13)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0, 0.2.0 (re-recorded)
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

No policy.json; empty grants; hard links; junctions; case variants; DNS rebinding; process descendants

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R11 | 020, 021, 035 |
| PLAN-2026-0001 R13 | 021, 035, 037, 093 |
| PLAN-2026-0002 R1 | T-34 regression: every 0.1.0 suite stays green on the 0.2.0 envelopes and input keys with no behaviour change |

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`, the v0.2.0 release candidate); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first (the FFI suites need `librivet`). Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

v0.1.0 run: commit `f15a82b`.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

v0.1.0 run: macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in `tests/conformance_sandbox.rs` and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
cargo test --workspace --all-targets --all-features --no-fail-fast   # full run → /tmp/p3-tests.log
cargo test --all-features --test conformance_sandbox
```

Tests executed (9):

- `conformance_sandbox::bootstrap_list_is_reported` — `io --include-bootstrap` publishes the fixed runtime-internal list (bundle, policy.json, CA bundle, resolver, tzdata, descriptors, stdio) and it grants nothing
- `conformance_sandbox::case_variants_and_hard_links` — brokered file effects: case variants of a denied path are still denied on case-insensitive volumes, and a hard link inside a granted tree is refused (S139)
- `conformance_sandbox::child_cannot_reach_the_network` — a sandboxed child cannot reach the network, even an origin the script itself may connect to (zero connections accepted)
- `conformance_sandbox::child_cannot_spawn_descendants` — process descendants: a sandboxed child cannot fork a grandchild to escape (xargs → touch never runs)
- `conformance_sandbox::child_cannot_write_outside_granted_paths` — with a policy present a child writes inside granted paths only: writes to ./data, the bundle root and /tmp are refused by the OS
- `conformance_sandbox::no_policy_denies_exec` — S66 without policy.json exec is denied before spawning (the marker file is never created); pure operations still run
- `conformance_sandbox::secret_destination_binding` — S140 a secret bound with `for ORIGIN` reaches that origin only: another granted origin is denied before connecting, and the value never appears in results, errors or traces
- `conformance_sandbox::secret_taint_covers_every_sink` — G23b secret taint reaches every sink: assignment, interpolation, list/object construction and (chained) base64 encoding are tracked; http headers/body toward an unbound origin, file writes, process args, TCP/UDP sends and nested request params are denied (files/processes always), a bound TCP origin and the bound HTTP origin (even base64-encoded) are allowed, returning an encoded secret is an error, and the canary never reaches a sink, result, error or trace
- `conformance_sandbox::unrepresentable_policy_refuses_before_spawn` — policies the backend cannot represent exactly (narrowed access, globs) fail unsupported.sandbox_backend before spawning

v0.1.0 run: `cargo test conformance_sandbox per OS`.

## Expected Results

Zero prohibited brokered effects; unsupported backend refuses before spawn

## Actual Results

### v0.2.0 run (2026-09-29, commit `14750b8`)

9 passed, 0 failed, 0 ignored on macOS at commit `14750b8` (v0.2.0 release candidate). This is the PLAN-2026-0002 T-34 regression: the 0.1.0 suite passes on the 0.2.0 envelopes and input keys (TASK-018 changed only request and response shapes). Green on ubuntu-latest and macos-latest in CI run 36505156729, which runs the same `cargo test --workspace --all-targets --all-features --no-fail-fast`. **PARTIAL → PASS.** On macOS the Seatbelt backend runs: children cannot write outside granted paths, reach the network or fork descendants. On Linux the Landlock + seccomp backend is gated by design ([ADR-0003](../decisions/adr-0003-process-sandbox-backends.md): not yet certified on a kernel ≥ 6.12 runner), and the suite is platform-aware (`sandbox_certified()`): every process test asserts the typed refusal `unsupported.sandbox_backend` (kind unsupported, exit 5) **before spawn**, and that nothing happened (no file created, 0 connections accepted, no grandchild). CI run 36505156729 ran it green on ubuntu-latest. This satisfies the T-08 criterion as written in PLAN-2026-0001 — "Zero prohibited brokered effects; unsupported backend refuses before spawn", `cargo test conformance_sandbox` per OS — because the refusal before spawn *is* the specified outcome for an uncertified backend, and no prohibited effect occurs on either supported OS. The Linux sandbox being gated remains a product **limitation** (recorded in RPT-2026-0015 and the release notes), not a gap in this test. Windows is no longer a supported platform (INC-2026-0011), so it is not a reason for PARTIAL.

### v0.1.0 run (2026-09-28, commit `f15a82b`)

9 passed, 0 failed, 0 ignored on macOS. The plan requires Linux and Windows too; those runs are pending CI.

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

v0.1.0 run: PARTIAL.

## Evidence

v0.2.0 run:

```text
conformance_sandbox                test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s

GitHub Actions run 36505156729 (commit 14750b8, conclusion: success)
  test (ubuntu-latest)  success    test (macos-latest)  success
  features (none|serve|grpc|quic|oauth|cli)  success ×6    deny  success
```

v0.1.0 run:

```text
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
```

## Evidence Sources

- The commands above, run at commit `14750b8`; full-suite output in `/tmp/p3-tests.log` (493 passed, 0 failed), read with `grep -E "test result|FAILED|panicked"`.
- Test source: `tests/conformance_sandbox.rs`.
- CI: [run 36505156729](https://github.com/olivierdevelops/rivet/actions/runs/36505156729) at the same commit (`test (ubuntu-latest)`, `test (macos-latest)`, six `features` jobs, `deny`: all `success`).

v0.1.0 run:

- Command above, run at commit `f15a82b`.
- Test source: `tests/conformance_sandbox.rs`.

## Executed By

Claude (automated run).

## Executed At

2026-09-29T14:23:39Z (v0.2.0 run; v0.1.0 run: 2026-09-28T10:44:24Z)

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-34
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-08
- [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f15a82b`. |
| 2 | 2026-09-29 | Claude | Re-recorded for v0.2.0 at commit `14750b8` (PLAN-2026-0002 T-34, TASK-061): PASS; PARTIAL → PASS (macOS Seatbelt; Linux typed refusal before spawn per ADR-0003; Windows unsupported). v0.1.0 run kept as history. |
