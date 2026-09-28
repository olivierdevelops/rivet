---
document_id: REF-2026-0029
title: "Rivet onboarding"
document_type: reference
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 3
authors: [Claude, Codex]
owner: Project maintainer
systems: [Rivet]
components: [language, execution, policy, transports]
affected_versions:
  from: "0.1.0"
  to: null
confidentiality: internal
scope: Navigation and status page.
reason: AGENTS.md requires an index.md in every documentation directory.
related_documents: [PLAN-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, index]
---

# Rivet onboarding

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** language, execution, policy, transports

## Purpose

Getting a new contributor (human or agent) from a fresh clone to a green change: toolchain, repository layout, the
VHCO loop, tests, documentation checks and common extension workflows (DOCUMENTATION §4.15).

```text
 new contributor
      │
      ▼
 ONB-2026-0001 ── toolchain ─▶ layout + five buckets ─▶ AGENTS.md loop ─▶ tests ─▶ docs checks
      │                                                                              │
      ├─▶ extending: new effect adapter · grammar change + corpus tests               │
      └─▶ stuck? troubleshooting/ TRBL-2026-0001..0003 ◀──────────────────────────────┘
```

## What belongs here

Environment setup, repository overview, first-contribution guides, local testing and glossary (`ONB-YYYY-NNNN`).
Not here: end-user instructions (manuals/), operating a deployment (operations/, runbooks/).
Naming: `onb-<year>-<nnnn>-<slug>.md`, document type `onboarding`.

## Active documents

| Document | Status | Summary |
|---|---|---|
| [ONB-2026-0001](onb-2026-0001-contributor-setup.md) | active | Contributor setup: Rust 1.90.0, protoc, cargo-deny, vhco; layout; the loop; 248 tests; docs checker; adding an effect adapter; grammar changes |

## Recently added or updated

- 2026-09-28: `perch install` now builds and installs globally with `bman add`.

- 2026-09-28: ONB-2026-0001 now documents the complete Perch development command set.
- 2026-09-28: ONB-2026-0001 created and verified at commit f40d4aa — PLAN-2026-0001 D-33.

## Deprecated, superseded or archived

None.

## Important relationships

ONB-2026-0001 summarises [AGENTS.md](../../AGENTS.md) and [DOCUMENTATION.md](../../DOCUMENTATION.md), which stay
authoritative.

## Unresolved work

Add a Linux/Windows setup section once CI results on those platforms are recorded.

## Reading order

1. [ONB-2026-0001](onb-2026-0001-contributor-setup.md)
2. [AGENTS.md](../../AGENTS.md)
3. [Troubleshooting](../troubleshooting/index.md)

## Related directories

[operations](../operations/index.md) · [runbooks](../runbooks/index.md) · [troubleshooting](../troubleshooting/index.md) · [current state](../README.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 3 | 2026-09-28 | Codex | Changed Perch installation to a release build followed by bman add, as requested by the maintainer. |
| 2 | 2026-09-28 | Codex | ONB-2026-0001 now documents the complete Perch development command set. |
| 1 | 2026-09-28 | Claude | Created. |
