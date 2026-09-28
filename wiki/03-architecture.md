# 3 · Architecture — the five folders & the closed world

[← Annotations](02-annotations.md) · [wiki index](README.md) · next: [validate vs sync →](04-validate-and-sync.md)

A VHCO program is **exactly five top-level folders**, with a strict **closed-world** import rule that
`vhco validate` enforces. The shape is the same in every language; only the spelling of the io bucket and the
import syntax change (Python spells it `surfaces/`; Rust nests the buckets under `src/`).

| Folder | Holds | May import |
|---|---|---|
| `domain/` | pure shared data types (the vocabulary) | **nothing internal** |
| `features/<feature>/` | **pure** use cases + their `ports.go` | `domain` (+ its own ports) |
| `io/<surface>/` | I/O surface adapters (`cli` / `http` / `worker` …) | `domain` |
| `infra/` | adapters that satisfy ports (stores, clients, backends) | `domain` |
| `orchestrator/` | the composition root: wire adapters → ports, register each surface | **everything** |

The rule in one line: `domain` imports nothing internal; `features`, `io`, `infra` import **only** `domain`
and never each other or `orchestrator`; **only `orchestrator` composes**. `validate` also allows a `docs/` and
a `tests/` folder at the top level — anything else top-level is a violation.

## The closed-world import matrix

Read a row as "this module is allowed to import that column".

| importer ↓ \ target → | `domain` | `features` | `io` | `infra` | `orchestrator` |
|---|---|---|---|---|---|
| **`domain`**       | —   | ✗ | ✗ | ✗ | ✗ |
| **`features/<f>`** | ✓   | ✗ (not even another feature) | ✗ | ✗ | ✗ |
| **`io/<surface>`** | ✓   | ✗ | ✗ (not another surface) | ✗ | ✗ |
| **`infra`**        | ✓   | ✗ | ✗ | (itself) | ✗ |
| **`orchestrator`** | ✓   | ✓ | ✓ | ✓ | (itself) |

Concrete violations (each is a real `validate` finding):

```go
// features/notes/add_note.go
import "myapp/infra/sqlite"   // ✗ a feature reaching for a concrete adapter — must take a PORT instead
```
```go
// features/notes/add_note.go
import "myapp/features/users" // ✗ feature-to-feature coupling — features never import each other
```
```go
// io/http/handler.go
import "myapp/features/notes" // ✗ io importing a feature directly — only the orchestrator joins them
```
```go
// domain/note.go
import "myapp/infra/clock"    // ✗ domain importing anything internal — domain is the leaf
```
```go
// infra/sqlite/store.go
import "myapp/orchestrator"   // ✗ infra reaching up to the composition root
```

The **only** module with no restrictions is `orchestrator`, because it is the sole place concrete adapters are
joined to ports. If two modules need to know about each other, that knowledge belongs in the orchestrator.

## The two load-bearing ideas

### Use cases are pure and surface-blind — and *why*

A use case receives **every external capability as a port parameter**. It never constructs an adapter, does
I/O, reads the clock, or knows whether a CLI, an HTTP request, or a background worker reached it. Two payoffs:

- **Testable in isolation** — there is nothing ambient to stub; a fake port passed as a parameter is the whole
  test setup.
- **Reachable from many surfaces** — because it knows nothing about *how* it was called, the same use case
  serves a CLI command, an HTTP route, and a screen tap with no change.

**Before (impure, surface-aware — wrong):**

```go
// features/notes/add_note.go
func AddNote(text string) (Note, error) {
    db := sqlite.Open("~/.notes/db")   // ✗ constructs a concrete adapter (I/O, hidden dependency)
    if os.Getenv("HTTP") != "" {       // ✗ knows about a surface
        text = sanitizeHTML(text)
    }
    return db.Insert(text)             // ✗ ambient effect, untestable without a real DB
}
```

**After (pure, surface-blind — right):**

```go
// features/notes/ports.go
// vhco:port NoteStore { add(Note) -> error; list() -> []Note }

// features/notes/add_note.go
// vhco:usecase notes.add_note(text: string) -> Note needs NoteStore
func AddNote(text string, store NoteStore) (Note, error) {  // capability arrives as a port
    n := Note{ID: newID(), Text: text}
    return n, store.add(n)                                   // no I/O, no surface knowledge
}
```

The HTML/sanitization decision moves to the **io** surface that needs it; opening the DB moves to **infra**;
joining them moves to the **orchestrator**.

### Ports carry function-level, typed contracts

A use case's `needs` are **ports**, and **each port is a set of named functions with typed inputs and
outputs** — never just a name. This is a mandatory invariant: a bare port name is not a contract.

```go
// features/notes/ports.go
// vhco:port NoteStore   { add(Note) -> error; list() -> []Note; delete(id: string) -> error }
// vhco:port IdGen       { next() -> string }
```

```go
// features/notes/add_note.go
// vhco:usecase notes.add_note(text: string) -> Note needs NoteStore, IdGen
func AddNote(text string, store NoteStore, ids IdGen) (Note, error) { … }
```

The function list is what lets `vhco doc` print the exact external-world contract per action
(`add(Note) → error`, `next() → string`), lets `validate`/`sync` reason about the surface, and lets a reader
know *precisely* what the outside world must provide. A port written as just `NoteStore` with no `{ … }` is
incomplete and a defect to fix.

## Registration is per surface, not per feature

Each surface owns its triggers in `orchestrator/setup_<surface>.<ext>` (or a `vhco:surface` annotation) and
lists the actions it routes in `calls`. Reaching one feature from **multiple** surfaces is just listing its
action in each surface's `calls` — never duplicated logic:

```go
// orchestrator/setup_cli.go
// vhco:surface cli  kind cli  calls notes/add_note, notes/list, notes/delete

// orchestrator/setup_http.go
// vhco:surface http kind http calls notes/add_note, notes/list
```

Here `notes/add_note` is reachable from both `cli` and `http`. Each surface needs its own
`orchestrator/setup_<surface>` file, or `validate` reports `IssueMissingSetup`. The action→surface mapping is
read from the orchestrator (the single binding site), so a shared surface backed by several features resolves
correctly in every language.

## The orchestrator is a composition root — no logic

`orchestrator/setup_<surface>` only **wires**: gather inputs from `infra` adapters, hand them to a
`features/*` use case, return the result. Every method should read as a thin forwarder — ideally one
expression. **Banned** in setup:

- business rules / conditionals on data (`if base == "…"`, policy decisions, choosing what to enforce);
- data shaping (filtering, dedup, combining results from two features);
- I/O (file reads/writes, path resolution beyond trivial glue).

When you reach for an `if`/loop/append in a setup method, refactor by the recipe:

| You're about to write… | Move it to… |
|---|---|
| a **decision based on a fact** (`if isAuthoredContract(path)`) | a named **`infra` helper** that returns the fact (`infra.IsAuthoredContract(path)`), then pass its result in |
| **combining / deciding over feature outputs** (diff + a conditional extra check) | **one feature entry point** that owns the policy (`syncspecs.Reconcile(expected, current, authored)`), so setup calls it once |
| **an effect** (read/write/stat/scan a file) | an **`infra` function** (`infra.ReadModelFile`, …) |

**Bad setup (logic has crept in):**

```go
// orchestrator/setup_cli.go
func (s *setup) Sync(path string) Result {
    expected := infra.ReadSpec(path)
    current  := infra.ScanCode(path)
    diff := diffSpecs(expected, current)          // ✗ data shaping in the orchestrator
    if filepath.Base(path) == "vhco-contract.json" {  // ✗ a policy decision on data
        diff = append(diff, extraContractChecks(current)...) // ✗ combining feature outputs
    }
    return render(diff)
}
```

**Good setup (a thin forwarder):**

```go
// orchestrator/setup_cli.go
func (s *setup) Sync(path string) Result {
    // the fact → infra; the policy (diff + conditional extra check) → ONE feature entry point
    return render(syncspecs.Reconcile(
        infra.ReadSpec(path),
        infra.ScanCode(path),
        infra.IsAuthoredContract(path),
    ))
}
```

The test is not "does `validate` pass" — it's *could this method be one expression?* If not, a decision, a
combination, or an effect is hiding in the orchestrator and belongs in `infra` (facts/effects) or `features`
(policy).

## What `validate` enforces here

Wrong/extra top folder · forbidden cross-module import · non-snake_case feature/use-case name · a feature with
no use case · an action mapped to >1 use case (the Exhaustive Use Case Invariant) · a surface with no
`setup_<surface>` file (`IssueMissingSetup`) · a dangling `vhco:trigger` (action not in that surface's
`calls`). It does **not** check reachability — an unwired use case is only a soft note in the explorer's
**Guarantees** view. Full detail: [validate vs sync](04-validate-and-sync.md).

## Gotchas

- **Features importing each other.** The most tempting violation. If `notes` needs something `users` knows,
  define a **port** on `notes` and have the orchestrator satisfy it with an adapter that calls `users` — the
  two features still never import each other.
- **A "sixth folder".** Any top-level folder other than the five (plus `docs/` and `tests/`) is a violation —
  including a `utils/`, `lib/`, `common/`, or `shared/` you create for "just a few helpers".
- **Shared helpers.** Pure shared *data* goes in `domain/`. A shared *capability* with side effects goes in
  `infra/` behind a port. There is no general-purpose shared-code folder — that's the point of the closed world.
- **Logic in `setup_*`.** The recurring drift. The moment a setup method grows an `if` on data, a loop, or an
  `append` that combines outputs, push it down (fact→infra, policy→one feature entry point, effect→infra).
- **`io` vs `infra`.** `io/<surface>` is the inbound edge (how users/systems reach the program — handler +
  controller); `infra` is the outbound edge (how the program reaches stores/clients/backends and satisfies
  ports). They never import each other.
- **Domain with behavior.** `domain/` is plain data and constants — no methods that do I/O, no imports of
  anything internal. An `Issue` is a code carried as data, not a function that prints.

## FAQ

**Q. Can features import each other?** No. Use a port + orchestrator wiring instead (see Gotchas).

**Q. Where do shared helpers go?** Shared data → `domain/`; shared effectful capability → `infra/` behind a
port. Never a new top-level folder.

**Q. What counts as a sixth-folder violation?** Any top-level directory besides `domain`, `features`, `io`
(`surfaces` in Python), `infra`, `orchestrator`, plus the allowed `docs/` and `tests/`.

**Q. Can two use cases share a port?** Within one feature, yes. Across features, no — each feature declares its
own ports, even if two interfaces look identical. This keeps features independently composable.

**Q. Does the orchestrator import `domain`?** Yes — it imports everything, including `domain`. It's the only
module with no import restrictions, because it's the single place concretes are joined to ports.

**Q. Why is the orchestrator allowed logic-free only?** Because every decision, combination, or effect it would
otherwise hold has a proper home (infra fact/effect, or a feature policy). Keeping setup as pure wiring makes
the composition auditable and the behavior testable where it lives.

## Canonical reference

`docs/wwd/how-it-works.md` and the human guide `docs/guide.md` §9. The import rule and rule set are specified
in `docs/ new/Vhco complete · MD.md`.
