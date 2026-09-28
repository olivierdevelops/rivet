---
document_id: REF-2026-0017
title: "Rivet incidents — resolved"
document_type: reference
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
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

# Rivet incidents — resolved

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** not-applicable → 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** language, execution, policy, audit, serve

## Purpose

Incidents in the `resolved` lifecycle state. See the [incidents index](../index.md).

## Active documents

| Incident | Severity | Status | Summary |
|---|---|---|---|
| [INC-2026-0001](inc-2026-0001-private-range-bypass-opaque-url-hosts.md) | S2 | resolved | Private-range rule skipped for udp/quic/tcp IP literals |
| [INC-2026-0002](inc-2026-0002-option-keyword-variable-misparse.md) | S3 | resolved | Variables named like option keywords parsed as options |
| [INC-2026-0003](inc-2026-0003-map-poll-assignment-and-until-misparse.md) | S3 | resolved | map/poll assignments and until conditions misparsed |
| [INC-2026-0004](inc-2026-0004-dag-dependents-saw-bare-values.md) | S3 | resolved | DAG dependents saw bare values instead of envelopes |
| [INC-2026-0005](inc-2026-0005-url-grant-path-prefix-match.md) | S2 | resolved | URL grant paths matched as raw string prefixes |
| [INC-2026-0006](inc-2026-0006-codec-keyword-variable-collision.md) | S3 | resolved | Codec keywords replaced by same-named variables at run time |

## Recently added or updated

- 2026-09-28: created during PLAN-2026-0001 P3.

## Deprecated, superseded or archived

None.

## Related directories

[incidents](../index.md) · [plans](../../plans/index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Created. |
