---
document_id: DEMO-2026-0002
title: "Create, read, update and delete a file"
document_type: demo
status: draft
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 4
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [examples]
affected_versions:
  from: not-applicable
  to: proposed-v0.1
applicable_environments: [development]
audience: [developers, reviewers]
scope: Illustrate the existing proposed interface with sample files; no runtime implementation or release claim.
reason: User requested sample files in folders with READMEs showing usage; UQ-17 (2026-09-28) adds declared outputs, policy.json-only policy and one serve for every surface; UQ-18 (2026-09-28) adds the generated I/O manifest and policy generate.
dependencies: [PROP-2026-0001, REF-2026-0002]
related_documents: [PROP-2026-0001, REF-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, examples, design]
confidentiality: internal
review_cycle: on-design-change
next_review_date: 2026-10-27
verified_against: not-implemented
---

# Create, read, update and delete a file

**Draft usage sample.** Rivet has no runtime or CLI implementation yet. These files make the proposed interface concrete; commands below are intended usage, not executed demos.

## Purpose

Create, read, update and delete a file. Delivery stage: **A**. Read [app.rivet](app.rivet) alongside this walkthrough.

| Operation ID | Output | Behavior |
|---|---|---|
| `notes.create` | `object {created}` | Exclusive create; declares `conflict.already_exists`. |
| `notes.read` | `object {text}` | Read JSON. |
| `notes.update` | `object {updated}` | Existing-only replacement. |
| `notes.delete` | `object {absent}` | Idempotent delete. |
| `notes.list` | `list json` | List output metadata. |

```text
   notes.create ──> out/note.json ──> notes.read ──> notes.update ──> notes.read
   (allow_write)     {text}           (allow_read)   (allow_write)          |
                                                                            v
                         notes.list (allow_read ./out) ──> notes.delete (allow_delete)
```

## Verified Against Version

None. Based on proposal revision 8 semantics (UQ-17) and S33–S39. No parser, runtime or network fixture execution is claimed.

## Prerequisites

A future Rivet build implementing this stage. External services below are controlled fixtures, not public services to contact. Commands assume the repository root initially, then the setup directory. Each folder is an independent bundle; do not concatenate folders with duplicate IDs.

## Setup

```sh
cd docs/demos/02-file-crud
mkdir -p out
```

[policy.json](policy.json) sits next to app.rivet, so every command below loads it automatically. It grants read, write and delete only under `./out`. Targets are resolved relative to the policy file's directory. Start with no `out/note.json`; a repeated create intentionally reports a conflict.

## Steps

### Command / Request

```sh
rivet --file app.rivet request notes.create --params '{"text":"first draft"}'
rivet --file app.rivet request notes.read --params '{}'
rivet --file app.rivet request notes.update --params '{"text":"reviewed draft"}'
rivet --file app.rivet request notes.read --params '{}'
rivet --file app.rivet request notes.list --params '{}'
rivet --file app.rivet request notes.delete --params '{}'
```

View the declared outputs:

```sh
rivet --file app.rivet outputs notes.create
rivet --file app.rivet outputs --all --json
```

### Expected Output / Response

In order, the results are `{"created":true}`, `{"text":"first draft"}`, `{"updated":true}`, `{"text":"reviewed draft"}`, a sorted entry list and `{"absent":true}`. Each call exits 0. A second create before the delete gives `conflict.already_exists` (exit 4). Updating a missing file does not create it; it fails with a `not_found` kind (exit 4).

`rivet outputs notes.create`:

```text
notes.create — Create a note
output  object   Confirmation that the note file was created.
  created  boolean  required  Always true on success.
emits    —
receives —
errors
  conflict.already_exists   out/note.json already exists; it is left unchanged.
```

`rivet outputs --all --json` (abbreviated to two of five entries):

```json
[
  {"id": "notes.create",
   "output": {"type": "object", "description": "Confirmation that the note file was created.",
              "properties": {"created": {"type": "boolean", "description": "Always true on success."}},
              "required": ["created"], "additionalProperties": false},
   "emits": null, "receives": null,
   "errors": [{"code": "conflict.already_exists", "description": "out/note.json already exists; it is left unchanged."}]},
  {"id": "notes.list",
   "output": {"type": "array", "items": {}, "description": "One metadata object per directory entry, sorted by name."},
   "emits": null, "receives": null, "errors": []}
]
```

## Effects and policy

```text
  capability     targets (relative to policy.json)   used by
  allow_read     ./out, ./out/**                     notes.read, notes.list
  allow_write    ./out/**                            notes.create, notes.update
  allow_delete   ./out/**                            notes.delete
```

Delete needs its own `allow_delete`; write authority does not imply it. Write and delete refuse files with a link count above 1 (`file.hardlink_refused`, exit 3). Selectors case-fold on case-insensitive volumes such as default macOS APFS. Without this policy.json (for example if you copy app.rivet elsewhere), every call is denied with `permission.denied` (exit 3), because absence means deny-by-default.

## Inspect before invoking

```sh
rivet --file app.rivet check --strict-docs
rivet --file app.rivet io --by target
rivet --file app.rivet io --check-policy
rivet --file app.rivet io --needs
rivet --file app.rivet io --access create,update,delete
rivet --file app.rivet policy explain notes.delete --params '{}' --json
```

`check --strict-docs` passes: all five public operations describe their params, output and every output field. No body uses `fail`, so declared errors are optional. `notes.create` declares its conflict code for documentation.

**`io --by target`** shows what each path is used for. Every target is a literal path, so knowledge is `exact`:

```text
TARGET            ACCESS                               CAPABILITY      ORIGIN         PHASE   NEEDS FILE     USED BY
./out/note.json   read, stat, create, update, delete   allow_read,     file create,   body    yes: read,     notes.create, notes.delete,
                                                       allow_write,    file delete,           stat, update   notes.read, notes.update
                                                       allow_delete    file read,
                                                                       file update
./out             list                                 allow_read      file list      body    yes            notes.list
```

The row for `./out/note.json` is mixed: `file read` and both `file update` sites need an existing note, while `file create` makes it and `file delete … missing ok` tolerates its absence. `io --needs` shows the same thing per operation:

```text
notes.create needs no existing files.
notes.delete needs no existing files.
notes.list needs, before it can run:
  ./out                 (file list)
notes.read needs, before it can run:
  ./out/note.json       (file read)
notes.update needs, before it can run:
  ./out/note.json       (file update)
```

**Each source verb maps to exactly one access verb.** `file update` produces two sites, because it checks that the file exists (stat) before replacing it:

```text
 app.rivet source line                        access    capability      policy.json grant
 -------------------------------------------  --------  --------------  -------------------------
 :9   file create "./out/note.json"   ──────▶  create    allow_write     ./out/**           allowed
 :19  file read   "./out/note.json"   ──────▶  read      allow_read      ./out, ./out/**    allowed
 :30  file update "./out/note.json"   ──┬───▶  stat      allow_read      ./out, ./out/**    allowed
                                        └───▶  update    allow_write     ./out/**           allowed
 :40  file delete "./out/note.json"   ──────▶  delete    allow_delete    ./out/**           allowed
 :50  file list   "./out"             ──────▶  list      allow_read      ./out              allowed
```

**`io --check-policy`** evaluates every site against the auto-discovered [policy.json](policy.json) and exits 0:

```text
OPERATION      KIND   ACCESS   TARGET            KNOWLEDGE   SOURCE         DECISION
notes.create   file   create   ./out/note.json   exact       app.rivet:9    allowed
notes.delete   file   delete   ./out/note.json   exact       app.rivet:40   allowed
notes.list     file   list     ./out             exact       app.rivet:50   allowed
notes.read     file   read     ./out/note.json   exact       app.rivet:19   allowed
notes.update   file   stat     ./out/note.json   exact       app.rivet:30   allowed
notes.update   file   update   ./out/note.json   exact       app.rivet:30   allowed
```

`io --access create,update,delete` keeps only the mutating rows: `notes.create` (create), `notes.update` (update) and `notes.delete` (delete). The access verbs also narrow grants. Changing the write grant to `{"capability": "allow_write", "targets": ["./out/**"], "access": ["create"]}` keeps `notes.create` allowed. It turns `notes.update` into `denied`, and `io --check-policy` then exits 3. Removing the `allow_delete` grant denies `notes.delete` the same way, because write authority never implies delete.

`--include-bootstrap` adds the fixed runtime-internal list under a separate `bootstrap` key: the bundle and imports, policy.json, the CA bundle, resolv.conf or the system resolver, tzdata, descriptor/schema files, and stdin/stdout/stderr. It is listed for transparency, never granted to scripts. Sandbox guarantees apply to **script-initiated effects through brokered adapters**. `io` performs no I/O and evaluates no source expression. Exit codes: 3 when `--check-policy` finds a reachable site denied or partial; 7 with `--strict` when any site is dynamic or opaque (`complete: false`); otherwise 0.

## Release Updates

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-01 | UQ-17 / R23 | Declared, described outputs | `rivet outputs --all --json` | Five output schemas | Not run — runtime does not exist |
| U-02 | UQ-17 / R24 | policy.json auto-discovered; `--policy` flag no longer needed here | Run commands without flags | Same results as before | Not run |
| U-03 | UQ-18 / R26 | `io` became the generated I/O manifest (targets, access verbs, capability, `--by`, `--check-policy`) | Inspect before invoking | Tables above | Not run — runtime does not exist |

## Cleanup

The final `notes.delete` removes the generated note. If the directory is empty, `rmdir out` removes it. Do not delete unrelated files placed in `out`.

## Verification Record

| Step | Verified by | Date | Result |
|---|---|---|---|
| JSON/JSONL parse, relative links, manifest IDs vs declared public IDs, no removed flags or bare durations | Claude static script | 2026-09-28 | Passed as documentation checks; not language conformance |
| Source IDs, JSON fixtures, links and documentation structure | Codex static review | 2026-09-28 | Checked as documentation; not language conformance |
| Parse/execute and validate expected output | Future implementation fixture suite | Not run | BLOCKED — runtime does not exist |

## Known Caveats

Expected values assume the declared fixture behavior. Request, trace and session IDs are generated; compare application results rather than literal IDs. Runtime errors remain typed and retain partial-effect information.

## Related Documents

- [All sample folders](../README.md)
- [Usage reference](../../references/ref-2026-0002-language-and-usage.md)
- [Proposal](../../proposals/approved/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 4 | 2026-09-28 | Claude | TASK-005/R26 (ADR-0001, proposal revision 8): `io --by target` gains ORIGIN, PHASE, NEEDS FILE (mixed `yes: read, stat, update` row); added `io --needs` output. |
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest: `io --by target`, source-verb → access-verb map (create/update/delete), `io --check-policy` against policy.json, `--access` filter and create-only narrowing. |
| 2 | 2026-09-28 | Claude | UQ-17: declared/described outputs and a declared conflict error; policy.json auto-discovery replaces `--policy policy.json` and `--sandbox` text; added View outputs, strict-docs, bootstrap and exit-code expectations. |
| 1 | 2026-09-28 | Codex | Added draft source files, prerequisites, invocation examples and expected behavior. |
