---
document_id: SYS-2026-0008
title: "policy.json schema v1 reference"
document_type: system
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
component_owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [policy, serve, connectors, auth]
affected_versions:
  from: "0.1.0"
  to: null
last_verified_version: "0.1.0-dev (commit f40d4aa)"
review_cycle: on-release
confidentiality: internal
scope: Every key, type, default and validation error of policy.json schema v1 as implemented by the Rivet loader, with discovery rules and verified examples.
reason: policy.json is Rivet's only configuration and only source of authority; operators need an exact, verified reference of what the loader accepts, what it rejects and with which message.
related_documents: [PROP-2026-0001, PLAN-2026-0001, SYS-2026-0003, SYS-2026-0004, SYS-2026-0006, SYS-2026-0009]
supersedes: null
superseded_by: null
tags: [rivet, system, policy, configuration, schema, reference, serve, auth]
---

# policy.json schema v1 reference

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** policy, serve, connectors, auth
> **Last Verified Version:** 0.1.0-dev (commit f40d4aa)

## Summary

`policy.json` is the only configuration file Rivet reads and the only place authority can be granted.
Schema v1 has seven top-level keys: `version` (required, must be `1`), `grants`, `deny`, `network`,
`limits`, `serve` and `approved`. The loader (`parse_policy` in `src/features/policy/load_policy.rs`) is
strict: an unknown key, a wrong type, an unknown capability, an access verb outside its capability, a
malformed selector, a non-positive limit or a bad `serve` block stops the program with
`policy.invalid` (exit 2) and a message naming the JSON pointer. Only the first error is reported.

```text
 policy.json v1
 ├── version   : 1                                   (required)
 ├── grants    : [ {capability, targets[], access[]?} ]   default []
 ├── deny      : [ {capability, targets[], access[]?} ]   default []
 ├── network   : { deny_private_ranges: bool }            default {true}
 ├── limits    : { max_concurrent_requests,               default 64
 │                 max_call_depth,                        default 16
 │                 max_buffered_bytes }                   default 268435456
 ├── serve     : { surfaces[], auth{type,…}, principals{name:{operations[]}} }
 │                                                        default all 5 surfaces, auth none, no map
 └── approved  : { snapshots[], overlaps[] }              default [] / []
```

## Responsibilities

| This reference covers | Implemented in |
|---|---|
| Where the file is found and what happens when it is absent | `src/infra/policy_file_reader.rs`, `src/features/policy/load_policy.rs` |
| Every key, type and default | `src/domain/policy.rs` (`Policy`, `Grant`, `NetworkPolicy`, `PolicyLimits`, `ServePolicy`, `ServeAuth`, `ApprovedHashes`) |
| Every validation branch and its exact message | `parse_policy`, `grant_list`, `validate_selector`, `serve_policy`, `check_keys`, `string_list` in `src/features/policy/load_policy.rs` |
| How each key is consumed at run time | broker (`src/features/policy/authorize_effect.rs`), dispatcher (`src/orchestrator/runtime.rs`, `src/features/execution/request_operation.rs`), serve (`src/features/serve/*.rs`), MCP connectors (`src/infra/mcp_client.rs`) |

## Boundaries and Non-Responsibilities

- How a decision is computed from grants and deny entries is described in
  [SYS-2026-0003](../components/sys-2026-0003-policy-broker-and-io-manifest.md); this document only
  summarizes the matching rules each selector obeys.
- Serve routes, principals on each surface, and WebSocket/MCP details are in
  [SYS-2026-0004](../components/sys-2026-0004-surfaces-and-serve.md).
- OAuth profiles and credential stores are declared in `.rivet` source, not in `policy.json`; only the
  `allow_auth` / `allow_credentials` grants live here ([SYS-2026-0006](../integrations/sys-2026-0006-oauth-and-credentials.md)).
- There is no environment variable, no command-line grant syntax and no other config file.

## Architecture

### Discovery

```text
                   rivet <command> --file DIR/app.rivet [--policy PATH]
                                         │
                           ┌─────────────┴──────────────┐
                    --policy PATH given?                 │ no
                           │ yes                         ▼
                           ▼                      DIR/policy.json is a file?
                PATH is a file? ── no ──▶ policy.invalid     │ yes              │ no
                           │ yes          "--policy PATH:    ▼                  ▼
                           ▼               no such file"   read bytes        Policy::deny_all(DIR)
                       read bytes          exit 2          sha256            present = false
                       sha256                                │                every application effect
                           └──────────────┬──────────────────┘                denied; pure operations run
                                          ▼
                                 parse_policy (strict v1) ── error ──▶ policy.invalid, exit 2, nothing runs
                                          │ ok
                                          ▼
                            Policy { present, file, base_dir = dir of the policy file,
                                     sha256 = "sha256:<hex>", grants, deny, network, limits, serve, approved }
```

- `--file` is required for every command that loads a bundle (`error[validation.usage]: --file PATH is
  required (the entry .rivet file)`, exit 2).
- `--policy PATH` is a path, never grant text; it replaces discovery entirely.
- `--endpoint URL` refuses `--policy` and `--file`: `error[validation.usage]: --endpoint cannot be combined
  with --file or --policy: the server owns the bundle and its policy`, exit 2.
- Library hosts: `Runtime::builder().file(p).policy_file(p)` (same rules as `--policy`),
  `.policy(Policy)`, and `rivet::policy_from_json(bytes, base_dir)` (same parser, file name `<memory>`).
  A builder with `.source(…)` and no policy uses deny-by-default.
- **Base directory.** Relative path selectors resolve against the directory of the policy file (shown as
  `base` by `rivet policy explain`), not against the working directory.

### Validation order

Checks run top to bottom; the first failure is returned. Keys are checked in document order
(`serde_json` `preserve_order`).

```text
 bytes ─▶ valid JSON? ─▶ top level is object? ─▶ every top-level key allowed? ─▶ version == 1 (integer)?
       ─▶ grants ─▶ deny ─▶ network ─▶ limits ─▶ approved ─▶ serve ─▶ Policy
          (for each entry: object? keys allowed? capability? targets non-empty? each selector valid?
           access verbs known and belonging to the capability?)
```

## Interfaces

| Command | Loads policy? | On `policy.invalid` |
|---|---|---|
| `rivet check --file F [--policy P]` | yes | prints the error, exit 2 |
| `rivet policy explain [ID] --file F` | yes | exit 2 |
| `rivet io …`, `rivet policy generate …` | yes | exit 2 |
| `rivet request ID --file F …` | yes | exit 2, nothing runs |
| `rivet serve --file F …` | yes | exit 2, nothing is bound |
| `Runtime::builder()….build()` | yes | `Err(RivetError{code: "policy.invalid"})` |

Error shape (CLI, stderr): `error[policy.invalid]: policy.json <pointer>: <message>`. The error carries
`details.pointer` (the JSON pointer; empty string for whole-document errors). A read failure of a
discovered file is `policy.invalid` with the message `<path>: <io error>`.

## Configuration

### Top level

| Key | Type | Required | Default | Invalid value → message (all `policy.invalid`, exit 2) |
|---|---|---|---|---|
| (document) | JSON | — | — | `policy.json : not valid JSON: <serde error>` |
| (document) | object | — | — | `policy.json : the top level must be an object` |
| any other key | — | — | — | `` policy.json /<key>: unknown key `<key>` (allowed: version, grants, deny, network, limits, serve, approved) `` |
| `version` | integer `1` | yes | — | missing, `2`, `"1"`, `1.0` → `` policy.json /version: `version` must be 1 `` |
| `grants` | list of grant entries | no | `[]` | see [grants and deny](#grants-and-deny) |
| `deny` | list of grant entries | no | `[]` | same as `grants` with pointer `/deny/…` |
| `network` | object | no | `{"deny_private_ranges": true}` | see [network](#network) |
| `limits` | object | no | 64 / 16 / 268435456 | see [limits](#limits) |
| `serve` | object | no | all surfaces, auth none, no principal map | see [serve](#serve) |
| `approved` | object | no | empty lists | see [approved](#approved) |

### grants and deny

Each entry is `{"capability": …, "targets": […], "access": […]?}`. `grants` allow; `deny` entries use the
same shape and **always win** over grants (a deny entry with `access` blocks only those verbs).

| Key | Type | Required | Default | Invalid value → message (pointer `/grants/N/…` or `/deny/N/…`) |
|---|---|---|---|---|
| (list) | list | — | — | `/grants: must be a list` |
| (entry) | object | — | — | `/grants/N: must be an object` |
| other key | — | — | — | `` /grants/N/<key>: unknown key `<key>` (allowed: capability, targets, access) `` |
| `capability` | string, one of 13 | yes | — | missing or non-string → `/grants/N/capability: is required`; unknown → `` /grants/N/capability: unknown capability `<name>` `` |
| `targets` | list of strings, ≥ 1 | yes | — | missing or `[]` → `/grants/N/targets: needs at least one target`; not a list → `/grants/N/targets: must be a list of strings`; item not a string → `/grants/N/targets/J: must be a string`; see selector rules below |
| `access` | list of verb strings | no | every verb of the capability | not a list → `/grants/N/access: must be a list of strings`; unknown → `` /grants/N/access/J: unknown access verb `<v>` ``; wrong capability → `` /grants/N/access/J: `<v>` does not belong to <capability> (allowed: <verbs>) `` |

Capabilities and their access verbs (`Capability::verbs` in `src/domain/policy.rs`):

| Capability | Verbs | Target form | Example target |
|---|---|---|---|
| `allow_read` | `read`, `list`, `stat`, `watch` | path glob | `./data/**` |
| `allow_write` | `create`, `update`, `append` | path glob | `./out/**` |
| `allow_delete` | `delete` | path glob | `./out/**` |
| `allow_network` | `connect` | URL, IP or CIDR | `https://api.example.com:443`, `udp://127.0.0.1:7000`, `quic://engine.example.com:4433` |
| `allow_listen` | `bind`, `listen`, `multicast_join` | URL, IP or CIDR | `udp://127.0.0.1:7001`, `239.1.2.3` |
| `allow_exec` | `exec` | path glob | `/usr/bin/git` |
| `allow_env` | `read` | env name (glob) | `CRM_CLIENT_SECRET` |
| `allow_pipe` | `read`, `write` | path glob | `./run/fifo` |
| `allow_unix` | `connect`, `listen` | path glob | `./run/app.sock` |
| `allow_mcp` | `call` | `CONNECTOR/KIND/NAME` | `crm/tools/search` |
| `allow_grpc` | `call` | `CONNECTOR/Service/Method` | `users/example.Users/GetUser` |
| `allow_auth` | `use`, `manage`, `status` | `PROFILE/ACCOUNT/VERB` | `crm_service/service/use` |
| `allow_credentials` | `read`, `write` | `PROFILE/ACCOUNT` | `crm_service/service` |

Selector validation (`validate_selector`), pointer `/grants/N/targets/J`:

| Rule | Message |
|---|---|
| `"*"` is always valid (matches every target; `policy explain` marks it `⚠ broad`) | — |
| blank or whitespace-only | `empty target` |
| `allow_network` / `allow_listen`: must contain `://`, be an IP, or contain `/` (CIDR) | `` `<t>` must be a URL such as https://host:443, an IP or a CIDR `` |
| path-like capabilities: no NUL byte | `path contains a NUL byte` |
| must compile as a glob | `` `<t>` is not a valid selector `` |

Matching at run time (details in [SYS-2026-0003](../components/sys-2026-0003-policy-broker-and-io-manifest.md)):
path selectors are joined to the policy file's directory and normalized; `*` stays within one path
segment, `**` crosses segments, and `dir/**` also matches `dir`. URL selectors match scheme, host and
port (defaults made explicit) and a path prefix; an empty or `/` path covers every path. IP and CIDR
selectors match URL hosts that are IP literals.

### network

| Key | Type | Default | Invalid value → message |
|---|---|---|---|
| (object) | object | `{"deny_private_ranges": true}` | `/network: must be an object` |
| other key | — | — | `` /network/<key>: unknown key `<key>` (allowed: deny_private_ranges) `` |
| `deny_private_ranges` | boolean | `true` | `/network/deny_private_ranges: must be true or false` |

With `true`, targets on loopback, RFC 1918, link-local (incl. `169.254.169.254`), CGNAT, unspecified,
broadcast, `fc00::/7` and `fe80::/10` addresses are denied on every scheme (including `udp://`,
`quic://`, `tcp://`) unless a grant names the IP, a CIDR containing it, or a URL with that IP host
literally; `"*"` never counts. Resolved hostnames are re-checked the same way before dialing.

### limits

| Key | Type | Default | Consumed by | Invalid value → message |
|---|---|---|---|---|
| (object) | object | — | — | `/limits: must be an object` |
| other key | — | — | — | `` /limits/<key>: unknown key `<key>` (allowed: max_concurrent_requests, max_call_depth, max_buffered_bytes) `` |
| `max_concurrent_requests` | positive integer | `64` | host semaphore for top-level requests; when full → `limit.concurrency` "limits.max_concurrent_requests (N) reached" (exit 5) | `/limits/max_concurrent_requests: must be a positive integer` |
| `max_call_depth` | positive integer | `16` | nested `(request …)` depth → `limit.call_depth` (exit 5) | `/limits/max_call_depth: must be a positive integer` |
| `max_buffered_bytes` | positive integer | `268435456` (256 MiB) | shown by `policy explain`; not enforced in this version | `/limits/max_buffered_bytes: must be a positive integer` |

`0`, negative numbers, fractions and strings are all rejected. `max_concurrent_requests` and
`max_call_depth` are stored as 32-bit values: larger numbers are accepted but wrap (see
[Known Limitations](#known-limitations)).

### serve

| Key | Type | Default | Invalid value → message |
|---|---|---|---|
| (object) | object | see below | `/serve: must be an object` |
| other key | — | — | `` /serve/<key>: unknown key `<key>` (allowed: surfaces, auth, principals) `` |
| `surfaces` | list of `http`, `sse`, `poll`, `ws`, `mcp` | all five | not a list → `/serve/surfaces: must be a list of strings`; unknown → `` /serve/surfaces/I: unknown surface `<x>` (http, sse, poll, ws, mcp) `` |
| `auth` | object with `type` | `{"type": "none"}` | not an object → `/serve/auth: must be an object`; missing or other `type` → `/serve/auth/type: must be none, bearer or mtls` |
| `principals` | object `name → {"operations": [pattern…]}` | absent (no map) | not an object → `/serve/principals: must be an object`; value not an object → `/serve/principals/<name>: must be an object`; other key → `` /serve/principals/<name>/<key>: unknown key `<key>` (allowed: operations) ``; `operations` not a list → `/serve/principals/<name>/operations: must be a list of strings` |

`serve.auth` variants:

| `type` | Allowed keys | Field rules and messages | Behaviour |
|---|---|---|---|
| `none` | `type` | extra key → `` /serve/auth/<key>: unknown key `<key>` (allowed: type) `` | principal `local`; loopback binds only — `rivet serve --listen 0.0.0.0:…` refuses with `serve.auth_required` "non-loopback listener requires serve.auth in policy.json" (exit 2) |
| `bearer` | `type`, `tokens` | `tokens` missing / not a list → `/serve/auth/tokens: is required for bearer auth`; entry without string `principal` → `` /serve/auth/tokens/I: needs `principal` ``; `sha256` missing or not 64 hex chars → `/serve/auth/tokens/I/sha256: must be 64 hex characters (the SHA-256 of the token)` | `Authorization: Bearer TOKEN`; SHA-256 of the token compared in constant time; missing header → `auth.required` (401), no match → `auth.invalid` (401) |
| `mtls` | `type`, `client_ca`, `principals` | `client_ca` missing → `/serve/auth/client_ca: is required for mtls`; entry without `principal` → `` /serve/auth/principals/I: needs `principal` ``; without `subject` → `` /serve/auth/principals/I: needs `subject` `` | validates, but `rivet serve` refuses to start: `unsupported.serve_mtls` (exit 5) |

Operation patterns in `serve.principals.<name>.operations`:

```text
 pattern          matches                               never matches
 "users.get"      exactly users.get                     —
 "demo.*"         demo.add, demo.x.y                    demo, rivet.io
 "*"              every public operation                rivet.io, rivet.policy.generate,
                                                        rivet.trace.show, rivet.connectors.sync
 "rivet.io"       rivet.io (exact entry required for the four sensitive built-ins)
```

- The loopback principal `local` (auth `none`) may call everything, including the sensitive built-ins.
- Without a `principals` map, every authenticated principal may call every public operation except the
  four sensitive built-ins.
- With a map, a principal that has no entry, or whose patterns do not match, gets `permission.denied`
  (HTTP 403, exit 3): "principal `ci` may not call `notes.save`".
- Generic built-ins (`rivet.request`, `rivet.list`, `rivet.describe`, `rivet.outputs`, `rivet.sessions.*`)
  are allowed; the operation they name is authorized separately. Listings are filtered by the same rule.

### approved

| Key | Type | Default | Consumed by | Invalid value → message |
|---|---|---|---|---|
| (object) | object | — | — | `/approved: must be an object` |
| other key | — | — | — | `` /approved/<key>: unknown key `<key>` (allowed: snapshots, overlaps) `` |
| `snapshots` | list of strings (`"sha256:<hex>"`) | `[]` | MCP connectors: the `schema` snapshot's sha256 must be listed or the bundle does not load ([SYS-2026-0009](../integrations/sys-2026-0009-mcp-client-connectors.md)); `rivet connectors sync` prints the hash to add | not a list → `/approved/snapshots: must be a list of strings`; item → `/approved/snapshots/I: must be a string` |
| `overlaps` | list of strings | `[]` | parsed only; not consumed in this version | same messages with `/approved/overlaps` |

## Runtime Behaviour

### Valid examples (from `docs/demos`)

File CRUD (`docs/demos/02-file-crud/policy.json`):

```json
{
  "version": 1,
  "grants": [
    {"capability": "allow_read", "targets": ["./out", "./out/**"]},
    {"capability": "allow_write", "targets": ["./out/**"]},
    {"capability": "allow_delete", "targets": ["./out/**"]}
  ],
  "deny": []
}
```

```text
$ cd docs/demos/02-file-crud && rivet check --file app.rivet
ok: 5 operations, 0 connectors, 0 auth profiles
$ rivet io --file app.rivet --check-policy
OPERATION     KIND  ACCESS  TARGET           KNOWLEDGE  SOURCE        DECISION
notes.create  file  create  ./out/note.json  exact      app.rivet:9   allowed
notes.delete  file  delete  ./out/note.json  exact      app.rivet:40  allowed
notes.list    file  list    ./out            exact      app.rivet:50  allowed
notes.read    file  read    ./out/note.json  exact      app.rivet:19  allowed
notes.update  file  stat    ./out/note.json  exact      app.rivet:30  allowed
notes.update  file  update  ./out/note.json  exact      app.rivet:30  allowed
6 allowed
exit=0
```

Other demo policies show each key in use:

| Demo | Keys exercised |
|---|---|
| `03-http/policy.json` | `allow_network` origin `https://api.example.com:443`, `network.deny_private_ranges` |
| `06-mcp-bridge/policy.json` | `allow_mcp` `crm/tools/search` plus the transport origin |
| `07-oauth2/policy.json` | `allow_auth` `crm_service/service/use`, `allow_credentials`, `allow_env`, `serve.surfaces ["http","mcp"]`, `serve.auth none` |
| `08-udp/policy.json` | literal loopback grant `udp://127.0.0.1:7000` |
| `09-quic/policy.json` | `quic://engine.example.com:4433` |
| `10-grpc/policy.json` | four `allow_grpc` methods, `limits`, all five surfaces |
| `11-sandbox/policy.json` | `deny` `./data/private/**` overriding `./data/**`, `serve.surfaces ["http"]` |
| `12-library/policy.json` | only `limits` (every effect denied) |

Access narrowing and deny (scratch bundle; full walk-through in
[SYS-2026-0003](../components/sys-2026-0003-policy-broker-and-io-manifest.md)):

```json
{
  "version": 1,
  "grants": [
    {"capability": "allow_read", "targets": ["./out/**"], "access": ["stat"]},
    {"capability": "allow_write", "targets": ["./out/**"], "access": ["create"]},
    {"capability": "allow_read", "targets": ["./data/**"]},
    {"capability": "allow_network", "targets": ["*", "http://127.0.0.1:18439"]}
  ],
  "deny": [
    {"capability": "allow_read", "targets": ["./data/private/**"]}
  ]
}
```

```text
$ rivet policy explain --file app.rivet
policy   ./policy.json (sha256:f288f0d547622c648aca3b8e87e2eff7f2bd46eb8634eabc01b3e31507dc19d4)
base     .
network  deny_private_ranges true
limits   64 concurrent, depth 16, 268435456 buffered bytes
grant    allow_read ./out/** access [stat]
grant    allow_write ./out/** access [create]
grant    allow_read ./data/**
grant    allow_network *, http://127.0.0.1:18439   ⚠ broad: "*" allows every target
deny     allow_read ./data/private/**

$ rivet request notes.touch --file app.rivet --params '{}'
{"request_id":"req_015d36e1ad","trace_id":"tr_015d36e1ad","error":{"kind":"permission","code":"permission.denied","message":"allow_write update on ./out/a.json denied: grant allow_write ./out/** access [create] does not include `update`","retryable":false,"effects":"none","source":{"file":"app.rivet","line":18,"column":5,"end_line":18,"end_column":43},"operation_id":"notes.touch","details":{"capability":"allow_write","access":"update","target":"./out/a.json"}}}
exit=3

$ rivet request cloud.meta --file app.rivet --params '{}'
{"request_id":"req_01583ea11d","trace_id":"tr_01583ea11d","error":{"kind":"permission","code":"permission.denied","message":"allow_network connect http://169.254.169.254:80/latest denied: 169.254.169.254 is a private/loopback/link-local address; grant it literally (e.g. \"http://169.254.169.254:80/latest\") to allow it","retryable":false,"effects":"none","source":{"file":"app.rivet","line":42,"column":5,"end_line":44,"end_column":8},"operation_id":"cloud.meta","details":{"capability":"allow_network","access":"connect","target":"http://169.254.169.254:80/latest"}}}
exit=3
```

(Request IDs vary per run.)

### Bearer serve example

Tokens are stored only as SHA-256 hex. Compute the hash without putting the token in a file you commit:

```text
$ printf %s dev-token-ci | shasum -a 256
a6c6871b8f3568d985f17c7458aff2582992a5007442b589ed5af28854bb8501  -
```

```json
"serve": {
  "surfaces": ["http", "mcp"],
  "auth": {"type": "bearer", "tokens": [
    {"principal": "ci",  "sha256": "a6c6871b8f3568d985f17c7458aff2582992a5007442b589ed5af28854bb8501"},
    {"principal": "ada", "sha256": "bf7e9889975e8d9483fed456e9651d9c73470c82857bc3e6384222a1d5f7f9c6"}
  ]},
  "principals": {
    "ci":  {"operations": ["data.*", "rivet.trace.show"]},
    "ada": {"operations": ["*"]}
  }
}
```

```text
$ rivet serve --file app.rivet --listen 127.0.0.1:18437
{"listen_addr":"127.0.0.1:18437","stdio":false,"surfaces":["http","mcp"],"auth_type":"bearer","catalog_version":"sha256:59177b01e562da8b51d93d66e917fad0dc78283a749be3512e34c5a4dd3d771f","policy_hash":"sha256:fd3a186d2587b4be4a44c6455578c126847f8e163da1e5a407c2d2a26a152e5b"}

$ curl -s -X POST http://127.0.0.1:18437/v1/request -H 'content-type: application/json' -d '{"id":"data.read","params":{}}'
{"request_id":"","trace_id":"","error":{"kind":"auth","code":"auth.required","message":"missing bearer token","retryable":false,"effects":"none"}}
$ curl -s -X POST http://127.0.0.1:18437/v1/request -H 'Authorization: Bearer nope' -H 'content-type: application/json' -d '{"id":"data.read","params":{}}'
{"request_id":"","trace_id":"","error":{"kind":"auth","code":"auth.invalid","message":"invalid bearer token","retryable":false,"effects":"none"}}

$ rivet --endpoint http://127.0.0.1:18437 --token-file ci.token request data.read --params '{}'
{"request_id":"req_016bf734bd","trace_id":"tr_016bf734bd","result":"hi\n","data_count":0,"effects":"none"}
$ rivet --endpoint http://127.0.0.1:18437 --token-file ci.token request notes.save --params '{"name":"b"}'
{"request_id":"req_036be364e7","trace_id":"tr_036be364e7","error":{"kind":"permission","code":"permission.denied","message":"principal `ci` may not call `notes.save`","retryable":false,"effects":"none"}}
exit=3
$ rivet --endpoint http://127.0.0.1:18437 --token-file ci.token list
ID           NAME         DESCRIPTION
data.read    Read input   Read the public input.
data.secret  Read secret  Read a denied file.
$ rivet --endpoint http://127.0.0.1:18437 --token-file ada.token io
error[permission.denied]: principal `ada` may not call `rivet.io`
exit=3
```

Refusals at start-up:

```text
$ rivet serve --file app.rivet --listen 0.0.0.0:18437            # policy without serve.auth
{"request_id":"","trace_id":"","error":{"kind":"validation","code":"serve.auth_required","message":"non-loopback listener requires serve.auth in policy.json","retryable":false,"effects":"none"}}
exit=2
$ rivet serve --file app.rivet --policy mtls.json --listen 127.0.0.1:18437
{"request_id":"","trace_id":"","error":{"kind":"unsupported","code":"unsupported.serve_mtls","message":"serve.auth type mtls needs a TLS listener, which this build does not provide yet; use bearer behind a TLS-terminating proxy","retryable":false,"effects":"none"}}
exit=5
```

### Invalid examples (verified with `rivet check --file app.rivet`)

Every line below is the exact stderr output for a `policy.json` containing the value on the left; every
one exits 2.

| policy.json | Output |
|---|---|
| `{"version":1,` | `error[policy.invalid]: policy.json : not valid JSON: EOF while parsing a value at line 1 column 13` |
| `[1]` | `error[policy.invalid]: policy.json : the top level must be an object` |
| `{"version":2}` | ``error[policy.invalid]: policy.json /version: `version` must be 1`` |
| `{"grants":[]}` | ``error[policy.invalid]: policy.json /version: `version` must be 1`` |
| `{"version":"1"}` | ``error[policy.invalid]: policy.json /version: `version` must be 1`` |
| `{"version":1.0}` | ``error[policy.invalid]: policy.json /version: `version` must be 1`` |
| `{"version":1,"extra":1}` | ``error[policy.invalid]: policy.json /extra: unknown key `extra` (allowed: version, grants, deny, network, limits, serve, approved)`` |
| `{"version":1,"grants":{}}` | `error[policy.invalid]: policy.json /grants: must be a list` |
| `{"version":1,"grants":["x"]}` | `error[policy.invalid]: policy.json /grants/0: must be an object` |
| `{"version":1,"grants":[{"targets":["*"]}]}` | `error[policy.invalid]: policy.json /grants/0/capability: is required` |
| `{"version":1,"grants":[{"capability":5,"targets":["*"]}]}` | `error[policy.invalid]: policy.json /grants/0/capability: is required` |
| `{"version":1,"grants":[{"capability":"allow_everything","targets":["*"]}]}` | ``error[policy.invalid]: policy.json /grants/0/capability: unknown capability `allow_everything` `` |
| `{"version":1,"grants":[{"capability":"allow_read"}]}` | `error[policy.invalid]: policy.json /grants/0/targets: needs at least one target` |
| `…"targets":[]…` | `error[policy.invalid]: policy.json /grants/0/targets: needs at least one target` |
| `…"targets":"./x"…` | `error[policy.invalid]: policy.json /grants/0/targets: must be a list of strings` |
| `…"targets":[3]…` | `error[policy.invalid]: policy.json /grants/0/targets/0: must be a string` |
| `…"targets":[" "]…` | `error[policy.invalid]: policy.json /grants/0/targets/0: empty target` |
| `{"capability":"allow_network","targets":["api.example.com"]}` | ``error[policy.invalid]: policy.json /grants/0/targets/0: `api.example.com` must be a URL such as https://host:443, an IP or a CIDR`` |
| `{"capability":"allow_read","targets":["./a[b"]}` | ``error[policy.invalid]: policy.json /grants/0/targets/0: `./a[b` is not a valid selector`` |
| `…"access":["fly"]…` | ``error[policy.invalid]: policy.json /grants/0/access/0: unknown access verb `fly` `` |
| `{"capability":"allow_read",…,"access":["delete"]}` | ``error[policy.invalid]: policy.json /grants/0/access/0: `delete` does not belong to allow_read (allowed: read, list, stat, watch)`` |
| `…"access":"read"…` | `error[policy.invalid]: policy.json /grants/0/access: must be a list of strings` |
| `{"capability":"allow_read","targets":["./x"],"mode":"r"}` | ``error[policy.invalid]: policy.json /grants/0/mode: unknown key `mode` (allowed: capability, targets, access)`` |
| `{"version":1,"deny":[{"capability":"allow_read","targets":["./x"],"access":["write"]}]}` | ``error[policy.invalid]: policy.json /deny/0/access/0: `write` does not belong to allow_read (allowed: read, list, stat, watch)`` |
| `{"version":1,"network":[]}` | `error[policy.invalid]: policy.json /network: must be an object` |
| `{"version":1,"network":{"deny_private_ranges":"yes"}}` | `error[policy.invalid]: policy.json /network/deny_private_ranges: must be true or false` |
| `{"version":1,"network":{"allow_private":true}}` | ``error[policy.invalid]: policy.json /network/allow_private: unknown key `allow_private` (allowed: deny_private_ranges)`` |
| `{"version":1,"limits":5}` | `error[policy.invalid]: policy.json /limits: must be an object` |
| `{"version":1,"limits":{"max_call_depth":0}}` | `error[policy.invalid]: policy.json /limits/max_call_depth: must be a positive integer` |
| `{"version":1,"limits":{"max_concurrent_requests":-1}}` | `error[policy.invalid]: policy.json /limits/max_concurrent_requests: must be a positive integer` |
| `{"version":1,"limits":{"max_buffered_bytes":"1MiB"}}` | `error[policy.invalid]: policy.json /limits/max_buffered_bytes: must be a positive integer` |
| `{"version":1,"limits":{"timeout":5}}` | ``error[policy.invalid]: policy.json /limits/timeout: unknown key `timeout` (allowed: max_concurrent_requests, max_call_depth, max_buffered_bytes)`` |
| `{"version":1,"approved":[]}` | `error[policy.invalid]: policy.json /approved: must be an object` |
| `{"version":1,"approved":{"snapshots":"x"}}` | `error[policy.invalid]: policy.json /approved/snapshots: must be a list of strings` |
| `{"version":1,"approved":{"snapshots":[1]}}` | `error[policy.invalid]: policy.json /approved/snapshots/0: must be a string` |
| `{"version":1,"approved":{"hashes":[]}}` | ``error[policy.invalid]: policy.json /approved/hashes: unknown key `hashes` (allowed: snapshots, overlaps)`` |
| `{"version":1,"serve":[]}` | `error[policy.invalid]: policy.json /serve: must be an object` |
| `{"version":1,"serve":{"listen":"0.0.0.0:80"}}` | ``error[policy.invalid]: policy.json /serve/listen: unknown key `listen` (allowed: surfaces, auth, principals)`` |
| `{"version":1,"serve":{"surfaces":"http"}}` | `error[policy.invalid]: policy.json /serve/surfaces: must be a list of strings` |
| `{"version":1,"serve":{"surfaces":["gopher"]}}` | ``error[policy.invalid]: policy.json /serve/surfaces/0: unknown surface `gopher` (http, sse, poll, ws, mcp)`` |
| `{"version":1,"serve":{"auth":"none"}}` | `error[policy.invalid]: policy.json /serve/auth: must be an object` |
| `{"version":1,"serve":{"auth":{}}}` | `error[policy.invalid]: policy.json /serve/auth/type: must be none, bearer or mtls` |
| `{"version":1,"serve":{"auth":{"type":"basic"}}}` | `error[policy.invalid]: policy.json /serve/auth/type: must be none, bearer or mtls` |
| `{"version":1,"serve":{"auth":{"type":"none","tokens":[]}}}` | ``error[policy.invalid]: policy.json /serve/auth/tokens: unknown key `tokens` (allowed: type)`` |
| `{"version":1,"serve":{"auth":{"type":"bearer"}}}` | `error[policy.invalid]: policy.json /serve/auth/tokens: is required for bearer auth` |
| `…"tokens":[{"sha256":"ab"}]…` | ``error[policy.invalid]: policy.json /serve/auth/tokens/0: needs `principal` `` |
| `…"tokens":[{"principal":"ci","sha256":"ab"}]…` | `error[policy.invalid]: policy.json /serve/auth/tokens/0/sha256: must be 64 hex characters (the SHA-256 of the token)` |
| `…"tokens":[{"principal":"ci"}]…` | `error[policy.invalid]: policy.json /serve/auth/tokens/0/sha256: must be 64 hex characters (the SHA-256 of the token)` |
| `{"version":1,"serve":{"auth":{"type":"bearer","tokens":[],"header":"x"}}}` | ``error[policy.invalid]: policy.json /serve/auth/header: unknown key `header` (allowed: type, tokens)`` |
| `{"version":1,"serve":{"auth":{"type":"mtls"}}}` | `error[policy.invalid]: policy.json /serve/auth/client_ca: is required for mtls` |
| `…"type":"mtls","client_ca":"./ca.pem","principals":[{"principal":"ci"}]…` | ``error[policy.invalid]: policy.json /serve/auth/principals/0: needs `subject` `` |
| `…"type":"mtls","client_ca":"./ca.pem","principals":[{"subject":"CN=ci"}]…` | ``error[policy.invalid]: policy.json /serve/auth/principals/0: needs `principal` `` |
| `{"version":1,"serve":{"principals":[]}}` | `error[policy.invalid]: policy.json /serve/principals: must be an object` |
| `{"version":1,"serve":{"principals":{"ci":[]}}}` | `error[policy.invalid]: policy.json /serve/principals/ci: must be an object` |
| `{"version":1,"serve":{"principals":{"ci":{"ops":[]}}}}` | ``error[policy.invalid]: policy.json /serve/principals/ci/ops: unknown key `ops` (allowed: operations)`` |
| `{"version":1,"serve":{"principals":{"ci":{"operations":"x"}}}}` | `error[policy.invalid]: policy.json /serve/principals/ci/operations: must be a list of strings` |
| `--policy ./nope.json` | `error[policy.invalid]: --policy ./nope.json: no such file` |

`--json` does not change this line for `check`; the pointer is also in the error's `details.pointer`.

### Accepted edge cases

These validated with `ok: 5 operations, 0 connectors, 0 auth profiles` (exit 0):

| policy.json fragment | Effect |
|---|---|
| `"serve":{"principals":{"ci":{}}}` | `ci` has an empty operation list: every non-generic call is denied |
| `"serve":{"surfaces":[]}` | no surface is enabled |
| `"serve":{"auth":{"type":"bearer","tokens":[]}}` | every request is `auth.invalid` / `auth.required` |
| extra keys inside a bearer `tokens[]` entry (e.g. `"note"`) | ignored |
| `"principals":"x"` inside an `mtls` auth block | treated as an empty list |
| no `policy.json` at all | deny-by-default (`rivet policy explain` prints `policy   none — …`) |

## Data and Storage

The loader produces an in-memory `Policy` (`src/domain/policy.rs`):

```text
Policy
├── present: bool                 false = no file (deny-by-default)
├── file: "./policy.json"         as discovered / given
├── base_dir: "."                 directory of the policy file (selector anchor)
├── sha256: "sha256:<hex>"        of the raw bytes; the policy_hash in serve receipts and trace events
├── grants / deny: [Grant{capability, targets, access: Option<[AccessVerb]>}]
├── network: {deny_private_ranges}
├── limits: {max_concurrent_requests: u32, max_call_depth: u32, max_buffered_bytes: u64}
├── approved: {snapshots, overlaps}
└── serve: {surfaces, auth: None | Bearer[{principal, sha256 (lower-cased)}] | Mtls{client_ca, principals}, principals?}
```

The file is read once at load; changes need a restart of the process.

## Dependencies

- `serde_json` with `preserve_order` (document-order validation), `globset` (selector validation),
  `sha2` (file hash), `url` (run-time URL matching).
- Consumers: the broker ([SYS-2026-0003](../components/sys-2026-0003-policy-broker-and-io-manifest.md)),
  serve ([SYS-2026-0004](../components/sys-2026-0004-surfaces-and-serve.md)), OAuth grants
  ([SYS-2026-0006](../integrations/sys-2026-0006-oauth-and-credentials.md)), MCP snapshot approval
  ([SYS-2026-0009](../integrations/sys-2026-0009-mcp-client-connectors.md)).

## Deployment

```text
 bundle/
 ├── app.rivet
 ├── policy.json          ◀── auto-discovered (same directory as --file)
 └── policies/
     └── ci.json          ◀── rivet --file app.rivet --policy policies/ci.json …
                               (relative selectors in ci.json resolve against policies/)
```

Start from `rivet policy generate --output policy.json`, review, then gate with
`rivet io --check-policy` (exit 0 when every site is allowed). Keep bearer tokens out of the repository;
only their SHA-256 belongs in `policy.json`.

## Security Boundaries

- Absent file = every application effect denied; `"*"` must be written explicitly.
- Unknown keys are errors, so a typo cannot silently widen or drop a rule.
- `deny` overrides `grants`; `access` narrows a capability to specific verbs.
- `network.deny_private_ranges` defaults to `true`; turning it off removes the SSRF guard for every
  scheme.
- `serve.auth` `none` is refused on non-loopback binds; bearer tokens are stored hashed; sensitive
  built-ins need exact principal entries.
- `approved.snapshots` is the review gate for MCP connector schemas.

## Observability

- `rivet policy explain` prints the loaded file, hash, base directory, network flag, limits and every
  grant/deny line (broad `"*"` flagged); `--json` prints `{present, file, sha256, grants, deny}` counts.
- `rivet serve` prints `policy_hash` in its start receipt; trace events carry the same hash.
- Validation errors name the JSON pointer in the message and in `details.pointer`.

## Known Limitations

- `limits.max_buffered_bytes` is validated and displayed but not enforced in this version.
- `approved.overlaps` is validated but not consumed in this version.
- `max_concurrent_requests` and `max_call_depth` are truncated to 32 bits without an error:
  `{"max_concurrent_requests": 4294967297}` loads and `policy explain` shows `limits   1 concurrent, …`.
- `serve.auth` `mtls` validates but `rivet serve` refuses to start (`unsupported.serve_mtls`).
- Bearer `tokens[]` entries and mTLS `principals[]` entries are not checked for unknown keys, and a
  non-list mTLS `principals` is treated as empty.
- Only the first validation error is reported.
- The file is not reloaded while a process runs.

## Last Verified Version

0.1.0-dev (commit f40d4aa), macOS, `target/debug/rivet`. Every message above was produced by that binary
(`rivet check`, `rivet policy explain`, `rivet io --check-policy`, `rivet request`, `rivet serve` on
127.0.0.1:18437) against `docs/demos/*` and scratch bundles. Request IDs and hashes vary per run and file.

## Related Documents

- [PROP-2026-0001 Rivet runtime](../../proposals/approved/prop-2026-0001-rivet-runtime.md)
- [PLAN-2026-0001 v0.1.0 implementation and release](../../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md)
- [SYS-2026-0003 Policy broker and I/O manifest](../components/sys-2026-0003-policy-broker-and-io-manifest.md)
- [SYS-2026-0004 Surfaces and serve](../components/sys-2026-0004-surfaces-and-serve.md)
- [SYS-2026-0006 OAuth and credentials](../integrations/sys-2026-0006-oauth-and-credentials.md)
- [SYS-2026-0009 MCP client connectors](../integrations/sys-2026-0009-mcp-client-connectors.md)
- [REF-2026-0002 Language and usage](../../references/ref-2026-0002-language-and-usage.md)
- [Demos](../../demos/README.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial current-state document (PLAN-2026-0001 D-22). |
