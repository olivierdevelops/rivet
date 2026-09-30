---
document_id: DEMO-2026-0021
title: "What librivet can do in 0.2.1: a capability tour"
document_type: demo
status: active
created_date: 2026-09-30
last_updated: 2026-09-30
document_revision: 1
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [ffi, library]
affected_versions:
  from: "0.2.1"
  to: null
applicable_environments: [development, embedded]
audience: [developers, integrators, reviewers]
scope: One C program and one Python script, built outside the repository against the shipped dist/v0.2.1 files, that walk every librivet capability — identity, runtime options, unary requests, pretty output, validation, globals, the sandbox policy and per-request restriction, streams, live input, timeout records, cancellation, deadlines, module objects, the import root, concurrent threads, highlighting and misuse handling.
reason: Maintainer question "what can rivet-ffi do now?" (2026-09-30); DEMO-2026-0017 builds from the repository and predates 0.2.1, this tour consumes the release artifacts as an integrator would.
dependencies: [REL-0.2.1, DEMO-2026-0017]
related_documents: [DEMO-2026-0017, REL-0.2.1, API-2026-0007, MAN-2026-0009, SYS-2026-0010, API-2026-0006, DEMO-2026-0019]
supersedes: null
superseded_by: null
tags: [rivet, demo, ffi, c-abi, python, ctypes, capabilities]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-30
verified_against: "0.2.1"
---

# What librivet can do in 0.2.1: a capability tour

> **Status:** Active
> **Created:** 2026-09-30
> **Last Updated:** 2026-09-30
> **Affected Versions:** 0.2.1 and later
> **Owner:** Project maintainer
> **Affected Components:** ffi, library

## Purpose

`rivet-ffi` builds **librivet**, Rivet's C ABI. Any language that can call C (C, C++, Python ctypes, Go cgo,
Swift, Zig, Node N-API, …) can load a `.rivet` bundle and run its operations **with the same sandbox, envelopes
and streaming as the CLI and `rivet serve`**, in-process, with no server and no Rust toolchain.

This tour answers "what can it do right now?" by running every capability once against the **shipped
0.2.1 files**, from a directory outside the repository, the way an integrator would receive them.
[DEMO-2026-0017 (15-ffi)](../15-ffi/README.md) is the build-from-source companion.

```text
  ┌───────────────────────── your process ──────────────────────────┐
  │                                                                 │
  │   C / C++ / Python / Go / Swift …                               │
  │        │  JSON string in                ▲ JSON string out       │
  │        ▼                                │ (caller frees)        │
  │   ┌──────────────── librivet (18 rivet_* functions) ─────────┐  │
  │   │  RivetRuntime ── bundle + policy + async runtime         │  │
  │   │     ├─ rivet_request ........ unary, blocking            │  │
  │   │     ├─ RivetCall ............ stream / live input        │  │
  │   │     └─ RivetModule .......... a .rivet file as an object │  │
  │   │  sandbox: policy.json ∩ ceiling ∩ per-request restrict   │  │
  │   └───────────────────────────────────────┬──────────────────┘  │
  └───────────────────────────────────────────┼─────────────────────┘
                                              ▼
                              files / HTTP / gRPC / QUIC / UDP / MCP …
                              (only what policy.json grants)
```

### Capability map

| # | Capability | Functions | Section |
|---|---|---|---|
| 1 | Version and ABI identity | `rivet_version`, `rivet_abi_version` | 1 |
| 2 | Build a runtime from a file, source text or root; bad options return an envelope | `rivet_runtime_new`, `rivet_runtime_free` | 2 |
| 3 | Unary calls, `"pretty": true`, validation and not-found errors, `global` constants | `rivet_request` | 3 |
| 4 | Sandbox: policy grants, deny rules, per-request `restrict` (narrow only) | `rivet_request` | 4 |
| 5 | Server streams (`emits`), record by record | `rivet_call_start`, `rivet_call_next` | 5 |
| 6 | Live input (`receives`), non-blocking polls with a `timeout` record | `rivet_call_send`, `rivet_call_finish_input` | 6 |
| 7 | Cancellation and `deadline_ms` | `rivet_call_cancel`, `rivet_call_free` | 7 |
| 8 | Modules: load a file as an object of operations, confined to the root | `rivet_load`, `rivet_module_*` | 8 |
| 9 | One runtime shared by many threads | `rivet_request` from pthreads | 9 |
| 10 | Syntax highlighting as JSON tokens, HTML or ANSI, no runtime needed | `rivet_highlight` | 10 |
| 11 | Misuse (NULL, foreign pointer, double free, freed handle) is refused, never UB | all | 11 |
| 12 | Same library from Python (ctypes) | via `examples/python/rivet.py` | 12 |

```text
  what you get from the release                      how you link it
  ────────────────────────────────                   ─────────────────────────────────────
  dist/v0.2.1/                                       shared:  -lrivet  (+ rpath)
  ├── lib/librivet.dylib   15 MiB  (.so on Linux)    static:  librivet.a + Libs.private
  ├── lib/librivet.a       59 MiB  (deps included)   pkg-config: --cflags --libs rivet
  ├── lib/rivet.pc         pkg-config file
  └── include/rivet.h      the whole API (124 lines)
```

## Verified Against Version

0.2.1: tag `v0.2.1`, commit `9074f593ea8c11cfb5a9e077bd7961c8c8701070`, using the `dist/v0.2.1/` artifacts
(checked against its `SHA256SUMS`). macOS 26.4.1 (Darwin 25.4.0, arm64), Apple clang (`cc`), Python 3.9.6,
2026-09-30. `rivet_version()` returns `0.2.1`, `rivet_abi_version()` returns `1`. Linux is covered by CI
(`conformance_ffi`, tag run 36638613406), not by this walk-through. Windows is not supported (INC-2026-0011).
Request, trace and session IDs vary between runs.

## Prerequisites

- The v0.2.1 release files (`dist/v0.2.1/`, or the same layout from the GitHub Release).
- A C compiler (`cc`) with pthreads; `python3` for step 12 (standard library `ctypes` only).
- Optional: `pkg-config` (not installed on the verification host; the equivalent flags are shown).

No Rust toolchain is needed.

## Setup

Copy the release and this folder somewhere outside the repository, as a consumer would:

```sh
WORK="$(mktemp -d)"
cp -R dist/v0.2.1                "$WORK/rivet-0.2.1"
cp -R docs/demos/18-ffi-tour     "$WORK/tour"
cp examples/python/rivet.py      "$WORK/tour/"       # ctypes wrapper, step 12 only
cd "$WORK/rivet-0.2.1" && shasum -a 256 -c SHA256SUMS && cd "$WORK/tour"
```

```text
  $WORK/tour/
  ├── app.rivet          6 operations: add, greet (global), data.read, data.private, events.count, chat.echo
  ├── policy.json        allow_read ./data/**  ·  deny allow_read ./data/private/**
  ├── data/public.json   readable
  ├── data/private/…     denied (deny overrides grant)
  ├── lib/users.rivet    module file: get, list  → users.get, users.list
  ├── tour.c             steps 1–11
  └── tour.py            step 12
```

Build it both ways. `rivet.pc` has `prefix=/usr/local`; with pkg-config use
`pkg-config --define-variable=prefix="$WORK/rivet-0.2.1" --cflags --libs rivet`.

```sh
P="$WORK/rivet-0.2.1"
# shared
cc tour.c -I"$P/include" -L"$P/lib" -lrivet -Wl,-rpath,"$P/lib" -lpthread -Wall -o tour-shared
# static (macOS Libs.private; Linux: see rivet.pc)
cc tour.c -I"$P/include" "$P/lib/librivet.a" \
   -framework Security -framework CoreFoundation -liconv -lc -lm -lpthread -Wall -o tour-static
otool -L tour-shared | grep rivet      # Linux: ldd
```

Expected: `tour-shared` (about 34 KiB) links `@rpath/librivet.dylib`. `tour-static` (about 27 MiB) has no
librivet dependency at all. Both build with no warnings from `tour.c`.

## Steps

Run either binary from the tour folder. The two print the same output, apart from the IDs:

```sh
./tour-shared          # or ./tour-static
```

The output below is from one run of `tour-shared`, cut into its sections.

### 1. Identity

#### Command / Request

```c
printf("abi %u, version %s\n", rivet_abi_version(), rivet_version());
```

#### Expected Output / Response

```text
== 1. identity
library        abi 1, version 0.2.1
```

Check the ABI major at startup. It changes only on a breaking change to `rivet.h`.

### 2. Runtime options

#### Command / Request

```c
rivet_runtime_new("{\"file\":\"app.rivet\",\"colour\":true}", &rt, &err);             /* typo */
rivet_runtime_new("{\"file\":\"app.rivet\",\"policy_file\":\"policy.json\"}", &rt, &err);
```

Options: `file`, **or** `source` + `path` + `root`, **or** `root` alone (a modules-only runtime); plus
`policy_file` | `policy_json`, `ceiling_json` (host limits that can only narrow) and `pretty`.

#### Expected Output / Response

```text
== 2. runtime: bad options are an envelope, not a crash
options-error  {"request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.ffi_argument","message":"unknown option `colour` (expected one of file, source, path, root, policy_file, policy_json, ceiling_json, pretty)","retryable":false,"details":{"argument":"colour"}},"effects":"none","data_count":0}
```

### 3. Unary requests, pretty output, validation, globals

#### Command / Request

```c
rivet_request(rt, "{\"operation\":\"demo.add\",\"data\":{\"a\":2,\"b\":3}}");
rivet_request(rt, "{\"operation\":\"demo.add\",\"data\":{\"a\":40,\"b\":2},\"pretty\":true}");
rivet_request(rt, "{\"operation\":\"demo.add\",\"data\":{\"a\":\"two\"}}");
rivet_request(rt, "{\"operation\":\"demo.nope\",\"data\":{}}");
rivet_request(rt, "{\"operation\":\"demo.greet\",\"data\":{\"who\":\"C\"}}");   /* uses global GREETING */
```

#### Expected Output / Response

```text
== 3. unary requests, pretty output, validation, globals
add            {"request_id":"req_01baa41765","trace_id":"tr_01baa41765","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}
pretty         {
  "request_id": "req_023b609ae2",
  "trace_id": "tr_023b609ae2",
  "operation": "demo.add",
  "type": "result",
  "status": "ok",
  "data": 42,
  "error": null,
  "effects": "none",
  "data_count": 0
}
invalid        {"request_id":"req_03b82bd577","trace_id":"tr_03b82bd577","operation":"demo.add","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.type","message":"parameter `a` must be an integer, got text","retryable":false,"operation_id":"demo.add","details":{"field":"a"}},"effects":"none","data_count":0}
unknown        {"request_id":"req_0438dd076c","trace_id":"tr_0438dd076c","operation":"demo.nope","type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"not_found.operation","message":"no operation `demo.nope`","retryable":false,"operation_id":"demo.nope"},"effects":"none","data_count":0}
global         {"request_id":"req_05b98159d1","trace_id":"tr_05b98159d1","operation":"demo.greet","type":"result","status":"ok","data":"hello C","error":null,"effects":"none","data_count":0}
```

```text
  every answer has one shape (API-2026-0006):
  { request_id, trace_id, operation, type, status, data, error, effects, data_count }
                                              │       │      │
                                ok ───────────┘  value │   null
                                error ───────────  null └── {kind, code, message, retryable, …}
```

### 4. The sandbox applies through the ABI

#### Command / Request

```c
rivet_request(rt, "{\"operation\":\"data.read\",\"data\":{}}");       /* granted */
rivet_request(rt, "{\"operation\":\"data.private\",\"data\":{}}");    /* denied by policy */
rivet_request(rt, "{\"operation\":\"data.read\",\"data\":{},"         /* narrowed by the caller */
                  "\"restrict\":{\"grants\":[{\"capability\":\"allow_read\","
                  "\"targets\":[\"./data/other/**\"],\"access\":[\"read\"]}]}}");
```

```text
  effective authority = host ceiling ∩ policy.json ∩ request restrict
                          (ceiling_json)   (grants − deny)   (can only narrow)

  request                        target                      policy grant  policy deny  restrict     result
  data.read                      ./data/public.json          ✔ data/**     –            (none)       ok
  data.private                   ./data/private/secret.json  ✔ data/**     ✔ private/** (none)       DENIED
  data.read + restrict other/**  ./data/public.json          ✔ data/**     –            ✘ no match   DENIED
```

#### Expected Output / Response

```text
== 4. the sandbox policy applies through the ABI
allowed        {"request_id":"req_063e4b978e","trace_id":"tr_063e4b978e","operation":"data.read","type":"result","status":"ok","data":{"message":"public data read through librivet"},"error":null,"effects":"none","data_count":0}
denied         {"request_id":"req_07bd00afd3","trace_id":"tr_07bd00afd3","operation":"data.private","type":"result","status":"error","data":null,"error":{"kind":"permission","code":"permission.denied","message":"allow_read read on ./data/private/secret.json denied: deny allow_read ./data/private/**","retryable":false,"source":{"file":"app.rivet","line":40,"column":5,"end_line":40,"end_column":59},"operation_id":"data.private","details":{"capability":"allow_read","access":"read","target":"./data/private/secret.json"}},"effects":"none","data_count":0}
restricted     {"request_id":"req_083c58a2e8","trace_id":"tr_083c58a2e8","operation":"data.read","type":"result","status":"error","data":null,"error":{"kind":"permission","code":"permission.denied","message":"allow_read read on ./data/public.json denied: request restriction: no grant for allow_read ./data/public.json","retryable":false,"source":{"file":"app.rivet","line":29,"column":5,"end_line":29,"end_column":51},"operation_id":"data.read","details":{"capability":"allow_read","access":"read","target":"./data/public.json"}},"effects":"none","data_count":0}
```

The host program gets the same deny-by-default sandbox as `rivet serve`. The error points at the offending
line of `app.rivet`.

### 5. Server stream

#### Command / Request

```c
RivetCall *call = rivet_call_start(rt, "{\"operation\":\"events.count\",\"data\":{}}");
while ((rec = rivet_call_next(call, 2000)) != NULL) { /* print */ rivet_string_free(rec); }
rivet_call_free(call);
```

#### Expected Output / Response

```text
== 5. server stream
stream         {"request_id":"req_09bf938e3d","trace_id":"tr_09bf938e3d","operation":"events.count","type":"data","seq":1,"data":1,"error":null}
stream         {"request_id":"req_09bf938e3d","trace_id":"tr_09bf938e3d","operation":"events.count","type":"data","seq":2,"data":2,"error":null}
stream         {"request_id":"req_09bf938e3d","trace_id":"tr_09bf938e3d","operation":"events.count","type":"data","seq":3,"data":3,"error":null}
stream         {"request_id":"req_09bf938e3d","trace_id":"tr_09bf938e3d","operation":"events.count","type":"result","seq":4,"status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}
```

### 6. Live input and the timeout record

#### Command / Request

```c
RivetCall *chat = rivet_call_start(rt, "{\"operation\":\"chat.echo\",\"data\":{}}");
rivet_call_send(chat, "\"hi\"");
rivet_call_next(chat, 2000);          /* the echo */
rivet_call_next(chat, 100);           /* nothing yet → {"type":"timeout"}, never blocks forever */
rivet_call_send(chat, "\"bye\"");
rivet_call_finish_input(chat);
/* drain until NULL */
```

```text
  host                         RivetCall (chat.echo)
  ────                         ─────────────────────
  send "hi"      ────────────▶ ack accepted_seq 1
  next(2000)     ◀──────────── data seq 1 "hi"
  next(100)      ◀──────────── {"type":"timeout"}      (poll again later)
  send "bye"     ────────────▶ ack accepted_seq 2
  finish_input   ────────────▶ ack input_closed true
  next …         ◀──────────── data seq 2 "bye", result seq 3 data 2, then NULL
```

#### Expected Output / Response

```text
== 6. live input: send, timeout record, finish
send           {"session_id":"ses_0238868e2a","accepted_seq":1,"input_closed":false}
next           {"request_id":"req_103efab29a","trace_id":"tr_103efab29a","operation":"chat.echo","type":"data","seq":1,"data":"hi","error":null}
next(100ms)    {"type":"timeout"}
send           {"session_id":"ses_0238868e2a","accepted_seq":2,"input_closed":false}
finish         {"session_id":"ses_0238868e2a","accepted_seq":null,"input_closed":true}
next           {"request_id":"req_103efab29a","trace_id":"tr_103efab29a","operation":"chat.echo","type":"data","seq":2,"data":"bye","error":null}
next           {"request_id":"req_103efab29a","trace_id":"tr_103efab29a","operation":"chat.echo","type":"result","seq":3,"status":"ok","data":2,"error":null,"effects":"none","data_count":2}
```

### 7. Cancel and deadline

#### Command / Request

```c
RivetCall *c2 = rivet_call_start(rt, "{\"operation\":\"chat.echo\",\"data\":{}}");
rivet_call_cancel(c2);                                   /* then drain */
rivet_call_start(rt, "{\"operation\":\"chat.echo\",\"data\":{},\"deadline_ms\":200}");   /* never fed */
```

#### Expected Output / Response

```text
== 7. cancel and deadline
cancelled      {"request_id":"req_11b41ea4e7","trace_id":"tr_11b41ea4e7","operation":"chat.echo","type":"result","seq":1,"status":"cancelled","data":null,"error":{"kind":"cancelled","code":"cancelled.session","message":"the session was cancelled","retryable":false},"effects":"none","data_count":0}
deadline       {"request_id":"req_123b4d5f1c","trace_id":"tr_123b4d5f1c","operation":"chat.echo","type":"result","seq":1,"status":"error","data":null,"error":{"kind":"timeout","code":"timeout.request","message":"`chat.echo` exceeded its 200 ms deadline","retryable":false,"source":{"file":"app.rivet","line":64,"column":5,"end_line":67,"end_column":8},"operation_id":"chat.echo"},"effects":"none","data_count":0}
```

Every call ends in exactly one terminal record (`ok`, `error` or `cancelled`), and then `NULL`.

### 8. Modules: a file as an object of operations

#### Command / Request

```c
rivet_load(rt, "lib/users.rivet", "users", &users, &err);
rivet_module_operations(users);
rivet_module_call(users, "get", "{\"id\":7}");            /* short ID → users.get */
rivet_module_call_start(users, "list", NULL);             /* streams work too */
rivet_load(rt, "../15-ffi/app.rivet", "x", &outside, &err);   /* outside the root */
rivet_module_free(users);                                  /* stays loaded in rt */
```

```text
  lib/users.rivet                 rivet_load(…, "users")          callers see
  operation get   ──────────────▶ users.get   ──────────────────▶ module "get"  / rt "users.get"
  operation list  ──────────────▶ users.list                      module "list" / rt "users.list"
                                  policy: the loader's policy.json only (modules bring none)
```

#### Expected Output / Response

```text
== 8. modules: a file as an object of operations
operations     [{"id":"get","operation":"users.get","name":"Get a user","description":"Return a synthetic user by ID.","emits":false,"receives":false},{"id":"list","operation":"users.list","name":"List users","description":"Emit three synthetic users.","emits":true,"receives":false}]
users.get      {"request_id":"req_1389d52a39","trace_id":"tr_1389d52a39","operation":"users.get","type":"result","status":"ok","data":{"id":7,"name":"user-7"},"error":null,"effects":"none","data_count":0}
users.list     {"request_id":"req_14090c5ad6","trace_id":"tr_14090c5ad6","operation":"users.list","type":"data","seq":1,"data":{"id":1,"name":"user-1"},"error":null}
users.list     {"request_id":"req_14090c5ad6","trace_id":"tr_14090c5ad6","operation":"users.list","type":"data","seq":2,"data":{"id":2,"name":"user-2"},"error":null}
users.list     {"request_id":"req_14090c5ad6","trace_id":"tr_14090c5ad6","operation":"users.list","type":"data","seq":3,"data":{"id":3,"name":"user-3"},"error":null}
users.list     {"request_id":"req_14090c5ad6","trace_id":"tr_14090c5ad6","operation":"users.list","type":"result","seq":4,"status":"ok","data":3,"error":null,"effects":"none","data_count":3}
outside-root   {"request_id":"req_158a7c7283","trace_id":"tr_158a7c7283","operation":"rivet.load","type":"result","status":"error","data":null,"error":{"kind":"permission","code":"permission.import_outside_root","message":"../15-ffi/app.rivet is outside the runtime root .","retryable":false},"effects":"none","data_count":0}
```

### 9. One runtime, many threads

#### Command / Request

`tour.c` starts 8 pthreads. Each makes 250 `rivet_request` calls on the **same** `RivetRuntime` and checks that
every envelope is `ok` with the right sum.

```text
  thread 0 ─┐
  thread 1 ─┤                                   RivetRuntime is thread-safe.
     …      ├──▶ rivet_request(shared_rt, …) ×250   A RivetCall belongs to one thread at a time.
  thread 7 ─┘
```

#### Expected Output / Response

```text
== 9. one runtime, many threads
threads        2000/2000 correct envelopes from 8 threads
```

### 10. Highlighting

#### Command / Request

```c
rivet_highlight("global N = 1", "json");     /* or "html", "ansi"; NULL = "json" */
rivet_highlight("global N = 1", "html");
```

#### Expected Output / Response

```text
== 10. highlighting (no runtime needed)
json           {"line":1,"col":1,"len":6,"class":"keyword","text":"global"}
{"line":1,"col":8,"len":1,"class":"global","text":"N"}
{"line":1,"col":10,"len":1,"class":"operator","text":"="}
{"line":1,"col":12,"len":1,"class":"number","text":"1"}

html           <pre class="rv-source"><code><span class="rv-keyword">global</span> <span class="rv-global">N</span> <span class="rv-operator">=</span> <span class="rv-number">1</span></code></pre>
```

The same token classes drive `rivet highlight`, the TextMate grammar and the VS Code extension
([16-editor](../16-editor/README.md)).

### 11. Misuse is refused, not undefined behaviour

#### Command / Request

```c
rivet_request(rt, NULL);
rivet_string_free(local_stack_buffer);    /* not from librivet */
rivet_runtime_free(rt);  rivet_runtime_free(rt);   /* double free */
rivet_request(rt, "…");                    /* use after free */
```

#### Expected Output / Response

```text
== 11. misuse is refused, not undefined behaviour
null-input     {"request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.ffi_argument","message":"`input_json` is NULL","retryable":false,"details":{"argument":"input_json"}},"effects":"none","data_count":0}
foreign-free   RIVET_ERROR
runtime-free   RIVET_OK
double-free    RIVET_ERROR
freed-handle   {"request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.ffi_argument","message":"unknown or already freed RivetRuntime handle","retryable":false,"details":{"argument":"RivetRuntime"}},"effects":"none","data_count":0}
```

Handles and returned strings are tracked. A pointer librivet did not hand out, or one it already freed, is
refused, not dereferenced.

### 12. The same library from Python

#### Command / Request

```sh
RIVET_LIB="$WORK/rivet-0.2.1/lib/librivet.dylib" python3 tour.py     # Linux: librivet.so
```

#### Expected Output / Response

```text
library  0.2.1 abi 1
add      5
global   hello Python
stream   [1, 2, 3, {'count': 3}]
denied   error permission.denied
module   ['get', 'list'] {'id': 7, 'name': 'user-7'}
```

A module is a Python object: `users.get(id=7)` calls `users.get` through `rivet_module_call`.

## What librivet does not do (0.2.1)

| Not available | Instead |
|---|---|
| Serving HTTP / MCP / WebSocket from the host process | `rivet serve` (the CLI); librivet is in-process only |
| Callbacks / push delivery into the host | Pull with `rivet_call_next(call, timeout_ms)` |
| Windows builds | Unsupported (INC-2026-0011) |
| crates.io / package-manager distribution | Release files, or build `rivet-ffi` from the `v0.2.1` tag (G-PUB) |
| ABI-stable structs | Everything crosses the boundary as JSON strings; only opaque handles and `RivetStatus` |

## Release Updates

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-13 | PROP-2026-0002 R13 | C ABI, shared and static | Setup, step 1 | Both links run; 0.2.1 / ABI 1 | this record; `conformance_ffi` |
| U-14 | PROP-2026-0002 R14 | Misuse returns envelopes | Steps 2, 11 | `validation.ffi_argument`, `RIVET_ERROR` | this record |
| U-15 | PROP-2026-0002 R15 | Streams, live input, cancel | Steps 5–7 | Records, timeout, `cancelled`, deadline | this record |
| U-23 | PROP-2026-0002 R23 | Modules through the ABI | Steps 8, 12 | `users.get`, stream, root refusal | this record |

## Cleanup

```sh
rm -rf "$WORK"
```

Nothing is written inside the repository.

## Verification Record

| Step | Verified By | Verified At | Result |
|---|---|---|---|
| Setup: shared (34 KiB) and static (27 MiB) links from `dist/v0.2.1`, outside the repository | Claude | 2026-09-30, v0.2.1 (9074f59), macOS 26.4.1 arm64 | PASS |
| 1–11 `tour-shared` (exit 0) | Claude | 2026-09-30, v0.2.1 (9074f59), macOS 26.4.1 arm64 | PASS |
| 1–11 `tour-static` (exit 0; output equal to shared apart from IDs) | Claude | 2026-09-30, v0.2.1 (9074f59), macOS 26.4.1 arm64 | PASS |
| 9 repeated 5 times: 2000/2000 each | Claude | 2026-09-30, v0.2.1 (9074f59), macOS 26.4.1 arm64 | PASS |
| 12 Python 3.9.6 ctypes against the shipped dylib | Claude | 2026-09-30, v0.2.1 (9074f59), macOS 26.4.1 arm64 | PASS |
| Linux | CI (`conformance_ffi`) | tag run 36638613406 | PASS |

## Known Caveats

- Request, trace and session IDs vary between runs.
- `rivet.pc` is written with `prefix=/usr/local`. If you install elsewhere, pass
  `--define-variable=prefix=…` to pkg-config or use the explicit flags above.
- The static link on macOS may print "built for newer macOS" warnings from ring's assembly. They are harmless;
  the release was built with `MACOSX_DEPLOYMENT_TARGET=26.0`.
- `rivet.h` documents `rivet_version()` with the example string `"0.2.0"`; the function returns the actual
  library version (`0.2.1` here).
- A bare `emits object` (no `field` lines) declares an object with no fields, so emitting `{id, name}` fails
  with `output.invalid`. `lib/users.rivet` declares its fields.
- Windows is not supported (INC-2026-0011).

## Related Documents

- [DEMO-2026-0017 15-ffi](../15-ffi/README.md) (build from source, misuse.c) · [REL-0.2.1](../../releases/rel-0.2.1-release-notes.md)
- [API-2026-0007 C ABI](../../api/api-2026-0007-c-abi.md) · [MAN-2026-0009](../../manuals/man-2026-0009-c-abi-and-ffi.md) · [SYS-2026-0010](../../system/components/sys-2026-0010-ffi-surface-and-packaging.md)
- [API-2026-0006 envelopes](../../api/api-2026-0006-envelopes.md) · [17-modules](../17-modules/README.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-30 | Claude | Created: capability tour against the v0.2.1 release files, C (shared and static) and Python, all executed. |
