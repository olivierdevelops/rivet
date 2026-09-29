---
document_id: ONB-2026-0001
title: "Rivet contributor setup"
document_type: onboarding
status: active
created_date: 2026-09-28
last_updated: 2026-09-30
document_revision: 6
authors: [Claude, Codex]
owner: Project maintainer
systems: [Rivet]
components: [language, registry, execution, policy, audit, transports, serve, cli, library, ffi]
affected_versions:
  from: "0.1.0"
  to: null
last_verified_version: "0.2.0-rc (main at 8031baa)"
next_review_date: 2026-10-29
applicable_environments: [development]
audience: [new-contributors, maintainers, agents]
confidentiality: internal
review_cycle: on-release
scope: Everything a new contributor needs to build, test, document and extend Rivet 0.2.0 — toolchain (Rust, cbindgen, a C compiler, Python for editors/), the Cargo workspace and features, layout, the vhco loop, test suites (build librivet before cargo test), docs checks, reading CI failures, adding an effect adapter, changing the grammar and regenerating the editor grammar.
reason: PLAN-2026-0001 D-33 — new contributors need one verified path from clone to a first green change; PLAN-2026-0002 D-39 updates it for the 0.2.0 workspace, features, FFI and editor tooling.
related_documents: [PLAN-2026-0001, PROP-2026-0001, PLAN-2026-0002, ADR-0001, ADR-0002, ADR-0003, ADR-0005, RES-2026-0001, TRBL-2026-0001, TRBL-2026-0002, TRBL-2026-0003, TRBL-2026-0004, TRBL-2026-0005, TRBL-2026-0006, TRBL-2026-0007, OPS-2026-0001, SYS-2026-0010, SYS-2026-0011, INC-2026-0011]
supersedes: null
superseded_by: null
tags: [rivet, onboarding, contributing, toolchain, vhco, testing, workspace, ffi, editors, ci]
---

# Rivet contributor setup

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-30
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** language, registry, execution, policy, audit, transports, serve, cli, library, ffi

## Summary

Rivet 0.2.0 is a Cargo **workspace** of two packages. `rivet-runtime` provides the library `rivet` and, with the
`cli` feature, the `rivet` binary. `rivet-ffi` builds `librivet` for C hosts. The code is organised with VHCO: five
code buckets whose import rules are enforced by `vhco validate`, a hand-authored design contract
(`vhco-contract.json`) and a generated model (`vhco.json`). This guide takes you from a fresh clone to a green
change. Commands and results were re-run on macOS arm64 on 2026-09-29 at main `8031baa` (0.2.0 release candidate).
**Supported development platforms are macOS and Linux**, and CI is green on both. Windows is not supported in 0.2.0
([INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)).

```text
 clone ─▶ toolchain (Rust, cbindgen, cc, python3) ─▶ cargo build --workspace --all-features   (builds librivet)
   │                                                        │
   │                                                        ▼
   │                          cargo test --workspace --all-targets --all-features (476 tests)
   │                                                        │
   │            vhco validate/sync/check ─▶ check_docs ─▶ editors checks ─▶ (perch gates runs all of it)
   │                                                        │
   └──────────────────────────── you are ready to follow the AGENTS.md loop ◀┘
```

## Toolchain

| Tool | Version verified | Why | Install |
|---|---|---|---|
| Rust | `1.90.0` + `rustfmt`, `clippy` (pinned in `rust-toolchain.toml`, profile minimal) | build, lint, format | `rustup` picks the pin up automatically inside the repo |
| cbindgen | `0.29.4` | regenerate / verify `ffi/include/rivet.h` (CI fails when the checked-in header differs) | `cargo install cbindgen` |
| C compiler + make | Apple clang 21 (`cc`) / gcc on Linux | build and run `examples/c` (`make -C examples/c test`) and the `conformance_ffi` suite | Xcode command-line tools / `build-essential` |
| protoc | `libprotoc 29.3` | only to regenerate the pinned gRPC descriptor sets in `tests/fixtures/grpc/*.pb` and demo 10 (`protoc --include_imports --descriptor_set_out=…`); the build does not run it; CI installs it | `brew install protobuf` / distro package |
| cargo-deny | `0.20.2` | licence/ban/source policy in `deny.toml` (ring-only TLS: aws-lc-rs and openssl are banned) | `cargo install cargo-deny` |
| vhco | `1.6.0` | architecture validation, contract drift, docs checks | project-provided binary on `PATH` |
| bman | installed CLI | global installation with `perch install` | project-provided binary on `PATH` |
| Perch | installed CLI | optional development command wrappers | project-provided binary on `PATH` |
| python3 | 3.9+ | `scripts/check_docs.py`, `scripts/ci_step.py`, `editors/*.py` (grammar generation, keyword drift, grammar tests, `.vsix` packaging), `examples/python`, `ffi/render_pc.py` | system |
| Node.js | **not needed** | the `.vsix` is packaged by `editors/vscode/package_vsix.py` and the grammar is tested by `editors/tests/check_grammar.py`, both Python (no `vsce`, no `vscode-tmgrammar-test`) | — |
| Disk | ≥ 30 GiB free | `target/` holds the debug workspace, tests and release artifacts (it reached tens of GiB during PLAN-2026-0002); `cargo clean` reclaims it | see [TRBL-2026-0003](../troubleshooting/trbl-2026-0003-linker-fails-with-no-space-left-on-device.md) |

Verify:

```sh
rustc --version          # rustc 1.90.0 (1159e78c4 2025-09-14)
protoc --version         # libprotoc 29.3
cargo deny --version     # cargo-deny 0.20.2
cbindgen --version       # cbindgen 0.29.4
cc --version             # Apple clang version 21.0.0 (clang-2100.0.123.102)
vhco --version           # vhco 1.6.0
cargo build --workspace --all-features   # target/debug/rivet + target/debug/librivet.{dylib|so,a}
target/debug/rivet --version             # rivet 0.2.1
```

A plain `cargo build` builds the library only. The binary needs `--features cli`, which `--all-features` includes.

## Workspace and Cargo features (0.2.0)

```text
 rivet/  [workspace] members = [".", "ffi"]      version: [workspace.package] (one version for both)
 ├── Cargo.toml   package rivet-runtime   lib "rivet"  (src/lib.rs = facade; src/internal.rs = hidden buckets)
 │                                        bin "rivet"  (src/main.rs; required-features = ["cli"])
 │   features:  default = [serve, grpc, quic, oauth]      cli = clap + the binary (off by default)
 └── ffi/         package rivet-ffi (publish = false) → librivet cdylib + staticlib, rivet.h, rivet.pc
```

| You want to… | Command |
|---|---|
| build everything, as CI does | `cargo build --workspace --all-features` |
| build only the binary | `cargo build --features cli` (`perch build` = release) |
| build librivet | `cargo build --release -p rivet-ffi` (`perch ffi` also verifies the header and runs the C/Python examples) |
| check a lean build | `cargo build -p rivet-runtime --no-default-features --features cli` (`perch features` builds each feature alone) |
| regenerate the C header | `cbindgen --config ffi/cbindgen.toml --crate rivet-ffi --output ffi/include/rivet.h` (`perch ffi_header`; commit the result) |

The FFI logic lives in `src/orchestrator/setup_ffi.rs` (the `ffi` surface, visible to VHCO); `ffi/src/lib.rs` holds
only the `extern "C"` shims. See [SYS-2026-0010](../system/components/sys-2026-0010-ffi-surface-and-packaging.md).

## Perch development commands

The root [`commands.perch`](../../commands.perch) provides these wrappers. Run `perch --help` for descriptions,
`perch main` for a short task list, or `perch --check` to validate the file. Bare `perch` displays Perch usage.

| Command | Runs / effect |
|---|---|
| `perch build` | `cargo build --locked --release --features cli --bin rivet` |
| `perch build_debug` | `cargo build --locked --features cli --bin rivet` |
| `perch install` | Build release (`--features cli`) with `--target-dir "<checkout>/target"`, then `bman add "<checkout>/target/release/rivet"` |
| `perch check` | `cargo check --locked --workspace --all-targets --all-features` |
| `perch tests` | `cargo build --locked --workspace --all-features` (librivet for `conformance_ffi`), then `cargo test --locked --workspace --all-targets --all-features` |
| `perch clippy` | `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` |
| `perch features` | builds `rivet-runtime` with no default features and with each of `serve`, `grpc`, `quic`, `oauth`, `cli` alone (T-10) |
| `perch ffi` | release `rivet-ffi`, `cbindgen --verify` of `rivet.h`, `make -C examples/c test`, `examples/python/{demo,modules}.py` |
| `perch ffi_header` | regenerates `ffi/include/rivet.h` with cbindgen |
| `perch fmt` | `cargo fmt --all`; edits source formatting |
| `perch fmt_check` | `cargo fmt --all -- --check`; reads source formatting |
| `perch docs_check` | `python3 scripts/check_docs.py` |
| `perch architecture` | `vhco validate .`, then `vhco sync .`, then `vhco check .` |
| `perch gates` | `fmt_check`, `check`, `clippy`, `tests`, `features`, `docs_check`, `architecture`, in that order |
| `perch help_cli` | `cargo run --locked --features cli --bin rivet -- --help` |
| `perch live` | `vhco live .`; contract explorer on port 7777, stop with Ctrl+C |
| `perch spec` | `vhco spec .`; regenerates `vhco.json` |
| `perch docs` | `vhco doc . --format html`; regenerates `vhco.html` |
| `perch clean` | `cargo clean`; removes Cargo build artifacts |
| `perch main` | Prints the task list |

```text
perch gates -> fmt_check -> check -> clippy -> tests -> features -> docs_check -> architecture
                   any failure -> nonzero exit; later tasks do not run
```

`tests` is plural because `test` is a reserved Perch built-in. Each task uses `dir "${script_dir}"`, so
`perch -f /path/to/rivet/commands.perch gates` works from another directory. Build output respects Cargo
configuration, including `CARGO_TARGET_DIR`; install explicitly uses the checkout's `target/` to pass the
correct artifact to bman. `bman help` shows the managed global bin directory; keep it on `PATH`.

Cargo, cbindgen, make, bman, Python 3 and VHCO are declared as optional Perch binary requirements so help and individual tasks
remain available when an unrelated tool is absent. The selected task still needs its executable on `PATH`;
missing tools or failing subprocesses produce a nonzero exit. Perch forwards the declared optional
`CARGO_TARGET_DIR`, `CARGO_HOME` and `RUSTUP_HOME` environment variables, plus its
default operational environment. Other tool overrides should be set in Cargo configuration or used with
direct Cargo commands. The Rust pin installs rustfmt and Clippy.

`gates` covers the listed development checks. CI additionally builds release artifacts, verifies the C header,
checks version agreement and runs cargo-deny. Run `perch build`, `perch ffi`, `python3 scripts/check_version.py` and
`cargo deny check licenses bans sources` for those checks (the version script assumes the default target
path unless given `--bin`). `spec`, `docs`, `fmt` and `clean` are explicit tasks outside `gates`.

## Repository Layout and the Five Buckets

```text
rivet/                   Cargo workspace: rivet-runtime (this dir) + rivet-ffi (ffi/)
├── src/
│   ├── lib.rs           the public facade (Runtime, Module, envelopes, Policy, Value, Error, highlight, …)
│   ├── internal.rs      #[doc(hidden)] pub mod internal — #[path] declarations of the five buckets below
│   ├── domain/          pure data: IR, envelope, modules, policy, contracts, errors (exit/HTTP registry), ports.rs
│   ├── features/<f>/    one file per use case + ports.rs — language, registry, execution, files, connectors,
│   │                    audit, policy, auth, datagrams, quic, grpc, sessions, serve, transports
│   ├── io/<surface>/    surface adapters — cli, http (+poll), ws, mcp
│   ├── infra/           adapters satisfying ports — capy_parser + rivet.capy grammar, execution_driver
│   │                    (interpreter + EffectAdapter/ResourceHandle), http/socket/process/udp/quic/grpc/
│   │                    oauth/mcp adapters, policy_broker, trace_store, serve_listener, sandbox_*
│   ├── orchestrator/    composition root — runtime.rs, transports.rs, setup_<surface>.rs (incl. setup_ffi.rs), builtins.rs
│   └── main.rs          the CLI entry (only with --features cli)
├── ffi/                 rivet-ffi: src/lib.rs shims, src/tests.rs safety tests, cbindgen.toml, include/rivet.h, rivet.pc.in
├── editors/             gen_grammar.py → keywords.json + rivet.tmLanguage.json; check_keywords.py; tests/check_grammar.py;
│                        vscode/ (package.json, package_vsix.py)
├── tests/               conformance_*.rs suites (T-xx) + support/, transport_support/, oauth_support/, p3_support/, fixtures/
├── examples/            dump_ast.rs (SyntaxTree of a file), embed.rs, modules.rs; c/ (demo.c, modules.c, Makefile);
│                        python/ (rivet.py ctypes wrapper, demo.py, modules.py)
├── docs/                all documentation (DOCUMENTATION.md standard); demos/ are the runnable samples
├── commands.perch       build, install, test, lint, documentation and VHCO tasks
├── scripts/check_docs.py  documentation checker (T-31) · scripts/ci_step.py (CI annotations) · check_version.py
├── vhco-contract.json   HAND-AUTHORED design contract (never generate it)
├── vhco.json · vhco.html  GENERATED model and explorer
├── deny.toml · rust-toolchain.toml · .github/workflows/ci.yml · .vhco.json
└── AGENTS.md · DOCUMENTATION.md · PROJECT.md
```

```text
                  may import ▶
 ┌──────────────┐
 │ orchestrator │──────────────┬──────────────┬──────────────┐
 └──────────────┘              ▼              ▼              ▼
                        ┌────────────┐  ┌──────────┐  ┌──────────┐
                        │ features/* │  │  io/*    │  │  infra   │     never each other,
                        └─────┬──────┘  └────┬─────┘  └────┬─────┘     never orchestrator
                              └──────────────┼─────────────┘
                                             ▼
                                       ┌──────────┐
                                       │  domain  │  imports nothing internal
                                       └──────────┘
```

Helper code that is not a use case goes in a sub-folder with its own `mod.rs` (for example
`src/features/audit/support/`); a loose `.rs` file in a feature folder is counted as a use case
([TRBL-2026-0001](../troubleshooting/trbl-2026-0001-vhco-helper-files-counted-as-use-cases.md)).

## The AGENTS.md Loop

Every change follows [AGENTS.md](../../AGENTS.md) "The loop":

```text
 1 CONTRACT ─▶ 2 EVAL ─▶ 3 CODE ─▶ 4 TEST ─▶ 5 VALIDATE ─▶ 6 DOCS
 edit vhco-     vhco live .   annotated   one vhco:test   vhco validate .  README + docs/
 contract.json  human         code,       per use case;   vhco sync . (0)  current state
 BY HAND        approves      claim todos green           vhco check .
      ▲                                                   vhco spec .
      └──────────────── new requirement: back to 1 ◀────────────────────────┘
```

- Never run `vhco spec . > vhco-contract.json` and never "fix" drift with `vhco sync . --update-spec`.
- `vhco assure .` is the one read-only gate: validate + sync + guarantees + the commands in `.vhco.json`
  (`cargo test --all-targets`, `cargo clippy --all-targets -- -D warnings`, `python3 scripts/check_docs.py`).

`vhco` reads only `src/`, so FFI logic and annotations live in `src/orchestrator/setup_ffi.rs`, not in `ffi/`. When
you rename a trigger or a step, edit the contract flow by hand
([TRBL-2026-0004](../troubleshooting/trbl-2026-0004-vhco-sync-drift-after-renaming-triggers-or-steps.md)). Contract
entries written ahead of code must follow
[TRBL-2026-0005](../troubleshooting/trbl-2026-0005-contract-written-ahead-of-code-drifts-on-ports-and-step-order.md).

## Running Tests and Conformance Suites

**Build first, then test.** `cargo test` does not produce `librivet`, and `conformance_ffi` links the C examples
against it ([TRBL-2026-0006](../troubleshooting/trbl-2026-0006-cargo-test-leaves-cdylib-and-staticlib-in-deps.md),
INC-2026-0010). CI and `perch tests` do the same:

```sh
cargo build --workspace --all-features                                   # librivet + the rivet binary
cargo test  --workspace --all-targets --all-features                     # everything: 476 tests in 39 test binaries
cargo test  --test conformance_envelope                                  # one suite
cargo test  --test conformance_samples                                   # demo + reference corpus (T-29)
cargo test  -p rivet-runtime --no-default-features --features cli --test conformance_features   # a lean build (T-10)
cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo deny check licenses bans sources                                   # bans ok, licenses ok, sources ok
```

```text
 cargo test (no build first)            cargo build --workspace --all-features ; cargo test …
   conformance_ffi: librivet missing ──✗   target/debug/librivet.{dylib|so,a} present ──▶ C + Python FFI tests run
```

| Suite (`tests/`) | Plan test | Covers |
|---|---|---|
| `conformance_language` | T-01 | compilation, header rules, duplicates, prefix calls, cycles, numeric path segments (`xs.0`) |
| `conformance_surfaces` | T-02 | the real `rivet` binary: one operation on every access point |
| `conformance_streams` | T-03 | SSE / JSONL / lines bodies, child output |
| `conformance_resources` | T-05 | scoped HTTP, TCP, Unix, WebSocket, processes |
| `conformance_mcp` | T-06 | outbound MCP connectors |
| `conformance_audit` | T-09 | every lowered effect site is audited |
| `conformance_oauth` | T-11 | OAuth 2.0 flows |
| `conformance_udp` / `conformance_quic` / `conformance_http3` | T-12 / T-13 / T-14 | datagrams, QUIC, HTTP/3 |
| `conformance_grpc` | T-17 | gRPC against pinned descriptor sets |
| `conformance_mcp_catalog` | T-18 | incoming MCP (`/mcp`, `serve --stdio`) |
| `conformance_sessions` | T-19 | live sessions |
| `conformance_outputs` | T-20 | declared outputs |
| `conformance_serve` | T-22 | one listener, same operation over REST/SSE/WS/poll/MCP |
| `conformance_io_manifest` | T-25 | golden I/O manifest rows from the demos (per-file bootstrap rows) |
| `conformance_policy_generate` | T-26 | `policy generate` |
| `conformance_samples` | T-29 | every demo bundle compiles; every ```` ```rivet ```` block of REF-2026-0002 parses and lowers |
| `conformance_envelope` (0.2.0) | PLAN-2026-0002 T-01…T-05, T-15 | one ResponseEnvelope / InputEnvelope on every surface, deprecated aliases, pretty |
| `conformance_globals` (0.2.0) | T-06…T-08 | `global` evaluation, check codes, manifest substitution |
| `conformance_modules` (0.2.0) | T-16, T-17, T-18, T-20 | `import`, namespacing, visibility, cycles, root confinement, `Runtime::load`, snapshots |
| `conformance_features` (0.2.0) | T-10 | `unsupported.feature` per compiled-out feature (run per build in the CI matrix) |
| `conformance_library` (0.2.0) | T-10 | the facade: `call`, `call_json`, scopes, modules |
| `conformance_ffi` (0.2.0) | T-11, T-12, T-19 | C examples, Python ctypes, handle safety (needs librivet built) |
| `conformance_highlight` (0.2.0) | T-13, T-14 | highlight goldens, the REF-2026-0002 and demo corpus, keyword drift |
| `ffi/src/tests.rs` (0.2.0) | T-12 | in-crate FFI safety tests (NULL, double free, wrong handle kind, panics) |

Result on 2026-09-29 (0.2.0-rc, main `8031baa`, macOS): `cargo build --workspace --all-features` then `cargo test --workspace --all-targets --all-features --no-fail-fast` gave 39 test binaries, **476 passed, 0 failed, 0 ignored**. On Linux the process sandbox refuses
(`unsupported.sandbox_backend`) until verified on a kernel ≥ 6.12
([ADR-0003](../decisions/adr-0003-process-sandbox-backends.md)); the process tests are platform-aware and expect that
refusal there.

## Reading CI failures

CI (`.github/workflows/ci.yml`) runs on `ubuntu-latest` and `macos-latest`. The jobs are fmt, clippy, release
build, debug build (librivet), tests, the C header check, docs and the version check, plus a `features` matrix
(`none`, `serve`, `grpc`, `quic`, `oauth`, `cli`) and cargo-deny. Job logs need admin rights, so every step runs
through `scripts/ci_step.py`, which re-emits the failing lines as public **check-run annotations**
([TRBL-2026-0007](../troubleshooting/trbl-2026-0007-reading-ci-failures-through-annotations.md)):

```text
 step output ─▶ ci_step.py: strip ANSI, keep FAILED / panicked / error / --> / assertion lines + tail
             ─▶ ≤ 4 chunks × 4000 chars ─▶ ::error title=<step>::…   (every step `if: always()`)
```

```sh
curl -s "https://api.github.com/repos/olivierdevelops/rivet/actions/runs?per_page=1"              # latest run id
curl -s "https://api.github.com/repos/olivierdevelops/rivet/actions/runs/<run>/jobs?per_page=50"  # job ids
curl -s "https://api.github.com/repos/olivierdevelops/rivet/check-runs/<job_id>/annotations?per_page=50" \
  | python3 -c "import json,sys; [print(a['title'], a['message']) for a in json.loads(sys.stdin.read(), strict=False) if a.get('title')]"
```

GitHub keeps at most 10 error annotations per step and 50 per job. Reproduce a failure locally with the same
command the step names (for example `cargo test --workspace --all-targets --all-features --no-fail-fast`).
Windows is not in the matrix (INC-2026-0011).

## Documentation Checks

```sh
python3 scripts/check_docs.py      # front matter, IDs, prefix↔type↔dir, status, file names, revisions,
                                   # links/anchors, fences, index membership → "check_docs: N files, 0 problem(s)"
vhco docs check .                  # DOC-* rules incl. claims checked against the code model (errors must be 0)
```

Every new document needs YAML front matter (DOCUMENTATION.md §5), the visible header (§7), a Change History whose
last row equals `document_revision`, a link from its directory `index.md`, and ASCII visuals.

## Adding a New Effect Adapter

Effects are statements such as `x = http get "…"` or `with udp "host:port" as sock … end`. The interpreter
(`src/infra/execution_driver.rs`) dispatches every non-built-in effect kind to an **`EffectAdapter`** registered by
name; scoped forms return a **`ResourceHandle`** owned by the enclosing `with` block.

```text
 .rivet source ─▶ Capy grammar (rivet.capy) ─▶ lowering (EFFECT_WORDS → EffectForm{kind,…})
                                                     │
                   io manifest: audit/support/effect_sites.rs ─▶ `rivet io`, `--check-policy`
                                                     │
 run: Interpreter ─▶ adapters["<kind>"] ─▶ EffectAdapter::run | ::open ─▶ Box<dyn ResourceHandle>
                                               │                           call/next/next_of/property/
                                               ▼                           open_child/close (≤ 5 s)
                          feature use case authorizes EVERY intent via PolicyEvaluator
                          (or ctx.authorize(capability, verb, target)) BEFORE any socket/file/process
```

Trait surface (all methods default to an `unsupported.*` error, so implement only what the form supports):

```rust
#[async_trait]
pub trait EffectAdapter: Send + Sync {
    async fn run(&self, ctx: &EffectCtx, form: &EffectForm, args: EvaluatedForm) -> RivetResult<Value>;          // one-shot
    async fn open(&self, ctx: &EffectCtx, form: &EffectForm, args: EvaluatedForm)
        -> RivetResult<Box<dyn ResourceHandle>>;                                                                   // `with`
}
#[async_trait]
pub trait ResourceHandle: Send {
    async fn call(&mut self, ctx: &EffectCtx, method: &str, args: Vec<EvalArg>) -> RivetResult<Value>;
    async fn next(&mut self, ctx: &EffectCtx) -> RivetResult<Option<Value>>;
    async fn next_of(&mut self, ctx: &EffectCtx, member: &str) -> RivetResult<Option<Value>>;
    async fn property(&mut self, ctx: &EffectCtx, name: &str) -> RivetResult<Value>;
    async fn open_child(&mut self, ctx: &EffectCtx, method: &str, args: Vec<EvalArg>,
                        options: Vec<(String, Vec<EvalArg>)>) -> RivetResult<Box<dyn ResourceHandle>>;
    async fn close(self: Box<Self>) -> RivetResult<()>;
    fn shared(&self) -> Option<Arc<dyn SharedResource>>;
}
```

Step by step (the UDP adapter is the smallest complete example: `features/datagrams/exchange_datagrams.rs`,
`infra/udp_adapter.rs`, registered in `orchestrator/runtime.rs` `register_transports`):

1. **Contract first.** Add the use case (in/out/needs port, todos, errors) to `vhco-contract.json` by hand and get it
   approved (`vhco live .`).
2. **Domain.** Add plan/result types to `src/domain/` (for example `domain/transports.rs`); add a variant to
   `EffectKind` in `domain/ir.rs` (`parse` / `as_str`) — unknown words become `EffectKind::Other(String)`.
3. **Feature use case + port.** `src/features/<feature>/<use_case>.rs` with a `// vhco:usecase` header and a driver
   port in `ports.rs`. The use case authorizes **every** intent (capability + access verb + target, e.g.
   `allow_network connect udp://HOST:PORT`) through `PolicyEvaluator` before the driver touches anything; the first
   denial returns `permission.denied` with no effect.
4. **Infra adapter.** `src/infra/<kind>_adapter.rs`: `// vhco:infra <name> satisfies <Port>`; implement
   `EffectAdapter` (and `ResourceHandle` for scoped forms). Handles are not values — they cannot be returned or
   passed to DAG nodes; `close` must finish inside the 5 s cleanup grace.
5. **Lowering.** Add the keyword to `EFFECT_WORDS` in `features/language/lowering/lower.rs` and extend the `with`
   resource error text; validate option tails there (Capy only captures `rest tail`).
6. **I/O manifest.** Teach `features/audit/support/effect_sites.rs` the new form so `rivet io` lists its sites with
   the same capability/verb/target the use case authorizes; otherwise `io --check-policy` cannot preview it.
7. **Wire it.** In the orchestrator, `interp.register_adapter("<kind>", Arc::new(YourAdapter::new(…)))`
   (`runtime.rs` or `transports.rs`). An unregistered kind fails at run time with `unsupported.adapter`.
   If the adapter pulls in a heavy dependency, put it behind a Cargo feature (0.2.0). Declare the feature in
   `Cargo.toml` with `dep:` entries, make `domain::capabilities::require_build_features` refuse bundles that use
   it (`unsupported.feature`, before anything runs), add a stand-in to `infra/unsupported_features.rs`, add a
   `rivet.capabilities` row reason and a `features` matrix entry, and extend `tests/conformance_features.rs`
   ([SYS-2026-0005](../system/integrations/sys-2026-0005-protocol-adapters.md#deployment)).
8. **Tests.** A `tests/conformance_<kind>.rs` suite with a local fixture server, `// vhco:test <feature.use_case> -- …`
   annotations, covering allow, deny (no packet/connection before denial), errors and cleanup; add golden rows to
   `conformance_io_manifest` if the demos use it.
9. **Validate + docs.** `vhco validate .`, `vhco sync .` (0), `cargo test`, `cargo clippy …`; then a demo under
   `docs/demos/`, reference examples in REF-2026-0002 (`conformance_samples` and `conformance_highlight` scan them),
   and the manual/system pages. If the form adds a keyword, regenerate the editor grammar (next section).

## Grammar Changes and the Corpus Tests

The surface grammar is `src/infra/rivet.capy` (Capy, pinned in `Cargo.toml` by git rev). Capy owns statement shapes,
blocks, value expressions and spans; Rivet lowering owns option-tail validation and semantics.

```text
 edit rivet.capy ─▶ cargo run --example dump_ast -- file.rivet   (inspect the SyntaxTree)
        │
        ▼
 cargo test --test conformance_language      unit rules (headers, duplicates, cycles)
 cargo test --test conformance_samples       CORPUS: 12 demo bundles compile; ≥ 90 ```rivet blocks in
                                             REF-2026-0002 parse cleanly and lower without structural errors
 cargo test --lib capy_parser                parser adapter tests (did-you-mean, indentation)
        │
        ▼
 negative cases: docs/research/res-2026-0001-capy-grammar-spike/negative-corpus.txt
 regenerate the spike corpus: python3 docs/research/res-2026-0001-capy-grammar-spike/extract_corpus.py OUT
```

```text
$ cargo run -q --example dump_ast -- docs/demos/01-catalog/app.rivet
operation id="demo.greet"@Some((1, 11, 21))
  name value="\"Greet a person\""@Some((2, 10, 26))
  …
```

Rules: a grammar change that alters an existing shape needs a contract/proposal update first; every new documented
shape gets a ```` ```rivet ```` example in REF-2026-0002 (the corpus test picks it up automatically); block sections
cannot carry arguments ([TRBL-2026-0002](../troubleshooting/trbl-2026-0002-capy-section-headers-cannot-take-arguments.md)).

### Regenerating the editor grammar (0.2.0)

`editors/keywords.json` and `editors/rivet.tmLanguage.json` are **generated** from `src/infra/rivet.capy`. The
highlighter embeds `keywords.json` with `include_str!`, so the CLI, the library, the C ABI and the TextMate grammar
share one keyword table ([SYS-2026-0011](../system/components/sys-2026-0011-highlighting-and-grammar-generation.md)).
Everything is standard-library Python; Node is not needed.

```text
 edit rivet.capy ─▶ python3 editors/gen_grammar.py            writes keywords.json + rivet.tmLanguage.json
                    python3 editors/gen_grammar.py --check    exit 1 when an output is stale (CI via the tests)
                    python3 editors/check_keywords.py         every rivet.capy literal classified
                    python3 editors/tests/check_grammar.py    TextMate regexes over REF-2026-0002 + demo apps
                    cargo test --test conformance_highlight   highlight goldens + corpus + drift
                    python3 editors/vscode/package_vsix.py    deterministic .vsix (version = workspace version)
```

Verified on 2026-09-29:

```text
$ python3 editors/gen_grammar.py --check
gen_grammar: generated files are up to date
$ python3 editors/check_keywords.py
check_keywords: 108 rivet.capy literals classified; 148 table words; generated files up to date
$ python3 editors/tests/check_grammar.py
check_grammar: 109 samples, 1731 lines tokenized; every declaration/control keyword scoped
```

A new keyword that `gen_grammar.py` cannot classify fails `check_keywords.py`. Add it to `CLASS_TABLE` in
`editors/gen_grammar.py`, then regenerate and commit both generated files.

## Troubleshooting

| Problem | Document |
|---|---|
| `vhco sync` reports extra use cases for helper files | [TRBL-2026-0001](../troubleshooting/trbl-2026-0001-vhco-helper-files-counted-as-use-cases.md) |
| Capy section headers cannot take arguments (try/catch filters) | [TRBL-2026-0002](../troubleshooting/trbl-2026-0002-capy-section-headers-cannot-take-arguments.md) |
| Link fails with "No space left on device" | [TRBL-2026-0003](../troubleshooting/trbl-2026-0003-linker-fails-with-no-space-left-on-device.md) |
| `vhco sync` drift after renaming a trigger or a flow step | [TRBL-2026-0004](../troubleshooting/trbl-2026-0004-vhco-sync-drift-after-renaming-triggers-or-steps.md) |
| Contract written ahead of code drifts on ports and step order | [TRBL-2026-0005](../troubleshooting/trbl-2026-0005-contract-written-ahead-of-code-drifts-on-ports-and-step-order.md) |
| `conformance_ffi` cannot find librivet (`cargo test` alone does not build it) | [TRBL-2026-0006](../troubleshooting/trbl-2026-0006-cargo-test-leaves-cdylib-and-staticlib-in-deps.md) |
| A CI step failed and the log is not readable | [TRBL-2026-0007](../troubleshooting/trbl-2026-0007-reading-ci-failures-through-annotations.md) |
| `cargo install --path .` installed no `rivet` | add `--features cli` (the binary's `required-features`) |
| macOS: `ld: warning: object file … was built for newer 'macOS' version` when linking C against `librivet.a` | noise; build librivet and the C program with one `MACOSX_DEPLOYMENT_TARGET` ([SYS-2026-0010](../system/components/sys-2026-0010-ffi-surface-and-packaging.md#deployment)) |
| Running `rivet serve` locally, tokens, policy rollout | [OPS-2026-0001](../operations/ops-2026-0001-operating-rivet-serve.md) |

## Glossary

| Term | Meaning |
|---|---|
| bundle | entry `.rivet` file plus the modules it imports (`import "PATH" as ALIAS`), and the one `policy.json` beside the entry |
| envelope | the one JSON shape of every answer (`ResponseEnvelope`) and request (`InputEnvelope`), 0.2.0 |
| facade | the stable `use rivet::…` API in `src/lib.rs`; the buckets are the hidden `rivet::internal::…` |
| feature | a Cargo feature (`serve`, `grpc`, `quic`, `oauth`, `cli`); a bundle needing a missing one is `unsupported.feature` |
| effect / site | an I/O statement; each lowered occurrence is a manifest site with capability, verb, target |
| broker | the policy evaluator every effect passes before it happens |
| principal | the authenticated caller of `rivet serve` (`local` on loopback without auth) |
| contract / model | `vhco-contract.json` (hand-authored design) / `vhco.json` (generated from code) |

## Related Documents

- [AGENTS.md](../../AGENTS.md) · [DOCUMENTATION.md](../../DOCUMENTATION.md)
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md)
- [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)
- [ADR-0002 Rust crate selection](../decisions/adr-0002-rust-crate-selection.md)
- [RES-2026-0001 Capy grammar spike](../research/res-2026-0001-capy-grammar-spike.md)
- [REF-2026-0002 Language and usage](../references/ref-2026-0002-language-and-usage.md)
- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) (D-39) · [ADR-0005 workspace and features](../decisions/adr-0005-workspace-package-and-features.md)
- [SYS-2026-0010 FFI, packaging, features](../system/components/sys-2026-0010-ffi-surface-and-packaging.md) · [SYS-2026-0011 Highlighting and grammar generation](../system/components/sys-2026-0011-highlighting-and-grammar-generation.md)
- [ARCH-2026-0001 Architecture](../architecture/arch-2026-0001-rivet-runtime-architecture.md)
- [Demos](../demos/README.md) · [Onboarding index](index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 6 | 2026-09-30 | Claude | v0.2.1 patch (PLAN-2026-0002 TASK-097): version strings, install tag v0.2.1; INC-2026-0013 behaviour where described. |
| 5 | 2026-09-30 | Claude | v0.2.0 release (PLAN-2026-0002 TASK-091): version strings and current-release wording updated to 0.2.0. |
| 4 | 2026-09-29 | Claude | PLAN-2026-0002 D-39 (TASK-079): workspace and Cargo features, cbindgen and a C compiler, Python-only editor tooling (no Node), build librivet before `cargo test` (TRBL-2026-0006), the 0.2.0 suites, reading CI failures (TRBL-2026-0007), grammar regeneration, updated Perch tasks, macOS/Linux only; Perch content kept. |
| 3 | 2026-09-28 | Codex | Changed Perch installation to a release build followed by bman add, as requested by the maintainer. |
| 2 | 2026-09-28 | Codex | Documented all Perch tasks, working-directory behavior, prerequisites and fail-fast development gates. |
| 1 | 2026-09-28 | Claude | Initial contributor guide, verified against 0.1.0-dev (f40d4aa). |
