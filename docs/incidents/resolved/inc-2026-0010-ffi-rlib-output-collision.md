---
document_id: INC-2026-0010
title: "rivet-ffi and rivet-runtime both produced librivet.rlib (output filename collision)"
document_type: incident
status: resolved
created_date: 2026-09-29
last_updated: 2026-09-29
document_revision: 2
authors: [Claude]
owner: Project maintainer
severity: "S3"
start_time: 2026-09-29T03:00:00Z
end_time: 2026-09-29T03:40:00Z
root_cause_status: identified
systems: [Rivet]
components: [library]
affected_versions:
  from: "0.2.0-dev"
  to: "0.2.0-dev"
confidentiality: internal
scope: Build defect in the PLAN-2026-0002 P2d workspace, found by the first Windows CI run; fixed before release.
reason: DOCUMENTATION §25 — unexpected defects found during implementation are recorded as incidents.
related_documents: [PLAN-2026-0002, ADR-0005, RES-2026-0004, INC-2026-0011]
supersedes: null
superseded_by: null
tags: [rivet, incident, ffi, build, cargo]
---

# rivet-ffi and rivet-runtime both produced librivet.rlib (output filename collision)

> **Status:** Resolved (fixed in `6f9943f`, PLAN-2026-0002)
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0-dev (unreleased)
> **Owner:** Project maintainer
> **Affected Components:** library

## Incident Summary

Two packages in the workspace produced a library file with the same name:
- `rivet-runtime`, whose `lib` is named `rivet`;
- `rivet-ffi`, whose `lib` is also named `rivet`, so that its shared and static libraries are called `librivet`. It
  was built with the crate types `cdylib`, `staticlib` **and** `rlib`.

Both wrote `target/<profile>/librivet.rlib`. Cargo warned on every platform ("output filename collision … may
become a hard error"). On Windows the `rivet` binary and the `rivet` library also shared `rivet.pdb`, and the
release link failed with `LNK1201: error writing to program database`.

```text
 rivet-runtime  lib "rivet"  ─▶ target/release/librivet.rlib  ◀─ collision ─┐
 rivet-ffi      lib "rivet"  ─▶ target/release/librivet.rlib ───────────────┘  (rlib crate type)
                              ─▶ librivet.dylib / librivet.a            (wanted)
 fix: rivet-ffi crate-type = [cdylib, staticlib]  → only the wanted artifacts; no .rlib
```

## Severity

S3: a build hazard. It silently overwrote an intermediate artifact and broke the Windows release link. No runtime behaviour was affected.

## Status

Resolved in commit `6f9943f`, before any release.

## Discovery Context

The first CI run with Windows in the matrix (run 36469534765, job "test (windows-latest)", step "build (release)"), read through the `ci_step.py` annotations.

## Start Time

2026-09-29T03:00:00Z

## End Time

2026-09-29T03:40:00Z

## Affected Systems

Rivet build (workspace).

## Affected Versions

0.2.0-dev only; v0.1.0 had no `rivet-ffi` crate.

## Affected Components

- library (`ffi/` crate packaging)

## Customer Impact

None: never released.

## Detection

A cargo warning on every OS, and a link failure on Windows.

## Reproduction Steps

1. Before the fix, run `cargo build --release --workspace --all-features`.
2. Observe `warning: output filename collision … Colliding filename is: …/target/release/librivet.rlib`.

## Timeline

```text
2026-09-29T03:00Z  Windows CI build fails with LNK1201; the annotation shows the collision warning
2026-09-29T03:20Z  reproduced locally on macOS (the same warning)
2026-09-29T03:40Z  fix committed in 6f9943f; 8 FFI safety tests pass in-crate; collision count 0
```

## Logs and Evidence

The CI annotation "build: last 80 lines (exit 101)", and the local `cargo build` output before and after the fix.

## Source Files

- `ffi/Cargo.toml` (`crate-type`)
- `ffi/src/lib.rs`, `ffi/src/tests.rs` (formerly `ffi/tests/ffi_safety.rs`)

## Root Cause

The `rlib` crate type was added only so that the integration tests in `ffi/tests/` could link against the
exported functions. That created a second `librivet.rlib`.

## Contributing Factors

The FFI library must be named `librivet` (`-lrivet`, `rivet.h`), and that is the same name as the runtime's
library crate.

## Resolution

`rivet-ffi` builds only `cdylib` and `staticlib`. The T-12 safety tests became a `#[cfg(test)]` module of the
crate (`ffi/src/tests.rs`), where they call the `extern "C"` functions directly.

## Verifying Tests

- `cargo test -p rivet-ffi --all-features`: 8 passed.
- `cargo test --test conformance_ffi`: 7 passed.
- `cargo build --release --workspace --all-features`: 0 collision warnings.
- Follow-up `8045343`: without the `rlib`, `cargo test` no longer builds `librivet` as a side effect, so CI and
  `perch tests` now run `cargo build --workspace --all-features` first. CI run 36483001760 is green on macOS and Linux.

## Corrective Actions

Fixed before release.

## Preventive Actions

CI builds the whole workspace with `--all-features`, so a new collision warning shows up in the build step's annotations.

## Owners

Implementer (fix); project maintainer (review).

## Remaining Risks

None on macOS or Linux. Windows is not supported (INC-2026-0011).

## Lessons Learned

Giving two workspace crates the same library name needs care: only the artifacts that are actually wanted may be built.

## Related Documents

- [PLAN-2026-0002](../../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md)
- [ADR-0005](../../decisions/adr-0005-workspace-package-and-features.md)
- [INC-2026-0011](../active/inc-2026-0011-windows-port-failures.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 2 | 2026-09-29 | Claude | Follow-up: build librivet before tests (8045343); CI green. |
| 1 | 2026-09-29 | Claude | Recorded and resolved. |
