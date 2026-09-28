---
document_id: REF-2026-0022
title: "Rivet security documentation index"
document_type: reference
status: active
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 3
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [policy, serve, auth, connectors, transports]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, embedded, server]
audience: [maintainers, operators, security-reviewers]
scope: Navigation and status page for docs/security — Rivet's threat model, guarantees, non-guarantees and platform security matrix.
reason: AGENTS.md "Directory Indexes" requires an index for every documentation directory; created with SEC-2026-0001 (PLAN-2026-0001 D-29).
related_documents: [PLAN-2026-0001, SEC-2026-0001, ADR-0003, INC-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, security, index]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-29
---

# Rivet security documentation index

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** policy, serve, auth, connectors, transports

## Purpose

`docs/security/` states what Rivet protects against, how, and where it stops. It describes the **implemented** behaviour of the current release (0.2.0: macOS and Linux; Windows not supported); design intent that is not implemented is listed as a non-guarantee, not as a feature.

```text
   policy.json ─▶ PolicyBroker ─▶ every effect attempt       (SEC-2026-0001 §Policy model)
   serve.auth  ─▶ every request on every surface            (§Serve authentication)
   OS sandbox  ─▶ every child process when policy.json exists (§Process sandbox matrix: macOS active, Linux gated)
   librivet    ─▶ host = principal; guarded C boundary        (§FFI trust boundary)
   import/load ─▶ root-confined modules under ONE policy      (§File modules)
```

## What belongs here

- Belongs: threat models, trust boundaries, guarantees and gaps, authentication/authorization design, secrets handling, platform security matrices, security review records.
- Does not belong: incident timelines ([incidents/](../incidents/index.md)), wire formats ([api/](../api/index.md)).

## Naming rules

`sec-YYYY-NNNN-<slug>.md`, `document_type: security`, ID prefix `SEC`, `confidentiality` at least `internal`.

## Active documents

| ID | Document | Covers |
|---|---|---|
| SEC-2026-0001 | [Policy and sandbox security model](sec-2026-0001-policy-and-sandbox-model.md) | threat model, guarantees G1–G12 (incl. ceiling/restrict narrowing and secret taint), non-guarantees, bootstrap I/O, SSRF/DNS rebinding, file confinement, sandbox per OS, secrets, OAuth, MCP trust boundary, serve auth, 0.2.0 additions (FFI trust boundary, globals and secrets, file modules, Cargo features), INC-2026-0001 |

## Recently added or updated

2026-09-28 — SEC-2026-0001 created for 0.1.0, verified against commit `f40d4aa`.

2026-09-28 — SEC-2026-0001 revision 2 (commit `829ca43`): secret taint enforced on every sink, host ceiling and per-request `restrict`, URL path segments (INC-2026-0005), OAuth cache key and no-expiry rule, MCP drift check.

2026-09-29 — SEC-2026-0001 revision 3 (0.2.0): FFI trust boundary, globals cannot hold secrets, module confinement and single policy, feature refusals, platform support (Windows dropped).

## Deprecated, superseded or archived

None.

## Important relationships

- Sandbox backend decision: [ADR-0003](../decisions/adr-0003-process-sandbox-backends.md).
- Fixed defect used as a worked example: [INC-2026-0001](../incidents/resolved/inc-2026-0001-private-range-bypass-opaque-url-hosts.md).
- Structure of the broker and adapters: [ARCH-2026-0001](../architecture/arch-2026-0001-rivet-runtime-architecture.md).

## Unresolved work and open questions

- Secret taint covers explicit flows only (implicit flows are out of scope by design).
- The `*` principal pattern matches `rivet.auth.*` (governed by `allow_auth`).
- Linux sandbox is gated until verified on kernel ≥ 6.12; Windows and other OSes are unsupported.
- No TLS/mTLS listener; traces and sessions are in-memory only.

## Recommended reading order

1. [SEC-2026-0001](sec-2026-0001-policy-and-sandbox-model.md) · 2. [Error registry](../api/api-2026-0005-error-registry.md) (`permission`, `auth`, `unsupported` kinds) · 3. [ARCH-2026-0001](../architecture/arch-2026-0001-rivet-runtime-architecture.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Created the security index with SEC-2026-0001. |
| 2 | 2026-09-28 | Claude | Recorded SEC-2026-0001 revision 2 (fix batch, commit 829ca43); open questions updated. |
| 3 | 2026-09-29 | Claude | 0.2.0: SEC-2026-0001 revision 3 recorded; purpose diagram adds the C ABI and modules; platform scope macOS and Linux |
