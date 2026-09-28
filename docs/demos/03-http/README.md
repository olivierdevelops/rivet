---
document_id: DEMO-2026-0003
title: "HTTP requests and typed recovery"
document_type: demo
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 5
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, transports, policy, audit, cli, serve, http]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development]
audience: [developers, reviewers]
scope: Runnable HTTP client demo — GET with bounded retries, an accepted 404 mapped to a declared error, POST without replay, encoded query parameters, output validation, trace attempts and the I/O manifest, against a shipped stdlib HTTP fixture.
reason: User requested sample files in folders with READMEs showing usage; UQ-17 adds declared outputs and policy.json-only policy; UQ-18 adds the generated I/O manifest; TASK-067 executed every step against the 0.1.0 release candidate.
dependencies: [PROP-2026-0001, REF-2026-0002]
related_documents: [PLAN-2026-0001, DEMO-2026-0015, DEMO-2026-0013, MAN-2026-0008, API-2026-0005, TEST-2026-0005, TEST-2026-0009, TEST-2026-0020, TEST-2026-0025]
supersedes: null
superseded_by: null
tags: [rivet, demo, http, retry, errors, trace]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-28
verified_against: "0.1.0"
---

# HTTP requests and typed recovery

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** language, transports, policy, audit, cli, serve, http

## Purpose

HTTP requests and typed recovery. Delivery stage: **A**. Read [app.rivet](app.rivet) alongside this walkthrough.

| Operation ID | Output | Behavior |
|---|---|---|
| `users.get` | `object {id, name}` | GET with `retry 3 on status [429, 503]`; an accepted 404 becomes declared error `users.not_found`. |
| `users.create` | `object {id, name}` | POST JSON without automatic retry. |
| `users.search` | `list json` | Encoded query parameters `q` and `limit`. |

```text
  users.get {id}
     |
     v
  GET <origin>/users/${id}           ${id} is percent-encoded as ONE path segment
     |  retry 3 on 429/503, backoff "100ms".."2s"   (one site, several attempts)
     |  accept status [200, 404]
     +-- 200 --> return body --> validated against output object {id, name} (closed)
     |              extra field --> output.invalid (exit 5, HTTP 500)
     +-- 404 --> fail "users.not_found"   declared error (exit 5, HTTP 502)
     +-- 500 --> http.status details.status 500 (exit 5, HTTP 502), not caught
```

## Verified Against Version

0.1.0. Verified on 0.1.0-dev at commit `829ca43`, the release candidate (the version bump to 0.1.0 happens at release, P5), with `target/release/rivet` on macOS 26.4.1 (Darwin 25.4.0, arm64), 2026-09-28. Every output block below was pasted from that run. Request and trace IDs, hashes and ports vary.

## Prerequisites

```sh
cargo build --release                       # from the repository root
export PATH="$PWD/target/release:$PATH"     # `rivet --version` prints rivet 0.1.0
```

- `python3` (3.8+, standard library only) for [fixtures/users_fixture.py](fixtures/users_fixture.py).
- `curl` for step 7. Loopback port 18830 (fixture) and 18831 (serve) free.

## Setup

```sh
cd docs/demos/03-http
```

The bundle targets `https://api.example.com`, a placeholder for a controlled service. [policy.json](policy.json) grants exactly `https://api.example.com:443`. Steps 1–2 inspect the unmodified folder. Steps 3–7 run against the local fixture in a scratch copy where the origin is rewritten to plain HTTP on loopback. The fixture contract:

```text
  GET  /users/42      200 {"id":42,"name":"Ada"}
  GET  /users/7       503 on the first attempt of each call, then 200 {"id":7,"name":"Grace"}
  GET  /users/9       200 {"id":9,"name":"Linus","role":"admin"}      (extra field)
  GET  /users/500     500 {"error":"boom"}
  GET  /users/<other> 404
  POST /users         201 {"id":101,"name":<name>}
  GET  /search?q=..   200 users whose name contains q
```

## Steps

### 1. Check the bundle and view outputs

#### Command / Request

```sh
rivet --file app.rivet check --strict-docs
rivet --file app.rivet outputs users.get
rivet --file app.rivet outputs --all --json
```

#### Expected Output / Response

```text
ok: 3 operations, 0 connectors, 0 auth profiles
```

```text
users.get — Get a user
output  object   The requested user.
  id      integer  required  Stable user ID.
  name    text     required  Display name.
emits    —
receives —
errors
  users.not_found   The fixture service answered 404 for this ID.
```

`outputs --all --json` prints one line with three entries sorted by ID; the `users.get` entry reformatted:

```json
{"id":"users.get",
 "output":{"type":"object","properties":{"id":{"type":"integer","description":"Stable user ID."},
                                         "name":{"type":"string","description":"Display name."}},
           "required":["id","name"],"additionalProperties":false,"description":"The requested user."},
 "emits":null,"receives":null,
 "errors":[{"code":"users.not_found","description":"The fixture service answered 404 for this ID."}]}
```

All exit 0. The only `fail` code is declared; deleting its `error` line makes `--strict-docs` exit 2.

### 2. I/O manifest

#### Command / Request

```sh
rivet --file app.rivet io --by target
rivet --file app.rivet io --check-policy
rivet --file app.rivet io --check-files
```

#### Expected Output / Response

```text
TARGET                       ACCESS             CAPABILITY     ORIGIN               PHASE    NEEDS FILE  USED BY
https://api.example.com:443  connect GET, POST  allow_network  http get, http post  connect  —           users.create, users.get, users.search
```

```text
OPERATION     KIND     ACCESS        TARGET                                             KNOWLEDGE        SOURCE        DECISION
users.create  network  connect POST  https://api.example.com/users                      exact            app.rivet:29  allowed
users.get     network  connect GET   https://api.example.com/users/{id}                 param_dependent  app.rivet:10  allowed
users.search  network  connect GET   https://api.example.com/search?q={query}&limit=10  param_dependent  app.rivet:41  allowed
3 allowed
```

`io --check-files` prints `… needs no existing files.` for each operation and `0 files`. All exit 0. In `--format json` each site carries `"method":"GET"`, `"protocol":"http1|http2"` and `"params":["id"]`. The `retry 3` block adds no sites; it is one site with up to four runtime attempts (step 6).

### 3. Local fixture run: rewrite the origin

#### Command / Request

```sh
WORK="$(mktemp -d)"
cp -R app.rivet policy.json fixtures "$WORK/"
cd "$WORK"
sed -i.bak -e 's#https://api\.example\.com:443#http://127.0.0.1:18830#g' \
           -e 's#https://api\.example\.com#http://127.0.0.1:18830#g' app.rivet policy.json
python3 fixtures/users_fixture.py --port 18830 2>fixture.log & FX=$!
rivet --file app.rivet io --check-policy
```

#### Expected Output / Response

The literal loopback grant `http://127.0.0.1:18830` passes `deny_private_ranges` (exit 0):

```text
OPERATION     KIND     ACCESS        TARGET                                            KNOWLEDGE        SOURCE        DECISION
users.create  network  connect POST  http://127.0.0.1:18830/users                      exact            app.rivet:29  allowed
users.get     network  connect GET   http://127.0.0.1:18830/users/{id}                 param_dependent  app.rivet:10  allowed
users.search  network  connect GET   http://127.0.0.1:18830/search?q={query}&limit=10  param_dependent  app.rivet:41  allowed
3 allowed
```

### 4. Successful calls

#### Command / Request

```sh
rivet --file app.rivet request users.get --params '{"id":42}'
rivet --file app.rivet request users.get --params '{"id":7}'
rivet --file app.rivet request users.create --params '{"name":"Ada"}'
rivet --file app.rivet request users.search --params '{"query":"a"}'
rivet --file app.rivet request users.search --params '{"query":"rust & capy"}'
```

#### Expected Output / Response

All exit 0. `users.get 7` recovers from the fixture's 503 by the declared retry. POST reports `effects: "committed"`:

```json
{"request_id":"req_0185cfeafd","trace_id":"tr_0185cfeafd","result":{"id":42,"name":"Ada"},"data_count":0,"effects":"none"}
{"request_id":"req_0183c2eecd","trace_id":"tr_0183c2eecd","result":{"id":7,"name":"Grace"},"data_count":0,"effects":"none"}
{"request_id":"req_017dd3bbe5","trace_id":"tr_017dd3bbe5","result":{"id":101,"name":"Ada"},"data_count":0,"effects":"committed"}
{"request_id":"req_017ca06195","trace_id":"tr_017ca06195","result":[{"id":7,"name":"Grace"},{"id":42,"name":"Ada"}],"data_count":0,"effects":"none"}
{"request_id":"req_017c72f06d","trace_id":"tr_017c72f06d","result":[],"data_count":0,"effects":"none"}
```

`fixture.log` shows the retry and the encoded query (the `&` stays query data):

```text
fixture GET /users/42 -> 200 traceparent=-
fixture GET /users/7 -> 503 traceparent=-
fixture GET /users/7 -> 200 traceparent=-
fixture POST /users -> 201 traceparent=-
fixture GET /search?q=a&limit=10 -> 200 traceparent=-
fixture GET /search?q=rust+%26+capy&limit=10 -> 200 traceparent=-
```

### 5. Failing calls

#### Command / Request

```sh
rivet --file app.rivet request users.get --params '{"id":404}'
rivet --file app.rivet request users.get --params '{"id":500}'
rivet --file app.rivet request users.get --params '{"id":9}'
rivet --file app.rivet request users.get --params '{"id":0}'
```

#### Expected Output / Response

Declared application error (exit 5):

```json
{"request_id":"req_018492dba5","trace_id":"tr_018492dba5","error":{"kind":"application","code":"users.not_found","message":"The fixture service answered 404 for this ID.","retryable":false,"effects":"none","source":{"file":"app.rivet","line":16,"column":9,"end_line":16,"end_column":40},"operation_id":"users.get","details":{"id":404}}}
```

Unaccepted upstream status (exit 5):

```json
{"request_id":"req_017e2d4f85","trace_id":"tr_017e2d4f85","error":{"kind":"http","code":"http.status","message":"GET http://127.0.0.1:18830/users/500 returned 500","retryable":false,"effects":"none","source":{"file":"app.rivet","line":10,"column":5,"end_line":14,"end_column":8},"operation_id":"users.get","details":{"status":500,"method":"GET"}}}
```

An extra field fails the closed output object (exit 5):

```json
{"request_id":"req_017ff52afd","trace_id":"tr_017ff52afd","error":{"kind":"output_invalid","code":"output.invalid","message":"`users.get` returned a result that does not match its declared output: at role expected no such field, found unexpected field","retryable":false,"effects":"none","operation_id":"users.get","details":{"violations":[{"path":"role","expected":"no such field","found":"unexpected field"}],"unexpected":["role"]}}}
```

`min 1` is checked before any I/O (exit 2):

```json
{"request_id":"req_017d4cdfed","trace_id":"tr_017d4cdfed","error":{"kind":"validation","code":"validation.min","message":"parameter `id` must be ≥ 1","retryable":false,"effects":"none","operation_id":"users.get"}}
```

### 6. Trace attempts through one host

The trace store belongs to the host that ran the request, so start `serve` and use the CLI as its client:

#### Command / Request

```sh
rivet --file app.rivet serve --listen 127.0.0.1:18831 2>serve.err & SV=$!
rivet --endpoint http://127.0.0.1:18831 request users.get --params '{"id":7}'
rivet --endpoint http://127.0.0.1:18831 trace show REQUEST_ID_FROM_ABOVE
rivet --endpoint http://127.0.0.1:18831 io users.get --trace REQUEST_ID_FROM_ABOVE
```

#### Expected Output / Response

```json
{"request_id":"req_01b95dca25","trace_id":"tr_01b95dca25","result":{"id":7,"name":"Grace"},"data_count":0,"effects":"none"}
```

`trace show` lists one broker decision per attempt, both for effect `users.get#1` (one line, abbreviated):

```json
{"request_id":"req_01b95dca25","attempts":[{"attempt":1,"effect_id":"users.get#1","operation_id":"users.get","phase":"decision","capability":"allow_network","access":"connect","target":"http://127.0.0.1:18830/users/7","decision":"allowed","policy_hash":"sha256:443fa11c…","source":{"file":"app.rivet","line":10,"column":5},"outcome":{"rule":"grant allow_network http://127.0.0.1:18830"}, …},{"attempt":2, …}],"complete":true,"next_cursor":null,"gaps":0}
```

`io --trace` joins planned sites with the recorded attempts:

```text
OPERATION  KIND     ACCESS       TARGET                             KNOWLEDGE        SOURCE        ATTEMPTS
users.get  network  connect GET  http://127.0.0.1:18830/users/{id}  param_dependent  app.rivet:10  2 allowed
```

Run in a separate CLI process instead (`rivet --file app.rivet trace show REQ`), the lookup fails with `not_found.trace` (exit 4), because each CLI process has its own in-memory trace store.

### 7. The same errors over HTTP

#### Command / Request

```sh
curl -sS -w ' %{http_code}\n' http://127.0.0.1:18831/v1/request -H 'content-type: application/json' -d '{"id":"users.get","params":{"id":404}}'
curl -sS -w ' %{http_code}\n' http://127.0.0.1:18831/v1/request -H 'content-type: application/json' -d '{"id":"users.get","params":{"id":500}}'
curl -sS -w ' %{http_code}\n' http://127.0.0.1:18831/v1/request -H 'content-type: application/json' -d '{"id":"users.get","params":{"id":9}}'
curl -sS -w ' %{http_code}\n' http://127.0.0.1:18831/v1/request -H 'content-type: application/json' \
  -H 'traceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01' -d '{"id":"users.get","params":{"id":42}}'
kill $SV $FX
```

#### Expected Output / Response

The error bodies equal the CLI ones; the statuses follow the error registry (`application` and `http` → 502, `output_invalid` → 500):

```text
{"request_id":"req_0135efe92d","trace_id":"tr_0135efe92d","error":{"kind":"application","code":"users.not_found",…,"details":{"id":404}}} 502
{"request_id":"req_0331645edf","trace_id":"tr_0331645edf","error":{"kind":"http","code":"http.status",…,"details":{"status":500,"method":"GET"}}} 502
{"request_id":"req_04b08b4c74","trace_id":"tr_04b08b4c74","error":{"kind":"output_invalid","code":"output.invalid",…,"unexpected":["role"]}}} 500
{"request_id":"req_02b55be162","trace_id":"4bf92f3577b34da6a3ce929d0e0e4736","result":{"id":42,"name":"Ada"},"data_count":0,"effects":"none"} 200
```

## Effects and policy

```text
  outcome table (verified in steps 4–7)
  ┌─────────────────────────────────────┬───────────────────┬──────┬──────┬───────────┐
  │ situation                           │ code              │ exit │ HTTP │ effects   │
  ├─────────────────────────────────────┼───────────────────┼──────┼──────┼───────────┤
  │ GET 200 / search                    │ —                 │  0   │ 200  │ none      │
  │ GET 503 then 200 (declared retry)   │ —                 │  0   │ 200  │ none      │
  │ POST 201                            │ —                 │  0   │ 200  │ committed │
  │ accepted 404 → fail                 │ users.not_found   │  5   │ 502  │ none      │
  │ unaccepted 500                      │ http.status       │  5   │ 502  │ none      │
  │ extra field in a closed output      │ output.invalid    │  5   │ 500  │ none      │
  │ id below min 1                      │ validation.min    │  2   │ 422  │ none      │
  └─────────────────────────────────────┴───────────────────┴──────┴──────┴───────────┘
```

Only the declared origin is authorized. Redirects stay disabled unless the block says `redirect follow limit N`; a different destination needs its own grant. Component-aware encoding means `${id}` can never add a path segment, `?`, `#` or `@`.

## Release Updates

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-04 | UQ-03 / R4 | Typed errors with kind, code, exit and HTTP status | Steps 5, 7 | Codes, exits and statuses in the outcome table | This README steps 5, 7 (2026-09-28, 829ca43); TEST-2026-0024 |
| U-06 | UQ-05/18 / R6 | Trace attempts carry `effect_id` | Step 6 | Two attempts for `users.get#1` | This README step 6; TEST-2026-0009 |
| U-12 | UQ-01/12 / R12 | HTTP client with retry, accept, decode, query encoding | Steps 4–5 | Results above | This README steps 4–5; TEST-2026-0005 |
| U-23 | UQ-17 / R23 | Declared outputs validated at return | Steps 1, 5 | `output.invalid` for an extra field | This README steps 1, 5; TEST-2026-0020 |
| U-26 | UQ-18 / R26 | Generated I/O manifest | Step 2 | Tables above | This README step 2; TEST-2026-0025 |

## Cleanup

```sh
kill $SV $FX 2>/dev/null
cd - && rm -rf "$WORK"
```

Nothing is written to this folder.

## Verification Record

| Step | Verified By | Verified At | Result |
|---|---|---|---|
| 1. `check --strict-docs`, `outputs` | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 2. `io --by target`, `io --check-policy` (exit 0), `io --check-files` (exit 0) | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 3. Scratch copy, fixture, rewritten `io --check-policy` | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 4. GET, retried GET, POST, two searches | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 5. `users.not_found`, `http.status`, `output.invalid`, `validation.min` | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 6. `trace show` and `io --trace` through `--endpoint` | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 7. HTTP 502 / 502 / 500 / 200 with traceparent | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |

Build: `cargo build` and `cargo build --release` at 829ca43; every command above was executed from this folder or the scratch copy and the output pasted from that run.

## Known Caveats

- The real HTTPS origin `api.example.com` is not contacted; the fixture is plain HTTP on loopback. TLS verification is covered by the HTTP/3 fixture in [09-quic](../09-quic/README.md) and by TEST-2026-0005.
- The inbound `traceparent` sets the Completion's `trace_id`; it is not forwarded on outbound HTTP calls (the fixture log shows `traceparent=-`).
- Traces live in the memory of the host that ran the request; use `--endpoint` against a running `serve` to read them.

## Related Documents

- [All sample folders](../README.md) · [demos index](../index.md) · [release verification guide](../demo-2026-0015-v0-1-0-release-verification.md)
- [Protocols and connectors manual](../../manuals/man-2026-0008-protocols-and-connectors.md) · [Error registry](../../api/api-2026-0005-error-registry.md)
- [Usage reference](../../references/ref-2026-0002-language-and-usage.md) · [Proposal](../../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 5 | 2026-09-28 | Claude | TASK-067: added fixtures/users_fixture.py (port 18830) and a scratch-copy fixture run; executed every step against 0.1.0-dev (829ca43) and pasted real output: check, outputs, manifest (summary line), success and retry, `users.not_found`, `http.status`, `output.invalid`, `validation.min`, trace attempts via `--endpoint`, HTTP 502/500 and traceparent; removed draft disclaimers; status active; verified_against 0.1.0. |
| 4 | 2026-09-28 | Claude | TASK-005/R26 (ADR-0001, proposal revision 8): `io --by target` gains ORIGIN (`http get, http post`), PHASE (`connect`) and NEEDS FILE (`—`). |
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest: origin-grouped `io --by target` and per-path `io --check-policy` (param_dependent, all allowed). |
| 2 | 2026-09-28 | Claude | UQ-17: declared outputs and error; 404 is mapped with `accept status` + `fail "users.not_found"` (upstream statuses are `http.status`); quoted durations; added policy.json in place of `--sandbox`; View outputs, strict-docs, bootstrap and exit codes. |
| 1 | 2026-09-28 | Codex | Added draft source files, prerequisites, invocation examples and expected behavior. |
