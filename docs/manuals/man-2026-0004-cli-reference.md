---
document_id: MAN-2026-0004
title: "Rivet CLI reference"
document_type: manual
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 2
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
scope: Every rivet 0.1.0 command, subcommand, flag, default and exit code, with a verified success and failure example for each.
reason: PLAN-2026-0001 row D-37 (DOCUMENTATION.md §31 CLI impact) — the CLI reference is derived from `rivet --help` of the build and each command was executed.
related_documents: [MAN-2026-0001, MAN-2026-0002, MAN-2026-0005, MAN-2026-0006, MAN-2026-0008, API-2026-0001, DEMO-2026-0001, DEMO-2026-0011]
supersedes: null
superseded_by: null
tags: [rivet, manual, cli, reference, exit-codes]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.1.0-dev (commit 829ca43)"
next_review_date: 2026-10-28
---

# Rivet CLI reference

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** cli, registry, execution, policy, audit, connectors, auth, serve, sessions

## Purpose

The complete command-line contract of `rivet` 0.1.0. Every row was taken from `rivet … --help` of the build and
every example was executed (from `docs/demos/NN-*/` unless stated) at commit `f40d4aa`; the rows and examples of the
fix batch (`graph`, `trace export`, `check` warnings, `policy explain --params`, the `--timeout` cap,
`connectors sync` output check) were executed at commit `829ca43`. Part of the
[Rivet manual](man-2026-0001-rivet-manual.md).

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

| Global option | Type / default | Purpose | Constraints |
|---|---|---|---|
| `--file FILE` | path; none | entry `.rivet` file; `policy.json` beside it is discovered | required for every local command (`validation.usage`, exit 2) |
| `--policy POLICY` | path; discovered file | use this policy file instead | a path, never grant text; missing file → `policy.invalid` (exit 2) |
| `--json` | flag | print JSON instead of tables | request results are always JSON |
| `--endpoint URL` | URL | send `request`, `list`, `describe`, `outputs`, `io`, `trace`, `auth` to a running server | not with `--file`/`--policy`; `check`, `graph`, `policy`, `serve` refused (exit 2) |
| `--token-file FILE` | path | bearer token for `--endpoint` | never pass tokens in argv or env |
| `-h`, `--help` / `-V`, `--version` | flag | help / `rivet 0.1.0-dev` | — |

Output streams: results and error envelopes of `request`-like commands go to **stdout**; compile errors,
tables' diagnostics (`io: complete=false …`, `policy generate: N grants …`) and the serve receipt go to
**stderr**.

### Exit codes

| Code | Meaning | Typical codes |
|---|---|---|
| 0 | success | — |
| 2 | syntax, validation, usage or configuration error | `syntax.*`, `check.*`, `validation.*`, `policy.invalid`, `stream.*`, `serve.auth_required` |
| 3 | permission or authentication | `permission.denied`, `file.hardlink_refused`, `auth.required`, `auth.invalid`; `io --check-policy` found a denied/unknown site; `io --check-files` found a file not permitted; `policy explain ID --params …` found a denied concrete target |
| 4 | not found or conflict | `not_found.*`, `conflict.*`; `io --check-files` found a missing file |
| 5 | dependency, runtime, unsupported, output_invalid, limit | `http.status`, `dns.resolve`, `process.exit`, `unsupported.*`, `output.invalid`, `limit.*`, application `fail` codes |
| 6 | timeout | `timeout.request`, `timeout.poll`, `timeout.scope` |
| 7 | inspection incomplete | `io --strict` with dynamic sites; `policy generate` with review items |
| 130 | cancelled | Ctrl-C during `request` (`cancelled.request`) |

`check` **warnings** (`warning: …` on stderr) never change the exit code.

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

Invoke one operation. `rivet request [OPTIONS] <ID>`

| Option | Default | Purpose |
|---|---|---|
| `--params JSON` | `{}` | parameters as a JSON object |
| `--stream` | off | print NDJSON envelopes (data items, then the result) |
| `--timeout D` | `30s` | request deadline, `^[0-9]+(ms\|s\|m\|h)$`, at most `10m` (600000 ms, the same host cap as HTTP `deadline_ms`) |
| `--input-jsonl -` | none | stdin JSON Lines as live input for a `receives` operation; needs `--stream`; EOF = finish input |

```text
 rivet request ID --params '{…}' ──► validate ──► run ──► stdout: Completion JSON             exit 0
                                          │             └► stdout: error envelope JSON        exit 2..6
                                          └ Ctrl-C ─────► stdout: cancelled.request envelope  exit 130
```

Success:

```text
$ rivet request --file app.rivet demo.add --params '{"a":2,"b":3}'
{"request_id":"req_01fcf2a8bd","trace_id":"tr_01fcf2a8bd","result":5,"data_count":0,"effects":"none"}
```

Streaming (`--stream`) and live input:

```text
$ rivet request --file app.rivet demo.countdown --stream
{"request_id":"req_01f871eedd","trace_id":"tr_01f871eedd","seq":1,"type":"data","data":3}
…
{"request_id":"req_01f871eedd","trace_id":"tr_01f871eedd","result":{"count":3},"data_count":3,"effects":"none","type":"result"}

$ printf '{"a":1}\n"two"\n' | rivet request --file app.rivet lang.echo_input --stream --input-jsonl -
… two data envelopes, then {"result":{"received":2},…,"type":"result"}
```

Without `--stream`, a streaming operation prints only the Completion (`"data_count":3`).

Failures:

| Command | Output (abridged) | Exit |
|---|---|---|
| `request --file app.rivet demo.add --params notjson` | `validation.params` "--params is not valid JSON" | 2 |
| `request --file app.rivet demo.add --params '{"a":"x"}'` | `validation.type` | 2 |
| `request --file app.rivet demo.nope` | `not_found.operation` | 4 |
| `request --file missing.rivet demo.add` | `error[not_found.source]: cannot read missing.rivet` | 4 |
| `request --file app.rivet data.private` (11-sandbox) | `permission.denied` "… denied: deny allow_read ./data/private/**" | 3 |
| `request --file app.rivet slow.wait --timeout 100ms` | `timeout.request` "exceeded its 100 ms deadline" | 6 |
| `request … --timeout 1x` | `validation.usage` "--timeout 1x: use digits plus ms, s, m or h" | 2 |
| `request … --timeout 20m` | `validation.usage` "--timeout 20m is 1200000 ms; the host cap is 600000 ms (10m)", `details {timeout_ms, max_ms}` | 2 |
| `request … lang.echo_input --input-jsonl -` (no `--stream`) | `validation.usage` | 2 |
| Ctrl-C while running | `cancelled.request` "the request was cancelled by its caller", `effects: "unknown"` | 130 |

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

`--json` prints `{"operations":[{"id","name","description","streaming"}…],"next_cursor":null}`. Private
operations are never listed; imported MCP tools are (for example `peer.tools.demo.add`). Failure: a compile
error in the bundle (exit 2).

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

`--json` prints the descriptor with JSON Schemas (`input`, `output`, `emits`, `receives`). Failure:
`describe demo.nope` → `error[not_found.operation]: no operation \`demo.nope\`` (exit 4).

### rivet outputs

`rivet outputs [ID] [--all] [--json]` — declared output, emits, receives and errors.

```text
$ rivet outputs --file app.rivet demo.countdown          # commit 2a751ab: emits/receives lines show their descriptions
demo.countdown — Count down
output  object   Summary returned after the last item.
  count   integer  required  Number of items emitted.
emits   integer  One countdown value per item.
receives —
errors   —
```

Failures: no ID and no `--all` → `error[validation.query]: pass exactly one of an operation ID or --all` (exit
2); unknown ID → `not_found.operation` (exit 4).

### rivet check

`rivet check [--strict-docs]` — compile and check without running anything.

```text
$ rivet check --file app.rivet
ok: 4 operations, 0 connectors, 0 auth profiles
```

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
(`check.unguarded_result`). Missing descriptions are reported only with `--strict-docs`. Captured on a scratch
bundle (commit `829ca43`):

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
an orphan `else`), `check.unknown_operation`, `check.call_cycle`, `registry.duplicate_id`, `policy.invalid`; exit 4
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

`--json` prints `{operation_id, root, nodes:[{id, kind, label, source, condition, conditional, after}], edges:[{from,
to, kind: contains|after}]}`; node kinds are `operation`, `call`, `connector`, `effect`, `branch`, `dag`, `node`,
`cycle`:

```text
$ rivet --file app.rivet --json graph report.partial          # 05-dag (abridged)
{"operation_id":"report.partial","root":"n0","nodes":[{"id":"n0","kind":"operation","label":"report.partial","source":"app.rivet:45",…},
 {"id":"n6","kind":"node","label":"node blocked after bad","source":"app.rivet:56","condition":null,"conditional":false,"after":["bad"]},…],
 "edges":[{"from":"n0","to":"n1","kind":"contains"},…,{"from":"n4","to":"n6","kind":"after"},…]}
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
| `--format table\|json\|markdown\|csv` | `table` | output format (`--json` = `--format json`) |
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

`rivet policy explain [ID] [--params JSON]` — the effective policy; with an ID, each of its sites and the
decision. With `--params`, the call's **param-dependent targets are filled in** from those params and evaluated:
the TARGET column shows the concrete path/URL (`KNOWLEDGE exact`) and the command **exits 3 when any would be
denied** (plus a `denied: …` line). Without `--params` a param-dependent site shows its template (`{path}`) and
the exit code is 0.

```text
  policy explain ID                 policy explain ID --params '{"path":"app.rivet"}'
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
`allow_read ./data/**` — commit `829ca43`):

```text
$ rivet --file app.rivet policy explain demo.read --params '{"path":"data/a.txt"}'          [exit 0]
…
OPERATION  KIND  ACCESS  TARGET      KNOWLEDGE  SOURCE        DECISION
demo.read  file  read    data/a.txt  exact      app.rivet:73  allowed

$ rivet --file app.rivet policy explain demo.read --params '{"path":"app.rivet"}'           [exit 3]
…
OPERATION  KIND  ACCESS  TARGET     KNOWLEDGE  SOURCE        DECISION
demo.read  file  read    app.rivet  exact      app.rivet:73  denied
denied: demo.read#1 allow_read app.rivet (read)
```

A `"*"` target is flagged: `grant    allow_network *   ⚠ broad: "*" allows every target`. Failure: an invalid
policy → `error[policy.invalid]: policy.json /version: \`version\` must be 1` (exit 2). Refused with
`--endpoint`.

### rivet policy generate

`rivet policy generate [IDS]... [--all] [--output PATH]` — least-privilege draft from the manifest; no IDs = all.
Without `--output` the draft goes to stdout; the summary goes to stderr.

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

```text
$ rivet --endpoint http://127.0.0.1:18411 trace show req_020a473102
{"request_id":"req_020a473102","attempts":[{"request_id":"req_020a473102","trace_id":"tr_020a473102","node_id":null,"attempt":1,"effect_id":"data.read#1","operation_id":"data.read","phase":"decision","capability":"allow_read","access":"read","target":"./data/public.json","decision":"allowed","policy_hash":"sha256:9475…","source":null,"outcome":{"rule":"grant allow_read ./data/**"}}],"complete":true,"next_cursor":null,"gaps":0}
```

Failure: a local `rivet trace --file app.rivet show req_…` → `not_found.trace` "no trace for request … in this
host's trace store" (exit 4) — a new process has an empty store. Pure requests record no decisions, so they have
no trace either.

`trace export` (commit `829ca43`):

```text
$ rivet --file app.rivet trace export req_08460e9160 --output ./out/t2.json                      [exit 4]
{"request_id":"","trace_id":"","error":{"kind":"not_found","code":"not_found.trace","message":"no trace for request `req_08460e9160` in this host's trace store","retryable":false,"effects":"none"}}

```

Use the remote form against the process that ran the request (commit `2a751ab`; the path is relative to that
server's bundle and needs `allow_write` access `create` in its policy):

```text
$ rivet --endpoint http://127.0.0.1:18908 trace export req_01a505ce1d --output ./out/trace.json           [exit 0]
{"request_id":"req_01a505ce1d","path":"./out/trace.json","events":1,"bytes":718}

$ rivet --endpoint http://127.0.0.1:18908 trace export req_01a505ce1d --output ./out/trace.json           [exit 4]
{"request_id":"req_03a2920b07","trace_id":"tr_03a2920b07","error":{"kind":"conflict","code":"conflict.already_exists","message":"./out/trace.json already exists","retryable":false,"effects":"none"}}
```

A library host calls `Runtime::export_trace(request_id, path)` ([MAN-2026-0007](man-2026-0007-embedding-library.md)).

### rivet auth

OAuth account management through the `rivet.auth.*` built-ins; tokens are never printed. Details and grants:
[MAN-2026-0008](man-2026-0008-protocols-and-connectors.md#oauth-20).

| Subcommand | Syntax | Result |
|---|---|---|
| `begin` | `auth begin PROFILE --account A` | authorization_code: `{transaction_id, expires_at, authorization_url}`; device_code: `{transaction_id, expires_at, verification_uri, user_code, interval_seconds}` |
| `complete` | `auth complete --params JSON \| --params-file PATH [--timeout D]` | `{"transaction_id":…,"callback":{…}}` or `{"transaction_id":…,"wait":true}`; connected status, or `{"state":"pending",…}` when the deadline arrives first |
| `status` | `auth status PROFILE --account A` | `{profile, account, state, scopes, expires_at, generation}` (never refreshes) |
| `disconnect` | `auth disconnect PROFILE --account A` | `{…,"local_only":true,"generation":N}` |
| `cancel` | `auth cancel TRANSACTION_ID` | cancels an open transaction |

Failures: `auth begin` on a client_credentials profile → `validation.auth_flow` (exit 2); missing `allow_auth`
grant → `permission.denied` "allow_auth status crm_service/service/status denied" (exit 3); unknown transaction →
`not_found.auth_transaction` (exit 4); unknown profile → `not_found.auth_profile` (exit 4). OAuth transactions
live in one process: run `begin` and `complete` against the same `rivet serve` with `--endpoint`.

### rivet connectors sync

`rivet connectors sync <NAME> --output PATH` — discover an MCP connector's tools/resources/prompts and write a
**new** candidate snapshot (never overwrites); prints the sha256 to approve in `policy.json`.

```text
$ rivet connectors --file app.rivet sync peer --output ./schemas/peer.json
{"connector":"peer","path":"./schemas/peer.json","sha256":"sha256:9173a4bb…","protocolVersion":"2025-11-25","tools":["demo.greet","demo.add",…],"resources":[],"prompts":[]}
wrote candidate snapshot ./schemas/peer.json (sha256:9173a4bb…); after review, approve it in policy.json: "approved": {"snapshots": ["sha256:9173a4bb…"]}
```

Failures: output exists → `conflict.already_exists` (exit 4) — checked **before** anything contacts the server;
unknown connector → `not_found.mcp_connector` (exit 4); no `allow_mcp` grant for `NAME/discover` →
`permission.denied` (exit 3).

```text
$ rivet --file app.rivet connectors sync crm --output ./schemas/crm.json          # 06-mcp-bridge  [exit 4]
{"request_id":"","trace_id":"","error":{"kind":"conflict","code":"conflict.already_exists","message":"./schemas/crm.json already exists; connectors sync never overwrites a snapshot","retryable":false,"effects":"none","details":{"path":"./schemas/crm.json"}}}
```

After a snapshot is approved, the first call of each connector session compares the server's live `tools/list`
with it; any exposed tool whose name or `inputSchema` changed fails `mcp.schema_drift` (exit 5) before the call is
sent — run `connectors sync` again and review the new candidate.

### rivet serve

`rivet serve [--listen HOST:PORT] [--stdio]` — serve REST, SSE, polling, WebSocket and MCP on one listener
(default `127.0.0.1:8080`), or MCP only over stdio. Runs until SIGINT/SIGTERM (exit 0). Full guide:
[MAN-2026-0006](man-2026-0006-serving-and-surfaces.md).

```text
$ rivet serve --file app.rivet --listen 127.0.0.1:18401
{"listen_addr":"127.0.0.1:18401","stdio":false,"surfaces":["http","sse","poll","ws","mcp"],"auth_type":"none","catalog_version":"sha256:67104f0e…","policy_hash":null}   (stderr)
```

After the receipt, one JSON access-log line per request follows on stderr; `GET /v1/health` answers
`{"status":"ok","catalog_version":…}`; SIGINT and SIGTERM drain (cancel in-flight requests and sessions, close
their handles within 5 s) and exit 0 — see [MAN-2026-0006](man-2026-0006-serving-and-surfaces.md#health-access-log-and-shutdown).

Failures: non-loopback without auth → `serve.auth_required` (exit 2); `--listen nothost` → `validation.usage`
(exit 2); `serve.auth.type` `mtls` → `unsupported.serve_mtls` (exit 5); refused with `--endpoint` (exit 2).

## Complete CLI Reference

| Command / Flag | Purpose and When to Use | Syntax / Type / Default | Inputs | Output / Exit Codes | Errors | Example | Since |
|---|---|---|---|---|---|---|---|
| `request` | run one operation (also built-ins such as `rivet.capabilities`) | `request ID [--params JSON] [--stream] [--timeout D ≤ 10m] [--input-jsonl -]` | bundle, params, stdin | Completion / NDJSON; 0,2–6,130 | validation, permission, not_found, timeout | `request demo.add --params '{"a":2}'` | 0.1.0 |
| `list` | catalog summary | `list [--outputs] [--json]` | bundle | table/JSON; 0,2 | compile errors | `list --outputs` | 0.1.0 |
| `describe` | full descriptors | `describe [IDS…] [--json]` | bundle | table/JSON; 0,2,4 | not_found.operation | `describe demo.add` | 0.1.0 |
| `outputs` | declared outputs | `outputs ID \| --all [--json]` | bundle | table/JSON Schema; 0,2,4 | validation.query | `outputs --all` | 0.1.0 |
| `check` | compile only | `check [--strict-docs]` | bundle, policy | `ok: …` + stderr warnings; 0,2,4 | syntax, check.unknown_function, docs | `check --strict-docs` | 0.1.0 |
| `graph` | static call graph | `graph ID [--all] [--json]` | bundle | tree/JSON; 0,2,4 | not_found.operation | `graph report.total` | 0.1.0 |
| `io` | I/O manifest | `io [IDS…] [--all] [--by …] [--kind K] [--access V] [--format …] [--check-policy] [--strict] [--needs] [--check-files] [--include-bootstrap] [--trace REQ]` | bundle, policy, files (probe) | table/JSON/MD/CSV; 0,2,3,4,7 | validation.usage | `io --by target` | 0.1.0 |
| `policy explain` | effective policy + decisions | `policy explain [ID] [--params JSON]` | bundle, policy | table; 0,2,3 | policy.invalid | `policy explain demo.read --params '{"path":"data/a.txt"}'` | 0.1.0 |
| `policy generate` | least-privilege draft | `policy generate [IDS…] [--all] [--output PATH]` | manifest | JSON; 0,4,7 | conflict.exists | `policy generate --all --output p.json` | 0.1.0 |
| `trace show` | decisions of one request | `trace show REQ` | server trace store | JSON; 0,4 | not_found.trace | `--endpoint URL trace show req_…` | 0.1.0 |
| `trace export` | save one trace to a new file | `trace export REQ --output PATH` | trace store; `allow_write create` | receipt JSON; 0,3,4 | not_found.trace, conflict.already_exists | `--endpoint URL trace export req_… --output ./audit/t.json` | 0.1.0 |
| `auth begin/complete/status/disconnect/cancel` | OAuth accounts | see above | profile, account, params file | JSON; 0,2,3,4 | validation.auth_flow, permission.denied | `auth status p --account a` | 0.1.0 |
| `connectors sync` | MCP snapshot candidate | `connectors sync NAME --output PATH` | live MCP server | JSON + stderr hint; 0,3,4 | conflict.already_exists | `connectors sync peer --output ./schemas/peer.json` | 0.1.0 |
| `serve` | all surfaces | `serve [--listen HOST:PORT] [--stdio]` | bundle, policy | stderr receipt; 0,2,5 | serve.auth_required, unsupported.serve_mtls | `serve --listen 127.0.0.1:8080` | 0.1.0 |
| `--file` | select bundle | path | — | — | validation.usage when absent | `--file app.rivet` | 0.1.0 |
| `--policy` | select policy file | path | — | — | policy.invalid | `--policy policies/read-only.json` | 0.1.0 |
| `--json` | JSON output | flag | — | — | — | `list --json` | 0.1.0 |
| `--endpoint` / `--token-file` | remote mode | URL / path | server | same as local | auth.required, validation.usage | `--endpoint http://127.0.0.1:8080 --token-file t list` | 0.1.0 |

## Errors and Recovery Reference

See the table in [the root manual](man-2026-0001-rivet-manual.md#errors-and-recovery-reference); every CLI error
is either an `error[CODE]: message` line (compile/usage) or the JSON error envelope (request-like commands).

## Limitations

The CLI-relevant rows of the [manual's Known Limitations](man-2026-0001-rivet-manual.md#known-limitations):

- No persistent trace store: `trace show`/`trace export` without `--endpoint` never find a trace (in-memory,
  per-process store).
- `--timeout` is not applied over the WebSocket duplex path (`--stream --input-jsonl -` with `--endpoint`).
- Bundles are single files (no `import`).
- Processes are sandboxed only on macOS; on Linux the sandbox is gated and Windows/other OSes are unsupported.

## Version Applicability

| Feature / Interface | Introduced | Changed | Deprecated / Removed | Applicable Environment |
|---|---|---|---|---|
| All commands and flags above | 0.1.0 | — | — | development, server |

## Related Documents

- [Rivet manual](man-2026-0001-rivet-manual.md) · [Quickstart](man-2026-0002-installation-and-quickstart.md) ·
  [Policy guide](man-2026-0005-policy-and-io-manifest-guide.md) · [Serving](man-2026-0006-serving-and-surfaces.md)
- [API-2026-0001 HTTP API](../api/api-2026-0001-http-rest-sse-polling.md)
- Demos: [01-catalog](../demos/01-catalog/README.md), [11-sandbox](../demos/11-sandbox/README.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial CLI reference for 0.1.0, every command executed against 0.1.0-dev commit f40d4aa. |
| 2 | 2026-09-28 | Claude | Fix batch through 829ca43 and 2a751ab: `rivet graph`, `rivet trace export` (remote form verified after the 2a751ab fix), emits/receives descriptions, `check` warnings and `check.unknown_function`, `policy explain --params` (exit 3), `--timeout` 10m cap, `connectors sync` output check before discovery and `mcp.schema_drift`, serve access log/health/drain, exit-code flow diagram; limitations aligned with MAN-2026-0001. |
