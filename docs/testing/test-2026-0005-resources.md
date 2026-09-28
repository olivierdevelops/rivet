---
document_id: TEST-2026-0005
title: "T-05 — integration / fault (UC-05 / R3, R12, R13)"
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
validated_plan_requirements: [PLAN-2026-0001 R3, PLAN-2026-0001 R12, PLAN-2026-0001 R13]
environment: "macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)"
executed_by: Claude (automated)
executed_at: 2026-09-28T06:36:46Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-05.
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-05 — integration / fault (UC-05 / R3, R12, R13)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

HTTP/socket/process fixtures; early return; EOF; forced cleanup faults; Stage C refusal (TASK-094)

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R3 | 022, 033–035 |
| PLAN-2026-0001 R12 | 033–035, 094 |
| PLAN-2026-0001 R13 | 021, 035, 037, 093 |

## Preconditions

A clean checkout at commit `19bd7c3`; Rust toolchain from `rust-toolchain.toml`; fixtures are started in-process on `127.0.0.1:0` by the tests (no external services).

## Test Environment

macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in `tests/conformance_resources.rs` and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
cargo test conformance_resources
```

Tests executed:

- `cancel_closes_handles_gracefully_in_reverse_order`
- `cancel_terminates_and_reaps_child_processes`
- `deadline_expiry_closes_handles_gracefully`
- `demo_03_http`
- `demo_04_socket_ping`
- `http_authorization`
- `http_interpolation_keeps_segments`
- `http_multipart_and_xml_bodies`
- `http_one_shot_methods`
- `http_status_retry_redirect`
- `https_with_ca_file`
- `process_one_shot`
- `process_refusals`
- `process_sandbox_confines_reads`
- `process_stream_break_reaps`
- `stage_c_refusals`
- `tcp_finish_send_raw_iteration`
- `tcp_length32`
- `tcp_newline_and_cleanup`
- `unix_json`
- `websocket_exchange`

## Expected Results

No live handles after 5 s; primary error kept; `unsupported.*` for Stage C

## Actual Results

21 passed, 0 failed, 0 ignored.

## Result

PASS

## Evidence

```text
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.70s
```

## Evidence Sources

- Command above, run at commit `19bd7c3`.
- Test source: `tests/conformance_resources.rs`.

## Executed By

Claude (automated run).

## Executed At

2026-09-28T06:36:46Z

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-05
- [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `19bd7c3`. |
