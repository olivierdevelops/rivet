---
document_id: REF-2026-0025
title: "Rivet manuals index"
document_type: reference
status: active
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 6
authors: [Claude, Codex]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, cli, policy, audit, serve, library, transports, connectors]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, server, embedded]
audience: [developers, operators, integrators, reviewers]
scope: Navigation and status for docs/manuals/ — the current-state manual set MAN-2026-0001 … MAN-2026-0010 (0.1.0 released; 0.2.0 release candidate documented).
reason: Every documentation directory needs an index.md (AGENTS.md "Directory Indexes"); PLAN-2026-0001 rows D-34 and TASK-089; PLAN-2026-0002 row D-28.
related_documents: [MAN-2026-0001, MAN-2026-0002, MAN-2026-0003, MAN-2026-0004, MAN-2026-0005, MAN-2026-0006, MAN-2026-0007, MAN-2026-0008, MAN-2026-0009, MAN-2026-0010, MIG-2026-0001, PLAN-2026-0001, PLAN-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, manual, index]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-29
---

# Rivet manuals index

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** language, cli, policy, audit, serve, library, transports, connectors

## Purpose of this directory

`docs/manuals/` holds the **canonical current-state book** for Rivet (DOCUMENTATION.md §4.14, §30): what a user
can do, why, and exact task instructions with verified examples. Manuals explain; demos under
[`docs/demos/`](../demos/index.md) demonstrate.

**Belongs here:** user, administrator, operator, integrator and developer manuals (`MAN-YYYY-NNNN`), organized by
task. **Does not belong here:** design proposals (`proposals/`), numbered syntax examples
(`references/`), wire-level API contracts (`api/`), implementation internals (`system/`), release history
(`releases/`).

Naming: `man-<year>-<nnnn>-<slug>.md`, lower case, ID matching the front matter.

## Recommended reading order

```text
                         ┌──────────────────────────────────────────┐
                         │ MAN-0001  Rivet manual (root book)       │
                         │ purpose · WHAT'S NEW 0.2.0 · concepts ·  │
                         │ CATALOGUE · KNOWN LIMITATIONS · glossary │
                         └───────────────┬──────────────────────────┘
      ┌──────────┬──────────┬────────────┼────────────┬──────────┬──────────┬──────────┬──────────┐
      ▼          ▼          ▼            ▼            ▼          ▼          ▼          ▼          ▼
   MAN-0002   MAN-0003   MAN-0004     MAN-0005     MAN-0006   MAN-0007   MAN-0008   MAN-0009   MAN-0010
   install +  language   CLI          policy.json  serve +    Rust       protocols  C ABI /    editors +
   quickstart (globals,  reference    + I/O        surfaces   library    +          FFI        highlight
              modules)   (--data…)    manifest     (envelopes)(facade)   features   (C/Py/Go)
```

## Active documents

| ID | Title | Audience | Status |
|---|---|---|---|
| [MAN-2026-0001](man-2026-0001-rivet-manual.md) | Rivet manual (root) | everyone | active |
| [MAN-2026-0002](man-2026-0002-installation-and-quickstart.md) | Installation and quickstart | new users, operators | active |
| [MAN-2026-0003](man-2026-0003-language-guide.md) | Language guide | developers | active |
| [MAN-2026-0004](man-2026-0004-cli-reference.md) | CLI reference | developers, operators | active |
| [MAN-2026-0005](man-2026-0005-policy-and-io-manifest-guide.md) | Policy and I/O manifest guide | administrators, reviewers | active |
| [MAN-2026-0006](man-2026-0006-serving-and-surfaces.md) | Serving and surfaces | operators, integrators | active |
| [MAN-2026-0007](man-2026-0007-embedding-library.md) | Embedding the library | Rust developers | active |
| [MAN-2026-0008](man-2026-0008-protocols-and-connectors.md) | Protocols and connectors | integrators | active |
| [MAN-2026-0009](man-2026-0009-c-abi-and-ffi.md) | Calling Rivet from C, Python and Go (librivet) | C/Python/Go developers | active |
| [MAN-2026-0010](man-2026-0010-editor-support-and-highlighting.md) | Editor support and syntax highlighting | script authors, tool authors | active |

All ten describe the **0.2.0 release candidate** (in progress; the latest published release is 0.1.0). MAN-0009/0010
were verified at `e7ed8ed`; MAN-0001…0008 were re-captured on 2026-09-29 from `cargo build --release --features cli`
with the source at `6f9943f` (the only source change after `e7ed8ed`). Supported platforms: macOS and Linux.
Clients of 0.1.0 read [MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md) first.

## Recently added or updated

- 2026-09-29 (envelope sweep, TASK-070/072): MAN-2026-0001…0008 re-captured as 0.2.0 envelopes; new sections —
  MAN-0001 "What's New in 0.2.0" and 0.2.0 catalogue rows, MAN-0004 `rivet highlight` and `--json` envelopes,
  MAN-0005 "Globals in targets" and "One policy across modules", MAN-0006 "Envelopes on every surface" and
  "Deprecation monitoring", MAN-0007 dependency/features and "Load files as module objects", MAN-0008 "Cargo
  features per protocol".
- 2026-09-29: 0.2.0 — MAN-2026-0009 (C ABI and FFI) and MAN-2026-0010 (editors and highlighting) added; the eight
  existing volumes scheduled for the 0.2.0 update (envelopes on every surface, `--data`/`--input`/`--pretty`,
  globals, modules, the `rivet-runtime` facade and Cargo features, `unsupported.feature`, `rivet highlight`), which
  the entry above completed; MAN-2026-0003 received its Globals and Modules chapters.

- 2026-09-28: `perch install` now builds and installs globally with `bman add`.

- 2026-09-28: MAN-2026-0002 now documents Perch build and install tasks.
- 2026-09-28: the whole set created for PLAN-2026-0001 P4 (rows D-34 … D-41).
- 2026-09-28: all eight volumes updated for the fix batch (commit `829ca43`): `else`, `with file open`, `check`
  warnings, `graph`, `trace export`, `rivet.capabilities`, `restrict`, library scopes and ceiling, health/access
  log/drain, `traceparent`, secret taint on every sink; MAN-2026-0001 now has a **Known Limitations** chapter that
  lists exactly the current limitations (TASK-095 documentation part).

## Deprecated, superseded or archived

None.

## Important relationships

- Implements PLAN-2026-0002 documentation rows D-13, D-14, D-20 … D-29 and D-46, and PLAN-2026-0001 rows D-34 … D-41; describes the build that implements
  [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md).
- Each task section links a sample folder in [`docs/demos/`](../demos/README.md).
- Wire contracts: [`docs/api/`](../api/api-2026-0001-http-rest-sse-polling.md); internals: [`docs/system/`](../system/index.md);
  operations: [OPS-2026-0001](../operations/ops-2026-0001-operating-rivet-serve.md).

## Unresolved work and open questions

- Release document `REL-0.2.0` is written in P5; "What's New" links it once it exists.
- The demo folders are being re-verified for 0.2.0 and demos 14–17 written (PLAN-2026-0002 TASK-075/076); the
  manuals link them by path.
- The findings recorded while writing the first version (no `else`, secret values returned/emitted, `with file
  open` unavailable, no `rivet.capabilities`, multicast manifest mismatch) are fixed in commit `829ca43`; the
  remaining limitations are the [Known Limitations](man-2026-0001-rivet-manual.md#known-limitations) chapter.
- The two defects found at `829ca43` (`rivet.trace.export` not dispatched; MCP `tools/list` missing two built-ins)
  were fixed in commit `2a751ab` (INC-2026-0007).
- Defects found by the 0.2.0 sweep, documented where they show and reported for triage (not fixed here): WS
  refused-input terminal records lack `seq` and report `data_count: 0`; `rivet highlight` drops the header tokens of
  an unclosed block; inline `output object … open true` is accepted but ignored; the remote CLI sends no `deadline_ms`
  over WebSocket.

## Related directories

[`docs/demos/`](../demos/index.md) · [`docs/references/`](../references/index.md) ·
[`docs/api/`](../api/api-2026-0001-http-rest-sse-polling.md) · [`docs/system/`](../system/index.md) ·
[`docs/plans/`](../plans/index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 6 | 2026-09-29 | Claude | Envelope sweep finished (TASK-070/072): MAN-0001…0008 re-captured on the 0.2.0-rc (source `6f9943f`) with their new 0.2.0 sections; verification line corrected; sweep defects listed. |
| 5 | 2026-09-29 | Claude | 0.2.0: MAN-2026-0009 and MAN-2026-0010 rows; reading-order diagram with ten volumes; status re-verified on the 0.2.0 release candidate |
| 4 | 2026-09-28 | Claude | Recorded the fix-batch update of all eight volumes (commits 829ca43 and 2a751ab) and the Known Limitations chapter. Perch entries unchanged. |
| 3 | 2026-09-28 | Codex | Changed Perch installation to a release build followed by bman add, as requested by the maintainer. |
| 2 | 2026-09-28 | Codex | MAN-2026-0002 now documents Perch build and install tasks. |
| 1 | 2026-09-28 | Claude | Created the manuals index for the 0.1.0 manual set MAN-2026-0001 … 0008. |
