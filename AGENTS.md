


# AGENTS.md — working on a VHCO project

> Drop this file at the root of a **VHCO** project (a closed-world, five-folder program whose architecture
> is declared in `vhco:` comments). It tells coding agents how to change the project. Replace the
> **Project facts** section at the bottom with your project's specifics.

**Read this before writing any code.** This is a **VHCO** program. You work in a strict, repeating loop:
**update the contract → the human evaluates it → you write code → you test it → you validate → you update the
docs.** Writing code before the contract is approved, letting `vhco validate .` go red, or finishing with the
README/`docs/` out of date breaks the project even if it compiles.

---

## The loop — do this for EVERY feature or change

```
  1. CONTRACT   Update vhco-contract.json (the spec) BY HAND, FIRST.
                Add/change the feature, use cases (in/out/needs), surfaces, flows, domain, todos.
                No code yet.

  2. EVAL       Run:  vhco live .    → http://127.0.0.1:7777
                (always renders vhco-contract.json; before you've authored one it
                 renders live from the code instead.)
                The human audits the design in the browser and tells you what to change.
                Edit the JSON → the page re-renders on save → repeat 1–2 until APPROVED.

  3. CODE       Only once approved: write annotated code that realises the spec
                (domain types, ports, pure use cases, surface wiring, infra adapters, edges).
                Claim each contract todo with a // vhco:todo <id> comment as you implement it.

  4. TEST       Write tests for what you implemented (one per use case at least), annotate each
                with // vhco:test <feature.use_case> -- <what it verifies>, and run them.
                ITERATE on the code until the tests pass — do not move on with red tests.
                  e.g. go test ./...   (or the project's test command)

  5. VALIDATE   After every meaningful edit, and again once tests are green:
                  vhco validate .    → architecture rules (folders, imports, per-surface registration,
                                       snake_case names, no dangling triggers). MUST stay green.
                  vhco sync .         → drift vs the design contract. Shrinks to 0 as you implement.
                  vhco check .        → validate any declared app-level guarantees (UI taps reach actions;
                                        routes have request+response) against the model.
                  vhco spec .         → regenerate vhco.json (the generated model) from the code.

  6. DOCS       Once vhco validate . passes (and sync is 0), bring the human docs back in sync with
                the project as it now is:
                  • update the top-level README so it reflects the current features/surfaces/commands;
                  • update the top-level docs/ folder (add/adjust the page for what you changed).
                The README and docs/ must always describe the CURRENT state — never a past one.

  ↻  New requirement? Go back to step 1 — update the contract first, every time.
```

> **The rules that make the loop work:** (1) the contract changes **before** the code, so the human can
> evaluate the design before you build it; (2) `vhco validate .` is run **continuously** — re-run it as you
> write each piece, never only at the end — so the code can never silently drift from the contract; (3) every
> change ends **tested** (step 4) and with the **README + `docs/` updated** (step 6), so the human-facing
> docs never fall behind the code. A change isn't done until all three hold.

### `validate` vs `sync` — they check different things (don't confuse them)

| | **`vhco validate .`** | **`vhco sync .`** |
|---|---|---|
| Compares against | **nothing** — rules baked into the tool; reads only the code | a **JSON spec** (`vhco-contract.json` by default, else `vhco.json`) |
| Question | *Is this a structurally legal VHCO program?* | *Does the code match the design?* |
| Fails on | wrong/extra top folder · forbidden cross-module import · non-snake_case name · an action mapped to >1 use case · a surface with no `setup_<surface>` file · a `vhco:trigger` whose action isn't in that surface's `calls` (dangling trigger) | features / use cases / surfaces / ports / domain types in the spec but **missing** from the code (or **extra**) · a use case that declares **no todo** (`no_todos_declared`, when a contract is present) · a contract **todo** with no `vhco:todo` claim (`todo_not_implemented`) |
| Green means | the architecture is sound | the code == the approved design |

**They're independent and disagree on purpose:**
- **`validate` ✓ but `sync` ✗** — the normal mid-build state: the structure is legal, but you haven't built
  every use case the contract calls for yet. `sync` counts the remaining gap (→ 0 when done).
- **`validate` ✗ but `sync` ✓** — the code matches the design but breaks a rule (e.g. a feature imports
  infra). Still illegal.

> **`validate` does NOT check reachability.** A use case no surface routes is not a `validate` violation —
> it shows only as a *soft* "X of Y reachable" note in the explorer's *Guarantees* view.

---

## Golden rules (memorise these)

1. **Design first, in the spec.** Hand-author the design in **`vhco-contract.json`** and render it with
   `vhco live` so the human can audit it *before* you write code. Only implement once approved.
2. **Two files, two phases — never confuse them.** `vhco-contract.json` (the **spec**) is hand-written
   design. `vhco.json` (the **model**) is **generated** from the code's annotations by `vhco spec` and is
   **never hand-edited**. ⛔ **Never** `vhco spec . > vhco-contract.json` — it overwrites your design with
   the current code's model.
3. **Validate continuously.** Re-run **`vhco validate .`** as you write *each* piece, not once at the end —
   keep it green the whole way. Run **`vhco sync .`** alongside to watch the design gap close. The change is
   done when `validate` is green, `sync` is 0, and the build + tests pass.
4. **Use cases are pure.** Every external capability is a **port parameter** — never construct an adapter or
   do I/O inside a use case.
5. **Use cases are surface-blind.** A use case has no idea whether a CLI, HTTP request, or worker reached
   it. No `vhco:trigger`, no surface-specific examples in a use-case file.
6. **Annotate everything you add.** A type, port, use case, surface call, adapter, or external edge that
   isn't annotated does not exist in the contract.
7. **Every use case declares ≥1 todo, and todos + steps must spell out the LOGIC step by step.** With a
   hand-authored `vhco-contract.json`, `vhco sync` requires a todo (`no_todos_declared`). Declare
   `"todos": [{ "id", "todo" }, …]` per use case and claim each with `// vhco:todo <id> -- …` above the
   implementing line; trace the execution with `// vhco:step …` lines. **These are not vague summaries** —
   together the todos and steps must let a reader reconstruct *how the code works* (the decisions, the order,
   the data, the failure paths) without reading the body. This is **required**, see
   [Todos and steps must spell out the logic](#todos-and-steps-must-spell-out-the-logic-required).
8. **Test what you build, and iterate until green.** Every use case you implement gets at least one test,
   annotated `// vhco:test <feature.use_case> -- <what it verifies>`. Run the suite and fix the code until it
   passes — never leave the loop with failing tests. The **Tests** view shows which use cases still have none.
9. **Keep the README and `docs/` in sync with the code.** A VHCO project has a top-level **`README.md`** and a
   top-level **`docs/`** folder, and both must always describe the project *as it is now*. After
   `vhco validate .` passes, update them to reflect what you changed (new feature/surface/command/edge). The
   contract/model are the machine view; the README + `docs/` are the human view — both ship together.

10. **Proposals and reports follow the canonical templates — always.** Every document you add under
    **`docs/proposals/`** must match the structure of
    **[`docs/proposals/proposal-2026-0002-cuda-packed-f16-prefill-projections.md`](docs/proposals/proposal-2026-0002-cuda-packed-f16-prefill-projections.md)**
    (the reference template), and every document under **`docs/reports/`** must match its most recent sibling
    in that folder. This is not optional formatting — consistency is what lets a reviewer navigate the corpus.

    > **In this repository, `DOCUMENTATION.md` [§4.2](DOCUMENTATION.md#42-proposals--type-proposal-prefix-prop),
    > [§12.1](DOCUMENTATION.md#121-proposal--proposal-templatemd), and
    > [§21](DOCUMENTATION.md#21-project-standards-and-proposal-validation) are authoritative for proposals**
    > and supersede the outline below where they differ. They require three
    > mandatory sections the outline lacks: a numbered **Requirements** table (each with a source
    > outside the proposal), numbered **Proposed changes**, and a bidirectional **Requirements
    > alignment** matrix. A change with no requirement is scope creep; a requirement with no change
    > is an unmet demand — both fail review. See
    > [`docs/proposals/implemented/prop-2026-0001-documentation-system-expansion.md`](docs/proposals/implemented/prop-2026-0001-documentation-system-expansion.md)
    > for the reference example.

    **Proposal template (`docs/proposals/`)** — required, in order:
    - **YAML frontmatter** with all of: `document_id` (`PROPOSAL-YYYY-NNNN`, next free number),
      `title`, `document_type: proposal`, `status` (`proposed` | `implemented` | `rejected` | `superseded`),
      `created_date`, `last_updated`, `document_revision`, `authors`, `owner`, `reviewers`, `systems`,
      `components`, `affected_versions` (`from`/`to`), `applicable_environments`, `audience`, `scope` (one
      sentence), `reason` (one paragraph), `dependencies`, `related_documents`, `supersedes`, `superseded_by`,
      `tags`, `confidentiality`, `review_cycle`, `next_review_date`.
    - **Body sections**, in order: `# <Title>` → **Decision requested** (or *Decision and implementation
      status* once built) → **Problem and evidence** → **What the reference engines do** (each reference in
      its own subsection with an *applicable lesson* and a clickable link into `.ignore/references/…`) →
      **Proposed interface and scope** → **Design** (increments) → **Sample API calls and CLI commands**
      (required whenever a user-facing surface changes — see below) → **Verification and promotion gates**
      (correctness, then quality/performance, then an **Expected result before measurement** forecast table) →
      **Risks and rollback** (table) → **Contract delta** → **Change History** (table, newest row first).
    - **A complexity assessment is MANDATORY** for any proposal that proposes building something:
      a short `## Complexity: N/5` section rating **engine reuse**, **new kernels/math**,
      **blast radius** (can it break already-shipped paths?), whether a **quality methodology
      already exists**, and **unknowns** — with a one-line verdict. State plainly that *difficulty
      is not effort*: a task can be long but easy, or short but hard. A reviewer deciding what to
      schedule needs to know which items are safe additions and which touch the engine everything
      else depends on.
    - **Sample API calls and CLI commands are MANDATORY** for any proposal that adds or changes a
      user-facing surface (an HTTP route, a CLI command, a flag). Show **copy-pasteable** examples:
      the exact `xllm …` invocations including flags, and a `curl` per route with a realistic request
      body **and the response shape**. A proposal that describes a new command without showing what a
      user actually types is incomplete — the samples are how a reviewer judges the ergonomics before
      the design is locked, and they become the docs/tests later. For a *declined* capability, show
      what a user gets instead (the typed refusal), so the failure mode is reviewed too.
    - **Honesty rules baked into the format**: state the non-goal explicitly in `scope`; put a forecast table
      with "a too-good result must be investigated" caution; never claim a gain the design can't produce.

    **Report template (`docs/reports/`)** — a dated `# <Title>` with a **Date + source** line, a **TL;DR /
    headline** first, then evidence → analysis → an honest scorecard/status table; link the proposal or plan it
    feeds. Match the newest existing report's shape.

    A proposal is a **decision request** — do not write code for its contract until the human approves it and the
    hand-authored `vhco-contract.json` terms pass `vhco live .` review (the contract-first non-negotiable).

---

## Todos and steps must spell out the logic (required)

The whole point of `vhco:todo` and `vhco:step` is **traceability**: a reader (human or another agent) should
be able to open a use case and understand *exactly how it works* from its todos and steps — the decisions it
makes, the order it does things, the data it builds, and how it can fail — **without reading the function
body**. A one-word label or a vague restatement of the name defeats this. Write them as the **step-by-step
logic of the implementation.**

**Rules:**

- **Todos = the plan, in order.** The set of todos for a use case should read like the numbered steps of the
  algorithm. Each todo names a concrete unit of work and says *what it does and why* — including the guard
  conditions and the failure path, not just the happy path.
- **Steps = the execution trace.** One `vhco:step` per meaningful operation, **in execution order**, each
  pointing at the real call/ref and describing what happens at that point (what's checked, what's built, what
  flows out, what error is returned). The lane groups by owner (`store.Add` → `store`).
- **Concrete, not generic.** Reference the actual values, types, and conditions (`text` after trimming, a new
  `Note{…}`, `ErrInvalid`), not "validate input" / "do the work" / "save it".
- **Cover the branches.** If the code can return early or error, a todo/step must say so and when.

**Bad — vague, tells you nothing the name didn't:**

```go
// vhco:todo validate -- validate input
// vhco:todo persist  -- save the note
func AddNote(text string, store NoteStore) (Note, error) {
    // vhco:step save store.Add -- save
    ...
}
```

**Good — a reader can reconstruct the logic:**

```go
// vhco:todo validate -- trim surrounding whitespace; if the text is empty after trimming, return ErrInvalid and write nothing
// vhco:todo persist  -- mint a unique id, build Note{id, text, done:false}, hand it to store.Add, and propagate any store error unchanged
func AddNote(text string, store NoteStore) (Note, error) {
    // vhco:step trim   strings.TrimSpace -- normalise so "   " counts as empty
    // vhco:step guard  return            -- empty text → return ErrInvalid before any write (no side effects)
    // vhco:step build  newID             -- mint a unique id and assemble the Note (done defaults to false)
    // vhco:step save   store.Add         -- persist through the port; on error return it unwrapped so callers see the cause
    ...
}
```

Apply the **same standard in the contract**: each `"todos"` entry's `todo` text is the detailed plan, not a
label — the contract is where a human reviews the intended logic before you build it. The corresponding
`vhco:error` lines document *how each failure path occurs* (see the marker rules below).

---

## The five folders (where code goes)

| Folder | Put here | May import |
|---|---|---|
| `domain/` | pure data types (the shared vocabulary) | nothing internal |
| `features/<feature>/` | pure use cases + their `ports.go` | `domain` (+ own ports) |
| `io/<surface>/` | surface adapters (`cli` / `http` / `worker` …) | `domain` |
| `infra/` | adapters that satisfy ports (stores, clients, backends) | `domain` |
| `orchestrator/` | composition root: wire adapters → ports, register each surface | everything |

The closed-world import rule is enforced by `vhco validate`: `domain` imports nothing internal; `features`,
`io`, `infra` import only `domain` and never each other or `orchestrator`; only `orchestrator` composes.

---

## Not on Go / Python / Rust / TypeScript? Configure the language (don't fake one)

Modeling works for **any** language out of the box (annotations are comments). But `vhco validate`'s two
code-reading checks — **registration** (`orchestrator/setup_<surface>.<ext>`) and the **import-boundary**
check — need to know your language's file extension and import syntax. For Go/Python/Rust/TypeScript that's
built in; for **anything else (Dart/Flutter, Kotlin, Swift, …) declare it in `.vhco.json`** and the project
becomes a first-class VHCO project — full `validate` enforcement, no workarounds:

```jsonc
// .vhco.json
{
  "language": {
    "name": "dart",                                        // any label except "go"
    "ext":  ".dart",                                       // use-case files + setup_<surface>.dart
    "import": "^\\s*import\\s+['\"](?:package:[^/]+/)?(?:\\.\\./)*(?:lib/)?(domain|features|surfaces|infra|orchestrator)/"
  },                                                        // capture group 1 = the imported bucket (drives the boundary check)
  "dirs": { "io": "surfaces" }                             // rename a bucket's folder (e.g. Python/Dart use surfaces/ for io)
}
```

- **`ext`** lets `validate` recognise `orchestrator/setup_<surface>.<ext>` (so registration passes).
- **`import`** is a regex whose **first capture group is the imported bucket** — that's what the closed-world
  check reads to catch e.g. a `features/` file importing `infra`.
- **`dirs`** renames a bucket's on-disk folder (folder names are independent of language). Absent config, the
  `io` bucket auto-resolves to `surfaces/` when that folder exists.

⛔ **Never** make a `setup_<surface>.py` (or any off-language) stub to force `validate` green — it's a "Dart
pretending to be Python," and the import check becomes cosmetic. Use the `language` config above, or
**descriptive mode** (`{ "mode": "descriptive" }`) if you don't want the architecture gate at all.

---

## Annotation quick-reference

Place each `// vhco:<kind> …` line **directly above** the code it describes. (Example below is a generic
notes service; comment symbol shown is `//` — it differs per language but the grammar is identical.)

```go
// domain/ — a shared data type
// vhco:domain Note { id: string; text: string; done: bool }

// features/notes/ports.go — a port (typed capability the use case needs)
// vhco:port NoteStore { add(Note) -> error; list() -> []Note; delete(id: string) -> error }

// --- features/notes/add_note.go — the USE CASE (PURE & SURFACE-BLIND) ---
// vhco:usecase notes.add_note(text: string) -> Note needs NoteStore
// vhco:label Add a note
// vhco:about Creates a note from text and stores it.
// vhco:example text="buy milk" => { "id": "n1", "text": "buy milk", "done": false }   // logical inputs => result
func AddNote(text string, store NoteStore) (Note, error) {
	// vhco:todo validate -- reject empty text         // claims a contract todo (checked by sync)
	// vhco:step save store.Add -- persist the note     // groups under the "store" lane (from the ref receiver)
	// vhco:error empty-text -- text is blank => ErrInvalid returns   // a declared failure mode
	...
}

// --- infra/ — the ADAPTER (declares what it actually TOUCHES) ---
// vhco:infra sqlite_store satisfies NoteStore
// vhco:db readwrite notes        -- the notes table
// vhco:file readwrite ~/notes.db -- the sqlite file
// vhco:env NOTES_DB              -- overrides the db path
type SqliteStore struct { ... }

// --- orchestrator/setup_http.go — SURFACE + TRIGGERS + API DOCS ---
// vhco:surface http kind http calls notes/add_note, notes/list
// vhco:trigger http notes/add_note = POST /notes
// vhco:api http notes/add_note POST /notes -- create a note
// vhco:request  { "text": "string — required" }
// vhco:response { "id": "string", "text": "string", "done": "bool" }

// --- notes_test.go — the TEST ---
// vhco:test notes.add_note -- stores a note and returns its id
func TestAddNote(t *testing.T) { ... }
```

**Every directive, by where it lives:**

| Lives in | Directives |
|---|---|
| `domain/` | `domain` |
| `features/<f>/` | `feature` (only when the feature can't be inferred from the folder) |
| `features/<f>/ports.go` | `port` |
| `features/<f>/<uc>.go` (use case, pure & surface-blind) | `usecase`, `label`, `about`, `example` (logical), `meta`, `ref`, `step`, `todo`, `error` |
| `infra/` (adapter) | `infra`, and the edges it performs: `env`, `arg`, `file`, `net`, `db` |
| `orchestrator/setup_<surface>.go` | `surface`, `wiring`, `trigger`, `api`/`request`/`response`, surface-specific `meta` |
| `*_test.go` | `test` |
| anywhere | `capture_start` / `capture_end`, `custom` |

- **`step`** — one execution-trace line: `// vhco:step <id> <ref> [@lane] -- <what happens>`. The **lane**
  (from `@<lane>` or the ref receiver, e.g. `store.Save` → `store`) groups steps by owner in the journey.
  Display-only.
- **`todo`** — a checked work item; declared in the contract, claimed in code; enforced by `sync`.
- **`error`** — `// vhco:error <id> -- <when> => <code> [returns|wraps|handled]`; a declared failure mode,
  shown in the use-case page and the **Errors** view.
- **`api`/`request`/`response`** — document a route/command; request & response are **destructured JSON
  objects** rendered as schema trees in the **API & CLI** view.
- **`capture_start`/`capture_end`** — grab a code region verbatim into the **Captured code** view.

---

## Do / don't

**Do**
- Decide the contract delta first, then write code to match it.
- Keep use cases pure — capabilities arrive as port parameters. One use case per file.
- Annotate every new type, port, use case, surface call, adapter, and external edge.
- Declare edges on the **adapter** that performs them (`vhco:db/net/file/env` above `vhco:infra`), not the
  use case.
- Reach a feature from multiple surfaces by adding it to each surface's `calls` — never duplicate it.
- Run `vhco validate .` and `vhco sync .` after every change; end green.

**Don't**
- **Don't let a use case know its I/O surface** — no `vhco:trigger`, no `lmx …`/`POST /…` example, no
  surface-specific `vhco:meta` in a use-case file. Those live in `orchestrator/setup_<surface>.go`.
- Don't do I/O inside a use case — pass a port; put the edge on the adapter.
- Don't import across modules illegally (feature→infra, io→feature, anything→orchestrator).
- Don't add a sixth top-level folder, or put two use cases in one file.
- Don't hand-edit `vhco.json`; it's generated. ⛔ Don't regenerate the **contract** from code.
- Don't register actions per feature — registration is per **surface**.

---

## Commands (run from the project root)

```sh
# design phase — vhco-contract.json is HAND-AUTHORED. Edit it in your editor.
vhco live .                  # render the spec in the browser (auto-picks vhco-contract.json if present)
# ⛔ NEVER run `vhco spec . > vhco-contract.json` — it overwrites your hand-authored design.

# implementation phase
vhco validate .              # architecture rules — MUST stay green (run after every edit)
vhco sync .                  # drift vs the design contract; 0 = built
vhco check .                 # validate declared app-level guarantees (ui.action_present, api routes) — non-zero on failure
vhco assure .                # ONE read-only gate: validate + sync + guarantees + tests + changed-file scope
vhco spec .                  # regenerate vhco.json (the GENERATED model) — never the contract
vhco sync . --update-spec    # writes vhco.json from code; never overwrites vhco-contract.json
vhco doc . --format html     # refresh the interactive explorer (vhco.html)

# inspect & render
vhco doc . --format md|html|mermaid   # md / interactive html / mermaid graph
vhco visualize .             # print every user action and the ports it needs
vhco flow .                  # per action: how it's received and handled, layer by layer
vhco coverage .              # how complete the annotations are
vhco query . "effects where kind=db"   # query the model (effects/actions/domain/surfaces/flows/markers/captures)
vhco diff <old.json> <new.json>         # behavioral delta between two snapshots
vhco version                 # the vhco tool version
```

If `vhco validate .` fails, fix the code/annotations — do not work around it. If `vhco sync .` reports
drift, the code is behind the design — implement the missing pieces until it reaches 0. Do **not** "fix" it
with `--update-spec`; the contract changes only by hand, in the design phase.

---

## Codebase intelligence (optional `.vhco.json` layers)

Beyond modeling, the tool can **enforce code policy and capabilities** from `.vhco.json`. All are optional;
add the blocks you want and run `vhco scan .` (add `--sarif` for CI, `--json` for agents).

```jsonc
// .vhco.json
{
  "comments": { ".myext": "//" },              // custom comment symbol for an extension
  "extract":  { "todo-markers": { "match": ["TODO", "FIXME"] } },  // lexical extractors → located signals
  "rules": [                                    // scoped assertions over the code
    { "name": "no-eval", "assert": "deny",      "match": ["eval("], "in": ["features/**"], "why": "no eval" },
    { "name": "license", "assert": "require",   "match": ["SPDX-License"], "in": ["**/*.go"] }
  ],
  "conventions": { "document-edges": { "require": ["net", "db"], "severity": "error" } }, // every net/db edge must be annotated
  "sandbox": {                                  // capability manifest — what the program is ALLOWED to touch
    "deny-undeclared": true, "on-violation": "error",
    "allow": { "net": ["api.example.com"], "file": ["~/.app/**"], "db": ["*"] }
  }
}
```

- **`vhco scan .`** runs the extractors + rules, detects inferred edges, checks the documentation
  convention, and enforces the capability **sandbox** (flags any `env/file/net/db` edge the manifest doesn't
  allow). `--sarif` emits CI-friendly output.
- **`vhco sandbox . --profile summary|netpolicy|seccomp`** compiles the capability manifest into a runtime
  enforcement profile.
- **`.vhco.overlay.json`** adds **sidecar edges** for code you can't annotate (vendored/generated) — a
  zero-touch way to complete the "What it touches" picture.

Rule asserts: `deny` (must not appear in scope), `allow-only` (only in the given globs), `require` (must
appear), `max <n>` (at most n hits). These are about *code hygiene*, separate from `validate` (architecture)
and `sync` (design drift).

---

## Keeping the human docs in sync (step 6, every change)

A VHCO project ships **two layers of documentation that must never disagree**:

| Layer | Files | Audience | Kept current by |
|---|---|---|---|
| **Machine model** | `vhco-contract.json` (design) + `vhco.json` (generated) + `vhco.html` | the tool / reviewers | steps 1 & 5 of the loop |
| **Human docs** | top-level **`README.md`** + top-level **`docs/`** folder | people | **step 6 of the loop** |

After `vhco validate .` passes, you are **not done** until the human docs match the new state:

- **`README.md`** — keep its feature list, surfaces/commands, install/run steps, and any examples current.
  If you added a surface or command, it must appear here. (vhco auto-detects a top-level `README.*` and a
  `docs/` folder when no `documentation` block is configured, and the explorer's **Documentation** view and
  search index their contents — so stale docs show up there too.)
- **`docs/`** — the prose explanations (architecture notes, how-tos, per-feature pages). Add or revise the
  page for whatever you changed; don't leave a doc describing a use case or edge that no longer exists.

Rule of thumb: if a human reading only the README + `docs/` would get a **wrong** picture of the current
project, the change isn't finished. Re-render `vhco doc . --format html` and skim the **Documentation** view
to confirm nothing reads stale.

---

## FAQ & troubleshooting

**`vhco sync .` reports drift but my code is correct — what now?** Drift means the code and the contract
disagree, not that the code is broken. If the *code* is right and the *contract* is stale, fix the contract by
hand (step 1) — that's the only sanctioned direction. If the contract is right, implement the missing pieces.
Never run `vhco sync . --update-spec` to silence it: that rewrites the generated `vhco.json`, not the
contract, and just hides the gap. ⛔ Never `vhco spec . > vhco-contract.json` either — it destroys the design.

**`vhco validate .` is green but `vhco sync .` isn't — is that a problem?** No, it's the normal mid-build
state: the structure is legal, but you haven't built every use case the contract calls for yet. Keep going;
`sync` shrinks to 0 as you implement.

**`vhco validate .` fails with a forbidden-import error.** A module imported across a closed-world boundary
(e.g. a `features/` use case imported `infra`, or `io` imported `features`). Use cases receive capabilities as
**port parameters**; the adapter does the I/O; only `orchestrator` composes. Remove the import and pass a port.

**`vhco sync .` says `no_todos_declared`.** A use case in the contract has no todos. Every use case needs ≥1:
add `"todos": [{ "id", "todo" }]` to it in `vhco-contract.json`, then claim each with `// vhco:todo <id>` in
code. `todo_not_implemented` = declared but not claimed; `extra_todo` = claimed but not in the contract.

**A use case I added doesn't show as reachable.** `validate` does **not** check reachability — an unwired use
case is legal, shown only as a soft note in the explorer's *Guarantees* view. To make it reachable, add it to
a surface's `calls` (and a `vhco:trigger`).

**Where does an edge (db/file/net/env) annotation go — the use case or the adapter?** The **adapter**
(`vhco:db/net/file/env` above the `vhco:infra` that performs it). Use cases are pure and touch nothing
directly. Edges roll up into the *What it touches* view from the adapter.

**My `vhco:trigger` causes a validate failure (dangling trigger).** The trigger names an action that isn't in
that surface's `calls`. Add the action to the surface's `calls`, or remove the stray trigger.

**Do I have to update the README / `docs/`?** Yes, every change — step 6. The annotations keep the *model*
honest automatically, but the *prose* is kept current only by you. A README/`docs/` page describing something
that no longer exists is a bug.

**Can one feature be reached from several surfaces?** Yes — list it in each surface's `calls`. Never duplicate
the feature. Registration is per **surface**, not per feature.

---

## Going deeper — the agent wiki

This file is the quick, drop-in playbook. For fuller, navigable references (annotation grammar, the
architecture rules, validate-vs-sync, todos/steps/tests, the marker directives, keeping docs in sync, the
command set, and the codebase-intelligence layer), see the **VHCO agent wiki index**:
[`wiki/README.md`](wiki/README.md).

> This `docs/agents/` bundle is **self-contained**; its links are all internal. Backtick'd paths like
> `docs/wwd/…` refer to the vhco **tool's own source repository** (fuller references) and are not shipped in
> this bundle.

---

## Project facts (fill these in)

- **Module / language:** Rust (edition 2024, toolchain 1.90.0 pinned in `rust-toolchain.toml`). Comment symbol: `//`.
- **Workspace crates** (one version in `[workspace.package]`): **`rivet-runtime`** at the root — library name
  `rivet` (the crate-root facade; internals under `#[doc(hidden)] rivet::internal`) plus the `rivet` binary, which
  needs the `cli` feature — and **`rivet-ffi`** in `ffi/` — the C ABI library `librivet` (cdylib + staticlib,
  header `ffi/include/rivet.h` generated by cbindgen, `rivet.pc`). vhco reads only `src/`, so every FFI annotation
  and all FFI logic live in `src/orchestrator/setup_ffi.rs`; `ffi/` is a thin `extern "C"` shell (ADR-0005).
- **Cargo features** (`rivet-runtime`): `serve`, `grpc`, `quic` (and HTTP/3), `oauth` — default on — and `cli`
  (the binary). A bundle that needs a compiled-out adapter fails at load with `unsupported.feature` (exit 5,
  HTTP 501); `rivet-ffi` passes the runtime features through.
- **Features:** `language`, `registry`, `execution`, `files`, `connectors`, `audit`, `policy`, `auth`,
  `datagrams`, `quic`, `grpc`, `sessions`, `serve`, `transports` (each under `src/features/<feature>/` with one
  file per use case plus `ports.rs`).
- **Surfaces:** `cli`, `http`, `library`, `mcp`, `ws`, `poll`, `ffi` (each with `orchestrator/setup_<surface>.rs`;
  `setup_serve.rs` mounts the network surfaces on one listener). Every surface speaks the 0.2.0 envelopes
  ([API-2026-0006](docs/api/api-2026-0006-envelopes.md)).
- **Editors:** `editors/` — `keywords.json` (one keyword table for the grammar and `rivet highlight`, embedded with
  `include_str!`), `gen_grammar.py` → `rivet.tmLanguage.json` (generated; regenerate, never hand-edit),
  `check_keywords.py` and `tests/` (drift and grammar checks), `vscode/` (the extension; `package_vsix.py` writes
  `dist/rivet-<version>.vsix` without Node.js).
- **Platforms:** macOS and Linux (CI green on both; the process sandbox is active on macOS and gated on Linux).
  Windows is not supported ([INC-2026-0011](docs/incidents/active/inc-2026-0011-windows-port-failures.md)).
- **Releases:** 0.1.0 released (tag `v0.1.0`); 0.2.0 in progress
  ([PLAN-2026-0002](docs/plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md)).
- **Spec (hand-authored design):** `vhco-contract.json` — render with `vhco live`. Approved by the maintainer
  in [ADR-0001](docs/decisions/adr-0001-approve-rivet-runtime-design.md); design source of truth is
  [PROP-2026-0001](docs/proposals/implemented/prop-2026-0001-rivet-runtime.md) (0.1.0) and
  [PROP-2026-0002](docs/proposals/approved/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md) (0.2.0,
  approved in [ADR-0004](docs/decisions/adr-0004-approve-envelopes-globals-library-ffi-highlighting.md)); execution
  ledgers are [PLAN-2026-0001](docs/plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) and
  [PLAN-2026-0002](docs/plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md).
- **Model (generated):** `vhco.json` — `vhco spec`, never hand-edited. **Explorer:** `vhco.html`.

```text
 vhco-contract.json (approved spec) ──▶ src/ code + vhco: comments ──▶ vhco spec ──▶ vhco.json (generated model)
          ▲                                                                              │
          └──────────────── contract first, then proposal, then code ◀── vhco validate . ┘
```

```text
 workspace (Cargo.toml)
 ├── rivet-runtime  (.)      lib `rivet` ── facade ── rivet::internal::{domain, features, io, infra, orchestrator}
 │     features: serve grpc quic oauth (default) · cli ─▶ bin `rivet`
 ├── rivet-ffi      (ffi/)   librivet.{a,dylib|so} + include/rivet.h ──calls──▶ orchestrator/setup_ffi.rs
 └── editors/                keywords.json ─▶ gen_grammar.py ─▶ rivet.tmLanguage.json ─▶ vscode/ (.vsix)
```


# AGENTS.md — annotating a non-VHCO project (descriptive mode)

> Drop this file at the root of **any ordinary codebase** (no five-folder VHCO structure) that you want to
> **model and visualize** with the `vhco` tool. It tells coding agents how to add `vhco:` comments so the
> tool can capture the project — its types, the actions a user can take, what it touches, its APIs, and how
> each action works — purely from comments, in **descriptive mode**.

This is **not** about enforcing an architecture. It's about making an existing project **navigable and
self-documenting**: annotate the code, run `vhco doc`, and get an interactive explorer + a single JSON
model of what the project actually is.

---

## 1. Turn it on

Create a `.vhco.json` at the project root:

```json
{
  "mode": "descriptive",
  "documentation": { "readme": "README.md" },
  "project": { "summary": "One line about what this project is." }
}
```

`"mode": "descriptive"` tells the tool: **this is any codebase, not a five-folder VHCO project.** It scans
the whole tree (skipping `.git`, `node_modules`, `vendor`, build dirs, …) and builds the model from your
annotations alone — no closed-world import rules, no required folders. Comment symbol is detected per file
extension (`//`, `#`, `--`, …); add custom ones under `"comments"` if needed.

---

## 2. The workflow

```
1. IMPLEMENT  make the code change the user asked for (this is an ordinary codebase — build it normally)
2. ANNOTATE   add // vhco:<kind> comments directly above the code they describe (anywhere in the tree)
3. TEST       write/extend tests for what you changed; annotate each with
              // vhco:test <feature.action> -- <what it verifies>; ITERATE until they pass
4. MODEL      vhco doc . --format html    → an interactive explorer (vhco.html)
              vhco spec . --stdout        → the whole project as one JSON document
5. INSPECT    open the explorer; see what was captured and what's missing
6. DOCS       update the README (and any docs/ pages) so they describe the project as it is now;
              run vhco coverage . and add annotations to fill the gaps it reports
```

There is **no contract and no `validate` gate** here (those are for five-folder VHCO projects). Your loop
is *implement → annotate → test → regenerate → look → keep the docs current*. `vhco sync` against a
hand-authored `vhco-contract.json` is optional if you later want a design doc to diff against, but it isn't
required.

> **Keep the docs in sync.** Whatever README / `docs/` the project already has must keep describing the
> project *as it is now* — when you add a feature or endpoint, update them in the same change. The explorer's
> **Documentation** view and search index their contents, so stale docs are visible.

---

## 3. The five roles you declare (from comments, wherever the code lives)

Even in a flat project, you describe the program with the same five concepts — placed wherever that code
already is, in any file:

| Role | Directive | Put it above… |
|---|---|---|
| **Domain type** — the shared vocabulary | `vhco:domain Name { field: type; … }` | a struct / class / type |
| **Port** — a capability an action needs | `vhco:port Name { method(args) -> result; … }` | an interface |
| **Use case** — one action a user can take | `vhco:usecase feature.action(in: T) -> Out needs Port` | the function/method that performs it |
| **Surface** — how the world reaches it | `vhco:surface http kind http calls feature/action, …` | the router / entrypoint |
| **Adapter** — the concrete implementation | `vhco:infra name satisfies Port` | the impl struct/class |

They don't need to live in special folders — the tool reads the annotations, not the layout. Two more
optional structural directives: **`vhco:feature <name>`** (declare a feature explicitly when it isn't
obvious from the surrounding code) and **`vhco:wiring <name> -- <what it wires>`** (mark a composition/setup
point for the map).

---

## 4. Worked example — a flat Go CRUD API

A conventional Books REST API (`models.go`, `store.go`, `service.go`, `handlers.go`, `main.go`), annotated:

```go
// models.go — the vocabulary
// vhco:domain Book { id: string; title: string; author: string; year: int; available: bool }
// vhco:domain NewBook { title: string; author: string; year: int }

// store.go — the port + the adapter that satisfies it, with its real edges
// vhco:port BookStore { list() -> []Book; get(id: string) -> Book, bool; save(book: Book) -> error; delete(id: string) -> error }
type BookStore interface { ... }

// vhco:infra file_store satisfies BookStore
// vhco:file readwrite ~/books.json -- the catalog is read and rewritten here as JSON
// vhco:env  BOOKS_FILE             -- overrides the catalog path
type fileStore struct { ... }

// service.go — the use cases (the actions), each pure, taking the store
// vhco:usecase books.create(input: NewBook) -> Book needs BookStore
// vhco:label Add a book
// vhco:about Validates a new book and stores it, returning the created book with its id.
func CreateBook(in NewBook, store BookStore) (Book, error) {
	// vhco:todo validate -- reject a blank title
	// vhco:step validate strings.TrimSpace -- ensure the title is present
	if strings.TrimSpace(in.Title) == "" {
		// vhco:error blank-title -- the title is empty => ErrInvalid returns
		return Book{}, ErrInvalid
	}
	// vhco:todo persist -- save the new book
	// vhco:step save store.Save -- write it to the catalog (groups under the "store" lane)
	...
}

// main.go — the surface + its routes (the HTTP API)
// vhco:surface http kind http calls books/create, books/list, books/get, books/update, books/delete
// vhco:trigger http books/create = POST /books
// vhco:api http books/create POST /books -- add a book to the catalog
// vhco:request  { "title": "string — required", "author": "string", "year": "int" }
// vhco:response { "id": "string", "title": "string", "author": "string", "year": "int", "available": "bool" }
```

Run `vhco doc . --format html` and the explorer shows: the domain types, the 5 use-case journeys, the
`http` surface, the `file_store` adapter, the `file`/`env` edges, the 5 API endpoints (with their JSON
request/response as schema trees), the todos, the failure modes, and the located source map.

> A complete, runnable copy of this example lives in the tool repo at
> `.ignore/testnon_VHCO/` — read it to see every directive in context.

---

## 5. The marker directives (all descriptive — none change `validate`/`sync`)

Use these to make the model rich. Each goes **directly above** the code it describes.

- **Edges — what the program touches.** On the adapter that performs them:
  `vhco:env VAR`, `vhco:arg --flag`, `vhco:file <read|write|readwrite|append|delete> <path>`,
  `vhco:net <GET|POST|…|grpc|ws> <url>`, `vhco:db <read|write|readwrite|delete|migrate> <table>`.
  They roll up into the **What it touches** view, grouped and linked to the adapter.
- **`step` — the execution trace (must spell out the logic).** `// vhco:step <id> <ref> [@lane] -- <what
  happens>`, one per meaningful operation, **in execution order**, above the code that does it. The **lane**
  (from `@<lane>` or the ref receiver, e.g. `store.Save` → `store`) groups steps by owner. Display-only — but
  **required to be detailed**: a reader should follow the steps and understand *how the action runs* (what's
  checked, what's built, what flows out, how it can fail) without reading the body. See
  [Todos and steps must spell out the logic](#todos-and-steps-must-spell-out-the-logic).
- **`todo` — work items (must spell out the logic).** `// vhco:todo <id> -- <what it does>` above the
  implementing line; renders a ✓ checklist on the use case. Write each as a concrete unit of the algorithm —
  the decision, the data, the failure path — not a vague label. (Without a `vhco-contract.json` there's
  nothing to enforce — they're shown, not checked — but they still document the logic.)
- **`error` — failure modes.** `// vhco:error <id> -- <when it occurs> => <code> [returns|wraps|handled]`,
  above the line that produces/handles the failure. Renders a "How it can fail" section + an **Errors**
  sidebar view, each linked to its use case.
- **`api`/`request`/`response` — endpoint docs.** Next to the route's `vhco:trigger`. `request`/`response`
  are **destructured JSON objects** (keys → type strings, nesting allowed) rendered as schema trees in the
  **API & CLI** view. Nothing to resolve; trivially parsed.
- **`capture_start`/`capture_end` — spotlight code.** Bracket a region (one to many functions); it's
  grabbed verbatim into the **Captured code** view.
- **`label` / `about` / `example` / `meta` / `ref`** — human framing on a use case (title, description, a
  sample `inputs => result`, labelled facts, doc links).
- **`test`** — `// vhco:test <feature.use_case> -- <what it verifies>` above a test function; rolls up into
  the **Tests** view (what's covered, what isn't).
- **`custom`** — `// vhco:custom <category> <value> -- <note>` for your own marker categories.

Every annotation is recorded with its `file:line` and the next code line, so the explorer's **Source map**
doubles as a navigator for the whole codebase.

---

## Todos and steps must spell out the logic

`vhco:todo` and `vhco:step` exist for **traceability**: a reader should open a use case and understand
*exactly how it works* from its todos and steps — the decisions, the order, the data, the failure paths —
**without reading the function body**. A one-word label or a restatement of the name defeats the purpose.
Write them as the **step-by-step logic of the implementation.**

- **Todos = the plan, in order** — each names a concrete unit of work and says *what it does and why*,
  including guards and the failure path, not just the happy path.
- **Steps = the execution trace** — one per meaningful operation, in execution order, pointing at the real
  call/ref and describing what happens there.
- **Concrete, not generic** — reference the real values/types/conditions, not "validate input" / "save it".

**Bad — tells you nothing the name didn't:**

```go
// vhco:todo validate -- validate input
// vhco:todo persist  -- save the book
// vhco:step save store.Save -- save
```

**Good — a reader can reconstruct the logic:**

```go
// vhco:todo validate -- trim the title; if empty after trimming, return ErrInvalid and write nothing
// vhco:todo persist  -- mint an id, build Book{...,available:true}, hand it to store.Save, propagate its error unchanged
// vhco:step trim  strings.TrimSpace -- normalise so "  " counts as empty
// vhco:step guard return            -- empty title → return ErrInvalid before any write
// vhco:step build newID             -- mint a unique id and assemble the Book
// vhco:step save  store.Save        -- persist through the store; on error return it unwrapped
```

(There's no gate enforcing this in descriptive mode, but vague todos/steps make the explorer useless — treat
the detail as required.)

---

## 6. How this differs from a five-folder VHCO project

| | **Descriptive (this file)** | **VHCO project** |
|---|---|---|
| Folder layout | **any** — annotate where the code is | strict five folders |
| `.vhco.json` | `"mode": "descriptive"` | default (closed-world) |
| `vhco validate .` | not the point (architecture rules assume five folders) | the gate — must stay green |
| `vhco-contract.json` + `sync` | optional | the design loop |
| Goal | **capture & visualize** an existing codebase | enforce a closed-world architecture |
| What you run | `vhco doc` / `spec` / `query` / `live` / `coverage` | `validate` + `sync` + `spec` |

Same annotation grammar; different intent. Here you're documenting reality, not enforcing structure.

---

## 7. Commands (run from the project root)

```sh
vhco doc . --format md|html|mermaid   # the explorer (html) — journeys, API & CLI, errors, what-it-touches; or md / mermaid
vhco spec . --stdout         # the whole project as one JSON document
vhco live .                  # serve the model in the browser; reloads as you edit the code
vhco visualize .             # print every action and the ports it needs
vhco flow .                  # per action: how it's received and handled, layer by layer
vhco coverage .              # how complete the annotations are (described actions, edges, tested?)
vhco query . "effects where kind=net"     # query the model: effects/actions/domain/surfaces/flows/markers/captures
vhco diff <old.json> <new.json>           # behavioral delta between two snapshots
vhco scan . [--sarif|--json]              # run .vhco.json extractors/rules + edge detection + capability checks
vhco sandbox . --profile summary|netpolicy|seccomp   # compile the capability manifest into a runtime profile
vhco version                 # the vhco tool version
```

### Codebase intelligence (optional `.vhco.json` layers)

The same `.vhco.json` can do more than enable descriptive mode — it can **find patterns and enforce
policy/capabilities** over the code:

```jsonc
{
  "mode": "descriptive",
  "comments": { ".myext": "//" },                      // custom comment symbols
  "extract":  { "todo-markers": { "match": ["TODO", "FIXME"] } },  // lexical extractors → located signals
  "rules": [ { "name": "no-eval", "assert": "deny", "match": ["eval("], "why": "no eval" } ],  // deny/allow-only/require/max
  "conventions": { "document-edges": { "require": ["net", "db"], "severity": "warn" } },        // every net/db edge must be annotated
  "sandbox": { "deny-undeclared": true, "allow": { "net": ["api.example.com"], "file": ["~/.app/**"] } } // allowed capabilities
}
```

`vhco scan .` runs all of the above (the capability **sandbox** flags any edge not in `allow`);
`vhco sandbox . --profile …` compiles the manifest into a runtime profile; and **`.vhco.overlay.json`** adds
**sidecar edges** for code you can't annotate (vendored/generated). All optional — add only what helps.

## FAQ & troubleshooting

**There's no contract and no `validate` — how do I know the model is right?** Three checks stand in for the
gate: (1) your **tests pass** — that's the real correctness gate in an ordinary codebase; (2)
**`vhco coverage .`** tells you, numerically, what the model is still missing (undescribed actions,
un-annotated edges); (3) you **read the explorer** (or `vhco query .`) and confirm the captured journeys,
edges, and APIs match what the code actually does. "Right" here means *complete and accurate*, not a
pass/fail.

**Where do the annotations go in a flat repo?** Directly above the code they describe, in whatever file
that code already lives in — there is no required folder. Domain types go above their structs, use cases
above their functions, edges above the line that performs them (on the adapter), the surface above the
router. The Books example puts `domain` in `models.go`, `port`/`infra`/edges in `store.go`,
`usecase`/`step`/`todo`/`error` in `service.go`, and `surface`/`trigger`/`api` in `main.go`.

**Why isn't my `vhco:todo` being enforced?** It can't be — there's no `vhco-contract.json` to check it
against in descriptive mode. Todos are **shown, not checked**: they render as a ✓ checklist on the use
case for documentation. A missing todo never fails anything here.

**My edges aren't showing in "What it touches".** Three usual causes: (1) the edge sits under the wrong
item — put `vhco:file`/`net`/`db`/`env`/`arg` above the `vhco:infra` adapter (or use case) that performs
it; (2) the directive isn't on its own comment line (a `vhco:` token trailing code is ignored); (3) the
file is pruned by the whole-tree scan (`vendor`, `node_modules`, build dirs, dot-dirs) or your
`ignore`/`include` globs — for code you can't annotate in place, use a `.vhco.overlay.json` sidecar.

**My `request`/`response` renders blank.** They must be **destructured JSON objects** (raw JSON strings
where the values are type strings), e.g. `{ "title": "string", "year": "int" }` — **not** a type reference
like `NewBook` or `[]Message`. The explorer `JSON.parse`s the literal; a bare type name has nothing to
parse.

**A step landed in the wrong lane (or no lane).** The lane comes from `@lane` if present, else from the
**receiver** of the ref: `store.Save` → lane `store`. A bare-function ref (`strings.TrimSpace`) has no
receiver and so no lane — add an explicit `@lane` to group it.

**`vhco coverage .` reports gaps I thought I'd filled.** Coverage reflects **annotations**, run *after* you
add them. An *inferred* edge (auto-detected) still counts as a gap until you add the matching `vhco:`
annotation — that's what `edges documented` measures. And anything in an ignored path simply isn't in the
model. Re-run after annotating, and check the **Source map** for files that produced no annotations at all.

**Will `vhco doc` clobber my files?** It writes the generated artifact (`vhco.html` / `vhco.md` /
`vhco.json`) — don't hand-edit those. `vhco spec . --stdout` prints to stdout and writes nothing.

---

## 8. Tips for good coverage

- Start with **domain types** and **use cases** (the actions) — they anchor everything.
- Put **edges** on the adapter that performs them, so "What it touches" is accurate.
- Give every endpoint a **`vhco:api` + `request`/`response`** so the API & CLI view reads like real docs.
- Use **`step`** to show *how* an action runs, and **`error`** to show *how it can fail* — together they
  document the action end to end.
- Re-run `vhco coverage .` and fill the gaps it reports.

---

## 9. Going deeper — the agent wiki

This file is the quick playbook. For fuller, navigable references (the full annotation grammar, every marker
directive, coverage, the command set, and the codebase-intelligence layer), see the **descriptive-mode agent
wiki index**: [`wiki/README.md`](wiki/README.md). It links the canonical tool docs in
`docs/wwd/`.




# DOCUMENT WRITING
User asks:
  write proposal   -> write to docs/proposals/<lifecycle>/   (draft/ unless stated)
  write report     -> write to docs/reports/
  write system doc -> write to docs/system/
  write plan doc   -> write to docs/plans/
  write research   -> write to docs/research/
  write checklist  -> write to docs/implementations/active/
  write impl report-> write to docs/implementations/completed/
  write test report-> write to docs/reports/tests/<year>/
  write demo       -> write to docs/demos/
  write release    -> write to docs/releases/
  write incident   -> write to docs/incidents/open/

> **Proposals are requirements-driven.** Every proposal MUST carry a numbered
> **Requirements** table (each requirement citing a source outside the proposal), numbered
> **Proposed changes**, and a **Requirements alignment** matrix readable in both directions —
> no change without a requirement, no requirement without a change or a stated non-goal.
> The mandatory coverage, template and validation rules are in `DOCUMENTATION.md`
> [§4.2](DOCUMENTATION.md#42-proposals--type-proposal-prefix-prop),
> [§12.1](DOCUMENTATION.md#121-proposal--proposal-templatemd), and
> [§21](DOCUMENTATION.md#21-project-standards-and-proposal-validation).


---

# Documentation Rules

All project documentation must be stored under `docs/`.

```text
docs/
├── README.md
├── index/
├── wiki/              how to write each document type
├── standards/         enforceable system design rules
├── profiles/          documentation profiles
├── architecture/
├── proposals/
├── discussions/
├── decisions/
├── plans/
├── implementations/   checklists + implementation reports
├── techniques/
├── system/
├── api/
├── data/
├── security/
├── operations/
├── runbooks/
├── troubleshooting/
├── manuals/
├── onboarding/
├── demos/
├── testing/
├── reports/
├── incidents/
├── releases/
├── migrations/
├── research/
├── references/
├── templates/
└── archive/
```

## Directory Indexes

Every documentation directory must contain an `index.md`. A `README.md` is **optional** and may be
added alongside it for lifecycle subfolders (e.g. `proposals/draft/`); it must not duplicate the
`index.md`. This is the authoritative rule — see
[`DOCUMENTATION.md` §22](DOCUMENTATION.md#22-folder-indexes-and-the-root-index).

Example:

```text
docs/proposals/index.md
docs/system/index.md
docs/reports/index.md
```

An `index.md` is a **navigation and status page**, not a file listing. Each must explain:

* the purpose of the directory;
* what documents belong there, and what does not;
* naming rules;
* active documents;
* recently added or updated documents;
* deprecated, superseded or archived documents;
* important relationships;
* unresolved work or open questions;
* recommended reading order;
* related directories and documents.

Update the directory index whenever a document is added, significantly changed, moved, deprecated, superseded, or archived.

The root `docs/README.md` is the **current-state page**, not a table of contents. Beyond linking every
major directory it must report: current project status, latest releases, active implementations, open
incidents, pending proposals, recent research, current architecture, important decisions, known
limitations, current risks, and documentation requiring updates.

## Required Document Metadata

Every Markdown document must begin with YAML metadata:

```yaml
---
document_id: PROP-2026-0012
title: Plugin Runtime Redesign
document_type: proposal
status: draft

created_date: 2026-07-15
last_updated: 2026-07-15

owner: Platform Team
authors:
  - Author Name

systems:
  - Main Platform

components:
  - plugin-runtime

affected_versions:
  from: "2.0.0"
  to: null

reason: Explain why this document was created.

related_documents: []
supersedes: null
superseded_by: null

tags:
  - plugins
  - architecture
---
```

At minimum, every document must identify:

* document ID;
* title;
* document type;
* status;
* creation date;
* last update date;
* owner;
* affected system or component;
* affected versions;
* reason for existence;
* related documents.

Use `not-applicable` when software versions do not apply.

## Document Status

Use one of the following statuses:

```text
draft
under-review
proposed
approved
active
implemented
completed
rejected
cancelled
deprecated
superseded
archived
```

Deprecated or superseded documents must link to their replacement.

Do not silently delete historical design, proposal, decision, incident, or migration documents. Archive them when they are no longer active.

## Naming Convention

Use lowercase kebab-case:

```text
<document-id>-<short-description>.md
```

Examples:

```text
adr-0012-use-delayed-rag-indexing.md
prop-2026-0015-plugin-runtime-redesign.md
rpt-2026-0042-retrieval-evaluation.md
inc-2026-0007-call-state-desynchronization.md
```

Do not use filenames such as:

```text
notes.md
final.md
final-v2.md
new-document.md
latest-report.md
```

Git provides revision history. Do not encode document revisions into filenames.

## Document Categories

#### ALWAYS USE LOTS OF ASCII VISUALS IN DOCUMENTS

* `wiki/`: how to write each document type — purpose, timing, sections, worked examples, common mistakes.
* `standards/`: versioned, enforceable system design rules that proposals must cite.
* `profiles/`: documentation profiles declaring which document types a project requires.
* `architecture/`: system structure, components, deployment and data flows.
* `proposals/`: suggested changes not yet approved.
* `discussions/`: options, questions and unresolved design conversations.
* `decisions/`: approved technical and architectural decisions.
* `plans/`: implementation, migration, rollout and testing plans.
* `implementations/`: living implementation checklists and implementation reports.
* `techniques/`: reusable engineering strategies and patterns.
* `system/`: documentation of the currently implemented system.
* `api/`: APIs, events, webhooks and integration contracts.
* `data/`: schemas, data lifecycle, retention and indexing.
* `security/`: security architecture, reviews and requirements.
* `operations/`: deployment, monitoring, backup and production operation.
* `runbooks/`: executable operational procedures.
* `troubleshooting/`: known problems, diagnosis and fixes.
* `manuals/`: user, administrator and installation guides.
* `onboarding/`: contributor and developer setup.
* `demos/`: demonstration scenarios, sample workflows and test data.
* `testing/`: test strategy, cases and acceptance criteria.
* `reports/`: completed investigations, assessments and results.
* `incidents/`: incidents, timelines, root causes and corrective actions.
* `releases/`: release notes, compatibility and upgrade instructions.
* `migrations/`: migration and rollback procedures.
* `research/`: evaluations, comparisons and feasibility studies.
* `references/`: glossaries, standards and stable reference information.
* `templates/`: approved document templates.
* `archive/`: documents that are no longer active.

## Document Relationships

Documentation should preserve the lifecycle of a change:

```text
research
  → discussion
  → proposal
  → decision
  → plan
  → implementation
  → testing
  → release
  → report or incident
  → migration, deprecation or archive
```

Documents must link to earlier and later documents in this lifecycle whenever applicable.

## Maintenance Rules

Update documentation when:

* behavior changes;
* an API or schema changes;
* a component is added, removed or renamed;
* supported versions change;
* a proposal is approved or rejected;
* a plan is completed;
* an incident reveals incorrect documentation;
* a document is replaced or becomes outdated.

Documentation describing current behavior must not remain marked `active` when it is known to be outdated.

## Agent Responsibilities

When an agent changes code, architecture, configuration, APIs, operational behavior or user-visible workflows, it must:

1. Find the relevant documentation.
2. Update existing documents instead of creating duplicates.
3. Create a new document when the change introduces a new proposal, decision, plan, incident or system concept.
4. Update the affected directory’s `index.md`.
5. Update `docs/README.md` when a major document or directory is introduced.
6. Record affected versions and components.
7. Link related proposals, decisions, plans, reports and releases.
8. Mark replaced documents as `superseded` or `deprecated`.
9. Keep documentation changes in the same pull request as the related implementation whenever practical.


full documentation at [./DOCUMENTATION.md]. Read before any edits to docs/


CODE and techniques REFERENCES are at: ignore/references
