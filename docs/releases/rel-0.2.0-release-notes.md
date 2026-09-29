---
document_id: REL-0.2.0
title: "Rivet 0.2.0 release notes"
document_type: release
status: completed
created_date: 2026-09-30
last_updated: 2026-09-30
document_revision: 2
authors: [Claude]
owner: Project maintainer
version: "0.2.0"
git_tag: v0.2.0
git_commit: 21bb2e94dfa476856495997c4f510f5537c65225
release_date: 2026-09-30
source_branch: main
previous_version: "0.1.0"
previous_tag: v0.1.0
release:
  version: "0.2.0"
  tag: v0.2.0
  commit: 21bb2e94dfa476856495997c4f510f5537c65225
  date: 2026-09-30
  branch: main
systems: [Rivet]
components: [language, registry, execution, audit, policy, serve, cli, http, ws, poll, mcp, library, ffi]
affected_versions:
  from: "0.2.0"
  to: null
confidentiality: internal
scope: Second release of Rivet (PLAN-2026-0002, PROP-2026-0002 revision 3) — envelopes, pretty output, global constants, file modules, the rivet-runtime library facade and Cargo features, the C ABI and syntax highlighting.
reason: DOCUMENTATION §33 — the release document is the traceability record of the release.
related_documents: [PLAN-2026-0002, PROP-2026-0002, ADR-0004, ADR-0005, RPT-2026-0015, DEMO-2026-0020, MIG-2026-0001, REL-0.1.0]
supersedes: null
superseded_by: null
tags: [rivet, release, v0.2.0]
---

# Release 0.2.0

> **Status:** Completed
> **Created:** 2026-09-30
> **Last Updated:** 2026-09-30
> **Affected Versions:** 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** language, registry, execution, audit, policy, serve, cli, http, ws, poll, mcp, library, ffi

## Release Identity

```text
Release Version:          0.2.0
Git Tag:                  v0.2.0
Git Commit:               21bb2e94dfa476856495997c4f510f5537c65225
Release Date:             2026-09-30
Source Branch:            main
Previous Version:         0.1.0
Previous Tag:             v0.1.0
Previous Release Commit:  f69b5b911f174b0198adb89c8305c8cce268fc11
Repository:               https://github.com/olivierdevelops/rivet (origin)
Supported platforms:      macOS, Linux (Windows unsupported: INC-2026-0011)
Build:                    cargo build --release --workspace --all-features (Rust 1.90.0)
Artifacts:                dist/v0.2.0/ — rivet binary, librivet.dylib + librivet.a, rivet.h, rivet.pc, rivet-0.2.0.vsix, SHA256SUMS
```

## Plan

[PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) implements
[PROP-2026-0002](../proposals/implemented/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md) revision 3,
approved in [ADR-0004](../decisions/adr-0004-approve-envelopes-globals-library-ffi-highlighting.md). The packaging
decision is [ADR-0005](../decisions/adr-0005-workspace-package-and-features.md).

```text
 PROP-2026-0002 r3 ─► ADR-0004 ─► PLAN-2026-0002 ─► 80 commits since v0.1.0 ─► TEST-2026-0001…0053 ─► RPT-2026-0015
                                                                                  └─► DEMO-2026-0020 ─► REL-0.2.0
```

## Project Standards Baseline

| Standards Index | Revision | Applicable Rules | Proposal Validation |
|---|---|---|---|
| [`docs/standards/index.md`](../standards/index.md) | current | AGENTS.md, DOCUMENTATION.md, STD-2026-0001, VHCO five-folder architecture | PASS |

## User Requirements

| Requirement | Source | Released Update | Result |
|---|---|---|---|
| PROP-2026-0002 R1 | UQ-03/05: one output shape | U-01 | PASS |
| PROP-2026-0002 R2 | UQ-05: status and error semantics | U-02 | PASS |
| PROP-2026-0002 R3 | UQ-03: every JSON output | U-03 | PASS |
| PROP-2026-0002 R4 | UQ-06: one input shape | U-04 | PASS |
| PROP-2026-0002 R5 | Compatibility for 0.1.0 clients | U-05 | PASS |
| PROP-2026-0002 R6 | UQ-04: readable JSON | U-06 | PASS |
| PROP-2026-0002 R7 | UQ-07: reuse values | U-07 | PASS |
| PROP-2026-0002 R8 | UQ-07: safe globals | U-08 | PASS |
| PROP-2026-0002 R9 | Manifest precision | U-09 | PASS |
| PROP-2026-0002 R10 | UQ-02: Cargo dependency | U-10 | PASS |
| PROP-2026-0002 R11 | UQ-02: lean builds | U-11 | PASS |
| PROP-2026-0002 R12 | UQ-02: publication path | U-12 | PASS |
| PROP-2026-0002 R13 | UQ-08: C ABI | U-13 | PASS |
| PROP-2026-0002 R14 | FFI safety | U-14 | PASS |
| PROP-2026-0002 R15 | FFI streams | U-15 | PASS |
| PROP-2026-0002 R16 | UQ-01: editors | U-16 | PASS |
| PROP-2026-0002 R17 | UQ-01: terminal/HTML | U-17 | PASS |
| PROP-2026-0002 R18 | DOCUMENTATION §§29–31 | U-18 | PASS |
| PROP-2026-0002 R19 | UQ-09: files as modules | U-19 | PASS |
| PROP-2026-0002 R20 | UQ-09: namespaced by alias | U-20 | PASS |
| PROP-2026-0002 R21 | Safe imports | U-21 | PASS |
| PROP-2026-0002 R22 | UQ-09: host object (Rust) | U-22 | PASS |
| PROP-2026-0002 R23 | UQ-09: host object (C/Python) | U-23 | PASS |
| PROP-2026-0002 R24 | UQ-09: loader's policy only | U-24 | PASS |

## Breaking Changes

0.2.0 changes the wire format on every surface. Migrate with
[MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md), which covers before/after per surface,
a `jq` mapping, the deprecation timeline and rollback (pin `v0.1.0`).

```text
 0.1.0                                   0.2.0
 {request_id, trace_id, result, …}    ─▶  {request_id, trace_id, operation, type, status, data, error, effects, data_count}
 {id, params} · --params                ─▶  {operation, data} · --data · --input   (id/params deprecated, removed in 0.3.0)
 package rivet, internal modules public ─▶  package rivet-runtime (lib rivet), facade; internals under rivet::internal
 binary always built                    ─▶  binary needs the `cli` Cargo feature
 policy explain --json denial on stdout ─▶  error envelope on stderr, exit 3
```

## Added

- **One response envelope and one input envelope on every surface** (CLI, HTTP/SSE, polling, WebSocket, MCP, Rust
  library, C ABI), with JSON Schemas; `--data`, `--input FILE|-`; opt-in pretty JSON (`--pretty`, `?pretty=true`).
- **`global NAME = EXPR`** constants, evaluated once at load and read-only. Targets built from globals are exact in
  `rivet io` and `policy generate`.
- **File modules:** `import "PATH" as ALIAS [public]` in `.rivet` files, `Runtime::load` / `load_as` in Rust,
  and `rivet_load` in C. Operations are namespaced by alias, and one loader policy covers every module.
- **Rust library:** package `rivet-runtime` with a stable `rivet::` facade, and Cargo features `serve`, `grpc`,
  `quic`, `oauth` and `cli`. A compiled-out feature fails with `unsupported.feature`.
- **C ABI:** `librivet`, shared and static, with `rivet.h` and `rivet.pc`. It exports 18 `rivet_*` functions,
  including pull-style call handles and module handles, and returns JSON strings in both directions. A Python
  ctypes example is included.
- **Syntax highlighting:** a TextMate grammar generated from the language keywords, a local VS Code extension
  (`.vsix`), `rivet highlight --format ansi|html|json`, and `rivet::highlight`.
- **Other:** `xs.0` list indexing, and `rivet.capabilities` now reports the compiled features (`build_features`)
  and `abi_version`.

## Changed

- Wire format and package layout (see [Breaking Changes](#breaking-changes)).
- `policy explain` takes `--data` (alias `--params`), passes params through calls into modules, and reports a
  denial under `--json` as an error envelope.
- Supported platforms are now macOS and Linux, with CI green on both. Windows is not supported.

## Fixed

These were found and fixed during the release work:
- [INC-2026-0009](../incidents/resolved/inc-2026-0009-numeric-index-paths-do-not-parse.md): numeric index paths
  did not parse.
- [INC-2026-0010](../incidents/resolved/inc-2026-0010-ffi-rlib-output-collision.md): an output-filename collision
  between the FFI and runtime libraries.
- [INC-2026-0012](../incidents/resolved/inc-2026-0012-documentation-and-demo-verification-defects.md): 19 defects
  found by verifying the documentation and demos. These covered envelope consistency, WebSocket refusals, MCP
  error naming, highlight on unclosed blocks, inline `open true`, the remote CLI timeout, `policy explain` and
  module-load details.

## Removed

- The 0.1.0 limitation "no import form"; file modules replace it.
- The Windows CI job, removed by maintainer decision (see Known Issues).

## Released Updates and Verification

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-01 | UQ-03/05 → R1 | Every outcome on every surface is a `ResponseEnvelope`: `request_id`, `trace_id`, `operation`, `type`, `status`, `data`, `error`, `effects`, `data_count` (+ `seq` on stream records, `ref` on WS frames) | [01-catalog](../demos/01-catalog/README.md) steps 1, 4–8; [10-grpc](../demos/10-grpc/README.md) steps 4–8; [12-library](../demos/12-library/README.md) step 2; [15-ffi](../demos/15-ffi/README.md) step 2 | Same nine keys in the same order on CLI, REST, SSE, polling, WS, MCP `structuredContent`, library and C | Recorded (per-surface examples below, 166a98b; folders at 8031baa); `conformance_envelope` (T-01); changed steps re-run at 7c25175 after INC-2026-0012 (01 steps 7–8, 10 steps 7–8, 12 step 2, per-surface WS/MCP above) |
| U-02 | UQ-05 → R2 | `status` ok / error / cancelled / accepted; `type` result / data; `effects` at the top level | [01-catalog](../demos/01-catalog/README.md) steps 1, 4, 6; [04-streaming](../demos/04-streaming/README.md) steps 6–7 | `status:error` + `data:null` on failures; `accepted` (202) for a polling open; `cancelled` on Ctrl-C (exit 130) | Recorded; `conformance_envelope` (T-01, T-04) |
| U-03 | UQ-03 → R3 | Built-ins, GET routes and every CLI `--json` output are envelopes whose `data` is the payload | [01-catalog](../demos/01-catalog/README.md) steps 1, 2, 4; [11-sandbox](../demos/11-sandbox/README.md) steps 3–5, 7, 8 | `rivet.describe`, `rivet.outputs`, `rivet.io`, `rivet.policy.*`, `rivet.trace.show`, `rivet.check` envelopes | Recorded; `conformance_envelope` (T-03); 11-sandbox step 3 re-run at 7c25175 (`policy explain --json` denial = error envelope on stderr, exit 3) |
| U-04 | UQ-06 → R4 | One `InputEnvelope {operation, data, …}`; CLI `--data` and `--input FILE\|-` | [01-catalog](../demos/01-catalog/README.md) steps 1, 4, 6, 7; [12-library](../demos/12-library/README.md) step 2; [15-ffi](../demos/15-ffi/README.md) step 2 | The same body accepted by `--input -`, REST, polling, WS, `Runtime::call` and `rivet_request` | Recorded; `conformance_envelope` (T-02); 01 step 7 and 12 step 2 re-run at 7c25175 |
| U-05 | UQ-06 (0.1.0 clients) → R5 | `id`/`params` and `--params` still accepted with a deprecation signal; mixing a key with its alias refused | [01-catalog](../demos/01-catalog/README.md) steps 1, 3, 4 | `warning[deprecated.params]`; `"deprecation":true`; `validation.input_envelope` 422 / exit 2 | Recorded; `conformance_envelope` (T-15) |
| U-06 | UQ-04 → R6 | `--pretty`, `?pretty=true`, `to_json_pretty()`, FFI `"pretty"`; refused with NDJSON and SSE | [01-catalog](../demos/01-catalog/README.md) steps 1, 4, 5; [12-library](../demos/12-library/README.md) step 2; SSE example below | Indented envelope; `validation.usage` exit 2 (`--pretty --stream`); `validation.pretty_stream` HTTP 400 | Recorded; `conformance_envelope` (T-05) |
| U-07 | UQ-07 → R7 | `global NAME = EXPR`: load-time, read-only constants shared by every operation of a file | [14-globals](../demos/14-globals/README.md) step 2 | `settings.show` returns the seven computed globals | Recorded; `conformance_globals` (T-06) |
| U-08 | UQ-07 → R8 | `syntax.global` and `check.global_not_constant`, `_forward_ref`, `_assign`, `_duplicate`, `_shadow` with spans | [14-globals](../demos/14-globals/README.md) step 6 | Six diagnostics, each exit 2 | Recorded; `conformance_globals` (T-07) |
| U-09 | UQ-07 → R9 | Globals substituted in the manifest and call graph; literal-and-global targets are `exact`; exact grants from `policy generate` | [14-globals](../demos/14-globals/README.md) steps 3–5 | `…/users/index.json?limit=50` `exact`; draft with 2 exact grants; one-line retarget | Recorded; `conformance_globals` (T-08). Deviation: a target with a param stays `param_dependent` (Known Caveats) |
| U-10 | UQ-02 → R10 | Package `rivet-runtime`, library `rivet`, public facade only (`rivet::{Runtime, Policy, Value, InputEnvelope, …}`, `rivet::types`) | [12-library](../demos/12-library/README.md) step 2 | A scratch crate with `package = "rivet-runtime"` compiles against the facade and exits 0 | Recorded; `examples/embed.rs`; `conformance_library` |
| U-11 | UQ-02 → R11 | Cargo features `serve`, `grpc`, `quic`, `oauth`, `cli`; a compiled-out adapter is `unsupported.feature` | [12-library](../demos/12-library/README.md) step 2; [01-catalog](../demos/01-catalog/README.md) step 1 (`rivet.capabilities`) | `build_features` `["serve","grpc","quic","oauth"]` (library) / plus `cli` (binary) | Success recorded. The failure half (`unsupported.feature`, exit 5, HTTP 501) is evidenced by `conformance_features` (T-10) only: no lean binary is built in the demos |
| U-12 | UQ-02 → R12 | Git dependency on tag `v0.2.0`; crates.io publication prepared and gated (G-PUB) | `cargo package --list -p rivet-runtime --allow-dirty`; after P5, `rivet-runtime = { git = "…/rivet", tag = "v0.2.0" }` | 177 files: `src/**`, `rivet.capy`, `editors/keywords.json`, the two examples; no `tests/`, `docs/`, `ffi/` | `cargo package --list` 177 files (166a98b). **Git-tag dependency verified** 2026-09-30: a fresh crate with `rivet = { package = "rivet-runtime", git = "https://github.com/olivierdevelops/rivet", tag = "v0.2.0", default-features = false }` resolved `git+…?tag=v0.2.0#21bb2e94…`, built, and `Runtime::call` returned `data: 5`. crates.io DEFERRED (G-PUB) |
| U-13 | UQ-08 → R13 | `librivet` as `cdylib` + `staticlib`, `include/rivet.h`, `rivet.pc`, `rivet_abi_version` | [15-ffi](../demos/15-ffi/README.md) Setup, step 1 | 18 `rivet_*` symbols; shared and static links run | Recorded; `conformance_ffi` (T-11); Linux in CI run 36483001760 |
| U-14 | UQ-08 → R14 | Null, UTF-8, JSON, panic and double-free guards; ownership and thread-safety rules | [15-ffi](../demos/15-ffi/README.md) step 5 | `validation.ffi_argument`, `validation.input_envelope`, `not_found.source` envelopes; second free returns 1 | Recorded; `conformance_ffi` (T-12) |
| U-15 | UQ-08 → R15 | Pull call handle: `rivet_call_start`, `_next`, `_send`, `_finish_input`, `_cancel`, `_free` | [15-ffi](../demos/15-ffi/README.md) step 2 | Items then one terminal record; echoed input; `cancelled` | Recorded; `conformance_ffi` (T-11) |
| U-16 | UQ-01 → R16 | Generated TextMate grammar; VS Code extension packaged as `.vsix` with Python only; drift check | [16-editor](../demos/16-editor/README.md) steps 1, 2, 5 | `rivet.rivet@0.1.0` installed in a scratch profile (0.2.0 after the bump); hand edit caught (exit 1) | Recorded (VS Code 1.108.1); `conformance_highlight` (T-13) |
| U-17 | UQ-01 → R17 | `rivet highlight` (ansi, html, json), `rivet::highlight::tokens`, `rivet_highlight` from parser spans | [16-editor](../demos/16-editor/README.md) steps 4, 6; [14-globals](../demos/14-globals/README.md) step 7 | 12 classes in 65 tokens; broken file: 11 tokens + `syntax.expression`, exit 2 | Recorded; `conformance_highlight` (T-14); 16-editor step 6 re-run at 7c25175 (unclosed block keeps header tokens) |
| U-18 | DOCUMENTATION §§29–31 → R18 | Every document in the plan's Documentation and Demo Checklist; demos executed and re-verified | `python3 scripts/check_docs.py`; `vhco docs check .` | 0 problems in `docs/demos`; 0 errors | Recorded for `docs/demos` at this commit (Verification Record). The rest of the P4 checklist is owned by other tasks and closes at P4 exit |
| U-19 | UQ-09 → R19 | `import "PATH" as ALIAS [public]`, relative to the importing file, root-confined; bootstrap reads name every file | [17-modules](../demos/17-modules/README.md) steps 1, 4; [08-udp](../demos/08-udp/README.md) step 5; [11-sandbox](../demos/11-sandbox/README.md) step 4 | `ok: 7 operations`; bootstrap `./app.rivet`, `./users.rivet`, `./billing.rivet` | Recorded; `conformance_modules` (T-16) |
| U-20 | UQ-09 → R20 | `ALIAS.ID` namespaces; internal by default, `public` to expose; transitive; a file reached twice compiles once | [17-modules](../demos/17-modules/README.md) steps 1–3 | Four IDs on CLI, REST and MCP; `users.get` 404 / exit 4 | Recorded; `conformance_modules` (T-16) |
| U-21 | UQ-09 → R21 | `syntax.import`, `not_found.import`, `permission.import_outside_root`, `check.import_cycle`, `check.import_duplicate`, `check.import_collision`, `limit.imports` | [17-modules](../demos/17-modules/README.md) step 6 | Exits 2, 4, 3, 2, 2, 2 at the `import` line | Recorded except `limit.imports` (`conformance_modules`, T-17, only); 17-modules step 6 re-run at 7c25175 |
| U-22 | UQ-09 → R22 | `Runtime::builder()` without an entry file; `rt.load` / `load_as` → `Module`; catalog snapshot swap | [17-modules](../demos/17-modules/README.md) step 7 (`cargo run --release --example modules`) | `users.get`, `billing.invoice`, `users.list` envelopes; duplicate alias refused | Recorded; `conformance_modules` (T-18); 17-modules step 7 re-run at 7c25175 |
| U-23 | UQ-09 → R23 | `rivet_load`, `rivet_module_*`; Python wrapper module object | [15-ffi](../demos/15-ffi/README.md) steps 3–4 | Same envelopes from C and Python; `check.import_duplicate` on reload | Recorded (166a98b); `conformance_ffi` (T-19); 15-ffi step 3 re-run at 7c25175 (`rivet_load` refusal carries IDs) |
| U-24 | UQ-09 → R24 | The loader's policy only; a module's `policy.json` ignored with a warning; `io`, `graph`, `policy explain`, `policy generate` cover modules | [17-modules](../demos/17-modules/README.md) steps 4–6 | `2 allowed` for sites in two modules; draft with 2 grants; `warning[check.module_policy_ignored]` | Recorded; `conformance_modules` (T-20); 17-modules step 6 (`request`/`serve` print the warning) and `policy explain --data` re-run at 7c25175 |

## Tests

| Test | Requirement | Result |
|---|---|---|
| [TEST-2026-0001](../testing/test-2026-0001-language.md) | PLAN-2026-0001 R1, PLAN-2026-0001 R2, PLAN-2026-0002 R1 | PASS |
| [TEST-2026-0002](../testing/test-2026-0002-surfaces.md) | PLAN-2026-0001 R8, PLAN-2026-0001 R9, PLAN-2026-0002 R1 | PASS |
| [TEST-2026-0003](../testing/test-2026-0003-streams.md) | PLAN-2026-0001 R3, PLAN-2026-0001 R4, PLAN-2026-0001 R9, PLAN-2026-0002 R1 | PASS |
| [TEST-2026-0004](../testing/test-2026-0004-files.md) | PLAN-2026-0001 R5, PLAN-2026-0001 R11, PLAN-2026-0002 R1 | PASS |
| [TEST-2026-0005](../testing/test-2026-0005-resources.md) | PLAN-2026-0001 R3, PLAN-2026-0001 R12, PLAN-2026-0001 R13, PLAN-2026-0002 R1 | PASS |
| [TEST-2026-0006](../testing/test-2026-0006-mcp.md) | PLAN-2026-0001 R7, PLAN-2026-0002 R1 | PASS |
| [TEST-2026-0007](../testing/test-2026-0007-dag.md) | PLAN-2026-0001 R10, PLAN-2026-0002 R1 | PASS |
| [TEST-2026-0008](../testing/test-2026-0008-sandbox.md) | PLAN-2026-0001 R11, PLAN-2026-0001 R13, PLAN-2026-0002 R1 | PASS |
| [TEST-2026-0009](../testing/test-2026-0009-audit.md) | PLAN-2026-0001 R6, PLAN-2026-0001 R13, PLAN-2026-0002 R1 | PASS |
| [TEST-2026-0010](../testing/test-2026-0010-library.md) | PLAN-2026-0001 R1, PLAN-2026-0001 R3, PLAN-2026-0001 R9, PLAN-2026-0002 R1 | PASS |
| [TEST-2026-0011](../testing/test-2026-0011-oauth.md) | PLAN-2026-0001 R16, PLAN-2026-0002 R1 | PASS |
| [TEST-2026-0012](../testing/test-2026-0012-udp.md) | PLAN-2026-0001 R15, PLAN-2026-0002 R1 | PASS |
| [TEST-2026-0013](../testing/test-2026-0013-quic.md) | PLAN-2026-0001 R17, PLAN-2026-0002 R1 | PASS |
| [TEST-2026-0014](../testing/test-2026-0014-http3.md) | PLAN-2026-0001 R18, PLAN-2026-0002 R1 | PASS |
| [TEST-2026-0015](../testing/test-2026-0015-auth-transport-policy.md) | PLAN-2026-0001 R15, PLAN-2026-0001 R18, PLAN-2026-0002 R1 | PASS |
| [TEST-2026-0016](../testing/test-2026-0016-operation-catalog.md) | PLAN-2026-0001 R20, PLAN-2026-0002 R1 | PASS |
| [TEST-2026-0017](../testing/test-2026-0017-grpc.md) | PLAN-2026-0001 R19, PLAN-2026-0002 R1 | PASS |
| [TEST-2026-0018](../testing/test-2026-0018-mcp-catalog.md) | PLAN-2026-0001 R21, PLAN-2026-0002 R1 | PASS |
| [TEST-2026-0019](../testing/test-2026-0019-sessions.md) | PLAN-2026-0001 R22, PLAN-2026-0002 R1 | PASS |
| [TEST-2026-0020](../testing/test-2026-0020-outputs.md) | PLAN-2026-0001 R23, PLAN-2026-0002 R1 | PASS |
| [TEST-2026-0021](../testing/test-2026-0021-policy-file.md) | PLAN-2026-0001 R24, PLAN-2026-0002 R1 | PASS |
| [TEST-2026-0022](../testing/test-2026-0022-serve.md) | PLAN-2026-0001 R25, PLAN-2026-0002 R1 | PASS |
| [TEST-2026-0023](../testing/test-2026-0023-syntax.md) | PLAN-2026-0001 R1, PLAN-2026-0001 R2, PLAN-2026-0002 R1 | PASS |
| [TEST-2026-0024](../testing/test-2026-0024-errors-limits-dag.md) | PLAN-2026-0001 R4, PLAN-2026-0001 R10, PLAN-2026-0002 R1 | PASS |
| [TEST-2026-0025](../testing/test-2026-0025-io-manifest.md) | PLAN-2026-0001 R6, PLAN-2026-0001 R26, PLAN-2026-0002 R1 | PASS |
| [TEST-2026-0026](../testing/test-2026-0026-policy-generate.md) | PLAN-2026-0001 R26, PLAN-2026-0002 R1 | PASS |
| [TEST-2026-0027](../testing/test-2026-0027-build-static.md) | PLAN-2026-0001 R1, PLAN-2026-0002 R10, PLAN-2026-0002 R11, PLAN-2026-0002 R13 | PASS |
| [TEST-2026-0028](../testing/test-2026-0028-architecture.md) | PLAN-2026-0001 R14, PLAN-2026-0002 R1, PLAN-2026-0002 R24 | PASS |
| [TEST-2026-0029](../testing/test-2026-0029-samples.md) | PLAN-2026-0001 R2, PLAN-2026-0001 R14, PLAN-2026-0002 R7, PLAN-2026-0002 R16, PLAN-2026-0002 R19 | PASS |
| [TEST-2026-0030](../testing/test-2026-0030-demos-e2e.md) | PLAN-2026-0001 R1, PLAN-2026-0001 R26, PLAN-2026-0002 R18 | PASS |
| [TEST-2026-0031](../testing/test-2026-0031-documentation.md) | PLAN-2026-0001 R14, PLAN-2026-0002 R18 | PASS |
| [TEST-2026-0032](../testing/test-2026-0032-traceability.md) | PLAN-2026-0001 R1, PLAN-2026-0001 R26, PLAN-2026-0002 R1…R24 | PASS |
| [TEST-2026-0033](../testing/test-2026-0033-release.md) | PLAN-2026-0001 R14, PLAN-2026-0002 R12 | PASS |
| [TEST-2026-0034](../testing/test-2026-0034-envelope-schema.md) | PLAN-2026-0002 R1, PLAN-2026-0002 R2 | PASS |
| [TEST-2026-0035](../testing/test-2026-0035-input-envelope.md) | PLAN-2026-0002 R4 | PASS |
| [TEST-2026-0036](../testing/test-2026-0036-builtin-json-envelopes.md) | PLAN-2026-0002 R3 | PASS |
| [TEST-2026-0037](../testing/test-2026-0037-stream-records.md) | PLAN-2026-0002 R1 | PASS |
| [TEST-2026-0038](../testing/test-2026-0038-pretty-json.md) | PLAN-2026-0002 R6 | PASS |
| [TEST-2026-0039](../testing/test-2026-0039-globals-values.md) | PLAN-2026-0002 R7 | PASS |
| [TEST-2026-0040](../testing/test-2026-0040-globals-diagnostics.md) | PLAN-2026-0002 R8 | PASS |
| [TEST-2026-0041](../testing/test-2026-0041-globals-manifest.md) | PLAN-2026-0002 R9 | PASS |
| [TEST-2026-0042](../testing/test-2026-0042-library-facade.md) | PLAN-2026-0002 R10 | PASS |
| [TEST-2026-0043](../testing/test-2026-0043-cargo-features.md) | PLAN-2026-0002 R11, PLAN-2026-0002 R12 | PASS |
| [TEST-2026-0044](../testing/test-2026-0044-c-abi.md) | PLAN-2026-0002 R13, PLAN-2026-0002 R15 | PASS |
| [TEST-2026-0045](../testing/test-2026-0045-ffi-safety.md) | PLAN-2026-0002 R14 | PASS |
| [TEST-2026-0046](../testing/test-2026-0046-editor-grammar.md) | PLAN-2026-0002 R16 | PASS |
| [TEST-2026-0047](../testing/test-2026-0047-highlight-cli.md) | PLAN-2026-0002 R17 | PASS |
| [TEST-2026-0048](../testing/test-2026-0048-legacy-input.md) | PLAN-2026-0002 R5 | PASS |
| [TEST-2026-0049](../testing/test-2026-0049-file-modules.md) | PLAN-2026-0002 R19, PLAN-2026-0002 R20 | PASS |
| [TEST-2026-0050](../testing/test-2026-0050-import-errors.md) | PLAN-2026-0002 R21 | PASS |
| [TEST-2026-0051](../testing/test-2026-0051-host-module-load.md) | PLAN-2026-0002 R22 | PASS |
| [TEST-2026-0052](../testing/test-2026-0052-ffi-modules.md) | PLAN-2026-0002 R23 | PASS |
| [TEST-2026-0053](../testing/test-2026-0053-module-policy.md) | PLAN-2026-0002 R24 | PASS |

## Validation

[RPT-2026-0015](../reports/rpt-2026-0015-validation-of-plan-2026-0002.md) records 24 requirements PASS,
0 PARTIAL and 0 FAIL. Requirements not yet PASS: none. CI evidence comes from GitHub Actions on
ubuntu-latest and macos-latest (tests, `cargo deny`, and a feature matrix of none, cli, serve, grpc, quic and
oauth).

## Measured Results

NOT APPLICABLE: PROP-2026-0002 makes no performance claim. It states a size cost only: about 60–80 bytes more per
response envelope.

## Incidents

| Incident | Severity | Title | Status |
|---|---|---|---|
| [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md) | S3 | Windows port failures catalogued when Windows was dropped from CI | active |
| [INC-2026-0009](../incidents/resolved/inc-2026-0009-numeric-index-paths-do-not-parse.md) | S4 | Numeric index paths (`xs.0`) do not parse and report a misleading assign_map error | resolved |
| [INC-2026-0010](../incidents/resolved/inc-2026-0010-ffi-rlib-output-collision.md) | S3 | rivet-ffi and rivet-runtime both produced librivet.rlib (output filename collision) | resolved |
| [INC-2026-0012](../incidents/resolved/inc-2026-0012-documentation-and-demo-verification-defects.md) | S3 | Defects found by v0.2.0 documentation and demo verification | resolved |
| [INC-2026-0013](../incidents/resolved/inc-2026-0013-input-jsonl-waits-for-stdin-eof.md) | S3 | rivet request --input-jsonl - waited for stdin EOF after the request ended | resolved |

## Troubleshooting

- [TRBL-2026-0004](../troubleshooting/trbl-2026-0004-vhco-sync-drift-after-renaming-triggers-or-steps.md): vhco sync reports flow drift after renaming a trigger or a step
- [TRBL-2026-0005](../troubleshooting/trbl-2026-0005-contract-written-ahead-of-code-drifts-on-ports-and-step-order.md): A contract use case written ahead of its code drifts on port parameter names, step order and surface calls
- [TRBL-2026-0006](../troubleshooting/trbl-2026-0006-cargo-test-leaves-cdylib-and-staticlib-in-deps.md): cargo test leaves librivet (cdylib and staticlib) in target/<profile>/deps, not target/<profile>
- [TRBL-2026-0007](../troubleshooting/trbl-2026-0007-reading-ci-failures-through-annotations.md): Reading GitHub Actions failures without admin rights (check-run annotations)

## Release Verification Guide and Demo

[DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md) has U-01…U-24 and per-surface success
and failure examples. It relies on demos 01–17 in [docs/demos](../demos/index.md), including the new 14-globals,
15-ffi, 16-editor and 17-modules.

## Manual

MAN-2026-0001…0010 describe 0.2.0 ([manuals index](../manuals/index.md)). MAN-2026-0009 (C ABI and FFI) and
MAN-2026-0010 (editor support and highlighting) are new.

## System

SYS-2026-0001…0011, ARCH-2026-0001, API-2026-0001…0007 (0006 and 0007 are new), SEC-2026-0001, OPS-2026-0001,
RUN-2026-0001/0002 and ONB-2026-0001 are updated for 0.2.0. MIG-2026-0001 is new.

## Documentation Impact

| Artifact | Decision | Updated Document or Reason |
|---|---|---|
| Release verification guide / demo | UPDATED | DEMO-2026-0020; demos 01–17 (14–17 new) |
| README.md | UPDATED | Current release 0.2.0, the 0.1.0 → 0.2.0 overview and install instructions |
| System documentation | UPDATED | SYS-2026-0001…0011 (0010 and 0011 new) |
| Architecture documentation | UPDATED | ARCH-2026-0001: workspace, facade, envelope edge, features, modules, ffi |
| API and CLI reference | UPDATED | API-2026-0001…0007, MAN-2026-0004 |
| Manual | UPDATED | MAN-2026-0001…0010 |

## Source Changes

| Source | Change | Reason |
|---|---|---|
| `src/domain/envelope.rs`, `features/serve/parse_input.rs`, every `orchestrator/setup_*.rs` | Added / modified | Envelopes, input and pretty output (R1–R6) |
| `src/infra/rivet.capy`, `features/language/{compile_globals,resolve_imports,highlight_source}.rs`, `features/registry/load_module.rs` | Added / modified | Globals, modules, highlighting (R7–R9, R16–R24) |
| `Cargo.toml` (workspace, features), `src/lib.rs` + `src/internal.rs` (facade) | Modified | Library as a dependency (R10–R12) |
| `ffi/` (rivet-ffi), `src/orchestrator/setup_ffi.rs`, `examples/{c,python}` | Added | C ABI (R13–R15, R23) |
| `editors/` (grammar generator, VS Code extension, packager) | Added | Highlighting (R16) |
| `tests/conformance_{envelope,globals,modules,features,ffi,highlight,verification_defects}.rs` | Added | T-01…T-20 and INC-2026-0012 regressions |
| `.github/workflows/ci.yml`, `scripts/ci_step.py`, `scripts/check_version.py`, `commands.perch` | Modified | CI on macOS and Linux, the feature matrix, readable CI failures, version sync |
| `vhco-contract.json`, `vhco.json`, `vhco.html` | Modified | Contract delta (contract first) and regenerated model |

## Known Issues

- **Windows is not supported.** CI found about 13 problem areas (paths, file replace, stdio MCP, UDP and others),
  catalogued in [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md) for a future port.
- **`rivet request --input-jsonl -` waits for stdin EOF after the request ends** (local and `--endpoint`):
  [INC-2026-0013](../incidents/resolved/inc-2026-0013-input-jsonl-waits-for-stdin-eof.md), found by the CI run on
  this tag. Fixed in **v0.2.1**; upgrade, or close stdin when the terminal record arrives.
- **No AddressSanitizer CI job** for `rivet-ffi` yet. Leak checks ran locally on macOS (T-12).
- **crates.io publication is deferred** (G-PUB: needs the owner's Capy published as `capy-lang`). Depend on the git
  tag instead.

## Limitations

```text
 not in 0.2.0                          behaviour
 ────────────────────────────────────  ──────────────────────────────────────────────────────
 Windows                                unsupported (INC-2026-0011)
 process sandbox on Linux               gated: unsupported.sandbox_backend (exit 5 / 501), ADR-0003
 mTLS serve authentication              refuses to start: unsupported.serve_mtls (exit 5)
 URL imports, hot reload, unload        imports are local paths; `import "https://…"` → syntax.import
 finally, Stage C adapters              as in 0.1.0
 persistent trace store                 process lifetime; rivet trace export saves one
 crates.io package                      git dependency on the tag (G-PUB)
```

## Follow-up Work

- A Windows port plan: work through INC-2026-0011 W-01…W-13 and re-enable the Windows CI job.
- An ASan job for `rivet-ffi` on Linux CI.
- 0.3.0: remove the deprecated `id`/`params`/`--params` input aliases (MIG-2026-0001 timeline).
- G-PUB: publish Capy as `capy-lang`, then `rivet-runtime`, to crates.io (owner decision).

## Release Completion Gate (DOCUMENTATION §34)

Walked on 2026-09-30 against tag `v0.2.0` (commit `21bb2e94dfa476856495997c4f510f5537c65225`).

| # | Condition | State | Evidence |
|---|---|---|---|
| 1 | Approved plan exists | ✔ | PLAN-2026-0002 (ADR-0004) |
| 2 | Proposal preserves the request and traces requirements | ✔ | PROP-2026-0002 r3 (UQ-01…UQ-09, R1–R24) |
| 3 | Every user-facing requirement has a UC | ✔ | UC-01…UC-11 |
| 4 | Changes and file rows map to requirements and forward | ✔ | TEST-2026-0032: no orphans |
| 5 | Standards baseline recorded and passed | ✔ | Project Standards Baseline above |
| 6 | Implementation matches the plan; deviations documented | ✔ | Plan findings table; RPT-2026-0015 |
| 7 | No unexplained open item in release scope | ✔ | TASK-038 DEFERRED (G-PUB, owner); TASK-095 artifacts built, GitHub Release upload manual (no token here) |
| 8 | Unexpected bugs documented | ✔ | INC-2026-0009…0013 |
| 9 | Reusable problems documented | ✔ | TRBL-2026-0004…0007 |
| 10 | Tests completed and recorded | ✔ | TEST-2026-0001…0053 |
| 11 | Every test links its requirement | ✔ | `validated_plan_requirements` |
| 12–14 | Measurable claims | N/A | none made |
| 15 | Validation report with a result per requirement | ✔ | RPT-2026-0015: 24 PASS |
| 16 | Every PARTIAL or FAIL referenced | ✔ | none remaining; tag CI failure → INC-2026-0013 |
| 17 | Verification guide, one U-row per update | ✔ | DEMO-2026-0020 U-01…U-24 |
| 18 | U-rows link requirement, action, expected result and evidence | ✔ | Released Updates table |
| 19 | Demo commands executed | ✔ | TEST-2026-0030 PASS; tagged-build smoke run |
| 20–22 | Manual integrated, feature catalogue, every CLI/API/error documented | ✔ | MAN-2026-0001…0010, API-2026-0001…0007 |
| 23–27 | Six documentation-impact decisions | ✔ | Documentation Impact above |
| 28 | Code and system docs consistent | ✔ | `vhco sync` 0; `vhco docs check` 0 errors |
| 29 | on-release docs reviewed | ✔ | next_review_date 2026-10-29 |
| 30–31 | Canonical version updated and synchronized | ✔ | `check_version.py --tag`: 0.2.0 everywhere |
| 32–34 | Release committed, clean tree, full SHA recorded | ✔ | `21bb2e9` release: v0.2.0 |
| 35–36 | Tag created, matches, resolves to the commit | ✔ | annotated `v0.2.0` → `21bb2e9` |
| 37 | Commit and tag pushed | ✔ | `origin` main and `v0.2.0`; CI on main green (36635293035); tag run: INC-2026-0013 |
| 38 | Known issues and limitations documented | ✔ | above |
| 39–44 | Release document complete, links, accurate, identity | ✔ | this document; `vhco docs release . REL-0.2.0` |
| 45 | Indexes regenerated | ✔ | docs/index.md, releases, testing, reports, proposals, incidents |

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) · [PROP-2026-0002](../proposals/implemented/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md) · [ADR-0004](../decisions/adr-0004-approve-envelopes-globals-library-ffi-highlighting.md)
- [RPT-2026-0015](../reports/rpt-2026-0015-validation-of-plan-2026-0002.md) · [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md) · [MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md)
- [REL-0.1.0](rel-0.1.0-release-notes.md) · [Tests](../testing/index.md) · [Incidents](../incidents/index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 2 | 2026-09-30 | Claude | Finalized with tag v0.2.0 and commit 21bb2e94dfa476856495997c4f510f5537c65225; §34 gate walked. |
| 1 | 2026-09-30 | Claude | Drafted for the release commit (tag and commit recorded after tagging). |
