# 2 · Annotations — the `vhco:` grammar

[← Workflow](01-workflow.md) · [wiki index](README.md) · next: [Architecture →](03-architecture.md)

The architecture is **declared in comments**, language-agnostically. Each directive is a comment line:

```
<comment-symbol> vhco:<kind> <body>            [ -- <optional description> ]
```

The comment symbol is built in per file extension (`//`, `#`, `--`, …); add custom ones under `comments` in
`.vhco.json`. Place each line **directly above** the code it describes — the annotation is recorded with its
`file:line` and the next non-blank code line, so the explorer's **Source map** doubles as a navigator.

> **Annotations DEFINE and OVERRIDE the model.** The tool does not reverse-engineer signatures per language —
> what you annotate is what exists. Unannotated code is **invisible** to the model. If a use case, port, or
> domain type has no `vhco:` comment, it does not appear in `vhco.json`, the diagrams, or `validate`/`sync`.

## How a line is parsed (so you know exactly what counts)

The scanner walks each file whose extension has a known comment symbol and, for every line:

1. **Strips to a comment.** The trimmed line must *start* with the comment symbol. A `vhco:` token sitting at
   the end of a code line, or inside a string, or mid-prose is **ignored** — annotations are only ever
   whole comment lines. (This is deliberate: prose like "the vhco label" never becomes model data.)
2. **Peels the description.** The body is split on the first ` -- ` (space-dash-dash-space). Everything
   before is the **directive core**; everything after is the **description / note**. The separator must be
   exactly `space hyphen hyphen space` — `--foo`, `-- ` at end of line with nothing after, or `—` (em dash)
   do **not** split.
3. **Matches a kind.** The core must match `vhco:<kind>` where `<kind>` is one of the known kinds below.
   An unknown kind (e.g. `vhco:thing`) does not match and the line is skipped entirely.
4. **Records a marker** (`kind, name, file, line, code`) where `code` is the next non-blank, non-`vhco:`
   line — and, for structural kinds, **updates the model**.

The full kind set (the only tokens that are directives):

```
structural    : domain  port  usecase  surface  infra  feature  wiring
presentation  : label   about example  meta     ref    step     trigger
edges         : env     arg   file     net      db
markers       : test    todo  error    api      request  response
                capture_start  capture_end  custom
```

> **It is `usecase`, never `use`.** This is the single most common mistake. `vhco:use …` does not match any
> kind and is silently dropped — your use case will simply not exist in the model. Always write `vhco:usecase`.

## Where each directive lives

| Lives in | Directives |
|---|---|
| `domain/` | `domain` |
| `features/<f>/` | `feature` (only when it can't be inferred from the folder) |
| `features/<f>/ports.go` | `port` |
| `features/<f>/<uc>.go` (use case — **pure & surface-blind**) | `usecase`, `label`, `about`, `example` (logical), `meta`, `ref`, `step`, `todo`, `error` |
| `infra/` (adapter) | `infra`, and the edges it performs: `env`, `arg`, `file`, `net`, `db` |
| `orchestrator/setup_<surface>.go` | `surface`, `wiring`, `trigger`, `api`/`request`/`response`, surface-specific `meta` |
| `*_test.go` | `test` |
| anywhere | `capture_start` / `capture_end`, `custom`, and use-case-owned edges (`env`/`arg`/`file`/`net`/`db`) |

The "lives in" column is a **convention**, not a parser rule — an annotation works wherever it sits. But the
ownership of some directives *is* positional: `step`/`todo`/`error` and use-case edges attach to the most
recent `usecase` above them; edges attach to the most recent `infra` if no `usecase` is closer;
`label`/`about`/`example`/`meta`/`ref` enrich the most recent structural item; `request`/`response` enrich the
most recent `api`. Put them in the conventional place and ownership is automatic.

---

## The structural directives (define the model)

Each of these creates or overrides a piece of `vhco.json`'s `domain` / `definition`.

### `domain` — an entity and its fields

```
<comment> vhco:domain <Name> { <field>: <Type>; <field>: <Type>; … }
```

- Fields are separated by `;` (semicolons). Each is `name: Type` (the `:` form) or `name Type` (space form).
- Types are **free text** — write them exactly as the host language spells them.

```go
// vhco:domain Note { id: string; text: string; done: bool }
type Note struct { ID, Text string; Done bool }
```

Produces a `Note` entity with three typed fields in the **Data types** view and the domain class diagram.

### `port` — a feature's typed function contract

```
<comment> vhco:port <Name> { <fn>(<InType>, …) -> <OutType>; <fn>(…) -> …; … }
```

- Lives in `features/<f>/ports.go`. The port belongs to the feature whose folder it sits under.
- Members are separated by `;`. Each member is a function: `name(params) -> result`. The `-> result` is
  optional (a void function may omit it). Params are comma-separated and may be bare types (`add(Note)`)
  or `name: Type` (`delete(id: string)`) — only the **type** is kept for the contract.
- The function list is **mandatory** — a port is never just a name. See [Architecture](03-architecture.md)
  for *why* this is an invariant.

```go
// vhco:port NoteStore { add(Note) -> error; list() -> []Note; delete(id: string) -> error }
```

Produces a `NoteStore` port with three typed functions, shown wherever the port appears (Use cases view,
Outside world view, the per-action external-world contract in `vhco doc`).

### `usecase` — an action's signature and the ports it needs

```
<comment> vhco:usecase <feature>.<action>(<name: Type>, …) -> <Out> needs <Port>, <Port>, …
```

- Either `.` **or** `/` separates feature from action: `notes.add_note` and `notes/add_note` both parse to
  the same action. The canonical written form is `feature.action`; the explorer displays `feature/use_case`.
- Inputs are **comma-separated** `name: Type` pairs (bare `Type` also accepted). This is function-call style,
  *not* semicolon-separated like `domain`/`port` members.
- `-> <Out>` is the single return type (optional for a void action).
- `needs <Port>, …` is a **comma list** of port names this action requires. Each named port must be declared
  with a `vhco:port` (in this feature's `ports.go`). The ports become the use case's `through` in flows and
  the parameters the orchestrator must satisfy.

```go
// vhco:usecase notes.add_note(text: string) -> Note needs NoteStore
func AddNote(text string, store NoteStore) (Note, error) { … }
```

Use cases are **pure** (capabilities arrive as the listed ports) and **surface-blind** — do not put a
`trigger` or a surface example in a use-case file. That belongs to the surface (the orchestrator).

### `surface` — a named I/O entry point and the actions it routes

```
<comment> vhco:surface <name> [kind <kind>] [calls <action>, <action>, …]
```

- `kind` is **optional** and free-ish text describing *how* interaction happens: `cli`, `http`, `screen`,
  `view`, `websocket`, `worker`, … The explorer presents the surface as **I/O** and shows this kind.
- `calls` is a **comma list** of actions (`feature.action` or `feature/action` — both normalize the same).
  This is the per-surface registration: it lists every action reachable from this surface.

```go
// vhco:surface http kind http calls notes/add_note, notes/list
// vhco:surface cli  kind cli  calls notes/add_note, notes/list, notes/delete
```

Registration is **per surface**. One feature can be reached from many surfaces simply by listing its action in
each surface's `calls` — `notes/add_note` above is on both `http` and `cli`. Each surface needs an
`orchestrator/setup_<surface>` file or `validate` reports `IssueMissingSetup`.

### `infra` — an adapter and the ports it satisfies

```
<comment> vhco:infra <name> [satisfies <Port>, <Port>, …]
```

- `satisfies` is an optional **comma list** of the ports this adapter implements. It connects a concrete
  adapter to the port contract(s) it fulfils, shown in the **Outside world** view.

```go
// vhco:infra sqlite_store satisfies NoteStore
```

### `feature` — a capability (usually inferred)

```
<comment> vhco:feature <name>
```

The feature name is normally inferred from the `features/<name>/` folder, so you rarely need this. Use it only
when the folder doesn't make the feature obvious (e.g. descriptive mode over a non-five-folder codebase).

### `wiring` — an orchestrator binding (for the map)

```
<comment> vhco:wiring <name>          [ -- composes the … surface ]
```

A located marker for an orchestrator binding. It's descriptive (shows where wiring happens); the real
surface→action structure comes from `surface`.

```go
// vhco:domain Note { id: string; text: string; done: bool }
// vhco:port NoteStore { add(Note) -> error; list() -> []Note; delete(id: string) -> error }
// vhco:usecase notes.add_note(text: string) -> Note needs NoteStore
// vhco:surface http kind http calls notes/add_note, notes/list
// vhco:infra sqlite_store satisfies NoteStore
// vhco:feature notes        -- (only if the folder doesn't make it obvious)
// vhco:wiring setup_http     -- composes the http surface
```

---

## The presentation directives (frame an item for the explorer)

These attach human content to the **most recent structural item** above them — they never create model
structure and are **ignored by `validate`/`sync`** (they only change what `vhco doc` shows). Put the first four
directly above the use case (or domain type) they describe.

```go
// vhco:usecase notes.add_note(text: string) -> Note needs NoteStore
// vhco:label Add a note
// vhco:about Creates a note from text and stores it.
// vhco:example text="buy milk" => { "id": "n1", "text": "buy milk", "done": false }
// vhco:meta idempotent = true
// vhco:ref spec = https://example.com/spec#add
func AddNote(text string, store NoteStore) (Note, error) { … }
```

| Directive | Body shape | Produces |
|---|---|---|
| `label` | free text | a friendly title for the item |
| `about` | free text | a plain-language description (the note after ` -- ` is appended) |
| `example` | `<input> => <result>` (`=>`, `->`, or `→`) | a worked sample; bare text with no separator is just input |
| `meta` | `<Label> = <value>` | a labelled fact (bare text → label only) |
| `ref` | `<label> = <target>` or bare `<target>` (URL or path from project root) | a reference link; a bare target takes its label from the ` -- ` note |

The explorer leads with this content (title · "how to use it" · examples · facts) behind a **Simple ⇄
Technical** toggle.

---

## Triggers, edges, traceability, markers — quick reference + pointers

These have dedicated pages; here is just enough grammar to place them correctly.

### `trigger` (in `orchestrator/setup_<surface>.go`)

```
<comment> vhco:trigger <surface> <feature>/<action> = <how it's invoked>   [ -- note ]
```

`<feature>/<action>` (or `feature.action`) is normalized to `feature.action` and must appear in that surface's
`calls`, or `validate` flags a dangling trigger. The right-hand side is the literal command/route/gesture.

```go
// vhco:trigger cli notes/add_note = notes add "<text>"
// vhco:trigger http notes/add_note = POST /notes
```

Full grammar for `trigger` + `api`/`request`/`response`: [Markers](06-markers.md).

### Edges — `env` / `arg` / `file` / `net` / `db`

```
<comment> vhco:env  <NAME>                     [ -- note ]
<comment> vhco:arg  <--flag or name>           [ -- note ]
<comment> vhco:file [read|write|rw|append|delete] <path>    [ -- note ]
<comment> vhco:net  [GET|POST|…|WS|TCP] <endpoint>          [ -- note ]
<comment> vhco:db   [read|write|rw|migrate] <target>        [ -- note ]
```

`file`/`net`/`db` accept an optional leading **mode/method** token (file defaults to `read`); `env`/`arg` take
the value verbatim. An edge attaches to the use case OR infra adapter it sits under, and rolls into the
**What it touches** inventory.

```go
// vhco:infra sqlite_store satisfies NoteStore
// vhco:file write ~/.notes/db.sqlite -- the notes database
// vhco:env NOTES_DB -- override the database path
```

> **Edges go where the effect *happens* — usually the infra adapter** (or a pure use case that genuinely owns
> the effect). Triggers go where the *surface invokes* an action — the orchestrator. Don't confuse the two:
> an edge says "this code touches the outside world", a trigger says "this surface calls this action".

Full design for edges: [Markers](06-markers.md).

### Traceability — `todo` / `step` / `test`

```
<comment> vhco:todo <id>                        -- <what it does>
<comment> vhco:step <id> <ref…> [@lane]         -- <role>
<comment> vhco:test <subject>                   -- <what it verifies>
```

`todo` and `step` attach to the enclosing use case; `test` sits above a test function. `todo` is the one
annotation that is **checked by `sync`** (every contract todo must be claimed). Full design:
[Todos, steps & tests](05-traceability.md).

### Markers — `error` / `capture_start` / `capture_end` / `custom`

```
<comment> vhco:error <id> -- <when> => <code> [returns|wraps [target]|handled]
<comment> vhco:capture_start <id> [title…]  -- <why>
…region of code…
<comment> vhco:capture_end [<id>]
<comment> vhco:custom <name> <value…>        [ -- note ]
```

`custom` is the open-ended escape hatch: `<name>` becomes the marker's **kind** so customs group by their own
category, and it never touches the structural model. Full design: [Markers](06-markers.md).

---

## How comment symbols are resolved per language

The scanner picks the comment symbol from the file's extension. Built-ins cover most languages:

| Symbol | Extensions (built-in) |
|---|---|
| `//` | `.go .rs .ts .tsx .js .jsx .java .c .h .cpp .hpp .cs .swift .kt .scala .dart .php .zig` |
| `#`  | `.py .rb .sh .bash .pl .r .ex .exs .yaml .yml .toml .nim` |
| `--` | `.lua .sql .hs .elm .ada` |
| `;`  | `.clj .cljs .el .lisp` |

So the *same* directive looks like this in three languages:

```go
// vhco:usecase notes.add_note(text: string) -> Note needs NoteStore     (Go, Rust, TS — //)
```
```python
# vhco:usecase notes.add_note(text: str) -> Note needs NoteStore         (Python, Ruby — #)
```
```sql
-- vhco:usecase reports.daily(date: date) -> Report needs ReportStore    (SQL, Lua, Haskell — --)
```

For an extension that isn't built in, add it to `.vhco.json`:

```json
{ "comments": { ".myext": "//", ".weird": "##" } }
```

Project entries win over built-ins. The scanner only looks at files whose extension has a symbol — a file type
with no known symbol is skipped entirely (add it to `comments` to include it).

---

## Common confusion

- **`usecase`, not `use`.** `vhco:use` matches nothing and is dropped. The model field is also `usecase`.
- **`/` vs `.` in action refs.** Both work everywhere an action is named (`usecase`, `surface calls`,
  `trigger`, `api`). They normalize to the same internal `feature.action`. The HTML explorer *displays*
  `feature/use_case`; write whichever you like.
- **Separators differ by directive.** `domain` and `port` members use **`;`**. `usecase` inputs,
  `surface calls`, and `infra satisfies` lists use **`,`**. Mixing them up means members get swallowed —
  if a port shows only its first function, you probably wrote `,` instead of `;`.
- **The ` -- ` separator is exact.** Space, two hyphens, space. `text -- note` splits; `text--note` and
  `text—note` (em dash) do not. JSON bodies for `request`/`response` are re-joined after the split, so a `--`
  inside JSON is safe, but elsewhere a stray ` -- ` will truncate your directive into a note.
- **Annotation must be a whole comment line.** A trailing `// vhco:…` after code on the same line is ignored.
  Put the annotation on its **own** line above the code.
- **Unannotated code is invisible.** Adding a function does not add a use case — you must annotate it. There
  is no per-language signature reverse-engineering you can rely on.
- **Edges vs triggers.** Edges (`env`/`arg`/`file`/`net`/`db`) live on the code that performs the effect
  (infra adapter / use case). Triggers live in the orchestrator and bind a surface to an action.
- **Presentation directives need a structural item above them.** A `label`/`about`/`example`/`meta`/`ref`
  with no preceding `usecase`/`domain`/`port`/`surface`/`infra`/`feature` attaches to nothing and is dropped.

## FAQ

**Q. Do I have to annotate every function?** No — only the VHCO items: domain types, ports, use cases,
surfaces, infra adapters. Helper functions inside a use case need no annotation. But anything you want in the
model *must* be annotated.

**Q. What if I have two ports with the same name in different features?** Fine — ports belong to the feature
whose folder they sit under. Each feature declares its own ports (features never share ports; see
[Architecture](03-architecture.md)).

**Q. Can a directive span multiple lines?** No. Each directive is a single comment line. For multi-line
content (a captured block) use `capture_start`/`capture_end`. For long descriptions, `\n` and `\t` escapes are
expanded in `example`/`meta`/`request`/`response` bodies.

**Q. How do I include a file type the scanner ignores?** Add its extension and comment symbol to
`comments` in `.vhco.json`. With no symbol, the file is never read.

**Q. Where does the model come from if not signatures?** From your annotations plus `readdir` of the folder
tree. Annotations DEFINE and OVERRIDE; the tree gives the feature/use-case backbone. Go gets the richest
extra enrichment, but the annotation contract is what every language relies on.

**Q. My port shows up but with no functions — why?** A port is mandatory function-level. Check you used `;`
between members and `name(params) -> result` syntax. An empty `{ }` or comma-separated members yield no
functions.

## Canonical reference

The exhaustive grammar (every kind, every field, plus how the explorer renders each):
`docs/wwd/annotations-and-visualization.md`. The actual scanner is `infra/annotations.go`.
