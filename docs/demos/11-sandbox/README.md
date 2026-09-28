---
document_id: DEMO-2026-0011
title: "Deny-by-default sandbox and I/O inventory"
document_type: demo
status: draft
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 3
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [examples]
affected_versions:
  from: not-applicable
  to: proposed-v0.1
applicable_environments: [development]
audience: [developers, reviewers]
scope: Illustrate the existing proposed interface with sample files; no runtime implementation or release claim.
reason: User requested sample files in folders with READMEs showing usage; UQ-17 (2026-09-28) adds declared outputs, policy.json-only policy and one serve for every surface; UQ-18 (2026-09-28) adds the generated I/O manifest and policy generate.
dependencies: [PROP-2026-0001, REF-2026-0002]
related_documents: [PROP-2026-0001, REF-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, examples, design]
confidentiality: internal
review_cycle: on-design-change
next_review_date: 2026-10-27
verified_against: not-implemented
---

# Deny-by-default sandbox and I/O inventory

**Draft usage sample.** Rivet has no runtime or CLI implementation yet. These files make the proposed interface concrete; commands below are intended usage, not executed demos.

## Purpose

Deny-by-default policy and I/O inventory. Delivery stage: **A**. Read [app.rivet](app.rivet) alongside this walkthrough.

| Operation ID | Output | Behavior |
|---|---|---|
| `demo.echo` | `text` | Pure work succeeds with no I/O grants. |
| `data.read` | `object {message}` | Read a granted file. |
| `data.private` | `object {message}` | Intentional deny case; declares `permission.denied`. |
| `data.snapshot` | `object {message}` | Transitive read plus exclusive write; declares `conflict.already_exists`. |

Four policy files show the same bundle under different authority. Only the file path is chosen on the command line; grants live in the files.

```text
                       demo.echo   data.read   data.private   data.snapshot
 policies/empty.json      ok        DENIED       DENIED          DENIED     ({"version":1} = deny-by-default)
 policies/read-only.json  ok        ok           DENIED          DENIED (no write)
 policies/create-only.json ok       ok           DENIED          ok, then conflict
                                                  (no grant)     (create only; never overwrites)
 policy.json (default)    ok        ok           DENIED (deny    ok, then conflict
                                                  overrides)     on repeat

 effective authority = host ceiling ∩ policy file ∩ per-request restriction (callers can only narrow)
```

## Verified Against Version

None. Based on proposal revision 5 semantics (UQ-17) and S62–S69 and S74–S77. No parser, runtime or network fixture execution is claimed.

## Prerequisites

A future Rivet build implementing this stage. External services below are controlled fixtures, not public services to contact. Commands assume the repository root initially, then the setup directory. Each folder is an independent bundle; do not concatenate folders with duplicate IDs.

## Setup

```sh
cd docs/demos/11-sandbox
mkdir -p out
```

JSON inputs and the policy files are included:

- [policy.json](policy.json) is auto-discovered and is the default.
- [policies/empty.json](policies/empty.json) contains no grants.
- [policies/read-only.json](policies/read-only.json) grants reads only. Its targets use `../data/**`, because targets are relative to the policy file's own directory.
- [policies/create-only.json](policies/create-only.json) narrows grants with `access`. It allows reading only `../data/public.json` and creating new files under `../out/**`, never updating or appending to them.

The private file is synthetic test data, not a secret.

## Steps

### Command / Request

```sh
rivet --file app.rivet --policy ./policies/empty.json request demo.echo --params '{"value":"ok"}'
rivet --file app.rivet --policy ./policies/empty.json request data.read --params '{}'
rivet --file app.rivet request data.read --params '{}'
rivet --file app.rivet request data.private --params '{}'
rivet --file app.rivet --policy ./policies/read-only.json policy explain data.snapshot --params '{}' --json
rivet --file app.rivet request data.snapshot --params '{}'
rivet --file app.rivet io --check-policy
rivet --file app.rivet io --strict --include-bootstrap --format json
```

To inspect a runtime trace, call through one long-lived host and query that same host with the returned request ID. The policy's `serve` block mounts only the `http` surface:

```sh
rivet --file app.rivet serve --listen 127.0.0.1:8080
```

In another terminal:

```sh
rivet --endpoint http://127.0.0.1:8080 request data.read --params '{}'
rivet --endpoint http://127.0.0.1:8080 trace show REPLACE_WITH_REQUEST_ID --json
rivet --endpoint http://127.0.0.1:8080 io data.read --trace REPLACE_WITH_REQUEST_ID
```

Every trace attempt carries its `effect_id`, so `io --trace` joins planned sites to actual attempts. It adds an `ATTEMPTS` column. The loopback principal `local` may call `rivet.io`; a network principal needs it listed explicitly in `serve.principals`:

```text
OPERATION   KIND   ACCESS   TARGET               KNOWLEDGE   SOURCE         ATTEMPTS
data.read   file   read     ./data/public.json   exact       app.rivet:15   1 (last: allowed)
```

View the declared outputs and errors:

```sh
rivet --file app.rivet outputs data.private
rivet --file app.rivet outputs --all --json
```

### Expected Output / Response

- Pure echo returns `"ok"` even under `empty.json` (exit 0).
- The read under `empty.json` and the policy-denied private read both fail with `permission.denied` (HTTP 403, exit 3) before any file content is accessed.
- The public read succeeds.
- `policy explain` under `read-only.json` names the missing `allow_write ./out/**`.
- The first snapshot succeeds. A repeated create fails with `conflict.already_exists` (exit 4).
- `io --check-policy` lists every site, including the denied `data.private` read and the transitive `data.read` call made by `data.snapshot`. It exits 3 because of that denial. The `--strict` manifest exits 0, because every target is a literal local path (`complete: true`). See [Inspect before invoking](#inspect-before-invoking) for every view.
- A malformed or unknown key in any policy file is `policy.invalid` (exit 2).

`rivet outputs data.private`:

```text
data.private — Try a denied read
output  object   Content of data/private/secret.json; never returned under the shipped policy.
  message  text     required  Synthetic fixture message.
emits    —
receives —
errors
  permission.denied   policy.json denies ./data/private/**; deny overrides grants.
```

`rivet outputs --all --json` (first entry shown):

```json
[
  {"id": "data.private",
   "output": {"type": "object", "description": "Content of data/private/secret.json; never returned under the shipped policy.",
              "properties": {"message": {"type": "string", "description": "Synthetic fixture message."}},
              "required": ["message"], "additionalProperties": false},
   "emits": null, "receives": null,
   "errors": [{"code": "permission.denied", "description": "policy.json denies ./data/private/**; deny overrides grants."}]}
]
```

## Effects and policy

Deny overrides grants. Policy comes only from a file: the auto-discovered policy.json, or the path given with `--policy`. There is no command-line grant syntax and no environment-variable policy. With no file at all, the behavior is the same as `policies/empty.json`. Write refuses files with a link count above 1 (`file.hardlink_refused`, exit 3). A pure call does not prove that arbitrary native host callbacks can be sandboxed.

## Inspect before invoking

```sh
rivet --file app.rivet check --strict-docs
rivet --file app.rivet io --check-policy
rivet --file app.rivet io --by target
rivet --file app.rivet io --by capability
rivet --file app.rivet io --kind file --access delete
rivet --file app.rivet io data.snapshot --check-policy --format json
rivet --file app.rivet policy generate
rivet --file app.rivet policy generate demo.echo data.read data.snapshot --output ./policy.draft.json
rivet --file app.rivet --policy ./policies/create-only.json io --check-policy
```

`check --strict-docs` passes: all four public operations describe their params, output and fields, and no body uses `fail`.

### From source to manifest

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

### By operation, checked against policy.json

`io --check-policy` evaluates each site against the auto-discovered [policy.json](policy.json):

```text
OPERATION       KIND   ACCESS   TARGET                       KNOWLEDGE   SOURCE         DECISION
data.private    file   read     ./data/private/secret.json   exact       app.rivet:26   denied
data.read       file   read     ./data/public.json           exact       app.rivet:15   allowed
data.snapshot   (calls data.read — see above)                                        app.rivet:37
data.snapshot   file   create   ./out/snapshot.json          exact       app.rivet:38   allowed
```

It exits **3**. `data.private` is denied, because the `deny` entry for `./data/private/**` overrides the `./data/**` grant. That is the demo's intended deny case. `demo.echo` has no rows.

### By target

```text
TARGET                       ACCESS   CAPABILITY    USED BY
./data/private/secret.json   read     allow_read    data.private
./data/public.json           read     allow_read    data.read, data.snapshot (via data.read)
./out/snapshot.json          create   allow_write   data.snapshot
```

### By capability

```text
allow_read
  ./data/private/secret.json   read     data.private
  ./data/public.json           read     data.read, data.snapshot (via data.read)
allow_write
  ./out/snapshot.json          create   data.snapshot
```

Only capabilities that have sites are shown. There is no `allow_delete`, `allow_network`, `allow_env` or other heading.

### Filter: who deletes files?

```text
$ rivet --file app.rivet io --kind file --access delete
OPERATION   KIND   ACCESS   TARGET   KNOWLEDGE   SOURCE
(no sites match kind=file access=delete)
```

It exits 0. No operation deletes anything, so the policy correctly has no `allow_delete` grant. An access verb from another kind (for example `--kind file --access connect`) is a usage error (exit 2).

### JSON excerpt

`io data.snapshot --check-policy --format json` returns the IoManifest. Both sites belong to the `data.snapshot` entry; the read is reached through `data.read`:

```json
{
  "bundle": {"file": "app.rivet", "sha256": "…"},
  "policy": {"file": "policy.json", "sha256": "…"},
  "complete": true,
  "sites": [
    {
      "effect_id": "data.read#1",
      "operation_id": "data.read",
      "kind": "file",
      "access": ["read"],
      "method": null,
      "protocol": null,
      "capability": "allow_read",
      "target": {"template": "./data/public.json", "scheme": null, "host": null, "port": null,
                 "path": "./data/public.json", "glob": null, "params": []},
      "knowledge": "exact",
      "condition": null,
      "call_chain": ["data.snapshot", "data.read"],
      "secrets": [],
      "source": {"file": "app.rivet", "line": 15, "column": 13},
      "decision": "allowed"
    },
    {
      "effect_id": "data.snapshot#1",
      "operation_id": "data.snapshot",
      "kind": "file",
      "access": ["create"],
      "method": null,
      "protocol": null,
      "capability": "allow_write",
      "target": {"template": "./out/snapshot.json", "scheme": null, "host": null, "port": null,
                 "path": "./out/snapshot.json", "glob": null, "params": []},
      "knowledge": "exact",
      "condition": null,
      "call_chain": ["data.snapshot"],
      "secrets": [],
      "source": {"file": "app.rivet", "line": 38, "column": 5},
      "decision": "allowed"
    }
  ],
  "targets": [
    {"target": "./data/public.json", "capability": "allow_read", "access": ["read"], "methods": [],
     "operations": ["data.snapshot"], "decision": "allowed"},
    {"target": "./out/snapshot.json", "capability": "allow_write", "access": ["create"], "methods": [],
     "operations": ["data.snapshot"], "decision": "allowed"}
  ],
  "bootstrap": []
}
```

`--format markdown` prints the same tables as Markdown for pasting into a review, and `--format csv` prints one row per site. Over HTTP the same manifest is `GET /v1/io?by=target&check_policy=true`; over MCP it is the built-in tool `rivet.io`.

### Generate a least-privilege draft and compare

`policy generate` builds a draft from the manifest of every public operation. It writes the draft to stdout and exits 0, because every site is `exact` and nothing needs review:

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
 allow_delete    —                                            —
 deny            —  (never generated)                         allow_read ./data/private/**
 network         deny_private_ranges: true                    default (true)
 serve           —  (never generated)                         surfaces [http], auth none
```

The draft grants exactly what the source uses, so it grants `data.private`'s secret read, which the hand-written file deliberately denies. The draft is a starting point for review, not an approval. Pass the operations you actually intend to allow to leave that grant out. With `--output`, the draft is written only if the path does not exist; otherwise the command fails with `conflict.exists` (exit 4) and never overwrites a reviewed policy. Delete `./policy.draft.json` after reviewing it. Dynamic or opaque sites are never granted: they are listed on stderr as review items, and the command exits 7. This bundle has none.

```text
$ rivet --file app.rivet policy generate demo.echo data.read data.snapshot
{"version": 1,
 "grants": [{"capability": "allow_read",  "targets": ["./data/public.json"],  "access": ["read"]},
            {"capability": "allow_write", "targets": ["./out/snapshot.json"], "access": ["create"]}],
 "network": {"deny_private_ranges": true}}
```

### Narrow a grant to specific access verbs

[policies/create-only.json](policies/create-only.json) is the reviewed, hand-edited form of that draft. It keeps the directory-wide `../out/**` target, so any new file may be created, but `access` limits writes to `create`:

```json
{
  "version": 1,
  "grants": [
    {"capability": "allow_read", "targets": ["../data/public.json"], "access": ["read"]},
    {"capability": "allow_write", "targets": ["../out/**"], "access": ["create"]}
  ]
}
```

```text
$ rivet --file app.rivet --policy ./policies/create-only.json io --check-policy
OPERATION       KIND   ACCESS   TARGET                       KNOWLEDGE   SOURCE         DECISION
data.private    file   read     ./data/private/secret.json   exact       app.rivet:26   denied
data.read       file   read     ./data/public.json           exact       app.rivet:15   allowed
data.snapshot   (calls data.read — see above)                                        app.rivet:37
data.snapshot   file   create   ./out/snapshot.json          exact       app.rivet:38   allowed
```

`data.private` is denied because nothing grants it, and the command exits 3. `io data.snapshot --check-policy` under this file exits 0. A `file update "./out/snapshot.json"` line would be denied twice over: its `stat` needs `allow_read` on `./out`, and `update` is not in the grant's `access` list. A repeated `request data.snapshot` still fails with `conflict.already_exists` (exit 4), because exclusive create never overwrites. A verb that does not belong to the capability, such as `{"capability": "allow_write", "targets": ["../out/**"], "access": ["delete"]}`, fails policy loading with `policy.invalid` (exit 2).

`--include-bootstrap` adds the fixed runtime-internal list under a separate `bootstrap` key: the bundle and imports, policy.json, the CA bundle, resolv.conf or the system resolver, tzdata, descriptor/schema files, and stdin/stdout/stderr. It is listed for transparency, never granted to scripts. Sandbox guarantees apply to **script-initiated effects through brokered adapters**. `io` performs no I/O and evaluates no source expression. Exit codes: 3 when `--check-policy` finds a reachable site denied or partial; 7 with `--strict` when any site is dynamic or opaque (`complete: false`); otherwise 0.

## Release Updates

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-01 | UQ-17 / R24 | `--sandbox ""` and `--sandbox 'allow_read=…'` became `policies/empty.json` and `policies/read-only.json` | Commands above | Same allow/deny results | Not run — runtime does not exist |
| U-02 | UQ-17 / R23 | Declared outputs and errors | `rivet outputs data.private` | Table above | Not run |
| U-03 | UQ-18 / R26 | `io` became the generated I/O manifest (targets, access verbs, capability, `--by`, `--check-policy`) | Inspect before invoking | Tables above | Not run — runtime does not exist |

## Cleanup

After review, remove only the generated `out/snapshot.json`, then the empty `out` directory, and `policy.draft.json` if you wrote one. Stop the host used for trace inspection; memory trace records disappear when it exits.

## Verification Record

| Step | Verified by | Date | Result |
|---|---|---|---|
| JSON/JSONL parse, relative links, manifest IDs vs declared public IDs, no removed flags or bare durations | Claude static script | 2026-09-28 | Passed as documentation checks; not language conformance |
| Source IDs, JSON fixtures, links and documentation structure | Codex static review | 2026-09-28 | Checked as documentation; not language conformance |
| Parse/execute and validate expected output | Future implementation fixture suite | Not run | BLOCKED — runtime does not exist |

## Known Caveats

Expected values assume the declared fixture behavior. Request, trace and session IDs are generated; compare application results rather than literal IDs. Runtime errors remain typed and retain partial-effect information.

## Related Documents

- [All sample folders](../README.md)
- [Usage reference](../../references/ref-2026-0002-language-and-usage.md)
- [Proposal](../../proposals/draft/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest walkthrough — source→manifest diagram, `io --check-policy`, `--by target`, `--by capability`, `--kind file --access delete`, `--format json` excerpt, `policy generate` compared with policy.json, new policies/create-only.json (`access: ["create"]` narrowing) and `io --trace` planned-vs-actual. |
| 2 | 2026-09-28 | Claude | UQ-17: removed `--sandbox` and intersection text; added policies/empty.json and policies/read-only.json; prefix `(request …)` call; declared outputs/errors; serve block in policy.json; View outputs, strict-docs, bootstrap and exit codes. |
| 1 | 2026-09-28 | Codex | Added draft source files, prerequisites, invocation examples and expected behavior. |
