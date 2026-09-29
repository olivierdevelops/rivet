---
document_id: DEMO-2026-0011
title: "Deny-by-default sandbox and I/O inventory"
document_type: demo
status: active
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 7
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [policy, audit, files, cli, serve, http]
affected_versions:
  from: "0.2.0"
  to: null
applicable_environments: [development]
audience: [developers, reviewers]
scope: Runnable policy demo — one bundle under four policy files (deny-by-default, read-only, access-narrowed, default with a deny), every I/O manifest view, the files each operation needs, brokered `--check-files`, `policy generate` compared with the hand-written policy, and planned-versus-actual trace attempts through one host.
reason: User requested sample files in folders with READMEs showing usage; UQ-13/17 ask for deny-by-default policy from policy.json only; UQ-18 adds the generated I/O manifest and policy generate; TASK-067 executed every step against the 0.1.0 release candidate. TASK-076 (PLAN-2026-0002) re-executed it against the 0.2.0 release candidate.
dependencies: [PROP-2026-0001, REF-2026-0002]
related_documents: [PLAN-2026-0001, DEMO-2026-0015, DEMO-2026-0013, MAN-2026-0005, TEST-2026-0008, TEST-2026-0021, TEST-2026-0025, TEST-2026-0026, PLAN-2026-0002, DEMO-2026-0020, MIG-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, demo, policy, sandbox, io-manifest, policy-generate]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-28
verified_against: "0.2.0"
---

# Deny-by-default sandbox and I/O inventory

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0 and later
> **Owner:** Project maintainer
> **Affected Components:** policy, audit, files, cli, serve, http

## Purpose

Deny-by-default policy and I/O inventory. Delivery stage: **A**. Read [app.rivet](app.rivet) alongside this walkthrough.

| Operation ID | Output | Behavior |
|---|---|---|
| `demo.echo` | `text` | Pure work succeeds with no I/O grants. |
| `data.read` | `object {message}` | Read a granted file. |
| `data.private` | `object {message}` | Intentional deny case; declares `permission.denied`. |
| `data.snapshot` | `object {message}` | Transitive read (via `data.read`) plus exclusive create; declares `conflict.already_exists`. |

Four policy files show the same bundle under different authority. Only the file path is chosen on the command line; grants live in the files.

```text
                            demo.echo   data.read   data.private   data.snapshot
 policies/empty.json          ok        DENIED       DENIED          DENIED     ({"version":1} = deny-by-default)
 policies/read-only.json      ok        ok           DENIED          DENIED (no write)
 policies/create-only.json    ok        ok           DENIED          ok, then conflict
                                                     (no grant)      (create only; never overwrites)
 policy.json (default)        ok        ok           DENIED (deny    ok, then conflict
                                                     overrides)      on repeat
```

## Verified Against Version

0.2.0. Verified on 0.2.0-dev at commit `8031baa`, the release candidate (the version string is bumped to 0.2.0 at release, P5), with `target/release/rivet` on macOS 26.4.1 (Darwin 25.4.0, arm64), 2026-09-29. Since 0.2.0 results, errors and every `--json`/`--format json` output are ResponseEnvelopes, and `request` takes `--data` ([migration guide](../../migrations/mig-2026-0001-response-and-input-envelopes.md)). The tables, policy decisions and exit codes are unchanged; `policy generate` without `--json` still prints the bare draft so `> policy.json` keeps working. Every output block below was pasted from that run. Request and trace IDs and hashes vary.

## Prerequisites

```sh
cargo build --release --features cli       # from the repository root
export PATH="$PWD/target/release:$PATH"     # the release candidate prints rivet 0.1.0 until the P5 bump
```

`curl` for step 8; loopback port 18811 free.

## Setup

```sh
cd docs/demos/11-sandbox
mkdir -p out
```

- [policy.json](policy.json) is auto-discovered: read `./data/**`, write `./out/**`, **deny** read `./data/private/**`, and a `serve` block mounting only `http`.
- [policies/empty.json](policies/empty.json) is `{"version": 1}`: no grants, identical to having no file at all.
- [policies/read-only.json](policies/read-only.json) grants reads only. Its targets use `../data/**`, because targets are relative to the policy file's own directory.
- [policies/create-only.json](policies/create-only.json) narrows with `access`: read only `../data/public.json` (`["read"]`) and only create under `../out/**` (`["create"]`).

The private file is synthetic test data, not a secret.

## Steps

### 1. Check the bundle and view outputs

#### Command / Request

```sh
rivet --file app.rivet check --strict-docs
rivet --file app.rivet outputs data.private
```

#### Expected Output / Response

```text
ok: 4 operations, 0 connectors, 0 auth profiles
```

```text
data.private — Try a denied read
output  object   Content of data/private/secret.json; never returned under the shipped policy.
  message  text     required  Synthetic fixture message.
emits    —
receives —
errors
  permission.denied   policy.json denies ./data/private/**; deny overrides grants.
```

Both exit 0. `outputs --all --json` prints a `rivet.outputs` envelope whose `data` lists four entries sorted by ID (`data.private`, `data.read`, `data.snapshot`, `demo.echo`).

### 2. Run under four policy files

#### Command / Request

```sh
rivet --file app.rivet --policy ./policies/empty.json request demo.echo --data '{"value":"ok"}'
rivet --file app.rivet --policy ./policies/empty.json request data.read
rivet --file app.rivet request data.read
rivet --file app.rivet request data.private
rivet --file app.rivet --policy ./policies/read-only.json request data.snapshot
rivet --file app.rivet request data.snapshot
rivet --file app.rivet request data.snapshot
```

#### Expected Output / Response

Pure work needs no grant (exit 0); a read under the empty policy is denied before the file is touched (exit 3):

```json
{"request_id":"req_01194a6be5","trace_id":"tr_01194a6be5","operation":"demo.echo","type":"result","status":"ok","data":"ok","error":null,"effects":"none","data_count":0}
{"request_id":"req_01181da81d","trace_id":"tr_01181da81d","operation":"data.read","type":"result","status":"error","data":null,"error":{"kind":"permission","code":"permission.denied","message":"allow_read read on ./data/public.json denied: no grant for allow_read ./data/public.json","retryable":false,"source":{"file":"app.rivet","line":15,"column":5,"end_line":15,"end_column":51},"operation_id":"data.read","details":{"capability":"allow_read","access":"read","target":"./data/public.json"}},"effects":"none","data_count":0}
```

Under policy.json the public read succeeds (exit 0) and the deny entry overrides the `./data/**` grant (exit 3):

```json
{"request_id":"req_01172a3305","trace_id":"tr_01172a3305","operation":"data.read","type":"result","status":"ok","data":{"message":"public demo data"},"error":null,"effects":"none","data_count":0}
{"request_id":"req_0116f417ed","trace_id":"tr_0116f417ed","operation":"data.private","type":"result","status":"error","data":null,"error":{"kind":"permission","code":"permission.denied","message":"allow_read read on ./data/private/secret.json denied: deny allow_read ./data/private/**","retryable":false,"source":{"file":"app.rivet","line":26,"column":5,"end_line":26,"end_column":59},"operation_id":"data.private","details":{"capability":"allow_read","access":"read","target":"./data/private/secret.json"}},"effects":"none","data_count":0}
```

Read-only has no write grant (exit 3):

```json
{"request_id":"req_0115ac7bd5","trace_id":"tr_0115ac7bd5","operation":"data.snapshot","type":"result","status":"error","data":null,"error":{"kind":"permission","code":"permission.denied","message":"allow_write create on ./out/snapshot.json denied: no grant for allow_write ./out/snapshot.json","retryable":false,"source":{"file":"app.rivet","line":38,"column":5,"end_line":38,"end_column":49},"operation_id":"data.snapshot","details":{"capability":"allow_write","access":"create","target":"./out/snapshot.json"}},"effects":"none","data_count":0}
```

The first snapshot writes `out/snapshot.json` (exit 0); the repeat never overwrites (exit 4):

```json
{"request_id":"req_0114acdb05","trace_id":"tr_0114acdb05","operation":"data.snapshot","type":"result","status":"ok","data":{"message":"public demo data"},"error":null,"effects":"committed","data_count":0}
{"request_id":"req_01139e2ec5","trace_id":"tr_01139e2ec5","operation":"data.snapshot","type":"result","status":"error","data":null,"error":{"kind":"conflict","code":"conflict.already_exists","message":"./out/snapshot.json already exists","retryable":false,"source":{"file":"app.rivet","line":38,"column":5,"end_line":38,"end_column":49},"operation_id":"data.snapshot"},"effects":"none","data_count":0}
```

Under create-only.json (after `rm out/snapshot.json`), `data.snapshot` succeeds then conflicts the same way, `data.read` succeeds, and `data.private` is denied with `no grant for allow_read ./data/private/secret.json`.

### 3. Explain one call

#### Command / Request

```sh
rivet --file app.rivet --policy ./policies/read-only.json policy explain data.snapshot --data '{}'
```

#### Expected Output / Response

The transitive read through `data.read` is allowed; the missing write grant is named (exit 3):

```text
policy   ./policies/read-only.json (sha256:32dccbdd428b22dcd654d7abfcaeb2b06949e066d8bdf2b1258cf77431489a8c)
base     ./policies
network  deny_private_ranges true
limits   64 concurrent, depth 16, 268435456 buffered bytes
grant    allow_read ../data/**
deny     allow_read ../data/private/**

OPERATION      KIND  ACCESS  TARGET               KNOWLEDGE  SOURCE        DECISION
data.read      file  read    ./data/public.json   exact      app.rivet:15  allowed
data.snapshot  (calls data.read — see above)                 app.rivet:37
data.snapshot  file  create  ./out/snapshot.json  exact      app.rivet:38  denied
denied: data.snapshot#1 allow_write ./out/snapshot.json (create)
```

`--data` takes the call's parameters; `--params` is an alias and prints no warning. With `--json` a denial is an error, like every other failing `--json` command. stdout stays empty, and stderr carries one `status: error` envelope of kind `permission` (exit 3). Its `error.details` holds the full explanation (`present`, `file`, `sha256`, `grants`, `deny`, `broad`, `sites` with per-site `decision`) plus `denied[]`. The two `sites` entries are elided here:

```json
{"request_id":"req_0180754edd","trace_id":"tr_0180754edd","operation":"rivet.policy.explain","type":"result","status":"error","data":null,"error":{"kind":"permission","code":"permission.denied","message":"1 effect site(s) of this call would be denied by the policy","retryable":false,"operation_id":"data.snapshot","details":{"present":true,"file":"./policies/read-only.json","sha256":"sha256:32dccbdd428b22dcd654d7abfcaeb2b06949e066d8bdf2b1258cf77431489a8c","grants":1,"deny":1,"broad":[],"sites":[…],"denied":[{"effect_id":"data.snapshot#1","capability":"allow_write","target":"./out/snapshot.json","access":"create"}]}},"effects":"none","data_count":0}
```

Without a denial, `--json` prints the `ok` `rivet.policy.explain` envelope on stdout (exit 0), with the explanation as `data`.

### 4. The manifest views

```text
 app.rivet ──parse/lower──▶ effect sites ──normalize──▶ IoManifest ──┬─▶ io --by operation|target|capability
                                │                                    ├─▶ io --check-policy  (◀── policy.json)
                                │                                    ├─▶ policy generate ──▶ draft policy.json
                                └──effect_id──▶ runtime trace ───────┴─▶ io --trace REQ (planned vs actual)

 demo.echo      (pure)                                          → no sites
 data.read      :15 file read   ./data/public.json              → read     allow_read
 data.private   :26 file read   ./data/private/secret.json      → read     allow_read
 data.snapshot  :37 (request "data.read" {})                    → reaches data.read's site (call_chain)
                :38 file create ./out/snapshot.json             → create   allow_write
```

#### Command / Request

```sh
rivet --file app.rivet io --check-policy
rivet --file app.rivet io --by target
rivet --file app.rivet io --by capability
rivet --file app.rivet io --kind file --access delete
rivet --file app.rivet io --kind file --access connect
rivet --file app.rivet io --strict --include-bootstrap --format json
```

#### Expected Output / Response

`--check-policy` exits **3**: the intended deny case.

```text
OPERATION      KIND  ACCESS  TARGET                      KNOWLEDGE  SOURCE        DECISION
data.private   file  read    ./data/private/secret.json  exact      app.rivet:26  denied
data.read      file  read    ./data/public.json          exact      app.rivet:15  allowed
data.snapshot  (calls data.read — see above)                        app.rivet:37
data.snapshot  file  create  ./out/snapshot.json         exact      app.rivet:38  allowed
2 allowed · 1 denied
```

```text
TARGET                      ACCESS  CAPABILITY   ORIGIN       PHASE  NEEDS FILE  USED BY
./data/private/secret.json  read    allow_read   file read    body   yes         data.private
./data/public.json          read    allow_read   file read    body   yes         data.read, data.snapshot (via data.read)
./out/snapshot.json         create  allow_write  file create  body   no          data.snapshot
```

```text
CAPABILITY / TARGET           ACCESS  USED BY                                   KNOWLEDGE
allow_read
  ./data/private/secret.json  read    data.private                              exact
  ./data/public.json          read    data.read, data.snapshot (via data.read)  exact
allow_write
  ./out/snapshot.json         create  data.snapshot                             exact

unused: allow_delete allow_network allow_listen allow_exec allow_env allow_pipe allow_unix allow_mcp allow_grpc allow_auth allow_credentials
```

Nobody deletes files (exit 0), and an access verb from another kind is a usage error (exit 2):

```text
OPERATION  KIND  ACCESS  TARGET  KNOWLEDGE  SOURCE
(no sites match kind=file access=delete)
error[validation.usage]: --access connect: not an access verb of kind file
```

The strict JSON manifest is a `rivet.io` envelope whose `data` has `"complete": true`, three sites and seven `bootstrap` entries (`./app.rivet`, `./policy.json`, `system CA bundle`, `/etc/resolv.conf / system resolver`, `tzdata`, `descriptor/schema files named by connectors (none here)`, `stdin, stdout, stderr`); every target is a literal path, so `--strict` exits 0.

### 5. JSON sites for one operation

#### Command / Request

```sh
rivet --file app.rivet io data.snapshot --check-policy --format json
```

#### Expected Output / Response

Stdout is a `rivet.io` envelope whose `data` is the IoManifest (`bundle`, `policy`, `complete`, `sites`, `targets`, `needs`, `bootstrap`); stderr prints `2 allowed` (exit 0). The two sites, abbreviated to their key fields:

```json
{"effect_id":"data.read#1","operation_id":"data.read","access":["read"],"capability":"allow_read","knowledge":"exact","call_chain":["data.snapshot","data.read"],"source":{"file":"app.rivet","line":15,"column":5},"origin":{"statement":"file read"},"phase":"body","requires_existing":true,"decision":"allowed"}
{"effect_id":"data.snapshot#1","operation_id":"data.snapshot","access":["create"],"capability":"allow_write","knowledge":"exact","call_chain":["data.snapshot"],"source":{"file":"app.rivet","line":38,"column":5},"origin":{"statement":"file create"},"phase":"body","requires_existing":false,"decision":"allowed"}
```

Each `targets` entry carries `target`, `capability`, `access`, `methods`, `protocols`, `origins`, `phases`, `needs_file`, `operations`, `used_by`, `knowledge` and `decision`.

### 6. Files each operation needs

```text
 io --needs ──▶ needed exact paths ──▶ policy broker: allow_read grant with "stat"
                                       (or no access list), and no deny match?
                                                      │
                             no ◀─────────────────────┴─────────────────────▶ yes
                             │                                                │
                             ▼                                                ▼
                       not_permitted                               stat + readability probe
                       (never probed)                          ┌──────────────┼──────────────┐
                                                               ▼              ▼              ▼
                                                            present        missing       unreadable

 exit 3   any not_permitted or unreadable   (checked first)
 exit 4   else, any missing
 exit 0   else
```

#### Command / Request

```sh
rivet --file app.rivet io --needs
rivet --file app.rivet io --check-files
rivet --file app.rivet io data.read data.snapshot --check-files
rivet --file app.rivet --policy ./policies/create-only.json io --check-files
```

#### Expected Output / Response

`io --needs` is static (exit 0); `./out/snapshot.json` is created, so it is not a need:

```text
data.private needs, before it can run:
  ./data/private/secret.json  (file read)
data.read needs, before it can run:
  ./data/public.json          (file read)
data.snapshot needs, before it can run:
  ./data/public.json          (file read, via data.read)
demo.echo needs no existing files.
```

Under policy.json the deny entry keeps the secret from being probed (exit 3):

```text
data.private needs, before it can run:
  ./data/private/secret.json  (file read)                  not_permitted
data.read needs, before it can run:
  ./data/public.json          (file read)                  present
data.snapshot needs, before it can run:
  ./data/public.json          (file read, via data.read)   present
demo.echo needs no existing files.
2 files · 1 present · 1 not_permitted
```

Only the operations the policy is meant to allow (exit 0):

```text
data.read needs, before it can run:
  ./data/public.json  (file read)                  present
data.snapshot needs, before it can run:
  ./data/public.json  (file read, via data.read)   present
1 file · 1 present
```

Under create-only.json the read grant is `"access": ["read"]` without `stat`, so the broker refuses every probe (exit 3), although `request data.read` still succeeds there:

```text
data.private needs, before it can run:
  ./data/private/secret.json  (file read)                  not_permitted
data.read needs, before it can run:
  ./data/public.json          (file read)                  not_permitted
data.snapshot needs, before it can run:
  ./data/public.json          (file read, via data.read)   not_permitted
demo.echo needs no existing files.
2 files · 2 not_permitted
```

### 7. Generate a least-privilege draft and compare

#### Command / Request

```sh
rivet --file app.rivet policy generate
rivet --file app.rivet policy generate demo.echo data.read data.snapshot --output ./policy.draft.json
rivet --file app.rivet policy generate demo.echo data.read data.snapshot --output ./policy.draft.json
cat policy.draft.json && rm policy.draft.json
rivet --file app.rivet --policy ./policies/create-only.json io --check-policy
rivet --file app.rivet --policy ./policies/create-only.json io data.snapshot --check-policy
```

#### Expected Output / Response

For every public operation (stdout; stderr `policy generate: 3 grants, 0 review items`; exit 0):

```json
{
  "version": 1,
  "grants": [
    {"capability": "allow_read", "targets": ["./data/private/secret.json"], "access": ["read"]},
    {"capability": "allow_read", "targets": ["./data/public.json"], "access": ["read"]},
    {"capability": "allow_write", "targets": ["./out/snapshot.json"], "access": ["create"]}
  ],
  "network": {"deny_private_ranges": true}
}
```

```text
                 generated draft                              hand-written policy.json
                 (what the code uses)                         (what the author intends)
 --------------  -------------------------------------------  ----------------------------------
 allow_read      ./data/public.json          [read]           ./data/**            (all read verbs)
                 ./data/private/secret.json  [read]  ◀─ !! ─▶ deny ./data/private/**  (deliberate)
 allow_write     ./out/snapshot.json         [create]         ./out/**             (create, update, append)
 deny            —  (never generated)                         allow_read ./data/private/**
 serve           —  (never generated)                         surfaces [http], auth none
```

The draft grants exactly what the source uses, including the secret read the hand-written file denies: it is a starting point for review, not an approval. Naming the intended operations leaves that grant out. The first `--output` writes the file (`policy generate: 2 grants, 0 review items`, exit 0); the second refuses to overwrite (exit 4):

```json
{"request_id":"","trace_id":"","operation":"rivet.policy.generate","type":"result","status":"error","data":null,"error":{"kind":"conflict","code":"conflict.exists","message":"refusing to overwrite ./policy.draft.json","retryable":false,"details":{"path":"./policy.draft.json"}},"effects":"none","data_count":0}
```

Without `--json`, `policy generate` prints the bare draft (so it can be redirected into a policy file); `policy generate --json` prints a `rivet.policy.generate` envelope whose `data.policy` is the draft.

`policy.draft.json` holds the two grants for `./data/public.json` `[read]` and `./out/snapshot.json` `[create]`. Under create-only.json, the whole-bundle check shows `data.private` `denied` (`2 allowed · 1 denied`, exit 3) and `io data.snapshot --check-policy` shows `2 allowed` (exit 0). A verb that does not belong to the capability fails loading (exit 2):

```text
error[policy.invalid]: policy.json /grants/0/access/0: `delete` does not belong to allow_write (allowed: create, update, append)
```

So does an unknown key such as `grantz`: `error[policy.invalid]: policy.json /grantz: unknown key `grantz` (allowed: version, grants, deny, network, limits, serve, approved)`.

### 8. Planned versus actual through one host

#### Command / Request

```sh
rivet --file app.rivet serve --listen 127.0.0.1:18811 2>serve.err & SV=$!
rivet --endpoint http://127.0.0.1:18811 request data.read
rivet --endpoint http://127.0.0.1:18811 trace show REPLACE_WITH_REQUEST_ID --json
rivet --endpoint http://127.0.0.1:18811 io data.read --trace REPLACE_WITH_REQUEST_ID
curl -sS -w ' %{http_code}\n' http://127.0.0.1:18811/v1/request -H 'content-type: application/json' -d '{"operation":"data.private","data":{}}'
kill $SV; rm -f serve.err
```

#### Expected Output / Response

```json
{"request_id":"req_02a949f6ba","trace_id":"tr_02a949f6ba","operation":"data.read","type":"result","status":"ok","data":{"message":"public demo data"},"error":null,"effects":"none","data_count":0}
```

`trace show --json` prints a `rivet.trace.show` envelope whose `data.attempts` lists one attempt: `"effect_id":"data.read#1"`, `"capability":"allow_read"`, `"access":"read"`, `"target":"./data/public.json"`, `"decision":"allowed"` and the `policy_hash`. `io --trace` joins it to the planned site:

```text
OPERATION  KIND  ACCESS  TARGET              KNOWLEDGE  SOURCE        ATTEMPTS
data.read  file  read    ./data/public.json  exact      app.rivet:15  1 allowed
```

The denied read over HTTP returns the same `permission.denied` envelope as step 2 with status `403`. The same manifest is `GET /v1/io?by=target&check_policy=true`, answered as a `rivet.io` envelope. The loopback principal `local` may call `rivet.io`; a network principal needs it listed explicitly in `serve.principals`.

## Effects and policy

```text
  outcome table (verified in steps 2, 7 and 8)
  ┌────────────────────────────────────────────┬─────────────────────────┬──────┬──────┐
  │ situation                                  │ code                    │ exit │ HTTP │
  ├────────────────────────────────────────────┼─────────────────────────┼──────┼──────┤
  │ pure call under any policy                 │ —                       │  0   │ 200  │
  │ no grant / deny entry matches              │ permission.denied       │  3   │ 403  │
  │ exclusive create onto an existing file     │ conflict.already_exists │  4   │ 409  │
  │ policy generate --output onto a file       │ conflict.exists         │  4   │  —   │
  │ unknown key or foreign access verb         │ policy.invalid          │  2   │  —   │
  └────────────────────────────────────────────┴─────────────────────────┴──────┴──────┘
```

Deny overrides grants. Policy comes only from a file: the auto-discovered policy.json, or the path given with `--policy`. There is no command-line grant syntax and no environment-variable policy. `io` performs no I/O and evaluates no source expression; `--check-files` is the only form that touches the filesystem, and only through the broker.

## Release Updates

0.2.0 updates shown here (numbering of the [v0.2.0 release verification guide](../demo-2026-0020-v0-2-0-release-verification.md)):

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-01 / U-02 | UQ-03/05 / R1, R2 | Results and errors are ResponseEnvelopes; HTTP 403 unchanged | Steps 2, 8 | Same codes and exits | This README steps 2, 8 (2026-09-29, 8031baa) |
| U-03 | UQ-03 / R3 | `io --format json`, `policy explain --json`, `policy generate --json`, `trace show --json`, `GET /v1/io` are envelopes | Steps 3–5, 7, 8 | `rivet.io`, `rivet.policy.explain`, `rivet.policy.generate`, `rivet.trace.show` | This README steps 3–5, 7, 8 |
| U-19 | UQ-09 / R19 | Bootstrap lists `./app.rivet` (and each module) instead of `(+ imports)` | Step 4 | Seven bootstrap entries starting with `./app.rivet` | This README step 4 |

Still verified from 0.1.0 (numbering of [DEMO-2026-0015](../demo-2026-0015-v0-1-0-release-verification.md)):

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-06 | UQ-05/18 / R6 | Trace attempts carry `effect_id`; `io --trace` joins planned and actual | Step 8 | `1 allowed` | This README step 8 (re-run 2026-09-29, 8031baa); TEST-2026-0009 |
| U-11 | UQ-13/17 / R11 | No policy = deny-by-default; deny overrides grants | Step 2 | `permission.denied` under empty.json and for `data.private` | This README step 2; TEST-2026-0008 |
| U-24 | UQ-17 / R24 | Policy only from policy.json / `--policy PATH`; `access` narrowing; strict schema | Steps 2, 3, 7 | Results per file; `policy.invalid` exit 2 | This README steps 2, 3, 7; TEST-2026-0021 |
| U-26 | UQ-18 / R26 | Manifest views, `--needs`, brokered `--check-files`, `policy generate` | Steps 4–7 | Tables above; exits 3 / 0 / 3; draft with 3 or 2 grants | This README steps 4–7; TEST-2026-0025, TEST-2026-0026 |

## Cleanup

```sh
rm -f out/snapshot.json policy.draft.json
rmdir out
```

Stop the step 8 host if it is still running; its memory trace records disappear when it exits.

## Verification Record

| Step | Verified By | Verified At | Result |
|---|---|---|---|
| 1. `check --strict-docs`, `outputs` | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 2. Requests under empty, default, read-only and create-only policies | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 3. `policy explain` (text and `--json`) | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 3. (re-run after INC-2026-0012) `policy explain data.snapshot --data '{}'` under read-only.json: text exit 3 (same table); `--json` → stdout empty, one `status: error` `permission.denied` envelope on stderr with the explanation and `denied[]` in `error.details` (exit 3); `data.read --json` `ok` envelope (exit 0) | Claude | 2026-09-29, commit 7c25175, macOS 26.4.1 arm64 | PASS |
| 4. `io --check-policy` (3), `--by target`, `--by capability`, filters, strict JSON (0) | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 5. `io data.snapshot --check-policy --format json` | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 6. `io --needs`; `io --check-files` exits 3 / 0 / 3 | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 7. `policy generate`, `--output` refusal, create-only checks, `policy.invalid` | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 8. `trace show`, `io --trace`, HTTP 403 through serve | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |

Verified on 0.2.0-dev at commit `8031baa`, the release candidate (`cargo build --release --workspace --all-features`); every command above was executed from this folder and the output pasted from that run (the two `policy.invalid` probes used throwaway policy files in a scratch directory). The 0.1.0 verification (TASK-067, commit 829ca43) is recorded in an earlier revision below. Step 3 was re-run with `policy explain --data` on 2026-09-29 at commit `7c25175` (source = `14750b8`) after the INC-2026-0012 fixes; its output above is from that run.

## Known Caveats

- Process sandboxing (`with process … sandbox`, Seatbelt on macOS; Linux Landlock/seccomp gated) is not part of this bundle; see TEST-2026-0008 and `rivet.capabilities` in [01-catalog](../01-catalog/README.md).
- A stale `out/snapshot.json` from an earlier run makes the first snapshot report `conflict.already_exists`; run the Cleanup first.
- `(calls X — see above)` rows mark call edges; the callee's own rows are listed under its own operation.

## Related Documents

- [All sample folders](../README.md) · [demos index](../index.md) · [v0.2.0 release verification guide](../demo-2026-0020-v0-2-0-release-verification.md) · [v0.1.0 guide](../demo-2026-0015-v0-1-0-release-verification.md)
- [Policy and I/O manifest guide](../../manuals/man-2026-0005-policy-and-io-manifest-guide.md)
- [Policy file tests TEST-2026-0021](../../testing/test-2026-0021-policy-file.md) · [I/O manifest tests TEST-2026-0025](../../testing/test-2026-0025-io-manifest.md) · [Policy generate tests TEST-2026-0026](../../testing/test-2026-0026-policy-generate.md)
- [Usage reference](../../references/ref-2026-0002-language-and-usage.md) · [Proposal](../../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 7 | 2026-09-29 | Claude | INC-2026-0012 re-verification (T-30) at 7c25175: step 3 uses `policy explain --data` (alias `--params`) and was re-run; the `--json` denial is now a `status: error` envelope on stderr (exit 3), captured; one Verification Record row. |
| 6 | 2026-09-29 | Claude | TASK-076 (PLAN-2026-0002 D-60): re-executed every step against the 0.2.0 release candidate (8031baa); `--params` → `--data` on `request` (kept on `policy explain`); results and errors replaced by 0.2.0 envelopes; JSON manifest, `policy explain --json`, `policy generate` conflict and `trace show --json` described as envelopes; bootstrap list without the `(+ imports)` placeholder; 0.2.0 Release Updates; verified_against 0.2.0 |
| 5 | 2026-09-28 | Claude | TASK-067: executed every step against 0.1.0-dev (829ca43) and pasted real output. Fixes: summary lines under each table; `--by capability` has a header and an `unused:` line; the JSON excerpt is replaced by the real site fields; `--check-files` summary counts files, not rows; `policy generate` stderr summary; `policy explain` output; `policy.invalid` messages; trace `ATTEMPTS` reads `1 allowed`; removed the stale committed `out/snapshot.json` (a leftover run output that made the first snapshot conflict); removed draft disclaimers; status active; verified_against 0.1.0. |
| 4 | 2026-09-28 | Claude | TASK-005/R26 (ADR-0001, proposal revision 8): `--by target` ORIGIN/PHASE/NEEDS FILE; new "Files each operation needs" section (`io --needs`, brokered `io --check-files` under policy.json and create-only.json, flow diagram); JSON excerpt gains origin/phase/requires_existing/secret and `needs`; U-04; exit codes 3/4 for `--check-files`. |
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest walkthrough — source→manifest diagram, `io --check-policy`, `--by target`, `--by capability`, `--kind file --access delete`, `--format json` excerpt, `policy generate` compared with policy.json, new policies/create-only.json (`access: ["create"]` narrowing) and `io --trace` planned-vs-actual. |
| 2 | 2026-09-28 | Claude | UQ-17: removed `--sandbox` and intersection text; added policies/empty.json and policies/read-only.json; prefix `(request …)` call; declared outputs/errors; serve block in policy.json; View outputs, strict-docs, bootstrap and exit codes. |
| 1 | 2026-09-28 | Codex | Added draft source files, prerequisites, invocation examples and expected behavior. |
