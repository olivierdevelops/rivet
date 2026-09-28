---
document_id: TEST-2026-0010
title: "T-10 — integration (UC-10 / R1, R3, R9)"
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
validated_plan_requirements: [PLAN-2026-0001 R1, PLAN-2026-0001 R3, PLAN-2026-0001 R9]
environment: "macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)"
executed_by: Claude (automated)
executed_at: 2026-09-28T10:44:27Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-10.
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-10 — integration (UC-10 / R1, R3, R9)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

Host Tokio runtime; abandoned future; scope exit

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R1 | 010, 014, 016 |
| PLAN-2026-0001 R3 | 022, 033–035 |
| PLAN-2026-0001 R9 | 022, 025 |

## Preconditions

A clean checkout at commit `f15a82b`; Rust toolchain from `rust-toolchain.toml`; fixtures are started in-process on `127.0.0.1:0` by the tests (no external services).

## Test Environment

macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in `tests/conformance_library.rs` and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
cargo test conformance_library
```

Tests executed:

- `data_sink_that_stops_early`
- `data_sink_typed_stop_cancels`
- `dropped_future_closes_the_socket`
- `dropped_future_kills_the_child_process`
- `io_manifest_and_policy_draft_from_the_library`
- `outputs_from_the_library`
- `policy_constructors_and_host_ceiling`
- `policy_from_json_matches_policy_files`
- `runtime_lives_inside_the_host_runtime`
- `scope_cancels_and_joins_what_it_started`
- `scope_stream_and_duplex`
- `stream_pull_in_the_host`

## Expected Results

No nested runtime; cleanup joined

## Actual Results

12 passed, 0 failed, 0 ignored.

## Result

PASS

## Evidence

```text
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
```

## Evidence Sources

- Command above, run at commit `f15a82b`.
- Test source: `tests/conformance_library.rs`.

## Executed By

Claude (automated run).

## Executed At

2026-09-28T10:44:27Z

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-10
- [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f15a82b`. |
