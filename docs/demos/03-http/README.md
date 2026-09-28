---
document_id: DEMO-2026-0003
title: "HTTP requests and typed recovery"
document_type: demo
status: draft
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 4
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [registry]
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

# HTTP requests and typed recovery

**Draft usage sample.** Rivet has no runtime or CLI implementation yet. These files make the proposed interface concrete; commands below are intended usage, not executed demos.

## Purpose

HTTP requests and typed recovery. Delivery stage: **A**. Read [app.rivet](app.rivet) alongside this walkthrough.

| Operation ID | Output | Behavior |
|---|---|---|
| `users.get` | `object {id, name}` | GET with bounded retries for safe reads; an accepted 404 becomes declared error `users.not_found`. |
| `users.create` | `object {id, name}` | POST JSON without automatic retry. |
| `users.search` | `list json` | Encoded query parameters. |

```text
  users.get {id}
     |
     v
  GET https://api.example.com/users/${id}      ${id} is percent-encoded as ONE path segment
     |  retry 3 on 429/503, backoff "100ms".."2s"
     |  accept status [200, 404]
     +-- 200 --> return body  -> validated against output object {id, name}
     +-- 404 --> fail "users.not_found"   (declared with an error line)
     +-- 500 --> http.status (details.status 500), kind http, exit 5  (not caught)
```

## Verified Against Version

None. Based on proposal revision 8 semantics (UQ-17) and S01, S07–S08 and S78. No parser, runtime or network fixture execution is claimed.

## Prerequisites

A future Rivet build implementing this stage. External services below are controlled fixtures, not public services to contact. Commands assume the repository root initially, then the setup directory. Each folder is an independent bundle; do not concatenate folders with duplicate IDs.

## Setup

```sh
cd docs/demos/03-http
```

Provide a controlled HTTPS fixture for api.example.com with a trusted certificate, or replace every origin in app.rivet and [policy.json](policy.json) together. Fixture contract:

- `GET /users/42` returns `{"id":42,"name":"Ada"}`.
- `GET /users/404` returns 404.
- `POST /users` echoes the created object with an `id`.
- `/search` returns a list.

No fixture server is shipped.

[policy.json](policy.json) is auto-discovered and grants exactly `https://api.example.com:443`. `network.deny_private_ranges` is true (also the default). If the fixture resolves to a loopback or RFC1918 address, the connection is denied unless a grant names that IP literally.

## Steps

### Command / Request

```sh
rivet --file app.rivet request users.get --params '{"id":42}'
rivet --file app.rivet request users.get --params '{"id":404}'
rivet --file app.rivet request users.create --params '{"name":"Ada"}'
rivet --file app.rivet request users.search --params '{"query":"rust & capy"}'
```

View the declared outputs:

```sh
rivet --file app.rivet outputs users.get
rivet --file app.rivet outputs --all --json
```

### Expected Output / Response

`GET 42` returns `{"id":42,"name":"Ada"}` (exit 0). `GET 404` fails with declared application error `users.not_found` and `details.id: 404` (exit 5). Other upstream statuses surface as `http.status` with `details.status`, kind `http`, HTTP 502 and exit 5. Permission is exit 3 and timeout is exit 6. The ampersand in the search term stays query data. POST success returns the created object. An ambiguous network failure during POST reports `effects: "unknown"`.

`rivet outputs users.get`:

```text
users.get — Get a user
output  object   The requested user.
  id       integer  required  Stable user ID.
  name     text     required  Display name.
emits    —
receives —
errors
  users.not_found   The fixture service answered 404 for this ID.
```

`rivet outputs --all --json` (first entry shown):

```json
[
  {"id": "users.get",
   "output": {"type": "object", "description": "The requested user.",
              "properties": {"id": {"type": "integer", "description": "Stable user ID."},
                             "name": {"type": "string", "description": "Display name."}},
              "required": ["id", "name"], "additionalProperties": false},
   "emits": null, "receives": null,
   "errors": [{"code": "users.not_found", "description": "The fixture service answered 404 for this ID."}]}
]
```

If the fixture adds an extra field to a user, the closed output object rejects it with `output.invalid` (HTTP 500, exit 5). Effects are preserved in the error. Add `open true` to the output block to allow extra fields.

## Effects and policy

Only the declared HTTPS origin is authorized. DNS, TLS and connection attempts appear in the inventory. Redirects stay disabled; a different destination needs its own grant. `io` marks the `/users/${id}` target as `param_dependent`, because its value depends on a caller parameter.

## Inspect before invoking

```sh
rivet --file app.rivet check --strict-docs
rivet --file app.rivet io --by target
rivet --file app.rivet io --check-policy
```

`check --strict-docs` passes. Each public operation describes its params, output and output fields. The only `fail` code, `users.not_found`, is declared. Deleting its `error` line is a warning under plain `check` and an error (exit 2) under `--strict-docs`.

**`io --by target`** groups all three operations under one origin:

```text
TARGET                        ACCESS              CAPABILITY      ORIGIN                PHASE     NEEDS FILE   USED BY
https://api.example.com:443   connect GET, POST   allow_network   http get, http post   connect   —            users.create, users.get, users.search
```

**`io --check-policy`** shows each path template. `{id}` and `{query}` are caller parameters, so those sites are `param_dependent`. The origin grant in [policy.json](policy.json) covers every path, so all three are `allowed` and it exits 0:

```text
OPERATION      KIND      ACCESS         TARGET                                              KNOWLEDGE         SOURCE         DECISION
users.create   network   connect POST   https://api.example.com/users                       exact             app.rivet:29   allowed
users.get      network   connect GET    https://api.example.com/users/{id}                  param_dependent   app.rivet:10   allowed
users.search   network   connect GET    https://api.example.com/search?q={query}&limit=10   param_dependent   app.rivet:41   allowed
```

The `retry 3` block does not add sites. It is one site with up to four runtime attempts, visible through `io --trace REQ` on a live host. In JSON each site records `"method"`, `"protocol": "http1"` or `"http2"` as negotiated, and `"params": ["id"]` or `["query"]`. Component-aware encoding means `{id}` can never add a path segment, `?`, `#` or `@`.

`--include-bootstrap` adds the fixed runtime-internal list under a separate `bootstrap` key: the bundle and imports, policy.json, the CA bundle, resolv.conf or the system resolver, tzdata, descriptor/schema files, and stdin/stdout/stderr. It is listed for transparency, never granted to scripts. Sandbox guarantees apply to **script-initiated effects through brokered adapters**. `io` performs no I/O and evaluates no source expression. Exit codes: 3 when `--check-policy` finds a reachable site denied or partial; 7 with `--strict` when any site is dynamic or opaque (`complete: false`); otherwise 0.

## Release Updates

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-01 | UQ-17 / R23 | Declared outputs and error `users.not_found` | `rivet outputs users.get` | Table above | Not run — runtime does not exist |
| U-02 | UQ-17 / R24 | `--sandbox 'allow_network=…'` replaced by policy.json | Run without flags | Same results | Not run |
| U-03 | UQ-18 / R26 | `io` became the generated I/O manifest (targets, access verbs, capability, `--by`, `--check-policy`) | Inspect before invoking | Tables above | Not run — runtime does not exist |

## Cleanup

No local files are created. Stop your fixture service.

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
- [Proposal](../../proposals/approved/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 4 | 2026-09-28 | Claude | TASK-005/R26 (ADR-0001, proposal revision 8): `io --by target` gains ORIGIN (`http get, http post`), PHASE (`connect`) and NEEDS FILE (`—`). |
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest: origin-grouped `io --by target` and per-path `io --check-policy` (param_dependent, all allowed). |
| 2 | 2026-09-28 | Claude | UQ-17: declared outputs and error; 404 is mapped with `accept status` + `fail "users.not_found"` (upstream statuses are `http.status`); quoted durations; added policy.json in place of `--sandbox`; View outputs, strict-docs, bootstrap and exit codes. |
| 1 | 2026-09-28 | Codex | Added draft source files, prerequisites, invocation examples and expected behavior. |
