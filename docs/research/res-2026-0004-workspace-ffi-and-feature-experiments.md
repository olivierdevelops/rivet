---
document_id: RES-2026-0004
title: "Workspace, C ABI symbol export and Cargo feature experiments (PLAN-2026-0002 E1–E3)"
document_type: research
status: completed
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 2
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [library, registry, cli]
affected_versions:
  from: "0.1.0"
  to: "0.2.0"
applicable_environments: [development, embedded]
audience: [maintainers, implementers, reviewers]
scope: Whether vhco accepts a Cargo workspace with a second crate (E1), whether #[no_mangle] shims in a rivet-ffi crate export from both a cdylib and a staticlib and link from C (E2), and whether cfg-gating adapter registration compiles with a feature switched off (E3). No production code changed.
reason: PLAN-2026-0002 TASK-005, TASK-006 and TASK-007; PROP-2026-0002 lists these three items as "experiment needed" (C-08, C-10) and as unknowns in its complexity assessment.
related_documents: [PROP-2026-0002, PLAN-2026-0002, ADR-0004, ADR-0005, ADR-0002]
supersedes: null
superseded_by: null
tags: [rivet, workspace, ffi, c-abi, cdylib, staticlib, cargo-features, vhco]
confidentiality: internal
review_cycle: on-change
next_review_date: 2026-10-28
---

# Workspace, C ABI symbol export and Cargo feature experiments (PLAN-2026-0002 E1–E3)

> **Status:** Completed — E1, E2 and E3 PASS on macOS 26.4 (arm64); Linux not run (no host)
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 → 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** library, registry, cli (C ABI and packaging)

## Summary

PROP-2026-0002 left three build unknowns before the packaging phases (P2c, P2d) could be planned with
confidence. All three were tested in a scratch copy of the repository (never in the checkout):

```text
  experiment                    question                                          result
 ┌──────┬──────────────────────────────────────────────────────┬───────────────────────────────────────────┐
 │ E1   │ does vhco accept a Cargo workspace whose second      │ PASS  validate ✓ sync ✓ check ✓ — vhco    │
 │      │ crate (ffi/) lives outside src/?                     │ scans only src/; ffi/ is invisible to it  │
 ├──────┼──────────────────────────────────────────────────────┼───────────────────────────────────────────┤
 │ E2   │ do #[no_mangle] shims in rivet-ffi export from a     │ PASS  dylib exports exactly 4 rivet_*     │
 │      │ cdylib AND a staticlib, and does C link both ways?   │ symbols; C links shared and static        │
 ├──────┼──────────────────────────────────────────────────────┼───────────────────────────────────────────┤
 │ E3   │ does cfg-gating one adapter's registration compile   │ PASS  grpc gated: 1 warning, 0 errors;    │
 │      │ with --no-default-features and drop its crates?      │ tonic/prost gone (197 → 182 crates)       │
 └──────┴──────────────────────────────────────────────────────┴───────────────────────────────────────────┘
```

The fallback named in the proposal (shims behind an `ffi` feature inside the main crate) is **not needed**.
The decision is recorded in [ADR-0005](../decisions/adr-0005-workspace-package-and-features.md).

## Question

1. **E1 (TASK-005).** PROP-2026-0002 adds a second crate, `rivet-ffi`, in `ffi/`. Does `vhco validate`, `sync`
   and `check` still pass with the root package renamed `rivet-runtime` and a workspace declared? Where must the
   `ffi` surface's `vhco:` annotations live?
2. **E2 (TASK-006).** Do `#[no_mangle] extern "C"` functions defined in `rivet-ffi` (which depends on the main
   crate) export from both `crate-type = ["cdylib", "staticlib"]`? Does `nm` list only `rivet_*` in the shared
   library? Can a C program link the static archive on macOS, and which native libraries does it need?
3. **E3 (TASK-007).** Can one runtime adapter (gRPC) be gated by a Cargo feature, with its registration in
   `orchestrator/runtime.rs` behind `cfg`, so that `--no-default-features` compiles and the adapter's crates
   leave the dependency graph? How entangled are the other planned features?

## Method

```text
 repo HEAD 5075a1b ──cp src tests examples Cargo.* .vhco.json vhco-contract.json──▶ scratch/e1 (session scratch dir)
                                                                                    │
   E1: edit Cargo.toml → [workspace] members [".", "ffi"]; package rivet-runtime;   │  vhco 1.6.0
       [lib] name "rivet"; ffi/{Cargo.toml, src/lib.rs, include/rivet.h}            ├─ validate / sync / check / spec
   E2: cargo build -p rivet-ffi --release; nm -gU; cc demo.c (shared + static)      ├─ rustc 1.90.0, Apple clang 21
   E3: tonic/prost/prost-reflect optional under feature grpc (default on);          └─ cargo check/build/tree
       #[cfg(feature = "grpc")] on infra::grpc_adapter + runtime registration
```

Environment: macOS 26.4 (arm64), rustc 1.90.0, cargo 1.90.0, vhco 1.6.0, Apple clang 21.0.0. The scratch copy
and its `target/` were deleted afterwards; the files that matter are reproduced below.

## Results

### E1 — vhco and the workspace

The scratch `Cargo.toml` gained:

```toml
[workspace]
members = [".", "ffi"]
resolver = "3"

[workspace.package]
version = "0.1.0"

[package]
name = "rivet-runtime"          # lib name stays "rivet" → `use rivet::…` unchanged
...
[lib]
name = "rivet"
path = "src/lib.rs"
```

| Probe | Result |
|---|---|
| `vhco validate .` with `ffi/` present | `✓ no violations` |
| `vhco sync .` (contract unchanged) | `✓ code matches vhco-contract.json` |
| `vhco check .` | `✓ 1 guarantee(s) hold` (7 routes documented) |
| `// vhco:surface ffi …` placed in `ffi/src/lib.rs` | **ignored**: `vhco spec` lists 6 surfaces, no `ffi` |
| same annotation in `src/orchestrator/setup_ffi.rs` | picked up: surface `ffi` kind `ffi` appears in the model |
| contract lists `ffi`, no `setup_ffi.rs` | `missing_surface surface ffi` (a normal sync gap) |
| a `vhco:trigger ffi …` line on an existing use case | `flow_triggers_differ` until the contract flow lists it too |

```text
   repository root
   ├── src/            ◀── vhco reads ONLY this tree (five buckets + setup_<surface>.rs)
   │   └── orchestrator/setup_ffi.rs   ◀── the ffi surface's annotations must live here
   ├── ffi/            ◀── invisible to vhco: #[no_mangle] shims only, no logic, no annotations
   └── Cargo.toml      [workspace] members = [".", "ffi"]
```

Two side findings about `vhco sync` that matter for every later phase:

- `sync` compares **flow triggers, flow outputs, flow ports and the ordered `layer: ref` pairs of a flow's
  handling** as well as surfaces, use cases, todos, ports and domain fields. Renaming a `vhco:trigger` line (for
  example `--params` → `--data`) or changing a use case's `vhco:step` sequence is a contract change: the flow in
  `vhco-contract.json` must be edited by hand in the same change
  ([TRBL-2026-0004](../troubleshooting/trbl-2026-0004-vhco-sync-drift-after-renaming-triggers-or-steps.md)).
- It does **not** compare prose (`about`, todo text, handling roles).

### E2 — symbol export and linking

`ffi/Cargo.toml` used `[lib] name = "rivet"`, `crate-type = ["cdylib", "staticlib"]` and a path dependency on
`rivet-runtime`. `ffi/src/lib.rs` defined four shims (`rivet_abi_version`, `rivet_version`, `rivet_check`,
`rivet_string_free`) with `#[unsafe(no_mangle)] pub extern "C"`. `rivet_check` compiles a source through
`Runtime::builder().source(…)` inside `catch_unwind`, and turns a null or non-UTF-8 pointer into
`{"error":"validation.ffi_argument"}`.

```text
$ cargo build -p rivet-ffi --release          # 1m07s cold for the two workspace crates (deps cached)
target/release/librivet.dylib   14,339,840 bytes
target/release/librivet.a       60,652,328 bytes

$ nm -gU target/release/librivet.dylib
_rivet_abi_version
_rivet_check
_rivet_string_free
_rivet_version                                   # count: 4 — nothing else is exported

$ nm -gU target/release/librivet.a | grep ' T _rivet_'
... T _rivet_abi_version / _rivet_check / _rivet_string_free / _rivet_version
  (the archive also carries 32,850 other defined globals: Rust std and dependencies; normal for a staticlib)

$ cargo rustc -p rivet-ffi --release --crate-type staticlib -- --print native-static-libs
note: native-static-libs: -framework Security -framework CoreFoundation -liconv -lSystem -lc -lm
```

```text
$ cc demo.c -I ffi/include -L target/release -lrivet -o demo_shared && DYLD_LIBRARY_PATH=target/release ./demo_shared
abi=1 version=0.1.0
{"operations":1}
{"error":"validation.ffi_argument"}

$ cc demo.c -I ffi/include target/release/librivet.a \
     -framework Security -framework CoreFoundation -liconv -lSystem -lc -lm -o demo_static && ./demo_static
abi=1 version=0.1.0
{"operations":1}
{"error":"validation.ffi_argument"}
demo_static: 27,207,976 bytes; otool -L shows only Security, CoreFoundation, libiconv, libSystem
```

Both packages naming their library `rivet` caused **no collision**: the cdylib and staticlib are `librivet.dylib`
and `librivet.a`, while the dependency produces `librivet-<hash>.rlib`. Cargo printed no warning.

Two linking details the P2d implementation must handle (they are not blockers):

| Observation | Consequence for P2d |
|---|---|
| `otool -L demo_shared` records the dylib's install name as an **absolute path** under `target/release/deps/` | Set `-install_name @rpath/librivet.dylib` (build script or `rustflags`) before shipping the dylib |
| The static link printed `ld: warning: object file … was built for newer 'macOS' version (26.4) than being linked (26.0)` for `ring`'s assembly objects, plus `ignoring duplicate libraries: '-lSystem'` | Build release artifacts with one `MACOSX_DEPLOYMENT_TARGET` for Rust and C; drop `-lSystem` from `rivet.pc` `Libs.private` |

### E3 — gating one adapter

The scratch `Cargo.toml` declared:

```toml
[features]
default = ["grpc"]
grpc = ["dep:tonic", "dep:prost", "dep:prost-reflect"]
```

and `src/infra/mod.rs` / `src/orchestrator/runtime.rs` gained `#[cfg(feature = "grpc")]` on the adapter module,
its imports and its registration block. Without the feature, a bundle that declares a gRPC connector fails at
load:

```rust
#[cfg(not(feature = "grpc"))]
if program.connectors.iter().any(|c| c.kind == "grpc") {
    return Err(RivetError::unsupported("unsupported.feature", "this build was compiled without the `grpc` feature")
        .with_details(Value::object([("feature", Value::text("grpc"))])));
}
```

| Probe | Result |
|---|---|
| `cargo check --no-default-features` | 0 errors, 1 warning (`unused import: crate::domain::ports::GrpcDriver`, needs its own `cfg`) |
| `cargo tree -e normal` lines matching `tonic\|prost` | 7 with the default, **0** without |
| unique normal dependencies of `rivet-runtime` | 197 with the default, 182 without |
| `cargo build --no-default-features --release --lib` | 45.9 s; `librivet.rlib` 22.9 MB |
| `vhco validate .` / `vhco sync .` on the gated code | both green (cfg attributes do not affect the model) |

Entanglement survey for the other planned features (files that name each crate):

```text
 feature   crates                     files that use them                          gating effort
 grpc      tonic prost prost-reflect  infra/grpc_adapter.rs (+ runtime.rs wiring)   LOW  (done in E3)
 quic      quinn h3 h3-quinn          infra/quic_adapter.rs, infra/h3_client.rs,    MEDIUM: HTTP/3 lives inside
                                      infra/http_adapter.rs, features/transports/   http_adapter (h3 upgrade path)
                                      exchange_http.rs, domain/transports.rs         and must refuse, not vanish
 serve     axum                       infra/serve_listener.rs, orchestrator/         MEDIUM: whole serve stack +
                                      setup_{serve,http,poll,ws,mcp}.rs              the CLI `serve` command
 oauth     keyring-core + stores      infra/oauth_adapter.rs (+ runtime wiring)      LOW–MEDIUM (like grpc; the
                                                                                     credential provider stays)
 cli       clap                       io/cli/mod.rs, orchestrator/setup_cli.rs       LOW: bin required-features;
                                                                                     `serve` needs serve
```

## Recommendation

1. Adopt the workspace exactly as proposed: root package `rivet-runtime` with `[lib] name = "rivet"`, member
   `ffi/` (package `rivet-ffi`, `[lib] name = "rivet"`, `crate-type = ["cdylib", "staticlib"]`). No vhco
   configuration is needed.
2. Keep every piece of FFI logic in `src/orchestrator/setup_ffi.rs` (the annotated `ffi` surface). `ffi/src/lib.rs`
   holds only `#[no_mangle]` shims, which vhco does not see.
3. Gate adapters one feature at a time with the E3 pattern: optional dependencies, `cfg` on the adapter module
   and its registration, and a load-time `unsupported.feature` (exit 5, HTTP 501, `details.feature`) when a bundle
   uses a compiled-out adapter. Do `grpc` and `oauth` first, then `serve` and `cli`, then `quic` (HTTP/3 must
   refuse inside the HTTP adapter).
4. `rivet.capabilities` already has a `features` key (the protocol table). Report compiled Cargo features under a
   new key; the contract (TASK-004) names it `build_features`.

## Limitations and open items

- macOS only. Linux (`.so`, `Libs.private` with `-lpthread -ldl -lm -lgcc_s` or similar) and Windows (`.dll`,
  `rivet.lib`) were not built: no host and no CI remote. P2d captures `native-static-libs` per OS when CI exists.
- E2 used 4 shims, not the full ABI 1 surface; the pull-handle (`rivet_call_*`) design is untested here.
- E3 gated one feature. The `quic` feature is the riskiest because HTTP/3 selection lives in the HTTP adapter.
- Build-time numbers are single cold runs on one machine; they are observations, not claims.

## Conclusion

All three unknowns are resolved in favour of the proposed design. vhco accepts the workspace (it reads only
`src/`), a separate `rivet-ffi` crate exports exactly the `rivet_*` symbols from its cdylib and links from C both
dynamically and statically on macOS, and feature-gating an adapter is a local change that removes its crates
from the graph. The fallback in PROP-2026-0002's risk table is not needed.

## Related Documents

- [PROP-2026-0002](../proposals/approved/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md) — C-08, C-10, R10–R13, complexity unknowns
- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) — TASK-005…TASK-009, P2c, P2d
- [ADR-0005](../decisions/adr-0005-workspace-package-and-features.md) — the decision this research supports
- [ADR-0004](../decisions/adr-0004-approve-envelopes-globals-library-ffi-highlighting.md) — design approval
- [ADR-0002](../decisions/adr-0002-rust-crate-selection.md) — the crates being gated

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 2 | 2026-09-28 | Claude | Corrected the E1 side finding: `vhco sync` also compares flow handling (`layer: ref` pairs); linked TRBL-2026-0004. |
| 1 | 2026-09-28 | Claude | E1 (vhco + workspace), E2 (cdylib/staticlib symbol export, C shared and static link) and E3 (grpc feature gate) run in a scratch copy; all PASS on macOS; entanglement survey for quic, serve, oauth and cli. |
