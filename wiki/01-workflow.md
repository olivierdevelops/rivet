# 1 · The loop — every change, in order

[← wiki index](README.md) · next: [Annotations →](02-annotations.md)

When a human asks for a change, you run **six steps in order**. The contract changes *before* the code; the
human docs change *after* the code passes. Skipping a step (especially **test** or **docs**) means the change
isn't done.

```
  1. CONTRACT   Edit vhco-contract.json BY HAND — add/adjust the feature, use cases (in/out/needs),
                surfaces, flows, domain, and per-use-case todos. No code yet.

  2. EVAL       vhco live .  → http://127.0.0.1:7777   (auto-picks vhco-contract.json)
                The human audits the design in the browser. Edit JSON → it re-renders on save →
                iterate 1–2 until APPROVED.

  3. CODE       Write annotated code that realises the approved spec: domain types, ports, pure
                use cases, surface wiring, infra adapters, edges. Claim each contract todo with a
                // vhco:todo <id> comment as you implement it.

  4. TEST       Write a test for each use case you built; annotate each
                // vhco:test <feature.use_case> -- <what it verifies>. Run them and ITERATE on the
                code until they pass. Never leave the loop with red tests.

  5. VALIDATE   vhco validate .   → architecture rules — MUST be green (run after every edit, not just here)
                vhco sync .       → drift vs the contract — MUST reach 0
                vhco spec .       → regenerate vhco.json (the model) from the code

  6. DOCS       Now that it's legal, built, and tested: update the human docs to match.
                • README.md  — current features / surfaces / commands / examples
                • docs/      — add or revise the page for what you changed
                The README + docs/ must describe the project AS IT IS NOW.

  ↻  New requirement? Back to step 1 — the contract changes first, every time.
```

vhco **models, validates, and visualizes** a VHCO project from `vhco:` **comment annotations**. It does *not*
generate, scaffold, or compile code — *you* write every line by hand. The loop below is how you write it
without drifting from the agreed design.

---

## The six steps in depth

### 1 · CONTRACT — hand-edit `vhco-contract.json`

**What to do.** Open `vhco-contract.json` in your editor and write the *design delta*: the new feature, its
use cases (`in` / `out` / `needs`), the surfaces that expose them, the flows, any new domain types, and a
`todos` list on each use case. This is the one document where you express *intent before code exists*.

- It is one unified document: `{ schema, module, language, domain, definition: { features, surfaces, infra },
  flows }`. See [Annotations](02-annotations.md) for what each field means and the canonical model in
  `docs/guide.md`.
- Give every use case **at least one todo** (`"todos": [{ "id": "...", "todo": "..." }]`). `sync` enforces
  this against a hand-authored contract (`no_todos_declared`) — see [validate vs sync](04-validate-and-sync.md).
- Type the port functions: a `need` is a named port with `functions`, each with typed `in`/`out`. A bare
  port name is not enough.

**What "done" looks like.** The JSON is valid (parses) and expresses the *complete* desired shape of the
change. No code written yet.

**What goes wrong.**
- ⛔ Running `vhco spec . > vhco-contract.json` to "fill it in" — this **overwrites your design with the
  current code** and destroys the whole point of designing first. The contract is *only* ever hand-edited.
- Writing code first, then back-filling the contract — the human never got to evaluate the design cheaply.
- Forgetting `todos` — `sync` will report `no_todos_declared` and never reach 0.

### 2 · EVAL — render it and let the human approve

**What to do.** Run `vhco live .` (it serves `http://127.0.0.1:7777` and auto-picks `vhco-contract.json` when
present). The human reads the rendered design in the browser. Each time you save the JSON the page re-renders
over a local SSE stream — no `vhco.html` is written, no JSON is rewritten. Iterate between steps 1 and 2 until
the human says **APPROVED**.

- If you leave temporarily invalid JSON mid-edit, the server stays up and shows a "waiting for valid JSON"
  page, then reloads on the next valid save. (See `docs/wwd/live-llm-workflow.md`.)
- In live mode **the JSON is the desired project**, so the design shows even before any code exists. Once
  code exists, the Guarantees view overlays code-vs-contract drift.

**What "done" looks like.** The human has explicitly approved the design in the browser.

**What goes wrong.** Skipping straight to code because the change "seems obvious." The contract-first gate
exists so the human can correct the *design* — the cheapest place to be wrong.

### 3 · CODE — write the annotated implementation

**What to do.** Now write code that realises the approved spec, annotated with `vhco:` comments so the model
stays tied to source:

- `domain/` — pure data types (`vhco:domain`).
- `features/<feature>/ports.go` — the typed ports (`vhco:port`).
- `features/<feature>/<use_case>.go` — one **pure, surface-blind** use case per file (`vhco:usecase`).
  Capabilities arrive as **port parameters**; never do I/O or construct an adapter inside a use case.
- `infra/` — adapters that satisfy ports, with their edges (`vhco:infra` + `vhco:db/net/file/env/arg`).
- `orchestrator/setup_<surface>.go` — wire adapters → ports and register the surface (`vhco:surface`,
  `vhco:trigger`, `vhco:api`). Registration is **per surface**.
- **Claim each contract todo** with `// vhco:todo <id> -- <what it does>` above the implementing line.

Run `vhco validate .` *as you write each piece* — not just at step 5.

**What "done" looks like.** The code compiles, every contract todo has a matching `vhco:todo` claim, and
`vhco validate .` is green.

**What goes wrong.**
- Giving a use case knowledge of its surface (`vhco:trigger` or a `POST /…` example in the use-case file) —
  use cases are surface-blind; triggers live in `orchestrator/setup_<surface>.go`.
- Doing I/O inside a use case instead of passing a port → forbidden-import or impurity.
- Forgetting an annotation: anything unannotated **does not exist** in the model, so `sync` reports it
  missing.

### 4 · TEST — write tests and iterate to green

**What to do.** Write at least one test per use case you built. Annotate each with
`// vhco:test <feature.use_case> -- <what it verifies>` so the coverage shows in the explorer's **Tests**
view. Run the suite (`go test ./...`, or the project's test command) and **fix the code until it passes**.

**What "done" looks like.** All tests pass; every new use case has a `vhco:test`.

**What goes wrong.** Moving on with red tests, or testing through an adapter/I/O. Because use cases are pure,
you pass a *fake port* as a parameter — there's nothing ambient to stub.

### 5 · VALIDATE — the two independent gates

**What to do.** Run both gates; they check different things and must both be satisfied:

```sh
vhco validate .     # architecture rules — reads ONLY the code, compares against nothing. MUST be green.
vhco sync .         # drift vs the contract. MUST reach 0.
vhco spec .         # regenerate vhco.json (the GENERATED model) from the code's annotations.
```

- `validate` answers *"is this a structurally legal VHCO program?"* (folders, imports, snake_case names,
  per-surface registration, no dangling triggers, no action mapped to two use cases).
- `sync` answers *"does the code match the approved design?"* — it walks down to **0** as you implement.
- `vhco spec .` writes `vhco.json` (the generated model). It never touches `vhco-contract.json`.

**What "done" looks like.** `validate` green **and** `sync` at 0.

**What goes wrong.** "Fixing" sync drift with `vhco sync . --update-spec` — that rewrites `vhco.json` from
code, not the contract; if drift remains it means the *code is behind the design* and you should implement the
missing pieces. The contract changes only by hand, in step 1. Full details: [validate vs sync](04-validate-and-sync.md).

### 6 · DOCS — bring the human docs back in sync

**What to do.** Now that the change is legal, built, and tested, update the **human layer** so a person
reading only it gets a correct picture:

- **`README.md`** — current feature list, surfaces/commands, install/run, examples.
- **`docs/`** — add or revise the page for what you changed.

vhco **auto-detects** a top-level `README.*` and `docs/` folder (or honour an explicit `documentation` block)
and **indexes their contents** into the explorer's **Documentation** view and project search — so stale prose
is visible, not hidden.

**What "done" looks like.** A human reading only README + `docs/` would not be misled about the current state.
Re-render `vhco doc . --format html` and skim the **Documentation** view to confirm. Full guidance:
[Docs & README in sync](07-docs-and-readme.md).

**What goes wrong.** Calling the change "done" when validate/sync/tests pass but the README still describes the
old behaviour. Docs are part of the change, not an afterthought.

---

## Worked end-to-end example — adding `notes.archive_note`

A human asks: *"let users archive a note instead of deleting it."* Watch one small change thread through all
six steps.

**1 · CONTRACT.** In `vhco-contract.json`, under the `notes` feature, add a use case and a todo, and extend
the port:

```jsonc
{
  "name": "archive_note",
  "label": "Archive a note",
  "in": [{ "name": "id", "type": "string" }],
  "out": "Note",
  "needs": [
    { "name": "NoteStore",
      "functions": [
        { "name": "Archive", "in": [{ "name": "id", "type": "string" }], "out": "Note" }
      ] }
  ],
  "todos": [{ "id": "guard-missing", "todo": "return ErrNotFound when the id is unknown" }]
}
```

Add `notes.archive_note` to the surface's `calls` and add a flow entry exposing it.

**2 · EVAL.** `vhco live .` → the human sees `archive_note` in the explorer, confirms the `Archive` port
function and the `http` trigger, and says APPROVED.

**3 · CODE.** Create `features/notes/archive_note.go`:

```go
// vhco:usecase notes.archive_note(id: string) -> Note needs NoteStore
// vhco:label Archive a note
func ArchiveNote(id string, store NoteStore) (Note, error) {
    // vhco:todo guard-missing -- return ErrNotFound when the id is unknown
    return store.Archive(id)
}
```

Add `Archive(id string) (Note, error)` to `NoteStore` in `ports.go`, implement it on the infra adapter, and
in `orchestrator/setup_http.go` add `// vhco:trigger http notes/archive_note = POST /notes/{id}/archive` and
the `notes/archive_note` entry to the surface's `calls`. Run `vhco validate .` — green.

**4 · TEST.** `notes_test.go`:

```go
// vhco:test notes.archive_note -- archives a note and returns it; unknown id => ErrNotFound
func TestArchiveNote(t *testing.T) { /* fake NoteStore, assert both paths */ }
```

`go test ./...` → fix until green.

**5 · VALIDATE.** `vhco validate .` green. `vhco sync .` → was reporting `missing_use_case use case
notes.archive_note` and `todo_not_implemented ... todo guard-missing` while you built; now **0**. Run
`vhco spec .` to refresh `vhco.json`.

**6 · DOCS.** Add "Archive a note (`POST /notes/{id}/archive`)" to the README's feature/endpoint list and
update the notes page in `docs/`. Re-render `vhco doc . --format html`, confirm the Documentation view reads
correctly. **Done.**

---

## Why this order

- **Contract before code** lets the human evaluate the *design* before you spend effort building it. It's
  the cheapest place to be wrong.
- **`validate` continuously** (not only at step 5) means the code can never silently drift into an illegal
  shape — you catch it the moment you introduce it.
- **Test before you call it done** proves the use case actually works, and the `vhco:test` annotation makes
  that coverage visible in the explorer's **Tests** view.
- **Docs last** because the README + `docs/` describe the *finished* state — updating them earlier would
  document something that doesn't exist yet. See [Docs & README in sync](07-docs-and-readme.md).

## Definition of done

A change is done when **all** of these hold:

- `vhco validate .` is **green**,
- `vhco sync .` reports **0** drift (every contract use case built, every todo claimed),
- the **tests pass** (and each new use case has a `vhco:test`),
- the **README + `docs/`** reflect the new state.

If any one is missing, the change is not finished — even if the code compiles and runs.

---

## FAQ

**Why the contract before the code?** Because the contract is where the human evaluates the *design* — the
cheapest place to be wrong. Building first means re-doing real code when the human wanted a different shape.
The contract is also the only thing that can be "wrong" before any code exists, which is exactly why it goes
first.

**What if `validate` is green but `sync` isn't?** That's the **normal mid-build state**. The structure is
legal; you just haven't built every use case / claimed every todo the contract calls for yet. Keep
implementing — `sync` counts down to 0. Do *not* run `--update-spec` to silence it. See
[validate vs sync](04-validate-and-sync.md).

**Can I skip the contract for a tiny change?** No. Even a one-field tweak goes through the contract first, so
the model and the human review stay authoritative. Skipping it is how drift starts. The contract edit for a
small change is itself small — it's not a tax, it's the source of truth.

**What if the human changes the design mid-build?** Go **back to step 1**: edit `vhco-contract.json`, re-run
`vhco live`, get re-approval, then adjust code/tests. `sync` will reflect the new gap. Never let the code lead
the contract.

**Do I regenerate the contract with `vhco spec`?** Never. `vhco spec .` writes `vhco.json` (the *generated*
model). `vhco-contract.json` (the *design*) is hand-edited only. `vhco spec . > vhco-contract.json` is
forbidden — it overwrites your design with whatever the code currently is.

**`validate` doesn't flag my use case that no surface routes — is that a bug?** No. `validate` does **not**
check reachability. An unrouted use case shows only as a soft "X of Y reachable" note in the explorer's
**Guarantees** view, not as a violation. If it should be reachable, add it to a surface's `calls`.

## Common confusion

- **Two JSON files do opposite things.** `vhco-contract.json` = hand-authored *design* (never regenerated).
  `vhco.json` = *generated* model from `vhco spec`. `.vhco.json` = tooling config (comment symbols, rules).
- **`validate` and `sync` are independent and disagree on purpose.** `validate` reads only code against baked
  rules; `sync` compares code to the contract. Green on one says nothing about the other.
- **Docs are part of the change.** "Compiles and tests pass" is not done if README/`docs/` still describe the
  old behaviour.

## Related

- The gates in depth: [validate vs sync](04-validate-and-sync.md).
- What you hand-write in the contract vs what's derived: [Annotations](02-annotations.md) and the human
  guide `docs/guide.md` §3.
- The live-design workflow rationale: `docs/wwd/live-llm-workflow.md`.
