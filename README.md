---
document_id: REF-2026-0003
title: "Rivet project status"
document_type: reference
status: draft
created_date: 2026-09-27
last_updated: 2026-09-28
document_revision: 7
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, execution, cli, http, mcp, library, policy]
affected_versions:
  from: not-applicable
  to: proposed-v0.1
applicable_environments: [development, embedded, server]
audience: [maintainers, developers, reviewers]
scope: Proposed Rivet behavior and design review; no implementation or release claim.
reason: Record the project brief and requested changes as reviewable contracts and examples.
dependencies: [PROJECT.md, DOCUMENTATION.md, AGENTS.md]
related_documents: ["PROP-2026-0001", "REF-2026-0001", "REF-2026-0002"]
supersedes: null
superseded_by: null
tags: [rivet, rust, capy, design]
confidentiality: internal
review_cycle: on-design-change
next_review_date: 2026-10-27
---

# Rivet

Rivet is a proposed Rust library and runtime for protocol-visible connections, scoped resources and composable DAG workflows, using Capy for parsing.

**Current state: design approved, implementation not started (design revision 8).** This repository contains the original brief, the approved proposal, the approved hand-authored contract, a reference of 159 numbered examples, twelve sample folders and the v0.1.0 plan. It has no runtime implementation, Cargo build, installed Rivet command or release. The maintainer approved the design, contract and plan on 2026-09-28 ([ADR-0001](docs/decisions/adr-0001-approve-rivet-runtime-design.md)); [PLAN-2026-0001](docs/plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) is in phase P1.

- [Proposal: runtime, syntax, interfaces and security boundaries](docs/proposals/approved/prop-2026-0001-rivet-runtime.md)
- [Sample folders: source files and usage READMEs](docs/demos/README.md) ([status and reading order](docs/demos/index.md))
- [Reference: numbered usage examples](docs/references/ref-2026-0002-language-and-usage.md)
- [Original request and inspected sources](docs/references/ref-2026-0001-request-and-evidence.md)
- [Original project brief](PROJECT.md)
- [Hand-authored design contract](vhco-contract.json)
- [Documentation current state](docs/README.md) and [navigation](docs/index.md)

## What the design proposes

```text
                       app.rivet  (many described operations per file)
                           |
             Capy Library::parse  ->  Rivet SyntaxTree  ->  immutable catalog
                           |                (inputs, declared outputs, errors, effects)
                           v
   policy.json ---> Policy (host ceiling ∩ policy.json ∩ per-request narrowing)
   (absent = deny-by-default)          |
                                       v
      +------------+-----------+-------+-------+-----------+-----------+
      |            |           |               |           |           |
     CLI         REST/SSE    polling       WebSocket      MCP       Rust library
   rivet ...   /v1/request  /v1/requests    /v1/ws        /mcp      Runtime::request
      |            |           |               |           |           |
      +------------+-----------+-------+-------+-----------+-----------+
                                       |
                    one dispatcher -> scoped resources -> brokered effects
```

Every effect in the catalog is also visible before anything runs:

```text
  app.rivet --parse/lower--> effect sites --normalize--> IoManifest --+--> rivet io --by operation|target|capability
                                  |                                   +--> rivet io --check-policy   (<-- policy.json)
                                  |                                   +--> rivet policy generate --> draft policy.json
                                  +--effect_id--> runtime trace ------+--> rivet io --trace REQ    (planned vs actual)
```

| Topic | Current design (revision 6) |
|---|---|
| Operations | Any number of named, described operations per file, one immutable catalog shared by every surface. |
| Call syntax | Capy **prefix calls**: `(request "math.double" {value: a})`; durations are quoted strings (`timeout "10s"`). |
| Declared outputs | Each operation may declare a described output (`output integer description "..."`) or a structured `output object ... end` block of `field` lines, plus `error "code" description "..."` lines. The runtime validates the result against it (`output.invalid`, exit 5). |
| Viewing outputs | `rivet outputs ID [--json]`, `rivet outputs --all`, `rivet describe ID` (Output section), `rivet list --outputs`, `GET /v1/operations/{id}/outputs`, MCP built-in tool `rivet.outputs`, `Runtime::outputs(id)`. |
| Policy | Configured **only** by a `policy.json` file beside the entry `.rivet` file, or selected with `--policy PATH`. No grant strings on the command line. No file means deny-by-default for new application I/O; pure operations still run. |
| I/O manifest | `rivet io` lists every effect site with its target (URL, path, host:port, env var, connector method), access verbs (read, create, update, delete, connect, bind, call …) and the capability that permits it. It can be viewed `--by operation\|target\|capability`, filtered with `--kind`/`--access`, rendered as table/json/markdown/csv and checked with `--check-policy` (exit 3 on a denied site). No I/O is performed. Also available as `GET /v1/io`, MCP `rivet.io` and `rt.io`. |
| Policy draft | `rivet policy generate [ID ...]` turns the manifest into a least-privilege policy.json draft: one grant per (capability, target), narrowed with the new optional `access` list. It never grants dynamic/opaque sites (exit 7) and never overwrites a file. The draft is for human review, not approval. |
| Serving | One `rivet serve [--listen 127.0.0.1:8080]` mounts HTTP, SSE, polling, WebSocket and MCP on one listener; `rivet serve --stdio` serves MCP over stdio. One authentication -> principal -> operation authorization path for every surface. |
| Protocols | HTTP/1.1-3, WebSocket, TCP, UDP, QUIC, gRPC (all four modes), MCP client and server, OAuth 2.0 — all required scope. |
| Errors | One registry: code -> kind -> HTTP status -> exit code -> retryable. |

```text
  output object description "The requested user."          rivet outputs users.get
      field id    integer required description "..."   ==>   users.get — Get a user
      field name  text    required description "..."          output  object  The requested user.
      field email text    optional description "..."            id     integer  required  ...
  end                                                           name   text     required  ...
  error "users.not_found" description "No user has this ID."  errors
                                                                users.not_found  No user has this ID.
```

Canonical commands (proposed, not runnable yet):

```sh
rivet --file app.rivet request demo.add --params '{"a":2,"b":3}'          # uses ./policy.json if present
rivet --file app.rivet --policy ./policies/ci.json request users.get --params '{"id":42}'
rivet --file app.rivet outputs demo.add --json
rivet --file app.rivet io --by target --check-policy                          # what every URL/path is used for
rivet --file app.rivet policy generate > policy.draft.json                    # least-privilege draft to review
rivet --file app.rivet serve --listen 127.0.0.1:8080
```

Remote MCP effects and unconfined native/process code are identified as trust boundaries, not misrepresented as locally enforceable. Sandbox guarantees apply to script-initiated effects through brokered adapters; the fixed runtime bootstrap I/O list is shown by `rivet io --include-bootstrap`.

## Approval gates and risks

```text
  design review ✔ ──> [G-LIC] ✔ owner authorized Capy ──> [G-SPIKE] every example parses ──> implementation
  (ADR-0001)            (ADR-0001)                          gate: S01–S159 and docs/demos/*.rivet
```

| Gate / risk | State |
|---|---|
| G-LIC — Capy licence | **Closed 2026-09-28.** Capy's LICENSE text is source-available while its Cargo.toml says MIT; the maintainer owns Capy and authorized Rivet to depend on and ship it ([ADR-0001](docs/decisions/adr-0001-approve-rivet-runtime-design.md)). |
| Parser spike | Implementation starts only after every numbered example and every `docs/demos` `.rivet` file parses cleanly with Capy's public `Library::parse`. |
| Platform enforcement | Process sandboxing and conditional file updates depend on platform support; still unproven. |

## Reviewing the contract

Review the contract with the installed VHCO tool:

```sh
vhco live vhco-contract.json --port 7787
```

Implementation requires the human contract review defined by [AGENTS.md](AGENTS.md). `vhco validate`, `sync` and `check` cannot pass a Rust implementation gate before `src/` exists; they currently report that missing directory. Do not add dummy code or weaken validation to conceal this design-phase state. The examples describe proposed usage and require implementation plus fixture tests before they can be called runnable.

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 7 | 2026-09-28 | Claude | Design approved (ADR-0001): design revision 8, 159 examples, G-LIC closed, PLAN-2026-0001 in P1. |
| 6 | 2026-09-28 | Claude | Revision 6 (UQ-18/R26): design revision 6; I/O manifest (`rivet io` targets, access verbs, views, `--check-policy`) and `rivet policy generate` rows, commands and diagram line. |
| 5 | 2026-09-28 | Claude | Revision 5 (UQ-17): declared outputs and `rivet outputs`, policy.json-only configuration, one `serve` for every surface, Capy prefix-call syntax, G-LIC licence gate and parser spike gate; example count left for the coordinator. |
| 4 | 2026-09-28 | Codex | Added twelve draft sample folders with source files, fixtures, request bodies and usage READMEs (UQ-16). |
| 3 | 2026-09-28 | Codex | Added gRPC, documented multi-operation catalogs, incoming MCP tools and duplex sessions; expanded reference to 120 examples. |
| 2 | 2026-09-28 | Codex | Added UDP, OAuth 2.0, QUIC/HTTP3 design coverage, traceability and 22 examples; updated current-state navigation. |
| 1 | 2026-09-27 | Codex | Added accurate design-phase entry point. |
