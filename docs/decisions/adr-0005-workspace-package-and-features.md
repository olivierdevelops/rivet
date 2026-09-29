---
document_id: ADR-0005
title: "Cargo workspace, package rivet-runtime, rivet-ffi crate and Cargo feature set for v0.2.0"
document_type: decision
status: approved
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
approval:
  decision_date: 2026-09-28
  approvers: [Project maintainer]
  basis: "ADR-0004 approval of PROP-2026-0002 (R10–R13, C-07–C-10 and the Contract Delta); PLAN-2026-0002 TASK-009 records the P1 experiment outcome here, and the proposal's fallback is not triggered"
systems: [Rivet]
components: [library, registry, cli]
affected_versions:
  from: "0.1.0"
  to: "0.2.0"
scope: The repository layout (workspace members), package and library names, where the C ABI's logic and shims live, and the Cargo feature set and gating pattern for v0.2.0.
reason: PLAN-2026-0002 TASK-009 — record the packaging decision after experiments E1–E3 (RES-2026-0004), including whether the risk-table fallback (FFI shims behind an `ffi` feature in the main crate) is needed.
related_documents: [RES-2026-0004, PROP-2026-0002, PLAN-2026-0002, ADR-0004, ADR-0002]
supersedes: null
superseded_by: null
tags: [rivet, workspace, packaging, ffi, cargo-features, decision, v0.2.0]
confidentiality: internal
review_cycle: on-change
next_review_date: 2026-12-28
---

# Cargo workspace, package rivet-runtime, rivet-ffi crate and Cargo feature set for v0.2.0

> **Status:** Approved
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 → 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** library, registry, cli (C ABI and packaging)

## Status

Approved on 2026-09-28 under the maintainer's approval of PROP-2026-0002
([ADR-0004](adr-0004-approve-envelopes-globals-library-ffi-highlighting.md)). The design choices below are the
proposal's; this ADR records that the P1 experiments ([RES-2026-0004](../research/res-2026-0004-workspace-ffi-and-feature-experiments.md))
confirmed them, so the proposal's fallback is **not** taken. Implementation is PLAN-2026-0002 P2c (TASK-030…035)
and P2d (TASK-040…044). A change to the package name, the workspace members or the default feature set needs a
new ADR.

## Context

PROP-2026-0002 R10–R13 ask for Rivet as a Cargo dependency (`rivet-runtime`, library `rivet`), Cargo features
that trim dependencies, a CLI behind a `cli` feature, and a C ABI (`librivet` as cdylib and staticlib). Three
unknowns were open: whether vhco accepts a second workspace crate, whether `#[no_mangle]` shims export correctly
from a separate crate, and whether adapter registration can be `cfg`-gated. RES-2026-0004 answered all three on
macOS:

```text
 E1 vhco + workspace ............ PASS  vhco reads only src/; ffi/ is invisible; setup_ffi.rs carries the surface
 E2 cdylib + staticlib export ... PASS  4/4 rivet_* exported, nothing else; C links shared and static
 E3 grpc feature gate ........... PASS  0 errors, tonic/prost leave the graph (197 → 182 crates)
```

## Decision

1. **Workspace layout.**

   ```text
   rivet/                          package rivet-runtime   [lib] name = "rivet"   [[bin]] rivet (required-features = ["cli"])
   ├── Cargo.toml                  [workspace] members = [".", "ffi"]; [workspace.package] version (single source)
   ├── src/{domain,features,io,infra,orchestrator}   VHCO five buckets (unchanged)
   │   └── orchestrator/setup_ffi.rs   the `ffi` surface: all FFI logic, handles, guards, vhco annotations
   └── ffi/                        package rivet-ffi       [lib] name = "rivet", crate-type = ["cdylib", "staticlib"]
       ├── src/lib.rs              #[unsafe(no_mangle)] pub extern "C" shims only (no logic, no annotations)
       ├── cbindgen.toml · include/rivet.h (checked in) · rivet.pc.in
   ```

2. **Names.** The root package is renamed `rivet` → `rivet-runtime` (crates.io `rivet` belongs to another
   project); the library keeps the name `rivet`, so `use rivet::…` is unchanged; the binary stays `rivet`. The FFI
   package is `rivet-ffi` and its library is named `rivet`, producing `librivet.{dylib,so,dll}` and
   `librivet.a`/`rivet.lib`. E2 showed no artifact collision with the dependency's `librivet-<hash>.rlib`.
3. **Where FFI code lives.** All logic in `src/orchestrator/setup_ffi.rs` (annotated surface `ffi`, contract
   entry added in TASK-004); `ffi/src/lib.rs` only converts C arguments and forwards. vhco needs no configuration.
4. **Cargo features.** `serve`, `grpc`, `quic` (including HTTP/3), `oauth`, `cli`; `default = ["serve", "grpc",
   "quic", "oauth"]` (Q-04). Gating pattern (E3): optional dependencies (`dep:` syntax), `#[cfg(feature = …)]` on
   the adapter module, its imports and its registration in `orchestrator/runtime.rs`, and a load-time
   `unsupported.feature` error (kind unsupported, exit 5, HTTP 501, `details.feature`) when a bundle uses a
   compiled-out adapter. Order of work: `grpc`, `oauth`, `serve` + `cli`, then `quic` (HTTP/3 must refuse inside
   the HTTP adapter rather than disappear).
5. **Capability report.** `rivet.capabilities` keeps its existing `features` key (the protocol table) and reports
   the compiled Cargo features under `build_features`, plus `abi_version` (contract domain `CapabilityReport`,
   TASK-004). This refines R11's wording "`rivet.capabilities` lists compiled features" without changing its
   intent.
6. **Linking details for P2d.** Set the dylib install name to `@rpath/librivet.dylib`; build release artifacts
   with one `MACOSX_DEPLOYMENT_TARGET`; generate `rivet.pc` `Libs.private` from `--print native-static-libs` per
   OS (macOS: `-framework Security -framework CoreFoundation -liconv -lc -lm`).

## Rationale

- The separate `rivet-ffi` crate keeps `cargo build` of the main crate from producing two extra 14–60 MB
  artifacts, and keeps `#[no_mangle]` symbols out of the Rust library (proposal "Alternatives": cdylib in the main
  crate rejected).
- Because vhco reads only `src/`, putting the FFI logic in `setup_ffi.rs` keeps the surface visible to
  `validate`/`sync` while the shim crate stays outside the architecture gate, exactly as the five-bucket rule
  intends (composition in `orchestrator/`).
- The E3 pattern is local: one module, one registration block, one load-time check per feature.

## Alternatives Considered

| Alternative | Why rejected |
|---|---|
| FFI shims behind an `ffi` feature in the main crate (the proposal's fallback) | Not needed: E1 and E2 passed; it would put C symbols and extra crate types in every build |
| Keep package name `rivet` | Taken on crates.io (checked 2026-09-28) |
| Report Cargo features under `features` in `rivet.capabilities` | That key already lists protocol support rows; reusing it would break existing readers |
| Gate everything with one `full` feature | Embedders could not drop gRPC or QUIC independently (R11) |

## Consequences

### Positive Consequences

- Embedders depend on `rivet = { package = "rivet-runtime", …, default-features = false, features = [...] }`.
- C hosts get a `librivet` whose exported surface is exactly the `rivet_*` functions.
- The architecture gate stays unchanged; no vhco configuration or exception is introduced.

### Negative Consequences

- `cargo install` of the CLI needs `--features cli` (documented in P4; `commands.perch` updated in TASK-034).
- Two packages share one version; `scripts/check_version.py` must read the workspace version (TASK-034, T-33).

### Risks

- `quic` gating touches the HTTP adapter (HTTP/3 upgrade path): medium risk, covered by the feature matrix (T-10).
- Linux and Windows linking are unverified until CI exists (TASK-096 blocked on a git remote).

## Implementation Impact

| Task | Change |
|---|---|
| TASK-030 | Workspace root, package rename, `required-features = ["cli"]`, `workspace.package.version` |
| TASK-031 | Facade in `src/lib.rs` |
| TASK-032 | Features and cfg gates per decision 4; `build_features` in `rivet.capabilities` |
| TASK-034 | `commands.perch`, CI matrix, `check_version.py` |
| TASK-040/041 | `setup_ffi.rs` + `ffi/` crate per decisions 1, 3 and 6 |

## Superseded Decisions

None.

## Related Documents

- [RES-2026-0004](../research/res-2026-0004-workspace-ffi-and-feature-experiments.md) — the experiments
- [PROP-2026-0002](../proposals/implemented/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md) — R10–R13, C-07–C-10
- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — P2c, P2d
- [ADR-0004](adr-0004-approve-envelopes-globals-library-ffi-highlighting.md) — design approval
- [ADR-0002](adr-0002-rust-crate-selection.md) — the dependency set being gated

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Recorded the workspace layout, names, FFI placement, feature set, gating pattern, `build_features` key and linking details after RES-2026-0004 (E1–E3 PASS; fallback not taken). |
