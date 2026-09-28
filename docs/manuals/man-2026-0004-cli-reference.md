---
document_id: MAN-2026-0004
title: "Rivet CLI reference"
document_type: manual
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
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
last_verified_version: "0.1.0-dev (commit f40d4aa)"
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
every example was executed (from `docs/demos/NN-*/` unless stated). Part of the
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
| `--endpoint URL` | URL | send `request`, `list`, `describe`, `outputs`, `io`, `trace`, `auth` to a running server | not with `--file`/`--policy`; `check`, `policy`, `serve` refused (exit 2) |
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
| 3 | permission or authentication | `permission.denied`, `file.hardlink_refused`, `auth.required`, `auth.invalid`; `io --check-policy` found a denied/unknown site; `io --check-files` found a file not permitted |
| 4 | not found or conflict | `not_found.*`, `conflict.*`; `io --check-files` found a missing file |
| 5 | dependency, runtime, unsupported, output_invalid, limit | `http.status`, `dns.resolve`, `process.exit`, `unsupported.*`, `output.invalid`, `limit.*`, application `fail` codes |
| 6 | timeout | `timeout.request`, `timeout.poll`, `timeout.scope` |
| 7 | inspection incomplete | `io --strict` with dynamic sites; `policy generate` with review items |
| 130 | cancelled | Ctrl-C during `request` (`cancelled.request`) |

Argument parsing errors (unknown subcommand, missing positional) are printed by the parser and exit 2.

## Task-Oriented Workflows

### rivet request

Invoke one operation. `rivet request [OPTIONS] <ID>`

| Option | Default | Purpose |
|---|---|---|
| `--params JSON` | `{}` | parameters as a JSON object |
| `--stream` | off | print NDJSON envelopes (data items, then the result) |
| `--timeout D` | `30s` | request deadline, `^[0-9]+(ms|s|m|h)$` |
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
$ rivet outputs --file app.rivet demo.countdown
demo.countdown — Count down
output  object   Summary returned after the last item.
  count   integer  required  Number of items emitted.
emits   integer
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

Other failures (exit 2): `syntax.*`, `check.unknown_operation`, `check.call_cycle`, `registry.duplicate_id`,
`policy.invalid`; exit 4 for a missing connector file (`not_found.mcp_snapshot`, `not_found.descriptor`); exit 2
for an unapproved snapshot (`mcp.snapshot_unapproved`). Refused with `--endpoint` (exit 2).

### rivet io

`rivet io [IDS]... [OPTIONS]` — the I/O manifest: every I/O site, its target, access verbs and capability.
Full workflows: [MAN-2026-0005](man-2026-0005-policy-and-io-manifest-guide.md#review-io-with-rivet-io).

| Option | Default | Purpose |
|---|---|---|
| `IDS…` / `--all` | all public operations | restrict to these operations (and what they call) |
| `--by operation|target|capability` | `operation` | table grouping |
| `--kind K` | all | `file`, `network`, `process`, `env`, `mcp`, `grpc`, `auth`, `credential`, … |
| `--access V[,V]` | all | filter by access verb (`create`, `delete`, `connect`, …) |
| `--format table|json|markdown|csv` | `table` | output format (`--json` = `--format json`) |
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
decision.

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

### rivet trace show

`rivet trace show <REQUEST_ID>` — broker decisions and attempts of one request, each with its `effect_id`.
Traces live in the memory of the process that ran the request, so use it with `--endpoint`:

```text
$ rivet --endpoint http://127.0.0.1:18411 trace show req_020a473102
{"request_id":"req_020a473102","attempts":[{"request_id":"req_020a473102","trace_id":"tr_020a473102","node_id":null,"attempt":1,"effect_id":"data.read#1","operation_id":"data.read","phase":"decision","capability":"allow_read","access":"read","target":"./data/public.json","decision":"allowed","policy_hash":"sha256:9475…","source":null,"outcome":{"rule":"grant allow_read ./data/**"}}],"complete":true,"next_cursor":null,"gaps":0}
```

Failure: a local `rivet trace --file app.rivet show req_…` → `not_found.trace` "no trace for request … in this
host's trace store" (exit 4) — a new process has an empty store. Pure requests record no decisions, so they have
no trace either.

### rivet auth

OAuth account management through the `rivet.auth.*` built-ins; tokens are never printed. Details and grants:
[MAN-2026-0008](man-2026-0008-protocols-and-connectors.md#oauth-20).

| Subcommand | Syntax | Result |
|---|---|---|
| `begin` | `auth begin PROFILE --account A` | authorization_code: `{transaction_id, expires_at, authorization_url}`; device_code: `{transaction_id, expires_at, verification_uri, user_code, interval_seconds}` |
| `complete` | `auth complete --params JSON | --params-file PATH [--timeout D]` | `{"transaction_id":…,"callback":{…}}` or `{"transaction_id":…,"wait":true}`; connected status, or `{"state":"pending",…}` when the deadline arrives first |
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

Failures: output exists → `conflict.already_exists` (exit 4); unknown connector → `not_found.mcp_connector`
(exit 4); no `allow_mcp` grant for `NAME/discover` → `permission.denied` (exit 3).

### rivet serve

`rivet serve [--listen HOST:PORT] [--stdio]` — serve REST, SSE, polling, WebSocket and MCP on one listener
(default `127.0.0.1:8080`), or MCP only over stdio. Runs until SIGINT/SIGTERM (exit 0). Full guide:
[MAN-2026-0006](man-2026-0006-serving-and-surfaces.md).

```text
$ rivet serve --file app.rivet --listen 127.0.0.1:18401
{"listen_addr":"127.0.0.1:18401","stdio":false,"surfaces":["http","sse","poll","ws","mcp"],"auth_type":"none","catalog_version":"sha256:67104f0e…","policy_hash":null}   (stderr)
```

Failures: non-loopback without auth → `serve.auth_required` (exit 2); `--listen nothost` → `validation.usage`
(exit 2); `serve.auth.type` `mtls` → `unsupported.serve_mtls` (exit 5); refused with `--endpoint` (exit 2).

## Complete CLI Reference

| Command / Flag | Purpose and When to Use | Syntax / Type / Default | Inputs | Output / Exit Codes | Errors | Example | Since |
|---|---|---|---|---|---|---|---|
| `request` | run one operation | `request ID [--params JSON] [--stream] [--timeout D] [--input-jsonl -]` | bundle, params, stdin | Completion / NDJSON; 0,2–6,130 | validation, permission, not_found, timeout | `request demo.add --params '{"a":2}'` | 0.1.0 |
| `list` | catalog summary | `list [--outputs] [--json]` | bundle | table/JSON; 0,2 | compile errors | `list --outputs` | 0.1.0 |
| `describe` | full descriptors | `describe [IDS…] [--json]` | bundle | table/JSON; 0,2,4 | not_found.operation | `describe demo.add` | 0.1.0 |
| `outputs` | declared outputs | `outputs ID | --all [--json]` | bundle | table/JSON Schema; 0,2,4 | validation.query | `outputs --all` | 0.1.0 |
| `check` | compile only | `check [--strict-docs]` | bundle, policy | `ok: …`; 0,2,4 | syntax, docs | `check --strict-docs` | 0.1.0 |
| `io` | I/O manifest | `io [IDS…] [--all] [--by …] [--kind K] [--access V] [--format …] [--check-policy] [--strict] [--needs] [--check-files] [--include-bootstrap] [--trace REQ]` | bundle, policy, files (probe) | table/JSON/MD/CSV; 0,2,3,4,7 | validation.usage | `io --by target` | 0.1.0 |
| `policy explain` | effective policy + decisions | `policy explain [ID] [--params JSON]` | bundle, policy | table; 0,2 | policy.invalid | `policy explain notes.update` | 0.1.0 |
| `policy generate` | least-privilege draft | `policy generate [IDS…] [--all] [--output PATH]` | manifest | JSON; 0,4,7 | conflict.exists | `policy generate --all --output p.json` | 0.1.0 |
| `trace show` | decisions of one request | `trace show REQ` | server trace store | JSON; 0,4 | not_found.trace | `--endpoint URL trace show req_…` | 0.1.0 |
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

- `trace show` without `--endpoint` never finds a trace (in-memory, per-process store).
- `--timeout` is not applied over the WebSocket duplex path (MAN-2026-0006).
- Windows is untested.

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
