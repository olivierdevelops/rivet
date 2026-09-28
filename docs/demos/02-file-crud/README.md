---
document_id: DEMO-2026-0002
title: "Create, read, update and delete a file"
document_type: demo
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 5
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, files, policy, audit, cli]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development]
audience: [developers, reviewers]
scope: Runnable file CRUD demo — exclusive create, read, existing-only update, idempotent delete and list under ./out, the I/O manifest, the files each operation needs, and the policy denials that narrow write and delete authority.
reason: User requested sample files in folders with READMEs showing usage; UQ-17 adds declared outputs and policy.json-only policy; UQ-18 adds the generated I/O manifest; TASK-067 executed every step against the 0.1.0 release candidate.
dependencies: [PROP-2026-0001, REF-2026-0002]
related_documents: [PLAN-2026-0001, DEMO-2026-0015, DEMO-2026-0013, MAN-2026-0005, TEST-2026-0004, TEST-2026-0020, TEST-2026-0021, TEST-2026-0025]
supersedes: null
superseded_by: null
tags: [rivet, demo, files, policy, io-manifest]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-28
verified_against: "0.1.0"
---

# Create, read, update and delete a file

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** language, files, policy, audit, cli

## Purpose

Create, read, update and delete one JSON file. Delivery stage: **A**. Read [app.rivet](app.rivet) alongside this walkthrough.

| Operation ID | Output | Behavior |
|---|---|---|
| `notes.create` | `object {created}` | Exclusive create; declares `conflict.already_exists`. |
| `notes.read` | `object {text}` | Read and decode JSON. |
| `notes.update` | `object {updated}` | Existing-only replacement (`not_found.file` when missing). |
| `notes.delete` | `object {absent}` | Idempotent delete (`missing ok`). |
| `notes.list` | `list json` | Name, type and size of each entry in `./out`. |

```text
   notes.create ──> out/note.json ──> notes.read ──> notes.update ──> notes.read
   (allow_write      {text}           (allow_read)   (allow_read stat +      |
    create)                                            allow_write update)   v
                         notes.list (allow_read list ./out) ──> notes.delete (allow_delete)
```

## Verified Against Version

0.1.0. Verified on 0.1.0-dev at commit `829ca43`, the release candidate (the version bump to 0.1.0 happens at release, P5), with `target/release/rivet` on macOS 26.4.1 (Darwin 25.4.0, arm64), 2026-09-28. Every output block below was pasted from that run. Request and trace IDs, sizes and hashes vary between runs.

## Prerequisites

```sh
cargo build --release                       # from the repository root
export PATH="$PWD/target/release:$PATH"     # `rivet --version` prints rivet 0.1.0
```

No fixture services are needed; everything happens under `./out` in this folder.

## Setup

```sh
cd docs/demos/02-file-crud
```

[policy.json](policy.json) sits next to app.rivet, so every command loads it automatically. It grants read, write and delete only under `./out`; targets are resolved relative to the policy file's directory.

```text
  capability     targets (relative to policy.json)   used by
  allow_read     ./out, ./out/**                     notes.read, notes.list, notes.update (stat)
  allow_write    ./out/**                            notes.create, notes.update
  allow_delete   ./out/**                            notes.delete
```

Steps 1–4 inspect the bundle without performing I/O. Step 5 creates `./out`.

## Steps

### 1. Check the bundle and view outputs

#### Command / Request

```sh
rivet --file app.rivet check --strict-docs
rivet --file app.rivet outputs notes.create
rivet --file app.rivet outputs --all --json
```

#### Expected Output / Response

```text
ok: 5 operations, 0 connectors, 0 auth profiles
```

```text
notes.create — Create a note
output  object   Confirmation that the note file was created.
  created  boolean  required  Always true on success.
emits    —
receives —
errors
  conflict.already_exists   out/note.json already exists; it is left unchanged.
```

`outputs --all --json` prints one line with five entries sorted by ID; two of them, reformatted:

```json
[
  {"id":"notes.create",
   "output":{"type":"object","properties":{"created":{"type":"boolean","description":"Always true on success."}},
             "required":["created"],"additionalProperties":false,
             "description":"Confirmation that the note file was created."},
   "emits":null,"receives":null,
   "errors":[{"code":"conflict.already_exists","description":"out/note.json already exists; it is left unchanged."}]},
  {"id":"notes.list",
   "output":{"type":"array","items":{},"description":"One metadata object per directory entry, sorted by name."},
   "emits":null,"receives":null,"errors":[]}
]
```

All exit 0.

### 2. I/O manifest by target

#### Command / Request

```sh
rivet --file app.rivet io --by target
rivet --file app.rivet io --access create,update,delete
```

#### Expected Output / Response

One row per target and capability. `file update` produces two sites: a `stat` (allow_read) that checks the file exists, then the `update` (allow_write):

```text
TARGET           ACCESS          CAPABILITY    ORIGIN                    PHASE  NEEDS FILE   USED BY
./out/note.json  read, stat      allow_read    file read, file update    body   yes          notes.read, notes.update
./out/note.json  create, update  allow_write   file create, file update  body   yes: update  notes.create, notes.update
./out/note.json  delete          allow_delete  file delete               body   no           notes.delete
./out            list            allow_read    file list                 body   yes          notes.list
```

`--access create,update,delete` keeps only the mutating sites:

```text
OPERATION     KIND  ACCESS  TARGET           KNOWLEDGE  SOURCE
notes.create  file  create  ./out/note.json  exact      app.rivet:9
notes.delete  file  delete  ./out/note.json  exact      app.rivet:40
notes.update  file  update  ./out/note.json  exact      app.rivet:30
```

```text
 app.rivet source line                        access    capability      policy.json grant
 -------------------------------------------  --------  --------------  -----------------
 :9   file create "./out/note.json"   ──────▶  create    allow_write     ./out/**
 :19  file read   "./out/note.json"   ──────▶  read      allow_read      ./out/**
 :30  file update "./out/note.json"   ──┬───▶  stat      allow_read      ./out/**
                                        └───▶  update    allow_write     ./out/**
 :40  file delete "./out/note.json"   ──────▶  delete    allow_delete    ./out/**
 :50  file list   "./out"             ──────▶  list      allow_read      ./out
```

Both exit 0.

### 3. `io --check-policy` and `policy explain`

#### Command / Request

```sh
rivet --file app.rivet io --check-policy
rivet --file app.rivet policy explain notes.delete --params '{}' --json
```

#### Expected Output / Response

```text
OPERATION     KIND  ACCESS  TARGET           KNOWLEDGE  SOURCE        DECISION
notes.create  file  create  ./out/note.json  exact      app.rivet:9   allowed
notes.delete  file  delete  ./out/note.json  exact      app.rivet:40  allowed
notes.list    file  list    ./out            exact      app.rivet:50  allowed
notes.read    file  read    ./out/note.json  exact      app.rivet:19  allowed
notes.update  file  stat    ./out/note.json  exact      app.rivet:30  allowed
notes.update  file  update  ./out/note.json  exact      app.rivet:30  allowed
6 allowed
```

Exit 0. `policy explain … --json` (exit 0) prints one line:

```json
{"present":true,"file":"./policy.json","sha256":"sha256:deccf2027323af83c9798a05d6cac1adb657a81852e54ac7b99ce3eca4b0bef3","grants":3,"deny":0,"broad":[],"sites":[{"effect_id":"notes.delete#1","operation_id":"notes.delete","kind":"file","access":["delete"],"method":null,"protocol":null,"capability":"allow_delete","target":{"template":"./out/note.json","expression":null,"scheme":null,"host":null,"port":null,"path":"./out/note.json","glob":null,"params":[]},"knowledge":"exact","condition":null,"call_chain":["notes.delete"],"secrets":[],"source":{"file":"app.rivet","line":40,"column":5},"origin":{"statement":"file delete"},"phase":"body","requires_existing":false,"secret":false,"via":null,"decision":"allowed","attempts":null}]}
```

### 4. Files each operation needs

#### Command / Request

```sh
rivet --file app.rivet io --needs
rivet --file app.rivet io --check-files
```

#### Expected Output / Response

`io --needs` (exit 0):

```text
notes.create needs no existing files.
notes.delete needs no existing files.
notes.list needs, before it can run:
  ./out            (file list)
notes.read needs, before it can run:
  ./out/note.json  (file read)
notes.update needs, before it can run:
  ./out/note.json  (file update)
```

`io --check-files` stats each needed file through the policy broker. Before setup nothing exists, so it exits **4**:

```text
notes.create needs no existing files.
notes.delete needs no existing files.
notes.list needs, before it can run:
  ./out            (file list)     missing
notes.read needs, before it can run:
  ./out/note.json  (file read)     missing
notes.update needs, before it can run:
  ./out/note.json  (file update)   missing
2 files · 2 missing
```

After step 5's create it reports `present` for both and `2 files · 2 present` (exit 0).

### 5. Create, read, update, list and delete

#### Command / Request

```sh
mkdir -p out
rivet --file app.rivet request notes.create --params '{"text":"first draft"}'
rivet --file app.rivet request notes.read --params '{}'
rivet --file app.rivet request notes.update --params '{"text":"reviewed draft"}'
rivet --file app.rivet request notes.read --params '{}'
rivet --file app.rivet request notes.list --params '{}'
rivet --file app.rivet request notes.delete --params '{}'
```

#### Expected Output / Response

Each call exits 0. Mutations report `effects: "committed"`, reads `"none"`:

```json
{"request_id":"req_016460ee1d","trace_id":"tr_016460ee1d","result":{"created":true},"data_count":0,"effects":"committed"}
{"request_id":"req_016223f565","trace_id":"tr_016223f565","result":{"text":"first draft"},"data_count":0,"effects":"none"}
{"request_id":"req_01615af355","trace_id":"tr_01615af355","result":{"updated":true},"data_count":0,"effects":"committed"}
{"request_id":"req_0161bbd03d","trace_id":"tr_0161bbd03d","result":{"text":"reviewed draft"},"data_count":0,"effects":"none"}
{"request_id":"req_0160d493b5","trace_id":"tr_0160d493b5","result":[{"name":"note.json","type":"file","size":31}],"data_count":0,"effects":"none"}
{"request_id":"req_015f76b5dd","trace_id":"tr_015f76b5dd","result":{"absent":true},"data_count":0,"effects":"committed"}
```

### 6. Failing examples: conflict, missing file, idempotent delete

#### Command / Request

```sh
rivet --file app.rivet request notes.update --params '{"text":"x"}'          # before any create
rivet --file app.rivet request notes.create --params '{"text":"first draft"}'
rivet --file app.rivet request notes.create --params '{"text":"again"}'      # second create
rivet --file app.rivet request notes.delete --params '{}'
rivet --file app.rivet request notes.delete --params '{}'                    # already absent
rivet --file app.rivet request notes.read --params '{}'                      # after delete
```

#### Expected Output / Response

Update never creates the file (exit 4):

```json
{"request_id":"req_016516cac5","trace_id":"tr_016516cac5","error":{"kind":"not_found","code":"not_found.file","message":"./out/note.json: no such file","retryable":false,"effects":"none","source":{"file":"app.rivet","line":30,"column":5,"end_line":30,"end_column":52},"operation_id":"notes.update"}}
```

The second create leaves the file unchanged (exit 4):

```json
{"request_id":"req_016389ab6d","trace_id":"tr_016389ab6d","error":{"kind":"conflict","code":"conflict.already_exists","message":"./out/note.json already exists","retryable":false,"effects":"none","source":{"file":"app.rivet","line":9,"column":5,"end_line":9,"end_column":52},"operation_id":"notes.create"}}
```

Both deletes return `{"absent":true}` and exit 0. The first reports `effects: "committed"`; the second removed nothing and reports `effects: "none"`. Reading after the delete fails with `not_found.file` at `app.rivet:19` (exit 4).

### 7. Narrowed and missing authority (scratch copy)

Do not edit this folder's policy.json; the manifest tests pin it. Use a scratch copy:

#### Command / Request

```sh
WORK="$(mktemp -d)"; cp app.rivet "$WORK/"; cd "$WORK"; mkdir out
rivet --file app.rivet request notes.create --params '{"text":"a"}'     # no policy.json at all
cat > policy.json <<'JSON'
{
  "version": 1,
  "grants": [
    {"capability": "allow_read", "targets": ["./out", "./out/**"]},
    {"capability": "allow_write", "targets": ["./out/**"], "access": ["create"]}
  ]
}
JSON
rivet --file app.rivet io --check-policy
rivet --file app.rivet request notes.create --params '{"text":"a"}'
rivet --file app.rivet request notes.update --params '{"text":"b"}'
rivet --file app.rivet request notes.delete --params '{}'
```

#### Expected Output / Response

Without a policy file every effect is denied (exit 3):

```json
{"request_id":"req_012552f7ed","trace_id":"tr_012552f7ed","error":{"kind":"permission","code":"permission.denied","message":"allow_write create on ./out/note.json denied: no policy.json: allow_write is denied by default (add a grant for ./out/note.json)","retryable":false,"effects":"none","source":{"file":"app.rivet","line":9,"column":5,"end_line":9,"end_column":52},"operation_id":"notes.create","details":{"capability":"allow_write","access":"create","target":"./out/note.json"}}}
```

With the create-only grant and no `allow_delete`, `io --check-policy` exits 3:

```text
OPERATION     KIND  ACCESS  TARGET           KNOWLEDGE  SOURCE        DECISION
notes.create  file  create  ./out/note.json  exact      app.rivet:9   allowed
notes.delete  file  delete  ./out/note.json  exact      app.rivet:40  denied
notes.list    file  list    ./out            exact      app.rivet:50  allowed
notes.read    file  read    ./out/note.json  exact      app.rivet:19  allowed
notes.update  file  stat    ./out/note.json  exact      app.rivet:30  allowed
notes.update  file  update  ./out/note.json  exact      app.rivet:30  denied
4 allowed · 2 denied
```

The runtime agrees: create succeeds (exit 0); update and delete fail with exit 3:

```json
{"request_id":"req_01225bd7b5","trace_id":"tr_01225bd7b5","error":{"kind":"permission","code":"permission.denied","message":"allow_write update on ./out/note.json denied: grant allow_write ./out/** access [create] does not include `update`","retryable":false,"effects":"none","source":{"file":"app.rivet","line":30,"column":5,"end_line":30,"end_column":52},"operation_id":"notes.update","details":{"capability":"allow_write","access":"update","target":"./out/note.json"}}}
{"request_id":"req_0122ed75fd","trace_id":"tr_0122ed75fd","error":{"kind":"permission","code":"permission.denied","message":"allow_delete delete on ./out/note.json denied: no grant for allow_delete ./out/note.json","retryable":false,"effects":"none","source":{"file":"app.rivet","line":40,"column":5,"end_line":42,"end_column":8},"operation_id":"notes.delete","details":{"capability":"allow_delete","access":"delete","target":"./out/note.json"}}}
```

Write authority never implies delete. Finally, a file with a second hard link is never written or deleted. Still in the scratch copy:

```sh
cp "$OLDPWD/policy.json" .      # restore the full grants
ln out/note.json out/link.json
rivet --file app.rivet request notes.update --params '{"text":"b"}'
```

```json
{"request_id":"req_012189a3f5","trace_id":"tr_012189a3f5","error":{"kind":"permission","code":"file.hardlink_refused","message":"./out/note.json has 2 hard links; write/delete refused","retryable":false,"effects":"none","source":{"file":"app.rivet","line":30,"column":5,"end_line":30,"end_column":52},"operation_id":"notes.update"}}
```

Exit 3; `notes.delete` gives the same code at `app.rivet:40`.

## Effects and policy

```text
  outcome table (verified in steps 5–7)
  ┌──────────────────────────────────────────┬─────────────────────────┬──────┬───────────┐
  │ situation                                │ code                    │ exit │ effects   │
  ├──────────────────────────────────────────┼─────────────────────────┼──────┼───────────┤
  │ create / update / delete succeed         │ —                       │  0   │ committed │
  │ read / list succeed                      │ —                       │  0   │ none      │
  │ create when the file exists              │ conflict.already_exists │  4   │ none      │
  │ update or read when the file is missing  │ not_found.file          │  4   │ none      │
  │ no policy.json / verb not granted        │ permission.denied       │  3   │ none      │
  │ target has more than one hard link       │ file.hardlink_refused   │  3   │ none      │
  └──────────────────────────────────────────┴─────────────────────────┴──────┴───────────┘
```

Selectors case-fold on case-insensitive volumes such as default macOS APFS. `io` performs no I/O and evaluates no source expression; `--check-files` performs brokered `stat` calls only.

## Release Updates

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-05 | UQ-04 / R5 | File CRUD with exclusive create, existing-only update, idempotent delete, hard-link refusal | Steps 5–7 | Results and codes in the outcome table | This README steps 5–7 (2026-09-28, 829ca43); TEST-2026-0004 |
| U-23 | UQ-17 / R23 | Declared, described outputs and errors | Step 1 | Five output schemas; `conflict.already_exists` listed | This README step 1; TEST-2026-0020 |
| U-24 | UQ-17 / R24 | policy.json auto-discovered; absent = deny; `access` narrows a grant | Steps 3, 7 | 6 allowed; no file → exit 3; create-only denies update | This README steps 3, 7; TEST-2026-0021 |
| U-26 | UQ-18 / R26 | Generated I/O manifest with `--needs` and `--check-files` | Steps 2–4 | Tables above; `--check-files` exit 4 then 0 | This README steps 2–4; TEST-2026-0025 |

## Cleanup

```sh
cd docs/demos/02-file-crud          # if you are still in the scratch copy: cd "$OLDPWD"
rm -rf "$WORK"                      # the step 7 scratch copy
rivet --file app.rivet request notes.delete --params '{}'
rmdir out
```

`rmdir` refuses when `out` holds files you placed there; remove only what this demo created.

## Verification Record

| Step | Verified By | Verified At | Result |
|---|---|---|---|
| 1. `check --strict-docs`, `outputs` | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 2. `io --by target`, `io --access` | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 3. `io --check-policy` (exit 0), `policy explain --json` | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 4. `io --needs`, `io --check-files` (exit 4 before setup, 0 after create) | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 5. Create, read, update, read, list, delete | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 6. Missing update, duplicate create, repeated delete, read after delete | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 7. No policy, create-only grant, no delete grant, hard-link refusal | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |

Build: `cargo build` and `cargo build --release` at 829ca43; every command above was executed from this folder (step 7 in a scratch copy) and the output pasted from that run.

## Known Caveats

- Request and trace IDs, the policy hash and the listed file size vary; compare results, codes and exits.
- `io --check-files` reports `missing` for `./out` until the directory exists, and exits 4.

## Related Documents

- [All sample folders](../README.md) · [demos index](../index.md) · [release verification guide](../demo-2026-0015-v0-1-0-release-verification.md)
- [Policy and I/O manifest guide](../../manuals/man-2026-0005-policy-and-io-manifest-guide.md)
- [File tests TEST-2026-0004](../../testing/test-2026-0004-files.md) · [Policy file tests TEST-2026-0021](../../testing/test-2026-0021-policy-file.md)
- [Usage reference](../../references/ref-2026-0002-language-and-usage.md) · [Proposal](../../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 5 | 2026-09-28 | Claude | TASK-067: executed every step against 0.1.0-dev (829ca43) and pasted real output. Fixes: `io --by target` now shows one row per target and capability (the old single mixed row was wrong); `--check-policy` summary line; `io --check-files` step (exit 4 before setup); failure outputs for missing update, duplicate create, no policy, create-only `access`, missing `allow_delete` and hard links; removed draft disclaimers; status active; verified_against 0.1.0. |
| 4 | 2026-09-28 | Claude | TASK-005/R26 (ADR-0001, proposal revision 8): `io --by target` gains ORIGIN, PHASE, NEEDS FILE (mixed `yes: read, stat, update` row); added `io --needs` output. |
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest: `io --by target`, source-verb → access-verb map (create/update/delete), `io --check-policy` against policy.json, `--access` filter and create-only narrowing. |
| 2 | 2026-09-28 | Claude | UQ-17: declared/described outputs and a declared conflict error; policy.json auto-discovery replaces `--policy policy.json` and `--sandbox` text; added View outputs, strict-docs, bootstrap and exit-code expectations. |
| 1 | 2026-09-28 | Codex | Added draft source files, prerequisites, invocation examples and expected behavior. |
