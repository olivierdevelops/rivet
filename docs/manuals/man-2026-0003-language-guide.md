---
document_id: MAN-2026-0003
title: "Rivet language guide"
document_type: manual
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 2
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, registry, execution, files, sessions, connectors, auth, transports]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, server, embedded]
audience: [developers, integrators]
scope: How to write .rivet bundles in 0.1.0 — operations, parameters, outputs, fields and errors, expressions, calls, control flow, try/catch, map/poll/iterate, dag, concurrent, with-blocks and resources, secrets, files, connectors and auth profiles — with verified success and failure examples.
reason: PLAN-2026-0001 row D-36 — the language manual describes the implemented grammar (src/infra/rivet.capy and its lowering), not the proposal; REF-2026-0002 keeps the numbered examples.
related_documents: [MAN-2026-0001, MAN-2026-0004, MAN-2026-0005, MAN-2026-0008, REF-2026-0002, SYS-2026-0001, SYS-2026-0002, DEMO-2026-0001, DEMO-2026-0002, DEMO-2026-0005]
supersedes: null
superseded_by: null
tags: [rivet, manual, language, capy, dsl]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.1.0-dev (commit 829ca43)"
next_review_date: 2026-10-28
---

# Rivet language guide

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** language, registry, execution, files, sessions, connectors, auth, transports

## Purpose

This volume teaches the `.rivet` language as implemented in 0.1.0. Each section says why you would use a form,
shows a copy-pasteable example, what it returns, and the error you get when it is misused. The 159 numbered
examples in [REF-2026-0002](../references/ref-2026-0002-language-and-usage.md) are the extended example set; all
of them parse with the 0.1.0 grammar, but where REF-2026-0002 and this guide differ, this guide describes the
build.

## Reading Order

```text
 file layout ─► operations ─► parameters ─► outputs/emits/receives/errors ─► values/expressions
      ─► calls ─► control flow ─► try/catch ─► map/poll/iterate ─► dag ─► concurrent/scope
      ─► with resources ─► files ─► secrets ─► connectors + auth profiles ─► errors reference
```

## Concepts

### File layout and lexical rules

```text
 app.rivet
 ├── auth NAME oauth2 … end            (0..n OAuth profiles)       ─┐  declarations, any order
 ├── connector NAME mcp|grpc … end     (0..n connectors)            │
 └── operation ID … end / pipeline ID … end   (1..n operations)   ─┘

 operation users.get                    ◄── ID: dotted identifiers, unique in the bundle
     name "Get a user"                  ┐
     description "Read one user."       │ HEADER — fixed order:
     private true                       │ name, description, private, param,
     param id integer required …        │ output, emits, receives, error
     output object description "…"      │
         field id integer required …    │
     end                                │
     error "users.not_found" description "…"
     ─────────────────────────────────── ┘ first body statement ends the header
     response = http get "https://…/${id}"
         accept status [200, 404]       ◄── resource OPTIONS lead their block
         decode json
     end
     return response.body
 end
```

| Rule | Detail | Violation |
|---|---|---|
| Indentation | four spaces per level; blocks end with `end` | `syntax.e0001 … expected closer "end"` |
| Comments | `#` to end of line | — |
| Strings | `"…"` with `\n \t \" \\ \xNN \uNNNN`; backticks for multiline | — |
| Interpolation | `"${dotted.path}"` only — no expressions inside `${}` | `syntax.interpolation` |
| Durations | always quoted: `"100ms"`, `"5s"`, `"2m"`, `"1h"` | `timeout 5s` → `syntax.option_misplaced` / `validation.*` |
| Calls | Capy prefix form `(fn arg …)`; never `fn(arg)` | `syntax.fcall_style` |
| Header order | `name, description, private, param, output, emits, receives, error` | `syntax.header_order` |
| Header after body | header lines only before the first statement | `syntax.option_after_body` |
| Output required | every operation declares `output` | `syntax.output_required` |

## Task-Oriented Workflows

All examples were compiled and run with `rivet 0.1.0-dev` (commit `f40d4aa`); the fix-batch forms (`else`,
infix inside literals, `with file open`, `if_version`, secret taint, DAG timestamps, unknown functions) were run at
commit `829ca43` from a scratch bundle. IDs and timestamps in outputs vary per run.

### Write an operation

Why: an operation is the unit every surface calls. `pipeline` is a synonym used for composition-heavy code.

```rivet
operation demo.greet
    name "Greet a person"
    description "Return a greeting for the supplied person."
    param person text required description "The person's display name."
    output text description "Greeting that contains the person's name."
    return "Hello, ${person}!"
end
```

```bash
rivet request --file app.rivet demo.greet --params '{"person":"Ada"}'
```

```text
{"request_id":"req_01fe6f6e2d","trace_id":"tr_01fe6f6e2d","result":"Hello, Ada!","data_count":0,"effects":"none"}
```

`private true` hides an operation from `list`, `describe`, every surface and direct calls; other operations can
still `(request …)` it (demo [05-dag](../demos/05-dag/README.md)):

```text
$ rivet request --file app.rivet math.double --params '{"value":1}'        # private → [exit 4]
{"…","error":{"kind":"not_found","code":"not_found.operation","message":"no operation `math.double`",…}}
```

Compile-time failures (exit 2):

```text
error[registry.duplicate_id]: operation ID `a.b` is declared twice (first at app.rivet:1)
error[syntax.output_required]: operation `a.b` must declare its `output`
error[syntax.option_after_body]: `description` is a header line and must come before the first body statement
error[syntax.header_order]: `private` is out of order; header order is name, description, private, param, output, emits, receives, error
```

### Declare parameters

```text
param NAME TYPE (required | default VALUE | optional) [min N] [max N] [enum [..]] [description "…"]
TYPE: text · integer · number · boolean · json · object · bytes · list T
```

```rivet
operation lang.control
    name "Control flow"
    description "if, for, while, iterate, break and += in one operation."
    param n integer default 3 min 1 max 10 description "How many items."
    param mode text default "sum" enum ["sum", "list"] description "Result shape."
    output json description "Sum or list."
    total = 0
    items = []
    for i in [1, 2, 3, 4, 5]
        if i > n
            break
        end
        total += i
        items += [i]
    end
    count = 0
    iterate max 4
        count += 1
    end
    if mode == "list"
        return items
    end
    return {total: total, count: count, len: (length items)}
end
```

| Call | Result / error | Exit |
|---|---|---|
| `--params '{}'` | `{"total":6,"count":4,"len":3}` | 0 |
| `--params '{"n":2,"mode":"list"}'` | `[1,2]` | 0 |
| `--params '{"n":20}'` | `validation.max` "parameter `n` must be ≤ 10" | 2 |
| `--params '{"mode":"x"}'` | `validation.enum` "parameter `mode` must be one of [\"sum\",\"list\"]" | 2 |
| `--params '{"zz":1}'` | `validation.unknown_field` with `details.known: ["n","mode"]` | 2 |
| `--params '{"a":"x"}'` on an integer | `validation.type` | 2 |
| missing required | `validation.required` "missing required parameter `a`" | 2 |

Unknown fields are always rejected; defaults are applied after the name check.

### Declare outputs, emits, receives and errors

Why: callers (and MCP clients, via `outputSchema`) rely on the declared shape; Rivet validates every result.

```rivet
operation demo.health
    name "Check availability"
    description "Return a constant readiness response without I/O."
    output object description "Readiness report for this catalog."
        field ready boolean required description "True whenever the host can run pure operations."
    end
    return {ready: true}
end
```

- `field NAME TYPE [required|optional] [description "…"]`; an `object` field opens a nested block closed by `end`.
- `open true` inside an object block allows extra fields.
- `emits TYPE` / `emits object … end` declares streamed items; `receives TYPE` declares live input. Their
  `description` is shown by `outputs`/`describe` and carried in the item JSON Schema (since `2a751ab`).
- `error "CODE" description "…"` documents a failure code; `fail "CODE" {details}` raises it.

Result violating the declaration (exit 5, HTTP 500):

```text
{"…","error":{"kind":"output_invalid","code":"output.invalid","message":"`lang.bad_output` returned a result that does not match its declared output: at $ expected integer, found text","retryable":false,"effects":"none","operation_id":"lang.bad_output","details":{"violations":[{"path":"$","expected":"integer","found":"text"}]}}}
```

A declared error raised with `fail` has kind `application` and exit 5; its message is the declared description:

```text
{"…","error":{"kind":"application","code":"users.not_found","message":"404 from the service.",…,"details":{"id":7}}}
```

Emitting without `emits` fails at run time: `stream.emits_undeclared` (exit 2).

`rivet check --strict-docs` requires descriptions on public operations, params, outputs and fields; each missing
one is `docs.description` / `docs.output_description` (exit 2).

### Stream data and receive live input

```rivet
operation lang.echo_input
    name "Echo live input"
    description "Receive items and emit each back."
    output object description "Summary."
        field received integer required description "Items received."
    end
    emits json description "Each input item."
    receives json description "Items to echo."
    n = 0
    for item in incoming
        emit item
        n += 1
    end
    return {received: n}
end
```

```bash
printf '{"a":1}\n"two"\n' | rivet request --file app.rivet lang.echo_input --stream --input-jsonl -
```

```text
{"request_id":"req_010049b3fd","trace_id":"tr_010049b3fd","seq":1,"type":"data","data":{"a":1}}
{"request_id":"req_010049b3fd","trace_id":"tr_010049b3fd","seq":2,"type":"data","data":"two"}
{"request_id":"req_010049b3fd","trace_id":"tr_010049b3fd","result":{"received":2},"data_count":2,"effects":"none","type":"result"}
```

```text
 stdin JSONL ──line──► incoming ──for item──► emit ──► NDJSON stdout
            EOF ─────► finish_input ──► loop ends ──► return {received: n} ──► type:"result"
```

| Misuse | Error | Exit |
|---|---|---|
| calling a `receives` operation without input | `stream.input_required` | 2 |
| `--input-jsonl -` without `--stream` | `validation.usage` "--input-jsonl - needs --stream" | 2 |
| a stdin line that is not JSON | `validation.input` "stdin line 2 is not JSON …; the request was cancelled" | 2 |
| a stdin line that does not match `receives` | `validation.input` "stdin line 2: input item at $ must be text, got integer; the request was cancelled", carrying the cancelled request's `request_id`/`trace_id` (since `2a751ab`) | 2 |

### Values and expressions

| Form | Example | Notes |
|---|---|---|
| literals | `1`, `1.5`, `"x"`, `true`, `null`, `[1, 2]`, `{a: 1}` | object keys are bare identifiers |
| paths | `response.body.items`, `obj.a` | missing key → `value.missing_key` (exit 2) |
| operators | `+ - * / %`, `== != < <= > >=`, `and`, `or`, `not`, parentheses | integer `/` truncates (`7 / 2` → `3`); `+` joins strings and lists; `+=` appends |
| interpolation | `"a=${obj.a}"` | dotted paths only |
| helpers | `(length x)`, `(keys obj)`, `(text 42)`, `(base64.encode "hi")`, `(base64.decode "AAEC")`, `(xml.element name attrs content)` | pure, never perform I/O; with `(request …)` and `(request.stream …)` these are the **only** functions |

Operator expressions work anywhere a value is expected, including inside object and list literals and call
arguments: `return {next: n - 1, items: [n + 1, n * 2]}` with `n = 5` returns `{"next":4,"items":[6,10]}`. `!` and
`&&` are not operators (`syntax.unknown_statement`, `syntax.trailing`); use `not` and `and`.

Any other prefix-call name is rejected **at compile time** with the closest built-in as a hint:

```text
$ rivet --file bad2.rivet check                                                            [exit 2]
error[check.unknown_function]: unknown function `lenght`
  --> bad2.rivet:6:13
   |
  6|     return (lenght items)
   |             ^^^^^^
  = hint: did you mean `length`?
```

```text
return {len: (length "héllo"), keys: (keys obj), t: (text 42), b64: (base64.encode "hi"), dotted: "a=${obj.a}"}
→ {"len":5,"keys":["a","b"],"t":"42","b64":"aGk=","dotted":"a=1"}
```

Scoping: **assignments are operation-scoped** (a variable assigned inside `for`/`if` is visible after it);
**loop variables, the `catch` variable `error`, `map` items and params are block-local**:

```text
for i in [1, 2] … inner = i … end ; return inner   → 2
for i in [1, 2] … end ; return i                    → value.missing_key "`i` is not defined" (exit 2)
```

### Call other operations

```rivet
value = (request "data.read" {})          # literal ID: checked at compile time
```

- A literal call to an unknown ID fails `check.unknown_operation` at compile time; mutual literal calls fail
  `check.call_cycle` ("operations call each other in a cycle: a.b → a.c → a.b").
- A **dynamic** ID must be constrained with an `allow` block; anything else is `permission.denied`:

```rivet
operation a.b
    param op text required
    output json
    v = (request op {})
        allow ["a.c"]
    end
    return v
end
```

```text
--params '{"op":"a.c"}' → "c"                                                         [0]
--params '{"op":"a.d"}' → permission.denied "dynamic request to a.d is not in the `allow` list"  [3]
```

- Nested calls share the caller's deadline and `limits.max_call_depth` (default 16).
- Consume a child's stream with `with (request.stream "id" {…}) as events`; iterating `events` yields the
  child's **data values** (not envelopes):

```rivet
total = 0
with (request.stream "a.c" {}) as events
    for ev in events
        total += ev
    end
end
return total                       # a.c emits 1 and 2 → 3
```

### Control flow

| Form | Rule |
|---|---|
| `if COND … end` / `if COND … else … end` | one `else` per `if`, at the `if`'s indentation, closed by the `if`'s `end`. There is **no `else if`**: nest an `if … end` inside the `else` body |
| `for NAME in ITER … end` | lists, `incoming`, streams and resource handles |
| `while COND … end` | any condition; bound it with a deadline (request `--timeout`, `scope timeout`) |
| `break` | exits the innermost loop |
| `iterate max N … end` | bounded sequential loop |
| `return EXPR` | exits the whole operation (also from inside `map`/`poll`) |
| `emit EXPR` | one streamed item (needs `emits`) |
| `fail "CODE" {…}` | raise an application error |

```rivet
operation demo.sign
    name "Sign"
    description "Classify a number with if/else."
    param n integer required description "Number."
    output text description "positive, zero or negative."
    if n > 0
        return "positive"
    else
        if n == 0
            return "zero"
        end
    end
    return "negative"
end
```

```text
--params '{"n":-3}' → {"request_id":"req_01d71955fd",…,"result":"negative",…}      [0]
--params '{"n":0}'  → {"request_id":"req_01d531049d",…,"result":"zero",…}          [0]

 if n > 0 ──yes──► return "positive"
    │ no
    └─ else ─► if n == 0 ──yes──► return "zero"
                  │ no
                  └──────────────► (after end) return "negative"
```

Misplaced `else` fails to compile (exit 2):

```text
error[syntax.else_if]: `else` takes no condition; `else if COND` is not supported
  --> bad.rivet:6:5
  = hint: put `else` alone on its line and nest `if COND … end` inside its body
error[syntax.else_without_if]: `else` must follow the body of an `if` at the same indentation (one `else` per `if`, closed by the `if`'s `end`)
  --> bad.rivet:8:5
  = hint: write `if COND` … `else` … `end`; for else-if, nest an `if COND … end` inside the `else` body
```

`rivet graph ID` shows both arms of every `if` (MAN-2026-0004).

### Recover with try and catch

```rivet
a = "none"
try
    x = (request "lang.boom" {})
catch error code "lang.boom"
    a = "caught ${error.code}"
end
b = "none"
try
    y = (request "lang.control" {n: 99})
catch error kind validation
    b = error.message
end
return {by_code: a, by_kind: b}
```

```text
{"by_code":"caught lang.boom","by_kind":"parameter `n` must be ≤ 10"}
```

- Filters: `catch error` (any), `catch error kind K`, `catch error code "C"`. A kind filter also matches codes
  that start with `K.`. Cancellation is never caught.
- `try` must be followed directly by `catch` (`syntax.try_without_catch`, `syntax.catch_without_try`).
- `finally` does **not** exist in 0.1.0: `syntax.unknown_statement: unknown statement \`finally\``. Put cleanup in
  `with` blocks, which always close.

### Map, poll and iterate

```rivet
squares = map v in [1, 2, 3] limit 2
    yield v * v
end
return squares                                   # → [1,4,9], input order kept
```

```rivet
status = poll every "1s" timeout "30s"
    job = (request "jobs.get" {id: id})
    until job.state == "done"
    yield job
end
```

- `map` runs at most `limit` items at once; `yield` gives each item's value.
- `poll` re-runs its body every interval; when the `until` condition holds, `yield` gives the block value. If the
  timeout passes first: `timeout.poll` "poll did not complete within 300 ms" (exit 6).

### Run a DAG

```rivet
pipeline report.total
    name "Compute a DAG total"
    description "Run two nodes independently and combine their results."
    param a integer required description "First starting value."
    param b integer required description "Second starting value."
    output object description "Combined DAG result."
        field total integer required description "(a * 2) + (b * 2)."
    end
    dag limit 2 timeout "5s" fail fast
        node first = (request "math.double" {value: a})
        node second = (request "math.double" {value: b})
        node total after [first, second] = (request "math.sum" {a: first.result, b: second.result})
    end
    return {total: total.result}
end
```

```text
 first ──┐
         ├──► total          node value = {status, result, error, started_at, ended_at}
 second ─┘                   status: succeeded | failed | blocked | …
```

Each node value carries RFC 3339 `started_at`/`ended_at` timestamps (millisecond precision). With `fail fast`, the
request's error keeps the failing node's `node_id` and adds `details.nodes` — every node with its status and
timestamps. Captured at `829ca43` (a scratch bundle with `w.one` returning 1 and `w.boom` failing `w.boom`):

```text
dag fail independent → return {a: a, b: b}
{"a":{"status":"succeeded","result":1,"error":null,"started_at":"2026-09-28T09:52:42.060Z","ended_at":"2026-09-28T09:52:42.066Z"},
 "b":{"status":"failed","result":null,"error":{"kind":"application","code":"w.boom",…,"node_id":"b","details":{}},"started_at":"…","ended_at":"…"}}

dag fail fast → [exit 5]
{"…","error":{"kind":"application","code":"w.boom","message":"always",…,"node_id":"b",
 "details":{"nodes":[{"id":"a","status":"succeeded","started_at":"2026-09-28T09:52:42.085Z","ended_at":"2026-09-28T09:52:42.085Z"},
                     {"id":"b","status":"failed","started_at":"2026-09-28T09:52:42.085Z","ended_at":"2026-09-28T09:52:42.085Z"}]}}}
```

With `fail independent`, `NODE.result` is `null` unless the node succeeded; `rivet check` warns
(`check.unguarded_result`) when a `return` reads it without an `if NODE.status == "succeeded"` guard.

| Mode | Behaviour | Verified output |
|---|---|---|
| `fail fast` (default) | first node failure fails the request with that node's error (`node_id` set) | `{"kind":"application","code":"demo.bad",…,"node_id":"x"}` exit 5 |
| `fail independent` | independent nodes finish; dependents of a failed node are `blocked` | `{"good":"succeeded","bad":"failed","blocked":"blocked"}` |
| either, fatal error | the error lists every node (`details.nodes`: id, status, `started_at`, `ended_at`) | see above |

`report.total` with `{"a":2,"b":3}` returns `{"total":10}`. Demo: [05-dag](../demos/05-dag/README.md).

### Concurrent tasks and scopes

```rivet
concurrent limit 2 timeout "2s" fail fast
    task left
        l = (request "lang.control" {n: 2})
    end
    task right
        r = (request "lang.control" {n: 3})
    end
end
```

Both tasks are joined before the block ends; `fail fast` cancels siblings on the first failure,
`fail independent` lets them finish. `scope timeout "D" … end` puts a deadline on everything inside:

```text
scope timeout "200ms" (poll that never finishes) → timeout.scope "scope exceeded 200 ms"  [exit 6]
```

### Own resources with with

```text
 with RESOURCE TARGET as NAME        acquire (broker check, connect, spawn…)
     option lines…                   ◄── options first
     body using NAME                 ◄── NAME cannot escape the block
 end                                 dispose: close handles in reverse order, within 5 s,
                                     on return, fail, break or cancel
```

Cancellation is **structured**: a caller cancel, a deadline or a server shutdown fires the request's cancellation
token, which the run observes at its next await point; it then leaves every `with` block normally, so each
handle's close runs (in reverse order, 5 s grace) and child processes are reaped. Only a run that ignores the
grace is dropped.

| Resource | Example header | Guide |
|---|---|---|
| HTTP stream | `with http get "URL" as events` + `stream sse\|jsonl\|lines\|bytes` | MAN-2026-0008 |
| WebSocket | `with websocket "wss://…" as socket` | MAN-2026-0008 |
| TCP / Unix | `with tcp "HOST:PORT" as conn` / `with unix "/path.sock" as conn` | MAN-2026-0008 |
| UDP | `with udp "HOST:PORT" as s`, `with udp bind …`, `with udp multicast …` | MAN-2026-0008 |
| QUIC | `with quic "quic://HOST:PORT" as c` + `with c.open bidi as stream` | MAN-2026-0008 |
| gRPC | `with grpc CONNECTOR.Method as rpc` | MAN-2026-0008 |
| process | `with command "/abs/bin" as p` | MAN-2026-0008 |
| child stream | `with (request.stream "id" {…}) as events` | this volume |
| file handle | `with file open "PATH" mode read\|write\|append as f` (+ `chunk_size N`) | [Files](#files) |

`with file watch …` is Stage C: it parses but fails at run time with `unsupported.stage_c` ("`with file watch` is
Stage C and not available in this build", exit 5).

### Files

Why: each verb states intent, so `rivet io` and `policy.json` can tell a create from an overwrite.

| Statement | Access verbs (capability) | Behaviour |
|---|---|---|
| `x = file read "P" as json\|text\|bytes` | read (`allow_read`) | missing → `not_found.file` |
| `file list "DIR"` | list (`allow_read`) | `[{name, type, size}]` sorted by name |
| `file stat "P"` | stat (`allow_read`) | `{path, type, size, version}` |
| `file create "P" json\|text\|bytes V` | create (`allow_write`) | exists → `conflict.already_exists` (exit 4), file unchanged |
| `file update "P" json V` (+ `if_version V` … `end`) | stat + update | missing → `not_found.file`; with `if_version`, a compare-and-replace under a file lock (below) |
| `file write "P" …` | create, update | create or replace |
| `file append "P" …` | append | — |
| `file delete "P"` (+ `missing ok` … `end`) | delete (`allow_delete`) | without `missing ok`, missing → `not_found.file` (exit 4) |
| `file copy "A" to "B"` / `file move "A" to "B"` | read A + create B (+ delete A for move) | — |

Verified run ([02-file-crud](../demos/02-file-crud/README.md), in a scratch copy because it writes `./out`):

```text
notes.read              → not_found.file "./out/note.json: no such file"      [4]
notes.create {"text":"hello"} → {"created":true}  effects "committed"         [0]
notes.create again      → conflict.already_exists "./out/note.json already exists" [4]
notes.update {"text":"v2"}    → {"updated":true}                              [0]
notes.delete (missing ok), twice → {"absent":true} both times                 [0]
```

**Conditional update.** `if_version V` compares the file's current `version` (from `file stat`) with `V` and
replaces the file only when they match; otherwise `conflict.version` (exit 4) and the file is unchanged. On Unix
the compare and the replace happen under an exclusive advisory `flock` on the file (the descriptor is re-checked
by device+inode after locking, the new content is written to a temporary sibling, fsynced and renamed while the
lock is held), so two Rivet writers can never both win. A writer waiting more than 10 s for the lock fails
`timeout.file_lock` (exit 6). Processes that write without taking the lock are not stopped. On platforms without
`flock` (non-Unix) a conditional update is refused with `unsupported.conditional_update` (exit 5) before anything
is written.

```text
 file stat  ──► version v:b628…        file update P … if_version "v:b628…"
                                          open no-follow ─► flock(EX) ─► same dev+ino? ─► version == V?
                                                                              │ no            │ no → conflict.version
                                                                              └ retry         └ yes → temp ─► fsync ─► rename
```

```text
cfg.update {"seen":"stale"}              → conflict.version "./out/log.txt changed: version v:b6285c57e8797db5, expected stale"  [4]
cfg.update {"seen":"v:b6285c57e8797db5"} → {"updated":true}  effects "committed"                                               [0]
```

**Scoped file handles.** `with file open "PATH" mode read|write|append as NAME` opens one handle for the block
(no symlinks anywhere on the path, relative to the bundle root) and closes it at `end`:

| Mode | Use | Grants (access verbs) | Notes |
|---|---|---|---|
| `read` (default) | `for chunk in NAME` yields **bytes** chunks of at most `chunk_size` | `allow_read` read | a non-regular file → `file.not_regular` (exit 2) |
| `write` | `NAME.write text\|bytes V` creates or truncates | `allow_write` create + update | hard-linked files refused |
| `append` | `NAME.write …` appends | `allow_write` append | never creates: missing → `not_found.file` (exit 4) |

`chunk_size N` is an **option line** directly under the `with` line (default 65536, allowed 1 … 8 MiB, outside →
`limit.chunk_size`); written on the `with` line itself it is not read. Each `write` returns
`{path, written, total}`. Iterating a write handle or writing a read handle is `unsupported.iterate` /
`unsupported.method`; `with file VERB` other than `open` is `validation.file_scope`; a bad mode is
`validation.file_mode` (exit 2).

```rivet
operation demo.copy
    name "Copy a file in chunks"
    description "Read a file through a scoped handle."
    param path text required description "File."
    output integer description "Chunks read."
    count = 0
    with file open path mode read as src
        chunk_size 4
        for chunk in src
            count = count + 1
        end
    end
    return count
end

operation log.write
    name "Write a log"
    description "Write two lines through one handle."
    param text text required description "Second line."
    output json description "Write receipt."
    with file open "./out/log.txt" mode write as out
        out.write text "one\n"
        r = out.write text text
    end
    return r
end
```

```text
demo.copy {"path":"data/a.txt"}   (12 bytes)   → 3                                                     [0]
demo.copy {"path":"data/missing.txt"}           → not_found.file "data/missing.txt: no such file"       [4]
log.append (before the file exists)             → not_found.file "./out/log.txt: no such file"          [4]
log.write {"text":"two\n"}                      → {"path":"./out/log.txt","written":4,"total":8}  committed [0]
log.append                                      → {"path":"./out/log.txt","written":6,"total":6}  committed [0]
```

A variable named like a codec never replaces the codec keyword: in `out.write text text` (or
`file append P text text`) the first `text` is the codec and the second is the variable.

Any write or delete through a file with more than one hard link is refused:

```text
{"…","error":{"kind":"permission","code":"file.hardlink_refused","message":"./out/link.txt has 2 hard links; write/delete refused",…}}   [3]
```

Relative paths resolve against the bundle directory; every file effect also needs a grant (MAN-2026-0005).

### Use secrets

```rivet
secret TOKEN from env "FIXTURE_TOKEN" for "http://127.0.0.1:18480"
response = http post "http://127.0.0.1:18480/users"
    header "Authorization" "Bearer ${TOKEN}"
    body json {name: name}
    decode json
end
```

- Reading the variable needs an `allow_env` grant for `FIXTURE_TOKEN`; without it: `permission.denied`
  "allow_env read FIXTURE_TOKEN denied: … (add a grant for FIXTURE_TOKEN)" (exit 3).
- An unset variable: `not_found.env` "environment variable RIVET_DEMO_TOKEN is not set" (exit 4).
- **The value is tainted and may reach only its bound origins.** Rivet tracks the value and every recognisable
  form derived from it by explicit flows — assignment, interpolation, string/list/object construction and the
  encoding helpers (`base64.encode`, `text`, …; derived forms of at least 4 bytes). Every sink is checked:
  headers, body, query and URL of any effect, `with` opens, socket/stream/UDP sends, process argv/stdin/env,
  file writes, nested `(request …)` params (so MCP and gRPC calls too), `return` and `emit`. Only a **network**
  destination whose `scheme://host:port` is one of the `for` origins passes; files, processes, Unix sockets,
  pipes, nested requests, `return` and `emit` never do. Refusals are `permission.denied` with
  `details.secret` (exit 3), before anything is sent or written. Implicit flows (branching on a secret, its
  length, timing) are not tracked.

```text
 secret token … for "https://api.example.com"
    │ "Bearer ${token}"  (base64.encode token)  {auth: token} …   ── still tainted
    ▼
  http … "https://api.example.com/…"   ✓        http … "https://other.example.com" ✗ bound to …
  file create … / command … / return / emit  ✗   (request "x" {t: token}) ✗
```

Captured at `829ca43` (`secret token from env "RIVET_DEMO_TOKEN" for "https://api.example.com"`, with `allow_env`
and `allow_write ./out/**` granted):

```text
return "Bearer ${token}"                → permission.denied "secret `token` cannot be returned; secrets may only reach their bound origins"  [3]
emit (base64.encode token)              → permission.denied "secret `token` cannot be emitted; secrets may only reach their bound origins"   [3]
file create "./out/t.txt" text token    → permission.denied "secret `token` may not reach `file`; secrets travel only to their bound network origins (https://api.example.com)"  [3]
(variable unset)                        → not_found.env "environment variable RIVET_DEMO_TOKEN is not set"  [4]
```

### Declare connectors and auth profiles

```rivet
auth crm_service oauth2
    flow client_credentials
    issuer "https://auth.example.com"
    token_url "https://auth.example.com/token"
    client_id "rivet-service"
    client_secret env "CRM_CLIENT_SECRET"
    client_auth basic
    scopes ["contacts.read"]
    resource_origins ["https://api.example.com:443"]
    store memory
end

connector crm mcp
    transport http "https://mcp.example.com/mcp"
    schema "./schemas/crm.json"
    expose tools ["search"]
end

connector users grpc
    endpoint "https://users.example.com:443"
    descriptor "./schemas/users.pb"
    service "example.Users"
end
```

Use a profile inside `http`, `grpc` or `connector` with `auth PROFILE account "ACCOUNT"`. Imported MCP tools
become operations named `CONNECTOR.tools.NAME` (for example `(request "crm.tools.search" {query: q})`); gRPC
methods are called with `grpc CONNECTOR.Method`. Loading fails until the connector's files exist and, for MCP,
until the snapshot is approved — see MAN-2026-0008.

## Complete Language Reference

Statement shapes (the 0.1.0 grammar, `src/infra/rivet.capy`):

| Keyword | Shape |
|---|---|
| declarations | `operation ID` / `pipeline ID` / `connector NAME mcp\|grpc` / `auth NAME oauth2` … `end` |
| header | `name`, `description`, `private`, `param`, `output`, `emits`, `receives`, `error` |
| assignment | `NAME = EXPR`, `NAME += EXPR`, `NAME = map …`, `NAME = poll …` |
| statements | `return`, `yield`, `emit`, `fail`, `break`, bare `(call …)`, `NAME.method …` |
| blocks | `if` (+ `else`), `while`, `for`, `try` + `catch`, `dag` + `node`, `concurrent` + `task`, `scope`, `iterate`, `with` (incl. `with file open`) |
| effects | `http METHOD URL`, `grpc C.M`, `command BIN`, `file VERB …`, `secret NAME from env "V" for "ORIGIN"` |
| functions | `request`, `request.stream`, `length`, `base64.encode`, `base64.decode`, `text`, `keys`, `xml.element` — any other name is `check.unknown_function` |
| options | `timeout`, `decode`, `body`, `header`, `query`, `accept`, `retry`, `redirect`, `version`, `tls`, `unix`, `stream`, `framing`, `max_frame`, `max_datagram`, `alpn`, `max_streams`, `migration`, `datagrams`, `message`, `metadata`, `args`, `stdin`, `env`, `cwd`, `bind`, `interface`, `missing`, `overwrite`, `if_version`, `chunk_size`, `limit`, `allow`, OAuth keys |

Not in 0.1.0: `else if`, `finally`, `import`, `${expression}` interpolation, function-call syntax, unquoted
durations; Stage C forms (`with file watch`, `with pipe`, TCP/Unix `tls`, `reconnect`, `interactive true`).

## Errors and Recovery Reference

| Error / Code | Surface | Cause | User-Visible Result | Recovery | Retry Safe | Related Feature |
|---|---|---|---|---|---|---|
| `syntax.e0001`, `syntax.unknown_statement` | compile | missing `end`, unknown keyword (`finally`, `import`) | caret at the line, exit 2 | fix the block | no | layout |
| `syntax.else_if`, `syntax.else_without_if` | compile | `else if COND`, orphan or second `else` | caret + hint, exit 2 | nest `if … end` inside `else` | no | control flow |
| `check.unknown_function` | compile | `(lenght x)` | hint "did you mean `length`?", exit 2 | use a built-in function | no | expressions |
| `syntax.fcall_style` | compile | `length(x)` | hint "write (length arg …)", exit 2 | use prefix calls | no | expressions |
| `syntax.interpolation` | compile | `"${1 + 1}"` | exit 2 | assign first, interpolate the name | no | expressions |
| `syntax.header_order`, `syntax.option_after_body`, `syntax.option_misplaced` | compile | header/option in the wrong place | exit 2 | move the line | no | layout |
| `check.unknown_operation`, `check.call_cycle`, `registry.duplicate_id` | compile | bad composition | exit 2 | fix IDs | no | calls |
| `validation.*` | request | bad params | 422 / exit 2 | fix params | no | params |
| `value.missing_key` | request | reading an absent key or out-of-scope name | exit 2 | check the path/scope | no | expressions |
| `stream.emits_undeclared`, `stream.input_required` | request | streaming misuse | exit 2 | declare `emits`; send input | no | streams |
| `output.invalid` | request | result shape wrong | 500 / exit 5 | fix the return or the declaration | no | outputs |
| `timeout.request`, `timeout.poll`, `timeout.scope` | request | deadline hit | 504 / exit 6 | raise the deadline | caller decides | deadlines |
| `unsupported.stage_c` | request | `with file watch` | exit 5 | use `file list`/`file stat` polling | no | files |
| `conflict.version`, `timeout.file_lock`, `unsupported.conditional_update` | request | `if_version` mismatch / lock held > 10 s / non-Unix | exit 4 / 6 / 5 | re-read and retry / retry / run on Unix | re-read first | files |
| `limit.chunk_size`, `file.not_regular`, `validation.file_mode` | request | bad file handle | exit 5 / 2 / 2 | fix the `with file open` line | no | files |
| `permission.denied` (`details.secret`) | request | a secret reached a sink outside its bound origins | exit 3 | send it only to the `for` origin | no | secrets |

## Limitations

The language rows of the [manual's Known Limitations](man-2026-0001-rivet-manual.md#known-limitations):

- No `import`: a bundle is one file.
- No `finally` (and no `else if`; nest an `if` inside `else`).
- Stage C forms are refused at run time (`with file watch`, `with pipe`, TCP/Unix `tls`, `reconnect`,
  `interactive true`).

Secret taint follows explicit flows only; implicit flows (for example `if SECRET == "x"`) are not tracked by design.

## Version Applicability

| Feature / Interface | Introduced | Changed | Deprecated / Removed | Applicable Environment |
|---|---|---|---|---|
| Grammar and lowering described here | 0.1.0 | — | — | all |

## Related Documents

- [Rivet manual](man-2026-0001-rivet-manual.md) · [CLI reference](man-2026-0004-cli-reference.md) ·
  [Policy guide](man-2026-0005-policy-and-io-manifest-guide.md) · [Protocols](man-2026-0008-protocols-and-connectors.md)
- [REF-2026-0002 numbered examples](../references/ref-2026-0002-language-and-usage.md)
- [SYS-2026-0001 compiler and catalog](../system/components/sys-2026-0001-compiler-and-catalog.md) ·
  [SYS-2026-0002 execution, scopes and DAG](../system/runtime/sys-2026-0002-execution-scopes-and-dag.md)
- Demos: [01-catalog](../demos/01-catalog/README.md), [02-file-crud](../demos/02-file-crud/README.md),
  [04-streaming](../demos/04-streaming/README.md), [05-dag](../demos/05-dag/README.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial language guide for 0.1.0, verified against 0.1.0-dev commit f40d4aa. |
| 2 | 2026-09-28 | Claude | Fix batch through 829ca43 and 2a751ab: `if … else … end` (`syntax.else_if`, `syntax.else_without_if`), infix inside objects/lists/arguments, `check.unknown_function`, DAG `started_at`/`ended_at` and `details.nodes`, `check.unguarded_result`, structured cancellation, `with file open` handles and `chunk_size`, `if_version` compare-and-replace, codec keywords, secret taint on every sink; limitations aligned with MAN-2026-0001. |
