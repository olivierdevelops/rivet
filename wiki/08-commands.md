# 8 · Commands — by phase

[← Docs & README in sync](07-docs-and-readme.md) · [wiki index](README.md) · next: [Codebase intelligence →](09-intelligence.md)

Run all of these from the project root. The first positional argument is always a **path** — usually `.`
(the current project). `vhco` with no arguments prints the usage block; `vhco <unknown>` prints an error
and the usage block and exits non-zero. Nothing here generates or scaffolds code — vhco only **models,
validates, and visualizes**. There is no `apply`, `init`, or `create-*`.

This page is the complete reference. The commands fall into four phases:

- **Design** — author and audit `vhco-contract.json` before writing code: `live`.
- **Build-check** — keep the code legal and on-target while you implement: `validate`, `sync`, `spec`, `check`, `assure`.
- **Inspect / render** — read or publish the model: `doc`, `flow`, `visualize`, `coverage`, `query`, `diff`, `version`.
- **Code policy** — the optional `.vhco.json` intelligence layer: `scan`, `sandbox`.

---

## Which command answers which question

| You want to know… | Command |
|---|---|
| Does my code satisfy the VHCO architecture rules? | `vhco validate .` |
| Does my code match the hand-authored design contract? | `vhco sync .` |
| Do the app's declared guarantees hold (a UI tap reaches its action; every route has request+response)? | `vhco check .` |
| Is this change fully assured (validate + sync + guarantees + tests + scope)? | `vhco assure .` |
| Regenerate the machine model of the code | `vhco spec .` |
| Audit the design in the browser while I edit it | `vhco live .` |
| Publish a shareable explorer / Markdown / Mermaid graph | `vhco doc . --format html\|md\|mermaid` |
| How is each user action received and handled, layer by layer? | `vhco flow .` |
| What ports does each user action need? | `vhco visualize .` |
| How complete / trustworthy is the model? | `vhco coverage .` |
| Ask a structured question (e.g. every db effect) | `vhco query . "effects where kind=db"` |
| What did this change do to behavior? | `vhco diff <old> <new>` |
| Lint code / enforce keyword + edge policy | `vhco scan .` |
| Compile a capability manifest to a runtime profile | `vhco sandbox . --profile <kind>` |
| What version of the tool am I running? | `vhco version` |

---

## Design phase

`vhco-contract.json` is **hand-authored**. You write it first, render it, get it approved, *then* code.

### `vhco init <path>`

Scaffolds the files a VHCO project needs so the design is always present: a starter **`vhco-contract.json`**
(seeded from the code if it already has `vhco:` annotations, otherwise a blank design template) and a minimal
`.vhco.json`.

- **Writes:** only the files that are **missing** — it **never overwrites** an existing contract or config.
- **Use it to:** start a project (`vhco init .`, then edit the contract), or bootstrap a contract when adopting
  vhco on an existing annotated codebase.
- **Also automatic:** for an *uninitialised* project (no contract **and** no `vhco.json`), the same starter is
  created the first time you run `vhco live` or `vhco sync`. A freshly-seeded contract may report
  `no_todos_declared` until you declare a todo per use case — the design-first todo rule.

### `vhco live <path|spec.json> [--spec <file>] [--host <host>] [--port 7777]`

Serves a JSON design as a **live explorer** in the browser and overlays a fresh code comparison on every
reload — so as you edit the contract *and* the code, the page re-renders to show both.

- **Reads:** the spec JSON. If you pass a directory, the spec is **always** `vhco-contract.json` — the
  hand-authored design — never the generated `vhco.json`. If that contract doesn't exist yet (a descriptive
  project, or before you've authored one), it renders the model **derived live from the code** instead, so
  the page always shows something. Pass a `.json` file directly (or `--spec`) to render a specific file; its
  directory becomes the project root. It also reads the project's annotated code to compute the live comparison.
- **Prints:** the explorer URL, the JSON contract path, the project path, then watches both for changes
  until you press Ctrl+C. It is a long-running server, not a one-shot.
- **Flags:** `--spec <file>` forces a specific spec; `--host` (default `127.0.0.1`) and `--port` (default
  `7777`, must be 0–65535).
- **Use it to:** audit the contract you are designing, before any code exists. This is step 2 of the loop.
- **Common mistakes:** Do not confuse with `doc --format html` — `live` is an interactive server that
  reloads; `doc` writes a static file once. Do not run `vhco spec . > vhco-contract.json` thinking it
  "refreshes" the design — that **destroys** your hand-authored contract (see the gotcha below).

> ⛔ **NEVER** `vhco spec . > vhco-contract.json`. `spec` writes the *generated* model from code;
> the contract is *authored by you* and is the target the code is built toward. Overwriting it with the
> code's current state erases the design and makes `sync` trivially "pass" against whatever you happen to
> have built.

---

## Build-check phase (run continuously)

### `vhco validate <path>`

Checks the code against the VHCO **architecture guarantees** — independent of any spec file.

- **Reads:** the project's `vhco:` annotations and folder layout. Compares against **no file**.
- **Prints:** `✓ no violations — the Exhaustive Use Case Invariant holds`, or a numbered list of
  violations, each with a `file:line` detail. **Exit code is non-zero when there are violations**, so CI
  can gate on it.
- **What it enforces** (the issue codes): extra/missing top-level folder, feature with no use cases,
  non-snake_case feature or use-case names, forbidden cross-module import, an action mapping to more than
  one use case, a **surface with no `orchestrator/setup_<surface>` file** (registration is per-surface),
  and a trigger that names a surface→action the surface doesn't route.
- **Use it to:** keep the architecture legal. Run it after **every** edit. It must stay green.
- **Common mistakes:** When it fails, fix the code or annotations — never work around it. `validate` does
  **not** read `vhco-contract.json` or `vhco.json`; a green `validate` says nothing about whether you've
  built the design (that's `sync`'s job).

### `vhco sync <path> [--spec <file>] [--update-spec]`

Reconciles the code against a JSON spec and reports **drift** — the gap between what the spec says and what
the code currently is.

- **Reads:** the code's annotations and the spec file. The spec defaults to `<path>/vhco.json`; pass
  `--spec <file>` to compare against another (commonly `vhco-contract.json`).
- **Prints (check mode, the default):** `✓ code matches <spec>` with exit 0, or `✗ code and <spec> differ
  in N place(s)` with each mismatch (`kind`, `where`, `detail`) and **exit 1**. When the spec is
  `vhco-contract.json` it adds guidance: the code is *behind the design* — implement the missing pieces.
- **Prints (`--update-spec`):** rewrites **`vhco.json`** from the code and reports what changed, exit 0.
  It **never** writes `vhco-contract.json`.
- **Use it to:** watch the design→build gap shrink to 0 as you implement.
- **Common mistakes:** If `vhco sync .` reports drift against the contract, **implement** the missing
  pieces — do **not** "fix" it by running `--update-spec` (that only rewrites the generated model, not the
  contract, and hides the real gap). `--update-spec` is for refreshing `vhco.json` after legitimate code
  changes, not for making `sync` go quiet.

### `vhco spec <path> [--stdout]`

Derives **`vhco.json`** — the whole program as one document: `definition` (features · surfaces · infra) +
`flows` + domain + the located `map` + edges/effects.

- **Reads:** the code's annotations and folder tree (the tree is the source of truth).
- **Writes:** `<path>/vhco.json` and prints `✓ wrote <path>/vhco.json`. With `--stdout` it prints the
  JSON document to stdout instead of writing the file.
- **Use it to:** regenerate the machine model after code changes; feed `vhco.json` to other tooling.
- **Common mistakes:** `spec` only ever produces the **generated** model — it is not a design tool. ⛔
  **Never** `vhco spec . > vhco-contract.json` (or `--stdout >` it). The contract is **hand-authored**; piping
  the code's current model over it **destroys your design** and makes `sync` trivially "pass" against whatever
  you happen to have built. (Bootstrapping a brand-new project has the same hazard — if you have no contract
  yet, write one by hand from the design you intend, don't dump code into it.) The contract changes only by
  hand, in step 1 of the loop.

### `vhco check <path>`

Validates the project's declared **app-level guarantees** against the model — the promises about what the
app must *expose* (not the architecture rules, which are `validate`'s job, nor design drift, which is
`sync`'s).

- **Reads:** the guarantees declared in **`vhco-contract.json`** (`guarantees[]` — product/app promises) and
  **`.vhco.json`** (`guarantees.rules[]` — reusable project policy), plus the code's `vhco:` annotations
  (screens, taps, surfaces, routes, request/response). Two guarantee kinds today:
  - **`ui.action_present`** — a UI interaction must reach an action through a `vhco:tap`. An optional `where`
    narrows it by `screen` / `gesture` / `placement`; with **no** `where`/`screen` it means "*some* tap/click
    must trigger this action, anywhere."
  - **`api.every_route_has_request_response`** — every route (optionally scoped to a `surface_kind`) carries
    **both** a documented `vhco:request` and a `vhco:response`.
- **Prints:** each guarantee as pass/fail with a **repairable** finding — what was expected, what was found,
  and the fix (e.g. add a `vhco:tap … @screen …` above the widget). **Exit code is non-zero when an
  error-severity guarantee doesn't hold**, so CI can gate on it.
- **Use it to:** prove, before code review, that the app exposes the promised actions in the promised places
  with the promised request/response shapes.
- **Common mistakes:** guarantees are separate from `validate` (architecture) and `sync` (contract drift) —
  a green `validate` says nothing about whether an `ui.action_present` guarantee holds. Put **product**
  promises in `vhco-contract.json` `guarantees[]`; put reusable **policy** in `.vhco.json`
  `guarantees.rules[]`. (Guarantees also work in descriptive mode.) Full schema:
  `docs/proposals/validated-app-guarantees.md` in the tool's own repo.

### `vhco assure <path>`

ONE read-only **gate over a change** — the single command that says whether a change is done. It composes
**five** gates into a pass/fail report and **exits non-zero when the change is not assured**.

- **Runs (in order):**
  1. **validate** — the VHCO architecture rules (`vhco validate`). **Skipped for descriptive projects.**
  2. **sync** — drift of the code vs `vhco-contract.json` (`vhco sync`).
  3. **guarantees** — the declared app-level guarantees (`vhco check`).
  4. **tests** — the commands in `.vhco.json` `assurance.tests.commands`, **actually run** (not just
     inspected).
  5. **scope** — the changed files vs `.vhco.json` `assurance.changed_files.allow` / `deny`, from a **real
     `git diff`**.
- **Reads:** `vhco-contract.json`, `.vhco.json` (`guarantees` + `assurance` blocks), the code's annotations,
  and the working tree's `git diff`. Read-only — it changes nothing.
- **Prints:** a per-gate pass/fail report; **exit code is non-zero unless every gate passes.**
- **Use it to:** get one authoritative "is this change actually complete?" answer in CI or before handing
  work back. Gates 4 (tests) and 5 (scope) are the anti-false-completion **teeth** — an agent can't satisfy
  them just by adding annotations, because tests must really run and the diff must stay inside the allowed
  paths.
- **Common mistakes:** `assure` doesn't replace running `validate`/`sync`/`check` while you work — it's the
  final gate that runs them together. If tests or scope fail, fix the code/diff; don't loosen
  `assurance.*` to make it pass. Full design: `docs/proposals/change-assurance-workflow.md` in the tool's
  own repo.

---

## Inspect & render phase

### `vhco doc <path> [--format md|html|mermaid] [--out <file>]`

Renders the model to a file.

- **Reads:** the code's annotations.
- **Writes:** with `--format html`, a single self-contained **interactive explorer** (`vhco.html`) — the
  audience-first SPA with per-use-case journeys and the "What it touches" edge inventory. With `--format
  md` (the **default**), a Markdown document with diagrams. With `--format mermaid`, a Mermaid graph.
  `--out <file>` (`-o`) overrides the output path. Prints `✓ wrote <file>`.
- **Flag shorthands:** `--html` ≡ `--format html`; `--md`/`--markdown` ≡ `--format md`; `-f` ≡ `--format`.
- **Use it to:** publish a shareable artifact — e.g. regenerate the explorer to check the Documentation
  view, or drop a Mermaid graph into a wiki.
- **Common mistakes:** `doc --format html` writes a **static** file (open it directly in a browser); it is
  not a live server. For the reload-on-save experience use `live`.

### `vhco flow <path>`

Prints, **per action**, how a user interaction is received and handled — the vertical path down through the
layers and back, grouped by lane.

- **Reads / prints:** derives the doc and renders its handling traces to stdout. No file is written.
- **Use it to:** understand or review a single action's end-to-end handling in the terminal.

### `vhco visualize <path>`

Prints **every user action and the ports it needs** — each surface and the protocols/methods it needs, and
each action with its port parameters.

- **Reads / prints:** the flow model, to stdout. No file is written.
- **Use it to:** see the external-world contract (ports + function signatures) at a glance.

### `vhco coverage <path>`

Reports how complete and trustworthy the model is.

- **Prints:** counts and percentages — actions, described actions, actions with examples, domain types,
  edges (annotated vs inferred), and the % of edges documented. No file is written.
- **Use it to:** find under-annotated parts of the model so you can raise coverage. Unannotated code does
  not exist in the model, so low coverage = blind spots.

### `vhco query <path> "<collection> [where <k>=<v> [and …]]" [--json]`

Runs a structured query over the model and returns matching rows.

- **Collections** (singular or plural both work): `effects`, `actions` (a.k.a. `usecases`), `domain` (a.k.a.
  `types`), `surfaces`, `flows`, `markers` (a.k.a. `map`), `captures`.
- **Filtering:** `where <field>=<value>`, chainable with `and`. Real fields per collection —
  - `effects`: `kind`, `value`, `mode`, `owner`, `provenance`, `tags`, `file`, `line`
  - `actions`: `feature`, `name`, `action`, `label`, `out`, `needs`, `surfaces`
  - `domain`: `name`, `label`, `fields`
  - `surfaces`: `name`, `kind`, `calls`
  - `flows`: `action`, `surfaces`, `output`
  - `markers`: `kind`, `name`, `file`, `line`
  - `captures`: `id`, `name`, `note`, `file`, `start`, `end`, `lines`
- **Prints:** `● <collection> — N row(s)` then each row as `label=value` columns. `--json` (anywhere on the
  line) emits the rows as JSON for agents.
- **Examples:**
  ```sh
  vhco query . "effects where kind=db"
  vhco query . "effects where kind=net and provenance=inferred"
  vhco query . "actions where needs=PaymentGateway"      # blast radius: what depends on this port
  vhco query . "actions where feature=billing" --json
  vhco query . "surfaces"
  ```
- **Use it to:** answer a precise question without grepping (and let an agent consume located JSON rows).

### `vhco diff <old> <new>`

Reports the **behavioral delta** between two snapshots — added/removed edges (effects), actions, and
surfaces.

- **Reads:** two projects **or** two `vhco.json` snapshots (`<old>` and `<new>` are both paths/files).
- **Prints:** `✓ no behavioral change…` (exit 0) or `● N behavioral change(s)` listing each `+`/`-` edge
  (with `file:line` for additions), action, and surface — **exit 1** when anything changed, so CI can
  require explicit review of behavior changes.
- **Use it to:** see what a PR did to the program's real edges — e.g. a new network egress or a new
  table write.
- **Common mistakes:** both arguments are required. `diff` compares two *given* snapshots; it does not
  diff "against git" for you — you supply the before/after (e.g. an old `vhco.json` you saved vs the
  current project).

### `vhco version`

Prints `vhco <version>`. Also available as `--version` / `-v`. No path argument.

---

## Code policy / capabilities (optional — driven by `.vhco.json`)

These run the **codebase-intelligence layer** and do nothing unless `.vhco.json` (or `.vhco.overlay.json`)
declares the relevant blocks. They are separate from `validate` (architecture) and `sync` (design). See
[Codebase intelligence](09-intelligence.md) for the config.

### `vhco scan <path> [--sarif|--json]`

Runs the `extract` extractors, the `rules` assertions, edge detection (annotated + inferred), the
documentation convention, and the capability `sandbox` check — all in one pass.

- **Prints (default text):** a count of located signals by category, edges by kind (annotated vs
  inferred), and rule findings (`⚠` warn / `✗` error) with `file:line`, message, and `why`. **Exit code is
  non-zero when any error-severity finding is present**; warn-only findings still exit 0.
- **`--sarif`:** emits SARIF 2.1.0 so findings land in GitHub code-scanning / security dashboards.
- **`--json`:** emits the full scan result as JSON (agent consumption).
- **Use it to:** lint the code against keyword/edge policy and check it stays inside the capability
  manifest, in CI or locally.

### `vhco sandbox <path> [--profile summary|netpolicy|seccomp]`

Compiles the `sandbox` capability manifest from `.vhco.json` into a runtime enforcement profile.

- **`--profile summary`** (default): a human-readable summary of the allowed capabilities.
- **`--profile netpolicy`**: a network-policy / egress allow-list shape.
- **`--profile seccomp`**: a seccomp-style syscall profile.
- **Prints:** the generated profile, or `no sandbox manifest — add a "sandbox" block to .vhco.json` if
  none is declared.
- **Use it to:** turn the reviewed, version-controlled manifest into the input for real runtime isolation
  (the static check keeps source honest; the generated profile enforces it at runtime).

---

## Reference card

| Command | Use it to… |
|---|---|
| `vhco live .` | audit the **contract** in the browser while designing (step 2) |
| `vhco validate .` | check the **architecture** is legal (step 5; run constantly) |
| `vhco sync .` | check the **code matches the contract**; watch drift → 0 (step 5) |
| `vhco check .` | check the declared **app-level guarantees** hold (UI taps reach actions; routes have request+response) |
| `vhco assure .` | ONE gate: **validate + sync + guarantees + tests + changed-file scope** → pass/fail |
| `vhco spec .` | regenerate **`vhco.json`** from code (never the contract) |
| `vhco doc . --format html` | regenerate the **explorer**, e.g. to check the Documentation view (step 6) |
| `vhco flow .` | trace, per action, how an interaction is handled layer by layer |
| `vhco visualize .` | print every user action and the ports it needs |
| `vhco coverage .` | find under-annotated parts of the model |
| `vhco query .` | answer a structured question about the model |
| `vhco diff <old> <new>` | see what a change did to behavior (added/removed edges, actions, surfaces) |
| `vhco scan .` / `vhco sandbox .` | enforce code policy / compile a capability profile |
| `vhco version` | print the tool version |

If `vhco validate .` fails, fix the code/annotations — don't work around it. If `vhco sync .` reports drift,
implement the missing pieces until it's 0; don't "fix" it with `--update-spec`.

---

## FAQ

**`spec` vs `sync` vs `doc` — what's the difference?**
`spec` **writes** the generated model `vhco.json` from code. `sync` **compares** the code to a spec and
reports drift (it does not render anything). `doc` **renders** the model to a human artifact (HTML / MD /
Mermaid). One writes data, one diffs, one renders.

**`live` vs `doc --format html`?**
Both show the explorer. `live` is a **long-running server** that reloads on every save and overlays a
fresh code comparison — use it while designing/editing. `doc --format html` writes a **static** `vhco.html`
once — use it to publish or check the rendered output.

**How do I query the model?**
`vhco query . "<collection> [where k=v]"`. Collections are `effects`, `actions`, `domain`, `surfaces`,
`flows`, `markers`, `captures`. Add `--json` for machine-readable rows. See the field list above.

**`diff` between what and what?**
Between two snapshots **you provide** — two project paths or two `vhco.json` files (e.g. an old saved
`vhco.json` vs the current project). It is not an implicit git diff.

**Which file does `sync` compare against by default?**
`<path>/vhco.json`. Pass `--spec vhco-contract.json` to check the code against the hand-authored design
contract instead.

**Why does `vhco scan .` print nothing interesting?**
Because no `.vhco.json` extract/rules/sandbox blocks are configured (or nothing matched). The intelligence
layer is opt-in. See [Codebase intelligence](09-intelligence.md).

**Why does a command exit non-zero even though it "ran fine"?**
`validate` (violations), `sync` (drift in check mode), `scan` (error-severity findings), and `diff` (any
change) intentionally exit non-zero so CI can gate on them. That's a signal, not a crash.
