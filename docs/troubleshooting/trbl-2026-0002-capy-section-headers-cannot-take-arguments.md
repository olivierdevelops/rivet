---
document_id: TRBL-2026-0002
title: "Capy block sections cannot carry arguments (try/catch filters)"
document_type: troubleshooting
status: resolved
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [language, capy_parser]
current_status: resolved
affected_versions:
  from: "0.1.0-dev"
  to: "0.1.0-dev"
confidentiality: internal
scope: Reusable knowledge from PLAN-2026-0001 implementation.
reason: DOCUMENTATION §26 — non-obvious problems that could recur are recorded as troubleshooting knowledge.
related_documents: [PLAN-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, troubleshooting]
---

# Capy block sections cannot carry arguments (try/catch filters)

> **Status:** Resolved
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0-dev
> **Owner:** Project maintainer
> **Affected Components:** language, capy_parser

## Problem

`try … catch error kind http … end` must parse with a filter on the `catch` line, but Capy `block_sections` accepts bare section keywords only, and a `block_closer` target cannot open its own body.

## Symptoms

With `block_closer catch`, the lines after `catch …` parsed as siblings of the enclosing block and its `end` closed the wrong block, yet the parse reported no diagnostics.

## Environment and Versions

capy-core 0.22.0 (commit 84f984c6).

## Investigation

Dumped the AST JSON (`examples/dump_ast.rs`, RES-2026-0001 harness) for the try/catch sample and walked the tree structure.

## Possible Causes

Section headers without captures; closers without bodies.

## Experiments and Attempts

`block_sections catch closer end` (rejects the filter); `block_closer catch` with `catch` as a block opener (body not attached); `try` as `block_dedent` plus a sibling `catch <filter> … end` block (correct tree).

## Root Cause

Capy grammar capabilities at the pinned commit.

## Solution or Workaround

`try` is `block_dedent`; `catch <filter>` is its own block closed by `end`; lowering pairs each `try` with the immediately following `catch` (`syntax.try_without_catch` otherwise). Surface syntax unchanged.

## Verification

Structural corpus walk (no stray `end`, every catch after a try) and tests/conformance_samples.rs.

## Remaining Limitations

`finally` sections are not supported by the grammar in v0.1.0.

## Current Status

Resolved.

```text
Problem → Symptoms → Investigation → Experiments → Root cause → Solution → Verified
```

## Related Documents

- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md)
- [RES-2026-0001](../research/res-2026-0001-capy-grammar-spike.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Recorded. |
