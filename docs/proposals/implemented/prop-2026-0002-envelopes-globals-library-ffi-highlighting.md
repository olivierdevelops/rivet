---
document_id: PROP-2026-0002
title: "Standard envelopes, pretty JSON, global constants, embeddable library, C ABI and syntax highlighting"
document_type: proposal
status: implemented
created_date: 2026-09-28
last_updated: 2026-09-30
document_revision: 4
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, registry, execution, serve, cli, http, mcp, ws, poll, library]
affected_versions:
  from: "0.2.0"
  to: null
applicable_environments: [development, embedded, server]
audience: [maintainers, developers, reviewers]
scope: >-
  Design for Rivet 0.2.0: one output envelope and one input envelope on every surface, an opt-in pretty JSON
  format, immutable top-level global constants, Rivet as a dependency of other Rust projects, a C ABI shared and
  static library, syntax highlighting, and file modules (`import` in `.rivet`, and loading a file as an object
  of operations from Rust, C and Python). Non-goals: mutable or shared global state, tree-sitter grammars,
  editor marketplace publishing, language bindings beyond examples, and changing exit codes or HTTP statuses.
reason: The maintainer asked for these capabilities after v0.1.0 (UQ-01…UQ-08, 2026-09-28).
dependencies: [PROP-2026-0001, PLAN-2026-0001, DOCUMENTATION.md, AGENTS.md]
related_documents: [PROP-2026-0001, REL-0.1.0, ADR-0001, ADR-0002, API-2026-0001, API-2026-0004, MAN-2026-0003]
supersedes: null
superseded_by: null
tags: [rivet, envelope, json, globals, library, ffi, c-abi, syntax-highlighting, modules, import]
confidentiality: internal
review_cycle: on-design-change
next_review_date: 2026-10-28
---

# Standard envelopes, pretty JSON, global constants, embeddable library, C ABI and syntax highlighting

> **Revision 3 amendment (2026-09-28):** the approved scope now also includes file modules. They add UQ-09,
> G-08, R19–R24, UC-10, UC-11, C-14, C-15, F-19–F-24 and T-16–T-20 (ADR-0004 revision 2).

> **Status:** Implemented
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** language, registry, execution, serve, cli, http, mcp, ws, poll, library

## Summary

Rivet 0.1.0 works, but every surface speaks a slightly different JSON dialect. You send `{id, params}` and receive
`{result}` on success, a separate `{error}` object on failure and `{type, data}` on streams. Rivet can only be
embedded from Rust by pointing at its whole internal module tree. Non-Rust programs cannot embed it at all, and
`.rivet` files are plain text in every editor.

This proposal makes Rivet 0.2.0 regular and embeddable.

```text
                 ┌──────────────────────── one input envelope ────────────────────────┐
                 │  {"operation": "users.get", "data": {"id": 42}, "deadline_ms": …}   │
                 └────────────────────────────────────┬───────────────────────────────┘
  CLI ─┐  HTTP/SSE ─┐  polling ─┐  WebSocket ─┐  MCP ─┐  Rust crate ─┐  C ABI (.so/.dylib/.dll/.a) ─┐
       └────────────┴───────────┴─────────────┴───────┴──────────────┴─────────────────────────────┘
                                                     │
                                  one dispatcher (unchanged, v0.1.0)
                                                     │
                 ┌──────────────────────── one output envelope ───────────────────────┐
                 │ {"request_id","trace_id","operation","type":"result","status":"ok", │
                 │  "data": {...}, "error": null, "effects": "none", "data_count": 0}   │
                 └─────────────────────────────────────────────────────────────────────┘
                     compact by default · --pretty / ?pretty=true / options.pretty

  app.rivet:  global api = "https://api.example.com"      ← immutable, shared by every operation
  editors:    rivet.tmLanguage.json + VS Code extension    ← highlighting from the real keyword table
  terminal:   rivet highlight app.rivet                    ← exact tokens from the Capy parser
  modules:    import "./users.rivet" as users  ·  (users.get {id: 1})  ·  rt.load("./users.rivet")?.call("get", …)
```

## Decision Requested

Approve the following design as the scope of a new plan (PLAN-2026-0002) targeting **v0.2.0**:

1. **Output envelope** `{request_id, trace_id, operation, type, status, data, error, effects, data_count}` on
   every surface. `data` and `error` are always present, and exactly one of them is non-null. This is a
   **breaking change** to 0.1.0 output and is allowed under 0.x semantic versioning.
2. **Input envelope** `{operation, data, deadline_ms?, restrict?, stream?}` with no `error` field. The 0.1.0
   keys `id`/`params` stay accepted as deprecated aliases through 0.2.x and are removed in 0.3.0.
3. **Pretty JSON** as an opt-in format: `--pretty`, `?pretty=true`, `Response::to_json_pretty`,
   `{"pretty": true}` over FFI. It is refused for line-delimited streams.
4. **Global constants**: top-level `global NAME = EXPR`, evaluated once at load and read-only.
5. **Rust library**: package renamed to `rivet-runtime` while the library keeps the name `rivet`. A stable
   `rivet::` facade and Cargo features that trim dependencies are added. The CLI moves behind a `cli` feature.
6. **C ABI**: a new `rivet-ffi` workspace crate producing `librivet` as `cdylib` and `staticlib`, with a
   generated `rivet.h` and a JSON-in/JSON-out API.
7. **Syntax highlighting**: a generated TextMate grammar, a local VS Code extension, `rivet highlight`, and a
   `rivet::highlight` tokenizer.
8. **File modules** (revision 3): `import "PATH" as ALIAS` in `.rivet`, and `Runtime::load` / `rivet_load`
   return a module object whose operations are namespaced `ALIAS.ID`. Loaded modules run under the loader's
   policy only.

Approval does **not** authorize:
- publishing to crates.io, which needs the owner to publish Capy under a free name first (gate G-PUB);
- publishing to the VS Code Marketplace;
- configuring a git remote;
- writing code before `vhco-contract.json` is updated and reviewed (AGENTS contract-first rule).

## Original User Request

| ID | What Was Asked or Said | Source and Date | Interpretation Notes |
|---|---|---|---|
| UQ-01 | "now i need syntax highlighting" | Conversation, 2026-09-28 | For `.rivet` sources in editors, and in the terminal through the same token rules. |
| UQ-02 | "library" | Conversation, 2026-09-28; clarified: "i want to be able to import it in another rust project as a dep" | Rivet usable as a Cargo dependency with a stable API. |
| UQ-03 | "standardize output" | Conversation, 2026-09-28 | Covered by UQ-05; also applies to every `--json` output, not only `request`. |
| UQ-04 | "option for pretty output formatting for json" | Conversation, 2026-09-28 | Opt-in; compact stays the default so pipes and NDJSON stay one line per record. |
| UQ-05 | "i prefer output {"request_id", ..., error: {..}, "data": {...}, ...}" | Conversation, 2026-09-28 | Flat envelope with top-level `request_id`, `error` and `data`. The `...` is filled in by this proposal. |
| UQ-06 | "i prefer input {"operation", ..., "data": {...}, ...}" (the example also showed `error`) | Conversation, 2026-09-28; clarified: drop `error` from input | `operation` replaces `id`, and `data` replaces `params`. Input has no `error`. |
| UQ-07 | "also allow global vars for better reuse of vars" | Conversation, 2026-09-28; clarified: immutable constants | Top-level read-only values shared by the file's operations. |
| UQ-08 | "also so shared static library with c abi that can be imported through ffi" | Conversation, 2026-09-28 | Both a shared (`.so`/`.dylib`/`.dll`) and a static (`.a`/`.lib`) library with a C header. |
| UQ-09 | "also we need to be able to execute from path to load the file as an object with many operations" | Conversation, 2026-09-28; clarified: inside `.rivet` (import) and host APIs (Rust, C, Python); names namespaced by alias; loader's policy only | A file is a module: loading it by path gives an object whose members are its operations. CLI run-from-path was not selected. |

## Problem and Evidence

### Current User Journey

```text
 script author ──> writes app.rivet in an editor ──> no colours, keywords look like names ──> typos found only by `rivet check`
 API client    ──> POST /v1/request {"id","params"} ──> success {"result"} / failure {"error"} / stream {"type"}
                                                     └─> three parsers per surface, `result` vs `data` naming
 CLI user      ──> rivet request … --json ──> one long line ──> pipes through `jq .` to read it
 Rust host     ──> rivet = { git = … } ──> compiles clap + axum + tonic + quinn + keyring ──> imports internal
                                                     modules whose paths change between releases
 C/Python/Go   ──> no entry point ──> must spawn the `rivet` binary and parse stdout
 script author ──> copies the same base URL / page size into 12 operations ──> one edit misses one copy
```

| Problem | Affected Users | Evidence and Inline Source | Consequence | UQ IDs |
|---|---|---|---|---|
| P-01 No highlighting | Script authors | No grammar or editor extension in the repository; `.rivet` has 101 option keywords (`src/infra/rivet.capy`) | Harder reading and reviewing | UQ-01 |
| P-02 Output shapes differ by outcome and surface | API/CLI clients, MCP hosts | `Completion::to_json` emits `result`; errors use `error_envelope`; streams use `Envelope::to_json` with `type` (`src/domain/contracts.rs`) | Clients branch on shape before reading | UQ-03, UQ-05 |
| P-03 Input naming differs from the preferred vocabulary | Clients | HTTP body `{id, params}` (API-2026-0001); WS `{type:"request", ref, id, params}` (API-2026-0002) | Inconsistent with the requested envelope | UQ-06 |
| P-04 No readable JSON | CLI users, HTTP debuggers | Every JSON output is single-line; only snapshot, manifest and trace files use `to_string_pretty` | Extra tooling for humans | UQ-04 |
| P-05 Library is not a clean dependency | Rust hosts | `src/lib.rs` makes all five internal modules `pub`; there are no Cargo features, so every dependent builds clap and axum; the crate name `rivet` on crates.io is owned by another project; `capy-core` is a git dependency, and crates.io `capy-core` is a different project | No stable API, heavy builds, not publishable | UQ-02 |
| P-06 No C ABI | C, C++, Python, Go, Node, Swift hosts | No `cdylib`/`staticlib` target, no header | Must shell out to the binary | UQ-08 |
| P-07 Repetition across operations | Script authors | 0.1.0 has no top-level value declarations (the grammar's top-level forms are `operation`, `pipeline`, `connector`, `auth`) | Drift between copies; URLs only visible per operation | UQ-07 |
| P-08 Single-file bundles | Script authors, Rust/C/Python hosts | 0.1.0 has no import form (limitation L1 in MAN-2026-0001); a runtime is built from exactly one entry file; `io --include-bootstrap` prints a `(+ imports)` placeholder | Large catalogs live in one file; hosts cannot load several files as separate objects | UQ-09 |

## Goals and Non-Goals

| ID | Goal and Observable Outcome | Problems Solved | How It Solves Them | Success Signal |
|---|---|---|---|---|
| G-01 | Every JSON a surface produces for a request is one envelope shape | P-02 | One `ResponseEnvelope` domain type and one serializer used by every surface | Schema test passes on CLI, HTTP, SSE, polling, WS, MCP, library and FFI |
| G-02 | Every surface accepts one input shape | P-03 | One `InputEnvelope` parser shared by surfaces | Same input file works with `rivet request --input`, `curl` and FFI |
| G-03 | Readable JSON on request | P-04 | One formatter with a `pretty` switch | `--pretty` output is 2-space indented and key order is unchanged |
| G-04 | Reuse values across operations safely | P-07 | Immutable, load-time `global` constants | A URL is declared once, and `rivet io` shows exact targets |
| G-05 | Add Rivet to any Rust project with one Cargo line | P-05 | Facade API, Cargo features, `cli` opt-in, package rename | A fresh crate builds `use rivet::Runtime` with `default-features = false` |
| G-06 | Call Rivet from any language with a C FFI | P-06 | `librivet` cdylib + staticlib + `rivet.h` | C and Python examples run from the release artifacts |
| G-07 | Coloured `.rivet` sources | P-01 | Generated TextMate grammar and VS Code extension; `rivet highlight` | Every REF-2026-0002 example is tokenized with no unknown keywords |
| G-08 | Split catalogs across files and load a file as an object | P-08 | `import … as ALIAS`; `Runtime::load`/`rivet_load` return a module object; `ALIAS.ID` namespacing; one policy | `users.rivet` loaded as `users` exposes `users.get`; `rivet io` covers every loaded file |

Explicit non-goals and boundaries:
- no mutable globals and no state shared between requests (G-04 is read-only by design);
- no `global` read from the environment, files or network (those are effects and stay inside operations);
- no tree-sitter grammar (possible follow-up) and no Marketplace or Open VSX publishing;
- no official Python, Go or Node packages (only examples over the C ABI);
- no change to exit codes, HTTP status codes, error codes, policy semantics or the MCP JSON-RPC framing;
- no JSON colourization (open question Q-03);
- no remote or URL imports, package registry, hot reload or unload of modules, and no CLI run-from-path or
  shebang execution (not selected for UQ-09).

## Proposed User Journey

```text
 author ─> opens app.rivet in VS Code ─> keywords/strings/${…}/comments coloured ─> saves
   │                                                         │
   ├─> global api = "https://api.example.com" ─> used in 12 operations ─> rivet io shows exact URLs
   │          └─> global x = (request …)  ─> rivet check: check.global_not_constant (exit 2) ─> fix
   │
 client ─> {"operation":"users.get","data":{"id":42}} ─> any surface
   │            ├─> ok     ─> {"type":"result","status":"ok","data":{…},"error":null,…}
   │            ├─> error  ─> {"type":"result","status":"error","data":null,"error":{…},…}  (+ exit/HTTP status)
   │            └─> stream ─> {"type":"data","seq":1,"data":…} … then one {"type":"result",…}
   │
   ├─> --pretty / ?pretty=true ─> same envelope, indented
   │          └─> --pretty --stream ─> validation.usage (exit 2): NDJSON must stay one line
   │
 Rust host ─> rivet = { package = "rivet-runtime", …, default-features = false } ─> use rivet::Runtime
 C host    ─> #include <rivet.h> ─> rivet_runtime_new(opts) ─> rivet_request(rt, input) ─> envelope JSON ─> rivet_string_free
```

## Requirements

| ID | Requirement | Type | Source and Relevance | Acceptance Criteria | Goal IDs |
|---|---|---|---|---|---|
| R1 | Every request outcome on every surface is a `ResponseEnvelope` with keys, in order: `request_id`, `trace_id`, `operation`, `type`, `status`, `data`, `error`, `effects`, `data_count` (plus `seq` on stream records and `ref` on WS frames). `data` and `error` are always present, and exactly one is non-null (both null only for `status: accepted`) | API | UQ-03, UQ-05 | A JSON Schema test over CLI, HTTP, SSE, NDJSON, polling, WS, MCP `structuredContent`, library and FFI outputs | G-01 |
| R2 | `status` ∈ `ok`, `error`, `cancelled`, `accepted`; `type` ∈ `result` (terminal or unary) and `data` (stream item). Error objects keep `kind`, `code`, `message`, `retryable`, `details`, `source`, `hint`; `effects` moves to the top level | API | UQ-05; REF error registry (API-2026-0005) | Every error code in API-2026-0005 renders with status `error` and the same `kind`/`code` as 0.1.0 | G-01 |
| R3 | Every `--json` CLI output and every built-in (`rivet.list`, `rivet.describe`, `rivet.io` …) is also an envelope whose `data` is the payload | API / CLI | UQ-03 | `rivet list --json`, `rivet io --json` and the rest parse with the envelope schema | G-01 |
| R4 | Input is an `InputEnvelope` `{operation, data, deadline_ms?, restrict?, stream?}` on HTTP, polling, WS request frames, the MCP `rivet.request` tool, `rivet request --input FILE\|-`, the library and FFI. It has no `error` key; `data` defaults to `{}` | API | UQ-06 | A shared parser test, and the same input file accepted on every surface | G-02 |
| R5 | `id`/`params` are accepted as deprecated aliases in 0.2.x. Mixing a key with its alias is `validation.input_envelope` (exit 2, HTTP 422). Using an alias adds a `Deprecation: true` HTTP header, a CLI stderr warning and a trace note | compatibility | UQ-06; v0.1.0 clients (REL-0.1.0) | The legacy body still works in 0.2.x and is refused with a hint in 0.3.0 | G-02 |
| R6 | Pretty JSON is opt-in on the CLI (`--pretty`), HTTP (`?pretty=true`), the library (`to_json_pretty`) and FFI (`"pretty": true`), with 2-space indent and unchanged key order. With NDJSON (`--stream`) or SSE it is `validation.usage` (exit 2) or HTTP 400 `validation.pretty_stream` | CLI / API / UX | UQ-04 | Golden files; the stream refusal is tested | G-03 |
| R7 | `global NAME = EXPR` at top level. EXPR is a constant expression: literals, lists, objects, arithmetic, comparisons, `${…}` interpolation and pure built-ins over earlier globals. It is evaluated once at load and is read-only in every operation of the bundle | functional | UQ-07 | Language tests; values are identical across concurrent requests | G-04 |
| R8 | Global errors are reported by `rivet check` (exit 2) with spans: `syntax.global` (malformed), `check.global_not_constant` (effect, `request`, `env`, `secret`, param or local use), `check.global_forward_ref`, `check.global_duplicate`, `check.global_shadow` (a param, local, loop variable or binding reuses a global name), `check.global_assign` | functional / UX | UQ-07 | Each code has a negative test with the exact line and column | G-04 |
| R9 | Globals are resolved statically in the I/O manifest and the call graph: a target built only from literals and globals is `exact` | functional / security | UQ-07; R26 of PROP-2026-0001 | `rivet io` shows `https://api.example.com/users/{id}` as `exact`, and `policy generate` produces the exact grant | G-04 |
| R10 | The package is `rivet-runtime` and the library crate stays `rivet`, so `use rivet::…` is unchanged. A facade exports `Runtime`, `RuntimeBuilder`, `Policy`, `Value`, `InputEnvelope`, `ResponseEnvelope`, `Envelope` (stream), `Error`, `DataSink`, `highlight`. Internal modules become `pub(crate)` or `#[doc(hidden)]` | API / release | UQ-02; crates.io `rivet` owned by another publisher (checked 2026-09-28) | A new crate depending on the git tag compiles `examples/embed.rs`; `cargo doc` shows only the facade | G-05 |
| R11 | Cargo features: `serve`, `grpc`, `quic` (includes HTTP/3), `oauth`, `cli`. Default = `serve, grpc, quic, oauth`; the `rivet` binary has `required-features = ["cli"]`. A bundle using a feature that was compiled out fails with `unsupported.feature` (exit 5) naming the feature; `rivet.capabilities` lists compiled features | API / operations | UQ-02 | A feature-matrix build in CI; the `unsupported.feature` test | G-05 |
| R12 | crates.io publication is prepared but gated (G-PUB): the owner publishes Capy as `capy-lang`, then `rivet-runtime` switches from the git dependency. Until then a git dependency on the `v0.2.0` tag is the supported path | release | UQ-02; crates.io forbids git dependencies; `capy-core` is taken | `cargo publish --dry-run` passes after G-PUB | G-05 |
| R13 | `rivet-ffi` builds `librivet` as `cdylib` and `staticlib`, with a cbindgen-generated `include/rivet.h` and a `rivet.pc`. Exported symbols are prefixed `rivet_`; `rivet_abi_version()` returns the ABI major version | API / release | UQ-08 | `nm` shows only `rivet_*`; C example links both dynamically and statically on macOS and Linux | G-06 |
| R14 | The FFI is JSON-in/JSON-out over UTF-8 NUL-terminated strings. Every returned string is freed with `rivet_string_free`. Panics are caught and become an `internal.panic` envelope; null or invalid-UTF-8 pointers become `validation.ffi_argument`. The runtime handle is thread-safe; a call handle belongs to one thread at a time | security / API | UQ-08 | Tests with ASan/Miri-style fuzzing of null, bad UTF-8, double free guards and a panic probe | G-06 |
| R15 | FFI streaming, live input and cancellation use a pull handle: `rivet_call_start`, `rivet_call_next(timeout_ms)`, `rivet_call_send`, `rivet_call_finish_input`, `rivet_call_cancel`, `rivet_call_free`. No callbacks cross the boundary | API | UQ-08 | C and Python examples stream, send input and cancel | G-06 |
| R16 | One keyword table, derived from `rivet.capy`, generates `editors/rivet.tmLanguage.json`. A local VS Code extension (`editors/vscode`) associates `.rivet` files, defines `#` comments, brackets and `end`-based indentation, and is packaged as a `.vsix` | UX / docs | UQ-01 | Drift test: grammar keywords == `rivet.capy` literals; snapshot tests over all REF examples | G-07 |
| R17 | `rivet highlight FILE [--format ansi\|html\|json]` and `rivet::highlight::tokens(src)` emit tokens from real parser spans (keyword, option, type, effect, string, interpolation, number, comment, operation id, global, variable, operator) | CLI / API | UQ-01 | Golden outputs; every token span lies inside the source | G-07 |
| R18 | Docs follow: API-2026-0001…0005, MAN-2026-0003/0004/0007, a new FFI manual, a migration guide (MIG) for the envelope change, demos `14-globals`, `15-ffi`, `16-editor`, and all 0.1.0 demos re-verified on the new envelopes | docs / release | DOCUMENTATION §§29–31 | T-31 check_docs 0 problems; every demo executed | G-01…G-07 |
| R19 | Top-level `import "PATH" as ALIAS [public]` in `.rivet`, before the first declaration. PATH is a string literal relative to the importing file and must resolve inside the runtime root (no `..` escape, no symlinks); ALIAS is an identifier. Imported files are bootstrap reads and are listed by `rivet io --include-bootstrap` (replacing the `(+ imports)` placeholder) | functional / language | UQ-09 | `rivet check` compiles a bundle across files; every imported file appears as a bootstrap site | G-08 |
| R20 | A module's public operations are addressed `ALIAS.ID` and callable as `(ALIAS.ID {…})` or `(request "ALIAS.ID" {…})`. Its private operations, globals, connectors and auth profiles stay inside the module. Imported operations are internal to the importing bundle unless the import is marked `public`, which adds them to the catalog (CLI, HTTP, MCP, WS, polling, library, FFI). One file imported under two aliases compiles once. Nested imports namespace transitively (`a.b.get`). Effect paths inside a module resolve against the runtime root | functional | UQ-09 (namespaced by alias) | Visibility and namespacing tests; `rivet list` shows only `public` imports | G-08 |
| R21 | Import errors are reported at `rivet check`/load time with spans in the importing file: `syntax.import` (malformed or after a declaration), `not_found.import` (exit 4), `permission.import_outside_root` (exit 3), `check.import_cycle` (with the cycle path), `check.import_duplicate` (alias reused), `check.import_collision` (a namespaced ID collides with a local ID), `limit.imports` (more than 256 files or depth over 16) | functional / UX | UQ-09 | One negative test per code | G-08 |
| R22 | Rust host API: `Runtime::load(path)` and `Runtime::load_as(path, alias)` (default alias = file stem) return a `Module` with `alias()`, `operations()`, `describe(id)`, `outputs(id)`, `call(id, data) -> ResponseEnvelope`, `stream(id, data)` and `duplex(id, data)`. `Runtime::builder().build()` may start with no entry file. Loading swaps in a new immutable catalog snapshot; in-flight requests keep their snapshot. Loaded modules are public in the runtime's catalog under their alias | API | UQ-09 (host APIs) | Library tests: load two files, call both, concurrent load while requests run | G-08 |
| R23 | C ABI: `rivet_load(rt, path, alias_or_null, &module, &error_json)`, `rivet_module_operations(module)` (JSON list), `rivet_module_call(module, id, data_json)` (envelope), `rivet_module_call_start(module, id, data_json)` (a `RivetCall`), `rivet_module_free(module)`. The Python example wraps a module as an object whose attributes are operations (`users.get(id=42)`) | API | UQ-09 (host APIs) | C and Python module examples run | G-06, G-08 |
| R24 | Loaded modules run under the **loader's policy only** (the entry bundle's `policy.json`, `--policy`, or the host `policy`/`ceiling`). A `policy.json` beside an imported or loaded file is ignored with the warning `check.module_policy_ignored`. `rivet io`, `policy generate`, `policy explain` and `rivet graph` cover every loaded file, with module source spans (`users.rivet:12`) and namespaced operation IDs | security | UQ-09 (loader's policy only) | Manifest and policy tests across modules; the warning test | G-08 |

## Use Cases

### Use-Case Catalogue

| ID | User Outcome | Actor | Surface and Trigger | Preconditions / Environment | Inputs | Outputs / Visible Result | Negative and Error Paths | Goal IDs | Requirement IDs | Test IDs |
|---|---|---|---|---|---|---|---|---|---|---|
| UC-01 | Call an operation and read one envelope shape | CLI/API/MCP client | `rivet request`, `POST /v1/request`, MCP `tools/call`, WS `request` | Bundle loads | `InputEnvelope` | `ResponseEnvelope` status ok | Validation, permission, not_found, timeout → status error with the same exit/HTTP code | G-01, G-02 | R1–R4 | T-01, T-02, T-03 |
| UC-02 | Stream items and a terminal result | Client | `--stream`, SSE, WS, polling | Operation `emits` | `InputEnvelope` + `stream:true` | `type:data` records then one `type:result` | Consumer stop → `cancelled`; item invalid → status error | G-01 | R1, R2 | T-04 |
| UC-03 | Read pretty JSON | Human at a terminal | `--pretty`, `?pretty=true` | — | Any JSON output | Indented envelope | `--pretty --stream` → validation.usage | G-03 | R6 | T-05 |
| UC-04 | Declare and reuse globals | Script author | `app.rivet`, `rivet check` | — | `global` lines | Values usable in every operation; exact manifest targets | Non-constant, forward ref, shadow, assign, duplicate → check.* exit 2 | G-04 | R7–R9 | T-06, T-07, T-08 |
| UC-05 | Embed Rivet in another Rust project | Rust developer | `Cargo.toml` dependency | Git tag `v0.2.0` reachable (needs a remote) | Bundle + policy | `ResponseEnvelope` values | Feature compiled out → unsupported.feature | G-05 | R10–R12 | T-09, T-10 |
| UC-06 | Call Rivet from C/Python/Go | Non-Rust developer | `librivet` + `rivet.h` | Release artifacts | JSON options + inputs | Envelope JSON strings | Null/invalid pointers, panics, bad JSON → error envelopes; never a crash | G-06 | R13–R15 | T-11, T-12 |
| UC-07 | Edit `.rivet` with colours | Script author | VS Code (or any TextMate host) | Extension installed from `.vsix` | `.rivet` file | Coloured tokens, comment toggling, bracket matching | Unknown future keyword → drift test fails in CI, not in the editor | G-07 | R16 | T-13 |
| UC-08 | Highlight on terminal or HTML | Author, docs pipeline | `rivet highlight` / `rivet::highlight` | — | Source | ANSI, HTML or JSON tokens | Syntax error → tokens up to the error plus the diagnostic (exit 2) | G-07 | R17 | T-14 |
| UC-09 | Migrate a 0.1.0 client | Existing client | Any surface | 0.1.0 client code | `{id, params}` | Works in 0.2.x with deprecation signals | Both `id` and `operation` → validation.input_envelope | G-02 | R5 | T-15 |
| UC-10 | Split a catalog across files with `import` | Script author | `.rivet` + `rivet check`/`request` | Files under the runtime root | `import "./users.rivet" as users` | `users.*` callable inside the bundle; `public` imports on every surface; `rivet io` covers modules | Missing file, outside root, cycle, duplicate alias, collision, limits → typed errors at check | G-08 | R19–R21, R24 | T-16, T-17, T-20 |
| UC-11 | Load a file as an object from a host | Rust / C / Python developer | `rt.load(path)`, `rivet_load`, Python wrapper | Runtime with a policy | Path (+ optional alias) | A module object; `call`, `operations`, streams | Same load errors as envelopes/`Err`; calls denied by the loader's policy | G-06, G-08 | R22–R24 | T-18, T-19 |

### UC-01 — Call an operation (normal and error journeys)

1. The client sends an `InputEnvelope`. The surface parser checks its shape (R4, R5).
2. The dispatcher runs the operation exactly as in 0.1.0.
3. The outcome is converted once into a `ResponseEnvelope` (R1, R2) and written by the surface. The exit code and
   HTTP status still come from the error registry.

```text
 surface ──parse_input──▶ InputEnvelope ──▶ Runtime::dispatch ──▶ Outcome ──▶ ResponseEnvelope::from ──▶ format(compact|pretty) ──▶ bytes
    │                          │                                    │
    │ bad shape                │ policy / validation / runtime      └── exit code + HTTP status from the registry (unchanged)
    └─▶ validation.input_envelope (exit 2 / 422), itself returned as a ResponseEnvelope
```

#### CLI Contract

```text
$ rivet --file app.rivet request users.get --data '{"id":42}'
{"request_id":"req_01b7e64875","trace_id":"tr_01b7e64875","operation":"users.get","type":"result","status":"ok","data":{"id":42,"name":"Ada"},"error":null,"effects":"none","data_count":0}
$ echo $?
0

$ rivet --file app.rivet request users.get --data '{"id":0}' --pretty
{
  "request_id": "req_01b7e64876",
  "trace_id": "tr_01b7e64876",
  "operation": "users.get",
  "type": "result",
  "status": "error",
  "data": null,
  "error": {
    "kind": "validation",
    "code": "validation.min",
    "message": "`id` must be at least 1",
    "retryable": false,
    "details": {"field": "id", "min": 1},
    "source": {"file": "app.rivet", "line": 3, "column": 5, "end_line": 3, "end_column": 40}
  },
  "effects": "none",
  "data_count": 0
}
$ echo $?
2

$ echo '{"operation":"users.get","data":{"id":42}}' | rivet --file app.rivet request --input -
{"request_id":"req_…","trace_id":"tr_…","operation":"users.get","type":"result","status":"ok","data":{"id":42,"name":"Ada"},"error":null,"effects":"none","data_count":0}
```

`--params` stays an alias of `--data` in 0.2.x and prints `warning[deprecated.params]` on stderr.

#### HTTP Contract

```text
client                     rivet serve                       dispatcher
  | POST /v1/request?pretty=true |                                |
  | {"operation","data"}         |── parse_input ────────────────▶|
  |                              |◀── Outcome ─────────────────────|
  | 200 / 4xx / 5xx + envelope   |                                |
  |◀─────────────────────────────|                                |
```

```sh
curl -s http://127.0.0.1:8080/v1/request \
  -H 'content-type: application/json' \
  -d '{"operation":"users.get","data":{"id":42},"deadline_ms":10000}'
# 200
# {"request_id":"req_…","trace_id":"tr_…","operation":"users.get","type":"result","status":"ok",
#  "data":{"id":42,"name":"Ada"},"error":null,"effects":"none","data_count":0}

curl -s 'http://127.0.0.1:8080/v1/request?pretty=true' -H 'content-type: application/json' \
  -d '{"operation":"users.get","data":{"id":999}}'
# 502 (users.not_found maps to HTTP 502 as in 0.1.0)
# {
#   "request_id": "req_…", "trace_id": "tr_…", "operation": "users.get",
#   "type": "result", "status": "error", "data": null,
#   "error": {"kind": "application", "code": "users.not_found", "message": "…", "retryable": false},
#   "effects": "none", "data_count": 0
# }

curl -s http://127.0.0.1:8080/v1/request -H 'content-type: application/json' \
  -d '{"id":"users.get","params":{"id":42}}' -D - | grep -i deprecation
# Deprecation: true          (0.2.x only; the body is the normal envelope)

curl -s http://127.0.0.1:8080/v1/request -H 'content-type: application/json' \
  -d '{"operation":"users.get","id":"users.get"}'
# 422 {"…","status":"error","data":null,"error":{"code":"validation.input_envelope",
#      "message":"use `operation` or the deprecated `id`, not both"},…}
```

#### Streams, WebSocket, polling and MCP

```text
NDJSON / SSE (one record per line or event)
{"request_id":"req_…","trace_id":"tr_…","operation":"events.count","type":"data","seq":1,"data":1,"error":null}
{"request_id":"req_…","trace_id":"tr_…","operation":"events.count","type":"data","seq":2,"data":2,"error":null}
{"request_id":"req_…","trace_id":"tr_…","operation":"events.count","type":"result","seq":3,"status":"ok","data":{"count":2},"error":null,"effects":"none","data_count":2}

WebSocket (subprotocol rivet.v1): same records plus "ref"
→ {"type":"request","ref":"c1","operation":"events.count","data":{}}
← {"ref":"c1","request_id":"req_…","trace_id":"tr_…","operation":"events.count","type":"data","seq":1,"data":1,"error":null}
→ {"type":"input","ref":"c2","seq":1,"data":{"text":"hi"}}          (input frames already use `data`)

Polling: POST /v1/requests {"operation","data"} → 202
{"request_id":"req_…","trace_id":"tr_…","operation":"events.count","type":"result","status":"accepted",
 "data":{"session_id":"ses_…","events_url":"/v1/requests/ses_…/events","next_send_seq":1},"error":null,"effects":"none","data_count":0}
GET …/events → {"events":[<records above>],"terminal":true}

MCP tools/call: the JSON-RPC framing is unchanged; the tool result carries the envelope
{"content":[{"type":"text","text":"<envelope JSON>"}],"structuredContent":<envelope>,"isError":<status == "error">}
rivet.request tool arguments: {"operation":"users.get","data":{"id":42}}
```

### UC-04 — Global constants

```rivet
# app.rivet (illustrative)
global api        = "https://api.example.com"
global users_url  = "${api}/users"
global page_size  = 50
global retry_on   = [429, 503]
global headers    = {accept: "application/json"}

operation users.get
    param id integer required min 1
    output json
    r = http get "${users_url}/${id}"
        header "accept" headers.accept
        decode json
    end
    return r.body
end

operation users.page
    param page integer default 1
    output json
    r = http get "${users_url}?limit=${page_size}&page=${page}"
        decode json
    end
    return r.body
end
```

```text
 load ──▶ parse ──▶ lower globals in order ──▶ evaluate (pure, no effects) ──▶ frozen Arc<GlobalScope>
                         │                        │
                         │                        └─ any effect/request/env/secret/param ──▶ check.global_not_constant
                         └─ name used before its line ──▶ check.global_forward_ref

 request ──▶ Frame { globals: Arc<GlobalScope> (read-only), scopes: [params, locals…] }
                lookup order: locals → params → globals ; assignment to a global name ──▶ check.global_assign (compile time)
```

```text
$ rivet --file app.rivet io --by target
TARGET                                   KIND     ACCESS       KNOWLEDGE  OPERATIONS
https://api.example.com/users/{id}       network  connect GET  exact      users.get
https://api.example.com/users?limit=50…  network  connect GET  exact      users.page

$ rivet --file bad.rivet check
error[check.global_not_constant]: global `token` cannot read the environment; globals are fixed at load time
  --> bad.rivet:1:16
   |
  1| global token = (env "API_TOKEN")
   |                ^^^^^^^^^^^^^^^^^
  = hint: declare `secret token from env "API_TOKEN" for "https://…"` inside the operation that uses it
$ echo $?
2
```

### UC-05 — Rust dependency

```toml
# another-project/Cargo.toml (illustrative; git until G-PUB, then crates.io)
[dependencies]
rivet = { package = "rivet-runtime", git = "https://github.com/olivierdevelops/rivet", tag = "v0.2.0",
          default-features = false, features = ["serve"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

```rust
// illustrative — the approved API is the facade listed in R10
use rivet::{InputEnvelope, Policy, Runtime};

#[tokio::main]
async fn main() -> Result<(), rivet::Error> {
    let rt = Runtime::builder()
        .file("app.rivet")
        .policy(Policy::from_file("policy.json")?)
        .build()?;
    let out = rt.call(InputEnvelope::new("users.get").data(serde_json::json!({"id": 42}))).await;
    println!("{}", out.to_json_pretty());           // the same envelope as every other surface
    if out.status.is_ok() { /* out.data */ } else { /* out.error */ }
    Ok(())
}
```

`Runtime::call` always returns a `ResponseEnvelope` (errors included). The existing
`Runtime::request(id, data, sink) -> Result<Completion, Error>` stays for Rust-idiomatic `?` use, and `Completion`
converts with `ResponseEnvelope::from`.

### UC-06 — C ABI

```c
/* illustrative; the header is generated by cbindgen from rivet-ffi */
#include <stdio.h>
#include <rivet.h>

int main(void) {
    RivetRuntime *rt = NULL;
    char *err = NULL;
    if (rivet_runtime_new("{\"file\":\"app.rivet\",\"policy_file\":\"policy.json\"}", &rt, &err) != RIVET_OK) {
        fprintf(stderr, "%s\n", err);              /* a ResponseEnvelope with status "error" */
        rivet_string_free(err);
        return 1;
    }
    char *out = rivet_request(rt, "{\"operation\":\"users.get\",\"data\":{\"id\":42},\"pretty\":true}");
    puts(out);                                      /* the envelope, pretty-printed */
    rivet_string_free(out);

    RivetCall *call = rivet_call_start(rt, "{\"operation\":\"events.count\",\"data\":{}}");
    char *rec;
    while ((rec = rivet_call_next(call, 5000)) != NULL) {   /* NULL after the terminal record */
        puts(rec);                                  /* {"type":"data",…} … {"type":"result",…} */
        rivet_string_free(rec);
    }
    rivet_call_free(call);
    rivet_runtime_free(rt);
    return 0;
}
```

```text
$ cc demo.c -I include -L target/release -lrivet -o demo                          # shared
$ cc demo.c -I include target/release/librivet.a $(pkg-config --libs --static rivet) -o demo   # static
$ ./demo
{
  "request_id": "req_…", … "status": "ok", "data": {"id": 42, "name": "Ada"}, "error": null, …
}
```

```python
# illustrative ctypes use (examples only; no official package)
import ctypes, json
lib = ctypes.CDLL("librivet.dylib")
lib.rivet_request.restype = ctypes.c_void_p
rt = ctypes.c_void_p(); err = ctypes.c_void_p()
lib.rivet_runtime_new(b'{"file":"app.rivet"}', ctypes.byref(rt), ctypes.byref(err))
p = lib.rivet_request(rt, json.dumps({"operation": "demo.add", "data": {"a": 2, "b": 3}}).encode())
print(json.loads(ctypes.string_at(p)))   # {'request_id': …, 'status': 'ok', 'data': 5, 'error': None, …}
lib.rivet_string_free(ctypes.c_void_p(p)); lib.rivet_runtime_free(rt)
```

```text
 C thread ──rivet_request──▶ [catch_unwind] ──▶ parse UTF-8 JSON ──▶ tokio runtime (owned by RivetRuntime)
    ▲                                                                   │
    └──── malloc'd UTF-8 envelope ◀── ResponseEnvelope::to_json ◀──────┘   (caller frees with rivet_string_free)

 RivetCall state machine
   started ──next()──▶ data … ──next()──▶ result ──next()──▶ NULL (done) ──free
      │  send()/finish_input() while running        │ cancel() ─▶ result{status:"cancelled"}
      └──────────── free() while running ─▶ cancel + bounded cleanup (5 s grace) ─▶ freed
```

### UC-07 / UC-08 — Syntax highlighting

```text
 src/infra/rivet.capy ──(build step: extract literals + token classes)──▶ editors/keywords.json
        │                                                                    │
        │                                                    ┌───────────────┴──────────────┐
        ▼                                                    ▼                              ▼
  Capy parser spans ──▶ rivet::highlight::tokens      editors/rivet.tmLanguage.json   editors/vscode/ (.vsix)
        │                     │                         (VS Code, Sublime, JetBrains    language-configuration.json
        ▼                     ▼                          TextMate bundles)              (# comments, brackets, end-indent)
  rivet highlight --format ansi|html|json
```

```text
 VS Code — app.rivet
 +---------------------------------------------------------------+
 | global api = "https://api.example.com"   # base URL           |
 | ^kw    ^glob ^string                       ^comment           |
 | operation users.get                                           |
 | ^kw       ^operation-id                                       |
 |     param id integer required min 1                           |
 |     ^opt  ^var ^type  ^opt     ^opt ^num                      |
 |     r = http get "${api}/users/${id}"                         |
 |             ^effect   ^interp        ^interp                  |
 | end                                                           |
 +---------------------------------------------------------------+
```

```text
$ rivet highlight app.rivet --format json | head -3
{"line":1,"col":1,"len":6,"class":"keyword","text":"global"}
{"line":1,"col":8,"len":3,"class":"global","text":"api"}
{"line":1,"col":14,"len":25,"class":"string","text":"\"https://api.example.com\""}
$ rivet highlight app.rivet --format html > app.html      # <span class="rv-keyword">…</span>
$ code --install-extension editors/vscode/rivet-0.2.0.vsix
```

### UC-10 — `import` inside `.rivet` (revision 3)

```rivet
# users.rivet (a module: its IDs are short; the importer namespaces them)
global api = "https://api.example.com"

operation get
    param id integer required min 1
    output json
    r = http get "${api}/users/${id}"
        decode json
    end
    return r.body
end

operation list
    output json
    return (get {id: 1})              # calls inside a module use the module's own IDs
end
```

```rivet
# app.rivet (the entry bundle)
import "./users.rivet" as users            # internal to this bundle
import "./billing.rivet" as billing public # also listed on every surface as billing.*

operation report.user
    param id integer required
    output json
    u = (users.get {id: id})                # same as (request "users.get" {id: id})
    inv = (billing.invoices {user: id})
    return {user: u, invoices: inv}
end
```

```text
 app.rivet ──import──▶ users.rivet ──(its own imports…)
     │         └─────▶ billing.rivet (public)
     ▼
 resolve (relative to importing file, inside runtime root) ─▶ compile each file once ─▶ namespace IDs by alias
     ─▶ one immutable catalog: report.user · users.get* · users.list* · billing.invoices · billing.pay
                                   (* internal: callable from app.rivet, not listed on surfaces)
     ─▶ one policy (the loader's) ─▶ one manifest covering every file
```

```text
$ rivet --file app.rivet list
ID                NAME              DESCRIPTION
billing.invoices  billing.invoices  …
billing.pay       billing.pay       …
report.user       report.user       …

$ rivet --file app.rivet io --by target
TARGET                              KIND     ACCESS       KNOWLEDGE  OPERATIONS
https://api.example.com/users/{id}  network  connect GET  exact      users.get (users.rivet:6)

$ rivet --file app.rivet io --include-bootstrap | grep bootstrap
bootstrap  file  read  ./app.rivet      exact  load
bootstrap  file  read  ./users.rivet    exact  load
bootstrap  file  read  ./billing.rivet  exact  load

$ rivet --file cyc.rivet check
error[check.import_cycle]: import cycle: cyc.rivet → a.rivet → cyc.rivet
  --> cyc.rivet:1:1
$ echo $?
2
```

### UC-11 — Load a file as an object from a host (revision 3)

```rust
// illustrative
use rivet::{Policy, Runtime};
let rt = Runtime::builder().policy(Policy::from_file("policy.json")?).root(".").build()?;   // no entry file needed
let users = rt.load("./users.rivet")?;                  // alias "users" (file stem)
for op in users.operations() { println!("{} — {:?}", op.id, op.description); }   // get, list
let out = users.call("get", serde_json::json!({"id": 42})).await;               // ResponseEnvelope
assert_eq!(out.operation, "users.get");
let billing = rt.load_as("./lib/billing.rivet", "billing")?;
let same = rt.call(rivet::InputEnvelope::new("users.get").data(serde_json::json!({"id": 42}))).await;
```

```c
/* illustrative */
RivetModule *users = NULL; char *err = NULL;
if (rivet_load(rt, "./users.rivet", NULL, &users, &err) != RIVET_OK) { puts(err); rivet_string_free(err); }
char *ops = rivet_module_operations(users);            /* ["get","list"] with descriptions, as JSON */
char *out = rivet_module_call(users, "get", "{\"id\":42}");   /* envelope, operation "users.get" */
rivet_string_free(ops); rivet_string_free(out); rivet_module_free(users);
```

```python
# illustrative wrapper shipped as an example (examples/python/rivet.py), not a package
rt = Rivet(policy_file="policy.json")
users = rt.load("./users.rivet")        # Module object
users.get(id=42)                        # → {'request_id': …, 'operation': 'users.get', 'status': 'ok', 'data': {…}, …}
users.operations()                      # ['get', 'list']
```

```text
 host ──load(path)──▶ read file (+ imports) under root ──▶ compile ──▶ namespace by alias ──▶ new catalog snapshot (Arc swap)
   │                                                                          │
   │                                         in-flight requests keep the old snapshot; new calls see the module
   └──module.call("get", data)──▶ dispatcher("users.get") ──▶ loader's policy ──▶ ResponseEnvelope
```

## Project Standards Baseline

| Standards Index | Revision | Validated At |
|---|---|---|
| [docs/standards/index.md](../../standards/index.md) (REF-2026-0009) | 5 | 2026-09-28 |
| AGENTS.md | Unversioned; contract-first, VHCO five buckets, proposal sections | 2026-09-28 |
| DOCUMENTATION.md | Document revision 4 | 2026-09-28 |

## Project Validation

| Rule | Applicability | Proposal Evidence | Initial Result | Exception or Follow-Up |
|---|---|---|---|---|
| Contract before implementation | Applies | [Contract Delta](#contract-delta) lists every use case, domain type and surface; no code written | PASS | Maintainer reviews the contract via `vhco live` before coding |
| Pure use cases and five code buckets | Applies | Envelope shaping is a pure domain function; globals and highlight tokenizing sit in `features/language`; the FFI surface is `orchestrator/setup_ffi.rs` behind thin `#[no_mangle]` shims in `rivet-ffi` | PASS | vhco must accept the second workspace crate; checked by T-10 |
| Requirements/change/file/test traceability | Applies | R/C/F/T tables and [Requirements Alignment](#requirements-alignment) | PASS | — |
| Canonical proposal template | Applies | DOCUMENTATION §12.1 sections, plus AGENTS complexity, reference engines, samples and forecast | PASS | — |
| Sample calls for changed surfaces | Applies | CLI, curl, WS, polling, MCP, Rust, C and Python samples above | PASS | — |
| Error registry single source | Applies | New codes listed in [Added, Changed and Removed Contracts](#added-changed-and-removed-contracts) | PASS | API-2026-0005 is updated in the same change |
| Design approval | Applies | [Approval](#approval) | NEEDS HUMAN REVIEW | No approval inferred from the drafting request |

## What the Reference Engines Do

### JSON response envelopes (Google JSON style guide, JSON:API)

The Google JSON style guide uses a top-level object with either `data` or `error`. JSON:API forbids `data` and
`errors` together. Applicable lesson: one envelope with mutually exclusive payloads keeps client parsing to a
single branch on `status`. Rivet keeps **both** keys present with `null`, so clients can use one static type.
Evidence: [Google JSON Style Guide](https://google.github.io/styleguide/jsoncstyleguide.xml),
[JSON:API document structure](https://jsonapi.org/format/#document-top-level) (not re-fetched in this session).

### Rust C ABI (Rust reference, cbindgen, SQLite-style handles)

`crate-type = ["cdylib", "staticlib"]` produces C-consumable libraries. `cbindgen` generates headers from
`extern "C"` items, and opaque handle APIs (for example `sqlite3*` with `*_open`/`*_close`) keep the ABI small.
Applicable lesson: keep the ABI to opaque handles and UTF-8 strings, catch every panic, and version the ABI
separately from the crate. Evidence: [Rust reference — linkage](https://doc.rust-lang.org/reference/linkage.html),
[cbindgen](https://github.com/mozilla/cbindgen), [SQLite C interface](https://www.sqlite.org/cintro.html).

### Cargo features and crates.io rules

Features make optional dependencies opt-in. crates.io rejects packages with git dependencies. Applicable lesson:
split the CLI and the heavy protocol stacks into features, and treat crates.io publication as its own gate.
Evidence: [Cargo features](https://doc.rust-lang.org/cargo/reference/features.html),
[specifying dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html).
crates.io lookups on 2026-09-28: `rivet` and `capy-core` belong to other projects, while `rivet-runtime`,
`rivet-ffi` and `capy-lang` are free.

### TextMate grammars and VS Code

VS Code, Sublime Text and JetBrains IDEs (through TextMate bundles) load TextMate grammars. VS Code adds
`language-configuration.json` for comments, brackets and indentation. Applicable lesson: one generated grammar
covers most editors. Tree-sitter would cover Neovim, Helix and Zed but needs a second grammar and is deferred.
Evidence: [VS Code syntax highlight guide](https://code.visualstudio.com/api/language-extensions/syntax-highlight-guide).

### Rivet 0.1.0 itself

The dispatcher, error registry, policy broker and effect analysis stay as they are. This proposal changes only the
edges (input parsing and output shaping), adds one language form (`global`) and adds packaging. Evidence:
[REL-0.1.0](../../releases/rel-0.1.0-release-notes.md), [SYS-2026-0004](../../system/components/sys-2026-0004-surfaces-and-serve.md).

## Complexity: 3/5 (modules: 4/5)

| Dimension | Rating and reason |
|---|---|
| Engine reuse | High: the dispatcher, policy, adapters and parser are reused as they are |
| New kernels/math | None |
| Blast radius | **High for the envelope** (every surface, every demo, every doc example and the remote client change together); low for globals, highlighting and packaging |
| Existing quality methodology | Strong: 400 conformance tests, demos executed literally, `check_docs`, `vhco` gates |
| Unknowns | Symbol export of `#[no_mangle]` functions through a workspace crate; static-link system library lists per OS; vhco acceptance of a second crate; the Cargo feature split touching the runtime assembly |

Verdict: the envelope is long but easy (mechanical, and well covered by existing tests). Globals are short and
moderately hard (scope rules and the static manifest). Modules (revision 3) are the hardest part: they touch the compiler and catalog identity
(namespacing, visibility, cycle detection) and add a runtime catalog swap, so they rate 4/5. The feature split and FFI are medium (build and linking
unknowns, but a small API). Highlighting is a safe addition. **Difficulty is not effort.**

## Implementation Design

### Environment and Feasibility

| Environment / Version | Required Capability | Feasibility Evidence and Source | Constraint or Unknown | Resolution |
|---|---|---|---|---|
| Rust 1.90, edition 2024 | `cdylib` + `staticlib` crate types | Rust reference, linkage | `#[no_mangle]` must be defined in the cdylib crate | Shims live in `rivet-ffi/src/lib.rs` |
| macOS, Linux (Windows best effort) | Static linking | `cargo rustc -- --print native-static-libs` | System libraries differ per OS (Security.framework, resolv, …) | Generated `rivet.pc` `Libs.private`; documented per OS |
| cbindgen | Header generation | Mozilla cbindgen | New build-time tool | `build.rs` in `rivet-ffi` or a checked-in header verified in CI |
| serde_json `preserve_order` | Stable key order for envelopes | Already enabled (Cargo.toml) | — | Proven |
| Capy parser spans | Exact highlight tokens | 0.1.0 diagnostics use the same spans | Recovery after a syntax error | Tokens up to the error; T-14 |
| Node.js (dev only) | TextMate snapshot tests | `vscode-tmgrammar-test` | New CI tool | Optional CI job; Python fallback checks keyword drift |
| VHCO 1.6 | Second workspace crate | Unknown | May require vhco config | Experiment in plan P1 (T-10) |

### Methods by Use Case

| Change ID | UC IDs | Method and Execution Order | Positive Path | Negative / Failure Path | Requirement Fulfilment | Feasibility |
|---|---|---|---|---|---|---|
| C-01 | UC-01, UC-02 | Domain `ResponseEnvelope` + `from(Outcome)`; replace `Completion::to_json`, `error_envelope`, `Envelope::to_json` call sites | Same values, new keys | Error kind, code and status mapping unchanged | R1, R2 | proven |
| C-02 | UC-01, UC-09 | Domain `InputEnvelope::parse(json, legacy_ok)` used by HTTP, polling, WS, MCP `rivet.request`, CLI `--input`, library, FFI | `{operation, data}` | Mixed keys → validation.input_envelope; alias → deprecation signal | R4, R5 | proven |
| C-03 | UC-01 | Built-ins and CLI `--json` outputs wrapped as envelopes | `data` = payload | — | R3 | proven |
| C-04 | UC-03 | `OutputFormat {Compact, Pretty}` threaded through the CLI writer, HTTP responder, library and FFI | Indented | Stream + pretty → validation.usage / 400 | R6 | proven |
| C-05 | UC-04 | Grammar `global` rule; lowering to `GlobalDecl`; `features/language/compile_globals.rs` evaluates constant expressions; `Frame` gains a read-only global scope | Values visible everywhere | check.global_* codes | R7, R8 | proven (same evaluator, pure subset) |
| C-06 | UC-04 | Effect analysis substitutes globals when building targets | exact knowledge | A non-literal global makes the part dynamic (cannot happen: globals are constant) | R9 | proven |
| C-07 | UC-05 | Facade module `rivet::` (re-exports); internal modules `pub(crate)`; tests and examples use only the facade | Clean docs | Integration tests that need internals move into crate unit tests or `#[doc(hidden)] pub mod internal` | R10 | proven |
| C-08 | UC-05 | Cargo features gate adapter registration and dependencies; the bin needs `cli` | Lean build | Compiled-out adapter → unsupported.feature | R11 | experiment needed (runtime assembly cfgs) |
| C-09 | UC-05 | Package rename; crates.io metadata; `publish --dry-run` after G-PUB | Git dependency now | crates.io blocked until capy-lang exists | R12 | blocked on G-PUB (owner) |
| C-10 | UC-06 | Workspace crate `rivet-ffi`; `orchestrator/setup_ffi.rs` holds the logic (handles, blocking runtime, pull calls); shims export C symbols; cbindgen header; pkg-config | C/Python examples run | Panic, null, UTF-8 and JSON errors → envelopes | R13–R15 | experiment needed (symbol export, static libs) |
| C-11 | UC-07 | `editors/gen_grammar` step from `rivet.capy` literals; VS Code extension files; `.vsix` via `vsce package` (dev tool) | Coloured files | Drift test fails the build | R16 | proven |
| C-12 | UC-08 | `features/language/highlight_source.rs` (pure) over the parser port; CLI `highlight` | ANSI/HTML/JSON | Syntax error → partial tokens + diagnostic | R17 | proven |
| C-13 | all | Docs, demos, migration guide, re-verification of 0.1.0 demos | — | — | R18 | proven |
| C-14 | UC-10 | Grammar `import` rule; use case `features/language/resolve_imports.rs` over a source-loader port (bounded, root-confined, no symlinks); compile each file once; namespace IDs; visibility and `public`; bootstrap sites; module spans in diagnostics, manifest and graph | Multi-file catalog | Typed import errors (R21); cycle path in the message | R19–R21, R24 | proven (the loader port exists; the compiler already carries a file per span) |
| C-15 | UC-11 | `Runtime::load/load_as` (`features/registry/load_module.rs` + catalog snapshot swap); `Module` facade type; FFI `rivet_load`/`rivet_module_*`; Python wrapper example | Module objects | Load errors → `Err`/error envelopes; loader policy denies | R22–R24 | experiment needed (catalog snapshot swap under concurrent requests) |

### Added, Changed and Removed Contracts

| Item | CRUD | Kind | Name / Key / Route / Event | Type, Default or Schema | Scope / Lifetime | Consumers | Requirements |
|---|---|---|---|---|---|---|---|
| Output envelope | UPDATE | type / field | `result` → `data`; + `operation`, `type`, `status`, `error:null`; `effects` moved out of `error` | `ResponseEnvelope` | Every response | All clients | R1, R2 |
| Input envelope | UPDATE | type / field | `id` → `operation`, `params` → `data` (aliases in 0.2.x) | `InputEnvelope` | Every request | All clients | R4, R5 |
| `--data`, `--input FILE\|-` | CREATE | flag | `rivet request` | JSON / path | CLI | Users | R4 |
| `--params` | UPDATE | flag | deprecated alias of `--data` | — | 0.2.x | Users | R5 |
| `--pretty` | CREATE | flag | global CLI flag | bool, default false | CLI | Users | R6 |
| `pretty` | CREATE | query / field | `?pretty=true` on JSON routes; `"pretty": true` in FFI input | bool | Request | HTTP/FFI | R6 |
| `Deprecation` | CREATE | header | HTTP response header when an alias is used | `true` | 0.2.x | HTTP clients | R5 |
| `global` | CREATE | language form | `global NAME = EXPR` | constant expression | Bundle, load-time | Scripts | R7 |
| `validation.input_envelope` | CREATE | error | exit 2, HTTP 422 | — | — | Clients | R4, R5 |
| `validation.pretty_stream` | CREATE | error | HTTP 400 (CLI uses validation.usage, exit 2) | — | — | Clients | R6 |
| `syntax.global`, `check.global_not_constant`, `check.global_forward_ref`, `check.global_duplicate`, `check.global_shadow`, `check.global_assign` | CREATE | error | exit 2 | — | `check`/load | Authors | R8 |
| `unsupported.feature` | CREATE | error | exit 5, HTTP 501 | `details.feature` | Runtime | Hosts | R11 |
| `validation.ffi_argument`, `internal.panic` | CREATE | error | FFI status + envelope | — | FFI | C hosts | R14 |
| `rivet highlight` | CREATE | command | `FILE [--format ansi\|html\|json]` | default ansi on a TTY, json otherwise | CLI | Authors, docs | R17 |
| Cargo features | CREATE | build | `serve`, `grpc`, `quic`, `oauth`, `cli` | default `serve,grpc,quic,oauth` | Build | Rust hosts | R11 |
| Package | UPDATE | build | `rivet` → `rivet-runtime` (lib name `rivet`, bin name `rivet`) | — | Release | Rust hosts | R10 |
| `librivet` | CREATE | artifact | `.so/.dylib/.dll`, `.a/.lib`, `rivet.h`, `rivet.pc` | ABI 1 | Release | C hosts | R13 |
| `rivet.capabilities` | UPDATE | built-in | + `features: [...]`, `abi_version` | — | — | Hosts | R11, R13 |
| `import` | CREATE | language form | `import "PATH" as ALIAS [public]` | top level, before declarations | Bundle | Scripts | R19, R20 |
| `ALIAS.ID` operation IDs | CREATE | naming | namespaced module operations | — | Catalog | All surfaces | R20 |
| `syntax.import`, `not_found.import`, `permission.import_outside_root`, `check.import_cycle`, `check.import_duplicate`, `check.import_collision`, `limit.imports`, `check.module_policy_ignored` (warning) | CREATE | error | exit 2 / 4 / 3 / 2 / 2 / 2 / 5 / warning | — | load / check | Authors | R21, R24 |
| `Runtime::load`, `load_as`, `Module` | CREATE | Rust API | module object | — | Runtime lifetime | Rust hosts | R22 |
| `rivet_load`, `rivet_module_operations`, `rivet_module_call`, `rivet_module_call_start`, `rivet_module_free` | CREATE | C ABI | module handle | — | Runtime lifetime | C hosts | R23 |

### C ABI surface (ABI version 1)

```c
/* illustrative prototypes; exact names and types are fixed by the contract review */
typedef struct RivetRuntime RivetRuntime;   /* thread-safe */
typedef struct RivetCall    RivetCall;      /* one thread at a time */
typedef enum { RIVET_OK = 0, RIVET_ERROR = 1 } RivetStatus;

uint32_t    rivet_abi_version(void);                         /* 1 */
const char *rivet_version(void);                             /* static "0.2.0"; do not free */
RivetStatus rivet_runtime_new(const char *options_json, RivetRuntime **out, char **error_json);
void        rivet_runtime_free(RivetRuntime *rt);            /* drains in-flight calls within the grace */
char       *rivet_request(RivetRuntime *rt, const char *input_json);        /* blocking; always an envelope */
RivetCall  *rivet_call_start(RivetRuntime *rt, const char *input_json);
char       *rivet_call_next(RivetCall *call, int64_t timeout_ms);           /* record, or NULL when done */
char       *rivet_call_send(RivetCall *call, const char *data_json);        /* ack or error envelope */
char       *rivet_call_finish_input(RivetCall *call);
void        rivet_call_cancel(RivetCall *call);
void        rivet_call_free(RivetCall *call);
char       *rivet_highlight(const char *source, const char *format);        /* tokens as JSON/HTML/ANSI */
typedef struct RivetModule RivetModule;   /* revision 3: a loaded file as an object; thread-safe */
RivetStatus rivet_load(RivetRuntime *rt, const char *path, const char *alias_or_null,
                       RivetModule **out, char **error_json);
char       *rivet_module_operations(RivetModule *m);                        /* JSON array of summaries */
char       *rivet_module_call(RivetModule *m, const char *id, const char *data_json);   /* envelope */
RivetCall  *rivet_module_call_start(RivetModule *m, const char *id, const char *data_json);
void        rivet_module_free(RivetModule *m);                              /* the module stays loaded in rt */
void        rivet_string_free(char *s);
```

`options_json` is `{file | source+path+root, policy_file | policy_json, ceiling_json?, pretty?}`. A timed-out
`rivet_call_next` returns `{"type":"timeout"}`, which is not an envelope; it tells the host to poll again.

### Architecture, Data, State and Interaction Visuals

```text
BEFORE (0.1.0)                                         AFTER (0.2.0)
src/domain/contracts.rs                                 src/domain/contracts.rs
  Completion::to_json  → {result}                         ResponseEnvelope (one serializer, compact|pretty)
  error_envelope       → {error}                          InputEnvelope    (one parser, legacy aliases)
  Envelope::to_json    → {type}                           Envelope (stream) → ResponseEnvelope records
src/io/{cli,http,mcp,…} each shape JSON                 surfaces call envelope::write(format)
src/lib.rs: pub mod domain/features/infra/io/orch.     src/lib.rs: facade `pub use` + pub(crate) internals
one crate (lib + bin, no features)                      workspace: rivet-runtime (lib+bin[cli]) · rivet-ffi (cdylib+staticlib)
(no globals)                                            features/language/compile_globals.rs · highlight_source.rs
(no editor support)                                     editors/{keywords.json, rivet.tmLanguage.json, vscode/}
```

```text
Workspace (proposed)
rivet/                       package rivet-runtime  (lib "rivet", bin "rivet" requires feature cli)
├── src/ {domain,features,io,infra,orchestrator}    ← VHCO five buckets, unchanged layout
│   └── orchestrator/setup_ffi.rs                   ← FFI surface logic (safe Rust, JSON in/out, handles)
├── ffi/                     package rivet-ffi      (crate-type cdylib + staticlib, lib name "rivet")
│   ├── src/lib.rs           #[no_mangle] extern "C" shims → rivet::ffi_surface
│   ├── cbindgen.toml · include/rivet.h · rivet.pc.in
├── editors/                 keywords.json (generated) · rivet.tmLanguage.json · vscode/
└── examples/                embed.rs (Rust) · c/demo.c · python/demo.py
```

## Expected Code and Documentation Changes

| ID | Path | CRUD | Symbol / Region | Exact Planned Edit | Reason | Change IDs | Requirement IDs | Dependencies | Test / Documentation Impact |
|---|---|---|---|---|---|---|---|---|---|
| F-01 | `src/domain/envelope.rs` | CREATE | `ResponseEnvelope`, `InputEnvelope`, `OutputFormat`, `Status`, `RecordType` | Types, `from(Outcome)`, `parse`, `to_json(format)` | One shape | C-01, C-02, C-04 | R1, R2, R4–R6 | — | T-01…T-05 |
| F-02 | `src/domain/contracts.rs` | UPDATE | `Completion`, `Envelope`, `error_envelope` | Serialize through F-01; `from_json` accepts both shapes for the remote client | Single serializer | C-01 | R1 | F-01 | T-01 |
| F-03 | `src/orchestrator/setup_{cli,http,ws,poll,mcp,library}.rs`, `src/io/**` | UPDATE | input parsing, output writing | Use F-01 parse/write; `--data`, `--input`, `--pretty`, `?pretty=true` | Surfaces | C-02…C-04 | R3–R6 | F-01 | T-02…T-05, T-15 |
| F-04 | `src/orchestrator/builtins.rs`, `remote_cli.rs`, `infra/remote_client.rs` | UPDATE | built-in results; remote decode | Envelope everywhere; client reads 0.2 envelopes | R3 | C-03 | R3 | F-01 | T-03 |
| F-05 | `src/infra/rivet.capy` | UPDATE | new `global` top-level function | `global NAME = EXPR` | Language | C-05 | R7 | — | T-06 |
| F-06 | `src/features/language/compile_globals.rs`, `lowering/lower.rs`, `domain/ir.rs` | CREATE / UPDATE | `GlobalDecl`, `GlobalScope`, constant evaluation, check codes | Load-time pure evaluation | Globals | C-05 | R7, R8 | F-05 | T-06, T-07 |
| F-07 | `src/infra/execution_driver.rs` | UPDATE | `Frame` lookup | read-only global scope after locals/params | Globals | C-05 | R7 | F-06 | T-06 |
| F-08 | `src/features/audit/inspect_effects.rs`, `call_graph` | UPDATE | target building | Substitute globals → exact | Manifest | C-06 | R9 | F-06 | T-08 |
| F-09 | `src/lib.rs`, module visibility | UPDATE | facade | `pub use` facade; internals `pub(crate)` | Stable API | C-07 | R10 | — | T-09 |
| F-10 | `Cargo.toml` (+ workspace), `src/orchestrator/runtime.rs` adapter registration | UPDATE | features, package rename, `required-features` | cfg-gated adapters; `unsupported.feature` | Lean builds | C-08, C-09 | R11, R12 | F-09 | T-10 |
| F-11 | `src/orchestrator/setup_ffi.rs` | CREATE | FFI surface logic | handles, blocking runtime, pull calls, panic/arg guards | C ABI | C-10 | R13–R15 | F-01, F-09 | T-11, T-12 |
| F-12 | `ffi/` (Cargo.toml, src/lib.rs, cbindgen.toml, include/rivet.h, rivet.pc.in) | CREATE | shims, header | `#[no_mangle] extern "C"` → F-11 | C ABI | C-10 | R13 | F-11 | T-11, T-12 |
| F-13 | `src/features/language/highlight_source.rs`, CLI `highlight` | CREATE | token classes; ANSI/HTML/JSON renderers | exact tokens | Highlighting | C-12 | R17 | — | T-14 |
| F-14 | `editors/` (gen script, `keywords.json`, `rivet.tmLanguage.json`, `vscode/`) | CREATE | grammar + extension | generated from `rivet.capy` | Editors | C-11 | R16 | — | T-13 |
| F-15 | `tests/conformance_envelope.rs`, `conformance_globals.rs`, `conformance_ffi.rs`, `conformance_highlight.rs`, `conformance_features.rs` + updates to all suites | CREATE / UPDATE | tests | new and updated assertions | Evidence | all | R1–R17 | — | T-01…T-15 |
| F-16 | `vhco-contract.json` | UPDATE | see Contract Delta | contract first | AGENTS | all | all | — | `vhco sync` 0 |
| F-17 | `docs/api/*`, `docs/manuals/*`, `docs/system/*`, new MAN (FFI), `docs/migrations/mig-2026-0001-*`, `docs/demos/{14-globals,15-ffi,16-editor}`, REF-2026-0002 | CREATE / UPDATE | docs | current-state docs, migration guide, demos | DOCUMENTATION §§29–31 | C-13 | R18 | all | T-31, T-30 |
| F-18 | `.github/workflows/ci.yml`, `commands.perch` | UPDATE | feature matrix, FFI build, `--features cli` for install/build | build | Release | C-08, C-10 | R11, R13 | F-10, F-12 | T-10, T-11 |
| F-19 | `src/infra/rivet.capy`, `lowering/lower.rs`, `domain/ir.rs` | UPDATE | `import` form, `ImportDecl` | grammar + lowering | Modules | C-14 | R19 | — | T-16 |
| F-20 | `src/features/language/resolve_imports.rs`, `compile_program.rs`, `src/infra/source_loader.rs` | CREATE / UPDATE | resolution, namespacing, visibility, errors, bootstrap sites | — | Modules | C-14 | R19–R21, R24 | F-19 | T-16, T-17 |
| F-21 | `src/features/audit/inspect_effects.rs`, `domain/call_graph.rs`, `features/policy/*` | UPDATE | module spans and namespaced IDs in manifest, graph, explain, generate | — | Modules | C-14 | R24 | F-20 | T-20 |
| F-22 | `src/features/registry/load_module.rs`, `src/orchestrator/runtime.rs`, facade `Module` | CREATE / UPDATE | load/load_as, catalog snapshot swap, builder without entry file | — | Modules | C-15 | R22 | F-20 | T-18 |
| F-23 | `src/orchestrator/setup_ffi.rs`, `ffi/src/lib.rs`, `ffi/include/rivet.h` | UPDATE | `rivet_load`, `rivet_module_*` | — | Modules | C-15 | R23 | F-22, F-11 | T-19 |
| F-24 | `examples/python/rivet.py`, `examples/c/modules.c`, `examples/modules.rs` | CREATE | module examples | — | Modules | C-15 | R22, R23 | F-22, F-23 | T-18, T-19 |

Counts (revision 3): 10 created areas (F-01, F-11–F-14, F-20, F-22, F-24, new tests, new docs); 14 updated; 0 deleted (0.1.0 serializers
are replaced in place).

## Alternatives Considered

| Alternative | Advantages | Disadvantages | Why Selected or Rejected | Requirement Impact |
|---|---|---|---|---|
| Keep 0.1.0 output and add a `--envelope v2` switch | No break | Two formats forever; every test doubled | Rejected: 0.x allows a clean break, and 0.1.0 has no external consumers yet (no remote) | R1 |
| Omit `data`/`error` when null (JSON:API style) | Smaller JSON | Clients must test key presence; typed decoders need optional fields | Rejected: the maintainer asked for both keys; always-present keys give one static type | R1 |
| `ok: true/false` instead of `status` | Simple | Cannot express `cancelled`/`accepted` | Rejected | R2 |
| Mutable or per-request globals | More power | Hidden state, order dependence, races | Rejected by the maintainer (immutable constants) | R7 |
| FFI callbacks for streaming | Push-based | Re-entrancy, thread and GIL hazards in hosts | Rejected in favour of a pull handle | R15 |
| cdylib/staticlib in the main crate | One crate | Every `cargo build` builds three artifacts; symbol and LTO issues | Rejected: separate `rivet-ffi` crate | R13 |
| Tree-sitter grammar first | Better editors beyond VS Code | Second grammar to maintain | Deferred (Q-02) | R16 |
| Keep package name `rivet` | No rename | Taken on crates.io | Rejected; lib name stays `rivet` | R10 |
| Keep original IDs for imports | No renaming | Collisions across files; unclear origin | Rejected by the maintainer (namespaced by alias) | R20 |
| Each module under its own policy.json | Module authors control I/O | Several authorities per runtime; harder audit | Rejected by the maintainer (loader's policy only) | R24 |
| All imported operations public by default | Less syntax | Silently widens every surface's catalog | Rejected: internal by default, `public` to expose | R20 |

## Risks and Rollback

| Risk | Trigger / Detection | Impact | Mitigation | Rollback Action | Owner |
|---|---|---|---|---|---|
| Envelope break misses a surface | Schema test T-01 over every surface | Clients see mixed shapes | Single serializer; grep gate for `"result":` in src | Revert the envelope increment (independent of the others) | Implementer |
| Demos and docs drift after the break | T-30 literal re-execution | Wrong docs | Re-verify all 12 demos in the same plan | — | Implementer |
| Feature split breaks the runtime assembly | Feature-matrix CI build (T-10) | Build failures for embedders | cfg per adapter registration only | Keep all features default-on | Implementer |
| FFI memory misuse by hosts | Fuzz tests; ASan build (T-12) | Crashes in the host process | Handle generation checks; documented ownership | Ship `librivet` as experimental in 0.2 | Implementer |
| Static linking fails on some OS | CI link test per OS | No static artifact there | Generated `Libs.private`; document | Ship the shared library only on that OS | Implementer |
| crates.io blocked | G-PUB not met | Git dependency only | Documented | — | Maintainer |
| vhco rejects a second crate | T-10 in plan P1 | Architecture gate red | Keep FFI logic in the main crate (F-11) | Move the shims into the main crate behind an `ffi` feature | Implementer |
| Catalog swap races with running requests | T-18 concurrent load test | Wrong operation resolved mid-request | Immutable `Arc` snapshots; a request resolves once at dispatch | Disable `load` after `build` (static catalogs only) | Implementer |
| Import graphs blow up load time | `limit.imports` (256 files, depth 16) | Slow start | Compile each file once (dedup by canonical path) | Lower the limits | Implementer |

## Security Impact

- FFI callers are host code with the same authority as a library host: principal `local`, policy from
  `policy.json` or `policy_json`, optionally narrowed by `ceiling_json`. No new privilege path.
- Every FFI entry catches panics, rejects null and invalid UTF-8, and never exposes Rust pointers except the two
  opaque handles. Freed handles are poisoned (generation check), so a double free becomes an error rather than
  undefined behaviour where detectable.
- Globals cannot hold secrets or read the environment or files (`check.global_not_constant`), so a secret can
  never become bundle-wide state. Secret taint rules (G23b) are unchanged.
- The pretty format changes whitespace only; redaction and the secret sink rules apply before formatting.
- `rivet highlight` reads only the named file (bootstrap I/O, like `check`).
- Imports and `load` are bootstrap reads confined to the runtime root: no `..` escape, no symlinks, no URLs.
  Every loaded module runs under the loader's single policy, and a module's own `policy.json` is ignored with a
  warning. A module cannot widen authority, and `rivet io` shows every file's effects before anything runs.

## Operational Impact

- The CLI install needs `--features cli`. `commands.perch` build/install tasks and CI pass it, and
  `cargo install rivet-runtime --features cli` is documented.
- New release artifacts: `librivet` (shared and static), `rivet.h`, `rivet.pc`, and `rivet-<ver>.vsix`.
- HTTP responses may carry `Deprecation: true` during 0.2.x; operators can monitor it in the access log
  (a new `deprecated=1` field).

## Compatibility Impact

| Area | 0.1.0 | 0.2.0 | 0.3.0 |
|---|---|---|---|
| Output | `{request_id, trace_id, result, data_count, effects}` / `{…, error}` | Envelope R1 | Envelope R1 |
| Input | `{id, params}` | `{operation, data}`; `{id, params}` accepted with deprecation | `{id, params}` refused (`validation.input_envelope`, with a hint) |
| CLI | `--params` | `--data` (`--params` alias, warning) | `--data` only |
| Exit codes / HTTP statuses / error codes | — | unchanged | unchanged |
| Rust API | internal modules public | facade; internals hidden | facade |
| `rivet --endpoint` | talks to 0.1 servers | talks to 0.2 servers; a 0.1 server gives `protocol.endpoint` with a version hint | — |
| Bundles | single file | `import` supported; single-file bundles unchanged | — |

## Migration Requirements

A migration document `docs/migrations/mig-2026-0001-response-and-input-envelopes.md` (DOCUMENTATION §4.21)
gives before/after tables per surface, a `jq` mapping (`.result` → `.data`), the deprecation timeline and a
rollback plan (pin `v0.1.0`).

## Test and Validation Design

| ID | Type | UC / Requirement IDs | Scenario and Purpose | Environment / Data | Exact Procedure or Command | Expected Result | Test File / Evidence Destination |
|---|---|---|---|---|---|---|---|
| T-01 | integration | UC-01, R1, R2 | Envelope schema on every surface, ok/error/cancelled | In-process serve, fixtures | `cargo test --test conformance_envelope` | Every output validates; key order fixed | `tests/conformance_envelope.rs` |
| T-02 | integration | UC-01, R4 | The same input file on CLI `--input`, HTTP, WS, polling, MCP, library | Temp bundle | same suite | Identical `data` | same |
| T-03 | integration | UC-01, R3 | Built-ins and `--json` outputs are envelopes | Bundle | same suite | Schema valid | same |
| T-04 | integration | UC-02, R1 | Stream records then exactly one result; consumer stop → cancelled | `emits` operation | same suite | Record sequence and statuses | same |
| T-05 | integration | UC-03, R6 | Pretty golden output; pretty+stream refused | — | same suite | 2-space indent; validation.usage / 400 | same |
| T-06 | unit + integration | UC-04, R7 | Globals visible in all operations; concurrent requests see the same values | Bundle with globals | `cargo test --test conformance_globals` | Values equal | `tests/conformance_globals.rs` |
| T-07 | failure | UC-04, R8 | Each `check.global_*` code with span | Bad bundles | same suite | Exact code, line and column; exit 2 | same |
| T-08 | integration | UC-04, R9 | Manifest exactness and `policy generate` with globals | Bundle | same suite | `exact` targets; exact grants | same |
| T-09 | build | UC-05, R10 | External crate uses only the facade | `examples/embed.rs`, scratch crate | `cargo run --example embed` | Builds and prints an envelope | examples/, CI |
| T-10 | build | UC-05, R11 | Feature matrix; compiled-out adapter → unsupported.feature; vhco accepts the workspace | CI | `cargo build --no-default-features`, per-feature builds, `vhco validate .` | All green | CI log |
| T-11 | integration | UC-06, R13, R15 | C and Python examples, shared and static | Release artifacts | `make -C examples/c && ./demo`; `python3 examples/python/demo.py` | Envelopes; streaming, input and cancel work | `tests/conformance_ffi.rs` + CI |
| T-12 | failure / security | UC-06, R14 | Null, bad UTF-8, bad JSON, panic probe, double free, free-while-running | ASan build | `cargo test -p rivet-ffi` | Error envelopes; no crash or leak | `ffi/tests/` |
| T-13 | regression | UC-07, R16 | Grammar keyword drift vs `rivet.capy`; TextMate snapshots over REF examples | Node (dev) | `python3 editors/check_keywords.py`; `npx vscode-tmgrammar-test` | No drift; snapshots stable | `editors/tests/` |
| T-14 | integration | UC-08, R17 | `rivet highlight` ansi/html/json golden; syntax-error partial tokens | Bundles | `cargo test --test conformance_highlight` | Goldens match; spans inside the source | `tests/conformance_highlight.rs` |
| T-15 | compatibility | UC-09, R5 | Legacy `{id, params}` accepted with deprecation signals; mixed keys refused | — | conformance_envelope | Deprecation header/warning; 422 on mix | same |
| T-16 | integration | UC-10, R19, R20 | Imports compile once; namespacing; internal vs `public`; `(alias.id …)` and `request`; nested imports | Multi-file bundles | `cargo test --test conformance_modules` | Catalog and calls as specified | `tests/conformance_modules.rs` |
| T-17 | failure | UC-10, R21 | Each import error code with its span; cycle path | Bad bundles | same | Exact codes and exits | same |
| T-18 | integration | UC-11, R22 | `load`/`load_as`, `operations`, `call`, streams; builder without an entry file; concurrent load during requests | Temp files | same | Envelopes; no race | same |
| T-19 | integration | UC-11, R23 | C module example and the Python wrapper | Release artifacts | `make -C examples/c modules`; `python3 examples/python/modules.py` | Envelopes | `tests/conformance_ffi.rs` |
| T-20 | security | UC-10, UC-11, R24 | Loader policy governs modules; module policy.json ignored with a warning; manifest, graph and generate cover modules | Bundles with a module-local policy.json | conformance_modules | Denied as by the loader; warning shown; spans point at module files | same |
| T-30 | manual / e2e | all | Re-execute all demos (0.1.0 set + 14/15/16) | Release candidate | follow READMEs | Every step matches | TEST document |
| T-31 | documentation | R18 | `check_docs`, `vhco docs check` | — | `python3 scripts/check_docs.py` | 0 problems | TEST document |

### Measurement and Validation

NOT APPLICABLE: no optimization or measurable performance claim is made. Envelope size grows by roughly 60–80 bytes
per response (the new keys). That is stated as a cost, not measured as a claim.

## Verification and Promotion Gates

1. **Correctness first:** T-01…T-15 green on macOS and Linux (CI needs a git remote), `vhco assure` green, demos re-executed.
2. **Quality:** `cargo doc` for the facade has no broken links; `cbindgen` header is checked in and diffs are reviewed; `.vsix` installs in a clean VS Code profile.
3. **Expected result before measurement:**

| Item | Forecast | Caution |
|---|---|---|
| Envelope change | ~all conformance suites need assertion updates; no behaviour change | A suite passing unchanged means it never inspected output keys: investigate |
| Globals | small grammar and lowering change; manifest precision improves for URL-building scripts | Exact targets appearing where a param is involved would be a bug |
| Features | `--no-default-features` build noticeably faster and smaller | A build that still pulls tonic/quinn means a feature leak |
| FFI | tiny API; most effort in build and link docs | Any crash in T-12 blocks release |
| Highlighting | safe addition | 100% tokens classified as keywords would indicate a regex error |

## Contract Delta

`vhco-contract.json` changes, made by the contract owner before code (AGENTS rule):
- **Domain:** `ResponseEnvelope`, `InputEnvelope`, `OutputFormat`, `EnvelopeStatus`, `RecordType`, `GlobalDecl`,
  `GlobalScope`, `HighlightToken`, `HighlightFormat`, `FfiOptions`. `Completion` and `Envelope` gain an
  envelope projection. `CapabilityReport` gains `features` and `abi_version`.
- **Use cases:** `language.compile_globals`, `language.highlight_source`, `serve.parse_input` (input envelope
  and legacy aliases).
- **Changed flows:** `execution.request_operation` output shaping; `audit.inspect_effects` global substitution.
- **Surfaces:** new `ffi` surface (`orchestrator/setup_ffi.rs`); `cli` gains `highlight`, `--data`, `--input`,
  `--pretty`; `http` routes document the `pretty` query and the `Deprecation` header. The
  `api.every_route_has_request_response` guarantee is updated with the new request and response shapes.
- **Revision 3 (modules):** domain `ImportDecl`, `ModuleRef`, `ModuleSummary`; use cases `language.resolve_imports`
  and `registry.load_module`; `CatalogSnapshot` swap in the runtime; the `library` and `ffi` surfaces gain the
  `load`/`rivet_load` triggers.
- **Infra:** none new (highlighting uses the existing Capy parser port; imports use the existing source loader).

## Documentation, Demo and Release Impact

| Artifact | Exact Path or Destination | CRUD | Required Content / Verification | Owner | Release Gate |
|---|---|---|---|---|---|
| API docs | `docs/api/api-2026-0001…0005` | UPDATE | Envelopes, `pretty`, deprecation, new errors | Implementer | T-31 |
| Manual | `docs/manuals/man-2026-0003` (globals), `0004` (CLI flags, `highlight`), `0007` (facade, features), new `man-2026-0009-c-abi-and-ffi.md`, new editor chapter | UPDATE / CREATE | Verified examples | Implementer | T-30 |
| System | `docs/system/*` (surfaces, compiler, library) | UPDATE | Current state | Implementer | TASK review |
| Architecture | `docs/architecture/arch-2026-0001` | UPDATE | Workspace, facade, ffi surface | Implementer | — |
| Migration | `docs/migrations/mig-2026-0001-response-and-input-envelopes.md` | CREATE | Before/after, timeline, rollback | Implementer | T-31 |
| Demos | `docs/demos/14-globals`, `15-ffi`, `16-editor`; re-verify 01–13 | CREATE / UPDATE | Executed literally | Implementer | T-30 |
| Reference | `docs/references/ref-2026-0002` | UPDATE | `global` syntax row and examples | Implementer | conformance_samples |
| Manual (modules) | `docs/manuals/man-2026-0003` (imports chapter), `0007` (`load`), `0009` (`rivet_load`), `0001` (L1 limitation removed; feature catalogue) | UPDATE | Verified examples | Implementer | T-30 |
| System (modules) | `docs/system/components/sys-2026-0001` (import resolution), `sys-2026-0003` (manifest across modules) | UPDATE | Current state | Implementer | review |
| Demo (modules) | `docs/demos/17-modules` | CREATE | import + host load, executed | Implementer | T-30 |
| README | `README.md` | UPDATE | Envelope quickstart, dependency snippet, FFI and editor pointers | Implementer | — |
| Version source | `Cargo.toml` (both packages) | UPDATE | 0.2.0; `scripts/check_version.py` also checks the FFI version | Implementer | T-33 |
| Release | `docs/releases/rel-0.2.0-release-notes.md` | CREATE | §33 | Implementer | §34 |

## Requirements Alignment

| Requirement | User Request | Goal | Use Cases / Internal Constraint | Changes | Files | Tests | Manual / Demo / Release Evidence |
|---|---|---|---|---|---|---|---|
| R1 | UQ-03, UQ-05 | G-01 | UC-01, UC-02 | C-01 | F-01, F-02, F-03 | T-01, T-04 | API docs, MIG |
| R2 | UQ-05 | G-01 | UC-01 | C-01 | F-01, F-02 | T-01 | API-2026-0005 |
| R3 | UQ-03 | G-01 | UC-01 | C-03 | F-04 | T-03 | MAN-0004 |
| R4 | UQ-06 | G-02 | UC-01 | C-02 | F-01, F-03 | T-02 | API docs |
| R5 | UQ-06 | G-02 | UC-09 | C-02 | F-01, F-03 | T-15 | MIG |
| R6 | UQ-04 | G-03 | UC-03 | C-04 | F-01, F-03 | T-05 | MAN-0004 |
| R7 | UQ-07 | G-04 | UC-04 | C-05 | F-05, F-06, F-07 | T-06 | MAN-0003, demo 14 |
| R8 | UQ-07 | G-04 | UC-04 | C-05 | F-06 | T-07 | API-2026-0005 |
| R9 | UQ-07 | G-04 | UC-04 | C-06 | F-08 | T-08 | MAN-0005 |
| R10 | UQ-02 | G-05 | UC-05 | C-07 | F-09 | T-09 | MAN-0007 |
| R11 | UQ-02 | G-05 | UC-05 | C-08 | F-10, F-18 | T-10 | MAN-0007, MAN-0002 |
| R12 | UQ-02 | G-05 | UC-05 | C-09 | F-10 | T-10 (dry run after G-PUB) | REL-0.2.0 |
| R13 | UQ-08 | G-06 | UC-06 | C-10 | F-11, F-12, F-18 | T-11 | MAN-0009, demo 15 |
| R14 | UQ-08 | G-06 | UC-06 | C-10 | F-11, F-12 | T-12 | MAN-0009 |
| R15 | UQ-08 | G-06 | UC-06 | C-10 | F-11, F-12 | T-11 | demo 15 |
| R16 | UQ-01 | G-07 | UC-07 | C-11 | F-14 | T-13 | demo 16 |
| R17 | UQ-01 | G-07 | UC-08 | C-12 | F-13 | T-14 | MAN-0004 |
| R18 | all | all | DOCUMENTATION §§29–31 | C-13 | F-17 | T-30, T-31 | REL-0.2.0 |
| R19 | UQ-09 | G-08 | UC-10 | C-14 | F-19, F-20 | T-16 | MAN-0003, demo 17 |
| R20 | UQ-09 | G-08 | UC-10 | C-14 | F-20 | T-16 | MAN-0003 |
| R21 | UQ-09 | G-08 | UC-10 | C-14 | F-20 | T-17 | API-2026-0005 |
| R22 | UQ-09 | G-08 | UC-11 | C-15 | F-22, F-24 | T-18 | MAN-0007, API-2026-0004 |
| R23 | UQ-09 | G-06, G-08 | UC-11 | C-15 | F-23, F-24 | T-19 | MAN-0009, API-2026-0007 |
| R24 | UQ-09 | G-08 | UC-10, UC-11 | C-14 | F-21 | T-20 | MAN-0005, SEC-2026-0001 |

Reverse check: every C-01…C-13 and F-01…F-18 maps to at least one requirement above. F-15 (tests) and F-16
(contract) serve all requirements.

## Plan Strategy and Estimated Work

One plan (PLAN-2026-0002 → v0.2.0) is enough. The increments are ordered so that the breaking change lands first and
everything built later (FFI, examples, docs) uses the final shapes.

```text
 P1 contract + experiments (vhco workspace, symbol export, static libs) ─┐
 P2a envelopes + input + pretty (R1–R6) ──────────────────────────────────┼─▶ P2c facade + features (R10–R12) ─▶ P2d C ABI (R13–R15)
 P2b globals (R7–R9) ──────────────── independent ────────────────────────┤
 P2e highlighting (R16–R17) ────────── independent ───────────────────────┤
 P2f modules (R19–R24) ─ after P2b (globals scoping) and before P2d's module ABI ┘
 P3 tests / validation ─▶ P4 docs, migration guide, demos 14–16, re-verify 01–13 ─▶ P5 release v0.2.0
```

## Open Questions

| ID | Question | Recommendation |
|---|---|---|
| Q-01 | May clients supply their own `request_id` in the input envelope (idempotency or correlation)? | No for 0.2.0: IDs stay server-assigned for uniqueness and trace integrity; WS `ref` and HTTP responses already correlate |
| Q-02 | Add a tree-sitter grammar (Neovim, Helix, Zed) now? | Later plan; the keyword table (F-14) makes it cheap to add |
| Q-03 | Colourize pretty JSON on a TTY (`--color auto`)? | Optional follow-up reusing the `highlight` ANSI renderer |
| Q-04 | Default Cargo features: all runtime features (proposed) or minimal? | All runtime features on and `cli` off: embedders opt out, and the CLI opts in |
| Q-05 | Publish Capy as `capy-lang` (the owner's decision, G-PUB)? | Yes, if crates.io publication of Rivet is wanted |
| Q-06 | Should `global` values appear in `rivet describe` or a `rivet globals` command? | Not in 0.2.0; they are visible in `rivet io` targets |
| Q-07 | Unload or hot-reload modules; URL or registry imports? | Not in 0.2.0 (non-goals); `load` only adds modules |

## Approval

| Gate | Decision | By | Date |
|---|---|---|---|
| G-DESIGN (this proposal) | approved ([ADR-0004](../../decisions/adr-0004-approve-envelopes-globals-library-ffi-highlighting.md)) | Project maintainer | 2026-09-28 |
| G-CONTRACT (`vhco-contract.json` delta) | approved as the Contract Delta above; applied in PLAN-2026-0002 P1 | Project maintainer | 2026-09-28 |
| G-PUB (Capy on crates.io as `capy-lang`) | open, optional (owner's decision; not in v0.2.0 scope) | Project maintainer | — |

Open questions Q-01…Q-06 are resolved with the recommendations in the table above (ADR-0004).

## Related Documents

- [PROP-2026-0001](../implemented/prop-2026-0001-rivet-runtime.md): the runtime design this extends
- [REL-0.1.0](../../releases/rel-0.1.0-release-notes.md): the current release
- [API-2026-0001](../../api/api-2026-0001-http-rest-sse-polling.md), [API-2026-0004](../../api/api-2026-0004-rust-library.md), [API-2026-0005](../../api/api-2026-0005-error-registry.md)
- [MAN-2026-0003](../../manuals/man-2026-0003-language-guide.md): the language guide (globals go here)
- [ADR-0002](../../decisions/adr-0002-rust-crate-selection.md): crate selection

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 4 | 2026-09-30 | Claude | Implemented in v0.2.0 (PLAN-2026-0002, REL-0.2.0); moved to implemented/. |
| 3 | 2026-09-28 | Claude | Amendment (UQ-09): file modules — `import … as ALIAS [public]` in `.rivet` and `Runtime::load`/`rivet_load` module objects; namespaced by alias; loader's policy only. Added P-08, G-08, R19–R24, UC-10/UC-11, C-14/C-15, F-19–F-24, T-16–T-20, Q-07. |
| 2 | 2026-09-28 | Claude | Approved (ADR-0004); moved to approved/; Q-01…Q-06 resolved with the recommendations; PLAN-2026-0002 created. |
| 1 | 2026-09-28 | Claude | Initial draft from UQ-01…UQ-08, with the maintainer's clarifications: library = Cargo dependency, globals = immutable constants, no `error` in input. |
