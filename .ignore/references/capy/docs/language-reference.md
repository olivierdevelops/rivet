# Language Reference

The complete surface grammar Capy ships with. Everything user-facing is
library-defined — this doc describes only the **fixed lexical structure** that
all libraries share.

## Tokens

The lexer produces the following kinds. **No keywords are reserved by the
engine** — words like `if`, `loop`, `end`, `true`, `false`, `null` are just
identifier tokens; their meaning depends on whether a library function or the
value-parser treats them specially.

| Token        | Description                                                                 |
|--------------|-----------------------------------------------------------------------------|
| `IDENT`      | A word: ASCII letters, digits, underscores, **and any non-ASCII rune** (accented Latin, CJK, emoji, em-dash, smart quotes). Must not start with a digit. |
| `NUMBER`     | An integer or float literal. May be negative.                               |
| `STRING`     | `"..."` or `'...'`. Both support `${expr}` interpolation at eval time.       |
| `TEMPLATE`   | `` `...` ``. **Multi-line.** Same interpolation rules as STRING.            |
| `PUNCT`      | A run of `= < > ! + - * / % & \| ^ ~ ? : , . ; @ $ # \`. Lexed greedily.    |
| `LPAREN/RPAREN/LBRACE/RBRACE/LBRACK/RBRACK` | `(` `)` `{` `}` `[` `]`                            |
| `NEWLINE`    | End of a logical line.                                                      |
| `INDENT/DEDENT` | Indent-level change at start of a line (4 spaces or 1 tab per level). |
| `EOF`        | End of input.                                                                |

Multi-character operators emerge naturally from the greedy punct lexer:
`==`, `!=`, `<=`, `>=`, `:=`, `->`, `=>`, `|>`, etc. are all single `PUNCT`
tokens. A library pattern matches the full text — write `{ kind: literal,
value: ":=" }` to match `:=`.

### Comments

`# ...` to end of line. Comment-only lines do not produce INDENT/DEDENT
changes.

### Strings

- `"..."` and `'...'` are **single-line**. Backslash escapes the next
  character (`\n`, `\t`, `\"`, `\\`, `\xNN`, `\uNNNN`).
- `` `...` `` (backtick / template) is **multi-line** — both inside
  library `write` blocks AND inside user scripts. Newlines between
  the opening and closing backtick become literal newlines in the
  captured value. Combine with `${decoded text}` (see
  [templates.md](templates.md#decoded-string)) to recover the
  user-intended form.
- Inside any string, `${expr}` is interpolated at eval time. `expr` may be a
  dotted identifier path; future versions will support full expressions.
- For type-checking purposes, the source representation including its
  quotes is what gets checked. `set_email "alice@example.com"` produces a
  capture text of `"alice@example.com"` (with quotes); validation strips
  the quotes before applying patterns.

### Indentation

- 4 spaces or 1 tab per level.
- Indentation is checked only at the start of a logical line and only when
  the bracket level is 0. Lines inside `( )`, `[ ]`, `{ }` do not produce
  INDENT/DEDENT tokens, but they DO produce NEWLINE tokens (value parsers
  skip them).

## Statements

A statement is a sequence of tokens terminated by NEWLINE (or end-of-file,
or a `}` if the statement is inside a delimiter-mode block body).

At each statement boundary, the parser tries each library function's
compiled `Elements` in priority order. The first complete match wins, where
"complete" means every element consumed plus a NEWLINE/EOF/`}` to follow.

## Block bodies

A function may declare itself a block opener via `block:`:

- `block: { closer: <function-name> }` — body is delimited by INDENT/DEDENT;
  after DEDENT, the named closer function must match.
- `block: { open: "{", close: "}" }` — body is delimited by the named
  tokens. The `}` (or whatever close token) ends the body; no closer
  function is involved.

Block bodies are nested: inside one block, you can have more blocks.

## Values (within an `any`-typed capture)

When a function captures `<x:any>`, the parser consumes one of:

| Literal kind | Examples |
|---|---|
| number | `42`, `-3.14` |
| string | `"foo"`, `'bar'`, `` `tpl` `` |
| bool | `true`, `false` |
| null | `null` |
| ident path | `x`, `user.address.city` |
| paren sub-call | `(str "hi" name)` |
| list literal | `[1, 2, 3]` |
| object literal | `{"k": "v", name: "Alice"}` (keys may be strings OR identifiers) |
| comparison | `a == b`, `a < b`, `not flag` |

### Operator precedence

Value expressions support infix operators with conventional precedence. All are
**left-associative**, so `a - b - c` is `(a - b) - c`.

| Binding | Operators |
|---|---|
| loosest | `or` |
| | `and` |
| | `==` `!=` `<` `>` `<=` `>=` |
| | `+` `-` |
| tightest | `*` `/` `%` |

```
a * b + c          →  (a * b) + c
a + b * c          →  a + (b * c)
a + 1 == b * 2     →  (a + 1) == (b * 2)
a and b or c       →  (a and b) or c
```

`not` is a prefix operator and binds tighter than all of them.

**Grouping.** `(` is primarily the *prefix-call* form — `(upper name)` calls a
helper — so parentheses group only where that is unambiguous: when the contents
parse as a complete expression that is not a bare identifier. `(a + b) * c`
groups; `(upper name)` is still a call; `(foo)` is still a zero-argument call.

Arithmetic is on the **values** the inner DSL and `${ … }` evaluate. A library
that wants `x = 4 + 5` as *source syntax* still declares the shape, and may now
capture the whole expression as one value rather than as three:

```
function assign
    arg capture var ident
    arg literal "="
    arg capture value any
end
```

## Object literals

Keys may be either quoted strings (`"name"`) or bare identifiers (`name`):

```
{"name": "Alice", age: 30}
{ "x": 1
, "y": 2
}                                # multi-line is fine
```

## Captures: source-text vs evaluated values

Every capture has two faces:

- **In `write` literals** — captures resolve to **source text**. `if x > 0` exposes
  `cond` as the literal `x > 0` so a Python emitter can write `if ${cond}:`.
- **In state-mutation statements** — captures resolve to **evaluated values**.
  `say "hello"` exposes `msg` as the string `hello` (no quotes) so
  `append context.greetings msg` stores the raw value.

This dual model lets one capture serve both render-by-text (templates) and
structured accumulation (context) without needing to convert.

## Error format

Engine errors carry a line and column. The CLI renders:

```
error: <message>
  N │ <offending source line>
    │ ^
```

When you embed Capy as a library, errors are `domain::errors::CapyError`
values with `line`, `col`, `msg` and `hint` fields; use
`domain::errors::format_with_source(&err, source)` to get the rendered form.

A *recovering* parse (`Library::parse`) produces `Diagnostic` values instead,
which add a severity, a stable code, a primary range and secondary labels — see
[diagnostics](diagnostics.md).
