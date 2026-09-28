---
document_id: TEST-2026-0019
title: "T-19 — e2e / fault (UC-18 / R22)"
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
validated_plan_requirements: [PLAN-2026-0001 R22]
environment: "macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)"
executed_by: Claude (automated)
executed_at: 2026-09-28T10:44:55Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-19.
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-19 — e2e / fault (UC-18 / R22)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

open/send/read/finish/cancel across CLI/HTTP/poll/WS/MCP/library; duplicate send; stale cursor

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R22 | 029–031 |

## Preconditions

A clean checkout at commit `f15a82b`; Rust toolchain from `rust-toolchain.toml`; fixtures are started in-process on `127.0.0.1:0` by the tests (no external services).

## Test Environment

macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in `tests/conformance_sessions.rs` and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
cargo test conformance_sessions
```

Tests executed:

- `cancel_after_completion_reports_the_terminal_state`
- `cancel_ownership_and_limits`
- `cursors_and_backpressure`
- `duplex_sequence_rules_through_the_library`
- `host_buffer_budget_is_reserved_by_session_queues`
- `idle_sessions_expire_without_a_session_call`
- `open_honours_a_requested_deadline`
- `polling_routes_drive_a_duplex_session`
- `session_operations_through_the_dispatcher`

## Expected Results

Ordered, bounded, principal-bound

## Actual Results

9 passed, 0 failed, 0 ignored.

## Result

PASS

## Evidence

```text
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.38s
```

## Evidence Sources

- Command above, run at commit `f15a82b`.
- Test source: `tests/conformance_sessions.rs`.

## Executed By

Claude (automated run).

## Executed At

2026-09-28T10:44:55Z

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-19
- [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f15a82b`. |
