---
document_id: ONB-2026-0001
title: "Rivet contributor setup"
document_type: onboarding
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 3
authors: [Claude, Codex]
owner: Project maintainer
systems: [Rivet]
components: [language, registry, execution, policy, audit, transports, serve, cli, library]
affected_versions:
  from: "0.1.0"
  to: null
last_verified_version: "0.1.0-dev (commit f40d4aa)"
applicable_environments: [development]
audience: [new-contributors, maintainers, agents]
confidentiality: internal
review_cycle: on-release
scope: Everything a new contributor needs to build, test, document and extend Rivet 0.1.0 — toolchain, layout, the vhco loop, test suites, docs checks, adding an effect adapter and changing the grammar.
reason: PLAN-2026-0001 D-33 — new contributors need one verified path from clone to a first green change.
related_documents: [PLAN-2026-0001, PROP-2026-0001, ADR-0001, ADR-0002, ADR-0003, RES-2026-0001, TRBL-2026-0001, TRBL-2026-0002, TRBL-2026-0003, OPS-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, onboarding, contributing, toolchain, vhco, testing]
---

# Rivet contributor setup

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** language, registry, execution, policy, audit, transports, serve, cli, library

## Summary

Rivet is one Rust crate (library + `rivet` binary) organised with VHCO: five code buckets whose import rules are
enforced by `vhco validate`, a hand-authored design contract (`vhco-contract.json`) and a generated model
(`vhco.json`). This guide takes you from a fresh clone to a green change. Commands and results were run on macOS
arm64 on 2026-09-28 at commit `f40d4aa`.

```text
 clone ─▶ install toolchain ─▶ cargo build ─▶ cargo test (248 tests) ─▶ vhco validate/sync ─▶ check_docs
   │                                                                                           │
   └──────────────────────────── you are ready to follow the AGENTS.md loop ◀──────────────────┘
```

## Toolchain

| Tool | Version verified | Why | Install |
|---|---|---|---|
| Rust | `1.90.0` + `rustfmt`, `clippy` (pinned in `rust-toolchain.toml`, profile minimal) | build, lint, format | `rustup` picks the pin up automatically inside the repo |
| protoc | `libprotoc 29.3` | only to regenerate the pinned gRPC descriptor sets in `tests/fixtures/grpc/*.pb` and demo 10 (`protoc --include_imports --descriptor_set_out=…`); the build does not run it; CI installs it | `brew install protobuf` / distro package |
| cargo-deny | `0.20.2` | licence/ban/source policy in `deny.toml` (ring-only TLS: aws-lc-rs and openssl are banned) | `cargo install cargo-deny` |
| vhco | `1.6.0` | architecture validation, contract drift, docs checks | project-provided binary on `PATH` |
| bman | installed CLI | global installation with `perch install` | project-provided binary on `PATH` |
| Perch | installed CLI | optional development command wrappers | project-provided binary on `PATH` |
| python3 | 3.9+ | `scripts/check_docs.py` | system |
| Disk | ≥ 20 GiB free | `target/` reached ≈ 14 GiB (debug + tests); release adds ≈ 0.5 GiB | see [TRBL-2026-0003](../troubleshooting/trbl-2026-0003-linker-fails-with-no-space-left-on-device.md) |

Verify:

```sh
rustc --version          # rustc 1.90.0 (1159e78c4 2025-09-14)
protoc --version         # libprotoc 29.3
cargo deny --version     # cargo-deny 0.20.2
vhco --version           # vhco 1.6.0
cargo build              # target/debug/rivet (≈ 53 MiB)
target/debug/rivet --version    # rivet 0.1.0-dev
```

## Perch development commands

The root [`commands.perch`](../../commands.perch) provides these wrappers. Run `perch --help` for descriptions,
`perch main` for a short task list, or `perch --check` to validate the file. Bare `perch` displays Perch usage.

| Command | Runs / effect |
|---|---|
| `perch build` | `cargo build --locked --release --bin rivet` |
| `perch build_debug` | `cargo build --locked --bin rivet` |
| `perch install` | Build release with `--target-dir "<checkout>/target"`, then `bman add "<checkout>/target/release/rivet"` (`.exe` on Windows) |
| `perch check` | `cargo check --locked --all-targets` |
| `perch tests` | `cargo test --locked --all-targets` |
| `perch clippy` | `cargo clippy --locked --all-targets -- -D warnings` |
| `perch fmt` | `cargo fmt --all`; edits source formatting |
| `perch fmt_check` | `cargo fmt --all -- --check`; reads source formatting |
| `perch docs_check` | `python3 scripts/check_docs.py` |
| `perch architecture` | `vhco validate .`, then `vhco sync .`, then `vhco check .` |
| `perch gates` | `fmt_check`, `check`, `clippy`, `tests`, `docs_check`, `architecture`, in that order |
| `perch help_cli` | `cargo run --locked --bin rivet -- --help` |
| `perch live` | `vhco live .`; contract explorer on port 7777, stop with Ctrl+C |
| `perch spec` | `vhco spec .`; regenerates `vhco.json` |
| `perch docs` | `vhco doc . --format html`; regenerates `vhco.html` |
| `perch clean` | `cargo clean`; removes Cargo build artifacts |
| `perch main` | Prints the task list |

```text
perch gates -> fmt_check -> check -> clippy -> tests -> docs_check -> architecture
                   any failure -> nonzero exit; later tasks do not run
```

`tests` is plural because `test` is a reserved Perch built-in. Each task uses `dir "${script_dir}"`, so
`perch -f /path/to/rivet/commands.perch gates` works from another directory. Build output respects Cargo
configuration, including `CARGO_TARGET_DIR`; install explicitly uses the checkout's `target/` to pass the
correct artifact to bman. `bman help` shows the managed global bin directory; keep it on `PATH`.

Cargo, bman, Python 3 and VHCO are declared as optional Perch binary requirements so help and individual tasks
remain available when an unrelated tool is absent. The selected task still needs its executable on `PATH`;
missing tools or failing subprocesses produce a nonzero exit. Perch forwards the declared optional
`CARGO_TARGET_DIR`, `CARGO_HOME` and `RUSTUP_HOME` environment variables, plus its
default operational environment. Other tool overrides should be set in Cargo configuration or used with
direct Cargo commands. The Rust pin installs rustfmt and Clippy.

`gates` covers the listed development checks. CI additionally builds release artifacts, checks version
agreement and runs cargo-deny. Run `perch build`, `python3 scripts/check_version.py` and
`cargo deny check licenses bans sources` for those checks (the version script assumes the default target
path unless given `--bin`). `spec`, `docs`, `fmt` and `clean` are explicit tasks outside `gates`.

## Repository Layout and the Five Buckets

```text
rivet/
├── src/
│   ├── domain/          pure data: IR, policy, contracts, errors (exit/HTTP registry), ports.rs
│   ├── features/<f>/    one file per use case + ports.rs — language, registry, execution, files, connectors,
│   │                    audit, policy, auth, datagrams, quic, grpc, sessions, serve, transports
│   ├── io/<surface>/    surface adapters — cli, http (+poll), ws, mcp
│   ├── infra/           adapters satisfying ports — capy_parser + rivet.capy grammar, execution_driver
│   │                    (interpreter + EffectAdapter/ResourceHandle), http/socket/process/udp/quic/grpc/
│   │                    oauth/mcp adapters, policy_broker, trace_store, serve_listener, sandbox_*
│   ├── orchestrator/    composition root — runtime.rs, transports.rs, setup_<surface>.rs, builtins.rs
│   ├── lib.rs · main.rs
├── tests/               conformance_*.rs suites (T-xx) + support/, transport_support/, oauth_support/, fixtures/
├── examples/dump_ast.rs print the SyntaxTree of a .rivet file
├── docs/                all documentation (DOCUMENTATION.md standard); demos/ are the runnable samples
├── commands.perch       build, install, test, lint, documentation and VHCO tasks
├── scripts/check_docs.py  documentation checker (T-31)
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

Current state at `f40d4aa`: `vhco validate .` → `✓ no violations`; `vhco sync .` → `✓ code matches vhco-contract.json`.

## Running Tests and Conformance Suites

```sh
cargo test                                   # everything: 107 unit tests + 18 conformance suites = 248 tests
cargo test --test conformance_serve          # one suite
cargo test --test conformance_samples        # demo + reference corpus (T-29)
cargo fmt --check && cargo clippy --all-targets -- -D warnings
cargo deny check licenses bans sources       # bans ok, licenses ok, sources ok
```

| Suite (`tests/`) | Plan test | Covers |
|---|---|---|
| `conformance_language` | T-01 | compilation, header rules, duplicates, prefix calls, cycles |
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
| `conformance_io_manifest` | T-25 | golden I/O manifest rows from the demos |
| `conformance_policy_generate` | T-26 | `policy generate` |
| `conformance_samples` | T-29 | all 12 demo bundles compile; every reference block parses and lowers |

Result on 2026-09-28: all suites `ok`, 248 passed, 0 failed; `clippy` exit 0; `cargo deny` all ok.
Formatting is checked by `perch fmt_check` and by CI on Linux, macOS and Windows; use `perch fmt` to apply rustfmt changes.

On Linux the process sandbox refuses (`unsupported.sandbox_backend`) until verified on a kernel ≥ 6.12
([ADR-0003](../decisions/adr-0003-process-sandbox-backends.md)); sandbox tests expect that refusal there.

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
8. **Tests.** A `tests/conformance_<kind>.rs` suite with a local fixture server, `// vhco:test <feature.use_case> -- …`
   annotations, covering allow, deny (no packet/connection before denial), errors and cleanup; add golden rows to
   `conformance_io_manifest` if the demos use it.
9. **Validate + docs.** `vhco validate .`, `vhco sync .` (0), `cargo test`, `cargo clippy …`; then a demo under
   `docs/demos/`, reference examples in REF-2026-0002, and the manual/system pages.

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

## Troubleshooting

| Problem | Document |
|---|---|
| `vhco sync` reports extra use cases for helper files | [TRBL-2026-0001](../troubleshooting/trbl-2026-0001-vhco-helper-files-counted-as-use-cases.md) |
| Capy section headers cannot take arguments (try/catch filters) | [TRBL-2026-0002](../troubleshooting/trbl-2026-0002-capy-section-headers-cannot-take-arguments.md) |
| Link fails with "No space left on device" | [TRBL-2026-0003](../troubleshooting/trbl-2026-0003-linker-fails-with-no-space-left-on-device.md) |
| Running `rivet serve` locally, tokens, policy rollout | [OPS-2026-0001](../operations/ops-2026-0001-operating-rivet-serve.md) |

## Glossary

| Term | Meaning |
|---|---|
| bundle | entry `.rivet` file plus the files it includes, and `policy.json` beside it |
| effect / site | an I/O statement; each lowered occurrence is a manifest site with capability, verb, target |
| broker | the policy evaluator every effect passes before it happens |
| principal | the authenticated caller of `rivet serve` (`local` on loopback without auth) |
| contract / model | `vhco-contract.json` (hand-authored design) / `vhco.json` (generated from code) |

## Related Documents

- [AGENTS.md](../../AGENTS.md) · [DOCUMENTATION.md](../../DOCUMENTATION.md)
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md)
- [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md)
- [ADR-0002 Rust crate selection](../decisions/adr-0002-rust-crate-selection.md)
- [RES-2026-0001 Capy grammar spike](../research/res-2026-0001-capy-grammar-spike.md)
- [REF-2026-0002 Language and usage](../references/ref-2026-0002-language-and-usage.md)
- [Demos](../demos/README.md) · [Onboarding index](index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 3 | 2026-09-28 | Codex | Changed Perch installation to a release build followed by bman add, as requested by the maintainer. |
| 2 | 2026-09-28 | Codex | Documented all Perch tasks, working-directory behavior, prerequisites and fail-fast development gates. |
| 1 | 2026-09-28 | Claude | Initial contributor guide, verified against 0.1.0-dev (f40d4aa). |
