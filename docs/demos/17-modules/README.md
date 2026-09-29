---
document_id: DEMO-2026-0019
title: "Files as modules: import, namespaces and one policy"
document_type: demo
status: active
created_date: 2026-09-29
last_updated: 2026-09-30
document_revision: 4
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, registry, audit, policy, cli, serve, library, ffi]
affected_versions:
  from: "0.2.0"
  to: null
applicable_environments: [development, embedded]
audience: [developers, reviewers, integrators]
scope: One entry bundle (app.rivet) importing two modules, one internal and one public, where the public module itself imports the internal one; `rivet list` and the HTTP and MCP catalogs; an internal call through app.rivet; `rivet io --by target`, `--include-bootstrap`, `--check-policy`, `policy generate` and `rivet graph` across files; retargeting a module global against a local server; the six import errors and the ignored module policy warning with exit codes; `cargo run --example modules` and pointers to the C and Python module objects in 15-ffi.
reason: PROP-2026-0002 UC-10 and UC-11 (UQ-09 "treat files as modules"); PLAN-2026-0002 D-71, TASK-075.
dependencies: [PROP-2026-0002, PLAN-2026-0002]
related_documents: [PROP-2026-0002, PLAN-2026-0002, DEMO-2026-0020, DEMO-2026-0016, DEMO-2026-0017, DEMO-2026-0012, MAN-2026-0003, MAN-2026-0005, MAN-2026-0007, MAN-2026-0009, API-2026-0005]
supersedes: null
superseded_by: null
tags: [rivet, demo, modules, import, namespaces, io-manifest, library]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-29
verified_against: "0.2.0"
---

# Files as modules: import, namespaces and one policy

> **Status:** Active
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-30
> **Affected Versions:** 0.2.0 and later
> **Owner:** Project maintainer
> **Affected Components:** language, registry, audit, policy, cli, serve, library, ffi

## Purpose

`import "PATH" as ALIAS [public]` splits a catalog across files. This demo shows what an author and a reviewer need to know:

1. a module's operations are addressed `ALIAS.ID`; an import is **internal** (callable from the importer, never listed) unless it is marked **`public`**;
2. a file reached twice (here `users.rivet`, from `app.rivet` and from `billing.rivet`) compiles **once**;
3. there is **one policy**: the entry bundle's. `rivet io`, `policy generate`, `policy explain` and `rivet graph` cover every file, with `file:line` sources in the module that owns each site;
4. every import mistake is refused at the `import` line with a precise code and exit status;
5. hosts can load a file at run time as a module object (Rust here; C and Python in [15-ffi](../15-ffi/README.md)).

Read [app.rivet](app.rivet), [users.rivet](users.rivet) and [billing.rivet](billing.rivet) alongside this walkthrough.

```text
  app.rivet (entry, owns policy.json)
  │  import "./users.rivet"   as users            internal ─┐
  │  import "./billing.rivet" as billing public   public    │
  │                                                         │
  │  report.user   ──call──▶ users.get                      │
  │                ──call──▶ billing.invoice                │
  │  report.remote ──call──▶ users.fetch ──http get──▶ ${api}/users/{id}.json
  │                                                         │
  ├──▶ billing.rivet ── import "./users.rivet" as people ───┤  (same file: compiled once, as users.*)
  │      invoice  ──call──▶ people.get  (= users.get)       │
  │      rates    ──file read──▶ ./rates.json               │
  └──▶ users.rivet ◀────────────────────────────────────────┘
         get · list · fetch          global greeting, api (private to this file)
```

| Operation ID | Declared in | Listed on surfaces | Effect |
|---|---|---|---|
| `report.user` | app.rivet | yes | none (calls `users.get`, `billing.invoice`) |
| `report.remote` | app.rivet | yes | through `users.fetch` |
| `billing.invoice` | billing.rivet | yes (`public`) | none (calls `people.get` = `users.get`) |
| `billing.rates` | billing.rivet | yes (`public`) | `file read ./rates.json` |
| `users.get`, `users.list` | users.rivet | no (internal) | none |
| `users.fetch` | users.rivet | no (internal) | `http get https://api.example.com/users/{id}.json` |

## Verified Against Version

0.2.0. Verified at commit `166a98b` (the source is identical to the `8031baa` release candidate; later commits touch documentation only) with `target/release/rivet` on macOS 26.4.1 (Darwin 25.4.0, arm64), 2026-09-29. The version string is bumped from 0.1.0 to 0.2.0 at release (P5), so `rivet --version` still prints `rivet 0.1.0`. Every output block below was pasted from that run. Request and trace IDs, hashes, timestamps and the scratch directory vary.

## Prerequisites

```sh
cargo build --release --workspace --all-features   # from the repository root
export PATH="$PWD/target/release:$PATH"
```

`python3` (standard library `http.server`) serves a user for step 5; loopback ports 18850 (step 3) and 18851 (step 5) must be free. Step 7 needs a Rust toolchain; the C and Python module steps live in [15-ffi](../15-ffi/README.md).

## Setup

```sh
cd docs/demos/17-modules
```

[policy.json](policy.json) is auto-discovered beside `app.rivet`. It grants exactly the two targets the manifest reports, although both sites live in modules: the origin `https://api.example.com:443` (in `users.rivet`) and the file `./rates.json` (in `billing.rivet`).

## Steps

### 1. Check and list: internal versus public

#### Command / Request

```sh
rivet --file app.rivet check --strict-docs
rivet --file app.rivet list
rivet --file app.rivet describe billing.invoice
```

#### Expected Output / Response

Seven operations compile (two in app.rivet, two in billing.rivet, three in users.rivet, which is compiled once although two files import it); four are listed (exit 0 each):

```text
ok: 7 operations, 0 connectors, 0 auth profiles
ID               NAME                 DESCRIPTION
report.user      User report          A user and their invoice, composed from both modules.
report.remote    Remote user          Fetch one user through the internal users module (network I/O in users.rivet).
billing.invoice  Invoice a user       An invoice for one user; calls people.get inside this module.
billing.rates    Read the rate table  Read the rate table (one file site in this module).
```

`describe` points at the module that declares the operation:

```text
billing.invoice — Invoice a user
An invoice for one user; calls people.get inside this module.
source   billing.rivet:7
delivery unary

params
  user   integer  required    The user id.

output  json     The invoice.
emits    —
receives —
errors   —
```

```text
  compiled (7)                          listed (4)
  ┌──────────────────────┐              ┌──────────────────┐
  │ report.user          │─────────────▶│ report.user      │
  │ report.remote        │─────────────▶│ report.remote    │
  │ billing.invoice      │── public ───▶│ billing.invoice  │
  │ billing.rates        │── public ───▶│ billing.rates    │
  │ users.get            │─┐            └──────────────────┘
  │ users.list           │ ├ internal: callable from app.rivet and billing.rivet (as people.*),
  │ users.fetch          │─┘           not listed and not requestable from outside (CLI, HTTP and MCP shown in steps 1–3)
  └──────────────────────┘
```

### 2. Request through app.rivet (an internal call)

#### Command / Request

```sh
rivet --file app.rivet request report.user --data '{"id":3}'
rivet --file app.rivet request billing.invoice --data '{"user":3}'
rivet --file app.rivet request billing.rates
rivet --file app.rivet request users.get --data '{"id":3}'
rivet --file app.rivet request report.user --data '{"id":0}'
rivet --file app.rivet request report.remote --data '{"id":3}'
```

#### Expected Output / Response

`report.user` reaches the internal `users.get` directly and again through `billing.invoice` (which calls it as `people.get`); the `greeting` and `rate` globals stay inside their modules (exit 0):

```json
{"request_id":"req_015cd94075","trace_id":"tr_015cd94075","operation":"report.user","type":"result","status":"ok","data":{"user":{"id":3,"name":"Hello, user 3"},"invoice":{"user":{"id":3,"name":"Hello, user 3"},"amount":20.0}},"error":null,"effects":"none","data_count":0}
{"request_id":"req_015bcebc95","trace_id":"tr_015bcebc95","operation":"billing.invoice","type":"result","status":"ok","data":{"user":{"id":3,"name":"Hello, user 3"},"amount":20.0},"error":null,"effects":"none","data_count":0}
{"request_id":"req_0141d1b18d","trace_id":"tr_0141d1b18d","operation":"billing.rates","type":"result","status":"ok","data":{"standard":0.2,"reduced":0.05},"error":null,"effects":"none","data_count":0}
```

The failures. The internal ID cannot be requested from outside (`not_found.operation`, exit 4); validation runs before the call (`validation.min`, exit 2); and `report.remote` reaches the network site in `users.rivet`, whose placeholder host does not resolve here (`dns.resolve`, exit 5; step 5 points it at a local server). The error's `source` is the module file and line, and `operation_id` names the internal operation that failed:

```json
{"request_id":"req_015a3cd625","trace_id":"tr_015a3cd625","operation":"users.get","type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"not_found.operation","message":"no operation `users.get`","retryable":false,"operation_id":"users.get"},"effects":"none","data_count":0}
{"request_id":"req_0159141e95","trace_id":"tr_0159141e95","operation":"report.user","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.min","message":"parameter `id` must be ≥ 1","retryable":false,"operation_id":"report.user"},"effects":"none","data_count":0}
{"request_id":"req_01af15f32d","trace_id":"tr_01af15f32d","operation":"report.remote","type":"result","status":"error","data":null,"error":{"kind":"dns","code":"dns.resolve","message":"cannot resolve api.example.com: failed to lookup address information: nodename nor servname provided, or not known","retryable":false,"source":{"file":"users.rivet","line":27,"column":5,"end_line":29,"end_column":8},"operation_id":"users.fetch"},"effects":"none","data_count":0}
```

### 3. The same catalog on HTTP and MCP

#### Command / Request

```sh
rivet --file app.rivet serve --listen 127.0.0.1:18850 2>serve.err & SV=$!
curl -sS -w ' %{http_code}\n' http://127.0.0.1:18850/v1/request -H 'Content-Type: application/json' -d '{"operation":"billing.invoice","data":{"user":3}}'
curl -sS -w ' %{http_code}\n' http://127.0.0.1:18850/v1/request -H 'Content-Type: application/json' -d '{"operation":"users.get","data":{"id":3}}'
curl -sS http://127.0.0.1:18850/v1/operations | python3 -c 'import json,sys; print([o["id"] for o in json.load(sys.stdin)["data"]["operations"]])'
H=$(curl -si http://127.0.0.1:18850/mcp -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' \
      --data-binary @../01-catalog/requests/initialize.mcp.json | grep -i mcp-session-id | tr -d '\r' | cut -d' ' -f2)
curl -sS http://127.0.0.1:18850/mcp -H "mcp-session-id: $H" -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' \
      --data-binary @../01-catalog/requests/initialized.mcp.json -o /dev/null -w '%{http_code}\n'
curl -sS http://127.0.0.1:18850/mcp -H "mcp-session-id: $H" -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' \
      -d '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
  | python3 -c 'import json,sys; t=sys.stdin.read(); d=json.loads(t[t.find("{"):]); print([x["name"] for x in d["result"]["tools"] if not x["name"].startswith("rivet.")])'
kill $SV; rm -f serve.err
```

#### Expected Output / Response

HTTP 200 for the public module operation and 404 for the internal one; the REST catalog and the MCP direct tools list the same four IDs (the `rivet.*` built-ins are filtered out above):

```text
{"request_id":"req_0103e91955","trace_id":"tr_0103e91955","operation":"billing.invoice","type":"result","status":"ok","data":{"user":{"id":3,"name":"Hello, user 3"},"amount":20.0},"error":null,"effects":"none","data_count":0} 200
{"request_id":"req_0280effb8a","trace_id":"tr_0280effb8a","operation":"users.get","type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"not_found.operation","message":"no operation `users.get`","retryable":false,"operation_id":"users.get"},"effects":"none","data_count":0} 404
['report.user', 'report.remote', 'billing.invoice', 'billing.rates']
202
['report.user', 'report.remote', 'billing.invoice', 'billing.rates']
```

`serve.err` starts with the startup line (`"surfaces":["http","sse","poll","ws","mcp"]`, `"auth_type":"none"`, one `catalog_version` and one `policy_hash` for the whole bundle) followed by one access-log line per request.

### 4. One manifest, one graph, one policy across files

#### Command / Request

```sh
rivet --file app.rivet io
rivet --file app.rivet io --by target
rivet --file app.rivet io --include-bootstrap | sed -n '/BOOTSTRAP/,$p'
rivet --file app.rivet io --check-policy
rivet --file app.rivet policy generate
rivet --file app.rivet graph report.user
rivet --file app.rivet graph report.remote
rivet --file app.rivet graph users.get
rivet --file app.rivet graph users.get --all
```

#### Expected Output / Response

`io` walks every file. Each site's `SOURCE` is the module that owns it; calls into modules are listed where they are made (exit 0):

```text
OPERATION        KIND     ACCESS       TARGET                                   KNOWLEDGE        SOURCE
billing.invoice  (calls users.get — no I/O)                                                      billing.rivet:12
billing.rates    file     read         ./rates.json                             exact            billing.rivet:20
report.remote    (calls users.fetch — see above)                                                 app.rivet:22
report.user      (calls users.get — no I/O)                                                      app.rivet:12
report.user      (calls billing.invoice — no I/O)                                                app.rivet:13
users.fetch      network  connect GET  https://api.example.com/users/{id}.json  param_dependent  users.rivet:27
```

`billing.invoice` calls `people.get`, but the manifest shows `users.get`: `users.rivet` was compiled once, under the shallowest namespace. `--by target` groups the same sites by what a policy must grant, and marks internal operations `(private)`:

```text
TARGET                       ACCESS       CAPABILITY     ORIGIN     PHASE    NEEDS FILE  USED BY
./rates.json                 read         allow_read     file read  body     yes         billing.rates
https://api.example.com:443  connect GET  allow_network  http get   connect  —           report.remote (via users.fetch), users.fetch (private)
```

The bootstrap reads name the entry file and every module:

```text
BOOTSTRAP (runtime-internal; listed, not governed by policy.json)
KIND  ACCESS       TARGET
file  read         ./app.rivet
file  read         ./users.rivet
file  read         ./billing.rivet
file  read         ./policy.json
file  read         system CA bundle
file  read         /etc/resolv.conf / system resolver
file  read         tzdata
file  read         descriptor/schema files named by connectors (none here)
pipe  read, write  stdin, stdout, stderr
```

The single `policy.json` beside app.rivet covers both module sites (exit 0), and `policy generate` drafts the same two grants from all three files:

```text
OPERATION        KIND     ACCESS       TARGET                                   KNOWLEDGE        SOURCE            DECISION
billing.invoice  (calls users.get — no I/O)                                                      billing.rivet:12
billing.rates    file     read         ./rates.json                             exact            billing.rivet:20  allowed
report.remote    (calls users.fetch — see above)                                                 app.rivet:22
report.user      (calls users.get — no I/O)                                                      app.rivet:12
report.user      (calls billing.invoice — no I/O)                                                app.rivet:13
users.fetch      network  connect GET  https://api.example.com/users/{id}.json  param_dependent  users.rivet:27    allowed
2 allowed
```

```text
{
  "version": 1,
  "grants": [
    {"capability": "allow_read", "targets": ["./rates.json"], "access": ["read"]},
    {"capability": "allow_network", "targets": ["https://api.example.com:443"]}
  ],
  "network": {"deny_private_ranges": true}
}
policy generate: 2 grants, 0 review items
```

`graph` expands calls across files, with the `file:line` of each call and site:

```text
report.user               app.rivet:7
├── call users.get        app.rivet:12
└── call billing.invoice  app.rivet:13
    └── call users.get    billing.rivet:12
report.remote                                                          app.rivet:17
└── call users.fetch                                                   app.rivet:22
    └── network  connect GET  https://api.example.com/users/{id}.json  users.rivet:27
```

An internal ID is refused like a private operation (exit 4) and shown with `--all` (exit 0):

```text
error[not_found.operation]: no operation `users.get`
users.get  users.rivet:7
```

### 5. Retarget a module by editing its global

A module's globals are private to it, so the origin lives in `users.rivet`, not in app.rivet. In a scratch copy, point it at a local server. Only `users.rivet` and the grant change.

#### Command / Request

```sh
WORK="$(mktemp -d)"; cp app.rivet users.rivet billing.rivet rates.json policy.json "$WORK/"; cd "$WORK"
sed -i.bak 's#^global api      = "https://api.example.com"#global api      = "http://127.0.0.1:18851"#' users.rivet
sed -i.bak 's#https://api.example.com:443#http://127.0.0.1:18851#' policy.json
diff users.rivet.bak users.rivet
mkdir -p www/users && printf '{"id":3,"name":"Ada"}\n' > www/users/3.json
(cd www && exec python3 -m http.server 18851 --bind 127.0.0.1 > ../http.log 2>&1) & HS=$!
rivet --file app.rivet io --check-policy | tail -2
rivet --file app.rivet request report.remote --data '{"id":3}'
rivet --file app.rivet request report.remote --data '{"id":4}'
grep GET http.log
```

#### Expected Output / Response

```text
5c5
< global api      = "https://api.example.com"
---
> global api      = "http://127.0.0.1:18851"
users.fetch      network  connect GET  http://127.0.0.1:18851/users/{id}.json  param_dependent  users.rivet:27    allowed
2 allowed
```

User 3 exists (exit 0); user 4 is `http.status` 404, reported at the module's line (exit 5):

```json
{"request_id":"req_01660d1585","trace_id":"tr_01660d1585","operation":"report.remote","type":"result","status":"ok","data":{"id":3,"name":"Ada"},"error":null,"effects":"none","data_count":0}
{"request_id":"req_0165d7c25d","trace_id":"tr_0165d7c25d","operation":"report.remote","type":"result","status":"error","data":null,"error":{"kind":"http","code":"http.status","message":"GET http://127.0.0.1:18851/users/4.json returned 404","retryable":false,"source":{"file":"users.rivet","line":27,"column":5,"end_line":29,"end_column":8},"operation_id":"users.fetch","details":{"status":404,"method":"GET"}},"effects":"none","data_count":0}
```

```text
127.0.0.1 - - [29/Sep/2026 07:06:46] "GET /users/3.json HTTP/1.1" 200 -
127.0.0.1 - - [29/Sep/2026 07:06:46] "GET /users/4.json HTTP/1.1" 404 -
```

### 6. Failing examples: the import errors

Each file in [errors/](errors/helper.rivet) has one mistake. They are refused by `rivet check` and, with the same diagnostic, by every command that loads the bundle (`request`, `serve`, `io`, …), at the `import` line.

```text
  errors/cycle.rivet ──▶ cycle_b.rivet ──▶ cycle.rivet         check.import_cycle               exit 2
  errors/missing.rivet ──▶ ./nowhere.rivet (absent)            not_found.import                 exit 4
  errors/outside.rivet ──▶ ../users.rivet (above the root)     permission.import_outside_root   exit 3
  errors/duplicate.rivet: alias h twice                        check.import_duplicate           exit 2
  errors/collision.rivet: local h.get vs imported h.get        check.import_collision           exit 2
  errors/late.rivet: import after `global`                     syntax.import                    exit 2
  errors/module_policy.rivet ──▶ sub/m.rivet + sub/policy.json check.module_policy_ignored      warning, exit 0
```

#### Command / Request

```sh
cd errors
for f in cycle missing outside duplicate collision late module_policy; do rivet --file $f.rivet check; echo "exit $?"; done
rivet --file duplicate.rivet check --json
rivet --file module_policy.rivet request top
cd ..
```

With `--file errors/X.rivet` the runtime root is `errors/`, so `outside.rivet`'s `../users.rivet` leaves it.

#### Expected Output / Response

```text
error[check.import_cycle]: import cycle: cycle.rivet → cycle_b.rivet → cycle.rivet
  --> cycle.rivet:2:1
   |
  2| import "./cycle_b.rivet" as b
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
exit 2
error[not_found.import]: cannot read nowhere.rivet: No such file or directory (os error 2)
  --> missing.rivet:2:1
   |
  2| import "./nowhere.rivet" as gone
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
exit 4
error[permission.import_outside_root]: import `../users.rivet` resolves outside the runtime root .
  --> outside.rivet:2:1
   |
  2| import "../users.rivet" as users
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
exit 3
error[check.import_duplicate]: alias `h` is imported twice in duplicate.rivet
  --> duplicate.rivet:3:1
   |
  3| import "./sub/m.rivet" as h
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^
exit 2
error[check.import_collision]: `h.get` from helper.rivet collides with the operation declared at collision.rivet:4
  --> collision.rivet:2:1
   |
  2| import "./helper.rivet" as h
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
exit 2
error[syntax.import]: `import` must come before the first declaration of the file
  --> late.rivet:3:1
   |
  3| import "./helper.rivet" as h
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
exit 2
warning[check.module_policy_ignored]: the policy.json beside sub/m.rivet is ignored: module `m` runs under the loader's policy
  --> module_policy.rivet:2:1
   |
  2| import "./sub/m.rivet" as m
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^
  = hint: grant what the module needs in the entry bundle's policy.json (or the host policy)
ok: 2 operations, 0 connectors, 0 auth profiles
exit 0
```

As JSON the error is a `rivet.check` envelope. Every `check.*` code has kind `syntax` in the registry (HTTP 422, exit 2), and a load-time failure has no request, so the IDs are empty:

```json
{"request_id":"","trace_id":"","operation":"rivet.check","type":"result","status":"error","data":null,"error":{"kind":"syntax","code":"check.import_duplicate","message":"alias `h` is imported twice in duplicate.rivet","retryable":false,"source":{"file":"duplicate.rivet","line":3,"column":1,"end_line":3,"end_column":28}},"effects":"none","data_count":0}
```

The module whose policy is ignored still runs, under the entry bundle's policy (none here: deny-by-default, which a pure module never needs). `request` (and `serve`) print the same warning on stderr once at load, then the result:

```text
warning[check.module_policy_ignored]: the policy.json beside sub/m.rivet is ignored: module `m` runs under the loader's policy
  --> module_policy.rivet:2:1
   |
  2| import "./sub/m.rivet" as m
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^
  = hint: grant what the module needs in the entry bundle's policy.json (or the host policy)
{"request_id":"req_010e7fe20d","trace_id":"tr_010e7fe20d","operation":"top","type":"result","status":"ok","data":{"pong":true},"error":null,"effects":"none","data_count":0}
```

`rivet --file module_policy.rivet serve --listen 127.0.0.1:18852` prints the same warning on stderr once, before its startup record, and then serves `top` normally (HTTP 200 `{"pong":true}`).

| Code | Kind | Exit | HTTP | Cause |
|---|---|---|---|---|
| `check.import_cycle` | syntax | 2 | 422 | files import each other |
| `not_found.import` | not_found | 4 | 404 | the imported file does not exist |
| `permission.import_outside_root` | permission | 3 | 403 | the path (or a symlink) leaves the runtime root |
| `check.import_duplicate` | syntax | 2 | 422 | an alias imported twice; `rt.load` of an alias already loaded |
| `check.import_collision` | syntax | 2 | 422 | a namespaced ID, connector or auth profile name clashes across files |
| `syntax.import` | syntax | 2 | 422 | a malformed import, or one after the first declaration |
| `limit.imports` | limit | 5 | 429 | more than 256 files or depth over 16 (not reproduced here) |
| `check.module_policy_ignored` | warning | 0 | — | a `policy.json` beside a module |

Kinds, HTTP statuses and exits are from the [error registry](../../api/api-2026-0005-error-registry.md); the exits were observed above.

### 7. Modules as host objects (Rust, C, Python)

A host can load files into a running runtime without an entry file. [examples/modules.rs](../../../examples/modules.rs) loads the same shape of modules from `examples/modules/`.

#### Command / Request

```sh
cd ../../..                                  # repository root
cargo run --release --example modules
```

#### Expected Output / Response

```text
get — One user by id.
list — The first two users (calls inside a module use its own IDs).
{
  "request_id": "req_01e8804c8d",
  "trace_id": "tr_01e8804c8d",
  "operation": "users.get",
  "type": "result",
  "status": "ok",
  "data": {
    "id": 42,
    "name": "Hello, user 42"
  },
  "error": null,
  "effects": "none",
  "data_count": 0
}
{
  "request_id": "req_026690bf9a",
  "trace_id": "tr_026690bf9a",
  "operation": "billing.invoice",
  "type": "result",
  "status": "ok",
  "data": {
    "user": {
      "id": 7,
      "name": "Hello, user 7"
    },
    "amount": 20.0
  },
  "error": null,
  "effects": "none",
  "data_count": 0
}
{"request_id":"req_03e5d461df","trace_id":"tr_03e5d461df","operation":"users.list","type":"result","status":"ok","data":[{"id":1,"name":"Hello, user 1"},{"id":2,"name":"Hello, user 2"}],"error":null,"effects":"none","data_count":0}
check.import_duplicate (syntax): a module named `users` is already loaded; use load_as(path, alias)
```

```text
 Runtime::builder().root("examples/modules").build()      no entry file, no policy.json (deny-by-default)
   ├─ rt.load("./users.rivet")                    ─▶ Module "users"   ─▶ users.call("get", {"id":42})
   ├─ rt.load_as("./lib/billing.rivet","billing") ─▶ Module "billing" ─▶ billing.call("invoice", {"user":7})
   ├─ rt.call(InputEnvelope::new("users.list"))   ─▶ the runtime dispatcher sees the namespaced IDs
   └─ rt.load("./users.rivet") again              ─▶ Err(check.import_duplicate); the catalog is unchanged
```

The same module objects from C (`rivet_load`, `rivet_module_call`, `rivet_module_call_start`, `rivet_module_free`) and from the Python ctypes wrapper (`rt.load(...)`, one attribute per operation) are steps 3 and 4 of [15-ffi](../15-ffi/README.md). Both load `examples/modules/` and print the same `users.get` and `billing.invoice` envelopes and the same `check.import_duplicate` refusal.

## Release Updates

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-19 | UQ-09 / R19 | `import "PATH" as ALIAS [public]`, resolved relative to the importing file and confined to the runtime root; bootstrap reads name every file | Steps 1, 4 | `ok: 7 operations`; bootstrap lists `./app.rivet`, `./users.rivet`, `./billing.rivet` | This README steps 1, 4 (2026-09-29, 166a98b); `tests/conformance_modules.rs` (T-16) |
| U-20 | UQ-09 / R20 | `ALIAS.ID` namespaces; internal by default, `public` exposes; a file imported twice compiles once | Steps 1–3 | Four IDs listed on CLI, REST and MCP; `users.get` 404 / exit 4 | This README steps 1–3; T-16 |
| U-21 | UQ-09 / R21 | `syntax.import`, `not_found.import`, `permission.import_outside_root`, `check.import_cycle`, `check.import_duplicate`, `check.import_collision` (`limit.imports` in tests only) | Step 6 | Exits 2, 4, 3, 2, 2, 2 at the `import` line | This README step 6; T-17 |
| U-22 | UQ-09 / R22 | `Runtime::builder().root(…)`, `rt.load` / `load_as` → `Module`, `Module::call` | Step 7 | `users.get` and `billing.invoice` envelopes; duplicate alias refused | This README step 7; `examples/modules.rs`; T-18 |
| U-23 | UQ-09 / R23 | `rivet_load`, `rivet_module_*`, Python module object | [15-ffi](../15-ffi/README.md) steps 3–4 | Same envelopes from C and Python | 15-ffi Verification Record; T-19 |
| U-24 | UQ-09 / R24 | One policy (the loader's); a module's `policy.json` is ignored with a warning; `io`, `graph`, `policy generate`, `policy explain` cover every file | Steps 4–6 | 2 allowed across two modules; draft with 2 grants; `check.module_policy_ignored` | This README steps 4–6; T-20 |

## Cleanup

```sh
kill $SV $HS 2>/dev/null           # steps 3 and 5, if still running
cd / && rm -rf "$WORK"             # the step 5 scratch copy
```

Nothing is written to this folder (step 3's `serve.err` is removed by its last command).

## Verification Record

| Step | Verified By | Verified At | Result |
|---|---|---|---|
| 1. `check --strict-docs`, `list`, `describe` | Claude (TASK-075) | 2026-09-29, commit 166a98b, macOS 26.4.1 arm64 | PASS |
| 2. `report.user`, `billing.invoice`, `billing.rates`; `users.get` exit 4; `validation.min` exit 2; `dns.resolve` exit 5 | Claude (TASK-075) | 2026-09-29, commit 166a98b, macOS 26.4.1 arm64 | PASS |
| 3. `serve`: REST 200/404, `/v1/operations`, MCP `tools/list` | Claude (TASK-075) | 2026-09-29, commit 166a98b, macOS 26.4.1 arm64 | PASS |
| 4. `io`, `--by target`, `--include-bootstrap`, `--check-policy`, `policy generate`, `graph` (and `--all`) | Claude (TASK-075) | 2026-09-29, commit 166a98b, macOS 26.4.1 arm64 | PASS |
| 5. Module global retargeted to a local http.server; 200 and 404 | Claude (TASK-075) | 2026-09-29, commit 166a98b, macOS 26.4.1 arm64 | PASS |
| 6. Six import errors with their exits; `--json` envelope; ignored module policy warning | Claude (TASK-075) | 2026-09-29, commit 166a98b, macOS 26.4.1 arm64 | PASS |
| 7. `cargo run --release --example modules` | Claude (TASK-075) | 2026-09-29, commit 166a98b, macOS 26.4.1 arm64 | PASS |
| C and Python module objects | Claude (coordinator), see [15-ffi](../15-ffi/README.md) | 2026-09-29, commit 166a98b, macOS 26.4.1 arm64 | PASS |
| 6. (re-run after INC-2026-0012) six import errors, identical diagnostics and exits (2, 4, 3, 2, 2, 2) and the warning with `check` (exit 0); `check --json` envelope (exit 2, empty IDs: `check` is not a request); `request top` prints `check.module_policy_ignored` then the result (exit 0); `serve` prints it once at load (port 18852) | Claude | 2026-09-29, commit 7c25175, macOS 26.4.1 arm64 | PASS |
| 7. (re-run after INC-2026-0012) `cargo run --release --example modules`: same output except IDs; duplicate load `check.import_duplicate (syntax)` | Claude | 2026-09-29, commit 7c25175, macOS 26.4.1 arm64 | PASS |
| Known Caveats: `policy explain report.remote --data '{"id":3}'` fills `users.fetch`'s target `…/users/3.json` `exact` (exit 0); `users.fetch --data` same; `--params` alias same | Claude | 2026-09-29, commit 7c25175, macOS 26.4.1 arm64 | PASS |

## Known Caveats

- `check.import_duplicate` from a host load (`rt.load`, `rivet_load`) keeps the registry kind `syntax` (exit 2, as `rivet check` reports it) and, from INC-2026-0012, carries request and trace IDs like every runtime envelope (step 7, 15-ffi step 3).
- `policy explain` takes `--data` (alias `--params`). Params flow through a call into a module: `policy explain report.remote --data '{"id":3}'` fills the callee's target, like `policy explain users.fetch --data '{"id":3}'`:

  ```text
  $ rivet --file app.rivet policy explain report.remote --data '{"id":3}'                          [exit 0]
  …
  OPERATION      KIND     ACCESS       TARGET                                KNOWLEDGE  SOURCE          DECISION
  report.remote  (calls users.fetch — see above)                                        app.rivet:22
  users.fetch    network  connect GET  https://api.example.com/users/3.json  exact      users.rivet:27  allowed
  ```
- An error raised inside a module names the module's internal operation in `error.operation_id` (`users.fetch`) while the envelope's `operation` is the requested one (`report.remote`).
- `limit.imports` (more than 256 files or depth over 16) is covered by `tests/conformance_modules.rs`, not reproduced here.
- Recorded on the release candidate before the version bump; the tagged `v0.2.0` build prints `rivet 0.2.0`.

## Related Documents

- [All sample folders](../README.md) · [demos index](../index.md) · [v0.2.0 release verification guide](../demo-2026-0020-v0-2-0-release-verification.md)
- [Language guide MAN-2026-0003](../../manuals/man-2026-0003-language-guide.md) · [Policy and I/O manifest guide MAN-2026-0005](../../manuals/man-2026-0005-policy-and-io-manifest-guide.md)
- [Embedding library MAN-2026-0007](../../manuals/man-2026-0007-embedding-library.md) · [C ABI and FFI MAN-2026-0009](../../manuals/man-2026-0009-c-abi-and-ffi.md) · [Error registry API-2026-0005](../../api/api-2026-0005-error-registry.md)
- [Globals demo DEMO-2026-0016](../14-globals/README.md) · [FFI demo DEMO-2026-0017](../15-ffi/README.md) · [Library demo DEMO-2026-0012](../12-library/README.md)
- [Proposal PROP-2026-0002](../../proposals/implemented/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md) · [Plan PLAN-2026-0002](../../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 4 | 2026-09-30 | Claude | v0.2.0 release (PLAN-2026-0002 TASK-091): version strings and current-release wording updated to 0.2.0. |
| 1 | 2026-09-29 | Claude | TASK-075 (PLAN-2026-0002 D-71): new demo; entry bundle with an internal and a public import (the public module imports the internal one), list/request/serve across files, `io`, `--by target`, bootstrap, `policy generate`, `graph`, module-global retarget, six import errors and the ignored module policy warning, `examples/modules.rs`; executed against 0.2.0-dev (166a98b, source = 8031baa). |
| 2 | 2026-09-29 | Claude | INC-2026-0012: `request` prints `check.module_policy_ignored` (re-captured); `policy explain --data` with params followed into `users.fetch` (captured); load refusals carry IDs; the `conformance_samples` caveat removed (test fixed). |
| 3 | 2026-09-29 | Claude | INC-2026-0012 re-verification (T-30) at 7c25175: step 6 (import errors, `--json`, `request` warning), `serve` printing the warning at load, step 7 (`examples/modules.rs`) and the `policy explain --data` caveat re-run; new IDs pasted; three Verification Record rows. |
