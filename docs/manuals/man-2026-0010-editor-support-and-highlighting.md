---
document_id: MAN-2026-0010
title: "Editor support and syntax highlighting for .rivet files"
document_type: manual
status: active
created_date: 2026-09-29
last_updated: 2026-09-29
document_revision: 1
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, cli, library, ffi]
affected_versions:
  from: "0.2.0"
  to: null
applicable_environments: [development]
audience: [script-authors, tool-authors, documentation-writers, contributors]
scope: Installing the local VS Code extension (.vsix built with Python, no Node.js), using the TextMate grammar in other editors, `rivet highlight` (ansi, html, json), `rivet::highlight` and `rivet_highlight`, using the HTML output in a docs pipeline, and regenerating the grammar from rivet.capy.
reason: PLAN-2026-0002 row D-14 (TASK-072) — the author's guide to the 0.2.0 highlighting feature (R16, R17); every command below was run on the release candidate.
related_documents: [MAN-2026-0001, MAN-2026-0003, MAN-2026-0004, SYS-2026-0011, API-2026-0004, API-2026-0007, PROP-2026-0002, PLAN-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, manual, editors, vscode, textmate, highlighting]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.2.0-rc (main at e7ed8ed)"
next_review_date: 2026-10-29
---

# Editor support and syntax highlighting for .rivet files

> **Status:** Active
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0 and later
> **Owner:** Project maintainer
> **Affected Components:** editors, language, cli, library, ffi

## Purpose

Read and write `.rivet` files with colours, comment toggling, bracket matching and `end`-based indentation, and
highlight Rivet sources in terminals, HTML documentation and your own tools. Part of the
[Rivet manual](man-2026-0001-rivet-manual.md). The executable demo is `docs/demos/16-editor/` (DEMO-2026-0018).

## Reading Order

1. [VS Code](#install-the-vs-code-extension) — most authors stop here.
2. [`rivet highlight`](#highlight-on-the-terminal-or-to-html) for terminals and docs.
3. [Other editors](#other-textmate-editors), [library and C use](#highlight-from-rust-or-c).
4. [Regenerating the grammar](#regenerate-the-grammar) (contributors).

## What's New in the Current Supported Release

All of it is new in 0.2.0: `editors/` (keyword table, TextMate grammar, VS Code extension), `rivet highlight`,
`rivet::highlight` and `rivet_highlight`.

## Project Goals and Boundaries

- One keyword table, generated from the parser grammar (`src/infra/rivet.capy`), feeds the editor grammar **and**
  the CLI highlighter, so they cannot drift (a test fails if they do).
- The extension is declarative (grammar + language configuration): no language server, no completion, no
  diagnostics in the editor. Run `rivet check` for diagnostics.
- Not on the VS Code Marketplace: you install a local `.vsix`.

## Concepts

| Token class | Examples | TextMate scope (VS Code theme colour) |
|---|---|---|
| `keyword` | `operation`, `global`, `import`, `if`, `for`, `return`, `true`, `null`, built-ins after `(` | `storage.type.declaration.rivet`, `keyword.control.rivet`, `constant.language.rivet`, `support.function.builtin.rivet` |
| `option` | `param`, `output`, `header`, `timeout`, `decode`, `required`, `min` | `keyword.other.option.rivet`, `storage.modifier.rivet` |
| `type` | `integer`, `text`, `json`, `list` | `support.type.rivet` |
| `effect` | `http get`, `file read`, `grpc`, `emit` | `support.function.effect.rivet`, `entity.name.function.effect-verb.rivet` |
| `string` | `"…"`, `'…'`, `` `…` `` | `string.quoted.*.rivet` |
| `interpolation` | `${api}` | `meta.interpolation.rivet` |
| `number` | `429`, `0.2` | `constant.numeric.rivet` |
| `comment` | `# base URL` | `comment.line.number-sign.rivet` |
| `operation_id` | `users.get`, `(users.list {…})` | `entity.name.function.operation-id.rivet` |
| `global` | a name declared by `global`, wherever it is used | `variable.other.constant.global.rivet` |
| `variable` | params, locals, import aliases | `variable.other.rivet`, `variable.parameter.rivet` |
| `operator` | `=`, `+`, `==`, `and`, `as` | `keyword.operator.*.rivet` |

## Architecture and Mental Model

```text
 src/infra/rivet.capy ──editors/gen_grammar.py──▶ editors/keywords.json ──┬──▶ editors/rivet.tmLanguage.json
   (the parser grammar)      (+ class table)        (GENERATED)          │     └─▶ editors/vscode/syntaxes/ (copy)
                                                                          │               └─▶ dist/rivet-<version>.vsix
                                                                          └──▶ include_str! in the Rust highlighter
                                                                                 ├─▶ rivet highlight FILE
                                                                                 ├─▶ rivet::highlight::tokens(src)
                                                                                 └─▶ rivet_highlight(src, fmt)
```

The **editor** colours with regular expressions (TextMate). The **CLI/library** highlighter uses the real parser's
spans, so it knows that `api` in `"${api}/users"` is a global and that `users.get` after `(` is an operation ID.

## Installation and Setup

### Install the VS Code extension

```text
[checkout] ─▶ python3 editors/vscode/package_vsix.py ─▶ dist/rivet-<version>.vsix ─▶ code --install-extension … ─▶ reload
```

```sh
python3 editors/vscode/package_vsix.py
# package_vsix: wrote /…/rivet/dist/rivet-0.1.0.vsix (4 extension files)
code --install-extension dist/rivet-0.1.0.vsix
code --list-extensions --show-versions | grep rivet
# rivet.rivet@0.1.0
```

The file name follows the workspace version (`rivet-0.2.0.vsix` in the release build; the capture above predates
the version bump). Python 3 is enough: the script writes the `.vsix` zip itself (no Node.js, no `vsce`). The
install was verified into an isolated extensions directory (`code --extensions-dir DIR --install-extension …`).

In VS Code:

```text
+---------------- VS Code — app.rivet ------------------------------------+
| 1. Open any *.rivet file: the status bar shows "Rivet"                   |
| 2. Colours: global api = "https://api.example.com"   # base URL          |
|             ^kw    ^glob ^string                       ^comment          |
| 3. Cmd+/ (Ctrl+/) toggles "# " on the selected lines                     |
| 4. Type "operation x.y" + Enter  → the next line is indented             |
|    Type "end"                    → the line dedents to the block start   |
| 5. Brackets () [] {} match and auto-close; quotes auto-close             |
+--------------------------------------------------------------------------+
   nothing coloured? → the file is not *.rivet, or pick "Rivet" in the language picker
```

Uninstall: `code --uninstall-extension rivet.rivet`.

### Other TextMate editors

`editors/rivet.tmLanguage.json` (scope `source.rivet`) works in any editor that loads TextMate grammars —
Sublime Text, JetBrains IDEs (TextMate Bundles plugin), and others. Point the editor at the file and associate
`*.rivet` with it. Comment, bracket and indentation rules are in `editors/vscode/language-configuration.json`;
other editors need their own equivalent.

## Feature Catalogue

| Feature | Why / When to Use It | Supported Surfaces | Since Version | Instructions | Demo |
|---|---|---|---|---|---|
| VS Code extension | Write `.rivet` with colours and indentation | VS Code 1.75+ | 0.2.0 | [Install](#install-the-vs-code-extension) | `docs/demos/16-editor/` |
| TextMate grammar | Other editors | TextMate hosts | 0.2.0 | [Other editors](#other-textmate-editors) | — |
| `rivet highlight` | Terminal review, HTML docs, token dumps | CLI | 0.2.0 | [Highlight](#highlight-on-the-terminal-or-to-html) | `docs/demos/16-editor/` |
| `rivet::highlight` | Rust tools | library | 0.2.0 | [Rust or C](#highlight-from-rust-or-c) | — |
| `rivet_highlight` | Non-Rust tools | C ABI | 0.2.0 | [Rust or C](#highlight-from-rust-or-c) | `docs/demos/15-ffi/` |

## Configuration and Environment Variables

| Name | Kind | Type / Allowed Values | Default | Required When | Scope | Effect | Security Notes | Example |
|---|---|---|---|---|---|---|---|---|
| `--format` | CLI flag | `ansi`, `html`, `json` | `ansi` on a terminal, `json` otherwise | — | one run | output format | reads only the named file | `--format html` |
| `--out DIR` | `package_vsix.py` flag | path | `<repo>/dist` | — | packaging | where the `.vsix` goes | — | `--out /tmp/vsix` |

## Task-Oriented Workflows

### Highlight on the terminal or to HTML

`rivet highlight PATH [--format ansi|html|json]` reads one file; no bundle is loaded and `--file` is not needed.

#### CLI Procedure

```text
$ rivet highlight tests/fixtures/highlight/app.rivet --format json | head -5
{"line":1,"col":1,"len":90,"class":"comment","text":"# Golden source for T-14 (tests/conformance_highlight.rs): every token class once or more."}
{"line":2,"col":1,"len":6,"class":"keyword","text":"import"}
{"line":2,"col":8,"len":15,"class":"string","text":"\"./users.rivet\""}
{"line":2,"col":24,"len":2,"class":"operator","text":"as"}
{"line":2,"col":27,"len":5,"class":"variable","text":"users"}

$ rivet highlight tests/fixtures/highlight/app.rivet --format html | head -4
<pre class="rv-source"><code><span class="rv-comment"># Golden source for T-14 (tests/conformance_highlight.rs): every token class once or more.</span>
<span class="rv-keyword">import</span> <span class="rv-string">&quot;./users.rivet&quot;</span> <span class="rv-operator">as</span> <span class="rv-variable">users</span> <span class="rv-keyword">public</span>
<span class="rv-keyword">global</span> <span class="rv-global">api</span> <span class="rv-operator">=</span> <span class="rv-string">&quot;https://api.example.com&quot;</span>   <span class="rv-comment"># base URL</span>
<span class="rv-keyword">global</span> <span class="rv-global">retry_on</span> <span class="rv-operator">=</span> [<span class="rv-number">429</span>, <span class="rv-number">503</span>]

$ rivet highlight tests/fixtures/highlight/app.rivet --format ansi | head -3 | cat -v
^[[90m# Golden source for T-14 (tests/conformance_highlight.rs): every token class once or more.^[[0m
^[[1;35mimport^[[0m ^[[32m"./users.rivet"^[[0m ^[[37mas^[[0m users ^[[1;35mpublic^[[0m
^[[1;35mglobal^[[0m ^[[1;33mapi^[[0m ^[[37m=^[[0m ^[[32m"https://api.example.com"^[[0m   ^[[90m# base URL^[[0m
```

Token rules: `line`/`col` are 1-based and count **characters** (not bytes); every token lies on one line;
brackets, commas, colons, whitespace and object keys are not tokens.

A syntax error keeps the tokens before it (stdout) and prints the diagnostic (stderr), exit 2:

```text
$ rivet highlight tests/fixtures/highlight/broken.rivet --format json
{"line":1,"col":1,"len":6,"class":"keyword","text":"global"}
…
{"line":4,"col":12,"len":4,"class":"type","text":"json"}
error[syntax.expression]: the expression after `return` does not parse
  --> tests/fixtures/highlight/broken.rivet:5:5
   |
  5|     return {x: }
   |     ^^^^^^^^^^^^
  = hint: check its brackets, commas, quotes and object keys; list items are read with `xs.0`
$ echo $?
2
```

#### Docs pipeline use

The HTML output is a `<pre class="rv-source"><code>` block with one `<span class="rv-CLASS">` per token and HTML
escaping. Style the twelve classes (`rv-keyword`, `rv-option`, `rv-type`, `rv-effect`, `rv-string`,
`rv-interpolation`, `rv-number`, `rv-comment`, `rv-operation_id`, `rv-global`, `rv-variable`, `rv-operator`)
in your site CSS, for example:

```sh
for f in docs/demos/*/app.rivet; do rivet highlight "$f" --format html > "site/$(basename "$(dirname "$f")").html"; done
```

### Highlight from Rust or C

```rust
// Rust (facade): tokens, or the tokens before an error plus the error
match rivet::highlight::tokens(source) {
    Ok(tokens) => for t in tokens { println!("{}:{} {:?}", t.line, t.col, t.class) },
    Err((tokens, err)) => eprintln!("{} tokens before {}", tokens.len(), err.code),
}
```

```c
/* C: "json" (default when NULL), "html" or "ansi"; free the result */
char *tokens = rivet_highlight("global x = 1\n", NULL);
/* {"line":1,"col":1,"len":6,"class":"keyword","text":"global"}
   {"line":1,"col":8,"len":1,"class":"global","text":"x"}
   {"line":1,"col":10,"len":1,"class":"operator","text":"="}
   {"line":1,"col":12,"len":1,"class":"number","text":"1"}                                  */
rivet_string_free(tokens);
```

An unknown format is an error envelope (`validation.ffi_argument`, "unknown format `svg` (expected json, html or
ansi)"). In `json` over the C ABI a syntax error adds a final error-envelope line.

### Regenerate the grammar

Contributors who change `src/infra/rivet.capy` (a new keyword or option) must regenerate:

```text
edit rivet.capy ─▶ python3 editors/gen_grammar.py ─▶ keywords.json + rivet.tmLanguage.json (+ vscode copy)
                      │ new literal has no class?  ─▶ add it to CLASS_TABLE in gen_grammar.py
                      └─▶ python3 editors/gen_grammar.py --check   (exit 1 when stale)
cargo test --test conformance_highlight  ─▶ runs editors/check_keywords.py (drift) and the golden tests
```

Never hand-edit `editors/keywords.json` or the grammar files. `editors/tests/check_grammar.py` applies the
generated regexes to every REF-2026-0002 example and demo app.

## Complete CLI Reference

| Command / Flag | Purpose and When to Use | Syntax / Type / Default | Inputs | Output / Exit Codes | Errors | Example | Since |
|---|---|---|---|---|---|---|---|
| `rivet highlight` | Tokens of one file | `rivet highlight PATH [--format F]` | a `.rivet` file | tokens; 0, 2 (syntax error or unreadable file) | `syntax.*`, `validation.usage` | `rivet highlight app.rivet --format html` | 0.2.0 |
| `--format` | Pick output | `ansi` \| `html` \| `json`; `ansi` on a TTY else `json` | — | — | clap usage error | `--format json` | 0.2.0 |

## Complete API and Event Reference

`rivet::highlight::{tokens, tokens_of, highlight, render}` (plus `HighlightToken`, `HighlightFormat`, `TOKEN_CLASSES`) ([API-2026-0004](../api/api-2026-0004-rust-library.md));
`rivet_highlight` ([API-2026-0007](../api/api-2026-0007-c-abi.md)).

## UI Screen and Interaction Reference

| Screen | Entry Path | Controls / Events | States | Result / Navigation | Errors / Recovery | Related Features |
|---|---|---|---|---|---|---|
| VS Code editor | open a `*.rivet` file | typing, `Cmd+/`, Enter, `end` | coloured / plain (wrong language) | colours, comments, indentation | plain text → select "Rivet" in the language picker | extension |
| Extensions view | `code --list-extensions` | install / uninstall | installed `rivet.rivet@<version>` | — | reinstall the `.vsix` | extension |

## Errors and Recovery Reference

| Error / Code / Message | Surface | Cause | User-Visible Result | Recovery | Retry Safe | Related Feature |
|---|---|---|---|---|---|---|
| `syntax.*` | `rivet highlight` | the file does not parse | tokens up to the error + diagnostic, exit 2 | fix the source (`rivet check`) | yes | highlight |
| `validation.usage` "rivet highlight nope.rivet: entity not found" | `rivet highlight` | missing or unreadable file | exit 2 | fix the path | yes | highlight |
| `validation.ffi_argument` | `rivet_highlight` | unknown format / NULL source | error envelope | pass json, html or ansi | yes | C ABI |
| version mismatch | `package_vsix.py`, `conformance_highlight` | `package.json` ≠ workspace version | script/test fails | sync `editors/vscode/package.json` | yes | packaging |

## Examples and Demos

`tests/fixtures/highlight/app.rivet` (every class), its goldens `app.jsonl`, `app.html`, `app.ansi`, and
`broken.rivet`/`broken.jsonl`. Demo: `docs/demos/16-editor/`.

## Operations, Observability and Maintenance

The `.vsix` is deterministic (sorted entries, fixed timestamps): the same sources give the same SHA-256, so the
release can publish a checksum.

## Edge Cases

- Multi-line backtick strings are split into one token per line.
- `end` and `else` closing flat option sub-blocks are classified by a gap scan (the parser keeps no node for them).
- `(ID …)` calls are `operation_id`; names declared with `global` are `global` everywhere (`retry_on.0`).

## Failure Modes, Recovery and Rollback

A bad grammar only affects colours; uninstall the extension or reinstall an older `.vsix`.

## Security and Compatibility

The extension runs no code (declarative only). `rivet highlight` reads one file and loads no bundle or policy.

## Limitations

- No language server (no completion, hover, go-to-definition or diagnostics in the editor).
- The grammar is regex-based: rare constructs may colour differently from `rivet highlight`.
- Grammar snapshot tests use a Python engine, not Oniguruma (VS Code's engine); the grammar uses only syntax both share.

## Troubleshooting References

| Symptom | Fix |
|---|---|
| `package_vsix` version error | Make `editors/vscode/package.json` `version` equal the workspace version |
| `gen_grammar.py --check` exits 1 | Run `python3 editors/gen_grammar.py` and commit the outputs |
| No colours in VS Code | Check the language mode is "Rivet"; reinstall the `.vsix`; reload the window |

## Glossary

TextMate grammar — regex rules mapping text to scopes. `.vsix` — a VS Code extension package (a zip).
Token class — one of the twelve classes above.

## Version Applicability

| Feature / Interface | Introduced | Changed | Deprecated / Removed | Applicable Environment |
|---|---|---|---|---|
| VS Code extension, TextMate grammar | 0.2.0 | — | — | VS Code ≥ 1.75, TextMate hosts |
| `rivet highlight`, `rivet::highlight`, `rivet_highlight` | 0.2.0 | — | — | macOS, Linux |

## Related Features

[Language guide](man-2026-0003-language-guide.md) · [CLI reference](man-2026-0004-cli-reference.md) · [FFI](man-2026-0009-c-abi-and-ffi.md)

## Related Documents

- [SYS-2026-0011](../system/components/sys-2026-0011-highlighting-and-grammar-generation.md) — how it is built
- `editors/vscode/README.md` — the extension's own page
- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) D-14

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-29 | Claude | Initial editor and highlighting manual (TASK-072, D-14) |
