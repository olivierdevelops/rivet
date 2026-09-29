---
document_id: DEMO-2026-0016
title: "Global constants shared by every operation"
document_type: demo
status: active
created_date: 2026-09-29
last_updated: 2026-09-30
document_revision: 3
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, audit, policy, cli]
affected_versions:
  from: "0.2.0"
  to: null
applicable_environments: [development]
audience: [developers, reviewers]
scope: One app.rivet with seven `global` constants used by four operations; the exact I/O manifest and least-privilege draft they produce; retargeting the whole bundle by editing one global; the six global diagnostics (`syntax.global`, `check.global_*`).
reason: PROP-2026-0002 UC-04 (UQ-07 "allow global vars for better reuse of vars"); PLAN-2026-0002 D-17, TASK-075.
dependencies: [PROP-2026-0002, PLAN-2026-0002]
related_documents: [PROP-2026-0002, PLAN-2026-0002, DEMO-2026-0020, DEMO-2026-0013, MAN-2026-0003, MAN-2026-0005, API-2026-0005, API-2026-0006]
supersedes: null
superseded_by: null
tags: [rivet, demo, globals, language, io-manifest]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-29
verified_against: "0.2.0"
---

# Global constants shared by every operation

> **Status:** Active
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-30
> **Affected Versions:** 0.2.0 and later
> **Owner:** Project maintainer
> **Affected Components:** language, audit, policy, cli

## Purpose

`global NAME = EXPR` declares a top-level, read-only constant that every operation in the file can read. This demo shows the four things that matter to an author:

1. globals remove repeated literals (one API origin, one page size, one header set);
2. they are evaluated **once at load** and can never change, so the I/O manifest can substitute them: a target built only from literals and globals is `exact` and gets an exact grant;
3. editing one global retargets every operation that uses it;
4. anything that is not a load-time constant is refused by `rivet check` with a precise span.

Read [app.rivet](app.rivet) alongside this walkthrough.

```text
  app.rivet
  ┌─────────────────────────────────────────────────────────────────┐
  │ global api       = "https://api.example.com"   ◀── edit here    │
  │ global users_url = "${api}/users"              (reads api)       │
  │ global page_size = 50                                            │
  │ global retry_on  = [429, 503]                                    │
  │ global headers   = {accept: "application/json"}                  │
  │ global data_dir  = "./data"                                      │
  │ global max_page  = page_size * 2               (reads page_size) │
  └───────────┬──────────────┬──────────────┬──────────────┬─────────┘
              │              │              │              │
         users.get      users.page     catalog.read   settings.show
     ${users_url}/{id}  ${users_url}/  ${data_dir}/    returns the
        .json (GET)     index.json?    catalog.json    globals (pure)
                        limit=50 (GET)   (read)
```

| Operation ID | Uses | Effect | Manifest knowledge |
|---|---|---|---|
| `users.get` | `users_url`, `headers` | `http get "${users_url}/${id}.json"` | `param_dependent` (the `{id}` param), host and path resolved |
| `users.page` | `users_url`, `page_size`, `headers` | `http get "${users_url}/index.json?limit=${page_size}"` | `exact` (literals and globals only) |
| `catalog.read` | `data_dir` | `file read "${data_dir}/catalog.json"` | `exact` |
| `settings.show` | all seven | none (pure) | no site |

```text
  load ──▶ parse ──▶ lower globals in declaration order ──▶ evaluate (pure) ──▶ frozen scope (read-only)
                              │                                   │
                              │                                   └─ env / secret / request / effect / param ──▶ check.global_not_constant
                              └─ name used before its line ──▶ check.global_forward_ref

  request ──▶ name lookup:  locals ──▶ params ──▶ globals          assignment to a global ──▶ check.global_assign
```

## Verified Against Version

0.2.0. Verified on 0.2.0-dev at commit `8031baa`, the release candidate (the version string is bumped from 0.1.0 to 0.2.0 at release, P5), with `target/release/rivet` on macOS 26.4.1 (Darwin 25.4.0, arm64), 2026-09-29. Every output block below was pasted from that run. Request and trace IDs, hashes and the http.server log timestamps vary.

## Prerequisites

```sh
cargo build --release --features cli       # from the repository root
export PATH="$PWD/target/release:$PATH"
```

`python3` (standard library `http.server`) serves [data/](data/users/7.json) for step 5. Loopback port 18840 must be free.

## Setup

```sh
cd docs/demos/14-globals
```

[policy.json](policy.json) is auto-discovered. It grants exactly the two targets the manifest reports: the origin `https://api.example.com:443` and the file `./data/catalog.json`. `api.example.com` is a placeholder host; step 5 points the bundle at a local server by changing the `api` global only. The failing examples live in [errors/](errors/not_constant.rivet), one file per diagnostic.

```text
  14-globals/
  ├── app.rivet               seven globals, four operations
  ├── policy.json             two exact grants
  ├── data/catalog.json       read by catalog.read
  ├── data/users/7.json       served in step 5 (users.get 7)
  ├── data/users/index.json   served in step 5 (users.page)
  └── errors/*.rivet          one file per global diagnostic (step 6)
```

## Steps

### 1. Check and list

#### Command / Request

```sh
rivet --file app.rivet check --strict-docs
rivet --file app.rivet list
```

#### Expected Output / Response

```text
ok: 4 operations, 0 connectors, 0 auth profiles
```

```text
ID             NAME                      DESCRIPTION
users.get      Get a user                Read one user; the URL is built from the users_url global.
users.page     List a page of users      Read the first page; every part of the URL comes from globals, so the target is exact.
catalog.read   Read the local catalog    Read a JSON file whose directory is the data_dir global.
settings.show  Show the shared settings  Return the globals as seen from inside an operation (pure, no I/O).
```

Both exit 0. Globals are not operations and are not listed.

### 2. Read the globals from inside an operation

#### Command / Request

```sh
rivet --file app.rivet request settings.show
rivet --file app.rivet request settings.show --data '{"page":3}' --pretty
rivet --file app.rivet request catalog.read
```

#### Expected Output / Response

Every value was computed once at load: interpolation (`users_url`), a list index (`retry_on.0`), an object key (`headers.accept`) and arithmetic (`max_page`). `offset` mixes the `page` parameter with the `page_size` global (exit 0):

```json
{"request_id":"req_010f477d9d","trace_id":"tr_010f477d9d","operation":"settings.show","type":"result","status":"ok","data":{"api":"https://api.example.com","users_url":"https://api.example.com/users","page_size":50,"first_retry":429,"accept":"application/json","max_page":100,"offset":0},"error":null,"effects":"none","data_count":0}
```

```json
{
  "request_id": "req_010e5d317d",
  "trace_id": "tr_010e5d317d",
  "operation": "settings.show",
  "type": "result",
  "status": "ok",
  "data": {
    "api": "https://api.example.com",
    "users_url": "https://api.example.com/users",
    "page_size": 50,
    "first_retry": 429,
    "accept": "application/json",
    "max_page": 100,
    "offset": 100
  },
  "error": null,
  "effects": "none",
  "data_count": 0
}
```

`catalog.read` reads `./data/catalog.json`, whose directory is the `data_dir` global (exit 0):

```json
{"request_id":"req_010eb9cb25","trace_id":"tr_010eb9cb25","operation":"catalog.read","type":"result","status":"ok","data":{"items":["bolt","nut","washer"],"source":"14-globals/data/catalog.json"},"error":null,"effects":"none","data_count":0}
```

### 3. The I/O manifest substitutes globals

#### Command / Request

```sh
rivet --file app.rivet io --by target
rivet --file app.rivet io --check-policy
rivet --file app.rivet io --check-files
rivet --file app.rivet io --strict
rivet --file app.rivet policy explain users.get --data '{"id":7}'
```

#### Expected Output / Response

No global name appears in any target: each was replaced by its value.

```text
TARGET                       ACCESS       CAPABILITY     ORIGIN     PHASE    NEEDS FILE  USED BY
./data/catalog.json          read         allow_read     file read  body     yes         catalog.read
https://api.example.com:443  connect GET  allow_network  http get   connect  —           users.get, users.page
```

```text
OPERATION     KIND     ACCESS       TARGET                                             KNOWLEDGE        SOURCE        DECISION
catalog.read  file     read         ./data/catalog.json                                exact            app.rivet:38  allowed
users.get     network  connect GET  https://api.example.com/users/{id}.json            param_dependent  app.rivet:16  allowed
users.page    network  connect GET  https://api.example.com/users/index.json?limit=50  exact            app.rivet:27  allowed
3 allowed
```

```text
  source text                                        manifest target                                      knowledge
  ─────────────────────────────────────────────────  ───────────────────────────────────────────────────  ───────────────
  "${users_url}/index.json?limit=${page_size}"  ──▶  https://api.example.com/users/index.json?limit=50    exact
  "${users_url}/${id}.json"                     ──▶  https://api.example.com/users/{id}.json              param_dependent
  "${data_dir}/catalog.json"                    ──▶  ./data/catalog.json                                  exact
```

`io --check-policy` exits 0. `io --check-files` stats the one needed file through the broker (`./data/catalog.json  (file read)   present`, `1 file · 1 present`, exit 0). `io --strict` exits 0: nothing is dynamic. `users.get` stays `param_dependent` because its path contains the `{id}` parameter (the 0.1.0 rule); its host and path prefix are resolved and grantable. Filling the parameter shows the concrete target (exit 0):

```text
policy   ./policy.json (sha256:7301d44583e6f9f16aeb06c3e26caf2eab194ca2eddbe3fae1b16b8bfa2db61c)
base     .
network  deny_private_ranges true
limits   64 concurrent, depth 16, 268435456 buffered bytes
grant    allow_network https://api.example.com:443
grant    allow_read ./data/catalog.json

OPERATION  KIND     ACCESS       TARGET                                KNOWLEDGE  SOURCE        DECISION
users.get  network  connect GET  https://api.example.com/users/7.json  exact      app.rivet:16  allowed
```

In `--format json` the `users.page` site is `"knowledge":"exact"` with `"template":"https://api.example.com/users/index.json?limit=50","host":"api.example.com","port":443,"params":[]`, and the `users.get` site keeps `"params":["id"]`.

### 4. A least-privilege draft from globals

#### Command / Request

```sh
rivet --file app.rivet policy generate
```

#### Expected Output / Response

The draft names the exact file and origin that the globals resolve to (stdout; stderr `policy generate: 2 grants, 0 review items`; exit 0):

```json
{
  "version": 1,
  "grants": [
    {"capability": "allow_read", "targets": ["./data/catalog.json"], "access": ["read"]},
    {"capability": "allow_network", "targets": ["https://api.example.com:443"]}
  ],
  "network": {"deny_private_ranges": true}
}
```

The shipped policy.json grants the same two targets; its file grant has no `access` list, so it also allows the `stat` that `io --check-files` performs (a draft `"access": ["read"]` grant makes `--check-files` report `not_permitted`, exit 3; see [11-sandbox](../11-sandbox/README.md#6-files-each-operation-needs)).

### 5. Retarget the bundle by editing one global

In this folder the placeholder host does not resolve (`dns.resolve`, exit 5). Point the bundle at a local server in a scratch copy: one line of app.rivet and the matching grant change.

#### Command / Request

```sh
rivet --file app.rivet request users.page               # in this folder
WORK="$(mktemp -d)"; cp -R app.rivet policy.json data "$WORK/"; cd "$WORK"
sed -i.bak 's#^global api        = "https://api.example.com"#global api        = "http://127.0.0.1:18840"#' app.rivet
sed -i.bak 's#https://api.example.com:443#http://127.0.0.1:18840#' policy.json
diff app.rivet.bak app.rivet
(cd data && python3 -m http.server 18840 --bind 127.0.0.1 > ../http.log 2>&1) & HS=$!
rivet --file app.rivet io --check-policy
rivet --file app.rivet request users.get --data '{"id":7}'
rivet --file app.rivet request users.page
rivet --file app.rivet request users.get --data '{"id":8}'
rivet --file app.rivet request users.get --data '{"id":0}'
grep GET http.log
```

#### Expected Output / Response

In the folder:

```json
{"request_id":"req_017dee0375","trace_id":"tr_017dee0375","operation":"users.page","type":"result","status":"error","data":null,"error":{"kind":"dns","code":"dns.resolve","message":"cannot resolve api.example.com: failed to lookup address information: nodename nor servname provided, or not known","retryable":false,"source":{"file":"app.rivet","line":27,"column":5,"end_line":30,"end_column":8},"operation_id":"users.page"},"effects":"none","data_count":0}
```

The whole source change is one line:

```text
3c3
< global api        = "https://api.example.com"
---
> global api        = "http://127.0.0.1:18840"
```

Both network operations follow it, because `users_url` is built from `api` (exit 0):

```text
OPERATION     KIND     ACCESS       TARGET                                            KNOWLEDGE        SOURCE        DECISION
catalog.read  file     read         ./data/catalog.json                               exact            app.rivet:38  allowed
users.get     network  connect GET  http://127.0.0.1:18840/users/{id}.json            param_dependent  app.rivet:16  allowed
users.page    network  connect GET  http://127.0.0.1:18840/users/index.json?limit=50  exact            app.rivet:27  allowed
3 allowed
```

```json
{"request_id":"req_01b06d5c8d","trace_id":"tr_01b06d5c8d","operation":"users.get","type":"result","status":"ok","data":{"id":7,"name":"Grace"},"error":null,"effects":"none","data_count":0}
{"request_id":"req_01ae632135","trace_id":"tr_01ae632135","operation":"users.page","type":"result","status":"ok","data":{"users":[{"id":7,"name":"Grace"}],"limit":50},"error":null,"effects":"none","data_count":0}
```

The failures: a missing user is `http.status` 404 (exit 5), and `min 1` is checked before any I/O (exit 2):

```json
{"request_id":"req_01ac460d0d","trace_id":"tr_01ac460d0d","operation":"users.get","type":"result","status":"error","data":null,"error":{"kind":"http","code":"http.status","message":"GET http://127.0.0.1:18840/users/8.json returned 404","retryable":false,"source":{"file":"app.rivet","line":16,"column":5,"end_line":19,"end_column":8},"operation_id":"users.get","details":{"status":404,"method":"GET"}},"effects":"none","data_count":0}
{"request_id":"req_01ab256605","trace_id":"tr_01ab256605","operation":"users.get","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.min","message":"parameter `id` must be ≥ 1","retryable":false,"operation_id":"users.get"},"effects":"none","data_count":0}
```

`http.log` shows the three requests that reached the server, with the `limit=50` query from the `page_size` global:

```text
127.0.0.1 - - [29/Sep/2026 06:39:37] "GET /users/7.json HTTP/1.1" 200 -
127.0.0.1 - - [29/Sep/2026 06:39:37] "GET /users/index.json?limit=50 HTTP/1.1" 200 -
127.0.0.1 - - [29/Sep/2026 06:39:37] "GET /users/8.json HTTP/1.1" 404 -
```

### 6. Failing examples: the six global diagnostics

#### Command / Request

```sh
cd docs/demos/14-globals          # back in the folder
for f in errors/*.rivet; do rivet --file "$f" check; echo "exit $?"; done
```

#### Expected Output / Response

Each file is refused at compile time with its own code, the exact span and exit 2. Nothing runs.

```text
  file                      code                          what is wrong
  ────────────────────────  ────────────────────────────  ──────────────────────────────────────────────
  errors/assign.rivet       check.global_assign           an operation assigns to a global
  errors/duplicate.rivet    check.global_duplicate        the same global declared twice
  errors/forward_ref.rivet  check.global_forward_ref      a global reads a global declared later
  errors/not_constant.rivet check.global_not_constant     a global reads the environment
  errors/shadow.rivet       check.global_shadow           a parameter reuses a global name
  errors/syntax.rivet       syntax.global                 `global 1st = 3`: not a name
```

```text
error[check.global_assign]: `page_size` is a global and globals are read-only
  --> errors/assign.rivet:6:5
   |
  6|     page_size = page_size + 1
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^
  = hint: assign to a new local name instead
exit 2
error[check.global_duplicate]: global `page_size` is declared twice in errors/duplicate.rivet
  --> errors/duplicate.rivet:2:8
   |
  2| global page_size = 100
   |        ^^^^^^^^^
exit 2
error[check.global_forward_ref]: global `users_url` reads `api`, which is not declared before it; globals see only earlier globals
  --> errors/forward_ref.rivet:1:20
   |
  1| global users_url = "${api}/users"
   |                    ^^^^^^^^^^^^^^
exit 2
error[check.global_not_constant]: global `token` cannot read the environment; globals are fixed at load time
  --> errors/not_constant.rivet:1:16
   |
  1| global token = (env "API_TOKEN")
   |                ^^^^^^^^^^^^^^^^^
  = hint: declare `secret token from env "…" for "https://…"` inside the operation that uses it
exit 2
error[check.global_shadow]: parameter `page_size` reuses the name of a global; globals cannot be shadowed
  --> errors/shadow.rivet:5:11
   |
  5|     param page_size integer default 10 description "Clashes with the global."
   |           ^^^^^^^^^
  = hint: rename the parameter
exit 2
error[syntax.global]: expected `global NAME = EXPR`; `1st` is not a global name (letters, digits and `_`)
  --> errors/syntax.rivet:1:8
   |
  1| global 1st = 3
   |        ^^^
exit 2
```

`check.global_not_constant` is why a global can never hold a secret: secrets stay `secret NAME from env "…" for "ORIGIN"` statements inside the operation that uses them.

### 7. Globals in the highlighter

#### Command / Request

```sh
rivet highlight app.rivet --format json | grep '"class":"global"' | head -3
```

#### Expected Output / Response

Every name declared by `global` is tokenized as `global` at its declaration and wherever an expression reads it (17 tokens in this file; a name inside `${…}` is part of an `interpolation` token):

```json
{"line":3,"col":8,"len":3,"class":"global","text":"api"}
{"line":4,"col":8,"len":9,"class":"global","text":"users_url"}
{"line":5,"col":8,"len":9,"class":"global","text":"page_size"}
```

The editor demo [16-editor](../16-editor/README.md) shows the ANSI and HTML forms.

## Release Updates

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-07 | UQ-07 / R7 | `global NAME = EXPR`: load-time, read-only constants shared by every operation | Step 2 | Values computed once (`users_url`, `retry_on.0`, `headers.accept`, `max_page`) | This README step 2 (2026-09-29, 8031baa); `tests/conformance_globals.rs` (T-06–T-08) |
| U-08 | UQ-07 / R8 | `syntax.global` and five `check.global_*` codes with spans, exit 2 | Step 6 | Six diagnostics, one per file | This README step 6 |
| U-09 | UQ-07 / R9 | Globals substituted in the manifest; literal-and-global targets are `exact`; exact grants in `policy generate` | Steps 3–5 | `…/users/index.json?limit=50` `exact`; draft with 2 exact grants | This README steps 3–5 |
| U-17 | UQ-01 / R17 | `global` token class in `rivet highlight` | Step 7 | `"class":"global"` | This README step 7 |

## Cleanup

```sh
kill $HS 2>/dev/null
cd - && rm -rf "$WORK"
```

Nothing is written to this folder.

## Verification Record

| Step | Verified By | Verified At | Result |
|---|---|---|---|
| 1. `check --strict-docs`, `list` | Claude (TASK-075) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 2. `settings.show` (compact and `--pretty`), `catalog.read` | Claude (TASK-075) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 3. `io --by target`, `io --check-policy` (0), `io --check-files` (0), `io --strict` (0), `policy explain --params` | Claude (TASK-075) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 3. (re-run after INC-2026-0012) `policy explain users.get --data '{"id":7}'` (exit 0): same table, `…/users/7.json` `exact` `allowed` | Claude | 2026-09-29, commit 7c25175, macOS 26.4.1 arm64 | PASS |
| 4. `policy generate` (2 exact grants) | Claude (TASK-075) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 5. `dns.resolve` in the folder; one-line retarget; GET 7, page, 404, `validation.min`; http.server log | Claude (TASK-075) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 6. Six global diagnostics (exit 2 each) | Claude (TASK-075) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 7. `highlight --format json` global tokens | Claude (TASK-075) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |

Verified on 0.2.0-dev at commit `8031baa`, the release candidate (`cargo build --release --workspace --all-features`). Every command above was executed from this folder or the scratch copy and the output pasted from that run. The step 3 `policy explain` command was re-run with `--data` on 2026-09-29 at commit `7c25175` (source = `14750b8`) after the INC-2026-0012 fixes.

## Known Caveats

- A target that contains a parameter stays `param_dependent` even when everything else comes from globals (PLAN-2026-0002 finding R9). The proposal's UC-04 sample showed `…/users/{id}` as `exact`; the build keeps the 0.1.0 rule, and `policy explain ID --data JSON` (alias `--params`) shows the concrete target.
- Globals are per file: an imported module has its own globals and cannot read the importer's ([17-modules](../17-modules/README.md)).
- Recorded on the release candidate before the version bump; the tagged `v0.2.0` build prints `rivet 0.2.0`.

## Related Documents

- [All sample folders](../README.md) · [demos index](../index.md) · [v0.2.0 release verification guide](../demo-2026-0020-v0-2-0-release-verification.md)
- [Language guide MAN-2026-0003](../../manuals/man-2026-0003-language-guide.md) · [Policy and I/O manifest guide MAN-2026-0005](../../manuals/man-2026-0005-policy-and-io-manifest-guide.md)
- [Error registry API-2026-0005](../../api/api-2026-0005-error-registry.md) · [Envelopes API-2026-0006](../../api/api-2026-0006-envelopes.md)
- [Proposal PROP-2026-0002](../../proposals/implemented/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md) · [Plan PLAN-2026-0002](../../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 3 | 2026-09-30 | Claude | v0.2.0 release (PLAN-2026-0002 TASK-091): version strings and current-release wording updated to 0.2.0. |
| 2 | 2026-09-29 | Claude | INC-2026-0012 re-verification (T-30) at 7c25175: step 3 `policy explain` uses `--data` (alias `--params`) and was re-run (output unchanged); caveat wording; one Verification Record row. |
| 1 | 2026-09-29 | Claude | TASK-075 (PLAN-2026-0002 D-17): new demo; seven globals across four operations, exact manifest and draft, one-line retarget against a local http.server, the six global diagnostics and highlight tokens; executed against 0.2.0-dev (8031baa). |
