---
document_id: REF-2026-0019
title: "Rivet troubleshooting"
document_type: reference
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 4
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [language, execution, policy, audit, serve]
affected_versions:
  from: not-applicable
  to: "0.1.0"
confidentiality: internal
scope: Navigation and status page.
reason: AGENTS.md requires an index.md in every documentation directory.
related_documents: [PLAN-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, index]
---

# Rivet troubleshooting

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** not-applicable → 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** language, execution, policy, audit, serve

## Purpose

Known problems, diagnosis and fixes worth keeping (DOCUMENTATION §4.13, §26).

## Active documents

| Document | Status | Summary |
|---|---|---|
| [TRBL-2026-0001](trbl-2026-0001-vhco-helper-files-counted-as-use-cases.md) | resolved | vhco counts helper files in a feature folder as use cases |
| [TRBL-2026-0002](trbl-2026-0002-capy-section-headers-cannot-take-arguments.md) | resolved | Capy block sections cannot carry arguments (try/catch filters) |
| [TRBL-2026-0003](trbl-2026-0003-linker-fails-with-no-space-left-on-device.md) | resolved | Build fails at link time with "No space left on device" |
| [TRBL-2026-0004](trbl-2026-0004-vhco-sync-drift-after-renaming-triggers-or-steps.md) | resolved | `vhco sync` reports flow drift after a trigger or step rename: flows compare trigger text and `layer: ref` steps; edit the contract flow by hand |
| [TRBL-2026-0005](trbl-2026-0005-contract-written-ahead-of-code-drifts-on-ports-and-step-order.md) | resolved | A use case authored in the contract before its code drifts on port parameter names, step order and surface calls; write types only, steps in source order, and add triggered actions to `calls` |

## Recently added or updated

- 2026-09-28: TRBL-2026-0005 added (PLAN-2026-0002 P2b/P2f).
- 2026-09-28: TRBL-2026-0004 added (PLAN-2026-0002 P2a).
- 2026-09-28: created during PLAN-2026-0001 P3.
- 2026-09-28: TRBL-2026-0002 revision 2 — the bare-section rule also explains why `else if` is refused (`syntax.else_if`).

## Deprecated, superseded or archived

None.

## Related directories

[plans](../plans/index.md) · [current state](../README.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Created. |
| 2 | 2026-09-28 | Claude | Recorded TRBL-2026-0002 revision 2. |
| 3 | 2026-09-28 | Claude | Added TRBL-2026-0004. |
| 4 | 2026-09-28 | Claude | Added TRBL-2026-0005. |
