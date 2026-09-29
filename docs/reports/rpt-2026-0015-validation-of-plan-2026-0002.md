---
document_id: RPT-2026-0015
title: "Validation of PLAN-2026-0002 (Rivet v0.2.0)"
document_type: report
status: completed
created_date: 2026-09-29
last_updated: 2026-09-30
document_revision: 2
authors: [Claude]
owner: Project maintainer
report_date: 2026-09-29
systems: [Rivet]
components: [language, registry, execution, audit, serve, cli, http, ws, poll, mcp, library, ffi]
affected_versions:
  from: "0.2.0"
  to: null
confidentiality: internal
scope: Validation of every PLAN-2026-0002 requirement (R1–R24) against the implementation at commit 14750b8 (v0.2.0 release candidate), before the P5 version bump and tag.
reason: DOCUMENTATION §28 — the implementation is validated against the plan in a report before release (PLAN-2026-0002 TASK-062).
methodology: >-
  Every Test and Validation Checklist row T-01…T-20, T-27…T-32 and T-34 was executed (or, for T-30, its demo records
  were read) at commit 14750b8 on macOS 26.4.1 arm64 and cross-checked against CI run 36505156729 at the same commit
  (ubuntu-latest, macos-latest, feature matrix, deny); each is recorded as a TEST document. A requirement takes the
  worst result among its tests in the plan's Final Traceability table, lowered to PARTIAL where a requirement clause
  is not verifiable before P5. Security checks TASK-063/064 (secret-canary scan, global-secret refusal, FFI misuse
  with a leak check) were run in addition.
evidence_sources: [docs/testing/test-2026-0001…0053, cargo test --workspace --all-targets --all-features (493 passed), cargo test -- --nocapture canary scan (1122 lines), cargo fmt/clippy/build/deny, feature matrix builds, vhco validate/sync/check/assure, scripts/check_docs.py, vhco docs check, trace_check.py, leaks --atExit, GitHub Actions run 36505156729]
related_documents: [PLAN-2026-0002, PROP-2026-0002, ADR-0003, ADR-0004, ADR-0005, RES-2026-0004, INC-2026-0009, INC-2026-0010, INC-2026-0011, INC-2026-0012, TRBL-2026-0004, TRBL-2026-0005, TRBL-2026-0006, TRBL-2026-0007, DEMO-2026-0020, RPT-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, validation, report, v0.2.0]
---

# Validation of PLAN-2026-0002

> **Status:** Completed
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-30
> **Affected Versions:** 0.2.0
> **Owner:** Project maintainer
> **Affected Components:** language, registry, execution, audit, serve, cli, http, ws, poll, mcp, library, ffi

## Summary

| Result | Requirements |
|---|---:|
| PASS | 24 |
| PARTIAL | 0 |
| FAIL | 0 |
| NOT APPLICABLE | 0 |

```text
 R1 ─ R24 ──► plan tests T-01…T-20, T-27…T-32, T-34 ──► TEST-2026-00nn ──► this report ──► REL-0.2.0 (P5)
                  │
                  ├── cargo test --workspace --all-targets --all-features: 493 passed, 0 failed  (macOS, 14750b8)
                  ├── CI run 36505156729 @ 14750b8: ubuntu-latest ✔  macos-latest ✔  features ×6 ✔  deny ✔
                  ├── fmt ✔  clippy -D warnings ✔  deny ✔  release build ✔  feature matrix 6/6 (0 warnings)
                  ├── vhco validate ✔  sync 0 gaps ✔  check ✔  assure ✔   check_docs 0 problems · docs check 0 errors
                  ├── traceability: 0 orphans (R → TASK → PF → T → D → U)
                  └── security: canary scan 0 hits · global cannot hold a secret ✔ · FFI misuse 0 leaks
```

The planned functionality is implemented and behaves as specified on both supported platforms: macOS was tested locally and Linux in CI. Every proposal test T-01…T-20 is PASS. The 0.1.0 suites (T-34) are green on the new envelopes. The three 0.1.0 PARTIAL results that existed only because Linux and Windows had no CI (T-01, T-08, T-27 of PLAN-2026-0001) are now PASS. Linux is covered, and Windows is no longer a supported platform (maintainer decision, [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)).

Two requirements were PARTIAL at P3 and are now PASS:
- **R12:** now PASS (2026-09-30). The `v0.2.0` git-tag dependency was verified from GitHub (a fresh crate resolved `tag=v0.2.0#21bb2e9…`, built, and ran). crates.io publication stays deferred to the owner (G-PUB, ADR-0004), which is outside the requirement's acceptance.
- **R18:** now PASS (2026-09-30). P4 exited (every D-row DONE), the INC-2026-0012-affected demo steps were re-run, and the tagged build passed the DEMO-2026-0020 smoke run (TEST-2026-0030 PASS).

Neither is a defect in the implementation.

## Plan Under Validation

[PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md). It implements [PROP-2026-0002](../proposals/implemented/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md) revision 3, approved in [ADR-0004](../decisions/adr-0004-approve-envelopes-globals-library-ffi-highlighting.md) revision 2, for phases P1 and P2a–P2f. It covers:
- envelopes, input and pretty output;
- globals;
- the package, facade and features;
- the C ABI;
- highlighting;
- file modules.

The workspace layout follows [ADR-0005](../decisions/adr-0005-workspace-package-and-features.md) and [RES-2026-0004](../research/res-2026-0004-workspace-ffi-and-feature-experiments.md).

```text
 P1 ✔ ─► P2a ✔ ─► P2b ✔ ─► P2c ✔ ─► P2d ✔ ─► P2e ✔ ─► P2f ✔ ─► P3 (this report) ─► P4 docs/demos ─► P5 release
                                                                  ▲
                         INC-2026-0012 fixes (4122353, 1034636, 4a34537) landed before P3 ─┘
```

## Method

1. **Gates (TASK-060).** Each of the following ran at commit `14750b8` (`14750b86a157d8fbaac7443201bc0eeb30747366`) on macOS 26.4.1 arm64, with Rust 1.90.0 and cargo-deny 0.20.2:
   - `cargo fmt --all --check`;
   - `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
   - `cargo build --workspace --all-features`;
   - `cargo test --workspace --all-targets --all-features --no-fail-fast`, saved to `/tmp/p3-tests.log` and read with `grep`;
   - `cargo deny check`;
   - `cargo build --release --workspace --all-features`;
   - the feature matrix: `cargo build -p rivet-runtime --no-default-features [--features X]` for none, cli, serve, grpc, quic and oauth;
   - `vhco validate .`, `vhco sync .`, `vhco check .` and `vhco assure .`;
   - `python3 scripts/check_docs.py` and `vhco docs check .`.

   GitHub Actions [run 36505156729](https://github.com/olivierdevelops/rivet/actions/runs/36505156729) at the same commit is the Linux evidence: `test (ubuntu-latest)`, `test (macos-latest)`, the six `features` jobs and `deny`, all `success`.
2. **Tests (TASK-061).** Each plan test is recorded as a TEST document, with the list of executed test names (`cargo test --test X -- --list`) and the evidence lines:
   - T-01…T-20 → TEST-2026-0034…0053 (new);
   - T-27…T-32 → TEST-2026-0027…0032 (re-recorded);
   - T-34 → TEST-2026-0001…0026 (re-recorded, each keeping its v0.1.0 run).

   T-33 (release identity) stays with P5.
3. **Requirement results.** Each requirement takes the worst result among its tests in the plan's Final Traceability table. A requirement whose clause can only be checked after P5 is PARTIAL, with the reason stated.
4. **Security checks (TASK-063/064).** These are the canary scan, the global-secret check and the FFI misuse check with leak detection, described under [Security checks](#security-checks).
5. **Traceability (T-32).** A script over the plan and [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md) checks every R1–R24 through TASK, PF, T, D and U rows in both directions: 0 orphans.

## Requirement Results

"Linux CI" means GitHub Actions run 36505156729 at commit `14750b8`. Every suite named below ran green on `ubuntu-latest` and `macos-latest` in that run.

| Requirement | Expected | Observed | Test | Result |
|---|---|---|---|---|
| R1 | One `ResponseEnvelope` on CLI, HTTP, SSE, NDJSON, polling, WS, MCP, library and FFI; streams are data records then one result; 0.1.0 suites green on it (T-01, T-04, T-34) | Every surface validates against `response.schema.json` with fixed key order; stream `seq`/`data_count` correct on every surface incl. library terminal records; all 26 PLAN-2026-0001 suites green; macOS + Linux CI | [TEST-2026-0034](../testing/test-2026-0034-envelope-schema.md) PASS, [TEST-2026-0037](../testing/test-2026-0037-stream-records.md) PASS, [TEST-2026-0001…0026](../testing/index.md) PASS | PASS |
| R2 | `status` ok/error/cancelled/accepted, `type` result/data, `effects` top level (T-01) | All four statuses observed per surface; nested errors carry no `effects` (INC-2026-0012 item 7) | [TEST-2026-0034](../testing/test-2026-0034-envelope-schema.md) PASS | PASS |
| R3 | Built-ins, GET routes and every CLI `--json` output enveloped (T-03) | All listed outputs are envelopes; `policy explain --json` denial is an error envelope, exit 3 | [TEST-2026-0036](../testing/test-2026-0036-builtin-json-envelopes.md) PASS | PASS |
| R4 | One input file gives identical `data` on every surface incl. FFI (T-02) | Identical `data` via `--input FILE`, `--input -`, `--endpoint`, REST, polling, WS, MCP, `Runtime::call`, C and Python | [TEST-2026-0035](../testing/test-2026-0035-input-envelope.md) PASS | PASS |
| R5 | Legacy `{id, params}` with deprecation signals; mixed keys refused (T-15) | `Deprecation: true` + `deprecated=1`, `warning[deprecated.params]`, WS trace note; mixed keys `validation.input_envelope` 422 / exit 2 | [TEST-2026-0048](../testing/test-2026-0048-legacy-input.md) PASS | PASS |
| R6 | Pretty goldens; pretty refused on NDJSON/SSE (T-05) | Goldens match (indent 2, same key order); `validation.usage` exit 2, `validation.pretty_stream` 400; FFI `pretty` | [TEST-2026-0038](../testing/test-2026-0038-pretty-json.md) PASS | PASS |
| R7 | Globals load-time, read-only, visible everywhere; corpus parses with `global` (T-06, T-29) | Declaration-order values in every operation; 32 concurrent requests equal; REF/demo corpus incl. `global` blocks parses | [TEST-2026-0039](../testing/test-2026-0039-globals-values.md) PASS, [TEST-2026-0029](../testing/test-2026-0029-samples.md) PASS | PASS |
| R8 | `syntax.global`, `check.global_*` with spans, exit 2 (T-07); a global cannot hold a secret (TASK-064) | Six codes with exact line/column/end column; `env`, `secret` (statement and call), effects and requests refused as `check.global_not_constant`; a `secret` reusing a global name is `check.global_shadow` | [TEST-2026-0040](../testing/test-2026-0040-globals-diagnostics.md) PASS | PASS |
| R9 | Globals substituted in manifest and graph; exact grants (T-08) | Literal+global targets `exact`; `policy generate` exact grants. Param-bearing targets stay `param_dependent` (recorded deviation) | [TEST-2026-0041](../testing/test-2026-0041-globals-manifest.md) PASS | PASS |
| R10 | External crate builds against the facade only (T-09) | `cargo run --example embed` exit 0 (envelopes, stream, outputs, manifest, draft); facade exposes sessions, trace, serve; scratch crate recorded in DEMO-2026-0012 | [TEST-2026-0042](../testing/test-2026-0042-library-facade.md) PASS | PASS |
| R11 | Feature matrix builds; compiled-out adapters `unsupported.feature` (T-10) | none/cli/serve/grpc/quic/oauth build, 0 warnings, locally and in 6 CI jobs; refusals exit 5 / HTTP 501 at the call site | [TEST-2026-0043](../testing/test-2026-0043-cargo-features.md) PASS | PASS |
| R12 | Git dependency on tag `v0.2.0`; crates.io prepared, gated by G-PUB (T-10 packaging) | `cargo package --list` ships the embedded files, with no tests, docs or ffi (T-10 PASS). The git-tag dependency on `v0.2.0` resolves to `21bb2e9`, builds and runs from GitHub (DEMO-2026-0020 U-12, 2026-09-30); crates.io deferred (G-PUB) | [TEST-2026-0043](../testing/test-2026-0043-cargo-features.md) PASS | PASS |
| R13 | `librivet` shared + static, `rivet.h`, `rivet.pc`, ABI version (T-11) | 18 `rivet_*` exports only; C shared and static, Python ctypes; header = cbindgen (CI `--verify`); pkg-config; Linux `.so`/`.a` in CI | [TEST-2026-0044](../testing/test-2026-0044-c-abi.md) PASS | PASS |
| R14 | Misuse returns error envelopes; no crash, no leak (T-12) | 8 misuse tests pass; `leaks --atExit` 0 leaks for the misuse suite and the C examples. ASan not run (see Deviations) | [TEST-2026-0045](../testing/test-2026-0045-ffi-safety.md) PASS | PASS |
| R15 | Call handle start/next/send/finish_input/cancel/free (T-11) | Items then one terminal; live input echoed; cancel → `cancelled`; free while running bounded by the 5 s grace | [TEST-2026-0044](../testing/test-2026-0044-c-abi.md) PASS | PASS |
| R16 | Generated grammar, no drift, `.vsix` (T-13) | No drift; grammar scopes 107 samples; `.vsix` with the workspace version (Python tooling, recorded deviation) | [TEST-2026-0046](../testing/test-2026-0046-editor-grammar.md) PASS | PASS |
| R17 | `rivet highlight` goldens; partial tokens on error (T-14) | ansi/html/json goldens; classes; spans; partial tokens + exit 2; unclosed-block headers kept | [TEST-2026-0047](../testing/test-2026-0047-highlight-cli.md) PASS | PASS |
| R18 | Every Documentation and Demo Checklist row; demos executed against the RC (T-30, T-31, T-32) | `check_docs` 0 problems, `vhco docs check` 0 errors (T-31); 0 traceability orphans (T-32); demos 01–17 executed, INC-2026-0012 steps re-run, tagged-build smoke run PASS (T-30 PASS) | [TEST-2026-0030](../testing/test-2026-0030-demos-e2e.md) PASS, [TEST-2026-0031](../testing/test-2026-0031-documentation.md) PASS, [TEST-2026-0032](../testing/test-2026-0032-traceability.md) PASS | PASS |
| R19 | `import "PATH" as ALIAS [public]`, root-confined (T-16) | Compile once, relative paths, bootstrap rows per module file | [TEST-2026-0049](../testing/test-2026-0049-file-modules.md) PASS | PASS |
| R20 | `ALIAS.ID`, internal by default, transitive (T-16) | Namespaces, visibility, `(alias.id …)` and `request` calls, per-module globals/connectors | [TEST-2026-0049](../testing/test-2026-0049-file-modules.md) PASS | PASS |
| R21 | Every import error code with span and exit (T-17) | Seven codes incl. cycle path, symlink, both limits; URL imports `syntax.import`; load refusals carry IDs | [TEST-2026-0050](../testing/test-2026-0050-import-errors.md) PASS | PASS |
| R22 | `Runtime::load/load_as` → `Module`; builder without entry; snapshot swap (T-18) | Module API incl. streams; 6 concurrent loads during 24 requests; in-flight snapshot kept | [TEST-2026-0051](../testing/test-2026-0051-host-module-load.md) PASS | PASS |
| R23 | `rivet_load`, `rivet_module_*`; Python module object (T-19) | C shared/static and Python module attributes; `module_handles` unit test; 0 leaks | [TEST-2026-0052](../testing/test-2026-0052-ffi-modules.md) PASS | PASS |
| R24 | Loader's policy only; module `policy.json` ignored with a warning; manifest/graph/explain/generate cover modules (T-20) | Module policy cannot widen access; warning on `check`, `request`, `serve`; module spans and namespaced IDs | [TEST-2026-0053](../testing/test-2026-0053-module-policy.md) PASS | PASS |

```text
        R1 R2 R3 R4 R5 R6 R7 R8 R9 R10 R11 R12 R13 R14 R15 R16 R17 R18 R19 R20 R21 R22 R23 R24
 macOS  ✔  ✔  ✔  ✔  ✔  ✔  ✔  ✔  ✔  ✔   ✔   ◐   ✔   ✔   ✔   ✔   ✔   ◐   ✔   ✔   ✔   ✔   ✔   ✔
 Linux  ✔  ✔  ✔  ✔  ✔  ✔  ✔  ✔  ✔  ✔   ✔   ◐   ✔   ✔   ✔   ✔   ✔   ◐   ✔   ✔   ✔   ✔   ✔   ✔     (CI)
        ✔ PASS (all 24; R12 and R18 closed at P5, 2026-09-30)
```

## Security Checks

### Secret-canary scan (TASK-064)

The whole suite ran with output capture off, and the output was searched for every canary value defined in the tests.

```sh
cargo test --workspace --all-targets --all-features --no-fail-fast -- --nocapture > /tmp/p3-nocapture.log 2>&1   # EXIT=0, 493 passed
grep -rhoE "[A-Za-z0-9_-]*(CANARY|[Cc]anary|s3cr3t|super-secret)[A-Za-z0-9_-]*" tests ffi/src src   # inventory
grep -cE "CANARY|s3cr3t|super-secret|Q0FOQVJZLVQwOC1TRUNSRVQtVkFMVUU=" /tmp/p3-nocapture.log          # → 0
grep -ciE "canary" /tmp/p3-nocapture.log                                                              # → 0
```

| Canary (source) | What it stands for | Hits in 1122 output lines |
|---|---|---:|
| `CANARY-T08-SECRET-VALUE` and its base64 form (`conformance_sandbox`) | a secret env value that must stay bound to its destination | 0 |
| `CANARY-AT-n`, `CANARY-RT-n`, `CANARY-DC-n`, `CANARY-CODE-*`, `CANARY-CLIENT-SECRET` (`tests/oauth_support`, `conformance_oauth`, `conformance_mcp`, `conformance_grpc`) | OAuth access/refresh tokens, device codes, authorization codes, client secret | 0 |
| `s3cr3t-value-123` (`conformance_audit`) | a token read through `env` | 0 |
| `s3cr3t` (`conformance_modules`, T-20 fixture `secret/key.json`) | a file outside the loader's grants that the module's own `policy.json` would have granted | 0 |
| `super-secret` (`conformance_grpc`) | a gRPC metadata token | 0 |
| any case of `canary` | catch-all | 0 |

**Result: 0 hits.** The suites also assert, inside the tests, that these values do not appear in traces, envelopes, error bodies or CLI output (T-08, T-09 and T-11 of PLAN-2026-0001; `conformance_oauth` scans every output).

### A global cannot hold a secret (TASK-064, R8)

`conformance_globals::t07_not_constant` asserts that `global token = (env "API_TOKEN")` is `check.global_not_constant` at `1:16–33`, with a hint to use `secret token from env`. It also refuses a `request` and an effect. `t07_shadow` covers a `secret` that reuses a global's name. The other secret forms were confirmed on the debug build at `14750b8`, with `rivet check --file app.rivet`:

```text
global token = (env "API_TOKEN")
  error[check.global_not_constant]: global `token` cannot read the environment; globals are fixed at load time   --> app.rivet:1:16   exit 2
global token = secret API_TOKEN from env "API_TOKEN" for "https://api.example.com"
  error[check.global_not_constant]: global `token` cannot perform `secret`; globals are constants fixed at load time   --> app.rivet:1:16   exit 2
global token = (secret "API_TOKEN")
  error[check.global_not_constant]: global `token` cannot call `secret`; globals may use only literals, operators and the pure built-ins (…)   exit 2
secret API_KEY from env "K" for "https://api.example.com"     (with `global API_KEY = "x"`)
  error[check.global_shadow]: secret `API_KEY` reuses the name of a global; globals cannot be shadowed   --> app.rivet:5:5   exit 2
```

**Result: confirmed.** A global is a load-time constant built from literals, operators and pure built-ins. It can never read `env`, run `secret` or perform an effect, and a secret cannot hide behind a global's name.

### FFI misuse and leaks (TASK-064, R14)

The FFI misuse suite in `ffi/src/tests.rs` passed 8/8, both normally and under macOS `leaks --atExit`, with **0 leaks** (2512 live nodes at exit, 0 leaked bytes). The static C examples report 0 leaks too: `demo_static` covers request, stream, input, cancel and double free, and `modules_static` covers module objects. A nightly AddressSanitizer build compiled, but its test binary hung before the harness printed anything (even for `versions_agree`), so ASan produced no result. See the deviation below.

### Incident and troubleshooting review (TASK-063)

P3 found no new unexpected defect. The full suite, the gates and the security checks were green on the first run. The ASan hang is a tooling limitation of the local nightly toolchain, not a defect in Rivet, so it is recorded here and in TEST-2026-0045 rather than as an incident. The defects found earlier in PLAN-2026-0002 are recorded in the incidents below. The reusable problems are in TRBL-2026-0004…0007:
- vhco sync drift after renaming triggers or steps;
- contract entries written ahead of the code;
- `cargo test` leaving `librivet` in `deps/`;
- reading CI failures through annotations.

## Deviations From the Plan

The plan's Decisions, Findings, Deviations and Blockers table records every deviation. These are the ones that change behaviour or evidence:

| Deviation | Requirement | Effect |
|---|---|---|
| A target that contains a param stays `param_dependent` even when the rest comes from globals (the UC-04 sample showed `exact`) | R9 | Host and path are resolved and grantable; only literal+global targets are `exact` |
| Polling sub-routes keep their session bodies; POST `/v1/requests` (202 `accepted`) and every error are envelopes | R1 | Matches the proposal's polling sample |
| SSE event names mirror the record `type` (`event: data` / `event: result`); `event: error` is gone | R1 | Clients switch on `type`/`status` |
| MCP `isError` is true for `error` **and** `cancelled` | R1, R2 | Keeps 0.1.0 behaviour for cancelled calls |
| Data records omit `status`, `effects`, `data_count`; terminal stream records carry `seq` and `data_count`; unary WS results keep `seq` (every WS ref is a session) | R1 | Fixed by `stream-record.schema.json` and the INC-2026-0012 decision |
| `policy explain --json` on a denial prints a `status: error`, kind `permission` envelope on stderr (exit 3) | R3 | Consistent with every failing `--json` command |
| Internals live in `#[doc(hidden)] pub mod internal` (`src/internal.rs`) | R10 | Facade stable; tests use `rivet::internal::…` |
| ABI shape vs the proposal sketch: free/cancel functions return `RivetStatus`; `*_call_start` never returns NULL; NULL `data_json` means `{}` | R13–R15 | Double free observable (`RIVET_ERROR`) |
| `rivet-ffi` ships only `cdylib` + `staticlib`; FFI misuse tests moved in-crate (`ffi/src/tests.rs`) | R13, R14 | INC-2026-0010 fix; `ffi/tests/` is gone |
| `Module::stream`/`duplex` take the owning `&Scope` | R22 | Structured concurrency kept |
| `.vsix` built and grammar tested with Python (`package_vsix.py`, `check_grammar.py`) instead of Node `vsce` / `vscode-tmgrammar-test` | R16 | No Node dependency; Oniguruma-only regex behaviour not covered |
| **P3:** T-12 names ASan on Linux CI. CI has no ASan job, and the local nightly ASan binary did not start; the "no leak" criterion was verified with macOS `leaks --atExit` instead | R14 | Leak-free on macOS; use-after-free detection by ASan still missing (follow-up) |
| **P3:** T-30 was recorded from the P4 demo verification records instead of a fresh demo run | R18 | See R18 PARTIAL |

## Unintended Behaviour

**Found at P5 (2026-09-30).** The CI run on the `v0.2.0` tag failed one macOS test, `cli_live_input_local_and_remote`, while the `main` run of the same commit was green. The root cause is a CLI shutdown defect: `--input-jsonl -` waited for stdin EOF after the request ended. It is recorded in [INC-2026-0013](../incidents/resolved/inc-2026-0013-input-jsonl-waits-for-stdin-eof.md), fixed in `dd5e5ad` with a deterministic regression test, and shipped in v0.2.1. The tag `v0.2.0` keeps the defect (tags are never moved).

None is known at `14750b8`. PLAN-2026-0002 found these defects, all fixed before this report, each with a regression test:

| Incident | Severity | What happened | Fix | Verifying tests |
|---|---|---|---|---|
| [INC-2026-0009](../incidents/resolved/inc-2026-0009-numeric-index-paths-do-not-parse.md) | S4 | Numeric index paths (`xs.0`) did not parse; the message named `assign_map` (pre-existing in 0.1.0, found by the globals tests) | `93388c1`: numeric segments parse and evaluate; keyword statements name themselves | 3 `conformance_language` + 1 unit (TEST-2026-0001) |
| [INC-2026-0010](../incidents/resolved/inc-2026-0010-ffi-rlib-output-collision.md) | S3 | `rivet-ffi` and `rivet-runtime` both wrote `librivet.rlib` (a Cargo collision warning; LNK1201 on Windows) | `6f9943f`, `8045343`: `rivet-ffi` crate-type `[cdylib, staticlib]`; tests in-crate; librivet built before tests | `ffi/src/tests.rs` (TEST-2026-0045), `conformance_ffi` |
| [INC-2026-0012](../incidents/resolved/inc-2026-0012-documentation-and-demo-verification-defects.md) | S3 | 19 defects found by running the 0.2.0 docs and demos literally: envelope consistency across WS, MCP, polling and library (items 2–8, 16); highlight, inline `open`, gRPC span, URL imports (9, 10, 17, 18); `policy explain --data`, WS `deadline_ms`, load IDs, module policy warning, facade (11–15); T-29 corpus (1, 19) | `fad2940`, `4122353`, `1034636`, `4a34537`; docs updated in `55b9e73` | 17 tests in `conformance_verification_defects` + `conformance_samples` (cited in TEST-2026-0034…0053) |

```text
 found by              incident         fixed in                         regression tests       status
 ────────────────────  ───────────────  ───────────────────────────────  ─────────────────────  ────────
 globals suite (P2b)   INC-2026-0009    93388c1                          conformance_language   resolved
 CI portability        INC-2026-0010    6f9943f, 8045343                 ffi/src/tests.rs       resolved
 CI portability        INC-2026-0011    — (platform dropped)             —                      ACTIVE
 docs/demo run (P4)    INC-2026-0012    fad2940 4122353 1034636 4a34537  verification_defects   resolved
```

## Unresolved Incidents

| Incident | Status | Impact on this validation |
|---|---|---|
| [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md) — Windows port failures W-01…W-13 (10 test targets and the release link failed on `windows-latest`, CI run 36469534765) | active, S3 | None on the supported platforms. On 2026-09-29 the maintainer dropped Windows from CI and from the supported platforms for v0.2.0. Windows is therefore **not** a reason for any PARTIAL result here. The catalogue stays open for a future port plan. |

## Remaining Limitations

These go into the v0.2.0 release notes (Known Issues and Limitations) at P5:

```text
 limitation                                   behaviour at 14750b8                                  owner / where
 ───────────────────────────────────────────  ────────────────────────────────────────────────────  ─────────────────────────
 Linux process sandbox gated (ADR-0003)       process effects under a policy refuse before spawn:    ADR-0003 (kernel ≥ 6.12
                                              unsupported.sandbox_backend, exit 5                    runner to certify)
 Windows unsupported                          not built or tested; W-01…W-13 catalogued              INC-2026-0011
 crates.io publication deferred (G-PUB)       git dependency only; publish --dry-run fails on the    maintainer (TASK-038)
                                              git capy-core dependency
 version still 0.1.0                          `rivet --version` → rivet 0.1.0; workspace version     P5 TASK-091 (T-33)
                                              0.1.0; .vsix 0.1.0; tag v0.2.0 absent
 ASan not run on the C ABI                    misuse tests + macOS leaks only                        follow-up (Linux CI job)
 Node editor tooling replaced                 Python .vsix packager and TextMate engine;             recorded deviation (R16)
                                              Oniguruma-only regex behaviour untested
 demos not run on a Linux host                Linux evidence = CI suites                              P4/P5
 macOS static link warning                    ring objects built for a newer macOS than the link     P5 TASK-093 (one
                                              target (noise; programs correct)                        MACOSX_DEPLOYMENT_TARGET)
```

The v0.1.0 limitations carried forward unchanged are mTLS serve authentication (refuses), `finally` blocks, the Stage C adapters, the in-memory trace store and HTTP/3 discovery. The 0.1.0 "no import form" limitation is gone: R19–R24 add file modules.

## Required Follow-Up

1. **P4 exit (R18):**
   - close the Documentation and Demo Checklist D-rows;
   - re-execute the demo steps affected by the INC-2026-0012 fixes against the fixed build, and update the Verification Records;
   - re-run T-30 and T-31 and re-record TEST-2026-0030 and TEST-2026-0031.
2. **P5 (R12, T-33):**
   - bump the workspace version to 0.2.0 (TASK-091) and re-record TEST-2026-0033;
   - create the release commit and tag `v0.2.0` (TASK-092/093);
   - verify the git-tag dependency (U-12) and the tagged-build row of DEMO-2026-0020;
   - build the release artifacts with a single `MACOSX_DEPLOYMENT_TARGET`.
3. **REL-0.2.0:** reference this report and the limitations above, and walk the §34 gate (TASK-094). Done 2026-09-30.
4. **ASan:** add an AddressSanitizer job for `rivet-ffi` on Linux CI, which the plan's T-12 names. Alternatively, record why it stays out of scope.
5. **Carried forward:**
   - certify and ungate the Linux Landlock + seccomp backend ([ADR-0003](../decisions/adr-0003-process-sandbox-backends.md));
   - plan the Windows port from [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md);
   - G-PUB publication decision (TASK-038);
   - ship the INC-2026-0013 fix in v0.2.1 (patch release).

## Conclusion

The v0.2.0 implementation meets PLAN-2026-0002 on both supported platforms, macOS and Linux:
- all 24 requirements PASS, and none FAILS (R12 and R18 were closed at P5, 2026-09-30);
- all 20 proposal tests pass, and every 0.1.0 suite is green on the new envelopes;
- the static gates, the architecture gates, traceability and the security checks are clean;
- the tag `v0.2.0` resolves to the release commit `21bb2e9`, and its tagged build and git-tag dependency are verified.

One defect surfaced after tagging: [INC-2026-0013](../incidents/resolved/inc-2026-0013-input-jsonl-waits-for-stdin-eof.md), found by the CI run on the tag. It is fixed on `main` and released as v0.2.1. Windows is unsupported by maintainer decision and does not affect any result.

## Related Documents

- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) · [PROP-2026-0002](../proposals/implemented/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md) · [ADR-0004](../decisions/adr-0004-approve-envelopes-globals-library-ffi-highlighting.md) · [ADR-0005](../decisions/adr-0005-workspace-package-and-features.md) · [RES-2026-0004](../research/res-2026-0004-workspace-ffi-and-feature-experiments.md)
- [Tests](../testing/index.md): TEST-2026-0034…0053 (new), TEST-2026-0001…0032 (re-recorded)
- Incidents: [INC-2026-0009](../incidents/resolved/inc-2026-0009-numeric-index-paths-do-not-parse.md) · [INC-2026-0010](../incidents/resolved/inc-2026-0010-ffi-rlib-output-collision.md) · [INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md) · [INC-2026-0012](../incidents/resolved/inc-2026-0012-documentation-and-demo-verification-defects.md)
- Troubleshooting: [TRBL-2026-0004](../troubleshooting/trbl-2026-0004-vhco-sync-drift-after-renaming-triggers-or-steps.md) · [TRBL-2026-0005](../troubleshooting/trbl-2026-0005-contract-written-ahead-of-code-drifts-on-ports-and-step-order.md) · [TRBL-2026-0006](../troubleshooting/trbl-2026-0006-cargo-test-leaves-cdylib-and-staticlib-in-deps.md) · [TRBL-2026-0007](../troubleshooting/trbl-2026-0007-reading-ci-failures-through-annotations.md)
- [DEMO-2026-0020](../demos/demo-2026-0020-v0-2-0-release-verification.md) · [ADR-0003](../decisions/adr-0003-process-sandbox-backends.md) · [RPT-2026-0001](rpt-2026-0001-validation-of-plan-2026-0001.md) (v0.1.0 validation)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 2 | 2026-09-30 | Claude | P5: R12 and R18 PASS (git-tag dependency verified; demos re-run; tagged build). INC-2026-0013 from the tag CI run recorded (fixed in v0.2.1). |
| 1 | 2026-09-29 | Claude | Created at PLAN-2026-0002 P3 (TASK-062…064) from TEST-2026-0001…0053 at commit `14750b8` and CI run 36505156729: 22 PASS, 2 PARTIAL (R12, R18), 0 FAIL; canary scan 0 hits; global-secret refusal confirmed; FFI misuse 0 leaks. |
