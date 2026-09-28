---
document_id: DEMO-2026-0003
title: "HTTP requests and typed recovery"
document_type: demo
status: active
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 6
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, transports, policy, audit, cli, serve, http]
affected_versions:
  from: "0.2.0"
  to: null
applicable_environments: [development]
audience: [developers, reviewers]
scope: Runnable HTTP client demo — GET with bounded retries, an accepted 404 mapped to a declared error, POST without replay, encoded query parameters, output validation, trace attempts and the I/O manifest, against a shipped stdlib HTTP fixture.
reason: User requested sample files in folders with READMEs showing usage; UQ-17 adds declared outputs and policy.json-only policy; UQ-18 adds the generated I/O manifest; TASK-067 executed every step against the 0.1.0 release candidate. TASK-076 (PLAN-2026-0002) re-executed it against the 0.2.0 release candidate.
dependencies: [PROP-2026-0001, REF-2026-0002]
related_documents: [PLAN-2026-0001, DEMO-2026-0015, DEMO-2026-0013, MAN-2026-0008, API-2026-0005, TEST-2026-0005, TEST-2026-0009, TEST-2026-0020, TEST-2026-0025, PLAN-2026-0002, DEMO-2026-0020, MIG-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, demo, http, retry, errors, trace]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-28
verified_against: "0.2.0"
---

# HTTP requests and typed recovery

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0 and later
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

0.2.0. Verified on 0.2.0-dev at commit `8031baa`, the release candidate (the version string is bumped to 0.2.0 at release, P5), with `target/release/rivet` on macOS 26.4.1 (Darwin 25.4.0, arm64), 2026-09-29. Since 0.2.0 results and errors are ResponseEnvelopes and input is `--data` on the CLI and `{operation, data}` over HTTP ([migration guide](../../migrations/mig-2026-0001-response-and-input-envelopes.md)). Every output block below was pasted from that run. Request and trace IDs, hashes and ports vary.

## Prerequisites

```sh
cargo build --release --features cli       # from the repository root
export PATH="$PWD/target/release:$PATH"     # the release candidate prints rivet 0.1.0 until the P5 bump
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

`outputs --all --json` prints one `rivet.outputs` envelope on one line; its `data` holds three entries sorted by ID. The `users.get` entry reformatted:

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
rivet --file app.rivet request users.get --data '{"id":42}'
rivet --file app.rivet request users.get --data '{"id":7}'
rivet --file app.rivet request users.create --data '{"name":"Ada"}'
rivet --file app.rivet request users.search --data '{"query":"a"}'
rivet --file app.rivet request users.search --data '{"query":"rust & capy"}'
```

#### Expected Output / Response

All exit 0. `users.get 7` recovers from the fixture's 503 by the declared retry. POST reports `effects: "committed"`:

```json
{"request_id":"req_012c6f6ec5","trace_id":"tr_012c6f6ec5","operation":"users.get","type":"result","status":"ok","data":{"id":42,"name":"Ada"},"error":null,"effects":"none","data_count":0}
{"request_id":"req_012cb1824d","trace_id":"tr_012cb1824d","operation":"users.get","type":"result","status":"ok","data":{"id":7,"name":"Grace"},"error":null,"effects":"none","data_count":0}
{"request_id":"req_0126c73155","trace_id":"tr_0126c73155","operation":"users.create","type":"result","status":"ok","data":{"id":101,"name":"Ada"},"error":null,"effects":"committed","data_count":0}
{"request_id":"req_012577236d","trace_id":"tr_012577236d","operation":"users.search","type":"result","status":"ok","data":[{"id":7,"name":"Grace"},{"id":42,"name":"Ada"}],"error":null,"effects":"none","data_count":0}
{"request_id":"req_012454c6f5","trace_id":"tr_012454c6f5","operation":"users.search","type":"result","status":"ok","data":[],"error":null,"effects":"none","data_count":0}
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
rivet --file app.rivet request users.get --data '{"id":404}'
rivet --file app.rivet request users.get --data '{"id":500}'
rivet --file app.rivet request users.get --data '{"id":9}'
rivet --file app.rivet request users.get --data '{"id":0}'
```

#### Expected Output / Response

Declared application error (exit 5). Every failure is the same envelope with `status: "error"` and `data: null`:

```json
{"request_id":"req_012355c7cd","trace_id":"tr_012355c7cd","operation":"users.get","type":"result","status":"error","data":null,"error":{"kind":"application","code":"users.not_found","message":"The fixture service answered 404 for this ID.","retryable":false,"source":{"file":"app.rivet","line":16,"column":9,"end_line":16,"end_column":40},"operation_id":"users.get","details":{"id":404}},"effects":"none","data_count":0}
```

Unaccepted upstream status (exit 5):

```json
{"request_id":"req_0123ab7c7d","trace_id":"tr_0123ab7c7d","operation":"users.get","type":"result","status":"error","data":null,"error":{"kind":"http","code":"http.status","message":"GET http://127.0.0.1:18830/users/500 returned 500","retryable":false,"source":{"file":"app.rivet","line":10,"column":5,"end_line":14,"end_column":8},"operation_id":"users.get","details":{"status":500,"method":"GET"}},"effects":"none","data_count":0}
```

An extra field fails the closed output object (exit 5):

```json
{"request_id":"req_0122eae30d","trace_id":"tr_0122eae30d","operation":"users.get","type":"result","status":"error","data":null,"error":{"kind":"output_invalid","code":"output.invalid","message":"`users.get` returned a result that does not match its declared output: at role expected no such field, found unexpected field","retryable":false,"operation_id":"users.get","details":{"violations":[{"path":"role","expected":"no such field","found":"unexpected field"}],"unexpected":["role"]}},"effects":"none","data_count":0}
```

`min 1` is checked before any I/O (exit 2):

```json
{"request_id":"req_012114db9d","trace_id":"tr_012114db9d","operation":"users.get","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.min","message":"parameter `id` must be ≥ 1","retryable":false,"operation_id":"users.get"},"effects":"none","data_count":0}
```

### 6. Trace attempts through one host

The trace store belongs to the host that ran the request, so start `serve` and use the CLI as its client:

#### Command / Request

```sh
rivet --file app.rivet serve --listen 127.0.0.1:18831 2>serve.err & SV=$!
rivet --endpoint http://127.0.0.1:18831 request users.get --data '{"id":7}'
rivet --endpoint http://127.0.0.1:18831 trace show REQUEST_ID_FROM_ABOVE
rivet --endpoint http://127.0.0.1:18831 io users.get --trace REQUEST_ID_FROM_ABOVE
```

#### Expected Output / Response

```json
{"request_id":"req_02e923f772","trace_id":"tr_02e923f772","operation":"users.get","type":"result","status":"ok","data":{"id":7,"name":"Grace"},"error":null,"effects":"none","data_count":0}
```

`trace show` prints a `rivet.trace.show` envelope whose `data` lists one broker decision per attempt, both for effect `users.get#1` (one line, abbreviated):

```json
{"request_id":"req_035cad49b7","trace_id":"tr_035cad49b7","operation":"rivet.trace.show","type":"result","status":"ok","data":{"request_id":"req_02e923f772","attempts":[{"request_id":"req_02e923f772","trace_id":"tr_02e923f772","node_id":null,"attempt":1,"effect_id":"users.get#1","operation_id":"users.get","phase":"decision","capability":"allow_network","access":"connect","target":"http://127.0.0.1:18830/users/7","decision":"allowed","policy_hash":"sha256:443fa11c…","source":{"file":"app.rivet","line":10,"column":5},"outcome":{"rule":"grant allow_network http://127.0.0.1:18830"}},{…"attempt":2,…}, …],…},"error":null,"effects":"none","data_count":0}
```

`io --trace` joins planned sites with the recorded attempts:

```text
OPERATION  KIND     ACCESS       TARGET                             KNOWLEDGE        SOURCE        ATTEMPTS
users.get  network  connect GET  http://127.0.0.1:18830/users/{id}  param_dependent  app.rivet:10  2 allowed
```

Run in a separate CLI process instead (`rivet --file app.rivet trace show REQ`), the lookup fails with `not_found.trace` (exit 4), because each CLI process has its own in-memory trace store:

```json
{"request_id":"","trace_id":"","operation":"rivet.trace.show","type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"not_found.trace","message":"no trace for request `req_02e923f772` in this host's trace store","retryable":false},"effects":"none","data_count":0}
```

### 7. The same errors over HTTP

#### Command / Request

```sh
curl -sS -w ' %{http_code}\n' http://127.0.0.1:18831/v1/request -H 'content-type: application/json' -d '{"operation":"users.get","data":{"id":404}}'
curl -sS -w ' %{http_code}\n' http://127.0.0.1:18831/v1/request -H 'content-type: application/json' -d '{"operation":"users.get","data":{"id":500}}'
curl -sS -w ' %{http_code}\n' http://127.0.0.1:18831/v1/request -H 'content-type: application/json' -d '{"operation":"users.get","data":{"id":9}}'
curl -sS -w ' %{http_code}\n' http://127.0.0.1:18831/v1/request -H 'content-type: application/json' \
  -H 'traceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01' -d '{"operation":"users.get","data":{"id":42}}'
kill $SV $FX
```

#### Expected Output / Response

The error bodies equal the CLI ones; the statuses follow the error registry (`application` and `http` → 502, `output_invalid` → 500):

```text
{"request_id":"req_055fc66c79","trace_id":"tr_055fc66c79","operation":"users.get","type":"result","status":"error","data":null,"error":{"kind":"application","code":"users.not_found",…,"details":{"id":404}},"effects":"none","data_count":0} 502
{"request_id":"req_06dec111ce","trace_id":"tr_06dec111ce","operation":"users.get","type":"result","status":"error","data":null,"error":{"kind":"http","code":"http.status",…,"details":{"status":500,"method":"GET"}},"effects":"none","data_count":0} 502
{"request_id":"req_075de0c31b","trace_id":"tr_075de0c31b","operation":"users.get","type":"result","status":"error","data":null,"error":{"kind":"output_invalid","code":"output.invalid",…,"unexpected":["role"]}},"effects":"none","data_count":0} 500
{"request_id":"req_08dda33ad8","trace_id":"4bf92f3577b34da6a3ce929d0e0e4736","operation":"users.get","type":"result","status":"ok","data":{"id":42,"name":"Ada"},"error":null,"effects":"none","data_count":0} 200
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

0.2.0 updates shown here (numbering of the [v0.2.0 release verification guide](../demo-2026-0020-v0-2-0-release-verification.md)):

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-01 / U-02 | UQ-03/05 / R1, R2 | Results and errors are ResponseEnvelopes on the CLI and HTTP; the error object keeps `kind`/`code` | Steps 4, 5, 7 | Same codes and exits as 0.1.0; `status` ok/error; HTTP 502/500 unchanged | This README steps 4, 5, 7 (2026-09-29, 8031baa) |
| U-03 | UQ-03 / R3 | `outputs --json`, `trace show` are envelopes | Steps 1, 6 | `rivet.outputs`, `rivet.trace.show` envelopes | This README steps 1, 6 |
| U-04 | UQ-06 / R4 | `--data` and `{operation, data}` | Steps 4–7 | Same results | This README steps 4–7 |

Still verified from 0.1.0 (numbering of [DEMO-2026-0015](../demo-2026-0015-v0-1-0-release-verification.md)):

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-04 | UQ-03 / R4 | Typed errors with kind, code, exit and HTTP status | Steps 5, 7 | Codes, exits and statuses in the outcome table | This README steps 5, 7 (re-run 2026-09-29, 8031baa); TEST-2026-0024 |
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
| 1. `check --strict-docs`, `outputs`, `outputs --all --json` (envelope) | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 2. `io --by target`, `io --check-policy` (exit 0), `io --check-files` (exit 0) | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 3. Scratch copy, fixture, rewritten `io --check-policy` | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 4. GET, retried GET, POST, two searches with `--data`; fixture log | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 5. `users.not_found`, `http.status`, `output.invalid`, `validation.min` | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 6. `trace show` and `io --trace` through `--endpoint`; `not_found.trace` from a separate process | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| 7. HTTP 502 / 502 / 500 / 200 with traceparent, `{operation, data}` bodies | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |

Verified on 0.2.0-dev at commit `8031baa`, the release candidate (`cargo build --release --workspace --all-features`); every command above was executed from this folder or the scratch copy and the output pasted from that run. The 0.1.0 verification (TASK-067, commit 829ca43) is recorded in revision 5 below.

## Known Caveats

- The real HTTPS origin `api.example.com` is not contacted; the fixture is plain HTTP on loopback. TLS verification is covered by the HTTP/3 fixture in [09-quic](../09-quic/README.md) and by TEST-2026-0005.
- The inbound `traceparent` sets the envelope's `trace_id`; it is not forwarded on outbound HTTP calls (the fixture log shows `traceparent=-`).
- Traces live in the memory of the host that ran the request; use `--endpoint` against a running `serve` to read them.

## Related Documents

- [All sample folders](../README.md) · [demos index](../index.md) · [v0.2.0 release verification guide](../demo-2026-0020-v0-2-0-release-verification.md) · [v0.1.0 guide](../demo-2026-0015-v0-1-0-release-verification.md) · [envelope reference](../../api/api-2026-0006-envelopes.md)
- [Protocols and connectors manual](../../manuals/man-2026-0008-protocols-and-connectors.md) · [Error registry](../../api/api-2026-0005-error-registry.md)
- [Usage reference](../../references/ref-2026-0002-language-and-usage.md) · [Proposal](../../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 6 | 2026-09-29 | Claude | TASK-076 (PLAN-2026-0002 D-52): re-executed every step against the 0.2.0 release candidate (8031baa) with the local fixture; `--params` → `--data`, HTTP bodies `{operation, data}`; every result and error replaced by the 0.2.0 ResponseEnvelope; `trace show` shown as a `rivet.trace.show` envelope (attempts now carry `request_id`, `trace_id`, `node_id`) plus the `not_found.trace` envelope; 0.2.0 Release Updates; verified_against 0.2.0 |
| 5 | 2026-09-28 | Claude | TASK-067: added fixtures/users_fixture.py (port 18830) and a scratch-copy fixture run; executed every step against 0.1.0-dev (829ca43) and pasted real output: check, outputs, manifest (summary line), success and retry, `users.not_found`, `http.status`, `output.invalid`, `validation.min`, trace attempts via `--endpoint`, HTTP 502/500 and traceparent; removed draft disclaimers; status active; verified_against 0.1.0. |
| 4 | 2026-09-28 | Claude | TASK-005/R26 (ADR-0001, proposal revision 8): `io --by target` gains ORIGIN (`http get, http post`), PHASE (`connect`) and NEEDS FILE (`—`). |
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest: origin-grouped `io --by target` and per-path `io --check-policy` (param_dependent, all allowed). |
| 2 | 2026-09-28 | Claude | UQ-17: declared outputs and error; 404 is mapped with `accept status` + `fail "users.not_found"` (upstream statuses are `http.status`); quoted durations; added policy.json in place of `--sandbox`; View outputs, strict-docs, bootstrap and exit codes. |
| 1 | 2026-09-28 | Codex | Added draft source files, prerequisites, invocation examples and expected behavior. |
