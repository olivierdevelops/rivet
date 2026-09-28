---
document_id: MAN-2026-0003
title: "Rivet language guide"
document_type: manual
status: active
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 4
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
scope: How to write .rivet bundles in 0.2.0 — global constants, file modules (import), operations, parameters, outputs, fields and errors, expressions, calls, control flow, try/catch, map/poll/iterate, dag, concurrent, with-blocks and resources, secrets, files, connectors and auth profiles — with verified success and failure examples.
reason: PLAN-2026-0001 row D-36 and PLAN-2026-0002 row D-22 — the language manual describes the implemented grammar (src/infra/rivet.capy and its lowering), not the proposal; REF-2026-0002 keeps the numbered examples.
related_documents: [MAN-2026-0001, MAN-2026-0004, MAN-2026-0007, MAN-2026-0009, PLAN-2026-0002, INC-2026-0009, MAN-2026-0005, MAN-2026-0008, REF-2026-0002, SYS-2026-0001, SYS-2026-0002, DEMO-2026-0001, DEMO-2026-0002, DEMO-2026-0005]
supersedes: null
superseded_by: null
tags: [rivet, manual, language, capy, dsl]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.2.0-rc (main at e7ed8ed)"
next_review_date: 2026-10-29
---

# Rivet language guide

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** language, registry, execution, files, sessions, connectors, auth, transports

## Purpose

This volume teaches the `.rivet` language as implemented in 0.2.0 (0.1.0 plus `global` constants, `import` of
file modules and numeric list indexes such as `xs.0`). Each section says why you would use a form,
shows a copy-pasteable example, what it returns, and the error you get when it is misused. The 159 numbered
examples in [REF-2026-0002](../references/ref-2026-0002-language-and-usage.md) are the extended example set; all
of them parse with the 0.2.0 grammar, but where REF-2026-0002 and this guide differ, this guide describes the
build.

## Reading Order

```text
 file layout ─► globals ─► modules (import) ─► operations ─► parameters ─► outputs/emits/receives/errors ─► values/expressions
      ─► calls ─► control flow ─► try/catch ─► map/poll/iterate ─► dag ─► concurrent/scope
      ─► with resources ─► files ─► secrets ─► connectors + auth profiles ─► errors reference
```

## Concepts

### File layout and lexical rules

```text
 app.rivet
 ├── import "./users.rivet" as users [public]   (0..n, before any declaration)
 ├── global NAME = EXPR                 (0..n constants, top level, in dependency order)
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

Examples were first verified on `rivet 0.1.0-dev` (commits `f40d4aa`, `829ca43`); every request output below was
re-captured on the 0.2.0 release candidate (`main` at `e7ed8ed`), so it shows the 0.2.0 envelope
([API-2026-0006](../api/api-2026-0006-envelopes.md)). IDs and timestamps in outputs vary per run. Operation input
is passed with `--data` (the 0.1.0 `--params` is a deprecated alias).

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
rivet request --file app.rivet demo.greet --data '{"person":"Ada"}'
```

```text
{"request_id":"req_0100406015","trace_id":"tr_0100406015","operation":"demo.greet","type":"result","status":"ok","data":"Hello, Ada!","error":null,"effects":"none","data_count":0}
```

`private true` hides an operation from `list`, `describe`, every surface and direct calls; other operations can
still `(request …)` it (demo [05-dag](../demos/05-dag/README.md)):

```text
$ rivet request --file app.rivet math.double --data '{"value":1}'        # private → [exit 4], on stderr
{"request_id":"req_01ff71ba15","trace_id":"tr_01ff71ba15","operation":"math.double","type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"not_found.operation","message":"no operation `math.double`","retryable":false,"operation_id":"math.double"},"effects":"none","data_count":0}
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
| `--data '{}'` | `{"total":6,"count":4,"len":3}` | 0 |
| `--data '{"n":2,"mode":"list"}'` | `[1,2]` | 0 |
| `--data '{"n":20}'` | `validation.max` "parameter `n` must be ≤ 10" | 2 |
| `--data '{"mode":"x"}'` | `validation.enum` "parameter `mode` must be one of [\"sum\",\"list\"]" | 2 |
| `--data '{"zz":1}'` | `validation.unknown_field` with `details.known: ["n","mode"]` | 2 |
| `--data '{"a":"x"}'` on an integer | `validation.type` | 2 |
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
{"request_id":"req_01fe09c445","trace_id":"tr_01fe09c445","operation":"lang.echo_input","type":"data","seq":1,"data":{"a":1},"error":null}
{"request_id":"req_01fe09c445","trace_id":"tr_01fe09c445","operation":"lang.echo_input","type":"data","seq":2,"data":"two","error":null}
{"request_id":"req_01fe09c445","trace_id":"tr_01fe09c445","operation":"lang.echo_input","type":"result","seq":3,"status":"ok","data":{"received":2},"error":null,"effects":"none","data_count":2}
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
| paths | `response.body.items`, `obj.a`, list items by position `xs.0`, `m.rows.1.0`, `"${xs.0}"` | missing key or index out of range → `value.missing_key` (exit 2) |
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

List items are read by position with a numeric path segment (0-based), in expressions and in interpolation
(fixed in 0.2.0 by INC-2026-0009; in 0.1.0 `xs.0` did not parse):

```rivet
operation lang.index
    name "List index"
    description "Read list items by position."
    output json description "Items."
    xs = [10, 20, 30]
    m = {rows: [[1, 2], [3, 4]]}
    return {first: xs.0, last: xs.2, cell: m.rows.1.0, text: "first=${xs.0}"}
end
```

```text
$ rivet request --file app.rivet lang.index
{"request_id":"req_01fbcc286d","trace_id":"tr_01fbcc286d","operation":"lang.index","type":"result","status":"ok","data":{"first":10,"last":30,"cell":3,"text":"first=10"},"error":null,"effects":"none","data_count":0}
# return xs.5 on a one-item list → [exit 2]
{…"status":"error",…"error":{"kind":"validation","code":"value.missing_key","message":"index 5 is out of range for `xs`",…,"source":{"file":"oob.rivet","line":4,"column":12,"end_line":4,"end_column":16},…}}
```

A keyword statement whose value does not parse reports `syntax.expression` "the expression after `return` does
not parse", with the hint "check its brackets, commas, quotes and object keys; list items are read with `xs.0`".

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
--data '{"op":"a.c"}' → "c"                                                           [0]
--data '{"op":"a.d"}' → permission.denied "dynamic request to a.d is not in the `allow` list"  [3]
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
--data '{"n":-3}' → {"request_id":"req_01fd1ae055",…,"status":"ok","data":"negative",…}      [0]
--data '{"n":0}'  → {"request_id":"req_01fc245e15",…,"status":"ok","data":"zero",…}          [0]

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
timestamps. Captured on 2026-09-29 from the 0.2.0 release candidate (a scratch bundle: private `w.one` returns 1,
private `w.boom` fails `w.boom`; `w.indep` and `w.fast` run both as DAG nodes and `return {a: a, b: b}` under an
`output object` with `open true`). IDs and timestamps differ on every run:

```text
$ rivet request --file app.rivet w.indep                  # dag fail independent — exit 0
{"request_id":"req_014141b875","trace_id":"tr_014141b875","operation":"w.indep","type":"result","status":"ok",
 "data":{"a":{"status":"succeeded","result":1,"error":null,"started_at":"2026-09-28T21:56:37.580Z","ended_at":"2026-09-28T21:56:37.581Z"},
         "b":{"status":"failed","result":null,"error":{"kind":"application","code":"w.boom","message":"always fails","retryable":false,"effects":"none",…,"operation_id":"w.boom","node_id":"b","details":{}},
              "started_at":"2026-09-28T21:56:37.580Z","ended_at":"2026-09-28T21:56:37.581Z"}},
 "error":null,"effects":"none","data_count":0}

$ rivet request --file app.rivet w.fast                   # dag fail fast — exit 5 (stderr)
{"request_id":"req_017bfb3ef5","trace_id":"tr_017bfb3ef5","operation":"w.fast","type":"result","status":"error","data":null,
 "error":{"kind":"application","code":"w.boom","message":"always fails","retryable":false,"source":{"file":"app.rivet","line":15,"column":5,"end_line":15,"end_column":21},"operation_id":"w.boom","node_id":"b",
          "details":{"nodes":[{"id":"a","status":"succeeded","started_at":"2026-09-28T21:56:32.323Z","ended_at":"2026-09-28T21:56:32.323Z"},
                              {"id":"b","status":"failed","started_at":"2026-09-28T21:56:32.323Z","ended_at":"2026-09-28T21:56:32.323Z"}]}},
 "effects":"none","data_count":0}
```

A node value (`NODE`) is a **language value**, not a wire envelope: it keeps its 0.1.0 shape `{status, result, error,
started_at, ended_at}`, and its `error` object still carries `effects` (PLAN-2026-0002 decision for TASK-015). The
request's own answer is the 0.2.0 envelope around it.

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

### Globals

Why: name a base URL, a page size or a list of retryable status codes once and reuse it in every operation of the
file. A global is evaluated **once at load**, is read-only, and is resolved statically, so `rivet io` and
`policy generate` see the real target. New in 0.2.0.

```rivet
global api        = "https://api.example.com"
global users_url  = "${api}/users"
global page_size  = 50
global retry_on   = [429, 503]
global headers    = {accept: "application/json"}

operation users.all
    output json
    r = http get users_url
        header "accept" headers.accept
        decode json
    end
    return r.body
end

operation info.show
    param page integer default 1
    output json
    return {url: "${users_url}?limit=${page_size}&page=${page}", first_retry: retry_on.0, accept: headers.accept}
end
```

```text
$ rivet --file app.rivet check
ok: 3 operations, 0 connectors, 0 auth profiles
$ rivet --file app.rivet request info.show --data '{"page":2}' --pretty
{
  "request_id": "req_01b96a1515",
  "trace_id": "tr_01b96a1515",
  "operation": "info.show",
  "type": "result",
  "status": "ok",
  "data": {
    "url": "https://api.example.com/users?limit=50&page=2",
    "first_retry": 429,
    "accept": "application/json"
  },
  "error": null,
  "effects": "none",
  "data_count": 0
}
$ rivet --file app.rivet io --by target
TARGET                       ACCESS       CAPABILITY     ORIGIN    PHASE    NEEDS FILE  USED BY
https://api.example.com:443  connect GET  allow_network  http get  connect  —           users.all, users.get
```

```text
 load ─▶ parse ─▶ globals in declaration order ─▶ evaluate (pure) ─▶ one frozen scope per file
                      │                              └─ env / secret / request / effect / param / local ─▶ check.global_not_constant
                      └─ name used before its line ─▶ check.global_forward_ref
 request ─▶ lookup order: locals ─▶ params ─▶ globals   (a global can never be shadowed or assigned)
```

Rules:

| Rule | Detail |
|---|---|
| Where | top level only, before or between declarations; `global` inside an operation is `syntax.global` |
| Allowed expressions | literals, lists, objects, operators, `${…}` interpolation of earlier globals, the pure built-ins `length`, `base64.*`, `text`, `keys`, `xml.element` |
| Not allowed | `env`, `secret`, `request`, effects, params, locals → `check.global_not_constant` (a global can never hold a secret) |
| Order | a global sees only earlier globals |
| Scope | the file that declares it; every module has its own globals |
| Manifest | a target made only of literals and globals is `exact`; a target that also uses a param stays `param_dependent`, with the global parts resolved (`https://api.example.com/users/{id}`) |
| Evaluation errors | keep their own code at load, e.g. `global z = 1 / 0` → `value.division_by_zero` (exit 2) |

Errors, captured with `rivet --file FILE check` (all exit 2):

```text
error[check.global_not_constant]: global `token` cannot read the environment; globals are fixed at load time
  --> bad.rivet:1:16
   |
  1| global token = (env "API_TOKEN")
   |                ^^^^^^^^^^^^^^^^^
  = hint: declare `secret token from env "…" for "https://…"` inside the operation that uses it

error[check.global_forward_ref]: global `a` reads `b`, which is not declared before it; globals see only earlier globals
  --> fwd.rivet:1:12

error[check.global_shadow]: parameter `api` reuses the name of a global; globals cannot be shadowed
  --> sh.rivet:4:11
  = hint: rename the parameter

error[check.global_assign]: `n` is a global and globals are read-only
  --> as.rivet:5:5
  = hint: assign to a new local name instead

error[check.global_duplicate]: global `a` is declared twice in dup.rivet
  --> dup.rivet:2:8

error[syntax.global]: expected `global NAME = EXPR`
  --> syn.rivet:1:1
```

`check.global_shadow` also covers loop variables, `map` items, `with … as` names, DAG nodes, tasks and secrets.
Demo: `docs/demos/14-globals/` (DEMO-2026-0016).

### Modules (import)

Why: split a large catalog across files, reuse a file of operations in several bundles, and keep helpers
private to the file that owns them. New in 0.2.0.

```rivet
# users.rivet — a module: its IDs are short; the importer namespaces them
global greeting = "Hello"

operation get
    description "One user by id."
    param id integer required min 1 description "The user id."
    output json description "The user."
    return {id: id, name: "${greeting}, user ${id}"}
end

operation list
    description "The first two users (calls inside a module use its own IDs)."
    output json description "Two users."
    return [(get {id: 1}), (request "get" {id: 2})]
end
```

```rivet
# lib/billing.rivet — imports resolve relative to the importing file
import "../users.rivet" as people

global rate = 0.2

operation invoice
    description "An invoice for one user."
    param user integer required description "The user id."
    output json description "The invoice."
    who = (people.get {id: user})
    return {user: who, amount: 100 * rate}
end
```

```rivet
# app.rivet — the entry bundle
import "./users.rivet" as users                    # internal: callable here, not listed on surfaces
import "./lib/billing.rivet" as billing public     # public: billing.* is on every surface

operation report.user
    description "A user and their invoice."
    param id integer required
    output json
    u = (users.get {id: id})
    inv = (billing.invoice {user: id})
    return {user: u, invoice: inv}
end
```

```text
 app.rivet ──import──▶ users.rivet            (alias users,   internal)
     └──────import──▶ lib/billing.rivet      (alias billing, public) ──import──▶ ../users.rivet (alias people)
 resolve (relative to the importing file, inside the runtime root) ─▶ compile each file once
 ─▶ one catalog: report.user · billing.invoice · users.get* · users.list*    (* internal)
 ─▶ one policy: the entry bundle's (a policy.json beside a module is ignored, with a warning)
```

```text
$ rivet --file app.rivet list
ID               NAME             DESCRIPTION
report.user      report.user      A user and their invoice.
billing.invoice  billing.invoice  An invoice for one user.

$ rivet --file app.rivet request billing.invoice --data '{"user":3}'
{"request_id":"req_0103e29add","trace_id":"tr_0103e29add","operation":"billing.invoice","type":"result","status":"ok","data":{"user":{"id":3,"name":"Hello, user 3"},"amount":20.0},"error":null,"effects":"none","data_count":0}

$ rivet --file app.rivet request users.get --data '{"id":3}'              # internal import → [exit 4]
{…"operation":"users.get",…"error":{"kind":"not_found","code":"not_found.operation","message":"no operation `users.get`",…}}

$ rivet --file app.rivet io --include-bootstrap
OPERATION        KIND  ACCESS  TARGET  KNOWLEDGE   SOURCE
billing.invoice  (calls users.get — no I/O)        lib/billing.rivet:10
report.user      (calls users.get — no I/O)        app.rivet:8
report.user      (calls billing.invoice — no I/O)  app.rivet:9

BOOTSTRAP (runtime-internal; listed, not governed by policy.json)
KIND  ACCESS       TARGET
file  read         ./app.rivet
file  read         ./users.rivet
file  read         ./lib/billing.rivet
file  read         ./policy.json (when present)
…
```

Rules:

| Rule | Detail |
|---|---|
| Form | `import "PATH" as ALIAS [public]`, at the top of the file, before the first declaration |
| PATH | a string literal relative to the importing file; must stay inside the runtime root (no `..` escape, no symlink escape) |
| Addressing | a module's operations are `ALIAS.ID`; call them as `(ALIAS.ID {…})` or `(request "ALIAS.ID" {…})` |
| Inside a module | the module's own short IDs (`(get {id: 1})`); built-in names win |
| Visibility | an import is internal to the importer unless marked `public` (then listed on the CLI, HTTP, MCP, WebSocket, polling, library and FFI) |
| Nesting | namespaces are transitive (`billing.people.get`); a file reached under two aliases compiles once (under the shallowest namespace) |
| Private state | a module's globals, connectors and auth profiles stay inside it; connector and auth profile **names** are still unique across the bundle (`check.import_collision`) because the single policy grants them by name |
| Policy | the loader's policy only; `rivet io`, `policy generate`, `policy explain` and `rivet graph` cover every file |
| Limits | at most 256 files and depth 16 (`limit.imports`) |

Import errors are reported by `rivet check` (and at load) at the `import` line:

```text
error[check.import_cycle]: import cycle: cyc.rivet → a.rivet → cyc.rivet                               [exit 2]
error[not_found.import]: cannot read missing.rivet: No such file or directory (os error 2)             [exit 4]
error[permission.import_outside_root]: import `../../outside.rivet` resolves outside the runtime root . [exit 3]
error[check.import_duplicate]: alias `x` is imported twice in dupa.rivet                               [exit 2]
error[syntax.import]: `import` must come before the first declaration of the file                     [exit 2]
error[check.import_collision]: `u.get` from m1.rivet collides with the operation declared at col.rivet:3  [exit 2]
warning[check.module_policy_ignored]: the policy.json beside sub/m.rivet is ignored: module `m` runs under the loader's policy
  = hint: grant what the module needs in the entry bundle's policy.json (or the host policy)
```

Hosts can also load a file at run time as a module object: `rt.load("./users.rivet")` in Rust
([MAN-2026-0007](man-2026-0007-embedding-library.md)), `rivet_load` in C and `rt.load(...)` in the Python example
wrapper ([MAN-2026-0009](man-2026-0009-c-abi-and-ffi.md#module-objects)). Demo: `docs/demos/17-modules/`
(DEMO-2026-0019).

## Complete Language Reference

Statement shapes (the 0.2.0 grammar, `src/infra/rivet.capy`):

| Keyword | Shape |
|---|---|
| top level | `import "PATH" as ALIAS [public]` (first), `global NAME = EXPR` |
| declarations | `operation ID` / `pipeline ID` / `connector NAME mcp\|grpc` / `auth NAME oauth2` … `end` |
| header | `name`, `description`, `private`, `param`, `output`, `emits`, `receives`, `error` |
| assignment | `NAME = EXPR`, `NAME += EXPR`, `NAME = map …`, `NAME = poll …` |
| statements | `return`, `yield`, `emit`, `fail`, `break`, bare `(call …)`, `NAME.method …` |
| blocks | `if` (+ `else`), `while`, `for`, `try` + `catch`, `dag` + `node`, `concurrent` + `task`, `scope`, `iterate`, `with` (incl. `with file open`) |
| effects | `http METHOD URL`, `grpc C.M`, `command BIN`, `file VERB …`, `secret NAME from env "V" for "ORIGIN"` |
| functions | `request`, `request.stream`, `length`, `base64.encode`, `base64.decode`, `text`, `keys`, `xml.element` — any other name is `check.unknown_function` |
| options | `timeout`, `decode`, `body`, `header`, `query`, `accept`, `retry`, `redirect`, `version`, `tls`, `unix`, `stream`, `framing`, `max_frame`, `max_datagram`, `alpn`, `max_streams`, `migration`, `datagrams`, `message`, `metadata`, `args`, `stdin`, `env`, `cwd`, `bind`, `interface`, `missing`, `overwrite`, `if_version`, `chunk_size`, `limit`, `allow`, OAuth keys |

Not in 0.2.0: `else if`, `finally`, URL imports, `${expression}` interpolation, function-call syntax, unquoted
durations; Stage C forms (`with file watch`, `with pipe`, TCP/Unix `tls`, `reconnect`, `interactive true`).

## Errors and Recovery Reference

| Error / Code | Surface | Cause | User-Visible Result | Recovery | Retry Safe | Related Feature |
|---|---|---|---|---|---|---|
| `syntax.e0001`, `syntax.unknown_statement` | compile | missing `end`, unknown keyword (`finally`) | caret at the line, exit 2 | fix the block | no | layout |
| `syntax.else_if`, `syntax.else_without_if` | compile | `else if COND`, orphan or second `else` | caret + hint, exit 2 | nest `if … end` inside `else` | no | control flow |
| `check.unknown_function` | compile | `(lenght x)` | hint "did you mean `length`?", exit 2 | use a built-in function | no | expressions |
| `syntax.fcall_style` | compile | `length(x)` | hint "write (length arg …)", exit 2 | use prefix calls | no | expressions |
| `syntax.interpolation` | compile | `"${1 + 1}"` | exit 2 | assign first, interpolate the name | no | expressions |
| `syntax.header_order`, `syntax.option_after_body`, `syntax.option_misplaced` | compile | header/option in the wrong place | exit 2 | move the line | no | layout |
| `check.unknown_operation`, `check.call_cycle`, `registry.duplicate_id` | compile | bad composition | exit 2 | fix IDs | no | calls |
| `syntax.global`, `check.global_*` (5) | compile | malformed global, non-constant, forward reference, duplicate, shadow, assignment | caret + hint, exit 2 | see [Globals](#globals) | no | globals |
| `syntax.import`, `check.import_cycle`, `check.import_duplicate`, `check.import_collision` | compile | malformed/late import, cycle, reused alias, colliding ID | caret, exit 2 | see [Modules](#modules-import) | no | modules |
| `not_found.import` / `permission.import_outside_root` / `limit.imports` | compile | missing file / escapes the root / > 256 files or depth > 16 | exit 4 / 3 / 5 | fix the path; flatten | no | modules |
| `check.module_policy_ignored` (warning) | compile | a `policy.json` beside a module | warning, exit 0 | grant in the entry policy | — | modules |
| `unsupported.feature` | load | the bundle uses gRPC, QUIC/HTTP/3 or OAuth in a build without that Cargo feature | exit 5 / 501 | rebuild with the feature ([MAN-2026-0008](man-2026-0008-protocols-and-connectors.md)) | no | features |
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

- Imports are local files under the runtime root only (no URL imports, no hot reload of imported files).
- No `finally` (and no `else if`; nest an `if` inside `else`).
- Stage C forms are refused at run time (`with file watch`, `with pipe`, TCP/Unix `tls`, `reconnect`,
  `interactive true`).

Secret taint follows explicit flows only; implicit flows (for example `if SECRET == "x"`) are not tracked by design.

## Version Applicability

| Feature / Interface | Introduced | Changed | Deprecated / Removed | Applicable Environment |
|---|---|---|---|---|
| Grammar and lowering described here | 0.1.0 | 0.2.0 | — | all |
| `global NAME = EXPR` | 0.2.0 | — | — | all |
| `import "PATH" as ALIAS [public]` | 0.2.0 | — | — | all |
| Numeric list index paths (`xs.0`) | 0.2.0 (INC-2026-0009) | — | — | all |

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
| 3 | 2026-09-29 | Claude | 0.2.0 (D-22, D-46): Globals and Modules (import) chapters with real check/request/io output; numeric list index paths (INC-2026-0009); outputs re-captured as envelopes; --data replaces --params; reference, errors, limitations and version rows updated |
| 4 | 2026-09-29 | Claude | Envelope sweep (TASK-070): the DAG `fail independent` / `fail fast` example re-captured on the 0.2.0-rc as envelopes (was a `829ca43` capture in the 0.1.0 shape); node values are language values and keep their shape. |
