---
document_id: TRBL-2026-0001
title: "vhco counts helper files in a feature folder as use cases"
document_type: troubleshooting
status: resolved
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [language, auth]
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

# vhco counts helper files in a feature folder as use cases

> **Status:** Resolved
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0-dev
> **Owner:** Project maintainer
> **Affected Components:** language, auth

## Problem

`vhco sync` reported extra use cases `language.expr`, `language.lower` and `auth.support` that the contract does not declare.

## Symptoms

`extra_use_case use case language.expr — in the code but not in the spec`; the generated model lists name-only use cases for helper modules.

## Environment and Versions

vhco 1.6.0, Rust layout `src/features/<feature>/*.rs`.

## Investigation

Generated the model with `vhco spec . --stdout` and inspected `definition.features[].use_cases`: every `.rs` file directly in a feature folder (except `mod.rs` and `ports.rs`) became a use case, annotated or not.

## Possible Causes

(a) missing `vhco:usecase` annotation; (b) file placement rule.

## Experiments and Attempts

A scratch project with `features/notes/support/helper.rs` (a subfolder with `mod.rs`) produced no extra use case and `vhco validate` stayed green.

## Root Cause

vhco models one use case per file in a feature folder (AGENTS.md "one use case per file"); subfolders are not scanned as use cases.

## Solution or Workaround

Move non-use-case helpers into a subfolder: `src/features/language/lowering/{expr,lower}.rs`, `src/features/auth/support/mod.rs` (commit 97f855e).

## Verification

`vhco sync .` → code matches vhco-contract.json; `vhco validate .` green.

## Remaining Limitations

None.

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
