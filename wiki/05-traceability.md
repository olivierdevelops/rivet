# 5 · Todos, steps & tests — traceability

[← validate vs sync](04-validate-and-sync.md) · [wiki index](README.md) · next: [Markers →](06-markers.md)

Three markers make a use case's *work*, *execution*, and *coverage* visible. They answer three different
questions about the same use case:

| Marker | Question it answers | Lives in | Sync-checked? |
|---|---|---|---|
| **todo** | *what must this use case do?* | `vhco-contract.json` (declare) + code (claim) | **yes** |
| **step** | *how does it run, in order?* | code only | no (descriptive) |
| **test** | *does it actually work?* | test files | no (descriptive) |

Only **todos** are enforced by `sync`. Steps and tests are descriptive — they enrich the explorer's
journey and coverage views but **never** cause `validate` or `sync` drift. Add them freely; you can never
make the build go red by writing (or omitting) a step or a test marker.

> **Required: write todos and steps as the step-by-step LOGIC, not summaries.** Their whole purpose is
> traceability — a reader should reconstruct *how the use case works* (the decisions, the order, the data, the
> failure paths) from the todos and steps **without reading the body**. A one-word label or a restatement of
> the name is a defect. Todos = the plan in order (each a concrete unit of work, including guards and error
> paths); steps = the execution trace in order, each pointing at the real call and saying what happens there.
> Be concrete (`text` after trimming, `Note{…}`, `ErrInvalid`), not generic ("validate input", "save it").

---

## Todos — declare the work, claim it in code (enforced)

A todo is a *checked* work item: the contract declares the work a use case must do, the code claims each
one, and `sync` proves they match. It turns "the use case's intended work" from prose into a checklist the
build is measured against.

### Two halves: declare, then claim

**1. Declare** in `vhco-contract.json`, on the use case, as `{ id, todo }` objects:

```jsonc
// vhco-contract.json → definition.features[].use_cases[]
{
  "name": "add_note",
  "todos": [
    { "id": "validate", "todo": "reject empty text" },
    { "id": "persist",  "todo": "store the note and return its id" }
  ]
}
```

**2. Claim** in code, on a comment line **directly above the implementing line**:

```go
func AddNote(text string, store NoteStore) (domain.Note, error) {
    // vhco:todo validate -- reject blank/whitespace-only titles before persisting
    if strings.TrimSpace(text) == "" { return domain.Note{}, ErrEmpty }
    // vhco:todo persist -- write it to the store and hand back the new id
    return store.Add(text)
}
```

The `<id>` (`validate`, `persist`) is what's matched against the contract. The text after ` -- ` is a free
note describing what the code does — it does **not** have to match the contract's `todo` text. Matching is
**by id only**; the two descriptions are the same item seen from two angles and may differ freely.

A `vhco:todo` attaches to the **enclosing use case** — the nearest `vhco:usecase` above it in the same file
(exactly how `vhco:step` attaches). You don't repeat the use-case name on the todo. The scanner records the
claim's `file:line` so the explorer can show *where* each item is implemented.

### Exact grammar

```
<comment> vhco:todo <id> -- <what the code does>
```

- `<id>` — the first whitespace-delimited token after `vhco:todo`. Required. This is the only part `sync`
  matches against the contract.
- ` -- <text>` — the description (everything after ` -- `). Optional but encouraged. Free text.
- Must sit under a `vhco:usecase`. A `vhco:todo` with no use case above it is ignored (no owner to attach
  to), and it never becomes a contract claim — a silent no-op, not an error.

### What `sync` enforces (three findings)

`sync` compares the contract's declared todos against the code's claims, by id:

- **`no_todos_declared`** — a use case **in a hand-authored `vhco-contract.json`** that declares *no*
  todos. Every use case in a contract must declare at least one. This stops "an action exists but does
  nothing" from shipping. This rule is **automatic for contracts and only for contracts** — the generated
  `vhco.json` is exempt, so demo projects that don't use todos are unaffected. No config flag; it keys off
  the spec being a `vhco-contract.json`.
- **`todo_not_implemented`** — a declared contract todo with **no** matching `vhco:todo <id>` claim
  anywhere in the code.
- **`extra_todo`** — a `vhco:todo <id>` claim in code whose id is **not** declared in the contract. Keeps
  the lists honest in both directions.

```
$ vhco sync .
✗ use case manage_notes.add_note todo persist
    — no `vhco:todo persist` claim in the code   (todo_not_implemented)
✗ use case manage_notes.add_note todo audit
    — claimed in the code but not declared in the contract   (extra_todo)
```

When every declared todo is claimed and nothing extra is claimed, the todo drift is **0** — the same
shrink-to-zero model as the rest of `sync` ([validate vs sync](04-validate-and-sync.md)).

### Done = declared AND claimed

A todo is **done** only when it is *both* declared in the contract *and* claimed in code (so it carries a
`file:line`). A todo with a declaration but no claim is **pending** (`☐`). The explorer's use-case node
shows a `done/total` badge and one `✓`/`☐` row per item; opening the node lists each todo with a copyable
`file:line` (or "not implemented"). This presence-of-a-location is exactly how done vs pending is told
apart, in both the generated model (`vhco doc`) and the live view (`vhco live`). Canonical reference:
`docs/wwd/todos.md`.

### Worked example — adding a use case end to end

1. **Declare** the work in the contract:
   ```jsonc
   { "name": "delete_note",
     "todos": [
       { "id": "find",   "todo": "look the note up by id" },
       { "id": "remove", "todo": "delete it and report whether it existed" }
     ] }
   ```
2. `vhco sync .` now reports two `todo_not_implemented` findings (declared, not yet claimed).
3. **Write the code and claim each todo**:
   ```go
   func DeleteNote(id string, store NoteStore) (bool, error) {
       // vhco:todo find -- locate the note by id, 404 if missing
       n, ok := store.Get(id)
       if !ok { return false, nil }
       // vhco:todo remove -- delete it; return true since it existed
       return true, store.Delete(n.ID)
   }
   ```
4. `vhco sync .` → todo drift back to 0. The use-case node shows `2/2 ✓`.

---

## Steps — the execution trace (descriptive)

A step records one line of a use case's execution, in order, **directly above the code that performs it**.
Reading the step comments tells you how the use case runs without reading the body.

### Exact grammar

```
<comment> vhco:step <id> <ref…> [@lane] -- <what happens here>
```

```go
// vhco:step validate strings.TrimSpace      -- ensure the title is present
// vhco:step save     store.Add @store        -- write it to the store
// vhco:step verify   checkDigest @core-logic -- explicit lane when the ref is bare
```

- **`<id>`** — the first token: a short label for the step (`validate`, `save`, `pull`, `gate`). In the
  parsed model this is stored as the step's `Layer`/name.
- **`<ref…>`** — the concrete thing it calls or does. Everything after the id (minus any `@lane` token) is
  the ref. It can be `receiver.Method` (`store.Add`), a bare function (`parseArgs`), or a few words.
- **`@lane`** — an optional explicit lane tag. It can appear *anywhere* in the body; the parser pulls out
  any token starting with `@` and uses the rest as the lane name. It is removed from the ref.
- ` -- <what happens>` — plain-language intent for the step.

Order of the lines = order of execution. Steps attach to the **enclosing use case** (a step that is not
under a `vhco:usecase` is dropped). Steps are display-only — they never affect `validate`/`sync`.

### Lanes — how steps group by owner

Steps render as a **contained, lane-grouped block** in the journey (not loose floating cards). The **lane**
is the owner a step belongs to, resolved by this rule (see `parseStep`/`receiverOf` in
`infra/annotations.go`):

1. **Explicit `@lane`** if present — `store.Add @writer` → lane `writer`.
2. **Else the receiver of the ref**, lowercased — `store.Add` → lane `store`; `Cache.Get` → lane `cache`.
   The receiver is the part before the first `.` or `(`.
3. **Else empty** — a bare ref like `parseArgs` (no receiver, no `@lane`) has no resolved lane; in the
   explorer those steps read as the use case's own "core logic" lane.

So in the common case you write nothing extra: `vhco:step save store.Add -- write it` lands in the `store`
lane because the ref names the receiver. Add `@lane` only when the ref is bare or you want to override what
the receiver implies.

```go
// All three group under the "store" lane:
// vhco:step add    store.Add    -- append the note           (receiver = store)
// vhco:step count  store.Count  -- how many remain            (receiver = store)
// vhco:step flush  sync @store  -- bare ref, lane forced       (@store tag)
```

### A real example (lmx — `run_models.run`)

```go
// vhco:usecase run_models.run(model: domain.Ref, messages: []domain.Message) -> domain.GenerateResponse needs Resolver, Puller, Gate, Backend
// vhco:step resolve  resolver.Resolve  -- is the model installed locally?     (lane: resolver)
// vhco:step pull     puller.Pull       -- download the blobs when not local    (lane: puller)
// vhco:step gate     gate.Check        -- check the model is allowed by policy (lane: gate)
// vhco:step generate backend.Generate  -- produce the completion               (lane: backend)
func Run(ref domain.Ref, messages []domain.Message, ...) (domain.GenerateResponse, error) {
    manifest, installed, remote, err := resolver.Resolve(ref)   // ← step "resolve"
    if !installed { manifest, err = puller.Pull(remote, ref) }  // ← step "pull"
    decision, err := gate.Check(ref, manifest.Digest)           // ← step "gate"
    ...
}
```

A use case with **no** `vhco:step` annotations still gets a derived trace (the honest five-folder
io → use case → port path). Authored steps **override** that derived trace, because the program is then
declaring its own logic. Canonical reference: `docs/wwd/execution-traces.md`; design detail:
`docs/wwd/implementation-trace.md`.

---

## Tests — capture what's verified (descriptive, part of "done")

Above a test function, name the use case it covers and what it checks:

```go
// vhco:test manage_notes.add_note -- stores a note and returns its id
func TestAddNote(t *testing.T) { … }
```

### Exact grammar

```
<comment> vhco:test <feature.use_case> -- <what it verifies>
```

- **`<feature.use_case>`** — the subject under test (the text before ` -- `). By convention this is the
  `feature.use_case` it covers, which is how the explorer links the test back to its use case.
- ` -- <what it verifies>` — the assertion in words.
- The annotation captures the **next code line** as the test function's signature, plus its `file:line`.

The explorer's **Tests** view groups these by use case, with cyclic navigation back to each use case, and
shows which use cases have **no** test — your coverage map. Capturing a test never changes
`validate`/`sync`, **but** writing and passing tests is part of "done": every use case you build gets at
least one `vhco:test`, and you iterate until the suite is green before the change is finished (step 4 of
[the loop](01-workflow.md)).

---

## Together

Todos say *what the use case must do*, steps say *how it runs*, tests say *that it works* — three angles on
the same use case, all visible in its journey. The error model (*how it can fail*) and the other descriptive
markers (edges, API/CLI, captures) are on [Markers](06-markers.md).

---

## Common confusion

- **"Why isn't my test/step causing drift?"** — by design. Only todos are reconciled by `sync`. Steps and
  tests are descriptive: they have no contract entry to disagree with, so they can't drift. If you want a
  use case's work to be *checked*, declare it as a **todo**, not a step.
- **"My `vhco:todo` doesn't show up."** — it must sit under a `vhco:usecase` in the same file. A todo with
  no enclosing use case is silently ignored (no owner). Also check the id is exactly the first token.
- **"`extra_todo` — but I did the work!"** — the id you claimed in code isn't declared in the contract. Add
  `{ "id": "<that-id>", "todo": "…" }` to the use case, or fix the id to match an existing declaration.
  `sync` matches by id, not by description text.
- **"Lane from `@tag` vs receiver."** — `@lane` always wins. Without it, the lane is the ref's receiver
  (`store.Add` → `store`). A bare ref (`parseArgs`) has no receiver, so it has no lane unless you add `@`.
- **"Steps changed but sync stayed green."** — expected. Authored `vhco:step`s are descriptive; refactor
  them freely. (The contract's *flows* trace is a separate thing; you don't author steps there.)
- **"Do todo text and contract text have to match?"** — no. Matching is by `id` only.

## FAQ

**Q: How many todos does a use case need?** At least one, when it's in a hand-authored
`vhco-contract.json` (else `no_todos_declared`). The generated `vhco.json` is exempt.

**Q: Can two pieces of code claim the same todo id?** Yes — multiple `vhco:todo validate` claims all
satisfy the one declaration. The first claim's location is what the explorer shows; the point is the
declared id has *some* code.

**Q: Where exactly does a step/todo comment go?** On its own comment line, directly above the line of code
it describes. The scanner records that next code line as context.

**Q: Can a step reference something that isn't a real method?** Yes — the ref is free text. It's only used
for display and lane inference; nothing resolves it.

**Q: Does a `vhco:test` have to be in a `_test.go` file?** No — the marker works in any file with a known
comment symbol. By convention it sits above the test function. The subject (before ` -- `) is what links it
to a use case.

**Q: I removed a todo claim from code but `sync` is still green — why?** Check whether the contract still
declares it. If the contract no longer declares it either, there's nothing to reconcile. Drift only appears
when declaration and claim disagree.
