---
document_id: MAN-2026-0003
title: "Rivet language guide"
document_type: manual
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
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
last_verified_version: "0.1.0-dev (commit f40d4aa)"
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

All examples were compiled and run with `rivet 0.1.0-dev` (commit `f40d4aa`). IDs in outputs vary per run.

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
- `emits TYPE` / `emits object … end` declares streamed items; `receives TYPE` declares live input.
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

### Values and expressions

| Form | Example | Notes |
|---|---|---|
| literals | `1`, `1.5`, `"x"`, `true`, `null`, `[1, 2]`, `{a: 1}` | object keys are bare identifiers |
| paths | `response.body.items`, `obj.a` | missing key → `value.missing_key` (exit 2) |
| operators | `+ - * / %`, `== != < <= > >=`, `and`, `or`, `not`, parentheses | integer `/` truncates (`7 / 2` → `3`); `+` joins strings and lists; `+=` appends |
| interpolation | `"a=${obj.a}"` | dotted paths only |
| helpers | `(length x)`, `(keys obj)`, `(text 42)`, `(base64.encode "hi")`, `(base64.decode "AAEC")`, `(xml.element name attrs content)` | pure, never perform I/O |

Inside object and list literals an operator expression must be parenthesized (or assigned first):
`return {d: (x / 2)}` works; `return {d: x / 2}` and `return [x + 1]` are `syntax.e0001`. `!` and `&&` are not
operators (`syntax.unknown_statement`, `syntax.trailing`); use `not` and `and`.

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
| `if COND … end` | no `else` in 0.1.0 — use a second `if` with the negated condition or an early `return` |
| `for NAME in ITER … end` | lists, `incoming`, streams and resource handles |
| `while COND … end` | any condition; bound it with a deadline (request `--timeout`, `scope timeout`) |
| `break` | exits the innermost loop |
| `iterate max N … end` | bounded sequential loop |
| `return EXPR` | exits the whole operation (also from inside `map`/`poll`) |
| `emit EXPR` | one streamed item (needs `emits`) |
| `fail "CODE" {…}` | raise an application error |

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
         ├──► total          node value = {status, result, error}
 second ─┘                   status: succeeded | failed | blocked | …
```

| Mode | Behaviour | Verified output |
|---|---|---|
| `fail fast` (default) | first node failure fails the request with that node's error (`node_id` set) | `{"kind":"application","code":"demo.bad",…,"node_id":"x"}` exit 5 |
| `fail independent` | independent nodes finish; dependents of a failed node are `blocked` | `{"good":"succeeded","bad":"failed","blocked":"blocked"}` |

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

| Resource | Example header | Guide |
|---|---|---|
| HTTP stream | `with http get "URL" as events` + `stream sse|jsonl|lines|bytes` | MAN-2026-0008 |
| WebSocket | `with websocket "wss://…" as socket` | MAN-2026-0008 |
| TCP / Unix | `with tcp "HOST:PORT" as conn` / `with unix "/path.sock" as conn` | MAN-2026-0008 |
| UDP | `with udp "HOST:PORT" as s`, `with udp bind …`, `with udp multicast …` | MAN-2026-0008 |
| QUIC | `with quic "quic://HOST:PORT" as c` + `with c.open bidi as stream` | MAN-2026-0008 |
| gRPC | `with grpc CONNECTOR.Method as rpc` | MAN-2026-0008 |
| process | `with command "/abs/bin" as p` | MAN-2026-0008 |
| child stream | `with (request.stream "id" {…}) as events` | this volume |

`with file open …` and `with file watch …` parse but fail at run time with `unsupported.adapter` ("`file`
resources are not available in this build") in 0.1.0.

### Files

Why: each verb states intent, so `rivet io` and `policy.json` can tell a create from an overwrite.

| Statement | Access verbs (capability) | Behaviour |
|---|---|---|
| `x = file read "P" as json|text|bytes` | read (`allow_read`) | missing → `not_found.file` |
| `file list "DIR"` | list (`allow_read`) | `[{name, type, size}]` sorted by name |
| `file stat "P"` | stat (`allow_read`) | `{path, type, size, version}` |
| `file create "P" json|text|bytes V` | create (`allow_write`) | exists → `conflict.already_exists` (exit 4), file unchanged |
| `file update "P" json V` | stat + update | missing → `not_found.file`; `if_version V` option guards |
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
- An unset variable: `not_found.env` (exit 4).
- The value is bound to the `for` origin: sending it to another origin is refused before connecting.
- **Limitation (0.1.0):** taint is not enforced on `return` or `emit`. An operation that returns or emits a
  secret (directly or interpolated) will expose it. Never do that; review every operation that declares `secret`.

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
| declarations | `operation ID` / `pipeline ID` / `connector NAME mcp|grpc` / `auth NAME oauth2` … `end` |
| header | `name`, `description`, `private`, `param`, `output`, `emits`, `receives`, `error` |
| assignment | `NAME = EXPR`, `NAME += EXPR`, `NAME = map …`, `NAME = poll …` |
| statements | `return`, `yield`, `emit`, `fail`, `break`, bare `(call …)`, `NAME.method …` |
| blocks | `if`, `while`, `for`, `try` + `catch`, `dag` + `node`, `concurrent` + `task`, `scope`, `iterate`, `with` |
| effects | `http METHOD URL`, `grpc C.M`, `command BIN`, `file VERB …`, `secret NAME from env "V" for "ORIGIN"` |
| options | `timeout`, `decode`, `body`, `header`, `query`, `accept`, `retry`, `redirect`, `version`, `tls`, `unix`, `stream`, `framing`, `max_frame`, `max_datagram`, `alpn`, `max_streams`, `migration`, `datagrams`, `message`, `metadata`, `args`, `stdin`, `env`, `cwd`, `bind`, `interface`, `missing`, `overwrite`, `if_version`, `chunk_size`, `limit`, `allow`, OAuth keys |

Not in 0.1.0: `else`, `finally`, `${expression}` interpolation, function-call syntax, unquoted durations.

## Errors and Recovery Reference

| Error / Code | Surface | Cause | User-Visible Result | Recovery | Retry Safe | Related Feature |
|---|---|---|---|---|---|---|
| `syntax.e0001`, `syntax.unknown_statement` | compile | missing `end`, unknown keyword (`else`, `finally`) | caret at the line, exit 2 | fix the block | no | layout |
| `syntax.fcall_style` | compile | `length(x)` | hint "write (length arg …)", exit 2 | use prefix calls | no | expressions |
| `syntax.interpolation` | compile | `"${1 + 1}"` | exit 2 | assign first, interpolate the name | no | expressions |
| `syntax.header_order`, `syntax.option_after_body`, `syntax.option_misplaced` | compile | header/option in the wrong place | exit 2 | move the line | no | layout |
| `check.unknown_operation`, `check.call_cycle`, `registry.duplicate_id` | compile | bad composition | exit 2 | fix IDs | no | calls |
| `validation.*` | request | bad params | 422 / exit 2 | fix params | no | params |
| `value.missing_key` | request | reading an absent key or out-of-scope name | exit 2 | check the path/scope | no | expressions |
| `stream.emits_undeclared`, `stream.input_required` | request | streaming misuse | exit 2 | declare `emits`; send input | no | streams |
| `output.invalid` | request | result shape wrong | 500 / exit 5 | fix the return or the declaration | no | outputs |
| `timeout.request`, `timeout.poll`, `timeout.scope` | request | deadline hit | 504 / exit 6 | raise the deadline | caller decides | deadlines |
| `unsupported.adapter` | request | `with file open/watch` | exit 5 | use one-shot file verbs | no | files |

## Limitations

- No `else`, no `finally`; no `with file open` / `with file watch` at run time.
- Secret taint is not enforced on `return`/`emit`.
- Implicit flows (for example `if SECRET == "x"`) are never tracked.

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
