---
document_id: MAN-2026-0004
title: "Rivet CLI reference"
document_type: manual
status: active
created_date: 2026-09-28
last_updated: 2026-09-30
document_revision: 6
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [cli, registry, execution, policy, audit, connectors, auth, serve, sessions]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, server]
audience: [developers, operators, integrators]
scope: Every rivet command, subcommand, flag, default and exit code of the 0.2.0 release candidate (with what changed since 0.1.0) — including `--data`, `--input`, `--pretty`, the `--params` deprecation, `rivet highlight` and the envelope printed by every JSON output — with a verified success and failure example for each.
reason: PLAN-2026-0001 row D-37 and PLAN-2026-0002 rows D-23, D-46 (DOCUMENTATION.md §31 CLI impact) — the CLI reference is derived from `rivet --help` of the build and each command was executed.
related_documents: [PLAN-2026-0002, API-2026-0006, MIG-2026-0001, MAN-2026-0010, API-2026-0005, MAN-2026-0001, MAN-2026-0002, MAN-2026-0005, MAN-2026-0006, MAN-2026-0008, API-2026-0001, DEMO-2026-0001, DEMO-2026-0011]
supersedes: null
superseded_by: null
tags: [rivet, manual, cli, reference, exit-codes]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.2.0-rc (source at 6f9943f)"
next_review_date: 2026-10-29
---

# Rivet CLI reference

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-30
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** cli, registry, execution, policy, audit, connectors, auth, serve, sessions

## Purpose

The complete command-line contract of `rivet`. The current release is **0.2.0** and this reference describes it: every row was taken from `rivet … --help` of
`cargo build --release --features cli` (source `6f9943f`) and every example was executed on 2026-09-29 (macOS 26.4)
from `docs/demos/NN-*/` or a scratch bundle as stated. **Request and trace IDs differ on every run**; `--version`
prints `rivet 0.2.1`. Part of the [Rivet manual](man-2026-0001-rivet-manual.md).

What changed for CLI users in 0.2.0 ([MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md#cli)):

```text
 0.1.0                                         0.2.0
 request … --params '{…}'                      request … --data '{…}'   |  --input FILE|-   (--params: deprecated, warns)
 stdout {"request_id","trace_id","result":…}   stdout: envelope, status "ok"   ·  stderr: envelope, status "error"|"cancelled"
 stdout {…,"error":{…,"effects"}}              exit codes unchanged (registry)
 --json: bare payload                          --json: envelope; payload in .data   (rivet list --json | jq .data.operations)
 —                                             --pretty (any JSON output; refused with --stream)
 —                                             rivet highlight FILE --format ansi|html|json
 cargo build                                   the binary needs --features cli
```

## Reading Order

Global options and exit codes first, then commands in the order `rivet --help` prints them.

## Concepts

```text
 rivet [GLOBAL OPTIONS] <COMMAND> [ARGS] [OPTIONS]

 local mode   rivet --file app.rivet [--policy p.json] <command>      compile the bundle in this process
 remote mode  rivet --endpoint URL [--token-file t] <command>         thin client of a running `rivet serve`

 global options may appear before or after the command:
   rivet --file app.rivet list        ==        rivet list --file app.rivet
```

### Global options

| Global option | Type / default | Purpose | Constraints |
|---|---|---|---|
| `--file FILE` | path; none | entry `.rivet` file; `policy.json` beside it is discovered | required for every local command (`validation.usage`, exit 2) |
| `--policy POLICY` | path; discovered file | use this policy file instead | a path, never grant text; missing file → `policy.invalid` (exit 2) |
| `--json` | flag | print JSON instead of tables; **every JSON output is a ResponseEnvelope** (0.2.0) whose `operation` is the built-in (`rivet.list`, `rivet.describe`, `rivet.outputs`, `rivet.check`, `rivet.io`, `rivet.graph`, `rivet.policy.explain`, `rivet.policy.generate`, `rivet.trace.show`, …) and whose `data` is the payload | request results are always JSON |
| `--pretty` | flag (0.2.0) | indent any JSON output (2 spaces, same key order) | refused with `--stream` (`validation.usage`, exit 2) |
| `--endpoint URL` | URL | send `request`, `list`, `describe`, `outputs`, `io`, `trace`, `auth` to a running server | not with `--file`/`--policy`; `check`, `graph`, `policy`, `serve` refused (exit 2) |
| `--token-file FILE` | path | bearer token for `--endpoint` | never pass tokens in argv or env |
| `-h`, `--help` / `-V`, `--version` | flag | help / `rivet 0.2.1` | — |

Output streams (0.2.0): a **success** envelope (`status: "ok"`) goes to **stdout**; an **error or cancelled**
envelope goes to **stderr** with the registry exit code, so `2>/dev/null` leaves stdout empty on failure. Compile
errors (`error[CODE]: …`), warnings (`warning[CODE]: …`, including `warning[deprecated.params]` and
`warning[deprecated.input]`), tables' diagnostics (`io: complete=false …`, `policy generate: N grants …`) and the serve
receipt also go to **stderr**.

```text
 rivet request … ──┬─ status "ok"                  ─▶ stdout   exit 0
                   ├─ status "error"               ─▶ stderr   exit 2–6 (error.kind)
                   ├─ status "cancelled"           ─▶ stderr   exit 130
                   └─ --stream: data records + the terminal record on stdout (an error terminal record included)
```

### Exit codes

| Code | Meaning | Typical codes |
|---|---|---|
| 0 | success | — |
| 2 | syntax, validation, usage or configuration error | `syntax.*`, `check.*` (incl. `check.global_*`, `check.import_*`), `validation.*` (incl. `validation.input_envelope`), `policy.invalid`, `stream.*`, `serve.auth_required` |
| 3 | permission or authentication | `permission.denied`, `permission.import_outside_root`, `file.hardlink_refused`, `auth.required`, `auth.invalid`; `io --check-policy` found a denied/unknown site; `io --check-files` found a file not permitted; `policy explain ID --data …` found a denied concrete target |
| 4 | not found or conflict | `not_found.*` (incl. `not_found.import`), `conflict.*`; `io --check-files` found a missing file |
| 5 | dependency, runtime, unsupported, output_invalid, limit | `http.status`, `dns.resolve`, `process.exit`, `unsupported.*` (incl. `unsupported.feature`, `unsupported.sandbox_backend` on Linux), `output.invalid`, `limit.*` (incl. `limit.imports`), application `fail` codes |
| 6 | timeout | `timeout.request`, `timeout.poll`, `timeout.scope` |
| 7 | inspection incomplete | `io --strict` with dynamic sites; `policy generate` with review items |
| 130 | cancelled | Ctrl-C during `request` (`cancelled.request`) |

**Warnings** (`warning[CODE]: …` on stderr — `check` warnings, `check.module_policy_ignored`, `deprecated.params`,
`deprecated.input`) never change the exit code.

```text
  rivet <command>
     │ compile ──── error ──► exit 2 (syntax.*, check.*)      warnings ──► stderr, continue
     │ policy  ──── error ──► exit 2 (policy.invalid)
     │ run / inspect
     ├── ok ───────────────────────────────────────────────────────────► 0
     ├── permission / auth / --check-policy / explain --params denied ─► 3
     ├── not_found / conflict / --check-files missing ────────────────► 4
     ├── dependency / runtime / limit / unsupported ──────────────────► 5
     ├── timeout ─────────────────────────────────────────────────────► 6
     ├── io --strict dynamic / policy generate review items ─────────► 7
     └── Ctrl-C ──────────────────────────────────────────────────────► 130
```

Argument parsing errors (unknown subcommand, missing positional) are printed by the parser and exit 2.

## Task-Oriented Workflows

### rivet request

Invoke one operation. `rivet request [OPTIONS] [ID]`

| Option | Default | Purpose |
|---|---|---|
| `ID` | — | operation ID (or a `rivet.*` built-in); optional with `--input`, whose envelope names it |
| `--data JSON` | `{}` | (0.2.0) the operation's input as a JSON object — the envelope's `data` |
| `--input FILE\|-` | none | (0.2.0) read a whole InputEnvelope `{operation, data, deadline_ms?, restrict?, stream?}` from FILE, or from stdin with `-`; not with `--data`/`--params` |
| `--params JSON` | — | **deprecated** alias of `--data` (removed in 0.3.0); prints `warning[deprecated.params]` on stderr |
| `--pretty` | off | (0.2.0) indent the envelope; refused with `--stream` |
| `--stream` | off | print NDJSON records (`type: "data"` items, then the one `type: "result"` record) |
| `--timeout D` | `30s` | request deadline, `^[0-9]+(ms\|s\|m\|h)$`, at most `10m` (600000 ms, the same host cap as `deadline_ms`) |
| `--input-jsonl -` | none | stdin JSON Lines as live input for a `receives` operation; needs `--stream`; EOF = finish input |

```text
 rivet request ID --data '{…}' ─┐
 rivet request --input req.json ├─▶ InputEnvelope ─▶ validate ─▶ run ─┬─▶ stdout: envelope status "ok"         exit 0
 echo '{…}' | rivet request     │                                     ├─▶ stderr: envelope status "error"      exit 2..6
       --input -               ─┘                                     └─ Ctrl-C ─▶ stderr: status "cancelled"   exit 130
 rivet request ID --params '{…}' ─▶ stderr warning[deprecated.params] ─▶ same as --data
```

Success (01-catalog):

```text
$ rivet request --file app.rivet demo.add --data '{"a":2,"b":3}'
{"request_id":"req_018f8b925d","trace_id":"tr_018f8b925d","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}

$ cat req.json
{"operation":"demo.add","data":{"a":2,"b":3},"deadline_ms":5000}
$ rivet request --file app.rivet --input req.json
{"request_id":"req_01c510b28d","trace_id":"tr_01c510b28d","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}

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

Deprecated 0.1.0 forms (still accepted in 0.2.x, with a warning on stderr):

```text
$ rivet request --file app.rivet demo.add --params '{"a":2,"b":3}'
warning[deprecated.params]: --params is deprecated; use --data (removed in 0.3.0)
{"request_id":"req_01e01ece25","trace_id":"tr_01e01ece25","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}

$ echo '{"id":"demo.add","params":{"a":40,"b":2}}' | rivet request --file app.rivet --input -
warning[deprecated.input]: input keys `id` and `params` are deprecated; use `operation` and `data` (removed in 0.3.0)
{"request_id":"req_01df43a525","trace_id":"tr_01df43a525","operation":"demo.add","type":"result","status":"ok","data":42,"error":null,"effects":"none","data_count":0}
```

Streaming (`--stream`, 01-catalog) and live input (`--input-jsonl -`, scratch bundle with the duplex `chat.echo`):

```text
$ rivet request --file app.rivet demo.countdown --stream
{"request_id":"req_018cb46e25","trace_id":"tr_018cb46e25","operation":"demo.countdown","type":"data","seq":1,"data":3,"error":null}
{"request_id":"req_018cb46e25","trace_id":"tr_018cb46e25","operation":"demo.countdown","type":"data","seq":2,"data":2,"error":null}
{"request_id":"req_018cb46e25","trace_id":"tr_018cb46e25","operation":"demo.countdown","type":"data","seq":3,"data":1,"error":null}
{"request_id":"req_018cb46e25","trace_id":"tr_018cb46e25","operation":"demo.countdown","type":"result","seq":4,"status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}

$ printf '"hi"\n"there"\n' | rivet request --file app.rivet chat.echo --stream --input-jsonl -
{"request_id":"req_01ec0f21bd","trace_id":"tr_01ec0f21bd","operation":"chat.echo","type":"data","seq":1,"data":"hi","error":null}
{"request_id":"req_01ec0f21bd","trace_id":"tr_01ec0f21bd","operation":"chat.echo","type":"data","seq":2,"data":"there","error":null}
{"request_id":"req_01ec0f21bd","trace_id":"tr_01ec0f21bd","operation":"chat.echo","type":"result","seq":3,"status":"ok","data":{"echoed":2},"error":null,"effects":"none","data_count":2}
```

Without `--stream`, a streaming operation prints only the terminal record (`"data_count":3`, no `seq`).

Failures (the error envelope goes to stderr; abridged to its code and message):

| Command | Output | Exit |
|---|---|---|
| `request --file app.rivet demo.add --data notjson` | `validation.params` "--data is not valid JSON: expected ident at line 1 column 2" | 2 |
| `request --file app.rivet demo.add --data '[1]'` | `validation.params` "params must be an object, got list" | 2 |
| `request --file app.rivet demo.add --data '{"a":"x"}'` | `validation.type` "parameter `a` must be an integer, got text" | 2 |
| `request … demo.add --data '{"a":1}' --params '{"a":1}'` | `validation.usage` "use --data (or the deprecated --params), not both" | 2 |
| `request … demo.add --input - --data '{"a":1}'` | `validation.usage` "--input carries the whole envelope; drop --data/--params" | 2 |
| `request … demo.greet --input req.json` (names `demo.add`) | `validation.usage` "the operation is named twice: `demo.greet` on the command line and `demo.add` in --input" | 2 |
| `echo '{"data":{"a":1}}' \| request … --input -` | `validation.required` "the input envelope needs `operation`" (`details.field`) | 2 |
| `echo '{"operation":"demo.add","id":"demo.add"}' \| request … --input -` | `validation.input_envelope` "use `operation` or the deprecated `id`, not both" (`details {key, alias}`) | 2 |
| `echo '[1]' \| request … --input -` | `validation.input_envelope` "the input envelope must be a JSON object" | 2 |
| `request … --input /nope.json` | `validation.usage` "--input /nope.json: entity not found" | 2 |
| `request … demo.countdown --stream --pretty` | `validation.usage` "--pretty cannot be used with --stream: NDJSON records must stay one per line" | 2 |
| `request --file app.rivet demo.nope` | `not_found.operation` | 4 |
| `request --file missing.rivet demo.add` | `error[not_found.source]: cannot read missing.rivet: No such file or directory (os error 2)` | 4 |
| `request --file app.rivet data.private` (11-sandbox) | `permission.denied` "allow_read read on ./data/private/secret.json denied: deny allow_read ./data/private/**" | 3 |
| `printf '5\n' \| request … chat.echo --stream --input-jsonl -` | terminal record `validation.input` "stdin line 1: input item at $ must be text, got integer; the request was cancelled" | 2 |
| `(sleep 1) \| request … chat.echo --stream --input-jsonl - --timeout 200ms` | terminal record `timeout.request` "`chat.echo` exceeded its 200 ms deadline" | 6 |
| `request … --timeout 1x` | `validation.usage` "--timeout 1x: use digits plus ms, s, m or h" | 2 |
| `request … --timeout 20m` | `validation.usage` "--timeout 20m is 1200000 ms; the host cap is 600000 ms (10m)", `details {timeout_ms, max_ms}` | 2 |
| `request … chat.echo --input-jsonl -` (no `--stream`) | `validation.usage` "--input-jsonl - needs --stream (output is NDJSON envelopes)" | 2 |
| Ctrl-C while running | status `cancelled`, `cancelled.request` "the request was cancelled by its caller", `effects: "unknown"` | 130 |

One full failure, as printed on stderr:

```text
$ rivet request --file app.rivet demo.add --data '{"a":"x"}'                                   [exit 2]
{"request_id":"req_018dfc02c5","trace_id":"tr_018dfc02c5","operation":"demo.add","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.type","message":"parameter `a` must be an integer, got text","retryable":false,"operation_id":"demo.add","details":{"field":"a"}},"effects":"none","data_count":0}
```

### rivet list

List public operations. `rivet list [--outputs] [--json]`

```text
$ rivet list --file app.rivet --outputs
ID              NAME                OUTPUT      DESCRIPTION
demo.greet      Greet a person      text        Return a greeting for the supplied person.
demo.add        Add two integers    integer     Add two signed integers and return their sum.
demo.health     Check availability  object      Return a constant readiness response without I/O.
demo.countdown  Count down          object      Emit 3, 2, 1 as data items and then return a summary.
```

`--json` prints the `rivet.list` envelope; the 0.1.0 payload is its `data`
(`rivet list --json | jq .data.operations`):

```text
$ rivet list --file app.rivet --json
{"request_id":"req_015ba23eed","trace_id":"tr_015ba23eed","operation":"rivet.list","type":"result","status":"ok","data":{"operations":[{"id":"demo.greet","name":"Greet a person","description":"Return a greeting for the supplied person.","streaming":false},…,{"id":"demo.countdown","name":"Count down","description":"Emit 3, 2, 1 as data items and then return a summary.","streaming":true}],"next_cursor":null},"error":null,"effects":"none","data_count":0}
```

Private operations are never listed; imported MCP tools are (for example `peer.tools.demo.add`), and so are the
operations of `public` modules under their namespaced IDs (`users.get`; 0.2.0). Failure: a compile error in the
bundle (exit 2).

### rivet describe

`rivet describe [IDS]...` — params, output, emits, receives, errors, source line and delivery (`unary` or
`session`). No IDs = every public operation.

```text
$ rivet describe --file app.rivet demo.add
demo.add — Add two integers
Add two signed integers and return their sum.
source   app.rivet:9
delivery unary

params
  a   integer  required    First operand.
  b   integer  default 0   Second operand; defaults to zero.

output  integer  Sum of a and b.
emits    —
receives —
errors   —
```

`--json` prints the `rivet.describe` envelope whose `data` is the descriptor with JSON Schemas (`input`,
`output`, `emits`, `receives`). Failure: `describe demo.nope` → `error[not_found.operation]: no operation
\`demo.nope\`` (exit 4); with `--json` the same failure is the error envelope on stderr:

```text
$ rivet describe --file app.rivet demo.nope --json                                              [exit 4]
{"request_id":"","trace_id":"","operation":"rivet.describe","type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"not_found.operation","message":"no operation `demo.nope`","retryable":false},"effects":"none","data_count":0}
```

### rivet outputs

`rivet outputs [ID] [--all] [--json]` — declared output, emits, receives and errors.

```text
$ rivet outputs --file app.rivet demo.countdown
demo.countdown — Count down
output  object   Summary returned after the last item.
  count   integer  required  Number of items emitted.
emits   integer  One countdown value per item.
receives —
errors   —
```

`--json`: `{…,"operation":"rivet.outputs",…,"data":{"id":"demo.add","output":{"type":"integer","description":"Sum of a and b."},"emits":null,"receives":null,"errors":[]},…}`.
Failures: no ID and no `--all` → `error[validation.query]: pass exactly one of an operation ID or --all` (exit
2); unknown ID → `not_found.operation` (exit 4).

### rivet check

`rivet check [--strict-docs]` — compile and check without running anything.

```text
$ rivet check --file app.rivet
ok: 4 operations, 0 connectors, 0 auth profiles

$ rivet check --file app.rivet --json                            # 0.2.0: an envelope (operation rivet.check)
{"request_id":"req_01585bffdd","trace_id":"tr_01585bffdd","operation":"rivet.check","type":"result","status":"ok","data":{"operations":4,"connectors":0,"auth_profiles":0,"warnings":0},"error":null,"effects":"none","data_count":0}
```

A failing `check --json` prints the error envelope on stderr (for example `check.global_not_constant`, exit 2;
[API-2026-0005](../api/api-2026-0005-error-registry.md#examples)).

`--strict-docs` also requires descriptions on public operations, params, outputs and fields:

```text
$ rivet check --file app.rivet --strict-docs          # on a bundle without descriptions   [exit 2]
error[docs.description]: operation `a.b` has no description
  --> app.rivet:1:1
error[docs.output_description]: `a.b` output has no description
```

**Warnings.** Without any flag, `check` also reports two warnings on stderr and still exits 0: a `fail "CODE"`
whose operation declares no `error "CODE"` line (`docs.undeclared_error`; with `--strict-docs` it becomes an error),
and a `return` that reads `NODE.result` of a `fail independent` DAG node without an `if NODE.status …` guard
(`check.unguarded_result`). Missing descriptions are reported only with `--strict-docs`. From 0.2.0 a module whose
directory holds its own `policy.json` adds `check.module_policy_ignored`. Captured on a scratch bundle (commit
`829ca43`; the rendering is unchanged in 0.2.0):

```text
$ rivet --file app.rivet check
warning[docs.undeclared_error]: `demo.oops` can fail with `demo.undeclared` but declares no `error "demo.undeclared"` line
  --> app.rivet:31:5
   |
 31|     fail "demo.undeclared" {why: "demo"}
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
warning[check.unguarded_result]: `a.result` is null unless `a` succeeded; guard it with `if a.status == "succeeded"`
  --> app.rivet:41:5
   |
 41|     return a.result
   |     ^^^^^^^^^^^^^^^
ok: 6 operations, 0 connectors, 0 auth profiles                                            [exit 0]
```

**Unknown functions** fail at compile time with the closest built-in as a hint (the functions are `request`,
`request.stream`, `length`, `base64.encode`, `base64.decode`, `text`, `keys`, `xml.element`):

```text
$ rivet --file bad2.rivet check                                                            [exit 2]
error[check.unknown_function]: unknown function `lenght`
  --> bad2.rivet:6:13
   |
  6|     return (lenght items)
   |             ^^^^^^
  = hint: did you mean `length`?
```

Other failures (exit 2): `syntax.*` (for example `syntax.else_if` for `else if COND`, `syntax.else_without_if` for
an orphan `else`), the 0.2.0 globals and modules codes (`syntax.global`, `check.global_*`, `syntax.import`,
`check.import_cycle|duplicate|collision`; exit 4 `not_found.import`, exit 3 `permission.import_outside_root`, exit 5
`limit.imports` — [MAN-2026-0003](man-2026-0003-language-guide.md#modules-import)), `unsupported.feature` (exit 5) in a
build without the needed Cargo feature, `check.unknown_operation`, `check.call_cycle`, `registry.duplicate_id`, `policy.invalid`; exit 4
for a missing connector file (`not_found.mcp_snapshot`, `not_found.descriptor`); exit 2 for an unapproved snapshot
(`mcp.snapshot_unapproved`). Refused with `--endpoint` (exit 2).

### rivet graph

`rivet graph <ID> [--all] [--json]` — the **static call graph** of one operation, computed from the compiled
bundle without running anything: literal `(request "id" …)` calls (expanded into the callee's own graph),
connector calls, effect sites, `dag` nodes with their `after` edges, and both arms of every `if`/`else` (marked).
`--all` allows a private operation. Local only (refused with `--endpoint`, exit 2).

```text
$ rivet --file app.rivet graph report.total                 # 05-dag
report.total                            app.rivet:29
└── dag                                 app.rivet:37
    ├── node first                      app.rivet:38
    │   └── call math.double            app.rivet:38
    ├── node second                     app.rivet:39
    │   └── call math.double            app.rivet:39
    └── node total after first, second  app.rivet:40
        └── call math.sum               app.rivet:40

$ rivet --file app.rivet graph demo.sign                     # scratch bundle: if … else … end
demo.sign          app.rivet:1
├── if n > 0       app.rivet:6
└── else           app.rivet:8
    └── if n == 0  app.rivet:9

$ rivet --file app.rivet graph demo.copy                     # an effect site
demo.copy                         app.rivet:53
└── file     read         {path}  app.rivet:59
```

`--json` prints the `rivet.graph` envelope whose `data` is `{operation_id, root, nodes:[{id, kind, label, source,
condition, conditional, after}], edges:[{from, to, kind: contains|after}]}`; node kinds are `operation`, `call`,
`connector`, `effect`, `branch`, `dag`, `node`, `cycle`:

```text
$ rivet graph --file app.rivet demo.add --json                 # 01-catalog
{"request_id":"req_0155d088d5","trace_id":"tr_0155d088d5","operation":"rivet.graph","type":"result","status":"ok","data":{"operation_id":"demo.add","root":"n0","nodes":[{"id":"n0","kind":"operation","label":"demo.add","source":"app.rivet:9","condition":null,"conditional":false,"after":[]}],"edges":[]},"error":null,"effects":"none","data_count":0}
```

Failures: `graph math.double` (private) → `not_found.operation` (exit 4; add `--all`); unknown ID →
`not_found.operation` (exit 4); `--endpoint` → `validation.usage` "check, graph, policy and serve work on a local
bundle (--file)…" (exit 2).

### rivet io

`rivet io [IDS]... [OPTIONS]` — the I/O manifest: every I/O site, its target, access verbs and capability.
Full workflows: [MAN-2026-0005](man-2026-0005-policy-and-io-manifest-guide.md#review-io-with-rivet-io).

| Option | Default | Purpose |
|---|---|---|
| `IDS…` / `--all` | all public operations | restrict to these operations (and what they call) |
| `--by operation\|target\|capability` | `operation` | table grouping |
| `--kind K` | all | `file`, `network`, `process`, `env`, `mcp`, `grpc`, `auth`, `credential`, … |
| `--access V[,V]` | all | filter by access verb (`create`, `delete`, `connect`, …) |
| `--format table\|json\|markdown\|csv` | `table` | output format (`--json` = `--format json`: the `rivet.io` envelope, manifest in `data`) |
| `--check-policy` | off | add a DECISION column; exit 3 if any site is denied/unknown |
| `--strict` | off | exit 7 if any site is dynamic/opaque |
| `--needs` | off | list the files each operation needs to exist before it runs |
| `--check-files` | off | probe those files: present / missing / not_permitted; exit 4 missing, 3 not permitted (local only) |
| `--include-bootstrap` | off | also list runtime-internal reads (bundle, policy, CA bundle, resolver, descriptors) |
| `--trace REQ` | none | add the recorded ATTEMPTS for one request (useful with `--endpoint`) |
| `--transitive` | on | follow literal `(request "id" …)` calls; accepted for clarity |

```text
$ rivet io --file app.rivet --check-policy                 # 11-sandbox                     [exit 3]
OPERATION      KIND  ACCESS  TARGET                      KNOWLEDGE  SOURCE        DECISION
data.private   file  read    ./data/private/secret.json  exact      app.rivet:26  denied
data.read      file  read    ./data/public.json          exact      app.rivet:15  allowed
data.snapshot  (calls data.read — see above)                        app.rivet:37
data.snapshot  file  create  ./out/snapshot.json         exact      app.rivet:38  allowed
2 allowed · 1 denied
```

Failures (exit 2): `--by nonsense` → "expected operation, target or capability"; `--format yaml` → "expected
table, json, markdown or csv"; `--access frob` → "not an access verb". With `--endpoint`, `--check-files` is
`validation.check_files_remote` (exit 2).

### rivet policy explain

`rivet policy explain [ID] [--data JSON]` — the effective policy; with an ID, each of its sites and the
decision. With `--data` (alias `--params`), the call's **param-dependent targets are filled in** from that data and
evaluated — also in the operations it calls with those values (`report.remote` passing `{id: id}` to
`users.fetch`) — the TARGET column shows the concrete path/URL (`KNOWLEDGE exact`) and the command **exits 3 when
any would be denied** (plus a `denied: …` line). Without `--data` a param-dependent site shows its template
(`{path}`) and the exit code is 0.

```text
  policy explain ID                 policy explain ID --data '{"path":"app.rivet"}'
  TARGET {path}  param_dependent    TARGET app.rivet  exact  DECISION denied   ──► exit 3
```

```text
$ rivet policy --file app.rivet explain notes.update       # 02-file-crud
policy   ./policy.json (sha256:deccf2027323af83c9798a05d6cac1adb657a81852e54ac7b99ce3eca4b0bef3)
base     .
network  deny_private_ranges true
limits   64 concurrent, depth 16, 268435456 buffered bytes
grant    allow_read ./out, ./out/**
grant    allow_write ./out/**
grant    allow_delete ./out/**

OPERATION     KIND  ACCESS  TARGET           KNOWLEDGE  SOURCE        DECISION
notes.update  file  stat    ./out/note.json  exact      app.rivet:30  allowed
notes.update  file  update  ./out/note.json  exact      app.rivet:30  allowed
```

With concrete params (scratch bundle, `demo.read` = `return file read path as text`, policy granting
`allow_read ./data/**`). `--data` is the primary flag; **`policy explain --params` is an alias, not deprecated**
(no warning): it is this command's own flag, unlike `request --params`:

```text
$ rivet --file app.rivet policy explain demo.read --data '{"path":"data/a.txt"}'            [exit 0]
…
OPERATION  KIND  ACCESS  TARGET      KNOWLEDGE  SOURCE        DECISION
demo.read  file  read    data/a.txt  exact      app.rivet:44  allowed

$ rivet --file app.rivet policy explain demo.read --data '{"path":"app.rivet"}'             [exit 3]
…
denied: demo.read#1 allow_read app.rivet (read)

OPERATION  KIND  ACCESS  TARGET     KNOWLEDGE  SOURCE        DECISION
demo.read  file  read    app.rivet  exact      app.rivet:44  denied
```

`--json` prints the `rivet.policy.explain` envelope (01-catalog, no policy):
`{…,"operation":"rivet.policy.explain",…,"data":{"present":false,"file":null,"sha256":null,"grants":0,"deny":0,"broad":[]},…}`.
When `--data` makes a site denied, `--json` agrees with exit 3: a `status: "error"` envelope (kind `permission`,
code `permission.denied`) on stderr, whose `error.details` holds the same explanation plus `denied[]`
(scratch bundle `f.read` = `file read "./data/${name}"`, policy granting `./data/a.txt`):

```text
$ rivet --file app.rivet --json policy explain f.read --data '{"name":"b.txt"}'             [exit 3]
{"request_id":"req_012a299dc5","trace_id":"tr_012a299dc5","operation":"rivet.policy.explain","type":"result","status":"error","data":null,"error":{"kind":"permission","code":"permission.denied","message":"1 effect site(s) of this call would be denied by the policy","retryable":false,"operation_id":"f.read","details":{"present":true,"file":"./policy.json",…,"sites":[…],"denied":[{"effect_id":"f.read#1","capability":"allow_read","target":"./data/b.txt","access":"read"}]}},"effects":"none","data_count":0}
```

A `"*"` target is flagged: `grant    allow_network *   ⚠ broad: "*" allows every target`. Failure: an invalid
policy → `error[policy.invalid]: policy.json /version: \`version\` must be 1` (exit 2). Refused with
`--endpoint`.

### rivet policy generate

`rivet policy generate [IDS]... [--all] [--output PATH]` — least-privilege draft from the manifest; no IDs = all.
Without `--output` the draft goes to stdout; the summary goes to stderr. Without `--json` the draft stays the
**bare policy file** (so `rivet policy generate > policy.json` keeps working in 0.2.0); with `--json` it is the
`rivet.policy.generate` envelope (`data` = `{policy, review, complete}`).

```text
$ rivet policy --file app.rivet generate data.snapshot     # 11-sandbox
{
  "version": 1,
  "grants": [
    {"capability": "allow_read", "targets": ["./data/public.json"], "access": ["read"]},
    {"capability": "allow_write", "targets": ["./out/snapshot.json"], "access": ["create"]}
  ],
  "network": {"deny_private_ranges": true}
}
policy generate: 2 grants, 0 review items
```

```text
$ rivet policy --file app.rivet generate --json                # 01-catalog
{"request_id":"req_015670a435","trace_id":"tr_015670a435","operation":"rivet.policy.generate","type":"result","status":"ok","data":{"policy":{"version":1,"grants":[],"network":{"deny_private_ranges":true}},"review":[],"complete":true},"error":null,"effects":"none","data_count":0}
policy generate: 0 grants, 0 review items
```

`--output policies/generated.json` writes the file (targets rebased to the file's directory, e.g.
`../data/public.json`) and refuses to overwrite: `conflict.exists` "refusing to overwrite
policies/generated.json" (exit 4). Dynamic sites become review items and exit 7 (08-udp:
`review  telemetry.receive#2  network connect  udp://{message.peer} … not granted (dynamic target)`).

### rivet trace show and export

`rivet trace show <REQUEST_ID>` — broker decisions and attempts of one request, each with its `effect_id`.
`rivet trace export <REQUEST_ID> --output PATH` — write that trace (already sanitized) as pretty JSON to a **new**
bundle-relative file; the write is itself an effect (`allow_write` access `create` on PATH) and never overwrites
(`conflict.already_exists`, exit 4). Traces live in the memory of the process that ran the request (no persistent
trace store), so use `show` with `--endpoint`:

```text
  process that ran req_X ── trace store (memory) ──► trace show  (--endpoint → rivet.trace.show)   ✓
                                                └──► trace export (Runtime::export_trace in that host) ✓
  new local `rivet` process ── empty store ──► not_found.trace (exit 4)
```

Captured against `rivet serve` on `127.0.0.1:18950` (the scratch `demo.read` bundle; its policy grants
`allow_write create ./out/**`):

```text
$ rivet --endpoint http://127.0.0.1:18950 request demo.read --data '{"path":"data/a.txt"}'
{"request_id":"req_01e270c735","trace_id":"tr_01e270c735","operation":"demo.read","type":"result","status":"ok","data":"hello world!","error":null,"effects":"none","data_count":0}

$ rivet --endpoint http://127.0.0.1:18950 trace show req_01e270c735
{"request_id":"req_025f87789a","trace_id":"tr_025f87789a","operation":"rivet.trace.show","type":"result","status":"ok","data":{"request_id":"req_01e270c735","attempts":[{"request_id":"req_01e270c735","trace_id":"tr_01e270c735","node_id":null,"attempt":1,"effect_id":"demo.read#1","operation_id":"demo.read","phase":"decision","capability":"allow_read","access":"read","target":"data/a.txt","decision":"allowed","policy_hash":"sha256:d33d79eb…97f1","source":{"file":"app.rivet","line":44,"column":5},"outcome":{"rule":"grant allow_read ./data/**"}}],"complete":true,"next_cursor":null,"gaps":0},"error":null,"effects":"none","data_count":0}

$ rivet --endpoint http://127.0.0.1:18950 trace export req_01e270c735 --output ./out/trace.json          [exit 0]
{"request_id":"req_03dc59ccf7","trace_id":"tr_03dc59ccf7","operation":"rivet.trace.export","type":"result","status":"ok","data":{"request_id":"req_01e270c735","path":"./out/trace.json","events":1,"bytes":718},"error":null,"effects":"committed","data_count":0}

$ rivet --endpoint http://127.0.0.1:18950 trace export req_01e270c735 --output ./out/trace.json          [exit 4]
{"request_id":"req_045f1f78cc","trace_id":"tr_045f1f78cc","operation":"rivet.trace.export","type":"result","status":"error","data":null,"error":{"kind":"conflict","code":"conflict.already_exists","message":"./out/trace.json already exists","retryable":false},"effects":"none","data_count":0}
```

Failure: a local `rivet --file app.rivet trace show req_…` → a new process has an empty store (exit 4):

```text
{"request_id":"","trace_id":"","operation":"rivet.trace.show","type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"not_found.trace","message":"no trace for request `req_01e270c735` in this host's trace store","retryable":false},"effects":"none","data_count":0}
```

Pure requests record no decisions, so they have no trace either. The path of `trace export` is relative to the
server's bundle and needs `allow_write` access `create` in its policy.

A library host calls `Runtime::export_trace(request_id, path)` ([MAN-2026-0007](man-2026-0007-embedding-library.md)).

### rivet auth

OAuth account management through the `rivet.auth.*` built-ins; tokens are never printed. Details and grants:
[MAN-2026-0008](man-2026-0008-protocols-and-connectors.md#oauth-20).

| Subcommand | Syntax | Result |
|---|---|---|
| `begin` | `auth begin PROFILE --account A` | authorization_code: `{transaction_id, expires_at, authorization_url}`; device_code: `{transaction_id, expires_at, verification_uri, user_code, interval_seconds}` |
| `complete` | `auth complete --data JSON \| --data-file PATH [--timeout D]` (aliases `--params`, `--params-file`: this command's own flags, not deprecated) | `{"transaction_id":…,"callback":{…}}` or `{"transaction_id":…,"wait":true}`; connected status, or `{"state":"pending",…}` when the deadline arrives first |
| `status` | `auth status PROFILE --account A` | `{profile, account, state, scopes, expires_at, generation}` (never refreshes) |
| `disconnect` | `auth disconnect PROFILE --account A` | `{…,"local_only":true,"generation":N}` |
| `cancel` | `auth cancel TRANSACTION_ID` | cancels an open transaction |

Failures: `auth begin` on a client_credentials profile → `validation.auth_flow` (exit 2); missing `allow_auth`
grant → `permission.denied` "allow_auth status crm_service/service/status denied" (exit 3); unknown transaction →
`not_found.auth_transaction` (exit 4); unknown profile → `not_found.auth_profile` (exit 4). OAuth transactions
live in one process: run `begin` and `complete` against the same `rivet serve` with `--endpoint`. Every `auth`
answer is an envelope whose `data` is the result above (0.2.0). A build without the `oauth` feature refuses any
bundle with an `auth … oauth2` profile at load (`unsupported.feature`, exit 5).

### rivet connectors sync

`rivet connectors sync <NAME> --output PATH` — discover an MCP connector's tools/resources/prompts and write a
**new** candidate snapshot (never overwrites); prints the sha256 to approve in `policy.json`.

```text
$ rivet connectors --file app.rivet sync peer --output ./schemas/peer.json            # captured at 829ca43; the 0.2.0 stdout is the rivet.connectors.sync envelope with this object in data
{"connector":"peer","path":"./schemas/peer.json","sha256":"sha256:9173a4bb…","protocolVersion":"2025-11-25","tools":["demo.greet","demo.add",…],"resources":[],"prompts":[]}
wrote candidate snapshot ./schemas/peer.json (sha256:9173a4bb…); after review, approve it in policy.json: "approved": {"snapshots": ["sha256:9173a4bb…"]}
```

Failures, in the order they are checked: no `allow_mcp` grant for `NAME/discover` → `permission.denied` (exit 3);
output exists → `conflict.already_exists` (exit 4) — **before** anything contacts the server; unknown connector →
`not_found.mcp_connector` (exit 4); unreachable server → `dns.*`/`connection.*` (exit 5).

```text
$ rivet --file app.rivet connectors sync crm --output ./schemas/crm.json          # 06-mcp-bridge (no discover grant)  [exit 3]
{"request_id":"","trace_id":"","operation":"rivet.connectors.sync","type":"result","status":"error","data":null,"error":{"kind":"permission","code":"permission.denied","message":"allow_mcp call crm/discover denied: no grant for allow_mcp crm/discover","retryable":false,"details":{"capability":"allow_mcp","access":"call","target":"crm/discover"}},"effects":"none","data_count":0}

$ rivet --file app.rivet connectors sync crm --output ./schemas/exists.json       # a copy with the grant, file present  [exit 4]
{"request_id":"","trace_id":"","operation":"rivet.connectors.sync","type":"result","status":"error","data":null,"error":{"kind":"conflict","code":"conflict.already_exists","message":"./schemas/exists.json already exists; connectors sync never overwrites a snapshot","retryable":false,"details":{"path":"./schemas/exists.json"}},"effects":"none","data_count":0}
```

After a snapshot is approved, the first call of each connector session compares the server's live `tools/list`
with it; any exposed tool whose name or `inputSchema` changed fails `mcp.schema_drift` (exit 5) before the call is
sent — run `connectors sync` again and review the new candidate.

### rivet serve

`rivet serve [--listen HOST:PORT] [--stdio]` — serve REST, SSE, polling, WebSocket and MCP on one listener
(default `127.0.0.1:8080`), or MCP only over stdio. Runs until SIGINT/SIGTERM (exit 0). Full guide:
[MAN-2026-0006](man-2026-0006-serving-and-surfaces.md).

```text
$ rivet serve --file app.rivet --listen 127.0.0.1:18960
{"listen_addr":"127.0.0.1:18960","stdio":false,"surfaces":["http","sse","poll","ws","mcp"],"auth_type":"none","catalog_version":"sha256:67104f0e…","policy_hash":null}   (stderr)
```

After the receipt, one JSON access-log line per request follows on stderr (with `"deprecated":1` for 0.1.0 input
keys); `GET /v1/health` answers the `rivet.health` envelope; SIGINT and SIGTERM drain (cancel in-flight requests and sessions, close
their handles within 5 s) and exit 0 — see [MAN-2026-0006](man-2026-0006-serving-and-surfaces.md#health-access-log-and-shutdown).

Failures: non-loopback without auth → `serve.auth_required` (exit 2); `--listen nothost` → `validation.usage`
(exit 2); `serve.auth.type` `mtls` → `unsupported.serve_mtls` (exit 5); a binary built without the `serve` feature →
`unsupported.feature` with `details.feature: "serve"` (exit 5); refused with `--endpoint` (exit 2). The refusal
envelopes are captured in [API-2026-0001](../api/api-2026-0001-http-rest-sse-polling.md#examples).

### rivet highlight

`rivet highlight <PATH> [--format ansi|html|json]` (0.2.0) — syntax-highlight a `.rivet` file from the parser's own
spans. No bundle is loaded and `--file` is not needed. Default format: `ansi` when stdout is a terminal, `json`
otherwise. Token classes and editor use: [MAN-2026-0010](man-2026-0010-editor-support-and-highlighting.md).

```text
 rivet highlight FILE ──parse (Capy)──▶ tokens {line, col, len, class, text}
      ├─ --format ansi ─▶ terminal colours          ├─ ok ─────────────▶ exit 0
      ├─ --format html ─▶ <span class="rv-CLASS">   └─ syntax error ──▶ tokens before it + error[CODE] (stderr), exit 2
      └─ --format json ─▶ one JSON token per line
```

```text
$ cat hl.rivet
global BASE = "https://api.example.com"

operation demo.ping
    name "Ping"
    output text description "pong"
    return "pong"
end

$ rivet highlight hl.rivet --format json
{"line":1,"col":1,"len":6,"class":"keyword","text":"global"}
{"line":1,"col":8,"len":4,"class":"global","text":"BASE"}
{"line":1,"col":13,"len":1,"class":"operator","text":"="}
{"line":1,"col":15,"len":25,"class":"string","text":"\"https://api.example.com\""}
{"line":3,"col":1,"len":9,"class":"keyword","text":"operation"}
{"line":3,"col":11,"len":9,"class":"operation_id","text":"demo.ping"}
{"line":4,"col":5,"len":4,"class":"keyword","text":"name"}
{"line":4,"col":10,"len":6,"class":"string","text":"\"Ping\""}
{"line":5,"col":5,"len":6,"class":"keyword","text":"output"}
{"line":5,"col":12,"len":4,"class":"type","text":"text"}
{"line":5,"col":17,"len":11,"class":"option","text":"description"}
{"line":5,"col":29,"len":6,"class":"string","text":"\"pong\""}
{"line":6,"col":5,"len":6,"class":"keyword","text":"return"}
{"line":6,"col":12,"len":6,"class":"string","text":"\"pong\""}
{"line":7,"col":1,"len":3,"class":"keyword","text":"end"}

$ rivet highlight hl.rivet --format html
<pre class="rv-source"><code><span class="rv-keyword">global</span> <span class="rv-global">BASE</span> <span class="rv-operator">=</span> <span class="rv-string">&quot;https://api.example.com&quot;</span>

<span class="rv-keyword">operation</span> <span class="rv-operation_id">demo.ping</span>
    <span class="rv-keyword">name</span> <span class="rv-string">&quot;Ping&quot;</span>
    <span class="rv-keyword">output</span> <span class="rv-type">text</span> <span class="rv-option">description</span> <span class="rv-string">&quot;pong&quot;</span>
    <span class="rv-keyword">return</span> <span class="rv-string">&quot;pong&quot;</span>
<span class="rv-keyword">end</span>
</code></pre>

$ rivet highlight hl.rivet --format ansi | cat -v | head -1
^[[1;35mglobal^[[0m ^[[1;33mBASE^[[0m ^[[37m=^[[0m ^[[32m"https://api.example.com"^[[0m
```

Failures:

```text
$ printf 'operation x.y\n    name "X"\n    return 1 +\nend\n' > broken.rivet
$ rivet highlight broken.rivet --format json                                                   [exit 2]
{"line":1,"col":1,"len":9,"class":"keyword","text":"operation"}
{"line":1,"col":11,"len":3,"class":"operation_id","text":"x.y"}
{"line":2,"col":5,"len":4,"class":"keyword","text":"name"}
{"line":2,"col":10,"len":3,"class":"string","text":"\"X\""}
error[syntax.expression]: the expression after `return` does not parse
  --> broken.rivet:3:5
   |
  3|     return 1 +
   |     ^^^^^^^^^^
  = hint: check its brackets, commas, quotes and object keys; list items are read with `xs.0`

$ rivet highlight broken.rivet --format json --json    # with --json the diagnostic is an error envelope (operation rivet.highlight)
…tokens…
{"request_id":"","trace_id":"","operation":"rivet.highlight","type":"result","status":"error","data":null,"error":{"kind":"syntax","code":"syntax.expression",…},"effects":"none","data_count":0}

$ rivet highlight nope.rivet                                                                   [exit 2]
error[validation.usage]: rivet highlight nope.rivet: entity not found

$ rivet highlight hl.rivet --format yaml                                                       [exit 2]
error: invalid value 'yaml' for '--format <FORMAT>'
  [possible values: ansi, html, json]
```

An unclosed block keeps every token before the error, its header included (fixed in 0.2.0, INC-2026-0012):

```text
$ printf 'operation x.y\n    name "X"\n    return (\n' > b.rivet; rivet highlight b.rivet --format json   [exit 2]
{"line":1,"col":1,"len":9,"class":"keyword","text":"operation"}
{"line":1,"col":11,"len":3,"class":"operation_id","text":"x.y"}
{"line":2,"col":5,"len":4,"class":"keyword","text":"name"}
{"line":2,"col":10,"len":3,"class":"string","text":"\"X\""}
error[syntax.expression]: the expression after `return` does not parse
  --> b.rivet:3:5
```

## Complete CLI Reference

| Command / Flag | Purpose and When to Use | Syntax / Type / Default | Inputs | Output / Exit Codes | Errors | Example | Since |
|---|---|---|---|---|---|---|---|
| `request` | run one operation (also built-ins such as `rivet.capabilities`) | `request [ID] [--data JSON \| --input FILE\|-] [--pretty] [--stream] [--timeout D ≤ 10m] [--input-jsonl -]` (`--params`: deprecated) | bundle, data, stdin | envelope (stdout ok / stderr error) or NDJSON records; 0,2–6,130 | validation (incl. `validation.input_envelope`), permission, not_found, timeout | `request demo.add --data '{"a":2}'` | 0.1.0 (`--data`, `--input`, `--pretty` 0.2.0) |
| `list` | catalog summary | `list [--outputs] [--json]` | bundle | table / `rivet.list` envelope; 0,2 | compile errors | `list --outputs` | 0.1.0 |
| `describe` | full descriptors | `describe [IDS…] [--json]` | bundle | table/JSON; 0,2,4 | not_found.operation | `describe demo.add` | 0.1.0 |
| `outputs` | declared outputs | `outputs ID \| --all [--json]` | bundle | table/JSON Schema; 0,2,4 | validation.query | `outputs --all` | 0.1.0 |
| `check` | compile only | `check [--strict-docs] [--json]` | bundle, modules, policy | `ok: …` + stderr warnings / `rivet.check` envelope; 0,2,3,4,5 | syntax, check.*, docs, import codes, unsupported.feature | `check --strict-docs` | 0.1.0 (`--json` envelope 0.2.0) |
| `graph` | static call graph | `graph ID [--all] [--json]` | bundle | tree/JSON; 0,2,4 | not_found.operation | `graph report.total` | 0.1.0 |
| `io` | I/O manifest | `io [IDS…] [--all] [--by …] [--kind K] [--access V] [--format …] [--check-policy] [--strict] [--needs] [--check-files] [--include-bootstrap] [--trace REQ]` | bundle, policy, files (probe) | table/JSON/MD/CSV; 0,2,3,4,7 | validation.usage | `io --by target` | 0.1.0 |
| `policy explain` | effective policy + decisions | `policy explain [ID] [--data JSON]` (alias `--params`) | bundle, policy | table; 0,2,3 | policy.invalid, permission.denied (`--json`) | `policy explain demo.read --data '{"path":"data/a.txt"}'` | 0.1.0 (`--data` 0.2.0) |
| `policy generate` | least-privilege draft | `policy generate [IDS…] [--all] [--output PATH]` | manifest | JSON; 0,4,7 | conflict.exists | `policy generate --all --output p.json` | 0.1.0 |
| `trace show` | decisions of one request | `trace show REQ` | server trace store | JSON; 0,4 | not_found.trace | `--endpoint URL trace show req_…` | 0.1.0 |
| `trace export` | save one trace to a new file | `trace export REQ --output PATH` | trace store; `allow_write create` | receipt JSON; 0,3,4 | not_found.trace, conflict.already_exists | `--endpoint URL trace export req_… --output ./audit/t.json` | 0.1.0 |
| `auth begin/complete/status/disconnect/cancel` | OAuth accounts | see above | profile, account, params file | JSON; 0,2,3,4 | validation.auth_flow, permission.denied | `auth status p --account a` | 0.1.0 |
| `connectors sync` | MCP snapshot candidate | `connectors sync NAME --output PATH` | live MCP server | JSON + stderr hint; 0,3,4 | conflict.already_exists | `connectors sync peer --output ./schemas/peer.json` | 0.1.0 |
| `serve` | all surfaces | `serve [--listen HOST:PORT] [--stdio]` | bundle, policy | stderr receipt; 0,2,5 | serve.auth_required, unsupported.serve_mtls, unsupported.feature | `serve --listen 127.0.0.1:8080` | 0.1.0 |
| `highlight` | colour a `.rivet` file | `highlight PATH [--format ansi\|html\|json]` | file | ANSI / HTML / JSON lines; 0,2 | syntax.*, validation.usage | `highlight app.rivet --format html` | 0.2.0 |
| `--file` | select bundle | path | — | — | validation.usage when absent | `--file app.rivet` | 0.1.0 |
| `--policy` | select policy file | path | — | — | policy.invalid | `--policy policies/read-only.json` | 0.1.0 |
| `--json` | JSON output (an envelope from 0.2.0) | flag | — | — | — | `list --json` | 0.1.0 |
| `--pretty` | indented JSON | flag | — | — | validation.usage with `--stream` | `request demo.add --data '{"a":1}' --pretty` | 0.2.0 |
| `--endpoint` / `--token-file` | remote mode | URL / path | server | same as local | auth.required, validation.usage | `--endpoint http://127.0.0.1:8080 --token-file t list` | 0.1.0 |

## Errors and Recovery Reference

See the table in [the root manual](man-2026-0001-rivet-manual.md#errors-and-recovery-reference) and the registry
[API-2026-0005](../api/api-2026-0005-error-registry.md); every CLI error is either an `error[CODE]: message` line
(compile/usage without `--json`) or the error envelope on stderr (request-like commands, and any command with `--json`).

## Limitations

The CLI-relevant rows of the [manual's Known Limitations](man-2026-0001-rivet-manual.md#known-limitations):

- No persistent trace store: `trace show`/`trace export` without `--endpoint` never find a trace (in-memory,
  per-process store).
- macOS and Linux only; Windows is not supported ([INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)).
- Processes are sandboxed only on macOS; on Linux the sandbox is gated (`unsupported.sandbox_backend`, exit 5).

## Version Applicability

| Feature / Interface | Introduced | Changed | Deprecated / Removed | Applicable Environment |
|---|---|---|---|---|
| All commands and flags above except those below | 0.1.0 | 0.2.0: JSON outputs are envelopes; errors of `request` go to stderr | — | macOS, Linux |
| `--data`, `--input`, `--pretty`, `highlight` | 0.2.0 | — | — | macOS, Linux |
| `request --params` | 0.1.0 | — | deprecated 0.2.0 (`warning[deprecated.params]`), removed 0.3.0 | macOS, Linux |
| `policy explain --params`, `auth complete --params`/`--params-file` | 0.1.0 | — | — (aliases of `--data`/`--data-file` from 0.2.0; not deprecated) | macOS, Linux |

## Related Documents

- [Rivet manual](man-2026-0001-rivet-manual.md) · [Quickstart](man-2026-0002-installation-and-quickstart.md) ·
  [Policy guide](man-2026-0005-policy-and-io-manifest-guide.md) · [Serving](man-2026-0006-serving-and-surfaces.md)
- [API-2026-0001 HTTP API](../api/api-2026-0001-http-rest-sse-polling.md) · [API-2026-0006 envelopes](../api/api-2026-0006-envelopes.md) · [API-2026-0005 errors](../api/api-2026-0005-error-registry.md) · [MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md) · [MAN-2026-0010 highlighting](man-2026-0010-editor-support-and-highlighting.md)
- Demos: [01-catalog](../demos/01-catalog/README.md), [11-sandbox](../demos/11-sandbox/README.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 6 | 2026-09-30 | Claude | v0.2.1 patch (PLAN-2026-0002 TASK-097): version strings, install tag v0.2.1; INC-2026-0013 behaviour where described. |
| 5 | 2026-09-30 | Claude | v0.2.0 release (PLAN-2026-0002 TASK-091): version strings and current-release wording updated to 0.2.0. |
| 1 | 2026-09-28 | Claude | Initial CLI reference for 0.1.0, every command executed against 0.1.0-dev commit f40d4aa. |
| 2 | 2026-09-28 | Claude | Fix batch through 829ca43 and 2a751ab: `rivet graph`, `rivet trace export` (remote form verified after the 2a751ab fix), emits/receives descriptions, `check` warnings and `check.unknown_function`, `policy explain --params` (exit 3), `--timeout` 10m cap, `connectors sync` output check before discovery and `mcp.schema_drift`, serve access log/health/drain, exit-code flow diagram; limitations aligned with MAN-2026-0001. |
| 3 | 2026-09-29 | Claude | 0.2.0 (D-23, D-46): `--data`, `--input FILE\|-`, `--pretty`, the `--params` deprecation warning (and the non-deprecated `policy explain`/`auth complete --params`), `rivet highlight` (formats, failures, a known deviation), envelopes for every JSON output (`list`, `describe`, `outputs`, `check`, `graph`, `io`, `policy explain/generate`, `trace`), stdout/stderr split, exit codes for the new codes; every example re-captured on the 0.2.0-rc (source `6f9943f`); `connectors sync` check order corrected; macOS/Linux. |
| 4 | 2026-09-29 | Claude | INC-2026-0012 fixes: `policy explain --data` (alias `--params`), params followed into called modules, `--json` denial = error envelope (exit 3); `auth complete --data/--data-file` (aliases `--params`/`--params-file`); `highlight` keeps the header tokens of an unclosed block (real output); WS `--timeout` limitation removed. |
