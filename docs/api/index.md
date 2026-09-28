---
document_id: REF-2026-0020
title: "Rivet API documentation index"
document_type: reference
status: active
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 4
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [http, poll, ws, mcp, library, serve, ffi]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, embedded, server]
audience: [developers, integrators, operators]
scope: Navigation and status page for docs/api — the current-state contracts of every Rivet access point and the shared error registry.
reason: AGENTS.md "Directory Indexes" requires an index for every documentation directory; created with the 0.1.0 API documents (PLAN-2026-0001 D-24 to D-28).
related_documents: [PLAN-2026-0001, PLAN-2026-0002, API-2026-0001, API-2026-0002, API-2026-0003, API-2026-0004, API-2026-0005, API-2026-0006, API-2026-0007, MIG-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, api, index]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-29
---

# Rivet API documentation index

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** http, poll, ws, mcp, library, serve, envelope, ffi

## Purpose

`docs/api/` holds the **current, implemented** contracts of Rivet's programmatic access points: routes, frames, tools, Rust items, request/response shapes, authentication and errors. Every example in these documents was captured from a real build. Design history stays in the [approved proposal](../proposals/implemented/prop-2026-0001-rivet-runtime.md); command-line usage belongs to the manuals; internal structure belongs to [architecture](../architecture/index.md).

```text
                    one bundle (app.rivet + policy.json, + imported modules)
                                   │
                           Runtime (one dispatcher)
     ┌───────────┬─────────────┬───┴─────────┬──────────────┬──────────────┬──────────────┐
     │ CLI       │ REST + SSE  │ polling     │ WebSocket    │ MCP          │ Rust library │ C ABI (librivet)
     │ (manual)  │ API-0001    │ API-0001    │ API-0002     │ API-0003     │ API-0004     │ API-0007
     └───────────┴─────────────┴─────────────┴──────────────┴──────────────┴──────────────┘
        every input and output: one envelope ── API-0006 (+ schemas/)
        every failure: one error registry ───── API-0005
```

## What belongs here

- Belongs: HTTP routes, WebSocket frames, MCP tools, library signatures, error codes, compatibility notes.
- Does not belong: CLI task walkthroughs (manuals), component internals (architecture), threat model (security), demos.

## Naming rules

`api-YYYY-NNNN-<slug>.md`, `document_type: api`, ID prefix `API`. One document per access point; the envelope reference and the error registry are shared. Machine-readable JSON Schemas live in [`schemas/`](schemas/response.schema.json) (`response`, `input`, `stream-record`).

## Active documents (recommended reading order)

| ID | Document | Covers |
|---|---|---|
| API-2026-0006 | [Response and input envelopes](api-2026-0006-envelopes.md) | The one output shape and one input shape of every surface; status/type values; error object; pretty; per-surface wrapping; deprecated `id`/`params`; schemas |
| API-2026-0001 | [HTTP REST, SSE and polling](api-2026-0001-http-rest-sse-polling.md) | `/v1/request` (envelopes, `?pretty=true`, `Deprecation`, `restrict`, `traceparent`), `/v1/operations…`, `/v1/io`, `/v1/policy/generate`, `/v1/health`, SSE framing, `/v1/requests…` sessions, auth, access log, drain |
| API-2026-0002 | [WebSocket rivet.v1](api-2026-0002-websocket-rivet-v1.md) | `/v1/ws` frames (`{type:"request",ref,operation,data,deadline_ms?}`, records with `ref`), refs, refusal records, limits, close semantics |
| API-2026-0003 | [MCP server tools](api-2026-0003-mcp-server-tools.md) | `/mcp` and `serve --stdio`: initialize, tools/list (`outputSchema` = envelope), `structuredContent` envelopes, `rivet.request {operation,data}`, session delivery |
| API-2026-0004 | [Rust library](api-2026-0004-rust-library.md) | `rivet-runtime` dependency and Cargo features, the facade, `Runtime::call`, `load`/`load_as`/`Module`, scopes, sessions, audit, highlight |
| API-2026-0007 | [C ABI (librivet)](api-2026-0007-c-abi.md) | The 18 `rivet_*` functions, ownership, threading, handle lifecycles, error envelopes, link commands |
| API-2026-0005 | [Error registry](api-2026-0005-error-registry.md) | 22 kinds → HTTP → exit → retryable; every emitted code with a table of the 19 codes added in 0.2.0 (input, globals, modules, features, FFI); `check` and deprecation warnings |

## Recently added or updated

2026-09-28 — all five documents created for the 0.1.0 release (PLAN-2026-0001 D-24 – D-28), verified against commit `f40d4aa`.

2026-09-28 (revision 2 of each) — updated for the post-P3 fix batch and re-verified against commit `829ca43`: `/v1/health`, `restrict`, `traceparent`, access log and SIGTERM drain; WebSocket lanes and specific refusal frames; MCP `tools/list` built-ins; library scopes, ceiling and `Policy` constructors; new error codes and `check` warnings.

2026-09-29 (revision 3) — 0.2.0: API-2026-0006 (envelopes) and API-2026-0007 (C ABI) added; API-2026-0001…0005 re-captured on the 0.2.0 release candidate (envelopes on every surface, new error codes, facade). Migration from 0.1.0: [MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md).

2026-09-29 (API-2026-0001…0005 revision 3, index revision 4) — envelope sweep finished (TASK-070): every example re-captured from the 0.2.0-rc (source `6f9943f`, ports 18950–18958), the library example recompiled through the facade.

## Deprecated, superseded or archived

The 0.1.0 input keys `id`/`params` and the CLI flag `--params` are deprecated aliases in 0.2.x and are removed in 0.3.0 (API-2026-0006).

## Important relationships

- Implements the surface requirements of [PROP-2026-0002](../proposals/approved/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md) (envelopes, facade, C ABI) under [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md), on top of [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md) under [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md).
- Security properties of these surfaces (serve auth, principal map, MCP trust boundary): [SEC-2026-0001](../security/sec-2026-0001-policy-and-sandbox-model.md).
- How the surfaces reach the shared dispatcher: [ARCH-2026-0001](../architecture/arch-2026-0001-rivet-runtime-architecture.md).

## Unresolved work and open questions

- Platforms: macOS and Linux; Windows is not supported in 0.2.0 ([INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)).
- Deviations found by the 0.2.0 sweep (reported, not fixed here): a WebSocket ref ended by a refused input gets a terminal record without `seq` and with `data_count: 0`; library `record()` terminal records have no `seq`; a rejected input envelope answers `operation: null` even when it named one.
- Limitations recorded in the documents: no mTLS listener, no SSE resume, no pagination, `--timeout` not applied on the WebSocket duplex path (the 0.2.0 server honours a frame's `deadline_ms`, but the remote CLI does not send it), no MCP resources/resource templates or legacy HTTP+SSE transport, no persistent trace store. The full list is the [manual's limitations chapter](../manuals/man-2026-0001-rivet-manual.md#known-limitations).
- The `rivet.trace.export` dispatch defect and the missing MCP `tools/list` entries found at `829ca43` were fixed in commit `2a751ab` (INC-2026-0007).
- Re-verify every captured example when the wire contracts change.

## Related directories

[migrations/](../migrations/index.md) · [architecture/](../architecture/index.md) · [security/](../security/index.md) · [demos/](../demos/index.md) · [plans/](../plans/index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Created the API index with API-2026-0001 to API-2026-0005. |
| 2 | 2026-09-28 | Claude | Recorded the fix-batch updates of all five API documents (commits 829ca43 and 2a751ab); refreshed limitations. |
| 3 | 2026-09-29 | Claude | 0.2.0: added API-2026-0006 and API-2026-0007 and the schemas; updated rows of API-2026-0001…0005 after the envelope sweep; diagram adds the C ABI. |
| 4 | 2026-09-29 | Claude | Envelope sweep finished (TASK-070): rows for API-2026-0002 (`deadline_ms`) and API-2026-0005 (19 new codes, deprecation warnings); sweep deviations; platform note. |
