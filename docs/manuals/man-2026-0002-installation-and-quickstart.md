---
document_id: MAN-2026-0002
title: "Rivet installation and quickstart"
document_type: manual
status: active
created_date: 2026-09-28
last_updated: 2026-09-30
document_revision: 6
authors: [Claude, Codex]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [cli, language, registry, policy, audit, serve, http, ffi]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, server]
audience: [developers, operators, new-users]
scope: Install rivet with Cargo (git tag, `--features cli`) or Perch (`perch build` / `perch install`), build librivet and the VS Code `.vsix`, and run the first check, request (0.2.0 envelopes), outputs, io, highlight and serve against the 01-catalog demo, with success and failure examples.
reason: PLAN-2026-0001 row D-35 and PLAN-2026-0002 rows D-21, D-46 — installation manual and first-run journey for the implemented build.
related_documents: [PLAN-2026-0002, MIG-2026-0001, API-2026-0006, MAN-2026-0009, MAN-2026-0010, INC-2026-0011, MAN-2026-0001, MAN-2026-0003, MAN-2026-0004, MAN-2026-0005, MAN-2026-0006, DEMO-2026-0001, PLAN-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, manual, installation, quickstart]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.2.0-rc (source at 6f9943f)"
next_review_date: 2026-10-29
---

# Rivet installation and quickstart

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-30
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** cli, language, registry, policy, audit, serve, http, ffi

## Purpose

Get from nothing to a running operation in about ten minutes: install the binary, compile a bundle, call an
operation, read its declared output, inspect its I/O and serve it over HTTP. Optional: build `librivet` for C,
Python or Go, and install the VS Code extension. Part of the [Rivet manual](man-2026-0001-rivet-manual.md).

The current release is **0.2.0** (tag `v0.2.0`). Examples were captured on its release candidate (2026-09-29,
source `6f9943f`); version fields show the released `0.2.0`.

## Reading Order

```text
 1 Prerequisites ─► 2 Install (cargo install | perch install | cargo build)
                  ─► 2b optional: librivet (C ABI) · VS Code .vsix
                  ─► 3 check ─► 4 request (envelopes) ─► 5 outputs ─► 6 io ─► 7 highlight ─► 8 serve ─► next steps
```

## Prerequisites

| Requirement | Detail |
|---|---|
| Rust toolchain | 1.90.0 (pinned by `rust-toolchain.toml`; `rustup` installs it on first `cargo` use) |
| Platform | **macOS or Linux** (CI green on both). **Windows is not supported** ([INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)). The process sandbox is active only on macOS; on Linux sandboxed spawns are refused with `unsupported.sandbox_backend` (see MAN-2026-0008). |
| Network | Only to fetch crates on the first build (including the pinned Capy git dependency). |
| Disk | A debug build needs several GB under `target/`; see TRBL-2026-0003 if the linker reports "no space left on device". |
| Tools for the examples | `curl`; `shasum` (macOS) or `sha256sum` (Linux) for bearer-token hashes. |
| Optional | a C compiler and `make` (librivet examples), `cbindgen` (only to regenerate `rivet.h`), Python 3 (the `.vsix` packager and the ctypes example), VS Code (`code` on `PATH`). |

## Installation and Setup

### Journey Overview

```text
 cargo install rivet-runtime --git … --tag v0.2.0 --features cli ─┐
 perch install  (from a checkout)                                 ├─► rivet --version ─► "rivet 0.2.0"
 cargo build --release --features cli (from a checkout)          ─┘
        |
        +-> "warning: none of the package's binaries are available …" -> you left out --features cli -> add it
        +-> compile error / linker "no space left" -> free disk (TRBL-2026-0003) -> retry
```

### Install with Cargo from the git tag

From 0.2.0 the package is **`rivet-runtime`** and the `rivet` binary is behind the **`cli` Cargo feature**
(the library does not need clap). There is no crates.io crate yet, so install from the git tag:

```sh
cargo install rivet-runtime --git https://github.com/olivierdevelops/rivet --tag v0.2.0 --features cli
rivet --version                      # rivet 0.2.0
```

Without `--features cli` Cargo builds only the library and installs no binary (captured with `--path .`):

```text
warning: none of the package's binaries are available for install using the selected features
  bin "rivet" requires the features: `cli`
…
Consider enabling some of the needed features by passing, e.g., `--features="cli"`
```
 The default features (`serve`,
`grpc`, `quic`, `oauth`) are included; a lean binary is
`--no-default-features --features cli` (then `rivet serve` exits 5 with `unsupported.feature`, and bundles that use
gRPC, QUIC/HTTP/3 or OAuth fail at load — MAN-2026-0008). To go back to 0.1.0: `cargo install --git
https://github.com/olivierdevelops/rivet --tag v0.1.0 --force` (0.1.0 needs no `--features`;
[MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md#decision-or-outcome)).

### Perch build and installation

The repository's [`commands.perch`](../../commands.perch) wraps the source-build commands. With Perch on
`PATH`, run from the checkout root:

```sh
perch --help
perch build             # cargo build --locked --release --features cli --bin rivet
perch build_debug       # cargo build --locked --features cli --bin rivet
perch install           # build release (--features cli), then bman add "<absolute binary path>"
perch ffi               # build librivet, verify rivet.h, run the C and Python examples (see below)
rivet --version         # rivet 0.2.0
```

The install task requires `bman` on `PATH`. It builds the optimized binary into this checkout's `target/`
with `cargo build --locked --release --features cli --bin rivet --target-dir "<checkout>/target"`, then invokes
`bman add "<checkout>/target/release/rivet"` (the command file still carries an `${exe_ext}` suffix for Windows,
which is no longer a supported platform). The quoted absolute path works when
the checkout path contains spaces. A failed build prevents installation; bman errors fail the task.

Bman manages the global bin directory; run `bman help` to see its location and ensure that directory is on
`PATH`. On the verified machine it is `/Users/oliverlaleau/Documents/bin`. If an older Rivet installation
appears earlier on `PATH`, your shell will continue to select that copy. Other build tasks respect
`CARGO_TARGET_DIR`; install explicitly selects the checkout's `target/`. The first build may fetch dependencies.

```text
perch install -> cargo build --release --features cli --target-dir <checkout>/target
              -> bman add "<checkout>/target/release/rivet" -> bman's global bin directory
```

From outside the checkout, use `perch -f /path/to/rivet/commands.perch build`; tasks run from the command
file's directory. `perch help_cli` builds and prints Rivet help. `perch main` lists tasks, while bare
`perch` prints Perch's own usage. The [contributor guide](../onboarding/onb-2026-0001-contributor-setup.md#perch-development-commands)
lists test, lint, documentation and architecture tasks.

### CLI Procedure

From a checkout (the repository root is a Cargo workspace: `rivet-runtime` at the root, `rivet-ffi` in `ffi/`):

```bash
cd /path/to/rivet                          # contains Cargo.toml
cargo build --release --features cli       # → target/release/rivet
target/release/rivet --version
```

Expected output on the release-candidate tree (the version is bumped to 0.2.0 by the release commit):

```text
rivet 0.2.0
```

`cargo build --features cli` gives a debug build in `target/debug/rivet`. Put the binary on your `PATH` if you like
(the rest of this manual writes plain `rivet`):

```bash
export PATH="$PWD/target/release:$PATH"
rivet --help
```

`rivet --help` lists the commands:

```text
Commands:
  request     Invoke one operation
  list        List public operations
  describe    Describe operations: params, output, errors, source
  outputs     Show declared outputs, emits, receives and errors
  check       Compile and check the bundle without running anything
  io          Generate the I/O manifest (every I/O site, target and access verb)
  graph       Static call graph of one operation: literal calls (expanded), connector calls, effect sites, DAG nodes with after edges, both `if` arms marked
  policy      Policy tools
  serve       Serve every surface (REST, SSE, polling, WebSocket, MCP) on one listener
  trace       Request traces recorded by this host
  connectors  Outbound MCP connectors
  auth        OAuth account management (rivet.auth.* built-ins); tokens are never printed
  highlight   Syntax-highlight a .rivet file from the parser's own spans (no bundle is loaded; --file is not needed). A syntax error prints the tokens before it and the diagnostic (exit 2)
  help        Print this message or the help of the given subcommand(s)
```

There is no crates.io crate, installer or prebuilt binary. To use Rivet as a Rust library, depend on the
git tag (`rivet = { package = "rivet-runtime", git = "https://github.com/olivierdevelops/rivet", tag = "v0.2.0" }`,
MAN-2026-0007).

### Optional: build librivet (C, Python, Go)

```text
 checkout ─▶ cargo build --release -p rivet-ffi ─▶ target/release/librivet.{a, dylib|so} + ffi/include/rivet.h
                                                  └─▶ cc app.c -I ffi/include -L target/release -lrivet …
```

```sh
cargo build --release -p rivet-ffi
ls target/release/librivet.*          # macOS: librivet.a librivet.dylib    Linux: librivet.a librivet.so
make -C examples/c test               # builds and runs the C examples (shared and static)
python3 examples/python/demo.py       # the ctypes wrapper
```

`perch ffi` runs the same steps and also verifies `rivet.h` against cbindgen. Linking flags per OS, `rivet.pc`,
threading, ownership and the module API are in [MAN-2026-0009](man-2026-0009-c-abi-and-ffi.md).

### Optional: install the VS Code extension

```sh
python3 editors/vscode/package_vsix.py          # writes dist/rivet-<version>.vsix (Python only; no Node.js)
code --install-extension dist/rivet-0.2.0.vsix  # rivet-0.1.0.vsix on the release-candidate tree
```

Other TextMate editors use `editors/rivet.tmLanguage.json`; see
[MAN-2026-0010](man-2026-0010-editor-support-and-highlighting.md).

## Task-Oriented Workflows

All commands below run from `docs/demos/01-catalog/`, whose `app.rivet` declares four pure operations
(`demo.greet`, `demo.add`, `demo.health`, `demo.countdown`) and has **no** `policy.json`. They were captured on
2026-09-29 (macOS 26.4) from the 0.2.0 release candidate. **Request and trace IDs such as `req_018f8b925d` are
generated per run and differ on your machine.**

```bash
cd docs/demos/01-catalog
```

### 1. Compile and check the bundle

Why: catch syntax and composition errors without running anything.

```bash
rivet check --file app.rivet
echo $?
```

```text
ok: 4 operations, 0 connectors, 0 auth profiles
0
```

Failure example — `--file` is required for every local command:

```bash
rivet check
```

```text
error[validation.usage]: --file PATH is required (the entry .rivet file)
```

Exit code 2. Failure example — a syntax error (here `else if`, which the language does not have: nest an `if`
inside the `else` instead) is reported with its line, a caret and a hint; exit code 2:

```text
error[syntax.else_if]: `else` takes no condition; `else if COND` is not supported
  --> bad.rivet:6:5
   |
  6|     if n > 0
   |     ^^^^^^^^
  = hint: put `else` alone on its line and nest `if COND … end` inside its body
error[syntax.else_without_if]: `else` must follow the body of an `if` at the same indentation (one `else` per `if`, closed by the `if`'s `end`)
  --> bad.rivet:8:5
   |
  8|     else if n < 0
   |     ^^^^
  = hint: write `if COND` … `else` … `end`; for else-if, nest an `if COND … end` inside the `else` body
```

`check` may also print `warning: …` lines (an undeclared `fail` code, an unguarded DAG result) and still exit 0;
see [MAN-2026-0004 §check](man-2026-0004-cli-reference.md#rivet-check).

### 2. List and call an operation

```bash
rivet list --file app.rivet
rivet request --file app.rivet demo.add --data '{"a":2,"b":3}'
```

```text
ID              NAME                DESCRIPTION
demo.greet      Greet a person      Return a greeting for the supplied person.
demo.add        Add two integers    Add two signed integers and return their sum.
demo.health     Check availability  Return a constant readiness response without I/O.
demo.countdown  Count down          Emit 3, 2, 1 as data items and then return a summary.
{"request_id":"req_018f8b925d","trace_id":"tr_018f8b925d","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}
```

The answer is a **response envelope** ([API-2026-0006](../api/api-2026-0006-envelopes.md)): `status: "ok"`, the
value in `data`, `error: null`, and `effects: "none"` (nothing outside Rivet changed). The same JSON comes back
from HTTP, WebSocket, MCP, Rust and C.

```text
 envelope ─▶ status ──┬─ "ok"        → use .data                         (stdout, exit 0)
                      ├─ "error"     → .error.code / .error.message      (stderr, exit 2–6)
                      └─ "cancelled" → .error.code (cancelled.*)          (stderr, exit 130)
```

Add `--pretty` to read it by eye:

```text
$ rivet request --file app.rivet demo.add --data '{"a":2,"b":3}' --pretty
{
  "request_id": "req_018e3dea25",
  "trace_id": "tr_018e3dea25",
  "operation": "demo.add",
  "type": "result",
  "status": "ok",
  "data": 5,
  "error": null,
  "effects": "none",
  "data_count": 0
}
```

Failure examples — an error envelope goes to **stderr** (stdout stays empty); exit code in brackets:

```text
$ rivet request --file app.rivet demo.add --data '{"a":"x"}'                            [2]
{"request_id":"req_018dfc02c5","trace_id":"tr_018dfc02c5","operation":"demo.add","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.type","message":"parameter `a` must be an integer, got text","retryable":false,"operation_id":"demo.add","details":{"field":"a"}},"effects":"none","data_count":0}

$ rivet request --file app.rivet demo.nope                                               [4]
{"request_id":"req_018c2ac4e5","trace_id":"tr_018c2ac4e5","operation":"demo.nope","type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"not_found.operation","message":"no operation `demo.nope`","retryable":false,"operation_id":"demo.nope"},"effects":"none","data_count":0}
```

Coming from 0.1.0? `--params` still works in 0.2.x but prints `warning[deprecated.params]: --params is deprecated;
use --data (removed in 0.3.0)` on stderr; see [MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md).

Streaming operation, printed as NDJSON records with `--stream` (data records, then one `type: "result"` record):

```bash
rivet request --file app.rivet demo.countdown --stream
```

```text
{"request_id":"req_018cb46e25","trace_id":"tr_018cb46e25","operation":"demo.countdown","type":"data","seq":1,"data":3,"error":null}
{"request_id":"req_018cb46e25","trace_id":"tr_018cb46e25","operation":"demo.countdown","type":"data","seq":2,"data":2,"error":null}
{"request_id":"req_018cb46e25","trace_id":"tr_018cb46e25","operation":"demo.countdown","type":"data","seq":3,"data":1,"error":null}
{"request_id":"req_018cb46e25","trace_id":"tr_018cb46e25","operation":"demo.countdown","type":"result","seq":4,"status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}
```

Without `--stream` only the terminal record is printed:
`{"request_id":"req_018bd7c975",…,"operation":"demo.countdown","type":"result","status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}`.

### 3. Read the declared outputs

```bash
rivet outputs --file app.rivet demo.countdown
```

```text
demo.countdown — Count down
output  object   Summary returned after the last item.
  count   integer  required  Number of items emitted.
emits   integer  One countdown value per item.
receives —
errors   —
```

`rivet outputs --file app.rivet demo.countdown --json` prints the same as JSON Schema; `--all` shows every
operation.

### 4. Inspect I/O before running anything

```bash
rivet io --file app.rivet
rivet policy --file app.rivet explain
```

```text
OPERATION  KIND  ACCESS  TARGET  KNOWLEDGE  SOURCE
(no I/O sites)
policy   none — no policy.json: every new application effect is denied (pure operations still run)
base     .
network  deny_private_ranges true
limits   64 concurrent, depth 16, 268435456 buffered bytes
```

This bundle is pure, so nothing needs a grant. For a bundle that touches files, run the same commands in
`docs/demos/02-file-crud/` or `docs/demos/11-sandbox/` and continue with MAN-2026-0005.

### 5. Highlight a source file

`rivet highlight` colours a `.rivet` file from the parser's own spans (no bundle is loaded, `--file` is not
needed). On a terminal the default is ANSI colour; piped, it prints one JSON token per line:

```text
$ rivet highlight app.rivet --format json | head -3
{"line":1,"col":1,"len":9,"class":"keyword","text":"operation"}
{"line":1,"col":11,"len":10,"class":"operation_id","text":"demo.greet"}
{"line":2,"col":5,"len":4,"class":"keyword","text":"name"}
$ rivet highlight app.rivet --format html | head -1
<pre class="rv-source"><code><span class="rv-keyword">operation</span> <span class="rv-operation_id">demo.greet</span>
```

More in [MAN-2026-0010](man-2026-0010-editor-support-and-highlighting.md).

### 6. Serve every surface

```text
 terminal A                                   terminal B
 ──────────                                   ──────────
 rivet serve --file app.rivet \
   --listen 127.0.0.1:18960
   stderr: {"listen_addr":…,"surfaces":[…]}
                                   ◄───────── curl -X POST …/v1/request {"operation":…,"data":…}
   (serves until Ctrl-C)           ─────────► {…,"status":"ok","data":5,…}
```

Terminal A:

```bash
rivet serve --file app.rivet --listen 127.0.0.1:18960
```

stderr receipt (one line):

```text
{"listen_addr":"127.0.0.1:18960","stdio":false,"surfaces":["http","sse","poll","ws","mcp"],"auth_type":"none","catalog_version":"sha256:67104f0e7faaeafee253db758a668a9b24aa4263677e9d5ea2451b32019f9730","policy_hash":null}
```

Terminal B:

```bash
curl -sS -X POST http://127.0.0.1:18960/v1/request \
  -H 'content-type: application/json' \
  -d '{"operation":"demo.add","data":{"a":2,"b":3}}'
rivet --endpoint http://127.0.0.1:18960 request demo.add --data '{"a":4,"b":5}'
curl -s http://127.0.0.1:18960/v1/health
```

```text
{"request_id":"req_01e5bcce1d","trace_id":"tr_01e5bcce1d","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}
{"request_id":"req_02653d9c0a","trace_id":"tr_02653d9c0a","operation":"demo.add","type":"result","status":"ok","data":9,"error":null,"effects":"none","data_count":0}
{"request_id":"req_03e15ec337","trace_id":"tr_03e15ec337","operation":"rivet.health","type":"result","status":"ok","data":{"status":"ok","catalog_version":"sha256:67104f0e7faaeafee253db758a668a9b24aa4263677e9d5ea2451b32019f9730","version":"0.2.0"},"error":null,"effects":"none","data_count":0}
```

While it runs, terminal A shows one JSON access-log line per request after the receipt:

```text
{"time":"2026-09-28T21:53:43.046Z","surface":"http","method":"POST","route":"/v1/request","principal":"local","operation":"demo.add","status":200,"duration_ms":2}
{"time":"2026-09-28T21:53:43.057Z","surface":"http","method":"POST","route":"/v1/request","principal":"local","operation":"demo.add","status":200,"duration_ms":0}
{"time":"2026-09-28T21:53:43.072Z","surface":"http","method":"GET","route":"/v1/health","principal":null,"operation":"rivet.health","status":200,"duration_ms":0}
```

Stop the server with Ctrl-C (or SIGTERM) in terminal A: it drains in-flight work and exits 0.

Failure example — listening on a non-loopback address without authentication is refused before binding:

```bash
rivet serve --file app.rivet --listen 0.0.0.0:18956
```

```text
{"request_id":"","trace_id":"","operation":"rivet.serve","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"serve.auth_required","message":"non-loopback listener requires serve.auth in policy.json","retryable":false},"effects":"none","data_count":0}
```

Exit code 2. Recovery: add bearer tokens to `policy.json` (MAN-2026-0006).

### Expected Result and Side Effects

| Step | Side effects |
|---|---|
| `cargo install` / `cargo build` / `perch install` | writes `target/` (or Cargo's install dir); downloads crates on first run; `perch install` adds the binary through `bman` |
| `cargo build -p rivet-ffi`, `package_vsix.py` | write `target/release/librivet.*` / `dist/rivet-<version>.vsix` |
| `highlight` | none (reads the file) |
| `check`, `list`, `describe`, `outputs`, `io`, `policy explain` | none (read the bundle and policy only) |
| `request` of a pure operation | none (`effects: "none"`) |
| `serve` | binds the TCP port until stopped; keeps sessions and traces in memory only |

### Verified Demo

[01-catalog (DEMO-2026-0001)](../demos/01-catalog/README.md). Every quickstart command above was run on 2026-09-29
against the 0.2.0 release candidate (`cargo build --release --features cli`, source `6f9943f`) from that folder on
macOS; `cargo install … --tag v0.2.0` becomes runnable when P5 publishes the tag. Editor and FFI demos:
`docs/demos/16-editor/` and `docs/demos/15-ffi/` (being added).

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `warning: none of the package's binaries are available for install` from `cargo install` | `--features cli` missing (library only) | re-run with `--features cli` |
| `warning[deprecated.params]` on stderr | a 0.1.0 script uses `--params` | switch to `--data` |
| `unsupported.feature` (exit 5) | a lean build lacks `serve`/`grpc`/`quic`/`oauth` | rebuild with default features |
| `error[validation.usage]: --file PATH is required` | local command without a bundle | add `--file app.rivet` |
| `error[not_found.source]: cannot read missing.rivet` (exit 4) | wrong path | check the path relative to your shell's directory |
| `error[policy.invalid]: --policy /x.json: no such file` (exit 2) | `--policy` names a missing file | fix the path; omit `--policy` to use discovery |
| `serve.auth_required` (exit 2) | non-loopback `--listen` with no `serve.auth` | bind `127.0.0.1` or configure bearer auth |
| `--listen nothost: use HOST:PORT with an IP address or localhost` (exit 2) | malformed listen address | use `127.0.0.1:PORT` |

## Limitations

- Build from source (Cargo, Perch) only; no crates.io crate, installer or prebuilt binary.
- macOS and Linux only; Windows is not supported ([INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)).
- See the [root manual's known limitations](man-2026-0001-rivet-manual.md#known-limitations).

## Version Applicability

| Feature / Interface | Introduced | Changed | Deprecated / Removed | Applicable Environment |
|---|---|---|---|---|
| Source build with Cargo | 0.1.0 | 0.2.0: package `rivet-runtime`, binary needs `--features cli` | — | macOS, Linux |
| Perch `build` / `install` | 0.1.0 | 0.2.0: `--features cli`; `perch ffi` | — | development |
| librivet, `.vsix` | 0.2.0 | — | — | macOS, Linux; VS Code |
| Quickstart commands | 0.1.0 | 0.2.0: envelopes, `--data`, `--pretty`, `highlight` | `--params` deprecated 0.2.0 | development, server |

## Related Documents

- [Rivet manual](man-2026-0001-rivet-manual.md) · [Language guide](man-2026-0003-language-guide.md) ·
  [CLI reference](man-2026-0004-cli-reference.md) · [Policy guide](man-2026-0005-policy-and-io-manifest-guide.md) ·
  [Serving](man-2026-0006-serving-and-surfaces.md)
- [C ABI and FFI](man-2026-0009-c-abi-and-ffi.md) · [Editor support](man-2026-0010-editor-support-and-highlighting.md) · [MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md) · [API-2026-0006](../api/api-2026-0006-envelopes.md)
- [Plan PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) · [Plan PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 6 | 2026-09-30 | Claude | v0.2.0 release (PLAN-2026-0002 TASK-091): version strings and current-release wording updated to 0.2.0. |
| 5 | 2026-09-29 | Claude | 0.2.0 (D-21, D-46): `cargo install rivet-runtime --git … --tag v0.2.0 --features cli`; Perch tasks with `--features cli` and `perch ffi` (Perch content kept); optional librivet build and `.vsix` install; quickstart re-captured with envelopes, `--data`, `--pretty`, stderr errors, stream records, `highlight`, serve on 18960 with health and access log; troubleshooting and version rows; macOS/Linux platforms, Windows unsupported. |
| 4 | 2026-09-28 | Claude | Fix batch through 829ca43 and 2a751ab: the syntax-error example now shows `syntax.else_if` (`if … else … end` exists), `check` warnings, access log, `/v1/health` and SIGTERM drain, the `emits` line shows its description; limitations link updated. Perch content unchanged. |
| 3 | 2026-09-28 | Codex | Changed Perch installation to a release build followed by bman add, as requested by the maintainer. |
| 2 | 2026-09-28 | Codex | Added Perch release/debug build and Cargo installation workflows, root selection and command discovery. |
| 1 | 2026-09-28 | Claude | Initial installation and quickstart for 0.1.0, verified against 0.1.0-dev commit f40d4aa. |
