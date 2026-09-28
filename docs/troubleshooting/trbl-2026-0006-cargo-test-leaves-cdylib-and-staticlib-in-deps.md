---
document_id: TRBL-2026-0006
title: "cargo test leaves librivet (cdylib and staticlib) in target/<profile>/deps, not target/<profile>"
document_type: troubleshooting
status: resolved
created_date: 2026-09-29
last_updated: 2026-09-29
document_revision: 2
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [library, ffi]
current_status: resolved
affected_versions:
  from: "0.2.0"
  to: null
confidentiality: internal
scope: Reusable knowledge from PLAN-2026-0002 P2d — where a workspace test run puts the C libraries of rivet-ffi, and how tests that link C or load them from Python must find them.
reason: DOCUMENTATION §26 — a test that looks for target/debug/librivet.dylib passes on a machine that once ran `cargo build -p rivet-ffi` and fails (or links a stale library) on a clean CI checkout.
related_documents: [PLAN-2026-0002, RES-2026-0004, ADR-0005]
supersedes: null
superseded_by: null
tags: [rivet, troubleshooting, cargo, ffi, cdylib, staticlib]
---

# cargo test leaves librivet (cdylib and staticlib) in target/<profile>/deps, not target/<profile>

> **Status:** Resolved
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** library, ffi

## Problem

`tests/conformance_ffi.rs` links the C examples against librivet and loads it from Python. It first looked for
`target/debug/librivet.dylib` and `target/debug/librivet.a`. After deleting those two files and running
`cargo test --workspace --all-features --no-run`, they were **not** recreated, although rivet-ffi was compiled.

## Symptoms

```text
$ rm target/debug/librivet.{dylib,a}; touch ffi/src/lib.rs
$ cargo test --workspace --all-features --no-run
   Compiling rivet-ffi v0.1.0 (…/ffi)
$ ls target/debug/librivet.*
target/debug/librivet.d                           ← only the dep-info file
$ ls target/debug/deps/librivet.{dylib,a}
target/debug/deps/librivet.a  target/debug/deps/librivet.dylib   ← the fresh libraries
```

## Environment and Versions

cargo 1.90.0, macOS 26.4 (arm64); workspace `rivet-runtime` + `ffi/` (`crate-type = ["cdylib", "staticlib", "rlib"]`).

## Investigation

```text
 cargo build -p rivet-ffi ──compile──▶ target/debug/deps/librivet.{dylib,a}
                            └─uplift──▶ target/debug/librivet.{dylib,a}      (hard link / copy)

 cargo test  --workspace  ──compile──▶ target/debug/deps/librivet.{dylib,a}   (lib built for ffi/tests)
                            └─ no uplift of library artifacts in test mode
```

## Possible Causes

1. The lib target is not compiled at all by `cargo test` (rejected: `Compiling rivet-ffi` is printed and
   `deps/librivet.dylib` has a fresh timestamp).
2. Cargo only copies ("uplifts") final artifacts to `target/<profile>` for `cargo build`, not for the
   libraries a test run builds as dependencies of test targets (confirmed by the listing above).

## Experiments and Attempts

| Attempt | Result |
|---|---|
| Look in `target/debug` only | Works locally only after a manual `cargo build -p rivet-ffi`; stale or missing in CI |
| Build the library from inside the test (`cargo build -p rivet-ffi`) | Rejected: a nested cargo on the same target directory waits on the build lock held by the outer `cargo test` |
| Look next to the test binary (`current_exe().parent()` = `target/<profile>/deps`), then `target/<profile>` | Works for `cargo test --workspace` and after `cargo build` |

## Root Cause

In test mode Cargo leaves the cdylib and staticlib of a workspace library in `target/<profile>/deps`; only
`cargo build` uplifts them to `target/<profile>`.

## Solution or Workaround

Tests that need librivet search `target/<profile>/deps` first (the directory of the running test binary), then
`target/<profile>`; the C Makefile and the Python wrapper take the directory (`TARGET=…`) or the file
(`RIVET_LIB=…`) explicitly. The suite must run as `cargo test --workspace` so rivet-ffi is compiled in the same
run; alone (`-p rivet-runtime`) it skips with a note, and with `CI` set a missing library fails.

## Verification

`cargo test --workspace --all-features --test conformance_ffi`: 7 passed, linking `target/debug/deps/librivet.*`
shared and static (commit `f9af92a`).

## Update (2026-09-29, revision 2)

The behaviour above only applied while rivet-ffi also built an `rlib` and had integration tests in `ffi/tests/`:
building those tests forced its `cdylib` and `staticlib` into `deps`. INC-2026-0010 removed the `rlib`, because
it collided with rivet-runtime's `librivet.rlib`, and moved the tests into the crate. Now **`cargo test` does not
produce librivet at all**. Build it explicitly first:

```text
 cargo build --workspace --all-features   ─▶ target/debug/librivet.{dylib|so,a}   (conformance_ffi finds it here)
 cargo test  --workspace --all-targets --all-features
```

CI (`.github/workflows/ci.yml`, step "build (debug, librivet for the FFI tests)") and `perch tests` run the build
first (commit 8045343).

## Remaining Limitations

With `-p rivet-runtime` alone the deps directory may hold an older librivet from a previous workspace run;
the suite cannot detect that staleness. CI always runs `--workspace`.

## Current Status

Resolved (documented procedure, implemented in `tests/conformance_ffi.rs`).

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — TASK-043
- [RES-2026-0004](../research/res-2026-0004-workspace-ffi-and-feature-experiments.md) — E2 (cdylib and staticlib export)
- [ADR-0005](../decisions/adr-0005-workspace-package-and-features.md) — workspace layout

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 2 | 2026-09-29 | Claude | Update after INC-2026-0010: cargo test no longer builds librivet; build it first (8045343). |
| 1 | 2026-09-29 | Claude | Recorded from PLAN-2026-0002 P2d. |
