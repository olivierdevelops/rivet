---
document_id: TEST-2026-0021
title: "T-21 — security (UC-08 / R24)"
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
validated_plan_requirements: [PLAN-2026-0001 R24]
environment: "macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)"
executed_by: Claude (automated)
executed_at: 2026-09-28T10:44:58Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-21.
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-21 — security (UC-08 / R24)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

No file; `{"version":1}`; `--policy`; unknown key; deny > grants; `access` verbs; private ranges

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R24 | 020 |

## Preconditions

A clean checkout at commit `f15a82b`; Rust toolchain from `rust-toolchain.toml`; fixtures are started in-process on `127.0.0.1:0` by the tests (no external services).

## Test Environment

macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in `tests/conformance_policy_file.rs` and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
cargo test conformance_policy_file
```

Tests executed:

- `access_verbs_narrow_grants`
- `deny_overrides_grants`
- `discovery_beside_the_entry_file`
- `explicit_policy_path`
- `invalid_policy_message_names_the_loaded_file`
- `per_request_restriction_cannot_widen`
- `policy_explain_flags_star`
- `policy_explain_params_evaluates_concrete_targets`
- `private_range_end_to_end`
- `private_ranges_denied_unless_named_literally`
- `request_restriction_narrows_never_widens_library`
- `request_restriction_on_http_mcp_ws`
- `schema_errors_name_their_json_pointer`
- `version_only_policy_is_empty`

## Expected Results

Deny-by-default; `policy.invalid` exit 2

## Actual Results

14 passed, 0 failed, 0 ignored.

## Result

PASS

## Evidence

```text
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.73s
```

## Evidence Sources

- Command above, run at commit `f15a82b`.
- Test source: `tests/conformance_policy_file.rs`.

## Executed By

Claude (automated run).

## Executed At

2026-09-28T10:44:58Z

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-21
- [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f15a82b`. |
