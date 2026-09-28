# Rivet for Visual Studio Code

Syntax highlighting and editing support for [Rivet](https://github.com/olivierdevelops/rivet) `.rivet` sources.

```text
 global api = "https://api.example.com"   # base URL
 ^decl  ^glob ^string                       ^comment
 operation users.get
 ^decl     ^operation id
     param id integer required min 1
     ^decl ^par ^type  ^modifier ^mod ^number
     r = http get "${api}/users/${id}"
     ^var ^effect  ^string ^interp
 end
 ^control
```

## Features

| Feature | Behaviour |
|---|---|
| Language | `.rivet` files open as **Rivet** |
| Highlighting | declarations, control keywords, option lines, types, effects, strings with `${…}` interpolation, numbers, `true`/`false`/`null`, `#` comments, operation IDs, globals, operators |
| Comments | `Ctrl+/` / `Cmd+/` toggles `#` line comments |
| Brackets | `()`, `[]`, `{}` matched and auto-closed; quotes and `` ` `` auto-closed |
| Indentation | Enter after `operation`, `if`, `for`, `with`, `try`, `dag`, … indents; typing `end`, `else` or `catch` dedents |
| Folding | indentation based (`end` closes the block) |

## Install

The extension is not on the Marketplace. Build the `.vsix` from a Rivet checkout (Python 3, no Node.js):

```sh
python3 editors/gen_grammar.py                  # regenerate the grammar from src/infra/rivet.capy
python3 editors/vscode/package_vsix.py          # → dist/rivet-<version>.vsix
code --install-extension dist/rivet-0.1.0.vsix
```

## How the grammar is made

```text
 src/infra/rivet.capy ─▶ editors/gen_grammar.py ─▶ editors/keywords.json ─▶ syntaxes/rivet.tmLanguage.json
                                                          │
                                                          └─▶ rivet highlight / rivet::highlight::tokens
```

The grammar and `keywords.json` are generated: never edit them by hand. `cargo test` fails when a keyword in
`rivet.capy` is missing from the table (`editors/check_keywords.py`).

The version of this extension equals the Rivet version (`Cargo.toml`).
