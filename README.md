---
document_id: REF-2026-0003
title: "Rivet project status"
document_type: reference
status: active
created_date: 2026-09-27
last_updated: 2026-09-30
document_revision: 13
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, execution, cli, http, mcp, library, policy, ffi]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, embedded, server]
audience: [maintainers, developers, reviewers]
scope: Current Rivet runtime (release 0.2.0), source build, installation and developer workflows, and links to the 0.2.0 documentation.
reason: Record the project brief and requested changes as reviewable contracts and examples.
dependencies: [PROJECT.md, DOCUMENTATION.md, AGENTS.md]
related_documents: ["PROP-2026-0001", "PROP-2026-0002", "PLAN-2026-0002", "MIG-2026-0001", "API-2026-0006", "API-2026-0007", "MAN-2026-0009", "MAN-2026-0010", "INC-2026-0011", "STD-2026-0001", "REF-2026-0001", "REF-2026-0002"]
supersedes: null
superseded_by: null
tags: [rivet, rust, capy, design]
confidentiality: internal
review_cycle: on-design-change
next_review_date: 2026-10-27
---

# Rivet

Rivet is a Rust library and runtime for protocol-visible connections, scoped resources and composable DAG workflows, using Capy for parsing.

**Current release: 0.2.0** (tag `v0.2.0`, [REL-0.2.0](docs/releases/rel-0.2.0-release-notes.md); plan
[PLAN-2026-0002](docs/plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md)). Previous release: 0.1.0
([REL-0.1.0](docs/releases/rel-0.1.0-release-notes.md)). The repository contains the Rust library, the `rivet` binary, the C ABI
library `librivet`, editor support, conformance tests and runnable source-build workflows. **Supported platforms:
macOS and Linux** (CI green on both); Windows is not supported
([INC-2026-0011](docs/incidents/active/inc-2026-0011-windows-port-failures.md)).
Start with the [current manual](docs/manuals/man-2026-0001-rivet-manual.md) and
[installation guide](docs/manuals/man-2026-0002-installation-and-quickstart.md).

```text
 0.1.0 (previous release)                       0.2.0 (current release)
 ────────────────────────                       ───────────────────────
 per-surface JSON shapes ("result", …)   ──▶    one response envelope everywhere (breaking; MIG-2026-0001)
 {id, params} · --params                 ──▶    {operation, data} · --data · --input (old keys deprecated)
 one file per bundle                     ──▶    global constants · import "./x.rivet" as x · rt.load
 package rivet, all modules public       ──▶    package rivet-runtime, facade, Cargo features
 Rust only                               ──▶    + C ABI librivet (C, Python, Go)
 —                                       ──▶    rivet highlight · TextMate grammar · VS Code .vsix
```

| 0.2.0 document | What it covers |
|---|---|
| [What's New in 0.2.0](docs/manuals/man-2026-0001-rivet-manual.md#whats-new-in-020) | every addition, with instructions and demos |
| [MIG-2026-0001](docs/migrations/mig-2026-0001-response-and-input-envelopes.md) | migrating 0.1.0 clients to the envelopes; rollback |
| [API-2026-0006](docs/api/api-2026-0006-envelopes.md) | the response and input envelopes (+ JSON Schemas) |
| [API-2026-0007](docs/api/api-2026-0007-c-abi.md) · [MAN-2026-0009](docs/manuals/man-2026-0009-c-abi-and-ffi.md) | the C ABI and calling Rivet from C, Python and Go |
| [MAN-2026-0010](docs/manuals/man-2026-0010-editor-support-and-highlighting.md) | editor support and `rivet highlight` |
| [MAN-2026-0003 §Globals / §Modules](docs/manuals/man-2026-0003-language-guide.md#globals) | `global` constants and file modules |
| [MAN-2026-0007](docs/manuals/man-2026-0007-embedding-library.md) | the Cargo dependency, features, facade and `Runtime::call` / `load` |

- [Proposal: runtime, syntax, interfaces and security boundaries](docs/proposals/implemented/prop-2026-0001-rivet-runtime.md)
- [Sample folders: source files and usage READMEs](docs/demos/README.md) ([status and reading order](docs/demos/index.md))
- [Reference: numbered usage examples](docs/references/ref-2026-0002-language-and-usage.md)
- [Original request and inspected sources](docs/references/ref-2026-0001-request-and-evidence.md)
- [Original project brief](PROJECT.md)
- [Hand-authored design contract](vhco-contract.json)
- [Documentation current state](docs/README.md) and [navigation](docs/index.md)
- [Orchestrator and cross-package implementation standard](docs/standards/std-2026-0001-orchestrator-and-cross-package-review.md) ([STD-2026-0001](docs/standards/std-2026-0001-orchestrator-and-cross-package-review.md))

## Build, install and develop

With Perch on `PATH`, run these from the checkout root. Rust 1.90.0, rustfmt and Clippy are pinned in
`rust-toolchain.toml`; Python 3 and VHCO are needed for documentation and architecture checks.

```sh
perch --help           # list all commands and descriptions
perch build            # optimized binary: target/release/rivet (cargo build --release --features cli)
perch build_debug      # debug binary: target/debug/rivet
perch install          # build release, then bman add "<absolute binary path>"
perch tests            # cargo test --locked --workspace --all-targets --all-features
perch features         # the library with no default features and with each Cargo feature alone
perch gates            # formatting, type-check, lint, tests, docs and VHCO
```

Installation requires `bman` on `PATH` and runs `bman add` with the quoted absolute release-binary path.
Bman manages the global bin directory; `bman help` shows its location. The install task explicitly builds
into this checkout's `target/` so it always installs that artifact. Other builds respect Cargo configuration,
including `CARGO_TARGET_DIR`. The first build may fetch dependencies from the network.

The checkout is a Cargo workspace: package `rivet-runtime` (library `rivet`; the `rivet` binary needs the
`cli` Cargo feature) and `ffi/` (`rivet-ffi`). Default features are `serve`, `grpc`, `quic` and `oauth`.

Other commands: `check`, `clippy`, `ffi` (build `librivet`, verify `rivet.h`, run the C and Python examples),
`ffi_header`, `fmt`, `fmt_check`, `docs_check`, `architecture`, `help_cli`, `live`, `spec`, `docs`, `clean`, and
`main`. `perch main` prints a short task list; bare `perch` shows Perch usage.
`gates` stops at the first failure. `fmt` edits Rust files, `clean` removes Cargo build artifacts, and
`spec` / `docs` regenerate `vhco.json` / `vhco.html`. `live` runs until interrupted.

From another directory, select the file explicitly:

```sh
perch -f /path/to/rivet/commands.perch build
```

Every task runs from the command file's directory. See the
[complete command table](docs/onboarding/onb-2026-0001-contributor-setup.md#perch-development-commands).
Direct Cargo and VHCO invocations remain available.

Without Perch, install the binary from the git tag with Cargo (the `cli` feature is required for the binary):

```sh
cargo install rivet-runtime --git https://github.com/olivierdevelops/rivet --tag v0.2.0 --features cli   # once P5 tags v0.2.0
cargo build --release -p rivet-ffi              # librivet (C ABI): target/release/librivet.{a,dylib|so}
python3 editors/vscode/package_vsix.py          # the VS Code extension: dist/rivet-<version>.vsix
```

A Rust project depends on `rivet = { package = "rivet-runtime", git = "https://github.com/olivierdevelops/rivet", tag = "v0.2.0" }`
([MAN-2026-0007](docs/manuals/man-2026-0007-embedding-library.md)).

## Quickstart (verified)

Verified on 2026-09-29 (macOS) with the 0.2.0 release candidate (`cargo build --release --features cli`); **request
and trace IDs vary on every run**. With 0.1.0 use `--params` instead of `--data` and expect the 0.1.0 output shape.
Save this as `app.rivet`:

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
 $ rivet --file app.rivet request demo.add --data '{"a":2,"b":3}'
 {"request_id":"req_010bf5f2ed","trace_id":"tr_010bf5f2ed","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}      exit 0

 $ rivet --file app.rivet request notes.save --data '{"text":"hi"}'       # no policy.json yet (stderr)
 {"request_id":"req_010a217d8d",…,"operation":"notes.save","type":"result","status":"error","data":null,
  "error":{"kind":"permission","code":"permission.denied",
   "message":"allow_write create on ./out/note.json denied: no policy.json: allow_write is denied by default (add a grant for ./out/note.json)",…},
  "effects":"none","data_count":0}                                                                             exit 3

 $ rivet --file app.rivet io                                              # every I/O site, nothing runs
 OPERATION   KIND  ACCESS  TARGET           KNOWLEDGE  SOURCE
 notes.save  file  create  ./out/note.json  exact      app.rivet:13

 $ rivet --file app.rivet policy generate > policy.draft.json            # review, then adopt
 $ mv policy.draft.json policy.json
 $ rivet --file app.rivet io --check-policy
 OPERATION   KIND  ACCESS  TARGET           KNOWLEDGE  SOURCE        DECISION
 notes.save  file  create  ./out/note.json  exact      app.rivet:13  allowed
 1 allowed

 $ rivet --file app.rivet request notes.save --data '{"text":"hi"}'
 {"request_id":"req_010769d155","trace_id":"tr_010769d155","operation":"notes.save","type":"result","status":"ok","data":{"saved":true},"error":null,"effects":"committed","data_count":0}
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
      +------------+-----------+-------+-------+-----------+-----------+-------------+
      |            |           |               |           |           |             |
     CLI         REST/SSE    polling       WebSocket      MCP       Rust library   C ABI
   rivet ...   /v1/request  /v1/requests    /v1/ws        /mcp      Runtime::call  librivet
      |            |           |               |           |           |             |
      +------------+-----------+-------+-------+-----------+-----------+-------------+
                                       |
                    one dispatcher -> scoped resources -> brokered effects
                                       |
      input  {operation, data, deadline_ms?, restrict?}  ──  output {request_id, trace_id, operation, type,
                                                               status, data, error, effects, data_count}
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
| Operations | Any number of named, described operations per file, one immutable catalog shared by every surface. From 0.2.0, `global NAME = …` constants per file and `import "./x.rivet" as x [public]` modules (namespaced `x.ID`, one policy). |
| Envelopes (0.2.0) | Every surface accepts `{operation, data, …}` and answers one response envelope; `--pretty` / `?pretty=true` indent it ([API-2026-0006](docs/api/api-2026-0006-envelopes.md)). |
| Call syntax | Capy **prefix calls**: `(request "math.double" {value: a})`; durations are quoted strings (`timeout "10s"`). |
| Declared outputs | Each operation may declare a described output (`output integer description "..."`) or a structured `output object ... end` block of `field` lines, plus `error "code" description "..."` lines. The runtime validates the result against it (`output.invalid`, exit 5). |
| Viewing outputs | `rivet outputs ID [--json]`, `rivet outputs --all`, `rivet describe ID` (Output section), `rivet list --outputs`, `GET /v1/operations/{id}/outputs`, MCP built-in tool `rivet.outputs`, `Runtime::outputs(id)`. |
| Policy | Configured **only** by a `policy.json` file beside the entry `.rivet` file, or selected with `--policy PATH`. No grant strings on the command line. No file means deny-by-default for new application I/O; pure operations still run. |
| I/O manifest | `rivet io` lists every effect site with its target (URL, path, host:port, env var, connector method), access verbs (read, create, update, delete, connect, bind, call …) and the capability that permits it. It can be viewed `--by operation\|target\|capability`, filtered with `--kind`/`--access`, rendered as table/json/markdown/csv and checked with `--check-policy` (exit 3 on a denied site). No I/O is performed. Also available as `GET /v1/io`, MCP `rivet.io` and `rt.io`. |
| Policy draft | `rivet policy generate [ID ...]` turns the manifest into a least-privilege policy.json draft: one grant per (capability, target), narrowed with the new optional `access` list. It never grants dynamic/opaque sites (exit 7) and never overwrites a file. The draft is for human review, not approval. |
| Serving | One `rivet serve [--listen 127.0.0.1:8080]` mounts HTTP, SSE, polling, WebSocket and MCP on one listener; `rivet serve --stdio` serves MCP over stdio. One authentication -> principal -> operation authorization path for every surface. |
| Protocols | HTTP/1.1-3, WebSocket, TCP, UDP, QUIC, gRPC (all four modes), MCP client and server, OAuth 2.0 — all required scope. From 0.2.0, `grpc`, `quic` (and HTTP/3), `oauth` and `serve` are Cargo features (default on); a build without one refuses the bundle with `unsupported.feature`. |
| Embedding (0.2.0) | Rust: the `rivet-runtime` facade (`Runtime::call`, `load`, `Module`). C, Python, Go: `librivet` (`rivet_request`, call handles, `rivet_load`). |
| Highlighting (0.2.0) | `rivet highlight FILE --format ansi\|html\|json`, a generated TextMate grammar and a VS Code `.vsix` ([MAN-2026-0010](docs/manuals/man-2026-0010-editor-support-and-highlighting.md)). |
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
rivet --file app.rivet request demo.add --data '{"a":2,"b":3}'            # uses ./policy.json if present
rivet --file app.rivet --policy ./policies/ci.json request users.get --data '{"id":42}'
echo '{"operation":"demo.add","data":{"a":2,"b":3}}' | rivet --file app.rivet request --input -
rivet --file app.rivet request demo.add --data '{"a":2,"b":3}' --pretty
rivet highlight app.rivet --format html > app.html
rivet --file app.rivet outputs demo.add --json
rivet --file app.rivet io --by target --check-policy                          # what every URL/path is used for
rivet --file app.rivet policy generate > policy.draft.json                    # least-privilege draft to review
rivet --file app.rivet serve --listen 127.0.0.1:8080
```

Remote MCP effects and unconfined native/process code are identified as trust boundaries, not misrepresented as locally enforceable. Sandbox guarantees apply to script-initiated effects through brokered adapters; the fixed runtime bootstrap I/O list is shown by `rivet io --include-bootstrap`.

## Status and platform limits

The maintainer authorized the Capy dependency in [ADR-0001](docs/decisions/adr-0001-approve-rivet-runtime-design.md).
0.1.0 validation results are in [RPT-2026-0001](docs/reports/rpt-2026-0001-validation-of-plan-2026-0001.md). For
0.2.0, CI is green on **macOS and Linux**; Windows was dropped from CI and from the supported platforms, and its
thirteen failures are catalogued in [INC-2026-0011](docs/incidents/active/inc-2026-0011-windows-port-failures.md).

```text
 platform   build + tests   process sandbox                              status
 macOS      CI green        Seatbelt, active                             supported
 Linux      CI green        Landlock + seccomp, gated:                   supported
                            unsupported.sandbox_backend (exit 5 / 501)
 Windows    —               none                                         not supported (INC-2026-0011)
```

| Not in this version | What happens |
|---|---|
| mTLS serve authentication | `serve` refuses to start: `unsupported.serve_mtls` (exit 5); use bearer tokens |
| Windows | Not a supported platform (INC-2026-0011) |
| Process sandbox on Linux | Backend gated until verified on kernel ≥ 6.12: `unsupported.sandbox_backend` (exit 5) |
| URL imports, hot reload of modules | Imports are local files under the root (0.2.0); `rt.load` adds modules at run time |
| `finally` | `try … catch … end` only; `with` blocks always close their handles |
| Stage C forms (FIFO, file watch, TCP TLS, interactive processes, reconnect) | Typed `unsupported.*` (exit 5), no effects |
| Persistent trace store | Traces live for the process; `rivet trace export REQ --output PATH` saves one |

Details: [Known Limitations](docs/manuals/man-2026-0001-rivet-manual.md#known-limitations) and the
[protocol manual](docs/manuals/man-2026-0008-protocols-and-connectors.md). (0.1.0 had no `import`; 0.2.0 adds file
modules.)

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
| 13 | 2026-09-30 | Claude | v0.2.0 release (PLAN-2026-0002 TASK-091): version strings and current-release wording updated to 0.2.0. |
| 12 | 2026-09-29 | Claude | TASK-080 / D-49: current release 0.1.0 with 0.2.0 in progress (0.1.0 → 0.2.0 diagram and links to MIG-0001, API-0006/0007, MAN-0009/0010, globals/modules, facade); Cargo install with `--features cli`, librivet and `.vsix`; quickstart re-captured as 0.2.0 envelopes with `--data`; runtime overview adds the C ABI and envelopes; platforms macOS and Linux, Windows unsupported (INC-2026-0011), Linux sandbox gated; STD-2026-0001 and Perch content kept. |
| 11 | 2026-09-29 | Codex | Added the orchestrator and cross-package implementation standard. |
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
