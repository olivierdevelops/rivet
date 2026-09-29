---
document_id: TEST-2026-0021
title: "T-21 — security (UC-08 / R24)"
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
validated_plan_requirements: [PLAN-2026-0001 R24, PLAN-2026-0002 R1]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-21; re-recorded for v0.2.0 (PLAN-2026-0002 T-34).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001, PLAN-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-21 — security (UC-08 / R24)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0, 0.2.0 (re-recorded)
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

No file; `{"version":1}`; `--policy`; unknown key; deny > grants; `access` verbs; private ranges

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R24 | 020 |
| PLAN-2026-0002 R1 | T-34 regression: every 0.1.0 suite stays green on the 0.2.0 envelopes and input keys with no behaviour change |

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`, the v0.2.0 release candidate); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first (the FFI suites need `librivet`). Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

v0.1.0 run: commit `f15a82b`.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

v0.1.0 run: macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in `tests/conformance_policy_file.rs` and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
cargo test --workspace --all-targets --all-features --no-fail-fast   # full run → /tmp/p3-tests.log
cargo test --all-features --test conformance_policy_file
```

Tests executed (14):

- `conformance_policy_file::access_verbs_narrow_grants` — access verbs narrow a grant (stat-only read denies read/list); absent access allows every verb of the capability only
- `conformance_policy_file::deny_overrides_grants` — deny entries override grants, including access-narrowed deny entries (deny append on the audit log, update still allowed)
- `conformance_policy_file::discovery_beside_the_entry_file` — S129 policy.json is discovered beside the entry file (never the current directory); removing it denies the same call (exit 3) while pure operations run
- `conformance_policy_file::explicit_policy_path` — S69 --policy PATH uses only that file (targets resolve from its directory, deny wins); grant text or a missing file is policy.invalid exit 2
- `conformance_policy_file::invalid_policy_message_names_the_loaded_file` — a policy.invalid message names the file actually loaded (`--policy PATH`), not always policy.json
- `conformance_policy_file::per_request_restriction_cannot_widen` — per-request data cannot widen authority: policy/grant fields in /v1/request bodies and MCP arguments are ignored and the call stays denied
- `conformance_policy_file::policy_explain_flags_star` — S67 `policy explain` flags every "*" grant as broad (text and --json); narrow grants are not flagged
- `conformance_policy_file::policy_explain_params_evaluates_concrete_targets` — G9 `policy explain ID --params JSON` fills param-dependent targets with that call's params, prints per-site decisions and exits 3 when any concrete target would be denied (0 when all are allowed; no --params keeps exit 0)
- `conformance_policy_file::private_range_end_to_end` — S130 end to end: a loopback fixture is unreachable under "*" (zero requests reach it) and reachable when its origin is granted literally
- `conformance_policy_file::private_ranges_denied_unless_named_literally` — private ranges (RFC1918, 127/8, ::1, 169.254.169.254, fc00::/7, fe80::/10, IPv4-mapped IPv6, localhost) are denied under "*" and allowed only when named literally
- `conformance_policy_file::request_restriction_narrows_never_widens_library` — G31 library request option: `restrict {grants}` narrows one request (and its nested calls) to the intersection with policy.json, a restriction naming ungranted targets never widens, other requests are unaffected, and a malformed restriction is policy.invalid with a /restrict pointer
- `conformance_policy_file::request_restriction_on_http_mcp_ws` — G31 surfaces: `restrict` on POST /v1/request, MCP tools/call and a WebSocket request frame narrows that call only (403 / isError / error frame), and a widening restriction stays denied
- `conformance_policy_file::schema_errors_name_their_json_pointer` — every schema error class is policy.invalid (validation, exit 2) naming the JSON pointer of the offending value
- `conformance_policy_file::version_only_policy_is_empty` — `{"version":1}` is a present, empty policy: every effect is denied, pure operations run, private ranges stay denied

v0.1.0 run: `cargo test conformance_policy_file`.

## Expected Results

Deny-by-default; `policy.invalid` exit 2

## Actual Results

### v0.2.0 run (2026-09-29, commit `14750b8`)

14 passed, 0 failed, 0 ignored on macOS at commit `14750b8` (v0.2.0 release candidate). This is the PLAN-2026-0002 T-34 regression: the 0.1.0 suite passes on the 0.2.0 envelopes and input keys (TASK-018 changed only request and response shapes). Green on ubuntu-latest and macos-latest in CI run 36505156729, which runs the same `cargo test --workspace --all-targets --all-features --no-fail-fast`.

### v0.1.0 run (2026-09-28, commit `f15a82b`)

14 passed, 0 failed, 0 ignored.

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

v0.1.0 run: PASS.

## Evidence

v0.2.0 run:

```text
conformance_policy_file            test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s

GitHub Actions run 36505156729 (commit 14750b8, conclusion: success)
  test (ubuntu-latest)  success    test (macos-latest)  success
  features (none|serve|grpc|quic|oauth|cli)  success ×6    deny  success
```

v0.1.0 run:

```text
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.73s
```

## Evidence Sources

- The commands above, run at commit `14750b8`; full-suite output in `/tmp/p3-tests.log` (493 passed, 0 failed), read with `grep -E "test result|FAILED|panicked"`.
- Test source: `tests/conformance_policy_file.rs`.
- CI: [run 36505156729](https://github.com/olivierdevelops/rivet/actions/runs/36505156729) at the same commit (`test (ubuntu-latest)`, `test (macos-latest)`, six `features` jobs, `deny`: all `success`).

v0.1.0 run:

- Command above, run at commit `f15a82b`.
- Test source: `tests/conformance_policy_file.rs`.

## Executed By

Claude (automated run).

## Executed At

2026-09-29T14:23:39Z (v0.2.0 run; v0.1.0 run: 2026-09-28T10:44:58Z)

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-34
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-21
- [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f15a82b`. |
| 2 | 2026-09-29 | Claude | Re-recorded for v0.2.0 at commit `14750b8` (PLAN-2026-0002 T-34, TASK-061): PASS. v0.1.0 run kept as history. |
