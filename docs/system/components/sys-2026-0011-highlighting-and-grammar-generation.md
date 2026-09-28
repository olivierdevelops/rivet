---
document_id: SYS-2026-0011
title: "Syntax highlighting and TextMate grammar generation"
document_type: system
status: active
created_date: 2026-09-29
last_updated: 2026-09-29
document_revision: 1
authors: [Claude]
owner: Project maintainer
component_owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, cli, library, ffi]
affected_versions:
  from: "0.2.0"
  to: null
last_verified_version: "0.2.0-rc (main at e7ed8ed)"
next_review_date: 2026-10-29
review_cycle: on-release
confidentiality: internal
scope: How Rivet 0.2.0 highlights `.rivet` sources — the keyword table generated from rivet.capy, the TextMate grammar and VS Code extension, the parser-span tokenizer (`language.highlight_source`), its token model and renderers, the drift and golden tests, and the Python-only packaging of the .vsix.
reason: PLAN-2026-0002 row D-16 (TASK-073); DOCUMENTATION §31 requires system documentation of the implemented highlighting pipeline; checked against editors/, src/features/language/highlight_source.rs and tests/conformance_highlight.rs.
related_documents: [PROP-2026-0002, PLAN-2026-0002, MAN-2026-0010, SYS-2026-0001, API-2026-0004, API-2026-0007, REF-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, system, highlighting, textmate, vscode, grammar, tokens]
---

# Syntax highlighting and TextMate grammar generation

> **Status:** Active
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0 and later
> **Owner:** Project maintainer
> **Affected Components:** language, editors, cli, library, ffi
> **Last Verified Version:** 0.2.0-rc (main at e7ed8ed)

## Summary

Two highlighters share **one keyword table**:

```text
                        src/infra/rivet.capy  (Capy grammar: every `arg literal "…"`, block sections)
                                   │ editors/gen_grammar.py  (+ CLASS_TABLE: declaration, control, option,
                                   │                           modifier, type, effect, builtin, …)
                                   ▼
                        editors/keywords.json  (GENERATED; the one table)
                 ┌─────────────────┴──────────────────────────┐
                 ▼                                             ▼ include_str!
   editors/rivet.tmLanguage.json (GENERATED)        src/features/language/highlight_source.rs
   editors/vscode/syntaxes/… (copy)                   parser spans + table ─▶ [HighlightToken]
   └─▶ package_vsix.py ─▶ dist/rivet-<ver>.vsix        ├─▶ rivet highlight (ansi | html | json)
       (VS Code, other TextMate editors: regex)        ├─▶ rivet::highlight::{tokens, highlight, render}
                                                       └─▶ rivet_highlight (C ABI)
```

## Responsibilities

- Classify every literal of the grammar once (`keywords.json`), and fail the build when a new literal has no class.
- Give editors a TextMate grammar (regex) and VS Code editing behaviour (comments, brackets, indentation).
- Give tools exact tokens computed from the real parser, including context the regex cannot know (globals,
  operation IDs, effects).

## Boundaries and Non-Responsibilities

- No language server, no semantic diagnostics in editors (use `rivet check`).
- The TextMate grammar is an approximation; `rivet highlight` is authoritative.

## Architecture

### Generator

`editors/gen_grammar.py` (standard library only) reads `rivet.capy`, extracts literals and block sections, maps
them through `CLASS_TABLE`, and writes `keywords.json` (keys: `literals`, `unclassified`, `classes`,
`option_modifiers`, `statements`, `symbol_operators`, `highlight_classes`, `highlight_class_of`, `scopes`) and the
grammar (scope name `source.rivet`). `--check` exits 1 when an output is stale.

### Tokenizer (`language.highlight_source`)

```text
 SourceFile ─parser.parse─▶ SyntaxTree (statement nodes + capture spans)
     │   per node: leading keyword · captures (id, var, name, value, cond, rest…) · glue literals (`in`, `=`, `map`)
     │   lex each span ─▶ classify with keywords.json
     ▼
 gap scan over text no node covers: `#` comments, `else`, the `end` of a flat option line's sub-block
     ▼
 syntax error? ─yes─▶ keep tokens starting before the earliest diagnostic + that diagnostic (syntax.*)
     ▼
 [HighlightToken {line, col, len, class, text}]  sorted, non-overlapping, one line each
```

Token model decisions (PLAN-2026-0002 findings): `col`/`len` count characters, 1-based; multi-line backtick
strings split per line; brackets, commas, colons, whitespace and object keys are not tokens; `true`/`false`/
`null`, built-ins after `(` and statement keywords are `keyword`; an effect head and its verb (`http get`) are
`effect`; names declared by `global` are `global` wherever referenced (`retry_on.0`); `(ID …)` calls are
`operation_id`.

Classes (12): `keyword`, `option`, `type`, `effect`, `string`, `interpolation`, `number`, `comment`,
`operation_id`, `global`, `variable`, `operator`.

### Renderers (`domain::highlight::render`, pure)

| Format | Output |
|---|---|
| `json` | one `{"line","col","len","class","text"}` object per line |
| `html` | `<pre class="rv-source"><code>` … `<span class="rv-CLASS">` (HTML-escaped; untokenized text copied) |
| `ansi` | SGR colours per class (e.g. keyword `1;35`, global `1;33`, string `32`, comment `90`, operator `37`) |

## Interfaces

| Interface | Entry |
|---|---|
| CLI | `rivet highlight PATH [--format ansi\|html\|json]` (default ansi on a TTY, json otherwise) |
| Rust | `rivet::highlight::tokens(src)`, `tokens_of(path, src)`, `highlight(src, fmt)`, `render(src, &tokens, fmt)` |
| C | `rivet_highlight(source, format)` (`json` when NULL; a syntax error adds an error-envelope line) |
| VS Code | extension `rivet.rivet`: language `rivet` for `*.rivet`, grammar `source.rivet`, `language-configuration.json` |

## Configuration

None at run time. The class of each literal is `CLASS_TABLE` in `gen_grammar.py`.

## Runtime Behaviour

`rivet highlight` loads no bundle and no policy. The keyword table is parsed once per process (`OnceLock`). A
missing file is `validation.usage` (exit 2); a syntax error prints the partial tokens on stdout and the
diagnostic on stderr (exit 2).

## Data and Storage

Generated files in the repository: `editors/keywords.json`, `editors/rivet.tmLanguage.json`,
`editors/vscode/syntaxes/rivet.tmLanguage.json`. `keywords.json` is also packaged in the crate (`include` in
`Cargo.toml`). The `.vsix` goes to `dist/` (git-ignored).

## Dependencies

Python 3 (generator, drift check, grammar check, packaging); no Node.js. The Rust side uses the Capy parser.

## Deployment

`python3 editors/vscode/package_vsix.py` writes a deterministic `.vsix` (sorted entries, fixed timestamps,
`.vscodeignore` honoured): `[Content_Types].xml`, `extension.vsixmanifest`, `extension/{package.json, README.md,
language-configuration.json, syntaxes/rivet.tmLanguage.json}`. `package.json` `version` must equal the workspace
version. Verified: `code --extensions-dir DIR --install-extension dist/rivet-0.1.0.vsix` lists `rivet.rivet@0.1.0`.

## Security Boundaries

The extension is declarative (no code). The highlighter reads one file and has no effects.

## Observability

Tests are the monitor:

| Test | What it guards |
|---|---|
| `t13_keyword_table_has_no_drift` | `editors/check_keywords.py`: every `rivet.capy` literal classified; outputs up to date; Rust lowering tables agree |
| `t13_textmate_grammar_scopes_every_sample` | `editors/tests/check_grammar.py`: the regexes scope every keyword in the REF-2026-0002 blocks and demo apps |
| `t13_vsix_packages_with_the_workspace_version` | packaging and version sync |
| `t14_cli_goldens_for_ansi_html_and_json` | `tests/fixtures/highlight/app.{ansi,html,jsonl}` |
| `t14_syntax_error_gives_partial_tokens_and_exit_2` | `broken.rivet` / `broken.jsonl` |
| `t14_every_reference_and_demo_token_lies_inside_the_source` | span validity over the whole corpus |

## Known Limitations

- The grammar test engine is Python, not Oniguruma; the grammar sticks to syntax both support.
- Capy keeps no node for some `end`/`else` words; the gap scan classifies them.

## Last Verified Version

0.2.0-rc (main at `e7ed8ed`, 2026-09-29): CLI outputs in MAN-2026-0010 captured; `.vsix` packaged and installed.

## Related Documents

- [MAN-2026-0010](../../manuals/man-2026-0010-editor-support-and-highlighting.md) — user guide
- [SYS-2026-0001](sys-2026-0001-compiler-and-catalog.md) — the parser and compiler
- [PLAN-2026-0002](../../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) D-16

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-29 | Claude | Initial system document (TASK-073, D-16) |
