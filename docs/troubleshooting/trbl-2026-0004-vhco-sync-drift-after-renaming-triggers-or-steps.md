---
document_id: TRBL-2026-0004
title: "vhco sync reports flow drift after renaming a trigger or a step"
document_type: troubleshooting
status: resolved
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [serve, cli]
current_status: resolved
affected_versions:
  from: "0.2.0"
  to: null
confidentiality: internal
scope: Reusable knowledge from PLAN-2026-0002 P1/P2a — which annotation edits make `vhco sync` report drift, and how to fix it without regenerating the contract.
reason: DOCUMENTATION §26 — the drift appeared after edits that look cosmetic (a flag rename in a trigger line, a new `vhco:step`), and every later v0.2.0 phase (globals, highlighting, FFI, modules) will hit it again.
related_documents: [PLAN-2026-0002, RES-2026-0004, TRBL-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, troubleshooting, vhco, contract]
---

# vhco sync reports flow drift after renaming a trigger or a step

> **Status:** Resolved
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** serve, cli

## Problem

After the v0.2.0 envelope work renamed `--params` to `--data` in `vhco:trigger` lines and added `vhco:step` lines,
`vhco sync .` reported drift although no use case, port or domain type had changed.

## Symptoms

```text
✗ code and vhco-contract.json differ in 14 place(s):
  - flow_triggers_differ   flow execution.request_operation — spec triggers [cli = rivet request ID --params JSON …], code triggers [cli = rivet request ID --data JSON …]
  - flow_handling_differs  flow serve.project_polling — spec handling [authorize: require_operation → open: driver.open → read: driver.read → encode: error_envelope],
                           code handling [authorize: require_operation → open: driver.open → accept: ResponseEnvelope::accepted → read: driver.read → encode: ResponseEnvelope::from_error]
```

## Environment and Versions

vhco 1.6.0, Rivet `main` during PLAN-2026-0002 P1/P2a (macOS).

## Investigation

Scratch copies of the contract were edited one field at a time and compared with `vhco sync . <copy>.json`
(RES-2026-0004 E1 used the same method):

```text
 edit in the contract copy                          sync reports?
 ─────────────────────────────────────────────────  ──────────────────────────────
 domain field name or type                          domain_fields_differ
 surface `calls` list                               surface_calls_differ
 use case / todo id / port                          missing_* / todo_not_implemented
 flow `triggers[].invocation` text                  flow_triggers_differ      ◀── exact string compare
 flow `output`, flow `through` (ports)              flow_output_differs / flow_ports_differ
 flow `handling[]` layer or ref                     flow_handling_differs     ◀── layer:ref sequence, in order
 flow `handling[].role`, use-case `about`, todo     not compared (prose)
 text, `examples`, `failures`
```

## Possible Causes

The contract flows are a design artifact that mirrors code annotations: `vhco:trigger` lines become flow triggers and
the `vhco:step <id> <ref>` lines of a use case become its flow's handling layers.

## Experiments and Attempts

Changing only a handling `role` left sync green; changing a `layer` or `ref`, adding a step, or editing one character
of a trigger's invocation produced drift.

## Root Cause

`vhco sync` compares flow triggers (exact text), outputs, ports and the ordered `layer: ref` pairs of the handling,
not only features, surfaces, use cases, todos, ports and domain fields.

## Solution or Workaround

When a trigger's wording or a use case's step sequence changes, edit the matching flow in `vhco-contract.json` **by
hand** in the same change (copy the exact invocation text; keep each step's id as `layer` and its ref as `ref`, and
write the role as prose). Never regenerate the contract from code (`vhco spec . > vhco-contract.json` erases the
design) and never use `--update-spec` to silence it.

```text
 code: // vhco:trigger cli execution/request_operation = rivet request ID --data JSON …
          │ exact text
          ▼
 contract: flows[action=execution.request_operation].triggers[surface=cli].invocation
```

## Verification

After the hand edits, `vhco sync .` listed only the planned future-phase gaps (globals, highlighting, FFI surface,
capability report); `vhco validate .` and `vhco check .` stayed green.

## Remaining Limitations

A contract written ahead of the code (future phases) can only guess step ids; the implementing phase must align the
flow handling with its final `vhco:step` lines.

## Current Status

Resolved (documented procedure).

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — TASK-004, P2a
- [RES-2026-0004](../research/res-2026-0004-workspace-ffi-and-feature-experiments.md) — E1 method
- [TRBL-2026-0001](trbl-2026-0001-vhco-helper-files-counted-as-use-cases.md) — another vhco model surprise

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Recorded from the P2a envelope work. |
