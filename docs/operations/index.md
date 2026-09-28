---
document_id: REF-2026-0027
title: "Rivet operations"
document_type: reference
status: active
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 3
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
> **Last Updated:** 2026-09-29
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
| [OPS-2026-0001](ops-2026-0001-operating-rivet-serve.md) | active | Operating `rivet serve`: install (`--features cli`), shapes, bind/auth/principals/surfaces, `Deprecation` monitoring, limits, in-memory trace store, logs and exit codes, health checks, upgrade/rollback, capacity (binary sizes) |

## Recently added or updated

- 2026-09-29: OPS-2026-0001 revision 3 for 0.2.0 (PLAN-2026-0002 D-38). It adds the `cli` feature install, monitoring of deprecated input (`deprecation: true`, `"deprecated":1`, trace rows) and the 0.1.x → 0.2.0 upgrade order. All examples were re-captured as envelopes on the 0.2.0-rc. Supported platforms are macOS and Linux.
- 2026-09-28: OPS-2026-0001 created and verified against 0.1.0-dev (commit f40d4aa) — PLAN-2026-0001 D-30.
- 2026-09-28: OPS-2026-0001 revision 2 for the fix batch (commit `829ca43`): `/v1/health`, access log, SIGTERM
  drain, enforced `max_buffered_bytes`.

## Deprecated, superseded or archived

None.

## Important relationships

OPS-2026-0001 is the parent of [RUN-2026-0001](../runbooks/run-2026-0001-rotate-serve-bearer-tokens.md) (token
rotation) and [RUN-2026-0002](../runbooks/run-2026-0002-roll-out-policy-change.md) (policy rollout).

## Unresolved work

Rivet (0.1.0 and 0.2.0) has no persistent trace store, metrics or configuration reload (it does have `GET /v1/health`, a per-request
access log and a SIGTERM drain since commit `829ca43`); revisit this directory when any of
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
| 2 | 2026-09-28 | Claude | OPS-2026-0001 revision 2 (fix batch, commit 829ca43): health route, access log, SIGTERM drain. |
| 3 | 2026-09-29 | Claude | OPS-2026-0001 revision 3 (0.2.0: cli install, Deprecation monitoring, envelope examples; D-38). |
