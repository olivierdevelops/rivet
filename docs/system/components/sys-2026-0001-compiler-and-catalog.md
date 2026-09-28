---
document_id: SYS-2026-0001
title: "Rivet compiler and operation catalog"
document_type: system
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 2
authors: [Claude]
owner: Project maintainer
component_owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, registry]
affected_versions:
  from: "0.1.0"
  to: null
last_verified_version: "0.1.0-dev (commit 829ca43)"
next_review_date: 2026-10-28
review_cycle: on-release
confidentiality: internal
scope: How Rivet turns a .rivet entry file into an immutable, described operation catalog (grammar, Capy AST boundary, lowering to IR, compile-time checks, declared outputs, registry) and how check/list/describe/outputs expose it.
reason: Every surface (CLI, HTTP, WebSocket, MCP, library) serves the same compiled catalog; maintainers need one current-state description of where parsing ends, what lowering enforces, which check codes exist and how the catalog is read.
related_documents: [PROP-2026-0001, PLAN-2026-0001, ADR-0002, RES-2026-0001, SYS-2026-0002, SYS-2026-0003, SYS-2026-0004, SYS-2026-0009]
supersedes: null
superseded_by: null
tags: [rivet, system, compiler, capy, grammar, ir, registry, catalog, outputs]
---

# Rivet compiler and operation catalog

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** language, registry
> **Last Verified Version:** 0.1.0-dev (commit 829ca43)

## Summary

The compiler turns one `.rivet` entry file into a `CompiledProgram`: a frozen list of operations, connector
declarations and auth profiles, pinned by a SHA-256 source hash. The registry wraps that program in an
immutable catalog of `RegistryEntry` rows. Every surface reads the same rows: `rivet list`, `describe` and
`outputs`, `/v1/operations`, MCP `tools/list` and `Runtime::list/describe/outputs`. Compilation performs
no I/O and runs nothing. The only file read is the host bootstrap read of the entry file.

```text
  app.rivet ──▶ DiskSourceLoader ──▶ SourceBundle ──▶ CapyParser ──▶ Capy AST JSON (schema_version 1)
  (bytes)       (bootstrap read)     (entry, root,    (embedded        │
                                      files[])         rivet.capy)     ▼
                                                                  SyntaxTree  (Rivet-owned, spans)
                                                                       │ Lowerer (lower.rs + expr.rs)
                                                                       ▼
                                                                  CompiledProgram (typed IR)
                                                                       │ bundle checks + sha256
                                                                       ▼
                                                                  ProgramRegistry (RegistryEntry[])
                                                                       │
                     ┌──────────────┬──────────────┬───────────────────┼──────────────┐
                     ▼              ▼              ▼                   ▼              ▼
                 rivet list    rivet describe  rivet outputs     /v1/operations   MCP tools/list
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
| CLI projection | `src/orchestrator/setup_cli.rs` | `check` (warnings on stderr), `list`, `describe`, `outputs`; `graph` reads the same registry (`features/audit/build_graph.rs`, SYS-2026-0003) |

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
 └────────────┘  not_found.source │  entry = PATH                 │  (only file read; exit 4 on failure)
                 (exit 4)         │  root  = dirname(PATH) or "." │
                                  │  files = [ {path, text} ]     │
                                  └──────────────┬───────────────┘
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
 │   anything else      ─▶ syntax.top_level                                      │
 │   expressions        ─▶ expr.rs parse_expr / parse_args (SpanMap for spans)   │
 └──────────────────────────────────────┬────────────────────────────────────────┘
                                        ▼
 ┌───────────────────────────────────────────────────────────────────────────────┐
 │ compile_program            src/features/language/compile_program.rs           │
 │   check_duplicates   registry.duplicate_id                                    │
 │   check_calls        check.unknown_operation, check.call_cycle                │
 │   check_references   check.unknown_auth_profile, check.unknown_connector      │
 │   errors? ─▶ Err(first error, others in `suppressed`)  (exit 2)               │
 │   source_hash = "sha256:" + H(path ‖ 0 ‖ text ‖ 0 …)                          │
 └──────────────────────────────────────┬────────────────────────────────────────┘
                                        ▼  Arc<CompiledProgram>
 ┌───────────────────────────────────────────────────────────────────────────────┐
 │ Runtime::assemble ─▶ ProgramRegistry::new(program)   src/infra/registry.rs    │
 │                      .with_imports(mcp entries) .with_effects(effect catalog) │
 │   entries = program.operations.map(entry_of)   (built once, never mutated)    │
 └───────────────────────────────────────────────────────────────────────────────┘
```

The whole pipeline runs inside `RuntimeBuilder::build` (`src/orchestrator/runtime.rs`). It runs before any
command, so every CLI command except `--endpoint` mode rejects a bundle that does not compile.

### Grammar (`src/infra/rivet.capy`)

The grammar declares one Capy `function` for each statement shape. Capy decides where a statement and
its captured values start and end. Rivet lowering decides what they mean. The grammar has line comments
(`#`) and an `OpId` pattern type (`^[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z_][A-Za-z0-9_]*)*$`). Blocks close
with `end`. The one exception is `try`, which closes at the dedent (`block_dedent`); lowering then pairs
it with the `catch … end` sibling that follows.

```text
 Top-level declarations           Header lines (inside operation)        Body statements
 ───────────────────────          ─────────────────────────────          ─────────────────────────────
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
 └── auth_profiles[] Declaration

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

## Interfaces

### Ports and use cases

| Item | Signature | File |
|---|---|---|
| `Parser` port | `parse(&SourceFile) -> RivetResult<SyntaxTree>` | `src/domain/ports.rs`, implemented by `CapyParser` |
| `SourceLoader` port | `load(entry) -> RivetResult<SourceBundle>` | implemented by `DiskSourceLoader` |
| `language.compile_program` | `(SourceBundle, &dyn Parser) -> CompiledProgram` | `compile_program.rs` |
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
compile the bundle first.

| Command | Reads | Output | Errors |
|---|---|---|---|
| `rivet --file F check [--strict-docs]` | `runtime.program()` | `ok: N operations, C connectors, A auth profiles` | Compile error: exit 2. `--strict-docs` findings (`docs.*`): exit 2 |
| `rivet --file F list [--outputs]` | `runtime.list()` (public entries) | Table: ID, NAME, [OUTPUT], DESCRIPTION. `--json`: `{"operations":[…],"next_cursor":null}` | none |
| `rivet --file F describe ID…` | `runtime.describe(ids)` | Params, output, emits, receives, errors, source, delivery. `--json`: one object, or an array for several IDs | `not_found.operation`: exit 4 |
| `rivet --file F outputs ID \| --all` | `runtime.outputs(id, all)` | Output type/description/fields, emits, receives, errors. `--json`: JSON Schema | Both or neither: `validation.query` exit 2. Unknown/private ID: exit 4 |

`check` ignores `--json` for compile errors: they are always printed as rendered text with a source
excerpt. The other commands print a JSON error envelope on stderr when `--json` is set.

### Verified examples: the demo catalog

These commands were run from `docs/demos/01-catalog`. The binary was `target/debug/rivet` at commit f40d4aa.

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
{"id":"demo.add","name":"Add two integers","description":"Add two signed integers and return their sum.","kind":"operation","input":{"type":"object","properties":{"a":{"type":"integer","description":"First operand."},"b":{"type":"integer","description":"Second operand; defaults to zero.","default":0}},"required":["a"],"additionalProperties":false},"output":{"type":"integer","description":"Sum of a and b."},"emits":null,"receives":null,"errors":[],"delivery":"unary","source":{"file":"app.rivet","line":9}}
exit=0

$ rivet --file app.rivet --json outputs demo.health
{"id":"demo.health","output":{"type":"object","properties":{"ready":{"type":"boolean","description":"True whenever the host can run pure operations."}},"required":["ready"],"additionalProperties":false,"description":"Readiness report for this catalog."},"emits":null,"receives":null,"errors":[]}
exit=0

$ rivet --file app.rivet --json list
{"operations":[{"id":"demo.greet","name":"Greet a person","description":"Return a greeting for the supplied person.","streaming":false},{"id":"demo.add","name":"Add two integers","description":"Add two signed integers and return their sum.","streaming":false},{"id":"demo.health","name":"Check availability","description":"Return a constant readiness response without I/O.","streaming":false},{"id":"demo.countdown","name":"Count down","description":"Emit 3, 2, 1 as data items and then return a summary.","streaming":true}],"next_cursor":null}
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

$ rivet --file typo.rivet --json list          # non-check commands honour --json
{"request_id":"","trace_id":"","error":{"kind":"syntax","code":"syntax.unknown_statement","message":"unknown statement `operaton`","retryable":false,"effects":"none","source":{"file":"typo.rivet","line":1,"column":1,"end_line":1,"end_column":16},"hint":"did you mean `operation`?"}}
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
$ rivet --file tour.rivet request demo.tour --params '{}'
{"request_id":"req_01c9a4acdd","trace_id":"tr_01c9a4acdd","result":{"doubled":[2,4,6],"greeting":"hello tour, dag gave 4"},"data_count":0,"effects":"none"}
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
| `--file PATH` | Entry file. `SourceBundle.root` is its directory (or `.`), and relative paths in source resolve against it |
| `--strict-docs` (on `check`) | Runs `strict_doc_findings` after a successful compile |
| `--json` | JSON output for `list`, `describe` and `outputs`. `check` output is unchanged |
| `--endpoint URL` | `list`, `describe` and `outputs` go to a running server instead of compiling locally. `check` is refused remotely |
| Library | `Runtime::builder().file(p)` or `.source(path, text, root)` supplies the bundle without a disk read |

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

These codes are defined in the current code. All have kind `syntax`, which means exit 2 and HTTP 422 on
remote surfaces (`ErrorKind::exit_code` in `src/domain/errors.rs`).

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

Nothing is persisted. The `CompiledProgram` and the `ProgramRegistry` entries live in memory behind `Arc`
for the life of the `Runtime`. A new catalog requires a new `Runtime`.

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

- **No execution at compile time.** Compilation never evaluates expressions, calls operations, reads
  other files, dials or spawns anything (`compile_program` is annotated "performs no I/O").
- **Single bootstrap read.** `DiskSourceLoader` reads exactly the `--file` path. It is declared in the
  contract as `vhco:file read *.rivet`.
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
  e.g. `warning: error[check.unguarded_result]: …`).
- `source_hash` (`sha256:…`) pins requests, sessions and manifests to the exact program. See SYS-2026-0002
  and SYS-2026-0003.
- Compilation emits no trace events. Tracing starts when a request starts.

## Known Limitations

- **Single-file bundles.** `DiskSourceLoader` loads only the entry file (`files` has one element), and
  the grammar has no import form (`import "x.rivet"` is `syntax.unknown_statement`). `compile_program` does loop over `files`, so a library caller that
  passes a `SourceBundle` directly could give it more than one file.
- **Missing descriptions are not warnings.** They are reported only under `--strict-docs`, where they are
  errors.
- **Static call checks cover only literal targets.** A computed `(request id …)` target is bounded at run
  time by the call-depth limit (SYS-2026-0002).
- **Some Capy messages are generic.** For example, an unclosed bracket may be reported as
  "expected `+=`, `.`, or `=` … in `assign_map`".
- **Option semantics are checked late.** Most option arguments are validated by the adapter when the
  operation runs, not by `check`.
- **No `finally`.** try/catch has no `finally` clause in 0.1.0.

## Last Verified Version

0.1.0-dev (commit 829ca43). First verified at f40d4aa by reading the listed source files and by running
`target/debug/rivet` `check`, `list`, `describe`, `outputs` and `request` against
`docs/demos/01-catalog/app.rivet`, and against scratch bundles (typo, duration, graph, header, fcall, tpl, dag,
try, yield, indent, noend, reserved, unknown, nodoc, tour) kept outside the repository. Re-verified at 829ca43
for `if … else … end`, `syntax.else_if`/`syntax.else_without_if`, `check.unknown_function`, the two `check`
warnings, infix inside literals and `import`/`finally` refusals, using scratch bundles. Outputs are pasted exactly as printed.
Request and trace IDs vary between runs.

## Related Documents

- [PROP-2026-0001: Rivet runtime (approved proposal)](../../proposals/approved/prop-2026-0001-rivet-runtime.md)
- [PLAN-2026-0001: v0.1.0 implementation and release](../../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) (deliverable D-15)
- [ADR-0002: Rust crate selection](../../decisions/adr-0002-rust-crate-selection.md) (pinned `capy-core`)
- [RES-2026-0001: Capy grammar spike](../../research/res-2026-0001-capy-grammar-spike.md)
- [REF-2026-0002: Language and usage reference](../../references/ref-2026-0002-language-and-usage.md)
- [SYS-2026-0002: Execution scopes and DAG](../runtime/sys-2026-0002-execution-scopes-and-dag.md)
- [SYS-2026-0003: Policy broker and I/O manifest](sys-2026-0003-policy-broker-and-io-manifest.md)
- [SYS-2026-0004: Surfaces and serve](sys-2026-0004-surfaces-and-serve.md)
- [SYS-2026-0009: MCP client connectors](../integrations/sys-2026-0009-mcp-client-connectors.md)
- [Demos](../../demos/README.md) (`01-catalog`)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial current-state document (PLAN-2026-0001 D-15). |
| 2 | 2026-09-28 | Claude | TASK-092 drift fix for the fix batch (829ca43, 2a751ab: emits/receives descriptions kept): `if … else … end` grammar and codes, `check.unknown_function` with did-you-mean, `check` warnings (`docs.undeclared_error`, `check.unguarded_result`), infix inside literals, `with file open`; obsolete limitations removed. |
