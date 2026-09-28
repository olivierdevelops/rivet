---
document_id: SYS-2026-0010
title: "FFI surface, workspace packaging, facade and Cargo features"
document_type: system
status: active
created_date: 2026-09-29
last_updated: 2026-09-29
document_revision: 1
authors: [Claude]
owner: Project maintainer
component_owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [ffi, library, features, sessions, registry]
affected_versions:
  from: "0.2.0"
  to: null
last_verified_version: "0.2.0-rc (main at e7ed8ed)"
next_review_date: 2026-10-29
review_cycle: on-release
confidentiality: internal
scope: The 0.2.0 package layout (workspace with rivet-runtime and rivet-ffi), the public facade of the `rivet` library versus the hidden `rivet::internal` modules, the Cargo features and how a compiled-out adapter is refused (`unsupported.feature`), and the `ffi` surface (src/orchestrator/setup_ffi.rs) behind librivet — handle table, guards, runtime/call/module lifecycles and the build artifacts.
reason: PLAN-2026-0002 row D-15 (TASK-073); DOCUMENTATION §31 requires system documentation of the implemented packaging and the new surface; checked against the code and the built artifacts.
related_documents: [ADR-0005, RES-2026-0004, TRBL-2026-0006, PROP-2026-0002, PLAN-2026-0002, API-2026-0007, API-2026-0004, MAN-2026-0009, SYS-2026-0004, SYS-2026-0007, ARCH-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, system, ffi, packaging, cargo-features, facade, workspace]
---

# FFI surface, workspace packaging, facade and Cargo features

> **Status:** Active
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0 and later
> **Owner:** Project maintainer
> **Affected Components:** ffi, library, packaging, features, sessions, registry
> **Last Verified Version:** 0.2.0-rc (main at e7ed8ed)

## Summary

Rivet 0.2.0 is a Cargo **workspace** of two packages that share one version. `rivet-runtime` is the library
(`rivet`) and, with the `cli` feature, the `rivet` binary. `rivet-ffi` is a thin C shim crate that builds
`librivet`. All FFI logic lives in the runtime crate as the `ffi` surface, so the VHCO model (which reads only
`src/`) sees it.

```text
 rivet/ (workspace, version in [workspace.package])
 ├── Cargo.toml ········ package rivet-runtime ── lib "rivet" (src/lib.rs facade + #[doc(hidden)] internal)
 │                                              └─ bin "rivet" (src/main.rs, required-features = ["cli"])
 ├── src/{domain,features,io,infra,orchestrator}   the five VHCO buckets (paths unchanged)
 │   ├── internal.rs ·· #[path] modules re-exported as rivet::internal::… (not a stable API)
 │   └── orchestrator/setup_ffi.rs ·· the `ffi` surface (handle table, guards, JSON in/out)
 ├── ffi/ ·············· package rivet-ffi (publish = false) ── lib "rivet": cdylib + staticlib
 │   ├── src/lib.rs ···· #[no_mangle] extern "C" shims only      src/tests.rs ·· safety tests
 │   ├── build.rs ······ install name @rpath/librivet.dylib (macOS) · soname librivet.so (Linux)
 │   ├── cbindgen.toml ─▶ include/rivet.h (checked in)    rivet.pc.in + render_pc.py ─▶ rivet.pc
 └── editors/keywords.json ·· shipped in the crate (the highlighter include_str!s it)
```

## Responsibilities

- Expose a small, stable Rust API at the crate root (the **facade**) and keep internals out of the docs.
- Gate heavy adapters behind Cargo features and refuse bundles that need a missing one, before anything runs.
- Build `librivet` (shared + static) with only `rivet_*` symbols exported and a generated header.
- Convert every C call into safe Rust, JSON in / JSON out, without undefined behaviour on misuse.

## Boundaries and Non-Responsibilities

- The dispatcher, policy and sessions are unchanged: the FFI calls `Runtime` like any other surface.
- No Windows artifacts (`rivet.dll`); no crates.io publication (gated on G-PUB: `capy-core` is a git dependency).
- No language bindings beyond examples (`examples/python/rivet.py`).

## Architecture

### Facade

```text
 use rivet::…  (public, documented)                     rivet::internal::… (#[doc(hidden)])
 ─────────────────────────────────────                  ────────────────────────────────────
 Runtime, RuntimeBuilder, Module, Scope,                 domain, features, io, infra, orchestrator
 StreamHandle, DuplexHandle, DuplexSender,               (Rivet's own tests, examples, rivet-ffi)
 InputEnvelope, ResponseEnvelope, Envelope,
 EnvelopeStatus, RecordType, OutputFormat,               inside the crate: pub(crate) use internal::{domain,…}
 Completion, DataEvent, DataSink, Policy, Value,         keeps every crate::domain::… path
 Error (= RivetError), ErrorKind, Result<T>,
 highlight, build_features, VERSION, ABI_VERSION,
 types::{GraphQuery, Catalog, OutputReport, Principal, RegistryEntry,
         IoQuery, IoReport, PolicyDraft, ModuleSummary, SessionLimits, SourceSpan}
```

Embedding `rivet serve` inside a host still needs `rivet::internal::orchestrator::setup_serve::{ServeOptions,
start}`: it is not part of the facade in 0.2.0.

### Cargo features

| Feature | Default | Enables | Missing → |
|---|---|---|---|
| `serve` | yes | axum listener: HTTP, SSE, polling, WebSocket, MCP | `rivet serve` exits 5 with `unsupported.feature` (`details.feature: "serve"`); `serve.surfaces` lists only `cli`, `library` |
| `grpc` | yes | tonic/prost dynamic gRPC | a `grpc` connector or effect → `unsupported.feature` at load |
| `quic` | yes | quinn QUIC and h3 HTTP/3 | a `quic` scope, `version 3` or `version prefer [3, …]` → `unsupported.feature` |
| `oauth` | yes | OAuth 2.0 profiles, keychain stores | an `auth NAME oauth2` profile → `unsupported.feature` |
| `cli` | no | clap and the `rivet` binary | no binary (`required-features`) |

```text
 load ─▶ compile ─▶ effect sites ─▶ require_build_features(compiled)
                                        ├─ all present ─▶ Runtime
                                        └─ missing ─────▶ unsupported.feature (kind unsupported, exit 5, HTTP 501)
                                                          every use reported: first + suppressed, source order
```

Real refusal from a `--no-default-features --features cli` build:

```text
$ rivet --file h3.rivet check
error[unsupported.feature]: HTTP/3 (`version 3` or `version prefer [3, …]`) in `web.get` needs the `quic` feature, which this build was compiled without (rebuild rivet-runtime with `--features quic`)
  --> h3.rivet:3:5
   |
  3|     r = http get "https://api.example.com/items"
   |     ^
$ echo $?
5
$ rivet --file app.rivet serve --listen 127.0.0.1:18953
{"request_id":"","trace_id":"","operation":"rivet.serve","type":"result","status":"error","data":null,"error":{"kind":"unsupported","code":"unsupported.feature","message":"`rivet serve` needs the `serve` feature, which this build was compiled without (rebuild rivet-runtime with `--features serve`)","retryable":false,"details":{"feature":"serve"}},"effects":"none","data_count":0}
$ echo $?
5
```

The same lean build reports `build_features: ["cli"]` and marks `grpc`, `http3`, `quic_v1` and `oauth2_*` rows
of `rivet.capabilities` `unsupported` with the reason "compiled without the `quic` Cargo feature
(unsupported.feature)". `rivet-ffi` passes the four runtime features through (its default = the runtime default).

### The `ffi` surface

```text
 C host ──rivet_*(…)──▶ ffi/src/lib.rs (CStr → bytes, handle → u64) ──▶ setup_ffi::* (safe Rust)
                                                                          │
     guard_text / guard_status: catch_unwind ─▶ internal.panic envelope  │
     arg(): NULL / non-UTF-8 ─▶ validation.ffi_argument                   │
     handle lookup in TABLE ─▶ unknown / freed / wrong kind ─▶ validation.ffi_argument
                                                                          ▼
     FfiRuntime { Runtime, multi-thread tokio runtime "rivet-ffi", pretty, closed }
        rivet_request      = block_on(Runtime::call_json)        ─▶ ResponseEnvelope::render
        rivet_call_start   = sessions.open_session (connection_owned)
        rivet_call_next    = sessions.read_events (≤ 5 s slices)  ─▶ one record per string
        rivet_load         = Runtime::load / load_as ─▶ FfiModule { Module }
     returned strings: CString::into_raw, address recorded in TABLE.strings
     rivet_string_free: address must be in TABLE.strings (else RIVET_ERROR), then CString::from_raw
```

**Handle table.** A handle is a never-reused token `(generation << 2) | kind` (kind 1 = runtime, 2 = call,
3 = module; never 0). One `Mutex<Table>` maps live tokens to `Arc` objects. Tokens are never dereferenced, so a
double free or use-after-free finds no entry and returns `RIVET_ERROR` or a `validation.ffi_argument` envelope.

## Interfaces

- Rust: the facade above ([API-2026-0004](../../api/api-2026-0004-rust-library.md)).
- C: the 18 functions of `ffi/include/rivet.h` ([API-2026-0007](../../api/api-2026-0007-c-abi.md)).
- VHCO: surface `ffi` (`// vhco:surface ffi kind ffi …` in `setup_ffi.rs`) calls `language/compile_program`,
  `policy/load_policy`, `serve/parse_input`, `execution/request_operation`, `execution/cancel_request`,
  `sessions/*`, `language/highlight_source`, `registry/load_module`.

## Configuration

Runtime options JSON (`file` | `source`+`path`+`root` | `root`, `policy_file` | `policy_json`, `ceiling_json`,
`pretty`), parsed by `domain::ffi::FfiOptions`; unknown keys are refused.

## Runtime Behaviour

### Runtime handle

```text
 rivet_runtime_new ─▶ OPEN ──rivet_request / call_start / load (any thread)──▶ OPEN
                        │
                        └─ rivet_runtime_free ─▶ closed=true ─▶ cancel in-flight calls, drain (5 s grace)
                                                 ─▶ entry removed ─▶ FREED (tokio shut down in the background)
                                                 second free ─▶ RIVET_ERROR
```

### Call handle

```text
 call_start ─▶ STARTED ─(next)─▶ data* ─(next)─▶ result ─(next)─▶ NULL, DONE
    │ failed start: a detached failure yields its error record, then NULL (never a NULL handle)
    │ send / finish_input while running (sessions.send_input / finish_input)
    │ cancel ─▶ sessions.cancel_session + execution.cancel_request ─▶ result{status:"cancelled"}
    │ no read for 60 s ─▶ idle lease cancels the session (cancelled.idle)
    └ free ─▶ running? cancel + 5 s bounded cleanup ─▶ FREED ; second free ─▶ RIVET_ERROR
```

A call is a **connection-owned** session of the shared session driver: it does not count toward the
8-sessions-per-principal limit of `rivet.sessions.*` ([SYS-2026-0007](../runtime/sys-2026-0007-sessions.md)).

### Module handle

```text
 rivet_load ─▶ Runtime::load(_as) ─▶ registry.load_module ─▶ new catalog snapshot (Arc swap) ─▶ LOADED
    │ module_call(id) = Module::call ─▶ dispatcher("alias.id")
    │ module_call_start(id) = a call handle over the namespaced operation
    └ rivet_module_free ─▶ handle FREED (module stays in the runtime catalog)
```

## Data and Storage

None persisted. The handle table and tracked strings live in process memory.

## Dependencies

`rivet-ffi` → `rivet-runtime` (path, `default-features = false`, features passed through). Build tools:
`cbindgen` (header regeneration; CI fails when the checked-in header differs), a C compiler for examples.

## Deployment

| Artifact | Built by | Notes |
|---|---|---|
| `rivet` binary | `cargo build --release --features cli` / `cargo install … --features cli` | default features + `cli` |
| `librivet.dylib` / `librivet.so` | `cargo build --release -p rivet-ffi` | install name `@rpath/librivet.dylib`; soname `librivet.so` |
| `librivet.a` | same | link with the OS system libraries (macOS: `-framework Security -framework CoreFoundation -liconv -lc -lm`; Linux: `-lgcc_s -lutil -lrt -lpthread -lm -ldl -lc`) |
| `rivet.h` | cbindgen (checked in) | ABI version 1 |
| `rivet.pc` | `python3 ffi/render_pc.py --prefix P` | `Libs.private` = the static list |

`rivet-ffi` builds **cdylib and staticlib only** (no `rlib`): an `rlib` named `librivet.rlib` collided with the
runtime's own `librivet.rlib` output (INC-2026-0010), so the safety tests moved into `ffi/src/tests.rs`.
`cargo test` leaves the cdylib/staticlib in `target/<profile>/deps`
([TRBL-2026-0006](../../troubleshooting/trbl-2026-0006-cargo-test-leaves-cdylib-and-staticlib-in-deps.md)).
On macOS, build the library and link C programs with one `MACOSX_DEPLOYMENT_TARGET`.

CI (ubuntu-latest, macos-latest) runs fmt, clippy `--workspace --all-targets --all-features`, the release
build, the full test suite and a feature matrix (`none`, `serve`, `grpc`, `quic`, `oauth`, `cli`) that builds
`rivet-runtime` with each feature alone and runs `conformance_features`.

## Security Boundaries

The C host is the principal (`local`); the policy in the options is its only grant. The boundary never
dereferences a caller pointer except to read a NUL-terminated string, catches panics, and tracks every string it
returns. See [SEC-2026-0001](../../security/sec-2026-0001-policy-and-sandbox-model.md#ffi-trust-boundary).

## Observability

The library prints nothing. Envelopes carry `request_id`/`trace_id`; `rivet.capabilities` reports
`build_features` and `abi_version`.

## Known Limitations

- `rivet_version()` and the `.vsix` follow the workspace version, bumped only by the release commit.
- The facade has no serve-embedding entry point (`rivet::internal` is needed).
- Linux static link flags and ASan runs are verified only in CI.
- Windows is not supported.

## Last Verified Version

0.2.0-rc (main at `e7ed8ed`, 2026-09-29): `nm -gU librivet.dylib` shows exactly the 18 `rivet_*` symbols;
`make -C examples/c test`, `examples/python/{demo,modules}.py` and a cgo program ran; the lean build refusal above
was captured.

## Related Documents

- [ADR-0005](../../decisions/adr-0005-workspace-package-and-features.md), [RES-2026-0004](../../research/res-2026-0004-workspace-ffi-and-feature-experiments.md)
- [API-2026-0007](../../api/api-2026-0007-c-abi.md), [MAN-2026-0009](../../manuals/man-2026-0009-c-abi-and-ffi.md)
- [ARCH-2026-0001](../../architecture/arch-2026-0001-rivet-runtime-architecture.md), [SYS-2026-0004](sys-2026-0004-surfaces-and-serve.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-29 | Claude | Initial system document (TASK-073, D-15): workspace, facade, features, `ffi` surface and handle lifecycles |
