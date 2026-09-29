---
document_id: SYS-2026-0001
title: "Rivet compiler and operation catalog"
document_type: system
status: active
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 3
authors: [Claude]
owner: Project maintainer
component_owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, registry]
affected_versions:
  from: "0.1.0"
  to: null
last_verified_version: "0.2.0-rc (main at 8031baa)"
next_review_date: 2026-10-29
review_cycle: on-release
confidentiality: internal
scope: How Rivet turns a .rivet entry file and the modules it imports into an immutable, described operation catalog (grammar, Capy AST boundary, import resolution, lowering to IR, module namespacing, global constants, compile-time checks, declared outputs, registry, catalog snapshots and run-time module loads) and how check/list/describe/outputs expose it.
reason: Every surface (CLI, HTTP, WebSocket, MCP, library) serves the same compiled catalog; maintainers need one current-state description of where parsing ends, what lowering enforces, which check codes exist and how the catalog is read.
related_documents: [PROP-2026-0001, PLAN-2026-0001, PROP-2026-0002, PLAN-2026-0002, ADR-0002, ADR-0004, RES-2026-0001, SYS-2026-0002, SYS-2026-0003, SYS-2026-0004, SYS-2026-0009, SYS-2026-0010, SYS-2026-0011, API-2026-0006, MAN-2026-0003]
supersedes: null
superseded_by: null
tags: [rivet, system, compiler, capy, grammar, ir, registry, catalog, outputs, globals, modules, imports]
---

# Rivet compiler and operation catalog

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** language, registry
> **Last Verified Version:** 0.2.0-rc (main at 8031baa)

## Summary

The compiler turns one `.rivet` entry file, and every file it imports, into a `CompiledProgram`: a frozen
list of operations, connector declarations, auth profiles and one frozen global scope per file, pinned by a
SHA-256 source hash. The registry wraps that program in an immutable catalog of `RegistryEntry` rows. Every
surface reads the same rows: `rivet list`, `describe` and `outputs`, `/v1/operations`, MCP `tools/list`,
`Runtime::list/describe/outputs` and the C ABI. Compilation performs no I/O and runs nothing. The only file
reads are the host bootstrap reads of the entry file and of each imported module (0.2.0), all confined to
the runtime root.

New in 0.2.0:

- **Globals.** `global NAME = EXPR` at the top level is evaluated once at load by `language.compile_globals`
  into a read-only `GlobalScope` per file.
- **Modules.** `import "PATH" as ALIAS [public]` is followed by `language.resolve_imports`. Each imported file
  compiles once, under the namespace `ALIAS.ID`.
- **Run-time loads.** A host can add a module to a running runtime with `Runtime::load` or `rivet_load`
  (`registry.load_module`). The load publishes a new catalog snapshot, and requests already running keep the
  snapshot they started on.
- **Wire output.** The `--json` outputs of `list`, `describe`, `outputs` and `check` are
  [ResponseEnvelopes](../../api/api-2026-0006-envelopes.md) whose `operation` names the built-in
  (`rivet.list`, …).

```text
  app.rivet ──▶ DiskSourceLoader ──▶ SourceBundle ──▶ resolve_imports ──▶ SourceBundle (+ modules[])
  (bytes)       (bootstrap read)     (entry, root,    (import lines only;   every imported file read once
                                      files[])         confined to root)    below the root, BFS namespaces)
                                                               │
                                                               ▼ for each file
                                              CapyParser ──▶ Capy AST JSON (schema_version 1)
                                              (embedded        │
                                               rivet.capy)     ▼
                                                          SyntaxTree  (Rivet-owned, spans)
                                                               │ Lowerer (lower.rs + expr.rs)
                                                               ▼
                                                          operations · connectors · auth · globals · imports
                                                               │ namespace_modules (ALIAS.ID, visibility)
                                                               │ bundle checks · compile_globals · sha256
                                                               ▼
                                                          CompiledProgram (typed IR + GlobalScope[])
                                                               │ CatalogSnapshot{version, bundle, program}
                                                               ▼
                                                          ProgramRegistry (RegistryEntry[])
                                                               │
          ┌──────────────┬──────────────┬──────────────┬───────┴──────┬──────────────┬──────────────┐
          ▼              ▼              ▼              ▼              ▼              ▼              ▼
      rivet list   rivet describe  rivet outputs  /v1/operations  MCP tools/list  Runtime::list  rivet_module_operations
```

## Responsibilities

| Responsibility | Where | Notes |
|---|---|---|
| Hold the surface grammar | `src/infra/rivet.capy` | Compiled into the binary with `include_str!` |
| Parse source with Capy and convert the result to Rivet types | `src/infra/capy_parser.rs` | This is the only file that imports `capy_core` |
| Rivet-owned syntax tree | `src/domain/syntax_tree.rs` | `SyntaxTree`, `SyntaxNode`, `Capture`, `SyntaxDiagnostic` |
| Source bytes and positions | `src/domain/source.rs`, `src/infra/source_loader.rs` | `SourceBundle`, `SourceFile`, `SourceSpan` |
| Lower statements to typed IR | `src/features/language/lowering/lower.rs` | Header order, option lines, try/catch pairing, effect forms, dag, map/poll, with |
| Parse expressions and arguments | `src/features/language/lowering/expr.rs` | Literals, `${dotted.path}` templates, prefix calls (checked against `BUILTIN_FUNCTIONS`), infix operators anywhere a value goes (objects, lists, call arguments) |
| Typed IR | `src/domain/ir.rs` | `Expr`, `Stmt`, `EffectForm`, `Operation`, `Declaration`, `CompiledProgram` |
| Whole-bundle checks, warnings and hash | `src/features/language/compile_program.rs` | Duplicate IDs, unknown/cyclic calls, unknown auth profiles and connectors; warnings `docs.undeclared_error` and `check.unguarded_result` |
| Declared output types | `src/features/language/compile_output_spec.rs`, `src/domain/outputs.rs` | `ValueSpec`, `OutputSpec`, `FieldSpec`, `ParamSpec`, `DeclaredError` |
| Documentation gate | `strict_doc_findings` in `lower.rs` | Only runs for `rivet check --strict-docs` |
| Immutable catalog | `src/infra/registry.rs` | `ProgramRegistry` implements the `Registry` port |
| Catalog queries | `src/features/registry/describe_operations.rs`, `inspect_outputs.rs` | Private filtering, not_found, output reports |
| Resolve file imports (0.2.0) | `src/features/language/resolve_imports.rs` | `language.resolve_imports`: BFS from the roots, root confinement, compile-once, cycles, limits, visibility |
| Namespace modules (0.2.0) | `src/features/language/lowering/modules.rs` | `namespace_modules`: `ALIAS.ID`, per-file call resolution, internal modules made private, `check.import_collision`, `check.module_policy_ignored` |
| Global constants (0.2.0) | `src/features/language/compile_globals.rs`, `src/domain/const_eval.rs` | `language.compile_globals`: declaration order, pure fold, shadow/assign checks |
| Run-time module loads (0.2.0) | `src/features/registry/load_module.rs`, `src/domain/modules.rs`, `RuntimeCatalog` in `src/orchestrator/runtime.rs` | `registry.load_module`: alias, compile next bundle, atomic snapshot swap |
| Module file reads (0.2.0) | `src/infra/source_loader.rs` `read_module`, `policy_beside` | Symlinks refused; `policy.json` beside a module only probed |
| Syntax highlighting (0.2.0) | `src/features/language/highlight_source.rs` | Reuses `CapyParser` spans; see [SYS-2026-0011](sys-2026-0011-highlighting-and-grammar-generation.md) |
| CLI projection | `src/orchestrator/setup_cli.rs` | `check` (warnings on stderr), `list`, `describe`, `outputs`, `highlight`; `graph` reads the same registry (`features/audit/build_graph.rs`, SYS-2026-0003) |

## Boundaries and Non-Responsibilities

```text
 ┌──────────────────── this document ─────────────────────┐   ┌──────────── elsewhere ─────────────┐
 │ grammar · parse · SyntaxTree · lowering · expr parser  │   │ running Stmt/Expr   → SYS-2026-0002 │
 │ compile checks · OutputSpec · RegistryEntry · catalog  │   │ policy / io manifest→ SYS-2026-0003 │
 │ rivet check / list / describe / outputs                │   │ HTTP/WS/MCP serve   → SYS-2026-0004 │
 └────────────────────────────────────────────────────────┘   │ MCP imports         → SYS-2026-0009 │
                                                              └─────────────────────────────────────┘
```

- **No execution.** The compiler never evaluates expressions or effects. It does resolve names that are
  known statically: a prefix-call function outside `BUILTIN_FUNCTIONS` (`request`, `request.stream`,
  `length`, `base64.encode`, `base64.decode`, `text`, `keys`, `xml.element`) fails `check.unknown_function`
  with a did-you-mean hint (`closest_builtin`, edit distance ≤ 3 or a prefix match such as `len` → `length`).
- **No option semantics beyond shape.** The grammar gives every option line (`timeout`, `header`, `tls`, …)
  a free `tail`. Lowering records it as `OptionLine { key, args, children }`. The protocol adapters check
  what each option means (SYS-2026-0005).
- **No policy.** The compiler does not load or check policy. `Runtime::build` loads `policy.json` after
  compilation succeeds (SYS-2026-0003 and SYS-2026-0008).
- **No effect inventory.** The effect catalog behind `rivet io` is computed from the same `CompiledProgram`
  in `Runtime::assemble` and attached with `ProgramRegistry::with_effects`. It is described in SYS-2026-0003.
- **No MCP schema import.** Imported connector operations are appended with `ProgramRegistry::with_imports`.
  See SYS-2026-0009.
- **`src/domain/effect_checks.rs` is not a compile check.** It holds the runtime `authorize` helper and the
  post-DNS private-range `checked_addr` helper that the transports use.

## Architecture

### Pipeline

```text
 ┌────────────┐  read_to_string   ┌──────────────────────────────┐
 │ --file PATH├──────────────────▶│ SourceBundle                  │  src/infra/source_loader.rs
 └────────────┘  not_found.source │  entry = PATH                 │  (bootstrap read; exit 4 on failure)
                 (exit 4)         │  root  = dirname(PATH) or "." │
                                  │  files = [ {path, text} ]     │
                                  └──────────────┬───────────────┘
                                                 ▼
 ┌───────────────────────────────────────────────────────────────────────────────┐
 │ resolve_imports (0.2.0)          src/features/language/resolve_imports.rs     │
 │   parse each file, lower only its `import` lines (syntax.import)              │
 │   BFS: read each new file once via read_module (below root, no symlinks)      │
 │   namespaces, cycles, limits, public marks ─▶ SourceBundle.modules[]          │
 └──────────────────────────────────────┬────────────────────────────────────────┘
                                        │ for each file
                                        ▼
 ┌───────────────────────────────────────────────────────────────────────────────┐
 │ CapyParser::parse                                   src/infra/capy_parser.rs  │
 │   Library::new(GRAMMAR) once (OnceLock)                                       │
 │   Library::parse(text) ─▶ ast_json::to_json ─▶ serde_json::Value              │
 │   schema_version == 1 ?  no ─▶ internal error                                 │
 │   /tree/stmts   ─▶ convert_node  ─▶ SyntaxNode{func, span, captures, body,    │
 │                                               closer}                         │
 │   /diagnostics  ─▶ convert_diagnostic ─▶ syntax.unknown_statement | syntax.eNNNN│
 │   /tree/errors  ─▶ syntax.unparsed   (only when no diagnostics)               │
 │   + indentation_diagnostics ─▶ syntax.indent                                  │
 └──────────────────────────────────────┬────────────────────────────────────────┘
                                        ▼  SyntaxTree { file, nodes, diagnostics }
 ┌───────────────────────────────────────────────────────────────────────────────┐
 │ Lowerer::lower_tree            src/features/language/lowering/lower.rs        │
 │   diagnostics? ─▶ each becomes RivetError(kind syntax); file is NOT lowered   │
 │   operation|pipeline ─▶ lower_operation ─▶ Operation                          │
 │   connector          ─▶ lower_declaration ─▶ Declaration (connectors)         │
 │   auth_profile       ─▶ lower_declaration ─▶ Declaration (auth_profiles)      │
 │   global (0.2.0)     ─▶ GlobalDecl{name, expr, span, expr_span}               │
 │   import (0.2.0)     ─▶ already handled by resolve_imports                    │
 │   anything else      ─▶ syntax.top_level                                      │
 │   expressions        ─▶ expr.rs parse_expr / parse_args (SpanMap for spans)   │
 └──────────────────────────────────────┬────────────────────────────────────────┘
                                        ▼
 ┌───────────────────────────────────────────────────────────────────────────────┐
 │ compile_program            src/features/language/compile_program.rs           │
 │   namespace_modules  ALIAS.ID, call resolution, check.import_collision (0.2.0)│
 │   check_duplicates   registry.duplicate_id                                    │
 │   check_calls        check.unknown_operation, check.call_cycle                │
 │   check_references   check.unknown_auth_profile, check.unknown_connector      │
 │   compile_globals    GlobalScope per file; check.global_* (0.2.0)             │
 │   errors? ─▶ Err(first error, others in `suppressed`)  (exit 2)               │
 │   source_hash = "sha256:" + H(path ‖ 0 ‖ text ‖ 0 …)   over every file        │
 └──────────────────────────────────────┬────────────────────────────────────────┘
                                        ▼  Arc<CompiledProgram> in a CatalogSnapshot
 ┌───────────────────────────────────────────────────────────────────────────────┐
 │ Runtime::assemble ─▶ Snapshot::build ─▶ ProgramRegistry::new(program)          │
 │   .with_imports(mcp entries) .with_effects(effect catalog)  infra/registry.rs  │
 │   entries = program.operations.map(entry_of)   (built once, never mutated)    │
 │   + Interpreter (global scopes), MCP peer, OAuth adapter  (runtime.rs)        │
 └───────────────────────────────────────────────────────────────────────────────┘
```

The whole pipeline runs inside `RuntimeBuilder::build` (`src/orchestrator/runtime.rs`). It runs before any
command, so every CLI command except `--endpoint` mode and `highlight` rejects a bundle that does not compile.
`RuntimeCatalog::compile` runs the same `resolve_imports` + `compile_program` pair for a run-time module load
(see [Run-time module loads](#run-time-module-loads-and-catalog-snapshots)).

### Grammar (`src/infra/rivet.capy`)

The grammar declares one Capy `function` for each statement shape. Capy decides where a statement and
its captured values start and end. Rivet lowering decides what they mean. The grammar has line comments
(`#`) and an `OpId` pattern type (`^[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z_][A-Za-z0-9_]*)*$`). Blocks close
with `end`. The one exception is `try`, which closes at the dedent (`block_dedent`); lowering then pairs
it with the `catch … end` sibling that follows.

```text
 Top-level declarations           Header lines (inside operation)        Body statements
 ───────────────────────          ─────────────────────────────          ─────────────────────────────
 import "PATH" as ALIAS [public]  (0.2.0; first, before declarations)
 global NAME = EXPR               (0.2.0; before or between declarations)
 operation ID … end               name "…"                               x = VALUE [rest]
 pipeline  ID … end               description "…"                        x = http get "…" ⏎ options… end
 connector NAME KIND … end        private true|false                     x += VALUE
 auth NAME KIND ⏎ … end           param NAME TYPE modifiers…             obj.method args…
                                  output TYPE … [⏎ field… end]            (request "id" {…})   bare call
                                  emits TYPE …  [⏎ field… end]            return | yield | emit VALUE
                                  receives TYPE … [⏎ field… end]          fail "code" [details]
                                  error "code" [description "…"]          break
                                                                         if / while / for X in Y … end
                                                                         try ⏎ … catch error … end
                                                                         dag [opts] ⏎ node … end
                                                                         concurrent [opts] ⏎ task N … end
                                                                         scope [timeout "D"] … end
                                                                         iterate max N … end
                                                                         with RESOURCE … as NAME … end
                                                                         x = map i in XS [limit N] … end
                                                                         x = poll every "D" timeout "D" … end
                                                                         file VERB PATH … ,  secret NAME …
```

The grammar also declares about 70 **option lines** (`opt_*`), for example `decode`, `timeout`, `body`,
`header`, `query`, `tls`, `framing`, `alpn`, `version`, `redirect`, `retry`, `until`, `metadata`,
`limit` and `multicast`. Each one takes `arg capture rest tail`. Lowering turns them into `OptionLine`
values keyed by their name without the `opt_` prefix.

Priorities settle ambiguous lines:

```text
  priority 20   assign_map, assign_poll       x = map … / x = poll …
  priority 10   assign, assign_block,         `message = …` and `stream.send …` win over the
                append_assign, member_call     `message`/`stream` option keywords
  priority  0   everything else
  priority -3   call_stmt (bare)              only a parenthesised prefix call statement
```

`when_followed_by indent` and `when_not_followed_by indent` separate the block form of a statement from
its flat form. Examples are `output` and `output_block`, `field` and `field_block`, `file` and
`file_block`, `body` and `opt_body_block`, and `auth_profile` and `auth_use`.

### Language forms enforced after parsing

| Form | Rule | Enforced in | Code on violation |
|---|---|---|---|
| Prefix calls | `(f a b)`. `f(a, b)` is rejected | `Lowerer::rhs` | `syntax.fcall_style` |
| Durations | Quoted `"100ms"`, `"5s"`, `"2m"` or `"1h"` only (`parse_duration_ms`) | `expr.rs` lexer, `Lowerer::duration` | `syntax.duration_unquoted`, `syntax.duration` |
| String templates | `${dotted.path}` only, with no expressions | `expr.rs` `lex_string` | `syntax.interpolation` |
| Indentation | 4 spaces or a tab per level. Lines inside open brackets or backtick strings are exempt | `capy_parser.rs` | `syntax.indent` |
| Header order | name, description, private, param, output, emits, receives, error | `lower_operation` | `syntax.header_order` |
| Header after body | Header lines must precede the first body statement | `lower_operation` | `syntax.option_after_body` |
| Output required | Every operation declares `output` | `lower_operation` | `syntax.output_required` |
| if/else | `if COND` … [`else` …] `end`: `else` is a Capy block **section** of `if`, alone on its line at the `if`'s indentation; lowering fills `Stmt::If.otherwise`. No `else if` | `rivet.capy` (`if` section `else`), `capy_parser.rs` diagnostic mapping, `lower_stmt` | `syntax.else_if` (`else COND`), `syntax.else_without_if` (orphan or second `else`) |
| try/catch | `try` must be followed by `catch error [kind K \| code "C"]`. There is no `finally` | `lower_block`, `catch_filter` | `syntax.try_without_catch`, `syntax.catch_without_try`, `syntax.catch` |
| Prefix-call functions | `(NAME arg …)` with NAME in `BUILTIN_FUNCTIONS` | `expr.rs` | `check.unknown_function` (+ hint) |
| yield | Only inside a `map` or `poll` body | `lower_stmt` (`BlockCtx::MapOrPoll`) | `syntax.yield` |
| map | The body must `yield`. The only option is `limit N` | `lower_stmt` | `syntax.map_yield`, `syntax.map` |
| poll | Needs `timeout "D"`. `every "D"` is optional | `lower_stmt` | `syntax.poll` |
| dag | Contains only `node` lines. Names are unique, every `after [..]` names a known node, and a node that reads another node must declare it in `after`. No cycles (Kahn) | `lower_stmt`, `check_dag` | `syntax.dag`, `syntax.dag_cycle` |
| with | `with RESOURCE … as NAME`. The resource is an effect word (including `file open PATH mode M` and `file watch …`), `(request.stream …)` or `HANDLE.open` | `lower_with` | `syntax.with` |
| global (0.2.0) | `global NAME = EXPR` at the top level only; NAME is an identifier | `lower.rs` | `syntax.global` |
| import (0.2.0) | `import "PATH" as ALIAS [public]`: a string literal PATH relative to the importing file (absolute → `syntax.import`; there are no URL imports: `"https://…"` is read as a relative path and fails `not_found.import`), an identifier ALIAS other than `rivet`, before the first declaration of the file | `Lowerer::imports` | `syntax.import` |
| Numeric path segments | `xs.0`, `m.rows.1.0`, `${xs.0}` index lists (INC-2026-0009, resolved in 0.2.0) | `expr.rs` | `syntax.expression` (never `assign_map`) |

### Expression parser (`src/features/language/lowering/expr.rs`)

Capy decides where an expression capture begins and ends. `expr.rs` then re-parses the capture's exact
source text, which `Capture::source_text` provides by slicing the source with the span. Byte offsets are
mapped back to file/line/column through `SpanMap`.

```text
 precedence (low → high, all left-associative)
   or
   and
   ==  !=  <  <=  >  >=
   +  -
   *  /  %
   unary:  not  -
   primary: int · float · "string with ${a.b}" · 'string' · `backtick` · [list] · {k: v}
            · dotted.path · (f arg …)  prefix call · (a + b)  grouping
```

`parse_args` splits option tails and effect heads into `Arg::Word` (bare keywords such as `get`, `required`,
`as`) and `Arg::Expr` values.

### IR (`src/domain/ir.rs`)

```text
 CompiledProgram
 ├── entry, root, source_hash ("sha256:…"), warnings[]
 ├── operations[]  Operation
 │     id · kind (operation|pipeline) · name (defaults to id) · description? · private
 │     params[ParamSpec] · output OutputSpec · emits? · receives? · errors[DeclaredError]
 │     body[Stmt] · span · file · calls[] (literal (request "id" …) targets, sorted, deduped)
 ├── connectors[]    Declaration { name, kind, options[OptionLine], span }
 ├── auth_profiles[] Declaration
 ├── globals[]       GlobalDecl { name, expr, span, expr_span }                  (0.2.0)
 ├── global_scopes[] GlobalScope { file, names[] (declaration order), values }   (0.2.0, frozen)
 └── modules[]       ModuleRef { alias, file, public, depth, imports[ImportDecl], policy_ignored } (0.2.0)
                     ImportDecl { path, alias, public, span, target (canonical alias) }
 Operation also gains `module` (the canonical alias, "" for the entry) and `param_spans` (0.2.0).

 Stmt  = Assign{var, rhs, options} | Append | Return | Yield | Emit | Fail{code, details} | Break
       | If | While | For | Try{body, filter, handler} | Dag{options, nodes} | Concurrent{tasks}
       | Iterate | Scope | With{form, source, bind, body} | Effect | Member | Call | Secret | Until
 Rhs   = Expr | Effect(EffectForm) | Member(MemberCall) | Map{item, iter, limit, body}
       | Poll{every, timeout, body}
 EffectForm = { kind: http|file|grpc|command|tcp|unix|pipe|udp|quic|websocket|request_stream
                      |connection|other, head: Arg[], options: OptionLine[], span, effect_ids[] }
 Expr  = Lit | Template[Lit|Path] | Path | List | Object | Call{func,args} | Binary | Not | Neg
```

Effect forms stay generic (a kind, head arguments and option lines). One representation therefore serves
the interpreter, the effect analysis and the I/O manifest.

### Global constants (`language.compile_globals`, 0.2.0)

`compile_program` hands the lowered `GlobalDecl`s to `compile_globals` after the bundle checks. The use case
builds one scope per file, strictly in declaration order, and folds each expression with the pure evaluator in
`src/domain/const_eval.rs`. The evaluator knows literals, lists, objects, operators, comparisons, `${…}` and
the pure built-ins `length`, `base64.*`, `text`, `keys` and `xml.element`.

```text
  GlobalDecl (source order, per file)
     │ same name twice in the file ─────────────────────────▶ check.global_duplicate   (second name)
     │ reads a later global, or itself ─────────────────────▶ check.global_forward_ref (the reference)
     │ env / secret / request / effect / param / local ─────▶ check.global_not_constant (the expression)
     │ a path its earlier globals do not have (headers.nope) ▶ check.global_not_constant
     ▼
  fold(expr, earlier globals of the file)
     │ evaluation fails ────────────────────────────────────▶ its own code (value.division_by_zero, value.type…)
     ▼
  GlobalScope{file, names, values}  (frozen)
     │ every operation of that file:
     │   param / for / map item / with … as / dag node / task / secret NAME ─▶ check.global_shadow
     │   NAME = …  |  NAME += …                                              ─▶ check.global_assign
     ▼
  program.global_scopes ─▶ Interpreter: HashMap<file, Arc<Value>>  (lookup order in SYS-2026-0002)
                        ─▶ effect analysis: globals substituted into targets (SYS-2026-0003)
                        ─▶ highlighter: names classed `global` (SYS-2026-0011)
```

All `check.global_*` errors join the other compile errors (first + `suppressed`, exit 2). A global can never
hold a secret or anything read from the environment. Real diagnostics from scratch bundles:

```text
$ rivet --file genv.rivet check
error[check.global_not_constant]: global `token` cannot read the environment; globals are fixed at load time
  --> genv.rivet:1:16
   |
  1| global token = (env "API_TOKEN")
   |                ^^^^^^^^^^^^^^^^^
  = hint: declare `secret token from env "…" for "https://…"` inside the operation that uses it
exit=2
$ rivet --file gfwd.rivet check
error[check.global_forward_ref]: global `a` reads `b`, which is not declared before it; globals see only earlier globals
  --> gfwd.rivet:1:12
exit=2
$ rivet --file gsh.rivet check
error[check.global_shadow]: parameter `api` reuses the name of a global; globals cannot be shadowed
  --> gsh.rivet:4:11
  = hint: rename the parameter
exit=2
$ rivet --file gas.rivet check
error[check.global_assign]: `n` is a global and globals are read-only
  --> gas.rivet:5:5
  = hint: assign to a new local name instead
exit=2
$ rivet --file gdiv.rivet check
error[value.division_by_zero]: division by zero
  --> gdiv.rivet:1:12
exit=2
$ rivet --file gin.rivet check
error[syntax.global]: `global` is only valid at the top level of a file (use a local assignment inside an operation)
  --> gin.rivet:3:5
exit=2
```

The source excerpts are trimmed after the first error to save space. A clean globals bundle (the
[language guide example](../../manuals/man-2026-0003-language-guide.md#globals)) runs as expected:

```text
$ rivet --file app.rivet request info.show --data '{"page":2}'
{"request_id":"req_013e7a8e55","trace_id":"tr_013e7a8e55","operation":"info.show","type":"result","status":"ok","data":{"url":"https://api.example.com/users?limit=50&page=2","first_retry":429,"accept":"application/json"},"error":null,"effects":"none","data_count":0}
```

### File modules (`language.resolve_imports` and `namespace_modules`, 0.2.0)

`resolve_imports` runs before compilation, in `RuntimeBuilder::build` and in every run-time load. It parses
each file but lowers only its `import` lines. It compiles nothing.

```text
 roots (depth 0): the entry file (alias "") + host-loaded modules (alias = given or file stem)
    │ breadth-first queue
    ▼
 file F (alias A) ── import "PATH" as X [public] ──┐
    │                                              ▼
    │  alias X twice in F ────────────────────────────▶ check.import_duplicate       (exit 2)
    │  PATH resolved against dir(F), normalized lexically
    │      escapes the root ──────────────────────────▶ permission.import_outside_root (exit 3)
    │  already reached? ─yes─▶ ImportDecl.target = its first alias (compile once)
    │  > 256 files or depth > 16 ─────────────────────▶ limit.imports                (exit 5)
    │  read_module(root, PATH): symlink on the way ────▶ permission.import_outside_root
    │                           missing ───────────────▶ not_found.import             (exit 4)
    │  new ModuleRef{alias = A ? "A.X" : "X", depth+1, public:false}
    │  policy.json beside it (other dir than the entry) ▶ policy_ignored = true
    ▼
 find_cycles (DFS over edges) ─────────────────────────▶ check.import_cycle at the first import of the cycle
 mark_public: roots public; a module is public when a chain of `public` imports from a root reaches it
    ▼
 SourceBundle{entry, root, files[every file], modules[ModuleRef]}
```

`namespace_modules` (in `lowering/modules.rs`) then runs inside `compile_program`:

| Step | Rule | Code on violation |
|---|---|---|
| Namespace | Each operation of a module with alias `A` becomes `A.ID`. Namespaces are transitive (`billing.people.get`), except that a file reached twice keeps its shallowest alias | — |
| Call resolution | Literal call targets resolve in the file's own scope: `rivet.*` first, then an import alias (`users.get` → canonical alias), then the file's own short IDs (`(get {…})`), then its own connectors. Calls by ID `(ID {…})` are rewritten to `(request "ID" …)`. Built-in function names win | `check.unknown_operation` |
| Visibility | Operations of a module that no public chain reaches become `private`. They can be called from inside the bundle but are not listed, and a surface call is `not_found.operation`. Another module's `private true` operation cannot be called | `check.unknown_operation` |
| Collisions | A namespaced ID equal to another ID, and connector or auth profile names that repeat across files (the single policy grants them by name) | `check.import_collision` (at the import) |
| Module policy | A `policy.json` beside a module is not loaded | warning `check.module_policy_ignored` |

A module's globals, connectors and auth profiles stay inside it (file-scoped). The whole bundle runs under the
loader's one policy (see [SYS-2026-0003](sys-2026-0003-policy-broker-and-io-manifest.md)). Real captures, from
the three-file bundle of the [language guide](../../manuals/man-2026-0003-language-guide.md#modules-import)
(`app.rivet` imports `users.rivet` as internal and `lib/billing.rivet` as `public`; `billing` imports
`../users.rivet` as `people`):

```text
$ rivet --file app.rivet check
ok: 4 operations, 0 connectors, 0 auth profiles
$ rivet --file app.rivet list
ID               NAME             DESCRIPTION
report.user      report.user      A user and their invoice.
billing.invoice  billing.invoice  An invoice for one user.
$ rivet --file app.rivet graph report.user
report.user               app.rivet:4
├── call users.get        app.rivet:8
└── call billing.invoice  app.rivet:9
    └── call users.get    lib/billing.rivet:9          ← `people.get` resolved to the shallowest alias
$ rivet --file app.rivet request users.get --data '{"id":3}'
{"request_id":"req_01dba357bd","trace_id":"tr_01dba357bd","operation":"users.get","type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"not_found.operation","message":"no operation `users.get`","retryable":false,"operation_id":"users.get"},"effects":"none","data_count":0}
exit=4
```

Import errors, each from a scratch bundle (excerpts trimmed):

```text
error[check.import_cycle]: import cycle: cyc.rivet → a.rivet → cyc.rivet                                  exit=2
error[permission.import_outside_root]: import `../../outside.rivet` resolves outside the runtime root .    exit=3
error[not_found.import]: cannot read missing.rivet: No such file or directory (os error 2)                exit=4
error[syntax.import]: `import` must come before the first declaration of the file                        exit=2
error[syntax.import]: import path `/etc/x.rivet` must be relative to the importing file, e.g. "./users.rivet"
error[syntax.import]: `rivet` is not a valid import alias (an identifier; `rivet` is reserved)
error[check.import_collision]: `u.get` from u.rivet collides with the operation declared at col.rivet:3    exit=2
warning[check.module_policy_ignored]: the policy.json beside sub/m.rivet is ignored: module `m` runs under the loader's policy
  = hint: grant what the module needs in the entry bundle's policy.json (or the host policy)       (check still ok, exit=0)
```

### Run-time module loads and catalog snapshots

A running runtime holds its catalog as `RwLock<Arc<Snapshot>>` (`src/orchestrator/runtime.rs`). A `Snapshot`
bundles the `CatalogSnapshot{version = source_hash, bundle, program}` (`src/domain/modules.rs`) with the
registry, the interpreter, the MCP peer and the OAuth adapter built from it.

```text
 Runtime::load("./users.rivet") | load_as(path, alias) | C rivet_load(rt, path, alias_or_null, &module, &err)
   └─▶ registry.load_module(ModuleLoad{path, alias?})             src/features/registry/load_module.rs
         path outside the root ───────────────────────▶ permission.import_outside_root
         alias = given ∨ file stem; not an identifier / "rivet" ▶ syntax.import
         alias already a namespace ────────────────────▶ check.import_duplicate (use load_as)
         next bundle = current depth-0 roots + {alias, public, depth 0}
         CatalogStore.compile  = resolve_imports + compile_program   (error ─▶ returned, nothing swapped)
         CatalogStore.publish  = Snapshot::build + atomic swap        (loads serialized by load_lock)
         ─▶ ModuleSummary{alias, file, public, operations [short IDs], warnings [module_policy_ignored]}

 time ──────────────────────────────────────────────────────────────────────────────▶
 Snapshot v1  ◀── request A (started before the load; A and its nested calls stay on v1)
        load ─▶ Snapshot v2 ◀── request B (sees users.get)
 a dynamic nested call from A to users.* ─▶ not_found.operation (it is not in v1)
```

The OAuth adapter is reused across a load when the auth profiles are unchanged. Otherwise it is rebuilt, and
pending authorizations of the replaced adapter are lost. Session receipts report the catalog they opened on
([SYS-2026-0007](../runtime/sys-2026-0007-sessions.md)). `Module::operations()` and `describe()` report short
IDs (`get`), while envelopes carry the namespaced `operation` (`users.get`). The Rust and C host APIs are in
[API-2026-0004](../../api/api-2026-0004-rust-library.md) and [API-2026-0007](../../api/api-2026-0007-c-abi.md).

### Syntax highlighting (link)

`language.highlight_source` (`rivet highlight`, `rivet::highlight`, `rivet_highlight`) runs the same
`CapyParser` over one file and classifies tokens from the parser's own spans. It loads no bundle, no imports and
no policy. The token classes, the TextMate grammar generator (`editors/`) and the drift test are described in
[SYS-2026-0011](sys-2026-0011-highlighting-and-grammar-generation.md).

```text
$ rivet highlight app.rivet --format json          (globals bundle; first lines)
{"line":1,"col":1,"len":6,"class":"keyword","text":"global"}
{"line":1,"col":8,"len":3,"class":"global","text":"api"}
{"line":1,"col":19,"len":1,"class":"operator","text":"="}
{"line":1,"col":21,"len":25,"class":"string","text":"\"https://api.example.com\""}
```

## Interfaces

### Ports and use cases

| Item | Signature | File |
|---|---|---|
| `Parser` port | `parse(&SourceFile) -> RivetResult<SyntaxTree>` | `src/domain/ports.rs`, implemented by `CapyParser` |
| `SourceLoader` port | `load(entry) -> RivetResult<SourceBundle>`, `read_module(root, path) -> SourceFile`, `policy_beside(path) -> bool` | implemented by `DiskSourceLoader` |
| `language.resolve_imports` (0.2.0) | `(SourceBundle, &dyn Parser, &dyn SourceLoader) -> SourceBundle` | `resolve_imports.rs` |
| `language.compile_program` | `(SourceBundle, &dyn Parser) -> CompiledProgram` | `compile_program.rs` |
| `language.compile_globals` (0.2.0) | `(CompiledProgram) -> GlobalScope[]` | `compile_globals.rs` |
| `language.highlight_source` (0.2.0) | `(source, format) -> (tokens, Option<error>)` | `highlight_source.rs` (SYS-2026-0011) |
| `CatalogStore` port (0.2.0) | `current() -> Arc<CatalogSnapshot>`, `compile(&SourceBundle) -> CatalogSnapshot`, `publish(CatalogSnapshot)` | `src/domain/ports.rs`, implemented by `RuntimeCatalog` |
| `registry.load_module` (0.2.0) | `(ModuleLoad{path, alias?}, &dyn CatalogStore) -> ModuleSummary` | `load_module.rs` |
| `language.compile_output_spec` | `(SyntaxNode) -> OutputSpec` | `compile_output_spec.rs` (same `typed_decl` lowering the compiler uses) |
| `Registry` port | `describe(CatalogQuery) -> Catalog`, `program()`, `effect_sites()` | implemented by `ProgramRegistry` |
| `registry.describe_operations` | `(CatalogQuery{ids, include_private}) -> Catalog` | `describe_operations.rs` |
| `registry.inspect_outputs` | `(OutputQuery{id?, all}) -> Vec<OutputReport>` | `inspect_outputs.rs` |

### The Capy AST JSON boundary

`capy_parser.rs` reads only these keys from Capy's versioned AST JSON (`schema_version` 1):

```text
 {
   "schema_version": 1,
   "tree": {
     "stmts":  [ { "func": "operation",
                   "span": {start_line, start_col, end_line, end_col},   ← 1-indexed, end_col exclusive
                   "captures": { "id": { "text": "demo.add", "is_expr": false,
                                         "span": {…}, "sub": [ …nodes… ] } },
                   "body":   { "stmts": [ … ] } | null,
                   "closer": { …node… } | null } ],
     "errors": [ { "span": {…} } ]
   },
   "diagnostics": [ { "code": "E0001", "message": "…", "primary": {span}, "help": "…" } ]
 }
```

For each capture, Rivet keeps both Capy's normalised `text` and `source_text`, the exact bytes sliced by
the span. Lowering always reads `source_text` (`SyntaxNode::text`), so `{ready: true}` keeps its source
form. No Capy type crosses into `domain`. A build that sees any other `schema_version` fails with an
internal error.

### CLI

`src/orchestrator/setup_cli.rs` wires four catalog commands. All four need `--file PATH`, and all four
compile the bundle (entry and imports) first. With `--json`, each prints one
[ResponseEnvelope](../../api/api-2026-0006-envelopes.md) whose `operation` is the built-in and whose `data` is the
payload shown below (`--pretty` indents it).

| Command | Reads | Output | Errors |
|---|---|---|---|
| `rivet --file F check [--strict-docs]` | `runtime.program()` | `ok: N operations, C connectors, A auth profiles`. `--json` (0.2.0): envelope `rivet.check`, `data` = `{operations, connectors, auth_profiles, warnings}` | Compile error: exit 2. `--strict-docs` findings (`docs.*`): exit 2 |
| `rivet --file F list [--outputs]` | `runtime.list()` (public entries) | Table: ID, NAME, [OUTPUT], DESCRIPTION. `--json`: envelope `rivet.list`, `data` = `{"operations":[…],"next_cursor":null}` | none |
| `rivet --file F describe ID…` | `runtime.describe(ids)` | Params, output, emits, receives, errors, source, delivery. `--json`: envelope `rivet.describe`, `data` = one object, or an array for several IDs | `not_found.operation`: exit 4 |
| `rivet --file F outputs ID \| --all` | `runtime.outputs(id, all)` | Output type/description/fields, emits, receives, errors. `--json`: envelope `rivet.outputs`, `data` = JSON Schema | Both or neither: `validation.query` exit 2. Unknown/private ID: exit 4 |

Without `--json`, `check` prints compile errors as rendered text with a source excerpt. With `--json`
(0.2.0), `check` and the other commands print an error envelope on stderr (`request_id` and `trace_id` are
`""`, because no request started).

### Verified examples: the demo catalog

These commands were run from `docs/demos/01-catalog` with the 0.2.0 release candidate
(`target/release/rivet`, `cargo build --release --features cli`, main at `8031baa`, macOS). Request and
trace IDs differ on every run.

```text
$ rivet --file app.rivet check
ok: 4 operations, 0 connectors, 0 auth profiles
exit=0

$ rivet --file app.rivet check --strict-docs
ok: 4 operations, 0 connectors, 0 auth profiles
exit=0

$ rivet --file app.rivet list --outputs
ID              NAME                OUTPUT      DESCRIPTION
demo.greet      Greet a person      text        Return a greeting for the supplied person.
demo.add        Add two integers    integer     Add two signed integers and return their sum.
demo.health     Check availability  object      Return a constant readiness response without I/O.
demo.countdown  Count down          object      Emit 3, 2, 1 as data items and then return a summary.
exit=0

$ rivet --file app.rivet describe demo.add
demo.add — Add two integers
Add two signed integers and return their sum.
source   app.rivet:9
delivery unary

params
  a   integer  required    First operand.
  b   integer  default 0   Second operand; defaults to zero.

output  integer  Sum of a and b.
emits    —
receives —
errors   —
exit=0

$ rivet --file app.rivet outputs demo.health
demo.health — Check availability
output  object   Readiness report for this catalog.
  ready   boolean  required  True whenever the host can run pure operations.
emits    —
receives —
errors   —
exit=0

$ rivet --file app.rivet --json describe demo.add
{"request_id":"req_01266ff8b5","trace_id":"tr_01266ff8b5","operation":"rivet.describe","type":"result","status":"ok","data":{"id":"demo.add","name":"Add two integers","description":"Add two signed integers and return their sum.","kind":"operation","input":{"type":"object","properties":{"a":{"type":"integer","description":"First operand."},"b":{"type":"integer","description":"Second operand; defaults to zero.","default":0}},"required":["a"],"additionalProperties":false},"output":{"type":"integer","description":"Sum of a and b."},"emits":null,"receives":null,"errors":[],"delivery":"unary","source":{"file":"app.rivet","line":9}},"error":null,"effects":"none","data_count":0}
exit=0

$ rivet --file app.rivet --json outputs demo.health
{"request_id":"req_01255f22ed","trace_id":"tr_01255f22ed","operation":"rivet.outputs","type":"result","status":"ok","data":{"id":"demo.health","output":{"type":"object","properties":{"ready":{"type":"boolean","description":"True whenever the host can run pure operations."}},"required":["ready"],"additionalProperties":false,"description":"Readiness report for this catalog."},"emits":null,"receives":null,"errors":[]},"error":null,"effects":"none","data_count":0}
exit=0

$ rivet --file app.rivet --json list
{"request_id":"req_0125a429e5","trace_id":"tr_0125a429e5","operation":"rivet.list","type":"result","status":"ok","data":{"operations":[{"id":"demo.greet","name":"Greet a person","description":"Return a greeting for the supplied person.","streaming":false},{"id":"demo.add","name":"Add two integers","description":"Add two signed integers and return their sum.","streaming":false},{"id":"demo.health","name":"Check availability","description":"Return a constant readiness response without I/O.","streaming":false},{"id":"demo.countdown","name":"Count down","description":"Emit 3, 2, 1 as data items and then return a summary.","streaming":true}],"next_cursor":null},"error":null,"effects":"none","data_count":0}
exit=0

$ rivet --file app.rivet --json check
{"request_id":"req_0123ca44d5","trace_id":"tr_0123ca44d5","operation":"rivet.check","type":"result","status":"ok","data":{"operations":4,"connectors":0,"auth_profiles":0,"warnings":0},"error":null,"effects":"none","data_count":0}
exit=0

$ rivet --file app.rivet describe demo.nope
error[not_found.operation]: no operation `demo.nope`
exit=4

$ rivet --file app.rivet outputs
error[validation.query]: pass exactly one of an operation ID or --all
exit=2
```

An operation that declares `emits` or `receives` is `delivery session` in `describe`, and
`"streaming": true` in `list --json`. `demo.countdown` is an example (`RegistryEntry::delivery`).

### Verified examples: syntax and check errors

These scratch files live outside the repository and are shown in full. The bundles do not compile, so
every command exits 2 before anything runs.

```text
$ cat typo.rivet
operaton demo.x
    output integer
    return 1
end
$ rivet --file typo.rivet check
error[syntax.unknown_statement]: unknown statement `operaton`
  --> typo.rivet:1:1
   |
  1| operaton demo.x
   | ^^^^^^^^^^^^^^^
  = hint: did you mean `operation`?
exit=2

$ rivet --file typo.rivet --json list          # an error envelope on stderr
{"request_id":"","trace_id":"","operation":"rivet.list","type":"result","status":"error","data":null,"error":{"kind":"syntax","code":"syntax.unknown_statement","message":"unknown statement `operaton`","retryable":false,"source":{"file":"typo.rivet","line":1,"column":1,"end_line":1,"end_column":16},"hint":"did you mean `operation`?"},"effects":"none","data_count":0}
exit=2

$ rivet --file typo.rivet --json check         # check --json too (0.2.0)
{"request_id":"","trace_id":"","operation":"rivet.check","type":"result","status":"error","data":null,"error":{"kind":"syntax","code":"syntax.unknown_statement","message":"unknown statement `operaton`","retryable":false,"source":{"file":"typo.rivet","line":1,"column":1,"end_line":1,"end_column":16},"hint":"did you mean `operation`?"},"effects":"none","data_count":0}
exit=2
```

```text
$ cat duration.rivet                      $ rivet --file duration.rivet check
operation demo.wait                       error[syntax.duration_unquoted]: `5s` is not a value; durations are quoted strings
    output json                             --> duration.rivet:4:17
    x = http get "https://example.com/"      |
        timeout 5s                          4|         timeout 5s
    end                                      |                 ^^
    return x                                = hint: write "5s"
end                                       exit=2
```

```text
$ cat graph.rivet                         $ rivet --file graph.rivet check
operation demo.a                          error[registry.duplicate_id]: operation ID `demo.a` is declared twice (first at graph.rivet:1)
    output integer                          --> graph.rivet:11:1
    return (request "demo.b" {})             |
end                                        11| operation demo.a
                                             | ^
operation demo.b                          error[check.call_cycle]: operations call each other in a cycle: demo.a → demo.b → demo.a
    output integer                          --> graph.rivet:6:1
    return (request "demo.a" {})             |
end                                         6| operation demo.b
                                             | ^
operation demo.a                          exit=2
    output integer
    return 1
end
```

The following cases are abbreviated to the first diagnostic line. Every one exits 2.

```text
 file (shape)                                  first diagnostic
 ────────────────────────────────────────────  ────────────────────────────────────────────────────────────
 x = request("demo.other", {})                 error[syntax.fcall_style]: `request(…)` is function-call style;
                                                 Rivet uses prefix calls   (hint: write (request arg …) …)
 return "n+1 = ${n + 1}"                       error[syntax.interpolation]: `${n + 1}` is not a dotted path;
                                                 compute it in an assignment first
 return 1  then  output integer                error[syntax.option_after_body]: `output` is a header line and
                                                 must come before the first body statement
                                               error[syntax.output_required]: operation `demo.bad` must declare
                                                 its `output`
 dag ⏎ node a after [b] = 1 ⏎ node b after [a] error[syntax.dag_cycle]: the dag's `after` edges form a cycle
 try ⏎ x = 1 ⏎ (no catch)                      error[syntax.try_without_catch]: `try` must be followed by
                                                 `catch error kind K` or `catch error code "C"`
 yield 1   (in an operation body)              error[syntax.yield]: `yield` only produces a value inside `map`
                                                 or `poll`; use `return` to exit the operation
 2-space indentation                           error[syntax.indent]: indentation of 2 spaces; use 4 spaces (or
                                                 one tab) per level        (one per offending line)
 operation rivet.mine                          error[syntax.reserved_id]: `rivet.*` IDs are reserved for
                                                 built-in operations
 missing `end`                                 error[syntax.e0001]: line 0: expected closer "end"
 return (request "demo.missing" {})            error[check.unknown_operation]: `demo.u` calls unknown operation
                                                 `demo.missing`
 --file missing.rivet                          error[not_found.source]: cannot read missing.rivet: No such file
                                                 or directory (os error 2)                 ← exit 4, not 2
```

`--strict-docs` on an operation with no descriptions and an undeclared `fail` code:

```text
$ cat nodoc.rivet
operation demo.nodoc
    param x integer required
    output integer
    fail "demo.oops"
end
$ rivet --file nodoc.rivet check
ok: 1 operations, 0 connectors, 0 auth profiles
$ rivet --file nodoc.rivet check --strict-docs
error[docs.description]: operation `demo.nodoc` has no description
  --> nodoc.rivet:1:1
   |
  1| operation demo.nodoc
   | ^
error[docs.param_description]: `demo.nodoc` parameter `x` has no description
  …
error[docs.output_description]: `demo.nodoc` output has no description
  …
error[docs.undeclared_error]: `demo.nodoc` can fail with `demo.oops` but declares no `error "demo.oops"` line
  --> nodoc.rivet:4:5
   |
  4|     fail "demo.oops"
   |     ^^^^^^^^^^^^^^^^
exit=2
```

### Verified example: a clean tour of the block forms

```rivet
operation demo.util.double
    name "Double"
    description "Multiply by two."
    private true
    param n integer required description "Input."
    output integer description "Twice n."
    return n * 2
end

operation demo.tour
    name "Grammar tour"
    description "Exercises the Rivet statement forms."
    param items json default [1, 2, 3] description "Numbers to double."
    output object description "Tour result."
        field doubled json required description "Mapped values."
        field greeting text required description "Template string."
    end
    error "demo.empty" description "Raised when items is empty."
    if (length items) == 0
        fail "demo.empty"
    end
    doubled = map item in items limit 2
        v = (request "demo.util.double" {n: item})
        yield v
    end
    dag limit 2 timeout "5s" fail fast
        node first = (request "demo.util.double" {n: 1})
        node second after [first] = (request "demo.util.double" {n: first.result})
    end
    try
        ready = (request "demo.util.double" {n: 2})
    catch error kind validation
        ready = 0
    end
    who = "tour"
    return {doubled: doubled, greeting: "hello ${who}, dag gave ${second.result}"}
end
```

```text
$ rivet --file tour.rivet check
ok: 2 operations, 0 connectors, 0 auth profiles
$ rivet --file tour.rivet list                       # private helper is hidden
ID         NAME          DESCRIPTION
demo.tour  Grammar tour  Exercises the Rivet statement forms.
$ rivet --file tour.rivet describe demo.util.double
error[not_found.operation]: no operation `demo.util.double`
exit=4
$ rivet --file tour.rivet request demo.tour
{"request_id":"req_01a41fbead","trace_id":"tr_01a41fbead","operation":"demo.tour","type":"result","status":"ok","data":{"doubled":[2,4,6],"greeting":"hello tour, dag gave 4"},"error":null,"effects":"none","data_count":0}
$ rivet --file tour.rivet outputs demo.tour
demo.tour — Grammar tour
output  object   Tour result.
  doubled   json     required  Mapped values.
  greeting  text     required  Template string.
emits    —
receives —
errors
  demo.empty   Raised when items is empty.
```

`request_id` and `trace_id` are different on every run.

## Configuration

The compiler and registry have no configuration file and read no environment variables. The inputs are:

| Input | Effect |
|---|---|
| `--file PATH` | Entry file. `SourceBundle.root` is its directory (or `.`): relative paths in source resolve against it, and every import must stay below it |
| `--strict-docs` (on `check`) | Runs `strict_doc_findings` after a successful compile |
| `--json` | A ResponseEnvelope for `list`, `describe`, `outputs` and (0.2.0) `check` |
| `--pretty` (0.2.0) | Indents the JSON output (2 spaces, same key order) |
| `--endpoint URL` | `list`, `describe` and `outputs` go to a running server instead of compiling locally. `check` is refused remotely |
| Library | `Runtime::builder().file(p)` or `.source(path, text, root)` supplies the bundle without a disk read of the entry (imports are still read below the root); `.root(dir)` starts with an empty catalog that `rt.load(…)` fills |
| Limits (0.2.0) | `MAX_FILES = 256` and `MAX_DEPTH = 16` in `resolve_imports.rs` (constants, not configurable) |

The grammar is fixed at build time (`GRAMMAR = include_str!("rivet.capy")`). The Capy version is pinned
in `Cargo.toml`; see ADR-0002.

## Runtime Behaviour

### Compile sequence (inside `RuntimeBuilder::build`)

```text
 CLI / library            DiskSourceLoader      CapyParser          Lowerer            compile_program      Registry
     │ build()                  │                   │                  │                     │                 │
     │── load(entry) ──────────▶│                   │                  │                     │                 │
     │◀── SourceBundle ─────────│                   │                  │                     │                 │
     │── compile_program ──────────────────────────────────────────────────────────────────▶│                 │
     │                          │                   │◀─ parse(file) ───────────────────────── │                 │
     │                          │                   │── SyntaxTree ──────────────────────────▶│                 │
     │                          │                   │                  │◀─ lower_tree ────────│                 │
     │                          │                   │                  │── ops/decls/errors ─▶│                 │
     │                          │                   │                  │   checks + hash      │                 │
     │◀──────────────────────── Ok(CompiledProgram) | Err(first, suppressed[]) ────────────── │                 │
     │── load policy.json, gRPC descriptors, MCP snapshots, effect catalog (see sibling docs) │                 │
     │── ProgramRegistry::new(program).with_imports(..).with_effects(..) ────────────────────────────────────▶ │
```

### Error collection

The `Lowerer` collects every error. It does not stop at the first. `compile_program` returns the first
error with the rest in `RivetError::suppressed`, and the CLI prints up to 20 of them followed by
`… and N more`. When a file has parser diagnostics, the lowerer reports those diagnostics and skips
lowering that file. This avoids a cascade of follow-on errors.

```text
  parse diagnostics? ──yes──▶ report them only ──▶ exit 2
        │no
        ▼
  lowering errors + bundle-check errors (all collected) ──any?──yes──▶ first + suppressed ──▶ exit 2
        │none
        ▼
  CompiledProgram (+ source_hash) ──▶ registry
```

### Did-you-mean for unknown statements

When Capy reports `E0001` at the first column of a line whose first word is not a grammar keyword,
`convert_diagnostic` rewrites the error to `syntax.unknown_statement` and suggests the nearest keyword by
Levenshtein distance (at most `max(2, len/3)`). Other Capy diagnostics keep their code, lower-cased with a
`syntax.` prefix (`syntax.e0001`).

### Catalog reads

```text
 describe(CatalogQuery{ids, include_private})
   entries ─▶ drop private unless include_private ─▶ keep ids (all when empty) ─▶ Catalog
 describe_operations: any requested id missing ─▶ not_found.operation (exit 4 / 404)
                      (a private id is indistinguishable from a missing one)
 inspect_outputs:     id XOR --all, else validation.query (exit 2); public only; sorted by id
```

### Check error codes

These codes are defined in the current code. Unless the table says otherwise, they have kind `syntax`, which
means exit 2 and HTTP 422 on remote surfaces (`ErrorKind::exit_code` in `src/domain/errors.rs`). The 0.2.0 codes
are registered in `domain::errors::LANGUAGE_CODES`.

| Stage | Code | Raised when | Source |
|---|---|---|---|
| Parse | `syntax.unknown_statement` | The first word of a line is not a keyword (with a did-you-mean hint) | `capy_parser.rs` |
| Parse | `syntax.e0001` (`syntax.<capy code>`) | Any other Capy diagnostic, such as a missing `end` or an unexpected token | `capy_parser.rs` |
| Parse | `syntax.unparsed` | Capy recovered an error region but reported no diagnostic | `capy_parser.rs` |
| Parse | `syntax.indent` | Indentation that is not a multiple of 4 spaces and contains no tab | `capy_parser.rs` |
| Expr | `syntax.duration_unquoted` | `5s` written without quotes | `expr.rs` |
| Expr | `syntax.number` | Malformed number literal | `expr.rs` |
| Expr | `syntax.character` | Unexpected character | `expr.rs` |
| Expr | `syntax.string` / `syntax.escape` | Unterminated string / bad escape | `expr.rs` |
| Expr | `syntax.interpolation` | `${…}` is unterminated or is not a dotted path | `expr.rs` |
| Expr | `syntax.expression` | Any other malformed expression | `expr.rs` |
| Lower | `syntax.top_level` | A statement outside operation/pipeline/connector/auth | `lower.rs` |
| Lower | `syntax.operation_id` / `syntax.reserved_id` | Invalid ID (dotted segments, 1–128 ASCII) / ID starts with `rivet.` | `lower.rs` |
| Lower | `syntax.header_order` / `syntax.option_after_body` | Header lines out of order / after the body started, or an option line after statements | `lower.rs` |
| Lower | `syntax.output_required` / `syntax.duplicate_output` | `output` missing / repeated | `lower.rs` |
| Lower | `syntax.duplicate_param` / `syntax.duplicate_error` / `syntax.duplicate_field` | Repeated name at one level | `lower.rs` |
| Lower | `syntax.param_modifier` | Unknown modifier, or both `required` and `default` | `lower.rs` |
| Lower | `syntax.type` / `syntax.fields` / `syntax.field_modifier` | Unknown type / field block on a non-object / bad field modifier | `lower.rs` |
| Lower | `syntax.option_expected` / `syntax.option_misplaced` | A statement where only option lines are allowed / an option outside the start of a resource block | `lower.rs` |
| Lower | `syntax.fcall_style` / `syntax.trailing` | `f(…)` call style / unexpected text after a value | `lower.rs` |
| Lower | `syntax.duration` | A duration that is not a quoted `N(ms\|s\|m\|h)` | `lower.rs` |
| Lower | `syntax.http` / `syntax.file` | Malformed `http METHOD URL` / `file VERB PATH` head | `lower.rs` |
| Parse | `syntax.else_if` / `syntax.else_without_if` | `else COND` / `else` not directly inside an `if` (orphan or second) | `capy_parser.rs` |
| Lower | `syntax.try_without_catch` / `syntax.catch_without_try` / `syntax.catch` | try/catch pairing and filter | `lower.rs` |
| Lower | `check.unknown_function` | A prefix call to a name outside `BUILTIN_FUNCTIONS` (hint: closest built-in) | `expr.rs` |
| Lower | `syntax.yield` / `syntax.map` / `syntax.map_yield` / `syntax.poll` / `syntax.until` | map/poll/yield rules | `lower.rs` |
| Lower | `syntax.dag` / `syntax.dag_cycle` | dag node rules / cycle in `after` edges | `lower.rs` |
| Lower | `syntax.concurrent` / `syntax.group_option` / `syntax.iterate` / `syntax.scope` | Block-form rules and group options (`limit`, `timeout`, `fail fast\|independent`) | `lower.rs` |
| Lower | `syntax.with` / `syntax.secret` / `syntax.statement` | with-resource shape / `secret NAME from env "VAR" for "ORIGIN"` shape / a statement not valid in this position | `lower.rs` |
| Bundle | `registry.duplicate_id` | An ID declared twice (details carry both spans) | `compile_program.rs` |
| Bundle | `check.unknown_operation` | A literal `(request "id" …)` to an ID that is not in the bundle, is not `rivet.*` and is not connector-prefixed | `compile_program.rs` |
| Bundle | `check.call_cycle` | Cycle in the literal call graph (DFS) | `compile_program.rs` |
| Bundle | `check.unknown_auth_profile` / `check.unknown_connector` | An `auth NAME` option or `grpc CONN.Method` naming an undeclared profile or connector | `compile_program.rs` |
| Docs gate | `docs.description` / `docs.param_description` / `docs.output_description` / `docs.field_description` / `docs.undeclared_error` | Only with `check --strict-docs`, for public operations | `lower.rs` `strict_doc_findings` |
| Warning | `docs.undeclared_error` | A `fail "CODE"` without an `error "CODE"` line — always a warning; an error under `--strict-docs` | `compile_program.rs` |
| Warning | `check.unguarded_result` | `return` reads `NODE.result` of a `fail independent` DAG node with no enclosing `if NODE.status …` | `compile_program.rs` `unguarded_results` |
| Lower (0.2.0) | `syntax.global` / `syntax.import` | Malformed or misplaced `global` / `import` line, bad alias, absolute path | `lower.rs` |
| Globals (0.2.0) | `check.global_duplicate` / `check.global_forward_ref` / `check.global_not_constant` | Name twice in a file / reads a later global / not a load-time constant | `compile_globals.rs` |
| Globals (0.2.0) | `check.global_shadow` / `check.global_assign` | An operation binds or assigns a global's name | `compile_globals.rs` |
| Globals (0.2.0) | `value.*` (for example `value.division_by_zero`) | A constant that fails to evaluate keeps its own code (kind validation, exit 2) | `const_eval.rs` |
| Imports (0.2.0) | `check.import_cycle` / `check.import_duplicate` | Import cycle / alias twice in one file, or a host load under a taken alias | `resolve_imports.rs`, `load_module.rs` |
| Imports (0.2.0) | `check.import_collision` | A namespaced ID or a connector/auth name collides across files | `lowering/modules.rs` |
| Imports (0.2.0) | `not_found.import` (exit 4) / `permission.import_outside_root` (exit 3) / `limit.imports` (exit 5) | Missing file / outside the root or through a symlink / more than 256 files or depth over 16 | `resolve_imports.rs`, `source_loader.rs` |
| Warning (0.2.0) | `check.module_policy_ignored` | A `policy.json` sits beside a module outside the entry's directory | `lowering/modules.rs` |

```text
 compile_program
   lowerer.errors non-empty ──▶ Err(first, suppressed = rest)            (exit 2)
   else program.warnings = strict_doc_findings(docs.undeclared_error) + unguarded_results(…)
        └─▶ rivet check prints "warning: <rendered>" on stderr, then "ok: …", exit 0
            (with --strict-docs the undeclared-code warning is reported once, as an error)
```

Load failures around compilation use other kinds. `not_found.source` (exit 4) means the entry file could
not be read. `validation.usage` (exit 2) means `--file` is missing.

## Data and Storage

Nothing is persisted. The `CompiledProgram` and the `ProgramRegistry` entries live in memory behind `Arc`.
In 0.1.0 they lasted for the life of the `Runtime`. From 0.2.0 they last for the life of a catalog snapshot: a
run-time module load publishes a new snapshot, and the old one is dropped when the last request holding it
finishes.

### Declared output model (`src/domain/outputs.rs`)

```text
 ValueSpec = text | integer | number | boolean | bytes | json
           | object { fields: FieldSpec[], open: bool }
           | list(ValueSpec)
 FieldSpec   { name, spec, required, description? }
 OutputSpec  { spec, description? }            default when absent: json (but `output` is required)
 ParamSpec   { name, spec, required, default?, min?, max?, enum_values?, description? }
 DeclaredError { code, description? }

 JSON Schema projection (ValueSpec::to_json_schema):
   text    → {"type":"string"}
   bytes   → {"type":"object","properties":{"$type":{"const":"bytes"},"base64":{"type":"string"}},
              "required":["$type","base64"]}
   object  → properties + required, additionalProperties:false unless `open true`
   params  → one object schema: required[], default, min/max, enum, description
```

### Registry entry (`src/infra/registry.rs` `entry_of`)

```text
 Operation ───────────────▶ RegistryEntry
   id, name, description      id, name, description, kind, private
   params, output             params, output, emits, receives, errors,
   emits, receives, errors    emits_description, receives_description (since 2a751ab), source (span)
   span                       raw_input_schema / raw_output_schema = None  (set only for MCP imports)
```

The declared `emits`/`receives` descriptions are kept on the entry (`emits_description`, `receives_description`)
and shown by `outputs`/`describe` (`emits   integer  One countdown value per item.`) and as `description` in the
item JSON Schema on REST, MCP and the library (commit `2a751ab`).

`RegistryEntry::streaming()` is true when `emits` or `receives` is present. `delivery()` then returns
`session`; otherwise it returns `unary`.

## Dependencies

| Dependency | Use |
|---|---|
| `capy_core` (git, pinned rev; ADR-0002) | `Library::new`, `Library::parse`, `ast_json::to_json` |
| `serde_json` | Reading the AST JSON and producing JSON Schema |
| `sha2` | `source_hash` |
| `clap` (CLI) | Command and flag parsing in `setup_cli.rs` |

Internal consumers of `CompiledProgram` and `Registry` are the interpreter (SYS-2026-0002), the effect
catalog and policy tools (SYS-2026-0003), serve surfaces (SYS-2026-0004) and MCP connectors (SYS-2026-0009).

## Deployment

The grammar and compiler are part of the single `rivet` binary and library crate. They need no separate
deployment step. The grammar is embedded, so a binary always parses with the grammar it was built with.
To change the grammar, edit `src/infra/rivet.capy` and rebuild.

## Security Boundaries

- **No execution at compile time.** Compilation never calls operations, reads files, dials or spawns
  anything (`compile_program` is annotated "performs no I/O"). The only expressions it evaluates are
  `global` constants, with a pure evaluator that has no environment, secret, request or effect access.
- **Bootstrap reads only.** `DiskSourceLoader` reads the `--file` path and, from 0.2.0, each imported or
  host-loaded module through `read_module`. Every module path is normalized lexically against the importing
  file, must stay below the runtime root, and must not cross a symlink (`permission.import_outside_root`).
  `policy_beside` only tests whether a `policy.json` exists; it never reads it. The contract declares these as
  `vhco:file read *.rivet` and `vhco:file read policy.json`. They are listed in the BOOTSTRAP section of
  `rivet io --include-bootstrap`, one row per file (SYS-2026-0003).
- **One policy.** A module never brings its own policy: the loader's policy governs every file.
- **Globals cannot hold secrets.** `env` and `secret` are refused in a global (`check.global_not_constant`).
- **Capy is isolated.** Only `src/infra/capy_parser.rs` imports Capy, and it depends only on the versioned
  AST JSON. The grammar has no host callbacks.
- **Reserved namespace.** Source cannot declare `rivet.*` IDs (`syntax.reserved_id`), so it cannot shadow
  built-ins.
- **Private operations.** `private true` entries are filtered out of every external catalog read.
  A private ID and a missing ID both produce the same `not_found.operation`, which avoids disclosing that
  the private operation exists.
- **Descriptions are data.** Names and descriptions are literal strings. They are never interpolated or
  executed.

## Observability

- Diagnostics carry `file:line:column` spans. The rendered form shows a source excerpt with a caret
  underline and an optional `= hint:` line. The JSON form carries `source {file, line, column, end_line,
  end_column}`.
- `rivet check` prints counts (`ok: N operations, C connectors, A auth profiles`) and, before them, every
  entry of `program.warnings` as a `warning: …` line on stderr (the rendered error form after the prefix,
  e.g. `warning[check.unguarded_result]: …`).
- `source_hash` (`sha256:…`) pins requests, sessions and manifests to the exact program. See SYS-2026-0002
  and SYS-2026-0003.
- Compilation emits no trace events. Tracing starts when a request starts.

## Known Limitations

- **Module imports are local files only.** There are no URL imports. A `"https://…"` path is read as a
  relative file path and fails `not_found.import`; it is not refused as `syntax.import`.
- **Modules cannot be unloaded.** A snapshot only grows; `rivet_module_free` frees the handle, not the module.
- **Missing descriptions are not warnings.** They are reported only under `--strict-docs`, where they are
  errors.
- **Static call checks cover only literal targets.** A computed `(request id …)` target is bounded at run
  time by the call-depth limit (SYS-2026-0002).
- **Some Capy messages are generic.** Most are now mapped: since INC-2026-0009 (resolved in 0.2.0), an
  unparsable value is `syntax.expression` ("the value assigned to `x` does not parse", with a bracket hint), never
  a message naming `assign_map`. Other Capy diagnostics keep the generic `syntax.eNNNN` text.
- **Option semantics are checked late.** Most option arguments are validated by the adapter when the
  operation runs, not by `check`.
- **No `finally`.** try/catch has no `finally` clause (0.1.0 and 0.2.0).

## Last Verified Version

0.2.0-rc (main at `8031baa`, 2026-09-29, macOS). The 0.2.0 sections were checked against
`resolve_imports.rs`, `lowering/modules.rs`, `compile_globals.rs`, `load_module.rs`, `source_loader.rs` and
`runtime.rs`. The captures were produced by running `target/release/rivet` (built with
`cargo build --release --features cli`) against `docs/demos/01-catalog/app.rivet` and against scratch bundles kept
outside the repository: the globals and modules bundles of MAN-2026-0003, plus `genv`, `gfwd`, `gsh`, `gas`,
`gdiv`, `gin`, `cyc`, `out`, `miss`, `late`, `abs`, `res`, `col`, `pol`, `typo`, `br` and `tour`. Outputs are
pasted as printed; some source excerpts are trimmed where marked. Request and trace IDs vary between runs.

History: 0.1.0-dev was first verified at f40d4aa and re-verified at 829ca43 (`if … else … end`,
`check.unknown_function`, the `check` warnings). The rendered diagnostics of the 0.1.0 scratch bundles
(duration, graph, fcall, tpl, dag, try, yield, indent, noend, reserved, unknown, nodoc) are unchanged in 0.2.0.

## Related Documents

- [PROP-2026-0001: Rivet runtime (approved proposal)](../../proposals/implemented/prop-2026-0001-rivet-runtime.md)
- [PLAN-2026-0001: v0.1.0 implementation and release](../../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) (deliverable D-15)
- [ADR-0002: Rust crate selection](../../decisions/adr-0002-rust-crate-selection.md) (pinned `capy-core`)
- [RES-2026-0001: Capy grammar spike](../../research/res-2026-0001-capy-grammar-spike.md)
- [REF-2026-0002: Language and usage reference](../../references/ref-2026-0002-language-and-usage.md)
- [SYS-2026-0002: Execution scopes and DAG](../runtime/sys-2026-0002-execution-scopes-and-dag.md)
- [SYS-2026-0003: Policy broker and I/O manifest](sys-2026-0003-policy-broker-and-io-manifest.md)
- [SYS-2026-0004: Surfaces and serve](sys-2026-0004-surfaces-and-serve.md)
- [SYS-2026-0009: MCP client connectors](../integrations/sys-2026-0009-mcp-client-connectors.md)
- [SYS-2026-0010: FFI surface and packaging](sys-2026-0010-ffi-surface-and-packaging.md) (`rivet_load`, module handles)
- [SYS-2026-0011: Highlighting and grammar generation](sys-2026-0011-highlighting-and-grammar-generation.md)
- [PROP-2026-0002](../../proposals/implemented/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md) (R7–R9 globals, R19–R24 modules) and [PLAN-2026-0002](../../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) (D-31)
- [API-2026-0006: Envelopes](../../api/api-2026-0006-envelopes.md) (the `--json` output shape)
- [MAN-2026-0003: Language guide](../../manuals/man-2026-0003-language-guide.md) (Globals, Modules)
- [Demos](../../demos/README.md) (`01-catalog`)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial current-state document (PLAN-2026-0001 D-15). |
| 2 | 2026-09-28 | Claude | TASK-092 drift fix for the fix batch (829ca43, 2a751ab: emits/receives descriptions kept): `if … else … end` grammar and codes, `check.unknown_function` with did-you-mean, `check` warnings (`docs.undeclared_error`, `check.unguarded_result`), infix inside literals, `with file open`; obsolete limitations removed. |
| 3 | 2026-09-29 | Claude | PLAN-2026-0002 D-31/D-47 (TASK-073, TASK-070): `global` lowering and `language.compile_globals`; `language.resolve_imports` (resolution, compile-once, cycles, limits, visibility) and `namespace_modules`; `registry.load_module` and catalog snapshots; highlighter link; `--json` outputs as envelopes (`check --json` added); 0.2.0 codes; bootstrap reads of modules; limitations updated; re-verified against the 0.2.0-rc. |
