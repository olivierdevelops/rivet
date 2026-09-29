---
document_id: MAN-2026-0009
title: "Calling Rivet from C, Python and Go (librivet)"
document_type: manual
status: active
created_date: 2026-09-29
last_updated: 2026-09-30
document_revision: 3
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [ffi, library, sessions, registry, language]
affected_versions:
  from: "0.2.0"
  to: null
applicable_environments: [embedded, development]
audience: [c-developers, python-developers, go-developers, integrators]
scope: Building librivet (shared and static), compiling and linking a C program on macOS and Linux, a walkthrough of the C API (requests, streams, live input, cancellation, highlighting, modules), the Python ctypes wrapper and module objects, a verified Go (cgo) program, errors and recovery, and troubleshooting.
reason: PLAN-2026-0002 row D-13 (TASK-072) — the integrator guide for non-Rust hosts; every program and output below was built and run against librivet from main.
related_documents: [MAN-2026-0001, MAN-2026-0007, API-2026-0007, API-2026-0006, SYS-2026-0010, PROP-2026-0002, PLAN-2026-0002, TRBL-2026-0006]
supersedes: null
superseded_by: null
tags: [rivet, manual, ffi, c, python, go, librivet]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.2.0-rc (main at e7ed8ed)"
next_review_date: 2026-10-29
---

# Calling Rivet from C, Python and Go (librivet)

> **Status:** Active
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-30
> **Affected Versions:** 0.2.0 and later
> **Owner:** Project maintainer
> **Affected Components:** ffi, library, sessions, registry, language

## Purpose

Run a `.rivet` catalog inside a program written in C, C++, Python, Go or any language with a C FFI — with the
same validation, policy, envelopes and errors as the CLI and `rivet serve`. Part of the
[Rivet manual](man-2026-0001-rivet-manual.md). The reference for every function is
[API-2026-0007](../api/api-2026-0007-c-abi.md). The executable demo is `docs/demos/15-ffi/` (DEMO-2026-0017).

**When to use it:** your host is not Rust and you want Rivet in-process (no server, no subprocess). Use
`rivet serve` instead when several processes share one runtime, and the [Rust library](man-2026-0007-embedding-library.md)
when the host is Rust.

## Reading Order

1. [Concepts](#concepts) and the [mental model](#architecture-and-mental-model).
2. [Build and link](#installation-and-setup).
3. [C walkthrough](#c-walkthrough), then [Python](#python-ctypes-wrapper) or [Go](#go-cgo).
4. [Errors and recovery](#errors-and-recovery-reference) and [troubleshooting](#troubleshooting-references).

## What's New in the Current Supported Release

Everything in this volume is new in 0.2.0: `librivet` (ABI version 1, 18 functions), the C header
`ffi/include/rivet.h`, `rivet.pc`, the C examples (`examples/c`) and the Python wrapper (`examples/python`).

## Project Goals and Boundaries

- JSON in, JSON out; no Rivet struct crosses the boundary; no callbacks.
- Every failure is an envelope string, never a crash (NULL pointers, bad UTF-8, freed handles, panics).
- macOS and Linux. **Windows is not supported** in 0.2.0.
- No official Python/Go packages: `examples/python/rivet.py` is an example wrapper to copy.

## Concepts

| Concept | Meaning |
|---|---|
| Runtime handle (`RivetRuntime*`) | One loaded bundle (or an empty root) plus its async runtime; thread-safe |
| Call handle (`RivetCall*`) | One running call you pull records from; one thread at a time |
| Module handle (`RivetModule*`) | A `.rivet` file loaded into a runtime under an alias; thread-safe |
| Envelope | The JSON every call returns ([API-2026-0006](../api/api-2026-0006-envelopes.md)) |
| Ownership | Every returned `char *` except `rivet_version()` is yours: free it with `rivet_string_free` |

## Architecture and Mental Model

```text
 your program                              librivet (Rust)
 ────────────                              ───────────────
 rivet_runtime_new(options) ────────────▶ build Runtime (compile bundle, load policy) ──▶ RivetRuntime*
 rivet_request(rt, input) ──(blocks)────▶ parse input ─▶ dispatch ─▶ policy ─▶ run ──────▶ char* envelope
 rivet_call_start(rt, input) ───────────▶ open a session ────────────────────────────────▶ RivetCall*
 rivet_call_next(call, 5000) ──(waits)──▶ next record | {"type":"timeout"} | NULL (done)
 rivet_string_free(s)                     frees what librivet returned
```

## Installation and Setup

### Prerequisites

| Need | Why | Check |
|---|---|---|
| Rust 1.90+ | builds `librivet` | `cargo --version` |
| A C compiler | builds your program | `cc --version` (verified: Apple clang 21) |
| Python 3 (optional) | the ctypes example | `python3 --version` |
| Go 1.24 (optional) | the cgo example | `go version` |

### Build librivet

```sh
cargo build --release -p rivet-ffi
ls target/release/librivet.*
# macOS: librivet.a  librivet.dylib      Linux: librivet.a  librivet.so
```

The header is `ffi/include/rivet.h` (generated by cbindgen and checked in). Exported symbols are exactly the 18
`rivet_*` functions:

```text
$ nm -gU target/release/librivet.dylib | awk '/ T /{print $3}'
_rivet_abi_version  _rivet_call_cancel  _rivet_call_finish_input  _rivet_call_free  _rivet_call_next
_rivet_call_send  _rivet_call_start  _rivet_highlight  _rivet_load  _rivet_module_call
_rivet_module_call_start  _rivet_module_free  _rivet_module_operations  _rivet_request
_rivet_runtime_free  _rivet_runtime_new  _rivet_string_free  _rivet_version
```

### Compile and link

```text
                 ┌──────── shared ────────┐                ┌───────── static ─────────┐
 app.c ──cc──▶   -L target/release -lrivet  -Wl,-rpath,…   │ target/release/librivet.a  + system libs
                 (needs librivet.{dylib,so} at run time)   │ (one self-contained binary)
```

| OS | Shared | Static |
|---|---|---|
| macOS | `cc app.c -I ffi/include -L target/release -lrivet -Wl,-rpath,$PWD/target/release -o app` | `cc app.c -I ffi/include target/release/librivet.a -framework Security -framework CoreFoundation -liconv -lc -lm -o app` |
| Linux | `cc app.c -I ffi/include -L target/release -lrivet -Wl,-rpath,$PWD/target/release -o app` | `cc app.c -I ffi/include target/release/librivet.a -lgcc_s -lutil -lrt -lpthread -lm -ldl -lc -o app` |

Or use `make -C examples/c` (it builds `demo`, `demo_static`, `modules`, `modules_static`) and `make -C examples/c test`.
With pkg-config: `python3 ffi/render_pc.py --prefix /usr/local > rivet.pc`, install `rivet.pc`, `librivet.*` and
`rivet.h` under the prefix, then `cc app.c $(pkg-config --cflags rivet) $(pkg-config --libs --static rivet)`.

**macOS deployment target.** Build `librivet` and link your program with the same `MACOSX_DEPLOYMENT_TARGET`
(for example `export MACOSX_DEPLOYMENT_TARGET=13.0` before both). Otherwise the static link prints, for `ring`'s
assembly objects:

```text
ld: warning: object file (…/librivet.a[331](…-ghashv8-armx-ios64.o)) was built for newer 'macOS' version (26.4) than being linked (26.0)
```

The program is still correct; the warning is noise.

## Feature Catalogue

| Feature | Why / When to Use It | Functions | Since | Instructions | Demo |
|---|---|---|---|---|---|
| Blocking request | Unary operations | `rivet_request` | 0.2.0 | [C walkthrough](#c-walkthrough) | `docs/demos/15-ffi/` |
| Streams | Operations that `emit` | `rivet_call_start`, `rivet_call_next` | 0.2.0 | [Streams](#streams-and-live-input) | `docs/demos/15-ffi/` |
| Live input | Operations that `receive` | `rivet_call_send`, `rivet_call_finish_input` | 0.2.0 | [Streams](#streams-and-live-input) | `docs/demos/15-ffi/` |
| Cancellation | Stop a running call | `rivet_call_cancel`, `rivet_call_free` | 0.2.0 | [Streams](#streams-and-live-input) | `docs/demos/15-ffi/` |
| Module objects | Load `.rivet` files at run time and call them by short ID | `rivet_load`, `rivet_module_*` | 0.2.0 | [Modules](#module-objects) | `docs/demos/17-modules/` |
| Highlighting | Colour `.rivet` sources in your tool | `rivet_highlight` | 0.2.0 | [MAN-2026-0010](man-2026-0010-editor-support-and-highlighting.md) | `docs/demos/16-editor/` |

## Configuration and Environment Variables

| Name | Kind | Type / Allowed Values | Default | Required When | Scope | Effect | Security Notes | Example |
|---|---|---|---|---|---|---|---|---|
| `file` | option | path | — | one bundle form | runtime | entry file; `policy.json` beside it is discovered | the file's directory is the root | `{"file":"app.rivet"}` |
| `source`, `path`, `root` | options | strings | — | in-memory bundle | runtime | compile text | root confines file effects and imports | `{"source":"…","path":"app.rivet","root":"."}` |
| `root` | option | path | — | module-only runtime | runtime | no entry file | deny-by-default | `{"root":"examples/modules"}` |
| `policy_file` / `policy_json` | options | path / object | discovery | granting effects | runtime | the policy | only way to grant I/O | `{"policy_json":{"version":1,"grants":[]}}` |
| `ceiling_json` | option | object | none | host limits | runtime | narrows the policy | can only narrow | `{"ceiling_json":{"version":1,"limits":{"max_concurrent_requests":8}}}` |
| `pretty` | option / input key | bool | false | — | runtime / call | indent returned JSON | — | `{"operation":"demo.add","pretty":true}` |
| `RIVET_LIB` | env var | path | `target/release` then `target/debug` | Python wrapper | process | which `librivet` to load | — | `RIVET_LIB=/usr/local/lib/librivet.so` |

## Task-Oriented Workflows

### C walkthrough

#### Journey Overview

```text
[rivet_runtime_new] ─▶ RIVET_OK ─▶ [rivet_request] ─▶ envelope (status ok|error) ─▶ [rivet_string_free]
        │                                   │
        └─▶ RIVET_ERROR + error envelope    └─▶ never NULL; errors are envelopes too
```

#### CLI Procedure

`examples/c/demo.c` loads `examples/ffi/app.rivet` (three pure operations: `demo.add`, `events.count`,
`chat.echo`). Build and run:

```sh
make -C examples/c && cd examples/c && ./demo ../ffi/app.rivet
```

Real output (IDs differ per run; the version reads `0.2.0` in the release build):

```text
abi 1 version 0.2.1
options-error {"request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.ffi_argument","message":"unknown option `colour` (expected one of file, source, path, root, policy_file, policy_json, ceiling_json, pretty)","retryable":false,"details":{"argument":"colour"}},"effects":"none","data_count":0}
request {"request_id":"req_01aaa1efc5","trace_id":"tr_01aaa1efc5","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}
pretty {
  "request_id": "req_022b664fda",
  "trace_id": "tr_022b664fda",
  "operation": "demo.add",
  "type": "result",
  "status": "ok",
  "data": 42,
  "error": null,
  "effects": "none",
  "data_count": 0
}
invalid {"request_id":"req_03a82cbd5f","trace_id":"tr_03a82cbd5f","operation":"demo.add","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.type","message":"parameter `a` must be an integer, got text","retryable":false,"operation_id":"demo.add","details":{"field":"a"}},"effects":"none","data_count":0}
```

The core of the program:

```c
char *err = NULL;
RivetRuntime *rt = NULL;
if (rivet_runtime_new("{\"file\":\"../ffi/app.rivet\"}", &rt, &err) != RIVET_OK) {
    fprintf(stderr, "%s\n", err);        /* error envelope */
    rivet_string_free(err);
    return 1;
}
char *out = rivet_request(rt, "{\"operation\":\"demo.add\",\"data\":{\"a\":2,\"b\":3}}");
puts(out);
rivet_string_free(out);
rivet_runtime_free(rt);                   /* RIVET_OK; a second free returns RIVET_ERROR */
```

#### Expected Result and Side Effects

The envelope string; no I/O happens unless the policy grants it. `rivet_runtime_free` cancels in-flight calls
(5 s grace) before returning.

### Streams and live input

```text
 call = rivet_call_start(rt, input)                     (never NULL)
   loop: rec = rivet_call_next(call, 5000)
           NULL                  → done (after the result record)
           {"type":"timeout"}    → nothing yet; loop
           {"type":"data",…}     → one item
           {"type":"result",…}   → terminal (status ok | error | cancelled)
   rivet_call_send(call, "\"hi\"") / rivet_call_finish_input(call)   (operations that `receive`)
   rivet_call_cancel(call)  → the records end with status "cancelled"
 rivet_call_free(call)       (cancels if still running)
```

Output of the same demo:

```text
record {"request_id":"req_0428dc76e4","trace_id":"tr_0428dc76e4","operation":"events.count","type":"data","seq":1,"data":1,"error":null}
record {"request_id":"req_0428dc76e4","trace_id":"tr_0428dc76e4","operation":"events.count","type":"data","seq":2,"data":2,"error":null}
record {"request_id":"req_0428dc76e4","trace_id":"tr_0428dc76e4","operation":"events.count","type":"data","seq":3,"data":3,"error":null}
record {"request_id":"req_0428dc76e4","trace_id":"tr_0428dc76e4","operation":"events.count","type":"result","seq":4,"status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}
stream-records 4
send {"session_id":"ses_022b6958c2","accepted_seq":1,"input_closed":false}
send {"session_id":"ses_022b6958c2","accepted_seq":2,"input_closed":false}
finish {"session_id":"ses_022b6958c2","accepted_seq":null,"input_closed":true}
record {"request_id":"req_05a989f571","trace_id":"tr_05a989f571","operation":"chat.echo","type":"data","seq":1,"data":"hi","error":null}
record {"request_id":"req_05a989f571","trace_id":"tr_05a989f571","operation":"chat.echo","type":"data","seq":2,"data":"there","error":null}
record {"request_id":"req_05a989f571","trace_id":"tr_05a989f571","operation":"chat.echo","type":"result","seq":3,"status":"ok","data":2,"error":null,"effects":"none","data_count":2}
chat-records 3
send {"session_id":"ses_03a820a757","accepted_seq":1,"input_closed":false}
first {"request_id":"req_062e4123e6","trace_id":"tr_062e4123e6","operation":"chat.echo","type":"data","seq":1,"data":"ping","error":null}
poll {"type":"timeout"}
record {"request_id":"req_062e4123e6","trace_id":"tr_062e4123e6","operation":"chat.echo","type":"result","seq":2,"status":"cancelled","data":null,"error":{"kind":"cancelled","code":"cancelled.session","message":"the session was cancelled","retryable":false},"effects":"none","data_count":1}
cancel-records 1
highlight {"line":1,"col":1,"len":6,"class":"keyword","text":"global"}
free 0
free-again 1
```

Rules: a call handle belongs to **one thread at a time**; a call not read for **60 s** is cancelled (idle lease);
`rivet_call_next` with a negative timeout waits until a record arrives.

### Module objects

Load `.rivet` files at run time into a runtime that may have no entry file. The module's operations are called
by short ID; the envelope names the namespaced ID.

```text
 rivet_runtime_new({"root": "examples/modules"}) ─▶ rt (empty catalog)
 rivet_load(rt, "./users.rivet", NULL)            ─▶ module "users"   (users.get, users.list)
 rivet_load(rt, "./lib/billing.rivet", "billing") ─▶ module "billing" (imports ../users.rivet itself)
 rivet_module_call(users, "get", "{\"id\":42}")    ─▶ envelope, operation "users.get"
 rivet_request(rt, {"operation":"users.get",…})    ─▶ same operation through the runtime
```

`make -C examples/c run-modules` (static build shown; IDs differ per run):

```text
operations [{"id":"get","operation":"users.get","name":"users.get","description":"One user by id.","emits":false,"receives":false},{"id":"list","operation":"users.list","name":"users.list","description":"The first two users (calls inside a module use its own IDs).","emits":false,"receives":false}]
call {"request_id":"req_01b37c0705","trace_id":"tr_01b37c0705","operation":"users.get","type":"result","status":"ok","data":{"id":42,"name":"Hello, user 42"},"error":null,"effects":"none","data_count":0}
bad-call {"request_id":"req_0232d6db5a","trace_id":"tr_0232d6db5a","operation":"users.get","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.min","message":"parameter `id` must be ≥ 1","retryable":false,"operation_id":"users.get"},"effects":"none","data_count":0}
call {"request_id":"req_03b1a2b517","trace_id":"tr_03b1a2b517","operation":"billing.invoice","type":"result","status":"ok","data":{"user":{"id":7,"name":"Hello, user 7"},"amount":20.0},"error":null,"effects":"none","data_count":0}
duplicate {"request_id":"","trace_id":"","operation":"rivet.load","type":"result","status":"error","data":null,"error":{"kind":"syntax","code":"check.import_duplicate","message":"a module named `users` is already loaded; use load_as(path, alias)","retryable":false},"effects":"none","data_count":0}
record {"request_id":"req_0431af8f24","trace_id":"tr_0431af8f24","operation":"users.list","type":"result","seq":1,"status":"ok","data":[{"id":1,"name":"Hello, user 1"},{"id":2,"name":"Hello, user 2"}],"error":null,"effects":"none","data_count":0}
request {"request_id":"req_05b0fe0329","trace_id":"tr_05b0fe0329","operation":"users.get","type":"result","status":"ok","data":{"id":7,"name":"Hello, user 7"},"error":null,"effects":"none","data_count":0}
module-free-again 1
```

Modules run under the **runtime's policy only**; a `policy.json` beside a module file is ignored
(`check.module_policy_ignored`). Load errors are the same codes as `import` in `.rivet`
([MAN-2026-0003](man-2026-0003-language-guide.md#modules-import)): `not_found.import`,
`permission.import_outside_root`, `check.import_cycle`, `check.import_duplicate`, `check.import_collision`,
`limit.imports`.

### Python (ctypes wrapper)

`examples/python/rivet.py` declares all 18 prototypes and copies/frees every returned string. It finds the library
through `RIVET_LIB`, else `target/release`, then `target/debug` of the checkout.

```python
import rivet                                            # examples/python/rivet.py
with rivet.Rivet(file="examples/ffi/app.rivet") as rt:  # rivet_runtime_new
    out = rt.request("demo.add", a=2, b=3)              # envelope dict
    with rt.stream("events.count") as call:             # rivet_call_start / next
        records = list(call)
with rivet.Rivet(root="examples/modules") as rt:
    users = rt.load("./users.rivet")                    # Module object
    users.get(id=42)                                    # attribute per operation
    users.operations()                                  # ['get', 'list']
```

`python3 examples/python/demo.py` and `python3 examples/python/modules.py` (real output):

```text
abi 1 version 0.2.1
options-error validation.ffi_argument
request {"request_id": "req_01ae55f7bd", "trace_id": "tr_01ae55f7bd", "operation": "demo.add", "type": "result", "status": "ok", "data": 5, "error": null, "effects": "none", "data_count": 0}
invalid error validation.type
stream [1, 2, 3, {'count': 3}] result ok
send 1
send 2
chat ['hi', 'there', 2]
first ping
poll timeout
cancel cancelled cancelled.session
highlight {"line":1,"col":1,"len":6,"class":"keyword","text":"global"}
operations ['get', 'list']
get users.get {"id": 42, "name": "Hello, user 42"}
list [1, 2]
invoice {"user": {"id": 7, "name": "Hello, user 7"}, "amount": 20.0}
duplicate check.import_duplicate
no-such-operation module 'users' has no operation 'missing'
request Hello, user 7
```

A failed `Rivet(...)` or `load(...)` raises `rivet.RivetError` whose `.envelope` is the error envelope; calls
return envelope dicts (check `["status"]`).

### Go (cgo)

Verified with Go 1.24.1 on macOS against `target/release/librivet.dylib` (replace `/path/to/rivet`):

```go
package main

/*
#cgo CFLAGS: -I/path/to/rivet/ffi/include
#cgo LDFLAGS: -L/path/to/rivet/target/release -lrivet -Wl,-rpath,/path/to/rivet/target/release
#include <stdlib.h>
#include <rivet.h>
*/
import "C"

import (
	"encoding/json"
	"fmt"
	"unsafe"
)

// take copies a librivet string into Go and frees it.
func take(p *C.char) string {
	if p == nil {
		return ""
	}
	defer C.rivet_string_free(p)
	return C.GoString(p)
}

func main() {
	opts := C.CString(`{"file":"/path/to/rivet/examples/ffi/app.rivet"}`)
	defer C.free(unsafe.Pointer(opts))
	var rt *C.RivetRuntime
	var errJSON *C.char
	if C.rivet_runtime_new(opts, &rt, &errJSON) != C.RIVET_OK {
		fmt.Println("error:", take(errJSON))
		return
	}
	defer C.rivet_runtime_free(rt)

	in := C.CString(`{"operation":"demo.add","data":{"a":2,"b":3}}`)
	defer C.free(unsafe.Pointer(in))
	var env struct {
		Status string          `json:"status"`
		Data   json.RawMessage `json:"data"`
	}
	json.Unmarshal([]byte(take(C.rivet_request(rt, in))), &env)
	fmt.Println("status", env.Status, "data", string(env.Data))

	sin := C.CString(`{"operation":"events.count","data":{}}`)
	defer C.free(unsafe.Pointer(sin))
	call := C.rivet_call_start(rt, sin)
	defer C.rivet_call_free(call)
	for {
		rec := C.rivet_call_next(call, 5000)
		if rec == nil {
			break
		}
		fmt.Println("record", take(rec))
	}
}
```

```text
$ go run .
status ok data 5
record {"request_id":"req_02c7f5f36a","trace_id":"tr_02c7f5f36a","operation":"events.count","type":"data","seq":1,"data":1,"error":null}
record {"request_id":"req_02c7f5f36a","trace_id":"tr_02c7f5f36a","operation":"events.count","type":"data","seq":2,"data":2,"error":null}
record {"request_id":"req_02c7f5f36a","trace_id":"tr_02c7f5f36a","operation":"events.count","type":"data","seq":3,"data":3,"error":null}
record {"request_id":"req_02c7f5f36a","trace_id":"tr_02c7f5f36a","operation":"events.count","type":"result","seq":4,"status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}
```

Keep a `RivetCall` on one goroutine (or guard it with a mutex); `RivetRuntime` may be shared.

#### Verified Demo

`docs/demos/15-ffi/` (DEMO-2026-0017) and `docs/demos/17-modules/` (DEMO-2026-0019), written for 0.2.0.

## Complete CLI Reference

Not applicable: librivet has no command line. Build commands are in [Installation and Setup](#installation-and-setup).

## Complete API and Event Reference

See [API-2026-0007](../api/api-2026-0007-c-abi.md) for all 18 functions.

## UI Screen and Interaction Reference

Not applicable (no UI).

## Errors and Recovery Reference

| Error / Code / Message | Surface | Cause | User-Visible Result | Recovery | Retry Safe | Related Feature |
|---|---|---|---|---|---|---|
| `validation.ffi_argument` "`input_json` is NULL" | any function | NULL / non-UTF-8 argument | error envelope | pass a valid NUL-terminated UTF-8 string | yes, after fixing | all |
| `validation.ffi_argument` "unknown option `colour` …" | `rivet_runtime_new` | unknown option key | `RIVET_ERROR` + envelope | use the listed keys | yes | runtime |
| `validation.ffi_argument` "unknown or already freed RivetRuntime handle" | any handle function | use after free, foreign pointer | error envelope, or `RIVET_ERROR` from a free | keep one owner per handle | no | handles |
| `validation.input_envelope` | `rivet_request`, `rivet_call_start` | input not JSON / not an object / mixed alias keys | error envelope | send `{operation, data}` | yes | input |
| `not_found.source` | `rivet_runtime_new` | `file` missing | `RIVET_ERROR` + envelope | fix the path | yes | runtime |
| `not_found.operation` | request / call start | unknown ID | envelope (call: first record, then NULL) | check `rivet_module_operations` / `rivet list` | yes | all |
| `check.import_duplicate` | `rivet_load` | alias already loaded | `RIVET_ERROR` + envelope | pass another alias | yes | modules |
| `internal.panic` | any | a Rust panic caught at the boundary | error envelope | report a bug with the input | no | all |
| `cancelled.session` | call records | `rivet_call_cancel`, free while running, or 60 s idle | terminal record `status: "cancelled"` | read calls promptly | — | streams |

## Examples and Demos

`examples/c/demo.c`, `examples/c/modules.c`, `examples/python/{rivet,demo,modules}.py`, `examples/ffi/app.rivet`,
`examples/modules/`. Demos: `docs/demos/15-ffi/`, `docs/demos/17-modules/`.

## Operations, Observability and Maintenance

The library writes nothing to stdout/stderr. Every envelope carries `request_id` and `trace_id` for your logs.
Upgrading: rebuild against the new `rivet.h`; `rivet_abi_version()` stays `1` for additive releases.

## Edge Cases

- `rivet_call_start` never returns NULL; a failed start gives its error record, then NULL.
- NULL `data_json` in module calls means `{}`.
- `rivet_string_free(NULL)` is `RIVET_OK`; freeing a pointer librivet did not return is `RIVET_ERROR` (no crash).
- Pretty output: each record is its own string, so pretty streams are fine over the ABI.

## Failure Modes, Recovery and Rollback

A host that crashes loses its in-flight calls; nothing persists. Roll back by relinking an older `librivet`
with the matching `rivet.h`.

## Security and Compatibility

The host process is the principal; the policy passed in the options is the only grant. Globals cannot hold
secrets. See [SEC-2026-0001](../security/sec-2026-0001-policy-and-sandbox-model.md#ffi-trust-boundary).
Compatible with macOS and Linux; not Windows.

## Limitations

- No Windows build (`rivet.dll` is not produced or tested).
- No callbacks: pull records with `rivet_call_next`.
- No official language packages (examples only).
- The Linux static link list is verified by CI, not on a local host.

## Troubleshooting References

| Symptom | Cause | Fix |
|---|---|---|
| `dyld: Library not loaded: @rpath/librivet.dylib` | no rpath | link with `-Wl,-rpath,<dir>` or set `DYLD_LIBRARY_PATH` |
| `error while loading shared libraries: librivet.so` | no rpath (Linux) | `-Wl,-rpath,<dir>` or `LD_LIBRARY_PATH` |
| `ld: warning: … built for newer 'macOS' version` | mixed deployment targets | same `MACOSX_DEPLOYMENT_TARGET` for both builds |
| `librivet.*` missing after `cargo test` | test builds stay in `target/<profile>/deps` | `cargo build --release -p rivet-ffi` ([TRBL-2026-0006](../troubleshooting/trbl-2026-0006-cargo-test-leaves-cdylib-and-staticlib-in-deps.md)) |
| Python `OSError: librivet.dylib not found` | not built | build, or `RIVET_LIB=/path/librivet.dylib` |

## Glossary

ABI — the binary interface (`rivet.h`, version 1). Envelope — the JSON answer. Handle — an opaque pointer-sized
token. Module — a `.rivet` file loaded into a runtime under an alias.

## Version Applicability

| Feature / Interface | Introduced | Changed | Deprecated / Removed | Applicable Environment |
|---|---|---|---|---|
| librivet ABI 1 (18 functions) | 0.2.0 | — | — | macOS, Linux |
| Module handles | 0.2.0 | — | — | macOS, Linux |

## Related Features

[Rust library](man-2026-0007-embedding-library.md) · [modules in `.rivet`](man-2026-0003-language-guide.md#modules-import)
· [highlighting](man-2026-0010-editor-support-and-highlighting.md)

## Related Documents

- [API-2026-0007](../api/api-2026-0007-c-abi.md), [API-2026-0006](../api/api-2026-0006-envelopes.md)
- [SYS-2026-0010](../system/components/sys-2026-0010-ffi-surface-and-packaging.md)
- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) D-13

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 3 | 2026-09-30 | Claude | v0.2.1 patch (PLAN-2026-0002 TASK-097): version strings, install tag v0.2.1; INC-2026-0013 behaviour where described. |
| 2 | 2026-09-30 | Claude | v0.2.0 release (PLAN-2026-0002 TASK-091): version strings and current-release wording updated to 0.2.0. |
| 1 | 2026-09-29 | Claude | Initial FFI manual (TASK-072, D-13): build/link per OS, C walkthrough, streams, modules, Python wrapper, verified Go program, errors, troubleshooting |
