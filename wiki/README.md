# VHCO agent wiki — index

The deeper reference behind [`../AGENTS.md`](../AGENTS.md), for agents working on a **closed-world,
five-folder VHCO project**. `AGENTS.md` is the quick playbook; these pages expand each topic so you never
have to guess.

> This `docs/agents/` bundle is **self-contained** — every link below is internal to it. Backtick'd paths
> like `docs/wwd/…` or `docs/guide.md` point to the **vhco tool's own repository** (the canonical, exhaustive
> references); they are not part of this distributable bundle.

## What this wiki is

`vhco` is a tool that **models, validates, and visualizes** a codebase from `vhco:` comment annotations.
It does **not** generate, scaffold, or compile code (there is no `apply`/`init`/`create-*`). These pages
describe how to build and maintain a project *the vhco way*: declare the architecture in comments, design
in a hand-authored contract, implement to match, and keep two gates green.

## How to use it

- **First time?** Read pages 1–4 in order — they are the model you must hold in your head.
- **Working a change?** Page 1 (the loop) is the checklist; page 8 (commands) is the reference; page 4
  (validate vs sync) explains the two gates you keep green.
- **Adding code that touches the outside world (net/db/file/…)?** Pages 2 and 6 cover edge annotations;
  page 9 covers the optional policy/sandbox layer.
- **Just need a command's exact flags / output?** Jump to page 8.
- **Anything about the optional `.vhco.json` intelligence layer?** Page 9.

## Read in this order

| # | Page | What it covers |
|---|---|---|
| 1 | [The loop / workflow](01-workflow.md) | the 6 steps for every change: **contract → eval → code → test → validate → docs** |
| 2 | [Annotations](02-annotations.md) | the `vhco:` directive grammar — what each is, and where it lives |
| 3 | [Architecture](03-architecture.md) | the five folders + the closed-world import rule |
| 4 | [validate vs sync](04-validate-and-sync.md) | the two gates — what each checks, why they disagree |
| 5 | [Todos, steps & tests](05-traceability.md) | the three traceability markers; what `sync` enforces |
| 6 | [Markers](06-markers.md) | errors · edges · API/CLI · captures (all descriptive) |
| 7 | [Docs & README in sync](07-docs-and-readme.md) | keeping the human docs current — **part of every change** |
| 8 | [Commands](08-commands.md) | the full command set, by phase |
| 9 | [Codebase intelligence](09-intelligence.md) | optional `.vhco.json` layers: **language/dirs** (make validate work for any language) · extract · rules · conventions · sandbox |

## The one-paragraph version

A VHCO program is **five top-level folders** with a strict closed-world import rule, whose architecture is
declared in `vhco:` comments. For every change you **(1)** edit the hand-authored design spec
`vhco-contract.json` first, **(2)** audit it in the browser with `vhco live .`, **(3)** write annotated code
to match, **(4)** write & pass tests, **(5)** keep `vhco validate .` green and `vhco sync .` at 0, and
**(6)** update the top-level `README.md` + `docs/` so the human docs match the new state. The model lives in
the comments and the tree, so it can never drift from the code: `validate` proves the architecture is legal,
`sync` proves the code matches the design, and `spec`/`doc` project the result. Never regenerate the contract
from code; never let a use case do I/O or know its surface.

## The non-negotiables (memorise)

1. **Design first, in `vhco-contract.json`** — render with `vhco live`, get human approval, *then* code.
   The contract is the target the code is built toward, not a byproduct of it.
2. **Two files, two phases:** `vhco-contract.json` = hand-authored design spec; `vhco.json` = generated
   model (the projection of the code's annotations). ⛔ **never** `vhco spec . > vhco-contract.json` — that
   overwrites your design with whatever you happen to have built. `vhco sync --update-spec` only ever writes
   `vhco.json`, never the contract.
3. **Use cases are pure and surface-blind** — capabilities arrive as **port parameters** (each port is a set
   of named functions with typed inputs/outputs); a use case does no I/O and never knows its surface (no
   `trigger`). The composition root (`orchestrator/setup_<surface>`) wires; it holds no logic.
4. **Registration is per surface, not per feature** — each surface has its own
   `orchestrator/setup_<surface>` file, and `validate` enforces it. One feature can be reached from many
   surfaces.
5. **Two gates, always:** `vhco validate .` stays green **continuously** (architecture); `vhco sync .`
   **shrinks to 0** (design drift). If `validate` fails, fix the code/annotations; if `sync` reports drift,
   implement the missing pieces — don't paper over it with `--update-spec`.
6. **Annotate everything you add** — unannotated code doesn't exist in the model. The model is
   language-agnostic because it's *declared in comments*, not reverse-engineered.
7. **Every use case declares ≥1 todo**, claimed in code (traceability — see page 5).
8. **Every change ends tested and with README + `docs/` updated** — the human docs are part of the change,
   not a follow-up.
