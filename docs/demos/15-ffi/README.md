---
document_id: DEMO-2026-0017
title: "Rivet from C and Python through librivet"
document_type: demo
status: active
created_date: 2026-09-29
last_updated: 2026-09-29
document_revision: 3
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [ffi, library, cli]
affected_versions:
  from: "0.2.0"
  to: null
applicable_environments: [development, embedded]
audience: [developers, integrators, reviewers]
scope: Build librivet (shared and static), check its exported symbols, link the shipped C example both ways, run a request, a stream, live input, cancellation and module calls, call the same library from Python through ctypes, and show argument misuse returning error envelopes.
reason: PROP-2026-0002 UC-06 (UQ-08) and UC-11 (R23); PLAN-2026-0002 D-18, TASK-075.
dependencies: [PROP-2026-0002, PLAN-2026-0002]
related_documents: [PROP-2026-0002, PLAN-2026-0002, DEMO-2026-0020, DEMO-2026-0019, API-2026-0007, MAN-2026-0009, SYS-2026-0010, API-2026-0006]
supersedes: null
superseded_by: null
tags: [rivet, demo, ffi, c-abi, python, ctypes]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-29
verified_against: "0.2.0"
---

# Rivet from C and Python through librivet

> **Status:** Active
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0 and later
> **Owner:** Project maintainer
> **Affected Components:** ffi, library, cli

## Purpose

`librivet` is Rivet's C ABI: a shared library (`librivet.dylib` / `librivet.so`), a static archive (`librivet.a`) and one header, [ffi/include/rivet.h](../../../ffi/include/rivet.h). Calls are JSON in and JSON out. The input is the same InputEnvelope as on every other surface, and every returned string is the same ResponseEnvelope or stream record.

```text
  C / Python / any FFI host                      librivet
  ─────────────────────────                      ─────────────────────────────────────────────
  rivet_runtime_new({"file":"app.rivet"})  ───▶  RivetRuntime (thread-safe)
  rivet_request(rt, InputEnvelope)         ───▶  char* ResponseEnvelope ──▶ rivet_string_free
  rivet_call_start(rt, InputEnvelope)      ───▶  RivetCall (pull handle, one thread at a time)
     rivet_call_next(call, timeout_ms)     ───▶  record … {"type":"timeout"} … NULL at the end
     rivet_call_send / _finish_input       ───▶  SessionAck
     rivet_call_cancel                     ───▶  terminal record, status "cancelled"
  rivet_load(rt, path, alias, &module)     ───▶  RivetModule ─▶ rivet_module_call / _operations
  rivet_highlight(source, format)          ───▶  tokens (no runtime needed)
```

```text
  RivetCall lifecycle
  start ──▶ open ──next──▶ data … ──▶ terminal record ──next──▶ NULL ──▶ rivet_call_free
              │  send / finish_input (live input)
              └─ cancel ──▶ terminal record status "cancelled" ──▶ NULL
```

[app.rivet](app.rivet) has three pure operations, one per call style:

| Operation ID | Style | Shows |
|---|---|---|
| `demo.add` | unary | `rivet_request`, `"pretty": true`, a validation error |
| `events.count` | server stream (`emits`) | `rivet_call_start` + `rivet_call_next` |
| `chat.echo` | live input (`receives`) | `rivet_call_send`, `rivet_call_finish_input`, `rivet_call_cancel` |

The programs are the shipped examples [examples/c/demo.c](../../../examples/c/demo.c), [examples/c/modules.c](../../../examples/c/modules.c) and [examples/python/demo.py](../../../examples/python/demo.py) (wrapper [rivet.py](../../../examples/python/rivet.py)). [misuse.c](misuse.c) in this folder is the failure example.

## Verified Against Version

0.2.0. Verified on 0.2.0-dev at commit `8031baa`, the release candidate, on macOS 26.4.1 (Darwin 25.4.0, arm64), 2026-09-29, with Apple clang (`cc`), Python 3.9.6 and the release `librivet`. `rivet_version()` still returns `0.1.0` until the P5 bump; `rivet_abi_version()` is 1. Linux (`librivet.so`, its `Libs.private` list, pkg-config) is verified by CI, not on this host. Windows is not supported (INC-2026-0011). Request, trace and session IDs vary.

## Prerequisites

- A Rust toolchain, a C compiler (`cc`), `make`, and `python3` (standard library `ctypes` only).
- On Linux, `pkg-config` for the static link line.

## Setup

Build the library from the repository root (the Makefile also builds it when it is missing):

```sh
cargo build --release -p rivet-ffi
ls -l target/release/librivet.*
nm -gU target/release/librivet.dylib | awk '{print $3}' | sort    # Linux: nm -D --defined-only …/librivet.so
```

Expected: `librivet.a` (about 59 MiB, dependencies included), `librivet.dylib` (about 15 MiB) and 18 exported symbols, all prefixed `rivet_`:

```text
_rivet_abi_version _rivet_call_cancel _rivet_call_finish_input _rivet_call_free _rivet_call_next _rivet_call_send
_rivet_call_start _rivet_highlight _rivet_load _rivet_module_call _rivet_module_call_start _rivet_module_free
_rivet_module_operations _rivet_request _rivet_runtime_free _rivet_runtime_new _rivet_string_free _rivet_version
```

Then work from this folder, with C binaries in a scratch directory:

```sh
cd docs/demos/15-ffi
OUT="$(mktemp -d)"
```

## Steps

### 1. Link shared and static

#### Command / Request

```sh
make -C ../../../examples/c OUT="$OUT" "$OUT/demo"
MACOSX_DEPLOYMENT_TARGET=26.4 make -C ../../../examples/c OUT="$OUT" "$OUT/demo_static"
otool -L "$OUT/demo" "$OUT/demo_static"          # Linux: ldd
```

The Makefile's link lines:

```text
  shared (macOS):  cc demo.c -I ffi/include -L target/release -lrivet -Wl,-rpath,<abs>/target/release
  static (macOS):  cc demo.c -I ffi/include target/release/librivet.a \
                      -framework Security -framework CoreFoundation -liconv -lc -lm
  static (Linux):  cc demo.c $(pkg-config --cflags rivet) $(pkg-config --libs --static rivet)
                   Libs.private = -lgcc_s -lutil -lrt -lpthread -lm -ldl -lc
```

`python3 ../../../ffi/render_pc.py --prefix PREFIX > rivet.pc` renders the pkg-config file for the host OS. On this Mac it printed `Version: 0.1.0`, `Libs: -L${libdir} -lrivet` and `Libs.private: -framework Security -framework CoreFoundation -liconv -lc -lm`.

#### Expected Output / Response

```text
$OUT/demo:
	@rpath/librivet.dylib (compatibility version 0.0.0, current version 0.0.0)
	/usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1356.0.0)
$OUT/demo_static:
	/System/Library/Frameworks/Security.framework/Versions/A/Security (…)
	/System/Library/Frameworks/CoreFoundation.framework/Versions/A/CoreFoundation (…)
	/usr/lib/libiconv.2.dylib (…)
	/usr/lib/libSystem.B.dylib (…)
```

The static binary (about 24 MiB) has no `librivet` dependency. Without `MACOSX_DEPLOYMENT_TARGET`, macOS `ld` prints 21 warnings of the form `object file (…librivet.a[…]) was built for newer 'macOS' version (26.4) than being linked (26.0)`, for the TLS library's assembly objects. The program is still correct (PLAN-2026-0002 finding TASK-042).

### 2. Request, stream, live input and cancel from C

#### Command / Request

```sh
"$OUT/demo" ./app.rivet
"$OUT/demo_static" ./app.rivet
```

#### Expected Output / Response

Both binaries print the same lines and exit 0. The unary part (a bad option, a request, the same request with `"pretty": true`, and a type error):

```text
abi 1 version 0.1.0
options-error {…"status":"error","data":null,"error":{"kind":"validation","code":"validation.ffi_argument","message":"unknown option `colour` (expected one of file, source, path, root, policy_file, policy_json, ceiling_json, pretty)",…}…}
request {"request_id":"req_01fb94754d","trace_id":"tr_01fb94754d","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}
pretty {
  "request_id": "req_027a711902",
  "trace_id": "tr_027a711902",
  "operation": "demo.add",
  "type": "result",
  "status": "ok",
  "data": 42,
  "error": null,
  "effects": "none",
  "data_count": 0
}
invalid {…"operation":"demo.add","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.type","message":"parameter `a` must be an integer, got text",…}…}
```

The stream: three `type: data` records, then one terminal record, then `rivet_call_next` returns NULL:

```text
record {"request_id":"req_0479c1be14","trace_id":"tr_0479c1be14","operation":"events.count","type":"data","seq":1,"data":1,"error":null}
record {…"type":"data","seq":2,"data":2,…}
record {…"type":"data","seq":3,"data":3,…}
record {…"operation":"events.count","type":"result","seq":4,"status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}
stream-records 4
```

Live input: two sends, finish, then the echoed items and the count:

```text
send {"session_id":"ses_027a7940d2","accepted_seq":1,"input_closed":false}
send {"session_id":"ses_027a7940d2","accepted_seq":2,"input_closed":false}
finish {"session_id":"ses_027a7940d2","accepted_seq":null,"input_closed":true}
record {…"operation":"chat.echo","type":"data","seq":1,"data":"hi",…}
record {…"operation":"chat.echo","type":"data","seq":2,"data":"there",…}
record {…"operation":"chat.echo","type":"result","seq":3,"status":"ok","data":2,…}
chat-records 3
```

Cancellation: after one item, a 50 ms poll returns `{"type":"timeout"}`, and `rivet_call_cancel` ends the call with one `cancelled` record:

```text
first {…"operation":"chat.echo","type":"data","seq":1,"data":"ping",…}
poll {"type":"timeout"}
record {…"type":"result","seq":2,"status":"cancelled","data":null,"error":{"kind":"cancelled","code":"cancelled.session","message":"the session was cancelled","retryable":false},"effects":"none","data_count":1}
cancel-records 1
highlight {"line":1,"col":1,"len":6,"class":"keyword","text":"global"}
free 0
free-again 1
```

`free 0` is `RIVET_OK`. Freeing the same runtime again returns `RIVET_ERROR` (1) instead of corrupting memory.

### 3. Module objects from C (`rivet_load`)

`examples/c/modules.c` starts a runtime rooted in `examples/modules/` and loads `users.rivet` and `billing.rivet` as
module objects. It lists and calls their operations, and shows two refusals: a failed validation and loading an
alias twice. Both refusals carry request and trace IDs; the refused load (`rivet.load`) keeps the registry kind `syntax`
(INC-2026-0012).

```sh
make -C examples/c all        # also builds modules and modules_static
cd examples/c && ./modules
```

```text
operations [{"id":"get","operation":"users.get","name":"users.get","description":"One user by id.","emits":false,"receives":false},{"id":"list","operation":"users.list","name":"users.list","description":"The first two users (calls inside a module use its own IDs).","emits":false,"receives":false}]
call {"request_id":"req_01ab71dad5","trace_id":"tr_01ab71dad5","operation":"users.get","type":"result","status":"ok","data":{"id":42,"name":"Hello, user 42"},"error":null,"effects":"none","data_count":0}
bad-call {"request_id":"req_022af25eba","trace_id":"tr_022af25eba","operation":"users.get","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.min","message":"parameter `id` must be ≥ 1","retryable":false,"operation_id":"users.get"},"effects":"none","data_count":0}
call {"request_id":"req_03a97828c7","trace_id":"tr_03a97828c7","operation":"billing.invoice","type":"result","status":"ok","data":{"user":{"id":7,"name":"Hello, user 7"},"amount":20.0},"error":null,"effects":"none","data_count":0}
duplicate {"request_id":"req_0429937b74","trace_id":"tr_0429937b74","operation":"rivet.load","type":"result","status":"error","data":null,"error":{"kind":"syntax","code":"check.import_duplicate","message":"a module named `users` is already loaded; use load_as(path, alias)","retryable":false},"effects":"none","data_count":0}
record {"request_id":"req_05a8b4c5e1","trace_id":"tr_05a8b4c5e1","operation":"users.list","type":"result","seq":1,"status":"ok","data":[{"id":1,"name":"Hello, user 1"},{"id":2,"name":"Hello, user 2"}],"error":null,"effects":"none","data_count":0}
request {"request_id":"req_062f76e80e","trace_id":"tr_062f76e80e","operation":"users.get","type":"result","status":"ok","data":{"id":7,"name":"Hello, user 7"},"error":null,"effects":"none","data_count":0}
module-free-again 1
```

```text
 rivet_load(rt, "./users.rivet", NULL) ──▶ RivetModule* (alias "users")
      │ rivet_module_operations ─▶ ["get","list"] as JSON
      │ rivet_module_call(m, "get", {"id":42}) ─▶ envelope, operation "users.get"
      │ rivet_module_call_start(m, "list", {}) ─▶ RivetCall* ─▶ records ─▶ NULL
      └ rivet_module_free(m) ─▶ 0 ; again ─▶ 1 (the module stays loaded in the runtime)
```

Operations are namespaced by alias (`users.get`), as with `import … as users` in `.rivet` (DEMO-2026-0019).

### 4. The same library from Python (ctypes)

`examples/python/rivet.py` is a small ctypes wrapper, shipped as an example rather than a package. It exposes a
runtime, and modules whose attributes are their operations.

```sh
cd examples/python
RIVET_LIB=../../target/release/librivet.dylib python3 demo.py      # librivet.so on Linux
RIVET_LIB=../../target/release/librivet.dylib python3 modules.py
```

```text
abi 1 version 0.1.0
options-error validation.ffi_argument
request {"request_id": "req_…", "trace_id": "tr_…", "operation": "demo.add", "type": "result", "status": "ok", "data": 5, "error": null, "effects": "none", "data_count": 0}
invalid error validation.type
stream [1, 2, 3, {'count': 3}] result ok
send 1
send 2
chat ['hi', 'there', 2]
```

```text
operations ['get', 'list']
get users.get {"id": 42, "name": "Hello, user 42"}
list [1, 2]
invoice {"user": {"id": 7, "name": "Hello, user 7"}, "amount": 20.0}
duplicate check.import_duplicate
no-such-operation module 'users' has no operation 'missing'
request Hello, user 7
```

`version 0.1.0` is the pre-release version of the release-candidate build. P5 sets it to 0.2.0.

### 5. Failing examples: argument misuse returns an envelope

`misuse.c` passes a NULL runtime, a NULL input, invalid UTF-8 and truncated JSON, then frees a runtime twice. Each
call returns an error envelope or a status code; none of them crashes.

```sh
cd docs/demos/15-ffi
cc misuse.c -I ../../../ffi/include -L ../../../target/release -lrivet \
   -Wl,-rpath,"$PWD/../../../target/release" -o misuse && ./misuse
```

```text
null-runtime {"request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.ffi_argument","message":"the RivetRuntime is NULL","retryable":false,"details":{"argument":"RivetRuntime"}},"effects":"none","data_count":0}
null-input {…"error":{"kind":"validation","code":"validation.ffi_argument","message":"`input_json` is NULL",…}…}
bad-utf8 {…"error":{"kind":"validation","code":"validation.ffi_argument","message":"`input_json` is not valid UTF-8: invalid utf-8 sequence of 1 bytes from index 19",…}…}
bad-json {…"error":{"kind":"validation","code":"validation.input_envelope","message":"the input envelope is not valid JSON: EOF while parsing a value at line 1 column 13",…}…}
missing-file {…"error":{"kind":"not_found","code":"not_found.source","message":"cannot read nope.rivet: No such file or directory (os error 2)",…}…}
free 0
free-again 1
```

| Misuse | Result |
|---|---|
| NULL handle or string | `validation.ffi_argument` envelope (`details.argument` names it) |
| Invalid UTF-8 | `validation.ffi_argument` |
| Malformed input JSON | `validation.input_envelope` |
| Missing bundle file | `not_found.source` in `error_json` |
| Second free of the same handle | `RIVET_ERROR` (1) |

## Release Updates

| Update | Requirement | What to check | Result |
|---|---|---|---|
| U-13 | R13 | 18 `rivet_*` symbols; shared and static links run | PASS |
| U-14 | R14 | NULL, UTF-8, JSON and double-free misuse returns envelopes or status codes | PASS |
| U-15 | R15 | Stream, live input and cancel through a `RivetCall` | PASS |
| U-23 | R23 | `rivet_load`, module calls and the Python module object | PASS |

## Cleanup

```sh
make -C examples/c clean
rm -f docs/demos/15-ffi/misuse
```

## Verification Record

| Step | Verified by | When / build / OS | Result |
|---|---|---|---|
| Setup: build, 18 symbols | Claude (TASK-075) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 1. Shared and static link | Claude (TASK-075) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 2. Request, stream, input, cancel | Claude (TASK-075) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 3. Module objects from C | Claude (coordinator) | 2026-09-29, commit 166a98b, macOS 26.4.1 arm64 | PASS |
| 4. Python ctypes | Claude (coordinator) | 2026-09-29, commit 166a98b, macOS 26.4.1 arm64 | PASS |
| 5. Misuse | Claude (coordinator) | 2026-09-29, commit 166a98b, macOS 26.4.1 arm64 | PASS |
| 3. (re-run after INC-2026-0012) `modules` built with `make -C examples/c OUT=<scratch>` against `target/release/librivet.dylib` and run from `examples/c`: `duplicate` is `check.import_duplicate`, kind `syntax`, with request/trace IDs; other lines unchanged; exit 0 | Claude | 2026-09-29, commit 7c25175, macOS 26.4.1 arm64 | PASS |
| Linux | CI (`conformance_ffi`) | run 36483001760 | PASS |

## Known Caveats

- Request and trace IDs vary between runs.
- On macOS, the static link prints "built for newer macOS" warnings from ring's assembly. They are harmless;
  build the library and the program with the same `MACOSX_DEPLOYMENT_TARGET` to silence them.
- A refused module load reports the registry kind `syntax` for `check.import_duplicate` (exit 2, as `rivet check`)
  and, from INC-2026-0012, minted request and trace IDs (step 3, `duplicate` line).
- Windows is not supported (INC-2026-0011).

## Related Documents

- [API-2026-0007 C ABI](../../api/api-2026-0007-c-abi.md) · [MAN-2026-0009](../../manuals/man-2026-0009-c-abi-and-ffi.md) · [SYS-2026-0010](../../system/components/sys-2026-0010-ffi-surface-and-packaging.md)
- [API-2026-0006 envelopes](../../api/api-2026-0006-envelopes.md) · [PLAN-2026-0002](../../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-29 | Claude | Created: shared/static links, request, stream, input, cancel, module objects, Python and misuse, all executed. |
| 2 | 2026-09-29 | Claude | INC-2026-0012: a refused `rivet_load` now carries request/trace IDs; caveat updated. |
| 3 | 2026-09-29 | Claude | INC-2026-0012 re-verification (T-30) at 7c25175: step 3 re-run; its output is pasted in full from that run (the `duplicate` refusal shows its request/trace IDs); one Verification Record row. |
