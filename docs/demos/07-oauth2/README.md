---
document_id: DEMO-2026-0007
title: "OAuth 2.0 without returning tokens"
document_type: demo
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 5
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [auth, policy, audit, transports, cli, serve, http]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development]
audience: [developers, reviewers]
scope: Runnable OAuth 2.0 client_credentials demo — token acquisition with a host-held secret, a bound resource origin, credential reuse in a long-lived host, typed failures (missing secret, rejected client, missing grant, slow token endpoint) and proof that tokens never appear in results, traces or logs, against a shipped stdlib OAuth fixture.
reason: User requested sample files in folders with READMEs showing usage; UQ-14 asks for OAuth 2.0; UQ-17 adds declared outputs and policy.json-only policy; TASK-067 executed every step against the 0.1.0 release candidate.
dependencies: [PROP-2026-0001, REF-2026-0002]
related_documents: [PLAN-2026-0001, DEMO-2026-0015, DEMO-2026-0013, MAN-2026-0008, TEST-2026-0011, TEST-2026-0015, TEST-2026-0021, TEST-2026-0025]
supersedes: null
superseded_by: null
tags: [rivet, demo, oauth2, auth, secrets]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-28
verified_against: "0.1.0"
---

# OAuth 2.0 without returning tokens

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** auth, policy, audit, transports, cli, serve, http

## Purpose

OAuth 2.0 without returning tokens. Delivery stage: **B**. Read [app.rivet](app.rivet) alongside this walkthrough.

| Operation ID | Output | Behavior |
|---|---|---|
| `contacts.list` | `object {contacts}`, open | Acquire a client_credentials token and call the protected contacts endpoint. |

```text
  contacts.list
     |  auth crm_service account "service"
     v
  broker ──allow_auth crm_service/service/use──> profile crm_service
     |        ──allow_credentials crm_service/service──> memory store (cache hit?)
     |        ──allow_env CRM_CLIENT_SECRET──> client secret      (only on miss/refresh)
     |        ──allow_network <auth origin>──> POST /token (client_auth basic)
     v
  GET <api origin>/contacts   (token attached only for bound resource_origins)
     v
  output {contacts: [...]}   -- the token never appears in results, traces or logs
```

## Verified Against Version

0.1.0. Verified on 0.1.0-dev at commit `829ca43`, the release candidate (the version bump to 0.1.0 happens at release, P5), with `target/release/rivet` on macOS 26.4.1 (Darwin 25.4.0, arm64), 2026-09-28. Every output block below was pasted from that run. Request and trace IDs, hashes and ports vary.

## Prerequisites

```sh
cargo build --release                       # from the repository root
export PATH="$PWD/target/release:$PATH"     # `rivet --version` prints rivet 0.1.0-dev until the P5 bump
```

- `python3` (standard library only) for [fixtures/oauth_fixture.py](fixtures/oauth_fixture.py): an authorization server (`POST /token`, client `rivet-service`, secret `demo-secret-not-real`, tokens `DEMO-AT-<n>`) and a resource server (`GET /contacts`, accepts only issued tokens). `DEMO_FIXTURE_TOKEN_DELAY=5` makes `/token` answer late.
- `curl`. Loopback ports 18870 (auth), 18871 (API) and 18872 (serve) free.

## Setup

```sh
cd docs/demos/07-oauth2
```

[policy.json](policy.json) is auto-discovered. It separates the five authorities a token-backed call needs, and mounts only the `http` and `mcp` surfaces with loopback auth `none`:

```text
  allow_auth         crm_service/service/use     use the profile for account "service"
  allow_credentials  crm_service/service         read/write the memory credential store
  allow_env          CRM_CLIENT_SECRET           read the client secret (only on miss/refresh)
  allow_network      https://auth.example.com:443, https://api.example.com:443
  serve              surfaces [http, mcp], auth none
```

`auth.example.com` and `api.example.com` are placeholders. Steps 1–2 inspect this folder; steps 3–7 run against the fixture in a scratch copy. The fixture secret is not a real credential; never put a real one in this folder or in shell history.

## Steps

### 1. Check the bundle and view outputs

#### Command / Request

```sh
rivet --file app.rivet check --strict-docs
rivet --file app.rivet outputs contacts.list
rivet --file app.rivet outputs --all --json
```

#### Expected Output / Response

```text
ok: 1 operations, 0 connectors, 1 auth profiles
```

```text
contacts.list — List protected contacts
output  object   The contacts API payload; never contains the access token.
  contacts  list json required  Contact objects returned by the resource server.
  (open: extra fields allowed)
emits    —
receives —
errors   —
```

```json
[{"id":"contacts.list","output":{"type":"object","properties":{"contacts":{"type":"array","items":{},"description":"Contact objects returned by the resource server."}},"required":["contacts"],"additionalProperties":true,"description":"The contacts API payload; never contains the access token."},"emits":null,"receives":null,"errors":[]}]
```

All exit 0.

### 2. I/O manifest: every effect a token may need

#### Command / Request

```sh
rivet --file app.rivet io --by target
rivet --file app.rivet io --check-policy
rivet --file app.rivet io --check-files
rivet --file app.rivet policy explain contacts.list --params '{}' --json
```

#### Expected Output / Response

One `http get … auth crm_service` expands into every effect an acquisition or refresh may need, listed even when a cached token would be reused:

```text
TARGET                        ACCESS        CAPABILITY         ORIGIN             PHASE    NEEDS FILE  USED BY
env CRM_CLIENT_SECRET         read          allow_env          client_secret env  body     —           contacts.list (secret, bound to https://auth.example.com:443)
crm_service/service           use           allow_auth         auth               body     —           contacts.list
crm_service/service           read, write   allow_credentials  auth               body     —           contacts.list
https://auth.example.com:443  connect POST  allow_network      token_url          connect  —           contacts.list (token endpoint)
https://api.example.com:443   connect GET   allow_network      http get           connect  —           contacts.list
```

`--check-policy` (exit 0):

```text
OPERATION      KIND        ACCESS        TARGET                            KNOWLEDGE  SOURCE        DECISION
contacts.list  env         read          env CRM_CLIENT_SECRET             exact      app.rivet:6   allowed
contacts.list  credential  read          crm_service/service               exact      app.rivet:21  allowed
contacts.list  credential  write         crm_service/service               exact      app.rivet:21  allowed
contacts.list  auth        use           crm_service/service               exact      app.rivet:21  allowed
contacts.list  network     connect POST  https://auth.example.com/token    exact      app.rivet:4   allowed
contacts.list  network     connect GET   https://api.example.com/contacts  exact      app.rivet:20  allowed
6 allowed
```

`io --check-files` prints `contacts.list needs no existing files.` and `0 files` (exit 0). `policy explain --json` (exit 0) lists the six sites with `"decision":"allowed"`; the env site carries `"origin":{"option":"client_secret env"}` and `"secret":true`. The manifest never contains a secret value or a token.

### 3. Local fixture run: rewrite both origins

#### Command / Request

```sh
WORK="$(mktemp -d)"; cp -R app.rivet policy.json fixtures "$WORK/"; cd "$WORK"
sed -i.bak -e 's#https://auth\.example\.com:443#http://127.0.0.1:18870#g' -e 's#https://auth\.example\.com#http://127.0.0.1:18870#g' \
           -e 's#https://api\.example\.com:443#http://127.0.0.1:18871#g' -e 's#https://api\.example\.com#http://127.0.0.1:18871#g' \
           app.rivet policy.json
python3 fixtures/oauth_fixture.py 18870 18871 2>fixture.log & FX=$!
rivet --file app.rivet io --check-policy
```

#### Expected Output / Response

`issuer`, `token_url`, `resource_origins` and both grants now name the loopback fixture literally (plain `http` is accepted only because both origins are loopback). Exit 0:

```text
OPERATION      KIND        ACCESS        TARGET                           KNOWLEDGE  SOURCE        DECISION
contacts.list  env         read          env CRM_CLIENT_SECRET            exact      app.rivet:6   allowed
contacts.list  credential  read          crm_service/service              exact      app.rivet:21  allowed
contacts.list  credential  write         crm_service/service              exact      app.rivet:21  allowed
contacts.list  auth        use           crm_service/service              exact      app.rivet:21  allowed
contacts.list  network     connect POST  http://127.0.0.1:18870/token     exact      app.rivet:4   allowed
contacts.list  network     connect GET   http://127.0.0.1:18871/contacts  exact      app.rivet:20  allowed
6 allowed
```

### 4. Secret missing, secret wrong, secret right

#### Command / Request

```sh
unset CRM_CLIENT_SECRET
rivet --file app.rivet request contacts.list --params '{}'
export CRM_CLIENT_SECRET=wrong-secret
rivet --file app.rivet request contacts.list --params '{}'
export CRM_CLIENT_SECRET=demo-secret-not-real
rivet --file app.rivet request contacts.list --params '{}'
```

#### Expected Output / Response

No secret: the token endpoint is never contacted (exit 5):

```json
{"request_id":"req_011fc9e8f5","trace_id":"tr_011fc9e8f5","error":{"kind":"application","code":"auth.client_secret_missing","message":"auth profile `crm_service`: client secret env CRM_CLIENT_SECRET is not set","retryable":false,"effects":"none","source":{"file":"app.rivet","line":20,"column":5,"end_line":23,"end_column":8},"operation_id":"contacts.list"}}
```

Rejected client (exit 5):

```json
{"request_id":"req_011e061b1d","trace_id":"tr_011e061b1d","error":{"kind":"application","code":"auth.token_endpoint_failed","message":"the token endpoint answered 401 (invalid_client)","retryable":false,"effects":"none","source":{"file":"app.rivet","line":20,"column":5,"end_line":23,"end_column":8},"operation_id":"contacts.list","details":{"status":401,"error":"invalid_client"}}}
```

Correct secret (exit 0); the result holds the contacts, never the token:

```json
{"request_id":"req_011d536575","trace_id":"tr_011d536575","result":{"contacts":[{"name":"Ada Lovelace","email":"ada@example.com"},{"name":"Grace Hopper","email":"grace@example.com"}]},"data_count":0,"effects":"none"}
```

### 5. Credential reuse in a long-lived host

#### Command / Request

```sh
rivet --file app.rivet serve --listen 127.0.0.1:18872 2>serve.err & SV=$!
rivet --endpoint http://127.0.0.1:18872 request contacts.list --params '{}'
rivet --endpoint http://127.0.0.1:18872 request contacts.list --params '{}'
rivet --endpoint http://127.0.0.1:18872 trace show REQUEST_ID_OF_THE_SECOND_CALL
curl -sS -o /dev/null -w 'ws %{http_code}\n' http://127.0.0.1:18872/v1/ws
rivet --endpoint http://127.0.0.1:18872 auth status crm_service --account service
```

#### Expected Output / Response

Both calls return the same contacts (exit 0). `fixture.log` shows one token issued for the two server calls: the second reuses the cached credential:

```text
FIXTURE token grant=client_credentials client_ok=False -> 401 (tokens issued: 0)
FIXTURE token grant=client_credentials client_ok=True -> 200 (tokens issued: 1)
FIXTURE contacts bearer_ok=True -> 200
FIXTURE token grant=client_credentials client_ok=True -> 200 (tokens issued: 2)
FIXTURE contacts bearer_ok=True -> 200
FIXTURE contacts bearer_ok=True -> 200
```

(The first two lines are step 4; separate CLI processes have independent memory stores.) `trace show` for the second call lists broker decisions for `contacts.list#6` (network), `#4` (`allow_auth use crm_service/service/use`) and `#2` (`allow_credentials read`), each with its `policy_hash` and matching rule; it contains no token. The `ws` surface is not mounted (`ws 404`). `auth status` needs its own grant (exit 3):

```json
{"request_id":"req_05dabafbe1","trace_id":"tr_05dabafbe1","error":{"kind":"permission","code":"permission.denied","message":"allow_auth status crm_service/service/status denied: no grant for allow_auth crm_service/service/status","retryable":false,"effects":"none","operation_id":"rivet.auth.status","details":{"capability":"allow_auth","access":"status","target":"crm_service/service/status"}}}
```

Then prove the token never leaked:

```sh
kill $SV
grep -c 'DEMO-AT' serve.err fixture.log
```

```text
serve.err:0
fixture.log:0
```

### 6. Missing grant

#### Command / Request

```sh
python3 - <<'PY'
import json; p = json.load(open("policy.json"))
p["grants"] = [g for g in p["grants"] if g["capability"] != "allow_env"]
open("policy.json", "w").write(json.dumps(p, indent=2))
PY
rivet --file app.rivet io --check-policy
rivet --file app.rivet request contacts.list --params '{}'
```

#### Expected Output / Response

The manifest predicts the denial (`5 allowed · 1 denied`, the env row `denied`, exit 3) and the runtime agrees (exit 3):

```json
{"request_id":"req_01d5b59505","trace_id":"tr_01d5b59505","error":{"kind":"permission","code":"permission.denied","message":"allow_env read CRM_CLIENT_SECRET denied: no grant for allow_env CRM_CLIENT_SECRET","retryable":false,"effects":"none","source":{"file":"app.rivet","line":20,"column":5,"end_line":23,"end_column":8},"operation_id":"contacts.list","details":{"capability":"allow_env","access":"read","target":"CRM_CLIENT_SECRET"}}}
```

### 7. Slow token endpoint

#### Command / Request

```sh
sed -e 's#https://auth\.example\.com:443#http://127.0.0.1:18870#g' -e 's#https://api\.example\.com:443#http://127.0.0.1:18871#g' \
    policy.json.bak > policy.json          # restore the full grants
kill $FX
DEMO_FIXTURE_TOKEN_DELAY=5 python3 fixtures/oauth_fixture.py 18870 18871 2>>fixture.log & FX=$!
rivet --file app.rivet request contacts.list --params '{}' --timeout 2s
kill $FX
```

#### Expected Output / Response

Exit 6:

```json
{"request_id":"req_01963813cd","trace_id":"tr_01963813cd","error":{"kind":"timeout","code":"timeout.auth_token","message":"the authorization server did not answer before the deadline","retryable":false,"effects":"none","source":{"file":"app.rivet","line":20,"column":5,"end_line":23,"end_column":8},"operation_id":"contacts.list"}}
```

## Effects and policy

```text
  outcome table (verified in steps 4–7)
  ┌───────────────────────────────────────┬───────────────────────────┬──────┬──────┐
  │ situation                             │ code                      │ exit │ HTTP │
  ├───────────────────────────────────────┼───────────────────────────┼──────┼──────┤
  │ token acquired, contacts returned     │ —                         │  0   │ 200  │
  │ CRM_CLIENT_SECRET not set             │ auth.client_secret_missing│  5   │ 502  │
  │ token endpoint rejects the client     │ auth.token_endpoint_failed│  5   │ 502  │
  │ a required grant is missing           │ permission.denied         │  3   │ 403  │
  │ token endpoint slower than --timeout  │ timeout.auth_token        │  6   │ 504  │
  └───────────────────────────────────────┴───────────────────────────┴──────┴──────┘
```

Allowing only the resource origin is not enough; env, credential, profile use and the token network each need their own grant. The token is attached only to origins listed in `resource_origins`. Secret taint is enforced on explicit flows (returning, emitting or sending a secret-derived value to a non-bound origin is refused); implicit flows such as branching on a secret are not tracked.

## Release Updates

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-13 | PROJECT §§6, 79–80 / R13 | Secrets bound to destinations, never returned or logged | Steps 4–5 | Contacts returned; `grep -c DEMO-AT` is 0 | This README steps 4–5 (2026-09-28, 829ca43); TEST-2026-0008, TEST-2026-0009 |
| U-16 | UQ-14 / R16 | OAuth 2.0 client_credentials, memory store, typed auth failures | Steps 4–7 | One token reused; codes in the outcome table | This README steps 4–7; TEST-2026-0011 |
| U-24 | UQ-17 / R24 | Five separate grants in policy.json; `serve` block narrows surfaces | Steps 2, 5, 6 | 6 allowed; `ws 404`; env denial exit 3 | This README steps 2, 5, 6; TEST-2026-0021 |
| U-26 | UQ-18 / R26 | Manifest lists env, credential, auth and both network sites | Step 2 | Five targets, six sites | This README step 2; TEST-2026-0025 |

## Cleanup

```sh
kill $SV $FX 2>/dev/null
unset CRM_CLIENT_SECRET
cd - && rm -rf "$WORK"
```

Stopping the server disposes of the memory credential store. Stopping a client does not revoke tokens at the provider. Nothing is written to this folder.

## Verification Record

| Step | Verified By | Verified At | Result |
|---|---|---|---|
| 1. `check --strict-docs`, `outputs` | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 2. `io --by target`, `io --check-policy` (0), `io --check-files` (0), `policy explain --json` | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 3. Scratch copy, fixture, rewritten `io --check-policy` | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 4. Missing secret, rejected client, success | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 5. Reuse through serve, trace, `ws 404`, `auth status` denied, no token in logs | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 6. Missing `allow_env` grant | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 7. `timeout.auth_token` | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |

Build: `cargo build` and `cargo build --release` at 829ca43; every command above was executed from this folder or the scratch copy and the output pasted from that run.

## Known Caveats

- Only `client_credentials` with `store memory` is exercised here. Authorization code + PKCE, device code, refresh and the keychain store are covered by TEST-2026-0011, not by this folder.
- The fixture is plain HTTP on loopback; a real authorization server must use HTTPS.
- `auth status` is governed by `allow_auth …/status`, separate from `…/use`; this policy grants only `use`.

## Related Documents

- [All sample folders](../README.md) · [demos index](../index.md) · [release verification guide](../demo-2026-0015-v0-1-0-release-verification.md)
- [Protocols and connectors manual](../../manuals/man-2026-0008-protocols-and-connectors.md)
- [OAuth tests TEST-2026-0011](../../testing/test-2026-0011-oauth.md) · [Auth/transport policy tests TEST-2026-0015](../../testing/test-2026-0015-auth-transport-policy.md)
- [Usage reference](../../references/ref-2026-0002-language-and-usage.md) · [Proposal](../../proposals/approved/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 5 | 2026-09-28 | Claude | TASK-067: added fixtures/oauth_fixture.py (ports 18870/18871) and a scratch-copy run; executed every step against 0.1.0-dev (829ca43) and pasted real output: check, outputs (`(open: extra fields allowed)`), manifest (summary line, `use` row before `read, write`), `auth.client_secret_missing`, `auth.token_endpoint_failed`, success, reuse through serve, trace, token-leak grep, `allow_env` denial, `timeout.auth_token`; removed draft disclaimers; status active; verified_against 0.1.0. |
| 4 | 2026-09-28 | Claude | TASK-005/R26 (ADR-0001, proposal revision 8): `io --by target` gains ORIGIN (`client_secret env`, `auth`, `token_url`, `http get`), PHASE and NEEDS FILE. |
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest: env/credential/auth/network sites by target and `io --check-policy` (all allowed). |
| 2 | 2026-09-28 | Claude | UQ-17: declared open output; policy.json auto-discovered with `serve` surfaces/auth; `serve --listen` replaces `--transport http … --mcp`; outputs route; View outputs, strict-docs, bootstrap and exit codes. |
| 1 | 2026-09-28 | Codex | Added draft source files, prerequisites, invocation examples and expected behavior. |
