# 6 · Markers — errors · edges · API/CLI · captures

[← Todos, steps & tests](05-traceability.md) · [wiki index](README.md) · next: [Docs & README in sync →](07-docs-and-readme.md)

These directives enrich the model. **All are descriptive** — none of them change `validate` or `sync`, so
add them freely; you can never make the build go red with an error, edge, endpoint, capture, or custom
marker. (Todos are the *only* sync-checked traceability marker — see [05](05-traceability.md).) Each
directive goes on its **own comment line, directly above the code it describes**, and the scanner records
its `file:line` plus the next code line as context.

| Marker | Goes above… | Owner | Renders in |
|---|---|---|---|
| `env` `arg` `file` `net` `db` | the line doing the I/O | the **adapter** (or use case) it sits under | "What it touches" |
| `api` + `request`/`response` | the trigger in the surface setup | the surface/action | "API & CLI" |
| `error` | the failing/handling line in the use case | the **use case** | "How it can fail" + Errors view |
| `capture_start`/`capture_end` | brackets a region | the region | "Captured code" |
| `custom` | any line | the marker itself | the map, grouped by category |

---

## Edges — what the program touches (on the adapter)

Declare every external edge on the **`infra` adapter** that actually performs it — **not** on the use case.
Use cases are pure; they reach the world through ports, and the adapter is where the real I/O happens.

```go
// vhco:infra sqlite_store satisfies NoteStore
// vhco:db   readwrite notes        -- the notes table
// vhco:file readwrite ~/notes.db   -- the sqlite file
// vhco:net  GET https://api…/x     -- fetches something
// vhco:env  NOTES_DB               -- overrides the db path
// vhco:arg  --db <path>            -- db path flag
type SqliteStore struct{ … }
```

### Exact grammar (five directives)

```
<comment> vhco:env  <NAME>                  [ -- description ]
<comment> vhco:arg  <flag-or-name>          [ -- description ]
<comment> vhco:file [<mode>]   <path>       [ -- description ]
<comment> vhco:net  [<method>] <target>     [ -- description ]
<comment> vhco:db   [<mode>]   <table>      [ -- description ]
```

- **`env`** — an environment variable. The value is the var name, verbatim. No qualifier.
- **`arg`** — a CLI flag/argument. The value is the flag, verbatim. No qualifier.
- **`file`** — a filesystem path with an **optional leading mode**: `read`, `write`, `readwrite` (alias
  `rw`), `append`, `delete`. If the first token isn't one of these, the whole value is the path and the
  mode defaults to **`read`** (see `fileModes`/`parseEffect`).
- **`net`** — a network target with an **optional leading method**: `GET`, `POST`, `PUT`, `PATCH`,
  `DELETE`, `HEAD`, `OPTIONS`, or a protocol word `grpc`, `ws`, `wss`, `tcp`, `udp`. If the first token
  isn't a recognized method, the whole value is the target and the method is blank (see `netMethods`).
- **`db`** — a table/collection with an **optional leading mode**: `read`, `write`, `readwrite` (`rw`),
  `delete`, `migrate`. Default `read` (see `dbModes`).

```go
// vhco:file write ~/.cache/notes/        -- where exported notes are cached
// vhco:file ./config.json                -- no mode → defaults to read
// vhco:net  grpc worker:7000             -- dispatch a job to a worker
// vhco:db   migrate schema_version       -- runs the migrations table
```

### Where they attach (owner) and what renders

An edge's owner is whatever structural item it sits under (`parseEffect` + the dispatch in
`scanAnnotations`):

- under a `vhco:infra <adapter>` → owner is **that adapter** (preferred — the adapter performs the I/O);
- under a `vhco:usecase` → owner is **that use case** (allowed, but use cases are meant to be pure);
- anywhere else (`main.go`, an orchestrator file) → no owner, but it still appears in the inventory,
  located by `file:line`.

Edges roll up into the **What it touches** view, grouped by kind (Environment / Arguments / Filesystem,
badged by mode / Network, badged by method and grouped by host), each row linking to its `file:line` and
the owning adapter. They are marker-only: the tool never checks that a declared file is actually opened or
forbids an undeclared call — they document intent and reality, only as complete as you make them. Canonical
reference: `docs/wwd/effects-and-resources.md`.

---

## Screens & taps — UI triggers in plain language (on the widget)

For a project with a UI (mobile / web / desktop) whose explorer may be read by **non-technical people**,
`vhco:screen` + `vhco:tap` describe *how a user reaches a use case* in plain words — no code required.

```
// vhco:screen <id> [route <path>] [via <plain path>] -- <title>
// vhco:tap <feature/action> [@<screen>] [<gesture>] [at <placement>] -- <label>
```
```dart
// vhco:screen task_detail route /tasks/:id via Home > Tasks > tap a task -- Task detail
class TaskDetailScreen {
  // vhco:tap manage_notes/delete_note swipe at the top-right trash icon -- Delete this note
  IconButton(icon: Icon(Icons.delete), onPressed: /* … */)
}
```

- **`screen`** — names a screen and the human **navigation path** to reach it (`via Home > Tasks > tap a
  task`, rendered `Home › Tasks › tap a task`).
- **`tap`** — sits above the real widget: which use case fires, the **gesture** (`tap` / `long-press` /
  `swipe` / `submit` / `navigate` / …), **where** on the screen, and a plain **label**. `@<screen>` defaults
  to the enclosing `vhco:screen`; recorded with its `file:line`.

The use-case journey then reads, human-first — *"Delete this note — swipe · on **Task detail** · the
top-right trash icon — Home › Tasks › tap a task"* — with the source location as a small, secondary hint. A
**Screens** sidebar view lists every screen, its path, and each on-screen action → use case (clickable). These
are a richer, located alternative to a plain `vhco:trigger`; both are **descriptive** and never affect
`validate` / `sync`.

---

## API / CLI — endpoint docs (next to the trigger)

Next to a `vhco:trigger`, document the route or command. The request and response bodies are **destructured
JSON objects** — raw JSON strings the explorer parses into schema trees.

```go
// vhco:surface http kind http calls notes/add_note
// vhco:trigger http notes/add_note = POST /notes
// vhco:api     http notes/add_note POST /notes -- create a note
// vhco:request  { "text": "string — required" }
// vhco:response { "id": "string", "text": "string", "done": "bool" }
```

### Exact grammar

```
<comment> vhco:api <surface> <action> <verb> <path> -- <description>
<comment> vhco:request  { …json shape… }
<comment> vhco:response { …json shape… }
```

The `vhco:api` header is parsed by `parseEndpoint` from at least three whitespace fields:

- **`<surface>`** — the surface name (`http`, `cli`, …).
- **`<action>`** — the action it routes to, normalized to `feature.use_case` (a `/` is converted to `.`,
  so `notes/add_note` becomes `notes.add_note`).
- **`<verb>`** — if this field is a recognized HTTP method (`GET`/`POST`/… per `netMethods`), it's the
  verb and the **rest** is the path. **Otherwise there is no verb**, and the entire tail (from this field
  on) is the path — that's the CLI-command shape (e.g. `vhco:api cli notes/add_note notes add -- …` gives
  path `notes add`, verb empty).
- **` -- <description>`** — the endpoint description.

`vhco:request` / `vhco:response` attach to the **most recent `vhco:api`** in the file. Their body is stored
**raw** (trimmed) — the explorer `JSON.parse`s it into a schema tree. Because JSON values can themselves
contain ` -- `, the parser rejoins any ` -- ` that was peeled off, so a value like
`"string — required"` survives. (Use an em-dash `—` or `--` inside the string; both are kept.)

### Use JSON, not Go types

Write request/response as **destructured JSON objects** — keys mapped to type strings, nesting allowed —
not type references:

```go
// good — a schema tree the explorer can render:
// vhco:request { "ids": "[]string", "filter": { "done": "bool" } }

// bad — nothing resolves a Go type here:
// vhco:request []Message
```

There is nothing to look up: the JSON *is* the schema. Use raw JSON strings so the **API & CLI** view can
list every endpoint with its method/path, request/response schema, and a link to the use case behind it.

---

## Errors — how a use case can fail (on the use case)

Declare each way a use case can fail, above the line that produces or handles the failure.

```go
// vhco:usecase manage_notes.add_note(text: string) -> domain.Note needs NoteStore
func AddNote(text string, store NoteStore) (domain.Note, error) {
    // vhco:error empty-text -- text is blank => ErrInvalid returns
    if strings.TrimSpace(text) == "" { return domain.Note{}, ErrInvalid }
    // vhco:error store-down -- the store is unreachable => ErrIO wraps store.write-failed returns
    return store.Add(text)
}
```

### Exact grammar

```
<comment> vhco:error <id> -- <when it occurs> => <code> [returns | wraps [<target>] | handled]
```

Parsed by `parseFailure` + `extractPropagation`:

- **`<id>`** — the first token after `vhco:error`. Required; a stable handle for this failure mode.
- **`<when it occurs>`** — the text after ` -- ` and before `=>`. The trigger condition, in words.
- **`=> <code>`** — after `=>` comes the resulting error/code (e.g. `ErrInvalid`, `domain.Error{denied}`).
  Optional: with no `=>`, the whole note is taken as the "when" and there's no code.
- **propagation keyword** — a trailing `returns`, `handled`, or `wraps [<target>]` is peeled off the code
  expression and recorded separately:
  - **`returns`** (default) — propagates to the caller unchanged.
  - **`wraps <target>`** — this failure re-expresses a downstream one; `<target>` names the wrapped
    failure (e.g. `wraps store.write-failed`). This is the link that builds a traceback.
  - **`handled`** — caught/turned into a result here; the chain terminates.

So `=> ErrIO wraps store.write-failed returns` parses to code `ErrIO`, `wraps store.write-failed`,
propagation `returns`. The keyword is found by scanning the right side from the end, so it works regardless
of how many words the code expression has.

### Owner and rendering

`vhco:error` attaches to the **enclosing use case** (it must sit under a `vhco:usecase`; one elsewhere is
dropped). It renders a **"How it can fail"** section on the use-case page **and** an **Errors** sidebar view
with cyclic navigation back to the use case; the `wraps` links let the explorer chain failures into a
traceback (origin → … → surface). Descriptive — never `validate`/`sync` drift. Canonical reference:
`docs/wwd/implementation-trace.md`.

---

## Captures — spotlight code verbatim (anywhere)

Bracket a region (one function to several) to grab it **verbatim** into the **Captured code** view.

```go
// vhco:capture_start drift-engine The drift engine -- the reconcile algorithm
func Reconcile(…) { … }
// vhco:capture_end drift-engine
```

### Exact grammar

```
<comment> vhco:capture_start <id> [title…] -- <why it's captured>
... one to many functions of real code ...
<comment> vhco:capture_end [<id>]
```

- **`<id>`** (required) — the first token after `capture_start`; correlates start with end.
- **`[title…]`** — any words before ` -- ` become a friendly title (defaults to the id if omitted).
- **` -- <why>`** — the description: why this block is worth watching.
- **`capture_end [<id>]`** — with an id, closes the matching open capture (searched newest-first); **bare,
  it closes the most recent open one**. A stray `capture_end` with no matching open capture is ignored.
- **Nesting** — pairs may nest (a capture inside a capture); opens are a stack (`matchOpenCapture`).

The captured code is the source **strictly between** the two marker lines, with surrounding blank lines
trimmed and the **common leading indentation removed** (dedented — see `captureBlock`). Regions are capped
at 400 lines (`maxCaptureLines`); an over-long region is truncated with a trailing `…`. Captures surface in
`vhco.json` (`captures[]`), the `vhco query . "captures"` collection, the **Captured code** explorer view,
and a located `capture` marker in the source map. Use it for the few load-bearing pieces a reader must see
exactly. Canonical reference: `docs/wwd/captures.md`.

---

## Custom — your own categories

```
<comment> vhco:custom <category> <value> -- <note>
```

`vhco:custom http_status deprecated -- being removed in v2` records an arbitrary labelled fact. The
**`<category>` becomes the marker's kind** (so customs group by their own category in the map) and the
`<value>` is the marker name. A bare `vhco:custom <category>` alone keeps kind `custom` and uses the
category as the name. Marker-only: it never alters the structural model, so it can't cause `validate`/`sync`
drift.

---

## Common confusion

- **"Where do edges go — use case or adapter?"** — the **adapter**. The adapter is the thing that performs
  the I/O, so its `vhco:db`/`net`/`file`/`env`/`arg` declare what *it* touches. Putting them on a use case
  is allowed and they'll still appear in the inventory, but use cases are meant to be pure — prefer the
  adapter so "What it touches" links to the code doing the work.
- **"Why isn't my error/edge/capture causing drift?"** — none of these are reconciled by `sync`. Only
  todos are ([05](05-traceability.md)). Errors, edges, endpoints, captures, and customs are descriptive;
  there's no contract entry for them to disagree with. Refactor freely.
- **"Request/response must be JSON, not a Go type."** — write `{ "text": "string" }`, not `[]Message`.
  Nothing resolves a type reference; the JSON object *is* the schema the explorer renders.
- **"My `vhco:request` attached to the wrong endpoint."** — it binds to the **nearest preceding
  `vhco:api`**. Keep the `vhco:api` line immediately above its `vhco:request`/`vhco:response`.
- **"My CLI `vhco:api` lost its path / got a weird verb."** — the third field is treated as the verb only
  if it's a recognized HTTP method. For a CLI command (`notes add`), the third field isn't a method, so the
  whole tail becomes the path and the verb is empty — which is correct.
- **"My error has no code."** — you omitted `=>`. Without `=>`, everything after ` -- ` is the "when" and
  the code is blank. Add `=> <code>` to record the resulting error.
- **"`wraps` target."** — `wraps` may be bare (just "this wraps something downstream") or name a target
  (`wraps registry.fetch-failed`). The target is the `<feature>.<id>` (or node id) of the wrapped failure.
- **"My capture grabbed too much / odd indentation."** — capture grabs *everything between* the two
  markers, dedented to the shallowest line and capped at 400 lines. Tighten the brackets to narrow it.

## FAQ

**Q: Do edges/errors need to sit under a structural item?** Errors must sit under a `vhco:usecase` (else
dropped). Edges attach to whatever is above them — adapter, use case, or nothing (still inventoried). API
request/response attach to the nearest `vhco:api`.

**Q: What modes/methods are recognized?** file: `read`/`write`/`readwrite`(`rw`)/`append`/`delete`. db:
`read`/`write`/`readwrite`(`rw`)/`delete`/`migrate`. net: HTTP verbs plus `grpc`/`ws`/`wss`/`tcp`/`udp`.
An unrecognized leading token is treated as part of the value (file/db default to `read`; net leaves the
method blank).

**Q: Can a capture span multiple functions?** Yes — the region is whatever lies between `capture_start` and
`capture_end`, which can be several functions. Captures may also nest.

**Q: Does a `vhco:custom` show up anywhere special?** It appears in the map/"Where things are" view grouped
under its category (the `<category>` token becomes the kind). It's not in the Outside-world inventory —
that view is specifically env/arg/file/net/db.

**Q: Can JSON in `vhco:request` contain ` -- `?** Yes. The parser splits off a trailing ` -- ` note, then
rejoins it for request/response bodies, so a value like `"string — required"` is preserved. Prefer `—` or
keep `--` inside the quoted string.

**Q: Will any of these markers ever break `vhco validate .`?** No. Every marker on this page is descriptive.
The only sync-checked marker is the **todo** on [05](05-traceability.md).
