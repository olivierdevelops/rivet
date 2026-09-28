---
document_id: REF-2026-0003
title: "Rivet project status"
document_type: reference
status: active
created_date: 2026-09-27
last_updated: 2026-09-28
document_revision: 10
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, execution, cli, http, mcp, library, policy]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, embedded, server]
audience: [maintainers, developers, reviewers]
scope: Current Rivet runtime, source build, installation and developer workflows; no published release claim.
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

Rivet is a Rust library and runtime for protocol-visible connections, scoped resources and composable DAG workflows, using Capy for parsing.

**Current state: implemented, unreleased (`0.1.0-dev`).** The repository contains the Rust library,
`rivet` binary, conformance tests and runnable source-build workflows. The approved design and implementation
plan are recorded in [ADR-0001](docs/decisions/adr-0001-approve-rivet-runtime-design.md) and
[PLAN-2026-0001](docs/plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md).
Start with the [current manual](docs/manuals/man-2026-0001-rivet-manual.md) and
[installation guide](docs/manuals/man-2026-0002-installation-and-quickstart.md).

- [Proposal: runtime, syntax, interfaces and security boundaries](docs/proposals/implemented/prop-2026-0001-rivet-runtime.md)
- [Sample folders: source files and usage READMEs](docs/demos/README.md) ([status and reading order](docs/demos/index.md))
- [Reference: numbered usage examples](docs/references/ref-2026-0002-language-and-usage.md)
- [Original request and inspected sources](docs/references/ref-2026-0001-request-and-evidence.md)
- [Original project brief](PROJECT.md)
- [Hand-authored design contract](vhco-contract.json)
- [Documentation current state](docs/README.md) and [navigation](docs/index.md)

## Build, install and develop

With Perch on `PATH`, run these from the checkout root. Rust 1.90.0, rustfmt and Clippy are pinned in
`rust-toolchain.toml`; Python 3 and VHCO are needed for documentation and architecture checks.

```sh
perch --help           # list all commands and descriptions
perch build            # optimized binary: target/release/rivet
perch build_debug      # debug binary: target/debug/rivet
perch install          # build release, then bman add "<absolute binary path>"
perch tests            # cargo test --locked --all-targets
perch gates            # formatting, type-check, lint, tests, docs and VHCO
```

Installation requires `bman` on `PATH` and runs `bman add` with the quoted absolute release-binary path.
Bman manages the global bin directory; `bman help` shows its location. The install task explicitly builds
into this checkout's `target/` so it always installs that artifact. Other builds respect Cargo configuration,
including `CARGO_TARGET_DIR`. The first build may fetch dependencies from the network.

Other commands: `check`, `clippy`, `fmt`, `fmt_check`, `docs_check`, `architecture`, `help_cli`, `live`,
`spec`, `docs`, `clean`, and `main`. `perch main` prints a short task list; bare `perch` shows Perch usage.
`gates` stops at the first failure. `fmt` edits Rust files, `clean` removes Cargo build artifacts, and
`spec` / `docs` regenerate `vhco.json` / `vhco.html`. `live` runs until interrupted.

From another directory, select the file explicitly:

```sh
perch -f /path/to/rivet/commands.perch build
```

Every task runs from the command file's directory. See the
[complete command table](docs/onboarding/onb-2026-0001-contributor-setup.md#perch-development-commands).
Direct Cargo and VHCO invocations remain available.

## Quickstart (verified)

The following was verified on macOS with the current build; request and trace IDs vary. Save this as `app.rivet`:

```rivet
operation demo.add
    description "Add two integers."
    param a integer required description "First addend."
    param b integer required description "Second addend."
    output integer description "The sum."
    return a + b
end

operation notes.save
    description "Save a note to ./out/note.json."
    param text text required description "The note."
    output json
    file create "./out/note.json" json {text: text}
    return {saved: true}
end
```

```text
 $ mkdir out
 $ rivet --file app.rivet request demo.add --params '{"a":2,"b":3}'
 {"request_id":"req_…","trace_id":"tr_…","result":5,"data_count":0,"effects":"none"}          exit 0

 $ rivet --file app.rivet request notes.save --params '{"text":"hi"}'     # no policy.json yet
 {… "error":{"kind":"permission","code":"permission.denied",
   "message":"allow_write create on ./out/note.json denied: no policy.json: …"}}           exit 3

 $ rivet --file app.rivet io                                              # every I/O site, nothing runs
 OPERATION   KIND  ACCESS  TARGET           KNOWLEDGE  SOURCE
 notes.save  file  create  ./out/note.json  exact      app.rivet:13

 $ rivet --file app.rivet policy generate > policy.draft.json            # review, then adopt
 $ mv policy.draft.json policy.json
 $ rivet --file app.rivet io --check-policy
 notes.save  file  create  ./out/note.json  exact  app.rivet:13  allowed

 $ rivet --file app.rivet request notes.save --params '{"text":"hi"}'
 {"request_id":"req_…","trace_id":"tr_…","result":{"saved":true},"data_count":0,"effects":"committed"}
```

```text
  no policy.json ──► deny-by-default (exit 3, effects none)
        │
  rivet io ──► rivet policy generate ──► human review ──► policy.json ──► request succeeds
```

Write the draft to a separate file. Redirecting straight into `policy.json` truncates it before Rivet reads it.

## Runtime overview

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

| Topic | Runtime interface |
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

Runtime command examples (provide your own `app.rivet` and policy):

```sh
rivet --file app.rivet request demo.add --params '{"a":2,"b":3}'          # uses ./policy.json if present
rivet --file app.rivet --policy ./policies/ci.json request users.get --params '{"id":42}'
rivet --file app.rivet outputs demo.add --json
rivet --file app.rivet io --by target --check-policy                          # what every URL/path is used for
rivet --file app.rivet policy generate > policy.draft.json                    # least-privilege draft to review
rivet --file app.rivet serve --listen 127.0.0.1:8080
```

Remote MCP effects and unconfined native/process code are identified as trust boundaries, not misrepresented as locally enforceable. Sandbox guarantees apply to script-initiated effects through brokered adapters; the fixed runtime bootstrap I/O list is shown by `rivet io --include-bootstrap`.

## Status and platform limits

The maintainer authorized the Capy dependency in [ADR-0001](docs/decisions/adr-0001-approve-rivet-runtime-design.md).
Validation results are in [RPT-2026-0001](docs/reports/rpt-2026-0001-validation-of-plan-2026-0001.md): every
requirement passes on macOS, and Linux and Windows runs wait on CI.

| Not in this version | What happens |
|---|---|
| mTLS serve authentication | `serve` refuses to start: `unsupported.serve_mtls` (exit 5); use bearer tokens |
| Process sandbox on Linux / Windows | Linux backend gated until verified on kernel ≥ 6.12; others unsupported: `unsupported.sandbox_backend` |
| Multi-file bundles (`import`) | One entry `.rivet` file per bundle |
| `finally` | `try … catch … end` only; `with` blocks always close their handles |
| Stage C forms (FIFO, file watch, TCP TLS, interactive processes, reconnect) | Typed `unsupported.*` (exit 5), no effects |
| Persistent trace store | Traces live for the process; `rivet trace export REQ --output PATH` saves one |

Details: [protocol manual](docs/manuals/man-2026-0008-protocols-and-connectors.md).

## Reviewing the contract

Review the contract with the installed VHCO tool:

```sh
vhco live vhco-contract.json --port 7787
```

Changes follow the human contract review defined by [AGENTS.md](AGENTS.md). Run `perch architecture` for
`vhco validate`, `sync` and `check`; run `perch gates` for the development checks. The Perch command set was
approved by the maintainer on 2026-09-28.

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 10 | 2026-09-28 | Claude | TASK-069: verified quickstart (deny-by-default → io → policy generate → allowed) and the current limitations table. |
| 9 | 2026-09-28 | Codex | Changed Perch installation to a release build followed by bman add, as requested by the maintainer. |
| 8 | 2026-09-28 | Codex | Added approved Perch build/install/development commands and corrected the obsolete pre-implementation status. |
| 7 | 2026-09-28 | Claude | Design approved (ADR-0001): design revision 8, 159 examples, G-LIC closed, PLAN-2026-0001 in P1. |
| 6 | 2026-09-28 | Claude | Revision 6 (UQ-18/R26): design revision 6; I/O manifest (`rivet io` targets, access verbs, views, `--check-policy`) and `rivet policy generate` rows, commands and diagram line. |
| 5 | 2026-09-28 | Claude | Revision 5 (UQ-17): declared outputs and `rivet outputs`, policy.json-only configuration, one `serve` for every surface, Capy prefix-call syntax, G-LIC licence gate and parser spike gate; example count left for the coordinator. |
| 4 | 2026-09-28 | Codex | Added twelve draft sample folders with source files, fixtures, request bodies and usage READMEs (UQ-16). |
| 3 | 2026-09-28 | Codex | Added gRPC, documented multi-operation catalogs, incoming MCP tools and duplex sessions; expanded reference to 120 examples. |
| 2 | 2026-09-28 | Codex | Added UDP, OAuth 2.0, QUIC/HTTP3 design coverage, traceability and 22 examples; updated current-state navigation. |
| 1 | 2026-09-27 | Codex | Added accurate design-phase entry point. |
