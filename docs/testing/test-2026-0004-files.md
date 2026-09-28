---
document_id: TEST-2026-0004
title: "T-04 — security / edge (UC-04 / R5, R11)"
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
validated_plan_requirements: [PLAN-2026-0001 R5, PLAN-2026-0001 R11]
environment: "macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)"
executed_by: Claude (automated)
executed_at: 2026-09-28T06:36:45Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-04.
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-04 — security / edge (UC-04 / R5, R11)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

Symlink escape, version race, hard link, missing/full disk

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R5 | 023 |
| PLAN-2026-0001 R11 | 020, 021, 035 |

## Preconditions

A clean checkout at commit `19bd7c3`; Rust toolchain from `rust-toolchain.toml`; fixtures are started in-process on `127.0.0.1:0` by the tests (no external services).

## Test Environment

macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in `tests/conformance_files.rs` and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
cargo test conformance_files` (temp root)
```

Tests executed:

- `access_narrowed_create_only`
- `atomic_replace_never_leaves_temp_files`
- `copy_and_move_guard_the_destination`
- `create_is_exclusive`
- `delete_grant_and_sibling_prefix`
- `delete_with_missing_policy`
- `demo_04_files_chunks_runs`
- `dotdot_and_absolute_paths_are_refused`
- `file_effects_are_traced_with_operation_and_span`
- `hard_links_are_refused`
- `list_and_stat`
- `no_policy_denies_every_file_verb`
- `parent_directories_are_never_created_implicitly`
- `read_codecs_and_failures`
- `scoped_file_handles_are_confined_and_typed`
- `scoped_read_yields_bounded_byte_chunks`
- `scoped_write_and_append_are_authorized_per_mode`
- `symlinked_directory_component_is_refused`
- `symlinked_file_is_refused`
- `update_replaces_existing_only`
- `update_with_version_guard`
- `write_upserts_and_append_needs_existing`

## Expected Results

Correct guards; nothing mutated outside the root

## Actual Results

22 passed, 0 failed, 0 ignored.

## Result

PASS

## Evidence

```text
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s
```

## Evidence Sources

- Command above, run at commit `19bd7c3`.
- Test source: `tests/conformance_files.rs`.

## Executed By

Claude (automated run).

## Executed At

2026-09-28T06:36:45Z

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-04
- [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `19bd7c3`. |
