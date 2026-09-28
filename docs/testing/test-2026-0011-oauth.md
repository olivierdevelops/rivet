---
document_id: TEST-2026-0011
title: "T-11 — integration / security (UC-11 / R16)"
document_type: test
status: completed
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [execution]
affected_versions:
  from: "0.1.0"
  to: null
validated_plan_requirements: [PLAN-2026-0001 R16]
environment: "macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)"
executed_by: Claude (automated)
executed_at: 2026-09-28T10:44:35Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-11.
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-11 — integration / security (UC-11 / R16)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

Three flows; state/issuer mismatch; slow_down; device `pending`; rotation race; invalid_grant

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R16 | 041 |

## Preconditions

A clean checkout at commit `f15a82b`; Rust toolchain from `rust-toolchain.toml`; fixtures are started in-process on `127.0.0.1:0` by the tests (no external services).

## Test Environment

macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in `tests/conformance_oauth.rs` and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
cargo test conformance_oauth
```

Tests executed:

- `authorization_code_flow_end_to_end`
- `cancel_is_idempotent`
- `cli_auth_commands`
- `client_credentials_acquire_cache_and_no_leak`
- `concurrent_acquires_single_flight`
- `demo_07_flows`
- `device_denied_and_expired`
- `device_flow_pending_slow_down_and_carry_over`
- `missing_grants_and_secret_fail_before_effects`
- `origin_binding_and_redirect_strip`
- `profile_rules_at_load`
- `refresh_rotation_race_and_invalid_grant`
- `state_and_issuer_mismatch`
- `unauthorized_mutation_is_not_retried`
- `uncertain_refresh_is_not_replayed`
- `user_token_without_expiry_is_never_reused`

## Expected Results

Tokens never escape; one refresh per key

## Actual Results

16 passed, 0 failed, 0 ignored.

## Result

PASS

## Evidence

```text
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.12s
```

## Evidence Sources

- Command above, run at commit `f15a82b`.
- Test source: `tests/conformance_oauth.rs`.

## Executed By

Claude (automated run).

## Executed At

2026-09-28T10:44:35Z

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-11
- [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f15a82b`. |
