---
document_id: TEST-2026-0011
title: "T-11 — integration / security (UC-11 / R16)"
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
validated_plan_requirements: [PLAN-2026-0001 R16, PLAN-2026-0002 R1]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-11; re-recorded for v0.2.0 (PLAN-2026-0002 T-34).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001, PLAN-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-11 — integration / security (UC-11 / R16)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0, 0.2.0 (re-recorded)
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

Three flows; state/issuer mismatch; slow_down; device `pending`; rotation race; invalid_grant

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R16 | 041 |
| PLAN-2026-0002 R1 | T-34 regression: every 0.1.0 suite stays green on the 0.2.0 envelopes and input keys with no behaviour change |

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`, the v0.2.0 release candidate); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first (the FFI suites need `librivet`). Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

v0.1.0 run: commit `f15a82b`.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

v0.1.0 run: macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in `tests/conformance_oauth.rs` and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
cargo test --workspace --all-targets --all-features --no-fail-fast   # full run → /tmp/p3-tests.log
cargo test --all-features --test conformance_oauth
```

Tests executed (16):

- `conformance_oauth::authorization_code_flow_end_to_end` — authorization_code + PKCE S256: begin returns only a challenge, complete checks state/issuer and returns connected status; the account is then usable; replay fails
- `conformance_oauth::cancel_is_idempotent` — cancel is idempotent, a cancelled transaction cannot complete, and a completed one reports connected
- `conformance_oauth::cli_auth_commands` — the CLI `rivet auth begin|status|cancel|complete` maps to rivet.auth.* and prints no secrets
- `conformance_oauth::client_credentials_acquire_cache_and_no_leak` — client_credentials: first use exchanges (Basic client auth from allow_env), later uses hit the cache, the token reaches only the bound origin and never a result or trace
- `conformance_oauth::concurrent_acquires_single_flight` — two concurrent first uses cause exactly one token request (single flight per credential identity)
- `conformance_oauth::demo_07_flows` — docs/demos/07-oauth2 runs against the fake provider: contacts.list returns the payload twice with one token exchange, and removing the auth-origin grant is permission.denied
- `conformance_oauth::device_denied_and_expired` — device denial is auth.access_denied and an expired challenge is auth.transaction_expired
- `conformance_oauth::device_flow_pending_slow_down_and_carry_over` — device flow: pending then slow_down; the deadline returns {"state":"pending"} without consuming the transaction; the next complete keeps the increased interval and connects
- `conformance_oauth::missing_grants_and_secret_fail_before_effects` — S93: without the token-endpoint network grant the use is denied before contacting the provider and no resource call follows; a missing secret env is auth.client_secret_missing (exit 5)
- `conformance_oauth::origin_binding_and_redirect_strip` — a Bearer is attached only to resource_origins: another origin is auth.origin_not_bound, and a redirect to another origin strips it
- `conformance_oauth::profile_rules_at_load` — profile rules are enforced at load: code flow without pkce s256, password flow and missing store are refused
- `conformance_oauth::refresh_rotation_race_and_invalid_grant` — an expired user token refreshes once for two concurrent uses (rotation race), rotation is committed, invalid_grant becomes auth.login_required
- `conformance_oauth::state_and_issuer_mismatch` — state and RFC 9207 issuer mismatches are auth.callback_invalid before any exchange and consume the transaction; foreign principals see not_found
- `conformance_oauth::unauthorized_mutation_is_not_retried` — a 401 on a mutation is not retried; a replay-safe GET invalidates the lease and retries once with a fresh token
- `conformance_oauth::uncertain_refresh_is_not_replayed` — a refresh whose answer never arrives is auth.refresh_uncertain and the old refresh token is never replayed
- `conformance_oauth::user_token_without_expiry_is_never_reused` — G20: a user-flow access token without expiry is never reused: every use refreshes (the account stays connected), and without a refresh token it is login_required

v0.1.0 run: `cargo test conformance_oauth`.

## Expected Results

Tokens never escape; one refresh per key

## Actual Results

### v0.2.0 run (2026-09-29, commit `14750b8`)

16 passed, 0 failed, 0 ignored on macOS at commit `14750b8` (v0.2.0 release candidate). This is the PLAN-2026-0002 T-34 regression: the 0.1.0 suite passes on the 0.2.0 envelopes and input keys (TASK-018 changed only request and response shapes). Green on ubuntu-latest and macos-latest in CI run 36505156729, which runs the same `cargo test --workspace --all-targets --all-features --no-fail-fast`.

### v0.1.0 run (2026-09-28, commit `f15a82b`)

16 passed, 0 failed, 0 ignored.

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

v0.1.0 run: PASS.

## Evidence

v0.2.0 run:

```text
conformance_oauth                  test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.16s

GitHub Actions run 36505156729 (commit 14750b8, conclusion: success)
  test (ubuntu-latest)  success    test (macos-latest)  success
  features (none|serve|grpc|quic|oauth|cli)  success ×6    deny  success
```

v0.1.0 run:

```text
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.12s
```

## Evidence Sources

- The commands above, run at commit `14750b8`; full-suite output in `/tmp/p3-tests.log` (493 passed, 0 failed), read with `grep -E "test result|FAILED|panicked"`.
- Test source: `tests/conformance_oauth.rs`.
- CI: [run 36505156729](https://github.com/olivierdevelops/rivet/actions/runs/36505156729) at the same commit (`test (ubuntu-latest)`, `test (macos-latest)`, six `features` jobs, `deny`: all `success`).

v0.1.0 run:

- Command above, run at commit `f15a82b`.
- Test source: `tests/conformance_oauth.rs`.

## Executed By

Claude (automated run).

## Executed At

2026-09-29T14:23:39Z (v0.2.0 run; v0.1.0 run: 2026-09-28T10:44:35Z)

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-34
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-11
- [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f15a82b`. |
| 2 | 2026-09-29 | Claude | Re-recorded for v0.2.0 at commit `14750b8` (PLAN-2026-0002 T-34, TASK-061): PASS. v0.1.0 run kept as history. |
