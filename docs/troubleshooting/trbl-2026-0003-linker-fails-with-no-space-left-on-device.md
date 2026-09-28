---
document_id: TRBL-2026-0003
title: "Build fails at link time with "No space left on device""
document_type: troubleshooting
status: resolved
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [execution]
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

# Build fails at link time with "No space left on device"

> **Status:** Resolved
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0-dev
> **Owner:** Project maintainer
> **Affected Components:** execution

## Problem

`cargo build` failed while linking the `rivet` binary.

## Symptoms

`error: linking with cc failed` … `ld: write() failed, errno=28 (No space left on device)`; `df -h` showed ~150 MiB free.

## Environment and Versions

macOS, several parallel git worktrees each with its own `target/` directory (1–4.5 GiB each), plus research prototype builds.

## Investigation

`du -sh .claude/worktrees/* target` and scratch directories identified ~27 GiB of build artifacts.

## Possible Causes

Per-worktree `target/` directories multiply the dependency build (quinn, tonic, rustls, axum, h3, …).

## Experiments and Attempts

Removing merged worktrees (`git worktree remove --force`) and prototype `target/` directories freed ~25 GiB; the build then succeeded unchanged.

## Root Cause

Disk exhaustion from duplicated build artifacts, not a code problem.

## Solution or Workaround

Remove worktrees after merging; keep one shared `target/` (or set `CARGO_TARGET_DIR`) when running parallel worktrees; `cargo clean` prototypes.

## Verification

`cargo build` and `cargo test` green after cleanup.

## Remaining Limitations

A full debug build of Rivet with tests needs roughly 9 GiB of `target/`.

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
