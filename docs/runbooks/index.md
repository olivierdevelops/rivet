---
document_id: REF-2026-0028
title: "Rivet runbooks"
document_type: reference
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 2
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [serve, auth, policy, audit]
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

# Rivet runbooks

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** serve, auth, policy, audit

## Purpose

Executable operational procedures for Rivet: exact commands, expected output, verification, rollback, failure
handling and escalation, each with a last validation date (DOCUMENTATION §4.12, template §12.8).

```text
          ┌────────────────────── rivet serve (OPS-2026-0001) ───────────────────────┐
          │                                                                          │
   credentials change                                               effect grants change
          ▼                                                                          ▼
 RUN-2026-0001 rotate bearer tokens                         RUN-2026-0002 roll out policy.json change
 add new hash ─▶ restart ─▶ switch ─▶ remove old ─▶ restart  stage ─▶ io --check-policy ─▶ policy explain
                                                             ─▶ install + restart ─▶ verify ─▶ (rollback)
```

## What belongs here

Step-by-step procedures someone other than the author can execute (`RUN-YYYY-NNNN`), each validated against a real
build. Not here: background and configuration reference (operations/), problem diagnosis (troubleshooting/).
Naming: `run-<year>-<nnnn>-<slug>.md`, document type `runbook`; front matter carries `last_validation_date`,
`applicable_environments` and `escalation_path`.

## Active documents

| Document | Status | Last validated | Summary |
|---|---|---|---|
| [RUN-2026-0001](run-2026-0001-rotate-serve-bearer-tokens.md) | active | 2026-09-28 | Rotate `serve.auth` bearer tokens with an overlap window; old token 401 after removal |
| [RUN-2026-0002](run-2026-0002-roll-out-policy-change.md) | active | 2026-09-28 | Roll out a `policy.json` change: preview, explain, restart, verify decisions, roll back |

## Recently added or updated

- 2026-09-28: RUN-2026-0001 and RUN-2026-0002 created and executed end to end against 0.1.0-dev (commit f40d4aa) —
  PLAN-2026-0001 D-31, D-32.
- 2026-09-28: both revised (revision 2) for the fix batch at commit `829ca43`: SIGTERM drains, access log and
  `/v1/health` monitoring; procedures unchanged.

## Deprecated, superseded or archived

None.

## Important relationships

Both runbooks depend on the facts in [OPS-2026-0001](../operations/ops-2026-0001-operating-rivet-serve.md): policy is
read only at startup, and SIGINT or SIGTERM is the graceful stop (both drain and exit 0 since commit `829ca43`).

## Unresolved work

Re-validate both runbooks on the 0.1.0 release build and on Linux; revise them if a configuration reload is added.

## Reading order

1. [OPS-2026-0001](../operations/ops-2026-0001-operating-rivet-serve.md)
2. [RUN-2026-0002](run-2026-0002-roll-out-policy-change.md)
3. [RUN-2026-0001](run-2026-0001-rotate-serve-bearer-tokens.md)

## Related directories

[operations](../operations/index.md) · [troubleshooting](../troubleshooting/index.md) · [current state](../README.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Created. |
| 2 | 2026-09-28 | Claude | Both runbooks revised for the fix batch (829ca43): SIGTERM drain, access log and `/v1/health` monitoring. |
