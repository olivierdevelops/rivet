---
document_id: MAN-2026-0005
title: "Rivet policy and I/O manifest guide"
document_type: manual
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 2
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [policy, audit, files, transports, auth, connectors, cli, serve]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, server, embedded]
audience: [administrators, operators, security-reviewers, developers]
scope: Writing policy.json (schema v1), access verbs, network targets and private ranges, limits, the generated I/O manifest (rivet io views, --check-policy, --strict, --needs, --check-files, --trace), policy generate, and the review workflow that ties them together.
reason: PLAN-2026-0001 row D-38 — administrator guide for the implemented policy broker and I/O manifest; every example executed against the 0.1.0-dev build.
related_documents: [MAN-2026-0001, MAN-2026-0004, MAN-2026-0006, MAN-2026-0008, SYS-2026-0003, SYS-2026-0008, RUN-2026-0002, DEMO-2026-0011, DEMO-2026-0002, DEMO-2026-0008]
supersedes: null
superseded_by: null
tags: [rivet, manual, policy, sandbox, io-manifest, least-privilege]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.1.0-dev (commit 829ca43)"
next_review_date: 2026-10-28
---

# Rivet policy and I/O manifest guide

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** policy, audit, files, transports, auth, connectors, cli, serve

## Purpose

Decide — and prove — what a Rivet bundle may touch. `policy.json` is the only source of authority; the I/O
manifest (`rivet io`) lists every effect site so you can review it, check it against the policy, and generate a
least-privilege draft. Part of the [Rivet manual](man-2026-0001-rivet-manual.md).

## Reading Order

```text
 how policy is found ─► write policy.json ─► access verbs ─► network + private ranges ─► limits
   ─► review I/O (rivet io) ─► check against policy ─► needed files ─► generate a draft ─► trace
   ─► the review workflow (all of the above in order)
```

## Concepts

```text
 rivet --file svc/app.rivet …                     rivet --file svc/app.rivet --policy ci.json …
            │                                                   │
   svc/policy.json exists? ── no ──► deny-by-default            ci.json (a path; never grant text)
            │ yes                    (pure operations still run)│
            ▼                                                   ▼
   strict schema v1 ── error ──► policy.invalid, exit 2 (names the JSON pointer); nothing runs
            │
            ▼
   each effect ─► (library host ceiling, then any per-request `restrict`, must also allow it)
                  deny entry matches? ── yes ─► denied
                  └ grant matches capability + target + access verb? ── no ─► denied
                  └ network target resolves to a private/loopback/link-local address
                    and no grant names that address literally? ── yes ─► denied
                  └ otherwise ─► allowed (and recorded in the trace)
```

- There are **no** command-line grants and **no** environment-variable policy.
- Relative file targets resolve against the **policy file's own directory** (`base` in `policy explain`).
- Library hosts pass the same schema (`Policy::from_file`, `Policy::from_json`) and may add a **host ceiling**
  (`.ceiling(Policy)`) that every attempt must also pass (MAN-2026-0007).
- A caller can **narrow** one request with `restrict {grants}`, never widen it ([below](#narrow-one-request-with-restrict)).

```text
  effective authority of one attempt
  ┌────────────────┐   ┌──────────────────┐   ┌──────────────────────────┐
  │ policy.json    │ ∩ │ host ceiling     │ ∩ │ restrict (this request,  │  ─► allowed only if ALL allow
  │ (or --policy)  │   │ (library only)   │   │  nested calls included)  │      and no deny matches
  └────────────────┘   └──────────────────┘   └──────────────────────────┘
```

## Task-Oriented Workflows

Examples run in `docs/demos/11-sandbox/` (read-only commands) or in a scratch copy (commands that write).

### Write a policy.json

Schema v1 — every key except `version` is optional; unknown keys are rejected:

```json
{
  "version": 1,
  "grants": [
    {"capability": "allow_read",  "targets": ["./data/**"]},
    {"capability": "allow_write", "targets": ["./out/**"], "access": ["create"]}
  ],
  "deny":     [{"capability": "allow_read", "targets": ["./data/private/**"]}],
  "network":  {"deny_private_ranges": true},
  "limits":   {"max_concurrent_requests": 64, "max_call_depth": 16, "max_buffered_bytes": 268435456},
  "approved": {"snapshots": ["sha256:…"], "overlaps": ["sha256:…"]},
  "serve":    {"surfaces": ["http", "sse", "poll", "ws", "mcp"],
               "auth": {"type": "none"},
               "principals": {"local": {"operations": ["*"]}}}
}
```

| Key | Rule |
|---|---|
| `version` | must be `1` |
| `grants[]` / `deny[]` | `{capability, targets[≥1], access?}`; `deny` always wins over `grants` |
| `targets` | glob selectors (`./out/**`, exact paths, `"*"`); network/listen targets must be a URL (`https://host:443`, `udp://…`, `tcp://…`, `quic://…`), an IP or a CIDR |
| `network.deny_private_ranges` | default `true` |
| `limits.*` | positive integers; defaults 64 / 16 / 268435456; a value wider than its field is rejected, never truncated (`max_concurrent_requests` and `max_call_depth` ≤ 4294967295, `max_buffered_bytes` ≤ 9223372036854775807) |
| `limits.max_buffered_bytes` | enforced: the bytes held by all session/stream queues of the host; a reservation beyond it fails `limit.buffered_bytes` (429, exit 5) |
| `approved.snapshots` | sha256 of reviewed MCP snapshots (MAN-2026-0008) |
| `approved.overlaps` | accepted and validated, but **unused** in 0.1.0 |
| `serve` | `surfaces`, `auth` (`none` / `bearer` / `mtls`), `principals` (MAN-2026-0006) |

The demo policy (`docs/demos/11-sandbox/policy.json`):

```json
{
  "version": 1,
  "grants": [
    {"capability": "allow_read", "targets": ["./data/**"]},
    {"capability": "allow_write", "targets": ["./out/**"]}
  ],
  "deny": [
    {"capability": "allow_read", "targets": ["./data/private/**"]}
  ],
  "serve": {
    "surfaces": ["http"],
    "auth": {"type": "none"}
  }
}
```

Result:

```text
$ rivet request --file app.rivet data.read
{"request_id":"req_018cb08bbd","trace_id":"tr_018cb08bbd","result":{"message":"public demo data"},"data_count":0,"effects":"none"}
$ rivet request --file app.rivet data.private                                                   [exit 3]
{"…","error":{"kind":"permission","code":"permission.denied","message":"allow_read read on ./data/private/secret.json denied: deny allow_read ./data/private/**","retryable":false,"effects":"none",…,"details":{"capability":"allow_read","access":"read","target":"./data/private/secret.json"}}}
```

Validation failures — each is `policy.invalid`, exit 2, and nothing runs (not even `check` or `serve`):

```text
error[policy.invalid]: policy.json /version: `version` must be 1
error[policy.invalid]: policy.json /grants/0/access/0: `delete` does not belong to allow_read (allowed: read, list, stat, watch)
error[policy.invalid]: policy.json /grants/0/targets/0: `example.com` must be a URL such as https://host:443, an IP or a CIDR
error[policy.invalid]: policy.json /serve/surfaces/0: unknown surface `gopher` (http, sse, poll, ws, mcp)
error[policy.invalid]: policy.json /limits/max_concurrent_requests: must be at most 4294967295
error[policy.invalid]: --policy /dev/null: no such file
```

### Access verbs

Each verb belongs to exactly one capability. `access` on a grant narrows it; absent `access` = every verb.

```text
  capability         verbs                          produced by
  ─────────────────  ─────────────────────────────  ─────────────────────────────────────────────────
  allow_read         read, list, stat, watch        file read/list/stat, update (stat), copy/move src,
                                                    tls ca_file/cert_file/key_file, body file
  allow_write        create, update, append         file create/update/write/append, copy/move dest
  allow_delete       delete                         file delete, move source
  allow_network      connect                        http, websocket, tcp, udp, quic, grpc endpoint, MCP http
  allow_listen       bind, listen, multicast_join   udp bind, udp multicast
  allow_exec         exec                           command, MCP transport command
  allow_env          read                           secret … from env, client_secret env
  allow_pipe         read, write                    pipes
  allow_unix         connect, listen                unix sockets, http … unix
  allow_mcp          call                           MCP tools/resources/prompts, CONNECTOR/discover
  allow_grpc         call                           gRPC methods (unary|server_stream|client_stream|bidi)
  allow_auth         use, manage, status            OAuth profile use and `rivet auth …`
  allow_credentials  read, write                    OAuth credential store
```

`file update` needs `allow_read` **stat** as well as `allow_write` **update** (it reads the current version
first); a scoped `with file open … mode write` needs `create` + `update`, `mode append` needs `append`.

Narrowing example — allow creating notes but never overwriting (`11-sandbox/policies/create-only.json`):

```json
{"capability": "allow_write", "targets": ["../out/**"], "access": ["create"]}
```

| Grant | Allows | Denies |
|---|---|---|
| `allow_write ./out/**` | create, update, append | — |
| `allow_write ./out/**` `["create"]` | create | update, append |
| `allow_read ./data/**` `["read"]` | read | list, stat (so `io --check-files` reports `not_permitted`) |

Target formats per capability:

| Capability | Target format | Example |
|---|---|---|
| file capabilities | path glob relative to the policy file | `./out/**`, `../data/public.json` |
| `allow_network`, `allow_listen` | `scheme://host:port`, IP or CIDR | `https://api.example.com:443`, `udp://127.0.0.1:7000`, `tcp://127.0.0.1:9000`, `quic://engine.example.com:4433`, `127.0.0.0/8` |
| `allow_exec` | absolute binary path (or bundle-relative `./bin`) | `/bin/cat` |
| `allow_env` | variable name | `CRM_CLIENT_SECRET` |
| `allow_mcp` | `CONNECTOR/tools/NAME`, `CONNECTOR/discover` | `crm/tools/search` |
| `allow_grpc` | `CONNECTOR/package.Service/Method` | `users/example.Users/GetUser` |
| `allow_auth` | `PROFILE/ACCOUNT/VERB` (globs allowed) | `crm_service/service/use`, `crm_device/ada/*` |
| `allow_credentials` | `PROFILE/ACCOUNT` | `crm_service/service` |

### Network targets and private ranges

`network.deny_private_ranges` (default `true`) denies RFC 1918, loopback, link-local, `169.254.169.254`,
`fc00::/7` and `fe80::/10` for every scheme — **after** DNS resolution — unless a grant names the address
literally (IP, CIDR or a URL with that IP). A hostname grant or `"*"` does not lift it.

Verified against a local HTTP fixture on `127.0.0.1:18480`:

| Grant | Result |
|---|---|
| `http://127.0.0.1:18480` | allowed |
| `127.0.0.1` or `127.0.0.0/8` | allowed |
| `http://localhost:18480` | denied: "127.0.0.1 is a private/loopback/link-local address; grant it literally (e.g. \"http://127.0.0.1:18480/users/42\") to allow it" |
| `"*"` | denied (same message) |
| grant + identical `deny` entry | denied: "… denied: deny allow_network http://127.0.0.1:18480" |
| `http://127.0.0.1:18480` with `"network": {"deny_private_ranges": false}` | allowed |

A URL grant with a **path** matches whole path segments: `https://api.example.com:443/users/42` covers
`/users/42` and `/users/42/…` but not `/users/420`; a selector ending in `/` covers everything below it, and an
explicit trailing `*` (`/users/4*`) is a raw prefix.

### Explain the effective policy

```text
$ rivet policy --file app.rivet explain
policy   ./policy.json (sha256:947547716ec424e0a8292cf06f958d9a8f40fbf67548e1477bfbe932ba382008)
base     .
network  deny_private_ranges true
limits   64 concurrent, depth 16, 268435456 buffered bytes
grant    allow_read ./data/**
grant    allow_write ./out/**
deny     allow_read ./data/private/**
```

With no `policy.json`: `policy   none — no policy.json: every new application effect is denied (pure operations
still run)`. With an ID, the operation's sites and decisions are appended (MAN-2026-0004). A `"*"` target is
flagged `⚠ broad: "*" allows every target`.

**Explain one concrete call.** `policy explain ID --params JSON` fills the operation's param-dependent targets
from those params, evaluates each against the policy exactly as the broker would, and **exits 3** when any is
denied — a pre-flight check that issues no permit and touches nothing (scratch bundle, commit `829ca43`;
`demo.read` = `return file read path as text`, policy granting `allow_read ./data/**`):

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

Without `--params` the same site prints its template (`{path}`, `param_dependent`) and the command exits 0.

### Narrow one request with restrict

Why: a caller (an agent, a multi-tenant front end, a library host) wants a request to run with **less** authority
than the server's `policy.json` — for example only one tenant's directory.

`restrict` is `{"grants": [...]}` in the policy.json grant format. It is accepted on `POST /v1/request`,
`POST /v1/requests` (polling), WebSocket `request` frames, MCP `tools/call` params and the library
(`Runtime::request_restricted`). Each broker decision of that request **and its nested calls** must pass the
loaded policy **and** the restriction; a restriction naming a target the policy does not grant adds nothing.
Any key other than `grants`, or a malformed grant, is `policy.invalid` with a `/restrict/…` pointer (422, exit 2).

```text
 policy.json grants allow_read ./data/**
   restrict {grants:[allow_read ./data/other/**]}   read data/a.txt   ─► denied "request restriction: no grant …"
   restrict {grants:[allow_read ./**]}              read app.rivet    ─► denied "no grant for allow_read app.rivet"
   (no restrict)                                    read data/a.txt   ─► allowed
```

Captured on `rivet serve --listen 127.0.0.1:18901` (commit `829ca43`):

```text
$ curl -s -X POST http://127.0.0.1:18901/v1/request -d '{"id":"demo.read","params":{"path":"data/a.txt"},
       "restrict":{"grants":[{"capability":"allow_read","targets":["./data/other/**"],"access":["read"]}]}}'     # 403
{…"error":{"kind":"permission","code":"permission.denied","message":"allow_read read on data/a.txt denied: request restriction: no grant for allow_read data/a.txt",…}}

$ curl -s -X POST http://127.0.0.1:18901/v1/request -d '{"id":"demo.read","params":{"path":"data/a.txt"},"restrict":{"bogus":1}}'   # 422
{…"error":{"kind":"validation","code":"policy.invalid","message":"restrict accepts only `grants` (got `bogus`); it can narrow, never grant",…,"details":{"pointer":"/restrict/bogus"}}}
```

### Review I/O with rivet io

Why: see every path, URL, command, env var, MCP tool and gRPC method a bundle can touch **before** running it.

```text
$ rivet io --file app.rivet                         # --by operation (default)
OPERATION      KIND  ACCESS  TARGET                      KNOWLEDGE  SOURCE
data.private   file  read    ./data/private/secret.json  exact      app.rivet:26
data.read      file  read    ./data/public.json          exact      app.rivet:15
data.snapshot  (calls data.read — see above)                        app.rivet:37
data.snapshot  file  create  ./out/snapshot.json         exact      app.rivet:38

$ rivet io --file app.rivet --by target
TARGET                      ACCESS  CAPABILITY   ORIGIN       PHASE  NEEDS FILE  USED BY
./data/private/secret.json  read    allow_read   file read    body   yes         data.private
./data/public.json          read    allow_read   file read    body   yes         data.read, data.snapshot (via data.read)
./out/snapshot.json         create  allow_write  file create  body   no          data.snapshot

$ rivet io --file app.rivet --by capability
CAPABILITY / TARGET           ACCESS  USED BY                                   KNOWLEDGE
allow_read
  ./data/private/secret.json  read    data.private                              exact
  ./data/public.json          read    data.read, data.snapshot (via data.read)  exact
allow_write
  ./out/snapshot.json         create  data.snapshot                             exact

unused: allow_delete allow_network allow_listen allow_exec allow_env allow_pipe allow_unix allow_mcp allow_grpc allow_auth allow_credentials
```

Filters and formats: `--kind file --access create`, `--format markdown`, `--format csv` (columns
`effect_id,operation_id,kind,access,method,protocol,capability,target,glob,knowledge,call_chain,secrets,source,origin,phase,requires_existing,secret,decision`),
`--json`. `--include-bootstrap` adds the runtime-internal reads that policy does not govern:

```text
BOOTSTRAP (runtime-internal; listed, not governed by policy.json)
KIND  ACCESS       TARGET
file  read         ./app.rivet (+ imports)
file  read         ./policy.json
file  read         system CA bundle
file  read         /etc/resolv.conf / system resolver
file  read         tzdata
file  read         descriptor/schema files named by connectors (none here)
pipe  read, write  stdin, stdout, stderr
```

UDP multicast sites mirror exactly what the runtime authorizes when the socket opens: `allow_network` connect on
the group, then `allow_listen` bind **and** `multicast_join` on the **bind address** (the `bind` option, or the
unspecified address on the group's port), so `io --check-policy` and the run agree.

Network and other kinds appear the same way, e.g. for 03-http-style bundles:

```text
TARGET                  ACCESS             CAPABILITY     ORIGIN                              PHASE    NEEDS FILE  USED BY
http://127.0.0.1:18480  connect GET, POST  allow_network  http get, http post, with http get  connect  —           users.get, …
env FIXTURE_TOKEN       read               allow_env      secret                              body     —           users.create (secret TOKEN, bound to http://127.0.0.1:18480)
```

Over the other surfaces: `rivet --endpoint URL io`, `GET /v1/io?by=target&check_policy=true`, the MCP tool
`rivet.io` and `Runtime::io` — non-local principals need an exact `rivet.io` listing (MAN-2026-0006).

### Check the manifest against policy

```text
 rivet io --check-policy ──► every reachable site: allowed | denied | unknown(dynamic)
        all allowed ─► exit 0            any denied/unknown ─► table still printed, exit 3
 rivet io --strict ─────► any dynamic/opaque target ─► "io: complete=false …", exit 7
```

```text
$ rivet io --file app.rivet --check-policy --policy policies/read-only.json                     [exit 3]
OPERATION      KIND  ACCESS  TARGET                      KNOWLEDGE  SOURCE        DECISION
data.private   file  read    ./data/private/secret.json  exact      app.rivet:26  denied
data.read      file  read    ./data/public.json          exact      app.rivet:15  allowed
data.snapshot  (calls data.read — see above)                        app.rivet:37
data.snapshot  file  create  ./out/snapshot.json         exact      app.rivet:38  denied
1 allowed · 2 denied
```

A literal call to an operation that has no I/O sites of its own is shown as `(calls X — no I/O)`; `(calls X — see
above)` means the callee's rows are listed separately (commit `2a751ab`):

```text
$ rivet --file io2.rivet --policy policy.json io --check-policy          # caller.op calls the pure pure.helper
OPERATION  KIND  ACCESS  TARGET        KNOWLEDGE  SOURCE        DECISION
caller.op  (calls pure.helper — no I/O)           io2.rivet:13
caller.op  file  read    ./data/a.txt  exact      io2.rivet:14  allowed
1 allowed
```

A dynamic target (08-udp replies to whoever sent the datagram):

```text
$ rivet io --file app.rivet --strict                                                            [exit 7]
telemetry.receive  network  connect  udp://{message.peer}  dynamic    app.rivet:24
io: complete=false — 1 of 3 sites is dynamic/opaque
  telemetry.receive#2  network connect  target from expression message.peer  (app.rivet:24)
```

Use `--check-policy` as a CI gate after every policy or bundle change (see RUN-2026-0002).

### Files an operation needs

`--needs` lists the files that must already exist; `--check-files` probes them through the broker (it needs the
`stat` verb):

```text
$ rivet io --file app.rivet --needs                          # 02-file-crud
notes.create needs no existing files.
notes.delete needs no existing files.
notes.list needs, before it can run:
  ./out            (file list)
notes.read needs, before it can run:
  ./out/note.json  (file read)
notes.update needs, before it can run:
  ./out/note.json  (file update)

$ rivet io --file app.rivet --check-files                                                       [exit 4]
…
notes.read needs, before it can run:
  ./out/note.json  (file read)     missing
notes.update needs, before it can run:
  ./out/note.json  (file update)   missing
2 files · 1 present · 1 missing
```

| Probe result | Meaning | Exit |
|---|---|---|
| `present` | exists and `stat` is granted | 0 when all present |
| `missing` | granted but absent | 4 |
| `not_permitted` | no `stat` grant (or denied) | 3 (wins over 4) |

`--check-files` is local only; over `--endpoint` it is `validation.check_files_remote` (exit 2).

### Generate a least-privilege draft

```text
$ rivet policy --file app.rivet generate --all               # 11-sandbox
{
  "version": 1,
  "grants": [
    {"capability": "allow_read", "targets": ["./data/private/secret.json"], "access": ["read"]},
    {"capability": "allow_read", "targets": ["./data/public.json"], "access": ["read"]},
    {"capability": "allow_write", "targets": ["./out/snapshot.json"], "access": ["create"]}
  ],
  "network": {"deny_private_ranges": true}
}
policy generate: 3 grants, 0 review items                    (stderr)
```

- Exact targets and exactly the verbs used; no `deny`, no `serve` block.
- `--output PATH` writes the file, rebasing targets to its directory (`../data/public.json` under
  `policies/`), and never overwrites (`conflict.exists`, exit 4).
- Dynamic sites are **not** granted; they become `review` lines and the exit code is 7 (draft still printed).
- Over HTTP: `POST /v1/policy/generate {"ids":[…]|"all":true}` returns `{policy, review, complete}` and never
  writes a file.

### Trace one request

Traces record every broker decision of a request, in the serving process's memory:

```text
$ rivet --endpoint http://127.0.0.1:18411 io data.read --trace req_020a473102
OPERATION  KIND  ACCESS  TARGET              KNOWLEDGE  SOURCE        ATTEMPTS
data.read  file  read    ./data/public.json  exact      app.rivet:15  1 allowed
```

`rivet --endpoint URL trace show REQ` prints the full attempts (MAN-2026-0004). A local CLI process has an
empty trace store (`not_found.trace`).

### The review workflow

```text
 ┌─ author ─────────────────┐   ┌─ reviewer ─────────────────────────────┐   ┌─ CI / deploy ────────────┐
 │ edit app.rivet           │   │ rivet io --by target   (what & where)  │   │ rivet check              │
 │ rivet io --strict        │──►│ rivet policy generate --output new.json│──►│ rivet io --check-policy  │
 │  (exit 7? make targets   │   │ diff new.json policy.json; narrow with │   │   exit 0 → deploy        │
 │   exact or review them)  │   │ "access", add "deny", keep private     │   │   exit 3 → block         │
 └──────────────────────────┘   │ ranges on; commit policy.json          │   │ rivet io --check-files   │
                                └────────────────────────────────────────┘   │   (before first run)     │
                                                                             └──────────────────────────┘
```

1. `rivet io --file app.rivet --strict` — make every target exact, or list each dynamic one for review.
2. `rivet policy --file app.rivet generate --all --output policies/candidate.json` — the least-privilege draft.
3. Review: compare with the current `policy.json`; add `deny` entries for sensitive paths; narrow `access`.
4. `rivet io --file app.rivet --check-policy` — must exit 0 for the operations you ship.
5. `rivet io --file app.rivet --check-files` — before the first run on a new host.
6. For a running server, roll out with [RUN-2026-0002](../runbooks/run-2026-0002-roll-out-policy-change.md).

### Expected Result and Side Effects

`policy explain`, `io` (all views) and `policy generate` without `--output` read only the bundle and policy.
`io --check-files` performs `stat` probes through the broker. `policy generate --output` creates one new file.

### Verified Demo

[11-sandbox (DEMO-2026-0011)](../demos/11-sandbox/README.md), plus [02-file-crud](../demos/02-file-crud/README.md)
and [08-udp](../demos/08-udp/README.md); every example above was executed with `rivet 0.1.0-dev` (commit
`f40d4aa`; `explain --params` and `restrict` at `829ca43`).

## Errors and Recovery Reference

| Error / Code / Message | Surface | Cause | User-Visible Result | Recovery | Retry Safe | Related Feature |
|---|---|---|---|---|---|---|
| `policy.invalid` | all | schema violation | JSON pointer named, exit 2 | fix that key | no | policy.json |
| `permission.denied` | all | no grant / deny wins / private range | `details.capability/access/target`, exit 3 | add a precise grant | no | grants |
| `unsupported.sandbox_backend` "cannot represent this policy exactly" | process start | `access`-narrowed or complex-glob read/write grants in a bundle that runs processes | exit 5 | use `DIR/**`, exact paths or `*` for those grants | no | sandbox |
| `conflict.exists` | `policy generate --output` | file exists | exit 4 | choose a new path | no | generate |
| `validation.usage` | `io` | bad `--by`, `--format`, `--access` | exit 2 | use a listed value | no | io |
| `validation.check_files_remote` | `--endpoint io --check-files` | probe is local only | exit 2 | run locally | no | needs |
| `not_found.trace` | `trace show`, `trace export`, `io --trace` | request not in this process's store | exit 4 | query the serving process | no | trace |
| `policy explain … --params` exit 3 | CLI | a concrete target would be denied | table + `denied:` line | add a grant or change the params | — | explain |
| `limit.buffered_bytes` | sessions / streams | `limits.max_buffered_bytes` reached host-wide | 429, exit 5 | drain sessions, raise the limit | yes, after backoff | limits |
| `policy.invalid` `/restrict/…` | HTTP, MCP, WS, polling, library | malformed `restrict` | 422, exit 2 | send `{"grants":[…]}` only | no | restrict |

## Limitations

The policy rows of the [manual's Known Limitations](man-2026-0001-rivet-manual.md#known-limitations):

- `approved.overlaps` is accepted but unused.
- The `*` principal pattern in `serve.principals` also matches `rivet.auth.*` (their use is governed by
  `allow_auth` grants).
- No persistent trace store: traces are in memory, per process, and bounded.

Behaviour to plan for (by design): the process sandbox cannot express `access`-narrowed or arbitrary-glob
read/write grants (MAN-2026-0008), and dynamic targets can only be reviewed, never pre-granted exactly.

## Version Applicability

| Feature / Interface | Introduced | Changed | Deprecated / Removed | Applicable Environment |
|---|---|---|---|---|
| policy.json schema v1 | 0.1.0 | — | — | all |
| `rivet io`, `--check-policy`, `--strict`, `--needs`, `--check-files`, `--trace` | 0.1.0 | — | — | all (`--check-files` local only) |
| `policy explain` (incl. `--params`), `policy generate` | 0.1.0 | — | — | all |
| per-request `restrict`, library ceiling | 0.1.0 | — | — | server, embedded |

## Related Documents

- [Rivet manual](man-2026-0001-rivet-manual.md) · [CLI reference](man-2026-0004-cli-reference.md) ·
  [Serving](man-2026-0006-serving-and-surfaces.md) · [Protocols](man-2026-0008-protocols-and-connectors.md)
- [SYS-2026-0003 policy broker and I/O manifest](../system/components/sys-2026-0003-policy-broker-and-io-manifest.md) ·
  [SYS-2026-0008 policy.json reference](../system/configuration/sys-2026-0008-policy-json-reference.md)
- [RUN-2026-0002 roll out a policy change](../runbooks/run-2026-0002-roll-out-policy-change.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial policy and I/O manifest guide for 0.1.0, verified against 0.1.0-dev commit f40d4aa. |
| 2 | 2026-09-28 | Claude | Fix batch through 829ca43 and 2a751ab (`— no I/O` call rows): `policy explain --params` (exit 3), per-request `restrict` section, host ceiling, enforced `max_buffered_bytes`, limit field widths, `approved.overlaps` unused, `update` needs `stat`, URL path segment matching, multicast manifest mirrors runtime; limitations aligned with MAN-2026-0001. |
