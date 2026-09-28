---
document_id: REF-2026-0025
title: "Rivet manuals index"
document_type: reference
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 4
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
scope: Navigation and status for docs/manuals/ — the current-state manual set MAN-2026-0001 … MAN-2026-0008 for Rivet 0.1.0.
reason: Every documentation directory needs an index.md (AGENTS.md "Directory Indexes"); PLAN-2026-0001 rows D-34 and TASK-089.
related_documents: [MAN-2026-0001, MAN-2026-0002, MAN-2026-0003, MAN-2026-0004, MAN-2026-0005, MAN-2026-0006, MAN-2026-0007, MAN-2026-0008, PLAN-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, manual, index]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-28
---

# Rivet manuals index

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
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
                         │ purpose · what's new · concepts ·        │
                         │ CATALOGUE · KNOWN LIMITATIONS · glossary │
                         └───────────────┬──────────────────────────┘
          ┌──────────────┬───────────────┼───────────────┬───────────────┬──────────────┐
          ▼              ▼               ▼               ▼               ▼              ▼
     MAN-0002       MAN-0003         MAN-0004        MAN-0005        MAN-0006       MAN-0007/0008
     install +      language         CLI             policy.json +   serve +        library /
     quickstart     guide            reference       I/O manifest    surfaces       protocols
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

All eight apply to Rivet **0.1.0** and were verified against `rivet 0.1.0-dev` (commit `f40d4aa`), then updated
and re-verified for the post-P3 fix batch at commit `829ca43`.

## Recently added or updated

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

- Implements PLAN-2026-0001 documentation rows D-34 … D-41; describes the build that implements
  [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md).
- Each task section links a sample folder in [`docs/demos/`](../demos/README.md).
- Wire contracts: [`docs/api/`](../api/api-2026-0001-http-rest-sse-polling.md); internals: [`docs/system/`](../system/index.md);
  operations: [OPS-2026-0001](../operations/ops-2026-0001-operating-rivet-serve.md).

## Unresolved work and open questions

- Release document `REL-0.1.0` does not exist yet; "What's New" will link it once created (P5).
- The demo folder READMEs are being re-verified for 0.1.0 (PLAN-2026-0001 TASK-067); until then the manual's
  examples are the verified record.
- The findings recorded while writing the first version (no `else`, secret values returned/emitted, `with file
  open` unavailable, no `rivet.capabilities`, multicast manifest mismatch) are fixed in commit `829ca43`; the
  remaining limitations are the [Known Limitations](man-2026-0001-rivet-manual.md#known-limitations) chapter.
- The two defects found at `829ca43` (`rivet.trace.export` not dispatched; MCP `tools/list` missing two built-ins)
  were fixed in commit `2a751ab` (INC-2026-0007) and the volumes were updated; no known defect is open.

## Related directories

[`docs/demos/`](../demos/index.md) · [`docs/references/`](../references/index.md) ·
[`docs/api/`](../api/api-2026-0001-http-rest-sse-polling.md) · [`docs/system/`](../system/index.md) ·
[`docs/plans/`](../plans/index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 4 | 2026-09-28 | Claude | Recorded the fix-batch update of all eight volumes (commits 829ca43 and 2a751ab) and the Known Limitations chapter. Perch entries unchanged. |
| 3 | 2026-09-28 | Codex | Changed Perch installation to a release build followed by bman add, as requested by the maintainer. |
| 2 | 2026-09-28 | Codex | MAN-2026-0002 now documents Perch build and install tasks. |
| 1 | 2026-09-28 | Claude | Created the manuals index for the 0.1.0 manual set MAN-2026-0001 … 0008. |
