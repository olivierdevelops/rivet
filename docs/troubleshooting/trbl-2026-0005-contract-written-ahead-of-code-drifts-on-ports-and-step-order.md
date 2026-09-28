---
document_id: TRBL-2026-0005
title: "A contract use case written ahead of its code drifts on port parameter names, step order and surface calls"
document_type: troubleshooting
status: resolved
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [language, registry, cli, library]
current_status: resolved
affected_versions:
  from: "0.2.0"
  to: null
confidentiality: internal
scope: Reusable knowledge from PLAN-2026-0002 P2b/P2f — three `vhco sync` mismatches that appear when a use case, its ports and its flow are authored in vhco-contract.json before the code exists, and how to write the contract so it matches on the first implementation.
reason: DOCUMENTATION §26 — each mismatch looked like a code defect but was a contract-authoring rule; P2c, P2d and P2e author contract entries the same way (highlight_source, the ffi surface, rivet_load).
related_documents: [PLAN-2026-0002, TRBL-2026-0004, RES-2026-0004]
supersedes: null
superseded_by: null
tags: [rivet, troubleshooting, vhco, contract]
---

# A contract use case written ahead of its code drifts on port parameter names, step order and surface calls

> **Status:** Resolved
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** language, registry, cli, library

## Problem

The P2f contract delta (TASK-100) described `language.resolve_imports` and `registry.load_module` in full before
any code existed. The first `vhco sync .` after implementing them still reported drift on entries that were
"the same" in both places.

## Symptoms

```text
✗ port_contract_differs  use case language.resolve_imports port SourceLoader —
    spec requires [… read_module(root: string, path: string) → SourceFile],
    code provides [… read_module(string, string) → SourceFile]
✗ flow_handling_differs  flow language.resolve_imports —
    spec handling [… read → cycles → public → policy], code handling [… read → policy → cycles → public]
✗ surface_calls_differ   surface cli — spec exposes [… language/compile_globals …], code exposes […]
    (P2b: the TASK-004 flow gave compile_globals a cli trigger, but the cli surface did not call it)
```

## Environment and Versions

vhco 1.6.0, Rivet `main` during PLAN-2026-0002 P2b/P2f (macOS).

## Investigation

Each finding was compared against the code annotation it comes from:

```text
 code annotation                                         what vhco keeps
 ──────────────────────────────────────────────────────  ───────────────────────────────────────
 // vhco:port SourceLoader { read_module(root: string,    read_module(string, string)  ◀─ names dropped
 //                          path: string) -> SourceFile }
 // vhco:step policy loader.policy_beside  (inside the     order = order of the step lines in the
 //   import loop, above the later `cycles` step)            file, not the order work "happens"
 // vhco:trigger cli language/compile_globals = …          requires cli `calls` to list the action
```

## Possible Causes

The contract was written from the design (named parameters, logical phases) rather than from the annotation
grammar vhco parses.

## Experiments and Attempts

Removing the parameter names from the contract port cleared `port_contract_differs`; moving `policy` before
`cycles` in the contract flow cleared `flow_handling_differs`; adding the action to the surface `calls` in both
the contract and the `vhco:surface` line cleared `surface_calls_differ` (a trigger without the call is a
`validate` failure — dangling trigger — so the call cannot be skipped on the code side).

## Root Cause

`vhco sync` compares port functions by parameter **types**, flow handling by the **textual order** of
`vhco:step` lines, and flow triggers only for actions the surface **calls**.

## Solution or Workaround

When authoring a use case ahead of the code:

1. write port function inputs as `{"type": …}` only (no `name`);
2. list flow `handling` in the order the `vhco:step` lines will appear in the source file (a step inside a
   loop counts where it is written);
3. for every flow trigger on surface S, add the action to S's `calls` in the contract **and** to the
   `vhco:surface S … calls …` line in `orchestrator/setup_S.rs`.

Edit the contract by hand (TRBL-2026-0004); never regenerate it.

## Verification

After the three hand edits `vhco sync .` listed only the P2c/P2d/P2e entries (8 gaps); `vhco validate .` and
`vhco check .` stayed green.

## Remaining Limitations

Parameter names in port signatures are lost in the model; keep them in the port's doc comment if they matter.

## Current Status

Resolved (documented procedure).

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — TASK-020…024, TASK-100…107
- [TRBL-2026-0004](trbl-2026-0004-vhco-sync-drift-after-renaming-triggers-or-steps.md) — trigger text and step drift

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Recorded from P2b/P2f. |
