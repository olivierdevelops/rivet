---
document_id: DEMO-2026-0018
title: "Editor support and syntax highlighting"
document_type: demo
status: active
created_date: 2026-09-29
last_updated: 2026-09-29
document_revision: 2
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, highlight_source, cli]
affected_versions:
  from: "0.2.0"
  to: null
applicable_environments: [development]
audience: [developers, reviewers, contributors]
scope: Package the VS Code extension as a `.vsix` with Python only; install it into an isolated extensions directory; what each token class looks like; `rivet highlight` in ansi, html and json on one sample file; regenerating the TextMate grammar and the keyword drift check; highlighting a file with a syntax error (partial tokens, diagnostic, exit 2).
reason: PROP-2026-0002 UC-07 and UC-08 (UQ-01 "syntax highlighting for .rivet files in editors, terminals and docs"); PLAN-2026-0002 D-19, TASK-075.
dependencies: [PROP-2026-0002, PLAN-2026-0002]
related_documents: [PROP-2026-0002, PLAN-2026-0002, DEMO-2026-0020, DEMO-2026-0016, MAN-2026-0010, MAN-2026-0004]
supersedes: null
superseded_by: null
tags: [rivet, demo, editor, vscode, highlighting, textmate]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-29
verified_against: "0.2.0"
---

# Editor support and syntax highlighting

> **Status:** Active
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0 and later
> **Owner:** Project maintainer
> **Affected Components:** language (highlight_source), cli, editors/vscode

## Purpose

Rivet 0.2.0 colours `.rivet` sources in three places from **one keyword table**:

1. **VS Code** (and any TextMate editor) through a generated grammar packaged as a `.vsix`;
2. **the terminal** through `rivet highlight FILE --format ansi`;
3. **documentation sites** through `rivet highlight FILE --format html` (or `json` tokens for your own renderer).

This demo builds and installs the extension without touching your VS Code profile, highlights [sample.rivet](sample.rivet) in the three formats, regenerates the grammar, shows how drift is caught, and highlights a broken file ([broken.rivet](broken.rivet)).

```text
 src/infra/rivet.capy ──editors/gen_grammar.py──▶ editors/keywords.json ──┬──▶ editors/rivet.tmLanguage.json
   (the parser grammar)      (+ class table)        (GENERATED)          │     └─▶ editors/vscode/syntaxes/ (copy)
                                                                          │               └─▶ dist/rivet-<version>.vsix ─▶ VS Code
                                                                          └──▶ include_str! in the Rust highlighter
                                                                                 ├─▶ rivet highlight FILE --format ansi|html|json
                                                                                 ├─▶ rivet::highlight::tokens(src)       (Rust)
                                                                                 └─▶ rivet_highlight(src, fmt)            (C ABI)
```

The editor colours with regular expressions (TextMate). The CLI, library and C highlighter use the real parser's spans, so they know that `api` is a global and that `users.get` after `(` is an operation ID.

## Verified Against Version

0.2.0. Verified at commit `166a98b` (the source is identical to the `8031baa` release candidate; later commits touch documentation only) with `target/release/rivet`, Python 3.9.6 and VS Code 1.108.1 on macOS 26.4.1 (Darwin 25.4.0, arm64), 2026-09-29. The version string is bumped from 0.1.0 to 0.2.0 at release (P5), so `rivet --version` prints `rivet 0.1.0` and the package is `dist/rivet-0.1.0.vsix`; the release build writes `dist/rivet-0.2.0.vsix`. Every output block below was pasted from that run; the scratch directory names and the `.vsix` SHA-256 (which changes with the version) vary.

## Prerequisites

```sh
cargo build --release --workspace --all-features   # from the repository root
export PATH="$PWD/target/release:$PATH"            # put this build first: an older `rivet` elsewhere on PATH has no `highlight`
```

| Needed for | Tool |
|---|---|
| steps 1, 5 | `python3` (standard library only; no Node.js, no `vsce`) |
| step 2 | the `code` command-line launcher (VS Code: *Shell Command: Install 'code' command in PATH*) |
| steps 3, 4, 6 | the `rivet` binary built above |

## Setup

```sh
cd docs/demos/16-editor        # steps 3, 4 and 6 run here; steps 1, 2 and 5 from the repository root
```

Nothing in this folder is a bundle: `sample.rivet` and `broken.rivet` are only highlighted, never loaded, so the `import` in `sample.rivet` does not need its target file.

## Steps

### 1. Build the `.vsix`

#### Command / Request

```sh
python3 editors/vscode/package_vsix.py
unzip -l dist/rivet-0.1.0.vsix
```

#### Expected Output / Response

```text
package_vsix: wrote /Users/…/rivet/dist/rivet-0.1.0.vsix (4 extension files)
Archive:  dist/rivet-0.1.0.vsix
  Length      Date    Time    Name
---------  ---------- -----   ----
      299  01-01-1980 00:00   [Content_Types].xml
     1790  01-01-1980 00:00   extension.vsixmanifest
     2051  01-01-1980 00:00   extension/README.md
     1718  01-01-1980 00:00   extension/language-configuration.json
      911  01-01-1980 00:00   extension/package.json
    11378  01-01-1980 00:00   extension/syntaxes/rivet.tmLanguage.json
---------                     -------
    18147                     6 files
```

The archive is deterministic (sorted entries, fixed 1980 timestamps): the same sources give the same bytes. `dist/` is git-ignored. The file name follows the workspace version, and `cargo test --test conformance_highlight` (`t13_vsix_packages_with_the_workspace_version`) fails if `editors/vscode/package.json` and `Cargo.toml` disagree.

```text
 dist/rivet-<version>.vsix  (a zip, Open Packaging Conventions)
 ├── [Content_Types].xml
 ├── extension.vsixmanifest          rivet.rivet@<version>, engine ^1.75.0
 └── extension/
     ├── package.json                contributes: language "rivet" (*.rivet) + grammar source.rivet
     ├── README.md
     ├── language-configuration.json # comments, brackets, end-based indentation
     └── syntaxes/rivet.tmLanguage.json   (generated, step 5)
```

### 2. Install it (isolated profile)

Install into a throw-away extensions directory and user-data directory so your real VS Code profile is never touched. For everyday use, drop the two `--…-dir` flags.

#### Command / Request

```sh
WORK="$(mktemp -d)"
code --extensions-dir "$WORK/ext" --user-data-dir "$WORK/user" --install-extension dist/rivet-0.1.0.vsix
code --extensions-dir "$WORK/ext" --user-data-dir "$WORK/user" --list-extensions --show-versions
ls "$WORK/ext/rivet.rivet-0.1.0"
code --extensions-dir "$WORK/ext" --user-data-dir "$WORK/user" --uninstall-extension rivet.rivet
```

#### Expected Output / Response

```text
Installing extensions...
Extension 'rivet-0.1.0.vsix' was successfully installed.
rivet.rivet@0.1.0
language-configuration.json
package.json
README.md
syntaxes
Uninstalling rivet.rivet...
Extension 'rivet.rivet' was successfully uninstalled!
```

Each command exits 0. VS Code 1.108.1 also printed a Node `DEP0169` `url.parse()` deprecation warning on stderr during the install; it comes from VS Code itself, not from the extension.

The everyday form, into your own profile:

```sh
code --install-extension dist/rivet-0.1.0.vsix      # rivet-0.2.0.vsix after the release bump
```

### 3. What gets coloured

Open any `*.rivet` file; the status bar shows **Rivet**. The mock below is [sample.rivet](sample.rivet) with the class of every character written under it, taken from the step 4 token output (letters in the legend). The actual colour depends on your theme, which maps the TextMate scope in the table.

```text
+-------------------- VS Code — sample.rivet (status bar: Rivet) -------------------+
|  1  # sample.rivet: one small file that uses all twelve token classes.            |
|     CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC            |
|  2  import "./users.rivet" as users public   # highlight never opens imports      |
|     KKKKKK SSSSSSSSSSSSSSS PP VVVVV KKKKKK   CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC      |
|  3  global api = "https://api.example.com"   # base URL                           |
|     KKKKKK GGG P SSSSSSSSSSSSSSSSSSSSSSSSS   CCCCCCCCCC                           |
|  4  global retry_on = [429, 503]                                                  |
|     KKKKKK GGGGGGGG P  NNN  NNN                                                   |
|  5                                                                                |
|  6  operation users.page                                                          |
|     KKKKKKKKK DDDDDDDDDD                                                          |
|  7      name "List a page of users"                                               |
|         KKKK SSSSSSSSSSSSSSSSSSSSSS                                               |
|  8      description "Read one page; the URL is built from a global."              |
|         KKKKKKKKKKK SSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSSS              |
|  9      param limit integer default 20 min 1 description "Page size."             |
|         KKKKK VVVVV TTTTTTT OOOOOOO NN OOO N OOOOOOOOOOO SSSSSSSSSSSS             |
| 10      output json description "The page returned by the API."                   |
|         KKKKKK TTTT OOOOOOOOOOO SSSSSSSSSSSSSSSSSSSSSSSSSSSSSSS                   |
| 11      r = http get "${api}/users?limit=${limit}"                                |
|         V P EEEE EEE SIIIIIISSSSSSSSSSSSSIIIIIIIIS                                |
| 12          timeout "5s"                                                          |
|             OOOOOOO SSSS                                                          |
| 13          decode json                                                           |
|             OOOOOO TTTT                                                           |
| 14      end                                                                       |
|         KKK                                                                       |
| 15      if r.status == 200 and limit > 0                                          |
|         KK VVVVVVVV PP NNN PPP VVVVV P N                                          |
| 16          return r.body                                                         |
|             KKKKKK VVVVVV                                                         |
| 17      end                                                                       |
|         KKK                                                                       |
| 18      return (users.get {id: 1})                                                |
|         KKKKKK  DDDDDDDDD      N                                                  |
| 19  end                                                                           |
|     KKK                                                                           |
+-----------------------------------------------------------------------------------+
  K keyword   O option   T type     E effect        S string   I interpolation
  N number    C comment  D operation_id  G global   V variable P operator
  Cmd+/ (Ctrl+/) toggles "# "   ·   Enter after `operation …` indents   ·   `end` dedents
  () [] {} match and auto-close   ·   nothing coloured? → pick "Rivet" in the language picker
```

| Class | Examples in sample.rivet | TextMate scope | ANSI colour (`--format ansi`) |
|---|---|---|---|
| `keyword` | `import`, `public`, `global`, `operation`, `name`, `param`, `output`, `if`, `return`, `end` | `storage.type.declaration.rivet`, `keyword.control.rivet` | bold magenta `1;35` |
| `option` | `default`, `min`, `description`, `timeout`, `decode` | `keyword.other.option.rivet`, `storage.modifier.rivet` | cyan `36` |
| `type` | `integer`, `json` | `support.type.rivet` | yellow `33` |
| `effect` | `http`, `get` | `support.function.effect.rivet` | bold blue `1;34` |
| `string` | `"./users.rivet"`, `"5s"` | `string.quoted.*.rivet` | green `32` |
| `interpolation` | `${api}`, `${limit}` | `meta.interpolation.rivet` | bold green `1;32` |
| `number` | `429`, `20`, `200` | `constant.numeric.rivet` | bright red `91` |
| `comment` | `# base URL` | `comment.line.number-sign.rivet` | grey `90` |
| `operation_id` | `users.page`, `users.get` | `entity.name.function.operation-id.rivet` | bold bright blue `1;94` |
| `global` | `api`, `retry_on` at their declaration | `variable.other.constant.global.rivet` | bold yellow `1;33` |
| `variable` | `users` (alias), `limit`, `r`, `r.status` | `variable.other.rivet` | uncoloured |
| `operator` | `=`, `as`, `==`, `and`, `>` | `keyword.operator.*.rivet` | white `37` |

### 4. `rivet highlight` in three formats

`rivet highlight PATH [--format ansi|html|json]` reads one file from the parser's own spans. No bundle is loaded and `--file` is not needed. The default is `ansi` when stdout is a terminal and `json` otherwise.

#### Command / Request

```sh
rivet highlight sample.rivet --format json | head -8
rivet highlight sample.rivet --format json | python3 -c 'import sys,json,collections; c=collections.Counter(json.loads(l)["class"] for l in sys.stdin); print(len(c), sum(c.values())); print(dict(sorted(c.items())))'
rivet highlight sample.rivet --format html | sed -n '1,3p;11p'
rivet highlight sample.rivet --format ansi | sed -n '1,4p;11p' | cat -v
```

#### Expected Output / Response

`json` gives one `{line, col, len, class, text}` object per token. `line` and `col` are 1-based and count characters; brackets, commas, colons, whitespace and object keys are not tokens:

```json
{"line":1,"col":1,"len":66,"class":"comment","text":"# sample.rivet: one small file that uses all twelve token classes."}
{"line":2,"col":1,"len":6,"class":"keyword","text":"import"}
{"line":2,"col":8,"len":15,"class":"string","text":"\"./users.rivet\""}
{"line":2,"col":24,"len":2,"class":"operator","text":"as"}
{"line":2,"col":27,"len":5,"class":"variable","text":"users"}
{"line":2,"col":33,"len":6,"class":"keyword","text":"public"}
{"line":2,"col":42,"len":31,"class":"comment","text":"# highlight never opens imports"}
{"line":3,"col":1,"len":6,"class":"keyword","text":"global"}
```

All twelve classes appear, in 65 tokens:

```text
12 65
{'comment': 3, 'effect': 2, 'global': 2, 'interpolation': 2, 'keyword': 15, 'number': 7, 'operation_id': 2, 'operator': 7, 'option': 6, 'string': 10, 'type': 3, 'variable': 6}
```

`html` is a `<pre class="rv-source"><code>` block with one `<span class="rv-CLASS">` per token, HTML-escaped (the interpolated string on line 11 is split into string and interpolation spans):

```html
<pre class="rv-source"><code><span class="rv-comment"># sample.rivet: one small file that uses all twelve token classes.</span>
<span class="rv-keyword">import</span> <span class="rv-string">&quot;./users.rivet&quot;</span> <span class="rv-operator">as</span> <span class="rv-variable">users</span> <span class="rv-keyword">public</span>   <span class="rv-comment"># highlight never opens imports</span>
<span class="rv-keyword">global</span> <span class="rv-global">api</span> <span class="rv-operator">=</span> <span class="rv-string">&quot;https://api.example.com&quot;</span>   <span class="rv-comment"># base URL</span>
    <span class="rv-variable">r</span> <span class="rv-operator">=</span> <span class="rv-effect">http</span> <span class="rv-effect">get</span> <span class="rv-string">&quot;</span><span class="rv-interpolation">${api}</span><span class="rv-string">/users?limit=</span><span class="rv-interpolation">${limit}</span><span class="rv-string">&quot;</span>
```

`ansi` wraps each token in an SGR colour (shown with `cat -v`; `^[` is ESC). Variables stay uncoloured:

```text
^[[90m# sample.rivet: one small file that uses all twelve token classes.^[[0m
^[[1;35mimport^[[0m ^[[32m"./users.rivet"^[[0m ^[[37mas^[[0m users ^[[1;35mpublic^[[0m   ^[[90m# highlight never opens imports^[[0m
^[[1;35mglobal^[[0m ^[[1;33mapi^[[0m ^[[37m=^[[0m ^[[32m"https://api.example.com"^[[0m   ^[[90m# base URL^[[0m
^[[1;35mglobal^[[0m ^[[1;33mretry_on^[[0m ^[[37m=^[[0m [^[[91m429^[[0m, ^[[91m503^[[0m]
    r ^[[37m=^[[0m ^[[1;34mhttp^[[0m ^[[1;34mget^[[0m ^[[32m"^[[0m^[[1;32m${api}^[[0m^[[32m/users?limit=^[[0m^[[1;32m${limit}^[[0m^[[32m"^[[0m
```

Every command exits 0. For a documentation site, render each demo once and style the twelve `rv-*` classes in your CSS:

```sh
for f in ../*/app.rivet; do rivet highlight "$f" --format html > "$TMPDIR/$(basename "$(dirname "$f")").html"; done
```

### 5. Regenerate the grammar and catch drift

The keyword table and both grammar files are generated from `src/infra/rivet.capy`. Contributors who add a keyword regenerate; `--check` and `check_keywords.py` fail when an output is stale or a literal has no class. Run from the repository root; the failing half runs in a scratch copy so the checkout is never modified.

```text
 edit rivet.capy ─▶ python3 editors/gen_grammar.py ─▶ keywords.json + rivet.tmLanguage.json (+ vscode copy)
                       │ new literal has no class?  ─▶ add it to CLASS_TABLE in gen_grammar.py
                       └─▶ python3 editors/gen_grammar.py --check     exit 1 when stale
 cargo test --test conformance_highlight ─▶ runs editors/check_keywords.py (drift) + the golden tests
```

#### Command / Request

```sh
python3 editors/gen_grammar.py --check
python3 editors/check_keywords.py

WORK="$(mktemp -d)"; cp -R editors src "$WORK/"; cd "$WORK"          # scratch copy
sed -i.bak 's/"keyword.control.rivet"/"keyword.control.hand-edited.rivet"/' editors/rivet.tmLanguage.json
diff editors/rivet.tmLanguage.json.bak editors/rivet.tmLanguage.json; rm editors/rivet.tmLanguage.json.bak
python3 editors/gen_grammar.py --check
python3 editors/check_keywords.py
python3 editors/gen_grammar.py
python3 editors/check_keywords.py
cd - && rm -rf "$WORK"
```

#### Expected Output / Response

In the checkout, both checks pass (exit 0):

```text
gen_grammar: generated files are up to date
check_keywords: 108 rivet.capy literals classified; 148 table words; generated files up to date
```

A hand edit to a generated file is caught by both (exit 1), and regenerating repairs it (exit 0):

```text
347c347
<           "name": "keyword.control.rivet"
---
>           "name": "keyword.control.hand-edited.rivet"
gen_grammar: stale generated files: editors/rivet.tmLanguage.json — run python3 editors/gen_grammar.py
check_keywords: editors/rivet.tmLanguage.json is stale (run python3 editors/gen_grammar.py)
gen_grammar: wrote editors/rivet.tmLanguage.json
check_keywords: 108 rivet.capy literals classified; 148 table words; generated files up to date
```

`cargo test --test conformance_highlight` runs the same drift check plus the golden and corpus tests (every `docs/demos/**/app.rivet` and every REF-2026-0002 block): `test result: ok. 8 passed; 0 failed`. `python3 editors/tests/check_grammar.py` applies the TextMate regexes themselves: `check_grammar: 110 samples, 1754 lines tokenized; every declaration/control keyword scoped`.

### 6. Failing example: a file with a syntax error

[broken.rivet](broken.rivet) has an unfinished object on line 6.

#### Command / Request

```sh
rivet highlight broken.rivet --format json; echo "exit $?"
rivet highlight nope.rivet --format json; echo "exit $?"
rivet highlight sample.rivet --format svg; echo "exit $?"
```

#### Expected Output / Response

The tokens before the error are still printed on stdout (11 here), then the diagnostic on stderr, exit 2. An editor or docs pipeline can therefore colour everything up to the mistake:

```text
{"line":1,"col":1,"len":72,"class":"comment","text":"# broken.rivet: line 6 has an unfinished object, so parsing stops there."}
{"line":2,"col":1,"len":6,"class":"keyword","text":"global"}
{"line":2,"col":8,"len":3,"class":"global","text":"api"}
{"line":2,"col":12,"len":1,"class":"operator","text":"="}
{"line":2,"col":14,"len":25,"class":"string","text":"\"https://api.example.com\""}
{"line":4,"col":1,"len":9,"class":"keyword","text":"operation"}
{"line":4,"col":11,"len":11,"class":"operation_id","text":"demo.broken"}
{"line":5,"col":5,"len":6,"class":"keyword","text":"output"}
{"line":5,"col":12,"len":4,"class":"type","text":"json"}
{"line":5,"col":17,"len":11,"class":"option","text":"description"}
{"line":5,"col":29,"len":17,"class":"string","text":"\"Never compiles.\""}
error[syntax.expression]: the expression after `return` does not parse
  --> broken.rivet:6:5
   |
  6|     return {x: }
   |     ^^^^^^^^^^^^
  = hint: check its brackets, commas, quotes and object keys; list items are read with `xs.0`
exit 2
```

A block that is never closed keeps the tokens of its header lines too (INC-2026-0012): lines before the error that no parsed statement covers are lexed as statement lines. With `printf 'operation x.y\n    name "X"\n    return (\n' > b.rivet`, `rivet highlight b.rivet --format json 2>/dev/null` prints lines 1 and 2 (exit 2):

```text
{"line":1,"col":1,"len":9,"class":"keyword","text":"operation"}
{"line":1,"col":11,"len":3,"class":"operation_id","text":"x.y"}
{"line":2,"col":5,"len":4,"class":"keyword","text":"name"}
{"line":2,"col":10,"len":3,"class":"string","text":"\"X\""}
```

A missing file and an unknown format are usage errors, also exit 2:

```text
error[validation.usage]: rivet highlight nope.rivet: entity not found
exit 2
error: invalid value 'svg' for '--format <FORMAT>'
  [possible values: ansi, html, json]

For more information, try '--help'.
exit 2
```

| Input | stdout | stderr | Exit |
|---|---|---|---|
| valid file | every token | — | 0 |
| syntax error | tokens before the error | `error[syntax.*]` with span and hint | 2 |
| missing file | — | `error[validation.usage]` | 2 |
| unknown `--format` | — | clap usage error | 2 |

## Release Updates

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-16 | UQ-01 / R16 | Generated TextMate grammar; VS Code extension packaged as `.vsix` with Python only | Steps 1, 2, 5 | `rivet.rivet@0.1.0` installed (0.2.0 after the bump); drift caught with exit 1 | This README steps 1, 2, 5 (2026-09-29, 166a98b); `tests/conformance_highlight.rs` T-13 |
| U-17 | UQ-01 / R17 | `rivet highlight` (ansi, html, json) from parser spans; partial tokens and exit 2 on a syntax error | Steps 4, 6 | 12 classes, 65 tokens; 11 tokens then `syntax.expression`, exit 2 | This README steps 4, 6; `tests/conformance_highlight.rs` T-14 |

## Cleanup

```sh
rm -rf "$WORK"                  # the scratch profile of step 2 and the scratch copy of step 5, if still present
rm -f dist/rivet-0.1.0.vsix     # optional; dist/ is git-ignored
```

Nothing is written to this folder.

## Verification Record

| Step | Verified By | Verified At | Result |
|---|---|---|---|
| 1. `package_vsix.py`, archive listing | Claude (TASK-075) | 2026-09-29, commit 166a98b, macOS 26.4.1 arm64 | PASS |
| 2. Install, list, uninstall in an isolated `--extensions-dir` / `--user-data-dir` (VS Code 1.108.1) | Claude (TASK-075) | 2026-09-29, commit 166a98b, macOS 26.4.1 arm64 | PASS |
| 3. Token classes and scopes (from the step 4 output) | Claude (TASK-075) | 2026-09-29, commit 166a98b, macOS 26.4.1 arm64 | PASS (colours inside the VS Code window were not captured; the mock is drawn from the token output) |
| 4. `highlight --format json`, `html`, `ansi` | Claude (TASK-075) | 2026-09-29, commit 166a98b, macOS 26.4.1 arm64 | PASS |
| 5. `gen_grammar.py --check`, `check_keywords.py`, drift in a scratch copy, `conformance_highlight` (8 passed), `check_grammar.py` | Claude (TASK-075) | 2026-09-29, commit 166a98b, macOS 26.4.1 arm64 | PASS |
| 6. Syntax error (11 tokens, exit 2), missing file, unknown format | Claude (TASK-075) | 2026-09-29, commit 166a98b, macOS 26.4.1 arm64 | PASS |
| 6. (re-run after INC-2026-0012) `highlight broken.rivet --format json` (11 tokens, `syntax.expression`, exit 2), missing file and `--format svg` (exit 2), unchanged; unclosed `operation` block keeps its header tokens (lines 1–2, exit 2) | Claude | 2026-09-29, commit 7c25175, macOS 26.4.1 arm64 | PASS |

## Known Caveats

- The release candidate still prints version `0.1.0`, so the package is `rivet-0.1.0.vsix`; the bump to 0.2.0 happens at P5.
- A global read inside `${…}` is part of one `interpolation` token, not a `global` token. Outside a string, `global` names are classified wherever an expression reads them ([14-globals](../14-globals/README.md) step 7).
- The TextMate grammar is regex-based, so an editor can colour some constructs differently from `rivet highlight`, which uses the parser's spans and is the reference. Only the CLI output was captured here; the VS Code window was not screenshotted.
- The extension is not published to a marketplace; install the local `.vsix`.

## Related Documents

- [All sample folders](../README.md) · [demos index](../index.md) · [v0.2.0 release verification guide](../demo-2026-0020-v0-2-0-release-verification.md)
- [Editor support and highlighting manual MAN-2026-0010](../../manuals/man-2026-0010-editor-support-and-highlighting.md) · [CLI reference MAN-2026-0004](../../manuals/man-2026-0004-cli-reference.md)
- [Globals demo DEMO-2026-0016](../14-globals/README.md) · [Modules demo DEMO-2026-0019](../17-modules/README.md)
- [Proposal PROP-2026-0002](../../proposals/approved/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md) · [Plan PLAN-2026-0002](../../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-29 | Claude | TASK-075 (PLAN-2026-0002 D-19): new demo; `.vsix` build and isolated install, token classes, `rivet highlight` ansi/html/json, grammar regeneration and drift check, syntax-error highlighting; executed against 0.2.0-dev (166a98b, source = 8031baa). |
| 2 | 2026-09-29 | Claude | INC-2026-0012 re-verification (T-30) at 7c25175: step 6 re-run (output unchanged); added the unclosed-block example that keeps header tokens; one Verification Record row. |
