---
document_id: REF-2026-0020
title: "Rivet API documentation index"
document_type: reference
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 2
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [http, poll, ws, mcp, library, serve]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, embedded, server]
audience: [developers, integrators, operators]
scope: Navigation and status page for docs/api — the current-state contracts of every Rivet access point and the shared error registry.
reason: AGENTS.md "Directory Indexes" requires an index for every documentation directory; created with the 0.1.0 API documents (PLAN-2026-0001 D-24 to D-28).
related_documents: [PLAN-2026-0001, API-2026-0001, API-2026-0002, API-2026-0003, API-2026-0004, API-2026-0005]
supersedes: null
superseded_by: null
tags: [rivet, api, index]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-28
---

# Rivet API documentation index

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** http, poll, ws, mcp, library, serve

## Purpose

`docs/api/` holds the **current, implemented** contracts of Rivet's programmatic access points: routes, frames, tools, Rust items, request/response shapes, authentication and errors. Every example in these documents was captured from a real build. Design history stays in the [approved proposal](../proposals/implemented/prop-2026-0001-rivet-runtime.md); command-line usage belongs to the manuals; internal structure belongs to [architecture](../architecture/index.md).

```text
                    one bundle (app.rivet + policy.json)
                                   │
                           Runtime (one dispatcher)
     ┌───────────┬─────────────┬───┴─────────┬──────────────┬──────────────┐
     │ CLI       │ REST + SSE  │ polling     │ WebSocket    │ MCP          │ Rust library
     │ (manual)  │ API-0001    │ API-0001    │ API-0002     │ API-0003     │ API-0004
     └───────────┴─────────────┴─────────────┴──────────────┴──────────────┘
                     every failure: one error registry ── API-0005
```

## What belongs here

- Belongs: HTTP routes, WebSocket frames, MCP tools, library signatures, error codes, compatibility notes.
- Does not belong: CLI task walkthroughs (manuals), component internals (architecture), threat model (security), demos.

## Naming rules

`api-YYYY-NNNN-<slug>.md`, `document_type: api`, ID prefix `API`. One document per access point; the error registry is shared.

## Active documents (recommended reading order)

| ID | Document | Covers |
|---|---|---|
| API-2026-0001 | [HTTP REST, SSE and polling](api-2026-0001-http-rest-sse-polling.md) | `/v1/request` (with `restrict`, `traceparent`), `/v1/operations…`, `/v1/io`, `/v1/policy/generate`, `/v1/health`, SSE framing, `/v1/requests…` sessions, auth, access log, drain |
| API-2026-0002 | [WebSocket rivet.v1](api-2026-0002-websocket-rivet-v1.md) | `/v1/ws` frames, refs (8 per socket, 16-frame lane each), specific refusal frames, limits, close semantics |
| API-2026-0003 | [MCP server tools](api-2026-0003-mcp-server-tools.md) | `/mcp` Streamable HTTP and `serve --stdio`: initialize, tools/list (direct tools + built-ins), `restrict`, session delivery |
| API-2026-0004 | [Rust library](api-2026-0004-rust-library.md) | `Runtime`, `RuntimeBuilder` (+ `.ceiling`), `Policy::from_file/from_json`, `Runtime::scope` stream/duplex, `request_restricted`, sessions, audit (`export_trace`, `graph`), embedding serve |
| API-2026-0005 | [Error registry](api-2026-0005-error-registry.md) | 22 kinds → HTTP → exit → retryable; every emitted code; `check` warnings |

## Recently added or updated

2026-09-28 — all five documents created for the 0.1.0 release (PLAN-2026-0001 D-24 – D-28), verified against commit `f40d4aa`.

2026-09-28 (revision 2 of each) — updated for the post-P3 fix batch and re-verified against commit `829ca43`: `/v1/health`, `restrict`, `traceparent`, access log and SIGTERM drain; WebSocket lanes and specific refusal frames; MCP `tools/list` built-ins; library scopes, ceiling and `Policy` constructors; new error codes and `check` warnings.

## Deprecated, superseded or archived

None.

## Important relationships

- Implements the surface requirements of [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md) under [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md).
- Security properties of these surfaces (serve auth, principal map, MCP trust boundary): [SEC-2026-0001](../security/sec-2026-0001-policy-and-sandbox-model.md).
- How the surfaces reach the shared dispatcher: [ARCH-2026-0001](../architecture/arch-2026-0001-rivet-runtime-architecture.md).

## Unresolved work and open questions

- 0.1.0 limitations recorded in the documents: no mTLS listener, no SSE resume, no pagination, `--timeout` not applied on the WebSocket duplex path, no MCP resources/resource templates or legacy HTTP+SSE transport, no persistent trace store. The full list is the [manual's limitations chapter](../manuals/man-2026-0001-rivet-manual.md#known-limitations).
- The `rivet.trace.export` dispatch defect and the missing MCP `tools/list` entries found at `829ca43` were fixed in commit `2a751ab` (INC-2026-0007).
- Re-verify every captured example when the wire contracts change.

## Related directories

[architecture/](../architecture/index.md) · [security/](../security/index.md) · [demos/](../demos/index.md) · [plans/](../plans/index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Created the API index with API-2026-0001 to API-2026-0005. |
| 2 | 2026-09-28 | Claude | Recorded the fix-batch updates of all five API documents (commits 829ca43 and 2a751ab); refreshed limitations. |
