---
document_id: REF-2026-0027
title: "Rivet operations"
document_type: reference
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
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

# Rivet operations

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** serve, auth, policy, audit

## Purpose

How to run Rivet as a service: deployment shapes, configuration an operator owns, logs, health checks, capacity,
upgrade and rollback (DOCUMENTATION §4.11). Step-by-step procedures live in [runbooks](../runbooks/index.md).

```text
 operations/  (what the service is and how it behaves in production)
      │  "how do I do X, exactly?"
      ▼
 runbooks/    (executable procedures with verification and rollback)
      │  "it broke"
      ▼
 troubleshooting/  (known problems and fixes)
```

## What belongs here

Environment configuration, deployment, monitoring, logging, scaling and production-readiness documents
(`OPS-YYYY-NNNN`). Not here: step-by-step procedures (runbooks/), design rationale (decisions/, proposals/).
Naming: `ops-<year>-<nnnn>-<slug>.md`, document type `operations`.

## Active documents

| Document | Status | Summary |
|---|---|---|
| [OPS-2026-0001](ops-2026-0001-operating-rivet-serve.md) | active | Operating `rivet serve`: shapes, bind/auth/principals/surfaces, limits, in-memory trace store, logs and exit codes, health checks, upgrade/rollback, capacity (binary sizes) |

## Recently added or updated

- 2026-09-28: OPS-2026-0001 created and verified against 0.1.0-dev (commit f40d4aa) — PLAN-2026-0001 D-30.

## Deprecated, superseded or archived

None.

## Important relationships

OPS-2026-0001 is the parent of [RUN-2026-0001](../runbooks/run-2026-0001-rotate-serve-bearer-tokens.md) (token
rotation) and [RUN-2026-0002](../runbooks/run-2026-0002-roll-out-policy-change.md) (policy rollout).

## Unresolved work

0.1.0 has no persistent trace store, access log, metrics or configuration reload; revisit this directory when any of
them ships.

## Reading order

1. [OPS-2026-0001](ops-2026-0001-operating-rivet-serve.md)
2. [Runbooks](../runbooks/index.md)

## Related directories

[runbooks](../runbooks/index.md) · [onboarding](../onboarding/index.md) · [troubleshooting](../troubleshooting/index.md) · [current state](../README.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Created. |
