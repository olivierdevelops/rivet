---
document_id: REF-2026-0023
title: "Rivet system documentation"
document_type: reference
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 2
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, registry, execution, files, connectors, audit, policy, auth, datagrams, quic, grpc, sessions, serve, transports, cli, http, library, mcp, ws, poll]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, embedded, server]
audience: [maintainers, implementers, operators, reviewers]
scope: Navigation and status for the current-state system documentation of Rivet 0.1.0.
reason: AGENTS.md requires an index.md in every documentation directory; system/ was created for PLAN-2026-0001 deliverables D-15 to D-23.
dependencies: [DOCUMENTATION.md, AGENTS.md]
related_documents: [PLAN-2026-0001, PROP-2026-0001, SYS-2026-0001, SYS-2026-0002, SYS-2026-0003, SYS-2026-0004, SYS-2026-0005, SYS-2026-0006, SYS-2026-0007, SYS-2026-0008, SYS-2026-0009, REF-2026-0024, REF-2026-0030, REF-2026-0031, REF-2026-0032]
supersedes: null
superseded_by: null
tags: [rivet, system, index]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-28
---

# Rivet system documentation

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** every Rivet feature and surface

## Purpose

`system/` describes the **currently implemented** Rivet runtime (DOCUMENTATION §4.7, template §12.6): what each
part of the code does, where it lives, how it is configured, how it behaves at run time, and where its limits are.
Every document cites the source modules it describes and was checked against the `rivet` binary built from commit
`f40d4aa` (`0.1.0-dev`).

**Belongs here:** current-state component, runtime, integration and configuration documents (`SYS-…`).
**Does not belong here:** the intended design (`../proposals/`), decisions and their rationale (`../decisions/`),
task-oriented how-to material (manuals), numbered syntax examples (`../references/`), release history.

**Naming:** `sys-<YYYY>-<NNNN>-<short-description>.md`, ID `SYS-<YYYY>-<NNNN>`, placed in the sub-folder matching its
subject. When the code changes, the document is revised in place (Change History row + `document_revision`).

## Map of the system

```text
                        ┌──────────────────────── surfaces (SYS-2026-0004) ─────────────────────────┐
   rivet CLI ─┐         │  CLI · --endpoint remote client · Rust library · rivet serve:             │
   library  ──┼────────▶│  REST /v1/* · SSE · polling · WebSocket /v1/ws · MCP /mcp · MCP --stdio   │
   clients  ──┘         └───────────────┬────────────────────────────────────────┬──────────────────┘
                                        │ principal + operation listing          │ sessions
                                        ▼                                        ▼
   app.rivet ──▶ ┌──── compiler + catalog (SYS-0001) ────┐        ┌──── sessions (SYS-0007) ────┐
                 │ rivet.capy grammar → AST JSON →        │        │ open · send · finish ·      │
                 │ SyntaxTree → IR → registry / outputs  │        │ read events · cancel         │
                 └──────────────────┬────────────────────┘        └──────────────┬───────────────┘
                                    ▼                                            │
                 ┌──── execution, scopes, DAG (SYS-0002) ─────────────────────────┘
                 │ dispatcher · limits · interpreter · with-handles · dag · structured cancel ·
                 │ deadlines · secret taint · restrict stack
                 └──────────────────┬───────────────────────────────────────────────┐
                                    │ every effect attempt                          │
                                    ▼                                               │
   policy.json ─▶ ┌── policy broker + I/O manifest (SYS-0003) ──┐                   │
   (SYS-0008)     │ load_policy · authorize_effect · trace store │                   │
                  │ rivet io · policy explain|generate           │                   │
                  └──────────────────┬───────────────────────────┘                   │
                                     ▼ allowed                                       │
      ┌──────────── protocol adapters (SYS-0005) ───────────┐   ┌─ OAuth (SYS-0006) ─┐ ┌─ MCP connectors (SYS-0009) ─┐
      │ HTTP/1.1·2·3 · SSE/JSONL · tcp/unix · websocket ·   │   │ client_credentials │ │ stdio / Streamable HTTP     │
      │ processes + sandbox · UDP · QUIC · gRPC · TLS       │   │ auth code + PKCE   │ │ reviewed snapshots · sync   │
      └─────────────────────────────────────────────────────┘   │ device code · store│ └─────────────────────────────┘
                                                                └────────────────────┘
```

## Active documents

| ID | Folder | Document | Components |
|---|---|---|---|
| SYS-2026-0001 | [components/](components/index.md) | [Compiler and operation catalog](components/sys-2026-0001-compiler-and-catalog.md) | language, registry |
| SYS-2026-0002 | [runtime/](runtime/index.md) | [Execution, scopes and DAG](runtime/sys-2026-0002-execution-scopes-and-dag.md) | execution, files |
| SYS-2026-0003 | [components/](components/index.md) | [Policy broker and I/O manifest](components/sys-2026-0003-policy-broker-and-io-manifest.md) | policy, audit |
| SYS-2026-0004 | [components/](components/index.md) | [Surfaces and serve](components/sys-2026-0004-surfaces-and-serve.md) | cli, library, serve, http, ws, poll, mcp |
| SYS-2026-0005 | [integrations/](integrations/index.md) | [Protocol adapters](integrations/sys-2026-0005-protocol-adapters.md) | transports, datagrams, quic, grpc, http |
| SYS-2026-0006 | [integrations/](integrations/index.md) | [OAuth 2.0 and credentials](integrations/sys-2026-0006-oauth-and-credentials.md) | auth |
| SYS-2026-0007 | [runtime/](runtime/index.md) | [Duplex sessions](runtime/sys-2026-0007-sessions.md) | sessions |
| SYS-2026-0008 | [configuration/](configuration/index.md) | [policy.json schema v1 reference](configuration/sys-2026-0008-policy-json-reference.md) | policy, serve, connectors, auth |
| SYS-2026-0009 | [integrations/](integrations/index.md) | [MCP client connectors](integrations/sys-2026-0009-mcp-client-connectors.md) | connectors, mcp |

Sub-folder indexes: [components/](components/index.md) (REF-2026-0024), [runtime/](runtime/index.md)
(REF-2026-0030), [integrations/](integrations/index.md) (REF-2026-0031), [configuration/](configuration/index.md)
(REF-2026-0032).

## Recommended reading order

```text
  1 SYS-0001 compiler ─▶ 2 SYS-0002 execution ─▶ 3 SYS-0003 policy broker ─▶ 4 SYS-0008 policy.json
        │                                                                          │
        └──────────────▶ 5 SYS-0004 surfaces ─▶ 6 SYS-0007 sessions               │
                                                                                  ▼
                       7 SYS-0005 protocol adapters ─▶ 8 SYS-0006 OAuth ─▶ 9 SYS-0009 MCP connectors
```

A reader who only operates a server needs SYS-0004 and SYS-0008; a reader extending a protocol starts at
SYS-0003 (every effect passes the broker) and then SYS-0005.

## Recently added or updated

- 2026-09-28: all nine documents and the five indexes created for PLAN-2026-0001 phase P4 (D-15 to D-23),
  verified against `0.1.0-dev` commit `f40d4aa`.
- 2026-09-28: all nine documents revised (revision 2) for the post-P3 fix batch and re-verified at commit
  `829ca43` (TASK-092 code ≈ docs drift check): `else`, `check.unknown_function` and warnings (SYS-0001);
  structured cancellation, DAG timestamps, `emits` validation, `with file open`, locked compare-and-replace,
  secret taint (SYS-0002); host ceiling, `restrict`, `explain --params`, `graph`, trace export, multicast manifest
  (SYS-0003); health, access log, drain, `traceparent`, WS lanes, MCP built-ins (SYS-0004); 8 MiB defaults
  (SYS-0005); token reuse and cache key (SYS-0006); sweeper, `deadline_ms`, cancel-wins, byte budget (SYS-0007);
  limit widths, `max_buffered_bytes` enforced (SYS-0008); drift check, sync order, opaque effects (SYS-0009).

## Deprecated, superseded or archived

None.

## Important relationships

- Design intent and rationale: [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md),
  [ADR-0002 crates](../decisions/adr-0002-rust-crate-selection.md),
  [ADR-0003 sandbox backends](../decisions/adr-0003-process-sandbox-backends.md).
- Delivery: [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) deliverables D-15 to D-23.
- Syntax examples: [REF-2026-0002](../references/ref-2026-0002-language-and-usage.md); runnable bundles:
  [demos](../demos/README.md).

## Unresolved work and open questions

0.1.0 known limitations (the complete list is the
[manual's Known Limitations](../manuals/man-2026-0001-rivet-manual.md#known-limitations)), recorded in the relevant
documents: no `import` form (single-file bundles), mTLS for `serve`, the gated Linux sandbox and unsupported
Windows/other OSes, `finally`, unused `approved.overlaps`, `*` matching `rivet.auth.*`, `--timeout` over the
WebSocket duplex path, MCP 401 without retry, Alt-Svc HTTP/3 discovery, connection pooling, a persistent trace
store, MCP resource templates and the legacy MCP HTTP+SSE transport, Stage C forms.

Drift found by TASK-092 at `829ca43` (the `rivet.trace.export` built-in had no dispatcher handler and MCP
`tools/list` omitted `rivet.capabilities` and `rivet.trace.export`) was fixed in commit `2a751ab` (INC-2026-0007).

`docs/system/deployment/` does not exist
yet: Rivet ships as a single binary and library with no deployment topology of its own.

## Related directories

[`../index.md`](../index.md) (root navigation) · [`../decisions/`](../decisions/index.md) ·
[`../proposals/`](../proposals/index.md) · [`../references/`](../references/index.md) ·
[`../demos/`](../demos/index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Created the system/ index for PLAN-2026-0001 D-15 to D-23. |
| 2 | 2026-09-28 | Claude | Recorded the fix-batch revision of all nine documents (TASK-092, commits 829ca43 and 2a751ab), the current limitations and the drift found and fixed. |
