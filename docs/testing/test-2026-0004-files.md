---
document_id: TEST-2026-0004
title: "T-04 — security / edge (UC-04 / R5, R11)"
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
validated_plan_requirements: [PLAN-2026-0001 R5, PLAN-2026-0001 R11, PLAN-2026-0002 R1]
environment: "macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011)"
executed_by: Claude (automated)
executed_at: 2026-09-29T14:23:39Z
result: PASS
confidentiality: internal
scope: Test definition and latest recorded result for plan test T-04; re-recorded for v0.2.0 (PLAN-2026-0002 T-34).
reason: DOCUMENTATION §27 — every plan test has a TEST document linked to the requirement it validates.
related_documents: [PLAN-2026-0001, PROP-2026-0001, PLAN-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, test]
---

# T-04 — security / edge (UC-04 / R5, R11)

> **Status:** Completed
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0, 0.2.0 (re-recorded)
> **Owner:** Project maintainer
> **Affected Components:** execution

## Purpose

Symlink escape, version race, hard link, missing/full disk

## Validated Requirements

| Plan Requirement | Description |
|---|---|
| PLAN-2026-0001 R5 | 023 |
| PLAN-2026-0001 R11 | 020, 021, 035 |
| PLAN-2026-0002 R1 | T-34 regression: every 0.1.0 suite stays green on the 0.2.0 envelopes and input keys with no behaviour change |

## Preconditions

A clean checkout of `main` at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`, the v0.2.0 release candidate); Rust 1.90.0 from `rust-toolchain.toml`; `cargo build --workspace --all-features` run first (the FFI suites need `librivet`). Fixtures are started in-process on `127.0.0.1:0` or in temporary directories by the tests; no external services.

v0.1.0 run: commit `f15a82b`.

## Test Environment

macOS 26.4.1 arm64 (aarch64-apple-darwin), Rust 1.90.0, Apple clang 21.0.0, Python 3.9.6, cbindgen 0.29.4 (local); GitHub Actions run 36505156729 at the same commit on ubuntu-latest and macos-latest (Rust 1.90.0, Python 3.12); Windows unsupported in 0.2.0 (INC-2026-0011).

v0.1.0 run: macOS (aarch64-apple-darwin), Rust 1.90.0; Linux and Windows not executed (no CI runner: TASK-051 blocked on a git remote)

## Test Data

Test fixtures and bundles defined in `tests/conformance_files.rs` and the shared helpers under `tests/` (in-process servers, temporary directories).

## Procedure

```sh
cargo test --workspace --all-targets --all-features --no-fail-fast   # full run → /tmp/p3-tests.log
cargo test --all-features --test conformance_files
```

Tests executed (24):

- `conformance_files::access_narrowed_create_only` — S146 an access-narrowed create-only grant allows create but denies update/write/append before any byte is written
- `conformance_files::atomic_replace_never_leaves_temp_files` — atomic replacement leaves no temporary sibling behind, after success and after a failed replace
- `conformance_files::codec_named_variables_keep_keywords` — a variable named like a codec (`text`, `json`) never replaces the codec keyword: `text text` writes the variable, `as text` still selects the codec
- `conformance_files::copy_and_move_guard_the_destination` — S40 copy/move never overwrite by default or with `overwrite false`; the destination stays unchanged
- `conformance_files::create_is_exclusive` — S33 create is exclusive: an existing destination is conflict.already_exists and stays unchanged
- `conformance_files::delete_grant_and_sibling_prefix` — delete needs allow_delete (write alone is insufficient); `./out/**` never matches the sibling `./out2`
- `conformance_files::delete_with_missing_policy` — S38 delete removes one file; missing fails unless `missing ok`; directories are not deleted
- `conformance_files::demo_04_files_chunks_runs` — G35: demo 04 `files.chunks` runs under its own policy.json (temp copy) and its emitted chunks concatenate to data/lines.txt; the manifest shows one allow_read read site
- `conformance_files::dotdot_and_absolute_paths_are_refused` — `..` escapes and absolute paths are refused (permission, exit 3) and nothing outside the root is touched
- `conformance_files::file_effects_are_traced_with_operation_and_span` — B2: file decisions in the trace and file permission errors carry the operation ID and the statement's source span
- `conformance_files::hard_links_are_refused` — S139 a hard-linked target is refused for update/write/append/delete/copy destination (file.hardlink_refused, exit 3); neither path changes
- `conformance_files::list_and_stat` — S39 list is sorted by name and stat reports size/version without reading contents into the result
- `conformance_files::no_policy_denies_every_file_verb` — S66 without policy.json every file verb is denied with zero effects while pure operations still run
- `conformance_files::parent_directories_are_never_created_implicitly` — Increment 4 parent directory creation is explicit: create/write/copy into a missing directory fail not_found and create nothing
- `conformance_files::read_codecs_and_failures` — S34 read decodes json/text/bytes; missing is not_found; malformed JSON and non-UTF-8 text are parse errors
- `conformance_files::scoped_file_handles_are_confined_and_typed` — G35: handles are confined (escapes and symlinks refused), mode misuse is typed, and `with file watch` is unsupported.stage_c
- `conformance_files::scoped_read_yields_bounded_byte_chunks` — G35: `with file open P mode read as h` + `chunk_size N` yields bytes chunks of at most N whose concatenation is the file; the scope closes the handle
- `conformance_files::scoped_write_and_append_are_authorized_per_mode` — G35: write mode creates/truncates and needs allow_write create+update, append needs an existing file and allow_write append; reads need allow_read read — a denial opens nothing
- `conformance_files::symlinked_directory_component_is_refused` — a symlinked directory component (inside or outside the root) is refused: a grant on ./out/** never reaches ./data through out/link
- `conformance_files::symlinked_file_is_refused` — a symlinked final component is refused for read/update/write/append/delete and its target is unchanged
- `conformance_files::trailing_chunk_size_on_the_with_line_applies` — `chunk_size N` written at the end of the `with file open … as NAME` line applies like its own option line
- `conformance_files::update_replaces_existing_only` — S35 update replaces an existing file only; a missing target is not_found and nothing is created
- `conformance_files::update_with_version_guard` — S36 if_version: the stat version guards the update; a changed file is conflict.version and stays unchanged
- `conformance_files::write_upserts_and_append_needs_existing` — S37 write creates or replaces explicitly; append adds to an existing file and never creates one

v0.1.0 run: `cargo test conformance_files (temp root)`.

## Expected Results

Correct guards; nothing mutated outside the root

## Actual Results

### v0.2.0 run (2026-09-29, commit `14750b8`)

24 passed, 0 failed, 0 ignored on macOS at commit `14750b8` (v0.2.0 release candidate). This is the PLAN-2026-0002 T-34 regression: the 0.1.0 suite passes on the 0.2.0 envelopes and input keys (TASK-018 changed only request and response shapes). Green on ubuntu-latest and macos-latest in CI run 36505156729, which runs the same `cargo test --workspace --all-targets --all-features --no-fail-fast`.

### v0.1.0 run (2026-09-28, commit `f15a82b`)

24 passed, 0 failed, 0 ignored.

## Result

PASS

Windows is not a supported platform in 0.2.0 (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)), so it is not a reason for a PARTIAL result.

v0.1.0 run: PASS.

## Evidence

v0.2.0 run:

```text
conformance_files                  test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s

GitHub Actions run 36505156729 (commit 14750b8, conclusion: success)
  test (ubuntu-latest)  success    test (macos-latest)  success
  features (none|serve|grpc|quic|oauth|cli)  success ×6    deny  success
```

v0.1.0 run:

```text
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.18s
```

## Evidence Sources

- The commands above, run at commit `14750b8`; full-suite output in `/tmp/p3-tests.log` (493 passed, 0 failed), read with `grep -E "test result|FAILED|panicked"`.
- Test source: `tests/conformance_files.rs`.
- CI: [run 36505156729](https://github.com/olivierdevelops/rivet/actions/runs/36505156729) at the same commit (`test (ubuntu-latest)`, `test (macos-latest)`, six `features` jobs, `deny`: all `success`).

v0.1.0 run:

- Command above, run at commit `f15a82b`.
- Test source: `tests/conformance_files.rs`.

## Executed By

Claude (automated run).

## Executed At

2026-09-29T14:23:39Z (v0.2.0 run; v0.1.0 run: 2026-09-28T10:44:19Z)

## Defects Raised

Defects found while building this suite are recorded as incidents (see [incidents](../incidents/index.md)) and fixed before this run.

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — Test and Validation Checklist row T-34
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — Test and Validation Checklist row T-04
- [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Executed and recorded at commit `f15a82b`. |
| 2 | 2026-09-29 | Claude | Re-recorded for v0.2.0 at commit `14750b8` (PLAN-2026-0002 T-34, TASK-061): PASS. v0.1.0 run kept as history. |
