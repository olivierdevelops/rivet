# 7 · Docs & README in sync — part of every change

[← Markers](06-markers.md) · [wiki index](README.md) · next: [Commands →](08-commands.md)

A VHCO project ships **two layers of documentation that must never disagree**. Keeping the human layer
current is **step 6 of [the loop](01-workflow.md)** — a change isn't done until it's done.

| Layer | Files | Audience | Kept current by |
|---|---|---|---|
| **Machine model** | `vhco-contract.json` (design) · `vhco.json` (generated) · `vhco.html` (explorer) | the tool / reviewers | steps 1 & 5 of the loop |
| **Human docs** | top-level **`README.md`** · top-level **`docs/`** folder | people reading the repo | **step 6 of the loop** |

## Why this matters

The whole premise of VHCO is that **the documentation can't drift from the code**. The annotations keep the
*model* tied to source automatically — `vhco spec` re-derives it, `validate`/`sync` gate it. But the
*prose* — the README and `docs/` — has no parser keeping it honest; only **discipline** does. Step 6 is that
discipline. Skip it and the repo's self-description rots: a human (or an agent picking the project up later)
reads a README that promises an endpoint that no longer exists, or misses a feature that does. Both the
machine model and the human docs **ship together and must agree** — that's the contract VHCO makes with anyone
reading the repo.

A doc describing a use case, surface, command, or edge that **no longer exists** is a **bug** in the docs,
exactly like dead code. Treat it as one.

## What a VHCO project is expected to have

- A top-level **`README.md`** — what the project is, its features, its surfaces/commands, install & run, a few
  examples.
- A top-level **`docs/`** folder — the prose: architecture notes, how-tos, per-feature pages, anything a
  human needs that doesn't belong in code comments.

## How vhco auto-detects and indexes them

vhco **auto-detects** a top-level `README.*` and a `docs/` folder when `.vhco.json`/`vhco-contract.json` has
no `documentation` block configured. You can point it explicitly:

```jsonc
// in .vhco.json or the contract
"documentation": { "readme": "README.md", "dir": "docs" }
```

Their **contents are indexed**, so they surface in two places:

- the explorer's **Documentation** view — the rendered prose, side-by-side with the model;
- project **search** — so a phrase describing stale behaviour is *findable*, not hidden.

In **live mode** (`vhco live .`), the captured contents of the README and `docs/` are refreshed on every
browser request, so the human sees the current prose as they audit the design. That means stale docs are
visible *during* review, not just after.

## The rule

> After `vhco validate .` passes (and `sync` is 0), update the README and `docs/` so a human reading **only
> them** would get a **correct** picture of the project as it is now.

## What to update, by kind of change

| You changed… | Update in README | Update in `docs/` |
|---|---|---|
| **New feature / use case** | add it to the feature list | add or extend a page describing what it does and how to use it |
| **New surface** | add it to the "surfaces"/usage section | reflect it in the relevant page (a new CLI/HTTP/worker surface changes how the project is reached) |
| **New command / endpoint** | add it to the usage/commands section with an example | document it on the relevant page (it's already in the **API & CLI** explorer view via `vhco:api`) |
| **New / changed edge** (db / file / net / env) | if the README documents configuration / permissions / env vars, reflect the new edge there | note any new setup the edge requires |
| **Removed something** | delete the prose that described it — a feature list promising a removed capability is a bug | delete or revise the page; remove stale examples |
| **Renamed something** | update every mention of the old name | update every mention, including examples and links |

> Notice the asymmetry of the **machine** side: `vhco:api`/`request`/`response` already put the route in the
> explorer's **API & CLI** view automatically — but that does *not* update the human README/`docs/`. The
> explorer is the model view; step 6 is about the prose a person reads. Both must agree.

## Checklist before you call a change done

- [ ] README's feature list includes everything you added and nothing you removed.
- [ ] README's surfaces / commands / endpoints match what the code now exposes.
- [ ] README's install / run / examples still work as written.
- [ ] `docs/` has a page (new or revised) for what you changed.
- [ ] No page in `docs/` describes a use case, surface, command, or edge that no longer exists.
- [ ] If you renamed anything, every mention (including examples) is updated.
- [ ] `vhco doc . --format html` regenerated and the **Documentation** view reads correctly.

## How to self-check you're in sync

```sh
vhco doc . --format html        # regenerate the explorer (vhco.html)
# open vhco.html → Documentation view: does the prose match the current features / surfaces / edges?
# use project search: does anything describe code that no longer exists?
```

A second, model-driven cross-check: list what the code actually exposes and confirm the README mentions it.

```sh
vhco visualize .                # every user action + the ports it needs — compare to the README's feature list
vhco query . "actions"          # the actions the model knows about (structured)
```

If a human reading the README + `docs/` would be **misled** about the current project, the change is **not
finished**. The machine model (contract / `vhco.json`) and the human docs (README / `docs/`) ship together and
always agree.

---

## FAQ

**Why can't the docs just lag a little?** Because the entire value proposition of vhco is *the repo's
self-description never drifts from the code*. The model is kept honest by the parser; the prose is kept honest
only by step 6. Letting it lag breaks the one guarantee the project exists to make.

**Does updating `vhco:api` / the contract update the README for me?** No. Annotations and the contract update
the **machine model** (and the explorer's API & CLI / Documentation views). They do **not** edit your
top-level `README.md` or `docs/` files — that's your job in step 6. The two layers are kept in sync by you,
not automatically.

**Where should new prose go — README or `docs/`?** README = the at-a-glance picture (what it is, its
features, how to run it, a few examples). `docs/` = the deeper prose (architecture notes, per-feature how-tos).
A new feature usually touches **both**: a line in the README feature list and a page (or section) in `docs/`.

**My change only touched code that's already documented — do I still update docs?** Only if the *behaviour or
interface* a human reads about changed. A pure internal refactor with identical behaviour and identical
surfaces/commands needs no prose change — but verify nothing in the README/`docs/` now reads stale (e.g. an
example output that changed).

**What if there's no `documentation` block configured?** vhco auto-detects a top-level `README.*` and a
`docs/` folder. You only need a `"documentation": { "readme": ..., "dir": ... }` block to point at
non-standard locations.

**How do I know the docs are actually being indexed?** Open `vhco.html` (or `vhco live .`) and check the
**Documentation** view shows your README/`docs/` contents, and that project **search** finds a phrase from
them. If they don't appear, configure the `documentation` block explicitly.

## Common confusion

- **Two layers, two owners of "current".** The machine model is kept current by steps 1 & 5; the human docs by
  step 6. Both must describe the project *as it is now*.
- **The explorer is not the README.** `vhco:api` populating the API & CLI view does not write your README.
- **A doc describing removed behaviour is a bug.** Deleting code means deleting (or revising) the prose that
  described it.

## Why not let the docs lag?

Because the whole point of vhco is that **the documentation can't drift from the code**. The annotations keep
the *model* tied to source automatically; this step keeps the *prose* tied to source by discipline. Both
together mean the repo's picture of itself is always the code as it is *now* — not as someone described it
months ago.
