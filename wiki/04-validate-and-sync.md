# 4 · `validate` vs `sync` — the two gates

[← Architecture](03-architecture.md) · [wiki index](README.md) · next: [Todos, steps & tests →](05-traceability.md)

These check **different things** and are **independent**. Confusing them is the most common mistake.

| | **`vhco validate .`** | **`vhco sync .`** |
|---|---|---|
| Compares against | **nothing** — rules baked into the tool; reads only the code | a **JSON spec** (`vhco-contract.json` by default, else `vhco.json`) |
| Question | *Is this a structurally legal VHCO program?* | *Does the code match the design?* |
| Reads | the project tree + annotations | the contract JSON **and** the code |
| Exit code | non-zero if any rule is violated | non-zero if there is any drift |
| Green/0 means | the architecture is sound | the code == the approved design |

## They disagree on purpose

`validate` and `sync` are **fully independent**. One being green tells you nothing about the other. There are
four possible states, and all four are normal at some point in the loop.

### State A — `validate` ✓ / `sync` ✓ (done)

The structure is legal **and** the code matches the approved contract. This is the only acceptable *finished*
state (plus passing tests and synced docs). Example: every contract use case is implemented, every todo is
claimed, no forbidden imports, every surface has its setup file.

### State B — `validate` ✓ / `sync` ✗ (the normal mid-build state)

The structure is legal, but you haven't built everything the contract calls for yet. This is where you spend
most of a change. Example: the contract declares `notes.archive_note` but you haven't written
`features/notes/archive_note.go` yet — `validate` is happy (what *exists* is legal), `sync` reports
`missing_use_case use case notes.archive_note`. Keep implementing; the count walks to 0.

### State C — `validate` ✗ / `sync` ✓ (legal-looking but illegal)

The code matches the design, but it breaks an architecture rule. Example: `features/notes/archive_note.go`
imports `infra` directly instead of receiving a port — `sync` sees the right shape, but `validate` reports
`forbidden cross-module import`. Still illegal; fix the structure (pass a port parameter, do the I/O in an
adapter).

### State D — `validate` ✗ / `sync` ✗ (early / broken)

Both fail — typical right after a half-finished edit. Example: you added a use case in a folder that isn't
snake_case *and* haven't implemented half the contract. Fix `validate` first (it's cheaper and unblocks
parsing), then close the `sync` gap.

So during a change you'll typically pass through **D → C/B → A**: get it legal, then make it match the design.

---

## Every `validate` failure

`validate` reads only the code and applies baked-in architecture rules. Each violation is one issue **code**
(an enum), rendered to the human text shown below.

| Issue (internal) | Rendered text | Triggered by | How to fix |
|---|---|---|---|
| `IssueExtraTopDir` | extra top-level folder (only the five VHCO folders are allowed) | a sixth top-level folder beyond `domain` `features` `io` `infra` `orchestrator` (plus allowed `docs/`, `tests/`) | move it under one of the five buckets, or delete it |
| `IssueMissingTopDir` | missing required top-level folder | one of the five required folders is absent | add the missing folder (even if it just holds the relevant code) |
| `IssueFeatureNoUseCases` | feature has no use cases | a `features/<feature>/` folder with zero use-case files | add a use case, or remove the empty feature folder |
| `IssueBadFeatureName` | feature name is not snake_case | a feature folder named e.g. `ManageItems` | rename to `manage_items` |
| `IssueBadUseCaseName` | use case name is not snake_case | a use-case file/name like `AddItem` | rename to `add_item` |
| `IssueForbiddenImport` | forbidden cross-module import | a module imports across a forbidden boundary (e.g. `features`→`infra`, `io`→`features`, anything→`orchestrator`) | remove the import; pass a port, or move the wiring into `orchestrator` |
| `IssueDuplicateAction` | action maps to more than one use case | the same action is routed to two use cases — breaks the **Exhaustive Use Case Invariant** | make the mapping one-to-one |
| `IssueMissingSetup` | surface has no orchestrator/setup_&lt;surface&gt; file (registration is per-surface) | a surface exists but `orchestrator/setup_<surface>.<ext>` is absent | add the `setup_<surface>` file (registration is **per surface**, not per feature) |
| `IssueTriggerNotRouted` | trigger names a surface→action the surface doesn't route (add the action to that surface's calls) | a `vhco:trigger <surface> <action>` whose action is **not** in that surface's `calls` (a dangling trigger) | add the action to the surface's `calls`, or remove the stray trigger |

> **`validate` does NOT check reachability.** A use case that no surface routes is *not* a violation. It shows
> only as a soft "X of Y reachable" / "Every use case reachable" note in the explorer's **Guarantees** view.
> If it should be reachable, add it to a surface's `calls` — but `validate` will stay green either way.

---

## Every `sync` drift / todo code

`sync` reconciles the **expected** contract against the **current** model derived from code, and emits one
mismatch per divergence. `missing_*` = the contract has it but the code doesn't (code is behind). `extra_*` =
the code has it but the contract doesn't (contract behind, or unsanctioned growth). Each mismatch names the
exact location.

### Definition — features, use cases, surfaces, infra

| Code | Meaning | Fix |
|---|---|---|
| `missing_feature` | feature in the contract, not in the code | implement the feature |
| `extra_feature` | feature in the code, not in the contract | add it to the contract (step 1) or remove the code |
| `missing_use_case` | use case in the contract, not in the code | write the use-case file |
| `extra_use_case` | use case in the code, not in the contract | add it to the contract or remove it |
| `use_case_input_differs` | the use case's `in` params differ from the contract | match the signature on one side |
| `use_case_output_differs` | the use case's `out` differs from the contract | match the return type |
| `missing_port` | a `need` (port) the contract requires is absent from the code | add the port parameter |
| `extra_port` | the code uses a port the contract doesn't list | add it to the contract or remove it |
| `port_contract_differs` | the port's function set/signatures differ | align the typed functions on the port |
| `missing_surface` | surface in the contract, not in the code | add the surface + its `setup_<surface>` |
| `extra_surface` | surface in the code, not in the contract | add it to the contract or remove it |
| `surface_calls_differ` | the surface routes different actions than the contract says | align the surface's `calls` |
| `missing_infra` | infra adapter in the contract, not in the code | implement the adapter |
| `extra_infra` | infra adapter in the code, not in the contract | add it to the contract or remove it |
| `infra_contracts_differ` | the adapter satisfies a different set of ports than declared | align its `satisfies` list |

### Domain types

| Code | Meaning | Fix |
|---|---|---|
| `missing_domain_type` | domain type in the contract, not in the code | add the `vhco:domain` type |
| `extra_domain_type` | domain type in the code, not in the contract | add it to the contract or remove it |
| `domain_fields_differ` | a domain type's fields differ | align the field list (name + type) |

### Flows

| Code | Meaning | Fix |
|---|---|---|
| `missing_flow` | flow (action) in the contract, not in the code | wire/route the action |
| `extra_flow` | flow in the code, not in the contract | add it to the contract or remove the routing |
| `flow_surfaces_differ` | the surfaces exposing the action differ | align the flow's `surfaces` / the surface `calls` |
| `flow_triggers_differ` | the triggers (`surface = invocation`) differ | align the `vhco:trigger` lines |
| `flow_ports_differ` | the ports the action traverses (`through`) differ | align the ports the use case needs |
| `flow_input_differs` | the flow's input params differ | align the input |
| `flow_output_differs` | the flow's output differs | align the output |
| `flow_handling_differs` | the layer-by-layer handling trace differs | align the handling steps |

### Todos (only against a hand-authored contract)

| Code | Meaning | Fix |
|---|---|---|
| `no_todos_declared` | a use case in the contract declares **no** todos | add `"todos": [{ "id", "todo" }]` to that use case in the contract |
| `todo_not_implemented` | a contract todo has **no** `vhco:todo <id>` claim in the code | add `// vhco:todo <id> -- …` above the implementing line |
| `extra_todo` | the code claims a `vhco:todo <id>` not declared in the contract | declare it in the contract, or remove the claim |

> The todo rules apply **only when the spec is the hand-authored `vhco-contract.json`** (`Reconcile` adds the
> `no_todos_declared` check only for an authored contract). The *generated* `vhco.json` is exempt, so a
> project that doesn't use todos isn't forced to.

---

## What `sync` compares — and what it ignores

`sync` reconciles the **definition** (features · use cases · surfaces · infra · ports), the **domain**
entities (by field list), the **flows** (surfaces, triggers, ports traversed, input, output, handling), and
the **todo** declarations/claims.

It does **not** compare the *descriptive* markers — `vhco:test`, `vhco:capture`, `vhco:error`, `vhco:api`,
`vhco:step`, `vhco:label`, `vhco:about`. Those enrich the model and the explorer but **never cause drift**, so
you can add, edit, or remove them freely without `sync` ever complaining.

---

## The contract is hand-authored — never regenerate it

`vhco sync . --update-spec` regenerates **`vhco.json`** (the generated model) and **never** touches
`vhco-contract.json`. ⛔ Never run `vhco spec . > vhco-contract.json` — it overwrites your design with the
current code. If `sync` reports drift, the code is **behind** the design; implement the missing pieces until
it reaches 0. Don't "fix" drift by editing the model — the contract changes only by hand, in step 1.

**When is `--update-spec` safe?** Only when the **generated `vhco.json`** (not the contract) is intentionally
the artefact you're refreshing — e.g. you're keeping a committed `vhco.json` snapshot for `vhco diff`, or a
project genuinely uses `vhco.json` as its checked-in model with no separate contract. It is **never** safe to
reach for `--update-spec` as a way to silence drift against `vhco-contract.json`: that just hides that the
code doesn't match the agreed design. Rule of thumb: if a `vhco-contract.json` exists, `--update-spec` is not
your drift fix — implementing the missing code is.

---

## Run them constantly

`validate` is cheap; run it after every meaningful edit, never only at the end. Run `sync` alongside to watch
the design gap close (State B → State A). See [the loop](01-workflow.md) for where each fits.

---

## FAQ

**`sync` says `extra_todo` — why?** You wrote `// vhco:todo <id>` in the code with an `id` the contract
doesn't declare for that use case. Either you renamed/typo'd the id (it must match the contract's `id`
exactly), or you genuinely added work the design never sanctioned. Fix: declare it in the contract (step 1) or
correct/remove the claim.

**`validate` doesn't flag my unwired use case — is that wrong?** No. `validate` does not check reachability. An
unrouted use case is legal; it just appears as a soft note in the **Guarantees** view. If it should be
reachable, add it to a surface's `calls`; `validate` will stay green regardless. (`sync` *will* care if the
contract's flow says the action is exposed and the code doesn't route it — that's `flow_surfaces_differ` /
`missing_flow`.)

**Is reachability checked anywhere?** Only softly, in the explorer's **Guarantees** view ("Every use case
reachable"). Neither `validate` nor `sync` fails on an unreachable-but-otherwise-correct use case.

**When is `--update-spec` safe?** See the section above: only to refresh the *generated* `vhco.json`, never to
silence drift against the hand-authored `vhco-contract.json`. If a contract exists, the fix for drift is code,
not `--update-spec`.

**`sync` is 0 but `validate` is red — did I finish?** No (State C). The code matches the design but breaks an
architecture rule. Fix the structure. A change is done only when **both** are satisfied (plus tests + docs).

**Which spec does `sync` use?** `vhco-contract.json` if present, otherwise `vhco.json`. The todo checks
(`no_todos_declared`) apply only when the spec is the hand-authored contract.

## Common confusion

- **`missing_*` means the code is behind the contract** (implement it). **`extra_*` means the code is ahead of
  the contract** (the design never approved it — add to the contract or remove from code).
- **Descriptive markers never cause drift.** Add `vhco:test`/`vhco:error`/`vhco:api` freely.
- **`validate` reads only code; `sync` reads code + contract.** That's why they can disagree.
- **The signature-level comparisons** (`use_case_input_differs`, `port_contract_differs`, flow comparisons)
  are richest for Go (full signatures via go/ast); for Python/Rust/TS they're best-effort, but the
  structural and todo comparisons are exact everywhere.
