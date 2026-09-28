---
document_id: SYS-2026-0006
title: "Rivet OAuth 2.0 and credentials"
document_type: system
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 2
authors: [Claude]
owner: Project maintainer
component_owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [auth, transports]
affected_versions:
  from: "0.1.0"
  to: null
last_verified_version: "0.1.0-dev (commit 829ca43)"
next_review_date: 2026-10-28
review_cycle: on-release
confidentiality: internal
scope: How Rivet validates `auth NAME oauth2` profiles, runs the client_credentials, authorization_code (PKCE S256) and device_code grants, keeps principal-bound authorization transactions, stores and refreshes tokens (memory or keychain), binds bearer tokens to resource origins, and exposes all of this through the rivet.auth.* built-ins and the `rivet auth …` CLI.
reason: OAuth is the only place where Rivet holds long-lived secrets on behalf of scripts. Maintainers and reviewers need one current-state description of which policy grants each step needs, where tokens live, which states a transaction passes through and which typed errors callers see.
related_documents: [PROP-2026-0001, PLAN-2026-0001, ADR-0002, SYS-2026-0003, SYS-2026-0004, SYS-2026-0005, SYS-2026-0008, SYS-2026-0009]
supersedes: null
superseded_by: null
tags: [rivet, system, oauth2, pkce, device-flow, credentials, keychain, tokens, auth]
---

# Rivet OAuth 2.0 and credentials

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** auth, transports
> **Last Verified Version:** 0.1.0-dev (commit 829ca43)

## Summary

Rivet is an OAuth 2.0 *client*. A bundle declares one or more `auth NAME oauth2 … end` profiles. An HTTP
effect (`http get … auth PROFILE account "A"`) or an HTTP MCP connector (`auth PROFILE account "A"`)
then asks the runtime for an origin-bound bearer **lease**. Scripts never see the token. Interactive
grants (authorization code and device code) are driven through five reserved built-ins,
`rivet.auth.begin`, `complete`, `status`, `disconnect` and `cancel`. The same built-ins back the
`rivet auth …` CLI, `POST /v1/request`, MCP tools, WebSocket and the library.

```text
                        ┌──────────────────────────── bundle (app.rivet) ─────────────────────────────┐
                        │  auth crm_service oauth2 … end     operation contacts.list                  │
                        │  auth crm_user    oauth2 … end       http get URL  auth crm_service account │
                        └────────────┬───────────────────────────────────────┬────────────────────────┘
                                     │ OAuthProfile::from_declaration         │ AuthAttach{profile,account}
                                     ▼ (validated, config_hash pinned)        ▼
 rivet auth … / /v1/request   ┌─────────────────────┐              ┌─────────────────────────────┐
 rivet.auth.* built-ins ─────▶│ features/auth/*     │              │ infra/http_adapter.rs       │
 (orchestrator/builtins.rs)   │ begin / complete /  │              │  attach(): allow_network    │
                              │ status / disconnect │              │  on the resource first      │
                              │ / cancel            │              └──────────────┬──────────────┘
                              └─────────┬───────────┘                             │ acquire_credential
                                        │ allow_auth + allow_credentials          │ (origin bound, use)
                                        ▼                                         ▼
                              ┌───────────────────────────────────────────────────────────────────┐
                              │ infra/oauth_adapter.rs  OAuthAdapter                                │
                              │   OAuthSessionDriver  +  CredentialProvider                         │
                              │   transactions (≤ 8 open / principal, ≤ 10 min)                     │
                              │   per-credential single-flight slot  ·  revoked-token digests       │
                              │   store: memory (process) │ keychain "NS" (keyring-core)            │
                              └───────────────────┬───────────────────────────────────────────────┘
                                                  │ POST token_url / device_url (form)
                                                  ▼ through transports.exchange_http
                                        authorization server (allow_network, allow_env for secret)
```

## Responsibilities

| Responsibility | Where | Notes |
|---|---|---|
| Validate profile options, refuse unsafe flows, pin a `config_hash` | `src/domain/auth.rs` (`OAuthProfile::from_declaration`) | `password` and `implicit` → `unsupported.auth_flow` |
| Load every profile of the program at runtime assembly | `src/infra/oauth_adapter.rs` (`OAuthAdapter::load`) | Only `oauth2` kind; otherwise `unsupported.auth_kind` |
| Authorize every auth use case before any state or network | `src/features/auth/support/mod.rs` (`authorize_auth`, `authorize_store`) | `allow_auth PROFILE/ACCOUNT/VERB` + `allow_credentials PROFILE/ACCOUNT` |
| Begin / complete / cancel principal-bound transactions | `src/features/auth/{begin,complete,cancel}_authorization.rs` → `OAuthSessionDriver` | Challenge data only; never verifier, state or tokens |
| Report sanitized account state | `src/features/auth/credential_status.rs` | No refresh, no network |
| Forget local credentials | `src/features/auth/disconnect_account.rs` | `local_only: true`, generation + 1 |
| Hand transports an origin-bound lease | `src/features/auth/acquire_credential.rs` → `CredentialProvider::acquire` | Wrapped by `AuthorizedCredentials` in `src/orchestrator/runtime.rs` |
| Acquire, cache, refresh and invalidate tokens | `src/infra/oauth_adapter.rs` | Single flight per credential identity |
| Map built-in params to use-case inputs | `src/orchestrator/builtins.rs` | `callback` object → `SecretCallback` |
| Map `rivet auth …` to built-ins | `src/orchestrator/setup_cli.rs` (`auth_request`) | `--params-file` keeps codes out of argv |

## Boundaries and Non-Responsibilities

- **No browser, no callback listener.** `begin` returns an `authorization_url`. The host (or a human)
  navigates to it and collects the redirect parameters. Rivet adds no HTTP callback route.
- **No endpoint discovery.** Issuer metadata (`/.well-known/…`) is never fetched. Every endpoint is
  configured literally in the profile.
- **No provider-side revocation.** `disconnect` forgets local tokens only and says so (`local_only: true`).
- **Bearer tokens only.** A token response with another `token_type` fails with `unsupported.token_type`.
  DPoP, MAC, private-key JWT client auth and dynamic registration are not implemented.
- **Not caller authentication.** Callers of `rivet serve` authenticate separately (serve `auth`, see
  [SYS-2026-0004](../components/sys-2026-0004-surfaces-and-serve.md)). OAuth here is outbound only.
- **No raw token API.** No built-in, surface or script expression returns an access or refresh token.

## Architecture

### Layers

```text
 surfaces            CLI `rivet auth …`   /v1/request   MCP tools/call   WS   Runtime::request
                           │                  │               │           │          │
                           └──────────────────┴───────┬───────┴───────────┴──────────┘
 orchestrator                          builtins.rs  dispatch_builtin (rivet.auth.*)
                                                      │  AuthContext{principal, operation_id, deadline_ms}
 features/auth        begin_authorization · complete_authorization · credential_status
                      disconnect_account · cancel_authorization · acquire_credential
                      support/mod.rs: check_account · resolve_profile · authorize_auth · authorize_store
                                                      │ ports.rs
 domain               auth.rs: OAuthProfile, AuthChallenge, CredentialStatus, CredentialLease, SecretString
                      ports.rs: OAuthSessionDriver, CredentialProvider, PolicyEvaluator
                                                      │
 infra                oauth_adapter.rs: OAuthAdapter (both ports), KeyringBackend, platform_backend()
                      http_adapter.rs: exchange_http (token/device POSTs) + attach()/exchange_authed()
```

The use cases in `src/features/auth/` do only the checks that do not need secrets: profile lookup,
account shape, flow/shape guards, destination binding and the two policy permits. Everything that
touches a secret, a clock or the network is in `OAuthAdapter`.

### Profile validation (`OAuthProfile::from_declaration`)

```text
 auth NAME oauth2
   flow                client_credentials | authorization_code | device_code     (required)
   client_auth         basic | post | none                                        (required)
   issuer, token_url   https:// (plain http only on loopback)                     (required)
   client_id           public string                                              (required)
   scopes              ["…"]  ≥ 1                                                 (required)
   resource_origins    ["scheme://host:port"] ≥ 1, no path                        (required)
   store               memory | keychain "NAMESPACE"                              (required)
   client_secret       env "VAR"   (never a literal)       needed by basic|post
   authorization_url, redirect_uri, pkce s256              needed by authorization_code
   device_url                                              needed by device_code
   audience, description                                   optional
```

| Rule | Error code |
|---|---|
| Unknown option key | `validation.auth_profile` |
| `flow password` / `flow implicit` | `unsupported.auth_flow` (RFC 9700) |
| `client_auth` other than basic/post/none | `unsupported.auth_method` |
| `authorization_code` without `pkce s256` | `validation.auth_profile` |
| `pkce` other than `s256`/`S256` | `validation.auth_profile` |
| `client_credentials` with `client_auth none` or no secret | `validation.auth_profile` |
| `basic`/`post` without `client_secret env "VAR"` | `validation.auth_profile` |
| Endpoint not https (non-loopback http) | `validation.auth_profile` |
| Resource origin with a path or non-http(s) scheme | `validation.auth_profile` |

Resource origins are normalized with the default port made explicit (`https://api.example.com` →
`https://api.example.com:443`). The `config_hash` is SHA-256 over the canonical JSON of every option.
It is part of the credential cache key, so an edited profile never reuses tokens minted for the old one.
The key also carries an explicit digest of the **audience**, the **set of resource origins** and the **set of
scopes** (each sorted and de-duplicated), so two identities that differ in any of them never share a token
even if the configuration hash ignored them.

### Grant flows

#### client_credentials (no transaction; first authorized use acquires)

```text
 script            http_adapter          acquire_credential         OAuthAdapter             auth server
   │ http get URL auth P account A │               │                        │                        │
   │──────────────▶│ allow_network connect URL (resource first)            │                        │
   │               │──CredentialInput{origin}─────▶│                        │                        │
   │               │               │ origin ∈ resource_origins? else auth.origin_not_bound          │
   │               │               │ scopes ⊆ profile, audience equal? else validation.auth_scope    │
   │               │               │ allow_auth P/A/use · allow_credentials P/A read                 │
   │               │               │──acquire──────────────▶│ lock slot(principal/P#hash/A@ident)   │
   │               │               │                        │ cached & fresh? ──yes──▶ lease         │
   │               │               │                        │ no: allow_env VAR → secret             │
   │               │               │                        │     allow_network token_url            │
   │               │               │                        │──POST grant_type=client_credentials───▶│
   │               │               │                        │◀──────{access_token, expires_in}──────│
   │               │               │                        │ allow_credentials P/A write → commit   │
   │               │◀──────────────CredentialLease (opaque)─│                                        │
   │               │ Authorization: Bearer … → GET URL      │                                        │
   │◀── response ──│ (401 → invalidate lease; one retry only if method replay-safe and not streaming)
```

A client_credentials token without `expires_in` is never reused; the next use exchanges again (the same holds
for user flows, below).

#### authorization_code + PKCE S256

```text
 caller                   rivet.auth.*                 OAuthAdapter                  user / browser     auth server
   │ begin {P, A}             │                             │                               │                  │
   │─────────────────────────▶│ allow_auth P/A/manage       │                               │                  │
   │                          │ allow_credentials P/A write │                               │                  │
   │                          │────────begin───────────────▶│ state = 32 random bytes        │                  │
   │                          │                             │ verifier = 32 random bytes     │                  │
   │                          │                             │ challenge = b64url(sha256(v))  │                  │
   │                          │                             │ Tx{open, expires now+600 s}    │                  │
   │◀─{transaction_id, authorization_url, expires_at}───────│ (no network)                   │                  │
   │ open URL ───────────────────────────────────────────────────────────────────────────────▶│── login ───────▶│
   │◀──────────────────────────────── redirect_uri?code=…&state=…&iss=… ──────────────────────│◀────────────────│
   │ complete {transaction_id, callback{code,state,redirect_uri,issuer?}}                      │                  │
   │─────────────────────────▶│ owner? manage? credentials write? callback present?            │                  │
   │                          │──────complete──────────────▶│ state == Tx.state (constant time)│                 │
   │                          │                             │ redirect_uri exact, issuer equal │                 │
   │                          │                             │ mismatch → Tx failed, auth.callback_invalid        │
   │                          │                             │ Tx → exchanging; take verifier   │                 │
   │                          │                             │──POST grant_type=authorization_code, code, ──────▶│
   │                          │                             │   redirect_uri, code_verifier, client_id          │
   │                          │                             │◀─────{access_token, refresh_token, expires_in}────│
   │                          │                             │ commit generation+1 → Tx connected                 │
   │◀─CredentialStatus{state:"connected", scopes, expires_at, generation}                                          │
```

A cancel that lands while the code is being exchanged wins: the adapter re-checks the transaction state
after the token reply and stores nothing when it is no longer `exchanging`.

#### device_code (RFC 8628) with deadline → `{"state":"pending"}`

```text
 caller                      rivet.auth.*                 OAuthAdapter                        auth server
   │ begin {P, A}                │                             │                                    │
   │────────────────────────────▶│ manage + credentials write  │                                    │
   │                             │────────begin───────────────▶│──POST device_url (scope, client)──▶│
   │                             │                             │◀─device_code, user_code, verif_uri─│
   │                             │                             │ Tx{open, interval, expires ≤ 600 s}│
   │◀─{transaction_id, verification_uri, user_code, expires_at, interval_seconds}─────────────────────│
   │                                                                                                   │
   │ complete {transaction_id}              (wait false: one check, never sleeps)                     │
   │ complete {transaction_id, wait:true}   (loops until connected, terminal error or deadline)       │
   │────────────────────────────▶│────────complete────────────▶│ one poller per Tx (poll mutex)     │
   │                             │                             │ ┌─ loop ───────────────────────┐   │
   │                             │                             │ │ next_poll in future?          │   │
   │                             │                             │ │   wait=false or past deadline │   │
   │                             │                             │ │     → {"state":"pending"}     │   │
   │                             │                             │ │   else sleep until next_poll  │   │
   │                             │                             │ │ POST token_url device_code ───┼──▶│
   │                             │                             │ │ authorization_pending → +int  │◀──│
   │                             │                             │ │ slow_down → interval += 5 s   │   │
   │                             │                             │ │ transport fault → 2× interval │   │
   │                             │                             │ │ timeout → pending             │   │
   │                             │                             │ │ 200 → commit → connected      │   │
   │                             │                             │ └───────────────────────────────┘   │
   │◀─ {"state":"pending", transaction_id, expires_at}   or   CredentialStatus{connected}             │
```

A `pending` answer never consumes the transaction. The caller calls `complete` again. The increased
`slow_down` interval is stored on the transaction, so it carries over to later calls. No poll runs
between calls.

### Transaction state machine

```text
                         begin (code: no network │ device: POST device_url)
                                  │
                                  ▼
       complete wait:false ┌──────────────┐  now ≥ expires_at (checked on every access)
       or deadline first   │     open     │──────────────────────────────────────────▶ expired
       ─ ─ (pending) ─ ─ ─▶│  (reported   │                                   (auth.transaction_expired)
                           │    "open")   │── cancel / disconnect ───────────────────▶ cancelled
                           └──────┬───────┘
          code: callback checks   │   device: token 200
          pass; device: token 200 │
                                  ▼
                           ┌──────────────┐── cancel / disconnect ───────────────────▶ cancelled
                           │  exchanging  │   (a late token reply is then discarded)
                           │ (reported    │
                           │    "open")   │── token error / store failure ───────────▶ failed
                           └──────┬───────┘── access_denied ─────────────────────────▶ denied
                                  │ commit ok
                                  ▼
                              connected

   callback state/redirect/issuer mismatch, or provider `error` other than access_denied ──▶ failed
   provider `error=access_denied` in the callback or token reply ─────────────────────────▶ denied
   device `expired_token` ────────────────────────────────────────────────────────────────▶ expired
```

- Terminal transactions drop their state, PKCE verifier and device code immediately (`Tx::end`).
- A second `complete` on a terminal transaction is `auth.callback_invalid` (or `auth.access_denied` /
  `auth.transaction_expired` for those states).
- `cancel` on a terminal transaction returns its terminal state unchanged (idempotent).
- Terminal records are pruned 20 minutes (2 × `MAX_TRANSACTION_SECS`) after creation.
- At most 8 open transactions per principal (`MAX_OPEN_PER_PRINCIPAL`); the ninth `begin` is
  `limit.auth_transactions`.

### Token lifecycle

```text
                     ┌──────────────── no entry / tokens dropped ─────────────────┐
                     ▼                                                             │
              disconnected ──complete (code/device) ─┐                              │
               (gen N)      ──client_credentials use─┤                              │
                     ▲                               ▼                              │
                     │                          connected ─── now + skew ≥ exp ──▶ expired
      disconnect     │                          (gen N+1)  ◀── refresh ok ──────────  │
      (gen +1,       │                               │        (same gen, rotated      │
      tombstone)     │                               │         refresh token kept     │
                     │                               │         or replaced)           │
                     └───────────────────────────────┤                               │
                                                     │ resource 401                   │
                                                     ▼                                │
                                          token digest → revoked set ─────────────────┘
                                          (next acquire exchanges/refreshes)

   refresh answers:  invalid_grant ─▶ tokens dropped, auth.login_required
                     timeout / connection fault after send ─▶ tokens dropped, auth.refresh_uncertain
                     unusable 200 body ─▶ tokens dropped, auth.refresh_uncertain
                     entry version changed meanwhile ─▶ auth.refresh_uncertain (no replay)
                     store write fails ─▶ auth.refresh_uncertain
```

Freshness (`usable`): the token is present, its SHA-256 is not in the revoked set, and
`now + skew < expires_at` with `skew = min(30 s, lifetime / 2)`. A token **without expiry is never reused**,
whatever the flow (`usable` returns false): client_credentials exchanges again; a user-flow account that holds
a refresh token is refreshed at every use (and reported `connected` by `auth status`); without a refresh token
the next use is `auth.login_required`.

```text
 acquire(P, A) ─▶ Stored record?
      ├─ access token, expiry known, now + skew < exp, not revoked ─▶ reuse (lease)
      ├─ no expiry / near expiry / revoked:
      │      client_credentials ─▶ new exchange
      │      user flow + refresh token ─▶ refresh (single flight) ─▶ lease
      │      user flow, no refresh token ─▶ auth.login_required
      └─ none ─▶ client_credentials: exchange │ user flow: auth.login_required (run auth begin/complete)
```

Generation: every new login (code, device, or a client_credentials exchange into an empty entry) sets
`generation = previous + 1`. Refresh keeps the generation. `disconnect` writes a token-free tombstone
with `generation + 1`, so leases from the old generation cannot be issued again.

## Interfaces

### Built-in operations (`src/orchestrator/builtins.rs`)

| ID | Params | Result | Needs |
|---|---|---|---|
| `rivet.auth.begin` | `{profile, account}` | `AuthChallenge` `{transaction_id, authorization_url?, verification_uri?, verification_uri_complete?, user_code?, expires_at, interval_seconds?}` | `allow_auth P/A/manage`, `allow_credentials P/A` write; device also `allow_network` device_url (+ `allow_env` if confidential) |
| `rivet.auth.complete` | `{transaction_id, callback?: {code, state, redirect_uri, issuer?\|iss?, error?}, wait?: bool}` | `CredentialStatus` or `{"state":"pending", transaction_id, expires_at}` | owner principal, `manage`, `allow_credentials` write, `allow_network` token_url |
| `rivet.auth.status` | `{profile, account}` | `{profile, account, state: connected\|expired\|disconnected, scopes, expires_at, generation}` | `allow_auth P/A/status`, `allow_credentials` read |
| `rivet.auth.disconnect` | `{profile, account}` | `{profile, account, local_only: true, generation}` | `manage`, `allow_credentials` write |
| `rivet.auth.cancel` | `{transaction_id}` | `{transaction_id, state}` | owner principal, `manage` |

The request deadline (`deadline_ms`, or CLI `--timeout`) bounds `complete` and every token POST. One
token-endpoint exchange is also capped at 30 s (`TOKEN_TIMEOUT`).

### CLI (`rivet auth …`)

```text
$ rivet auth --help
OAuth account management (rivet.auth.* built-ins); tokens are never printed

Usage: rivet auth [OPTIONS] <COMMAND>

Commands:
  begin       Start an authorization_code or device_code transaction; prints the challenge
  complete    Complete a transaction. Pass callback codes with --params-file, never argv
  status      Sanitized credential status (never refreshes)
  disconnect  Forget local credentials (local_only; no provider revocation)
  cancel      Cancel an open authorization transaction
  help        Print this message or the help of the given subcommand(s)

Options:
      --file <FILE>              Entry .rivet file (policy.json beside it is discovered automatically)
      --policy <POLICY>          Use this policy file instead of the discovered policy.json (a path, never grants)
      --json                     Print JSON instead of tables
      --endpoint <ENDPOINT>      Send request/list/describe/outputs/io/trace/auth to a running `rivet serve` at this URL instead of loading a bundle (cannot be combined with --file)
      --token-file <TOKEN_FILE>  With --endpoint: read the server bearer token from this file (never argv or env)
  -h, --help                     Print help
```

| Command | Built-in | Extra flags |
|---|---|---|
| `rivet auth begin PROFILE --account A` | `rivet.auth.begin` | |
| `rivet auth complete --params-file PATH` / `--params JSON` | `rivet.auth.complete` | `--timeout D` (for example `30s`; sets the request deadline) |
| `rivet auth status PROFILE --account A` | `rivet.auth.status` | |
| `rivet auth disconnect PROFILE --account A` | `rivet.auth.disconnect` | |
| `rivet auth cancel TRANSACTION_ID` | `rivet.auth.cancel` | |

The CLI always runs as the `local` principal. `complete` parse errors report only line and column, never
the input, because it may hold a callback code. `store memory` lives inside one process, so a
multi-step flow (`begin` then `complete`) must go to one long-lived runtime: `rivet serve` plus
`--endpoint URL`.

### Ports (`src/domain/ports.rs`, re-exported by `src/features/auth/ports.rs`)

```text
 OAuthSessionDriver  profile(name) · transaction(id, principal) · begin · complete · status · disconnect · cancel
 CredentialProvider  profile(name) · acquire(CredentialInput) -> CredentialLease · invalidate(&CredentialLease)
```

`CredentialLease {handle: SecretString, scope_id, origin, generation, expires_at}` is only for
adapters. `http_adapter.rs` and `connectors/invoke_mcp.rs` are its only consumers.

## Configuration

### Source: profile and use

```text
auth crm_service oauth2                         operation contacts.list
    flow client_credentials                         …
    issuer "https://auth.example.com"               response = http get "https://api.example.com/contacts"
    token_url "https://auth.example.com/token"          auth crm_service account "service"
    client_id "rivet-service"                           decode json
    client_secret env "CRM_CLIENT_SECRET"           end
    client_auth basic                               return response.body
    scopes ["contacts.read"]                    end
    resource_origins ["https://api.example.com:443"]
    store memory
end
```

(From `docs/demos/07-oauth2/app.rivet`.)

### Policy (`policy.json`)

| Capability | Target | Access verbs | Used by |
|---|---|---|---|
| `allow_auth` | `PROFILE/ACCOUNT/VERB`, or `PROFILE/ACCOUNT` with `"access": [VERB…]` | `use`, `manage`, `status` | use = acquire/refresh; manage = begin/complete/cancel/disconnect; status = status |
| `allow_credentials` | `PROFILE/ACCOUNT` | `read`, `write` | store read (acquire, status) and write (commit, disconnect, begin pre-check) |
| `allow_env` | `VAR` of `client_secret env "VAR"` | `read` | only when a token/device POST is actually made |
| `allow_network` | token_url / device_url origin | `connect` | every token/device POST, through `exchange_http` |
| `allow_network` | resource origin | `connect` | checked *before* a lease is requested |

`authorize_auth` tries `PROFILE/ACCOUNT/VERB` first and then `PROFILE/ACCOUNT` with the verb, so the
two spellings are equivalent. `manage` alone does not permit `status`. Loopback or private-range
endpoints must be named literally (`http://127.0.0.1:18440`), as for any network grant (see
[SYS-2026-0008](../configuration/sys-2026-0008-policy-json-reference.md)).

Verified inventory for `docs/demos/07-oauth2` (one `http … auth` line expands into every effect a token
acquisition may need, even when a cached token would be reused):

```text
$ rivet --file app.rivet io --by target
TARGET                        ACCESS        CAPABILITY         ORIGIN             PHASE    NEEDS FILE  USED BY
env CRM_CLIENT_SECRET         read          allow_env          client_secret env  body     —           contacts.list (secret, bound to https://auth.example.com:443)
crm_service/service           use           allow_auth         auth               body     —           contacts.list
crm_service/service           read, write   allow_credentials  auth               body     —           contacts.list
https://auth.example.com:443  connect POST  allow_network      token_url          connect  —           contacts.list (token endpoint)
https://api.example.com:443   connect GET   allow_network      http get           connect  —           contacts.list

$ rivet --file app.rivet io --check-policy
OPERATION      KIND        ACCESS        TARGET                            KNOWLEDGE  SOURCE        DECISION
contacts.list  env         read          env CRM_CLIENT_SECRET             exact      app.rivet:6   allowed
contacts.list  credential  read          crm_service/service               exact      app.rivet:21  allowed
contacts.list  credential  write         crm_service/service               exact      app.rivet:21  allowed
contacts.list  auth        use           crm_service/service               exact      app.rivet:21  allowed
contacts.list  network     connect POST  https://auth.example.com/token    exact      app.rivet:4   allowed
contacts.list  network     connect GET   https://api.example.com/contacts  exact      app.rivet:20  allowed
6 allowed
```

```text
$ rivet --file app.rivet describe contacts.list
contacts.list — List protected contacts
Acquire a service credential and call the authorized resource origin.
source   app.rivet:13
delivery unary

params
  —

output  object   The contacts API payload; never contains the access token.
  contacts  list json required  Contact objects returned by the resource server.
  (open: extra fields allowed)
emits    —
receives —
errors   —
```

## Runtime Behaviour

### Verified in `docs/demos/07-oauth2` (no live IdP)

The demo policy grants only `crm_service/service/use`. The management built-ins are therefore denied,
and `begin` on a client_credentials profile is refused by flow:

```text
$ rivet --file app.rivet auth status crm_service --account service          # exit 3
{"request_id":"req_015a3a8d05","trace_id":"tr_015a3a8d05","error":{"kind":"permission","code":"permission.denied","message":"allow_auth status crm_service/service/status denied: no grant for allow_auth crm_service/service/status","retryable":false,"effects":"none","operation_id":"rivet.auth.status","details":{"capability":"allow_auth","access":"status","target":"crm_service/service/status"}}}

$ rivet --file app.rivet auth begin crm_service --account service           # exit 2
{"request_id":"req_01593ddaed","trace_id":"tr_01593ddaed","error":{"kind":"validation","code":"validation.auth_flow","message":"profile `crm_service` uses client_credentials: the first authorized use acquires a token; begin/complete are for authorization_code and device_code","retryable":false,"effects":"none","operation_id":"rivet.auth.begin"}}

$ rivet --file app.rivet auth cancel auth_01nope                              # exit 4
{"request_id":"req_0158cd467d","trace_id":"tr_0158cd467d","error":{"kind":"not_found","code":"not_found.auth_transaction","message":"no authorization transaction `auth_01nope`","retryable":false,"effects":"none","operation_id":"rivet.auth.cancel"}}

$ rivet --file app.rivet request contacts.list --params '{}'                  # exit 5, CRM_CLIENT_SECRET unset
{"request_id":"req_01ff3a2915","trace_id":"tr_01ff3a2915","error":{"kind":"application","code":"auth.client_secret_missing","message":"auth profile `crm_service`: client secret env CRM_CLIENT_SECRET is not set","retryable":false,"effects":"none","source":{"file":"app.rivet","line":20,"column":5,"end_line":23,"end_column":8},"operation_id":"contacts.list"}}
```

With the secret set, the demo needs a live authorization server at `auth.example.com` and a resource
server at `api.example.com`. Those are not provided.

### Verified against a local fixture authorization server

The three grants were run end to end against a small scratch fixture. The fixture is a Python HTTP
server on `127.0.0.1:18440` that serves `/token`, `/device` and a bearer-protected `/contacts`. The
bundle is a copy of the demo profile plus `crm_user` (authorization_code, `client_auth none`,
`pkce s256`, `redirect_uri "http://127.0.0.1:18441/callback"`) and `crm_device` (device_code). Its
policy grants
`allow_auth` `crm_user/ada` and `crm_device/ada` with `"access": ["manage","status","use"]`,
`allow_credentials` for the three accounts, `allow_env CRM_CLIENT_SECRET` and
`allow_network http://127.0.0.1:18440`. IDs, timestamps, `state` and `code_challenge` vary per run.

**client_credentials (standalone CLI, then a served runtime):**

```text
$ CRM_CLIENT_SECRET=… rivet --file app.rivet request contacts.list --params '{}'
{"request_id":"req_0117be9825","trace_id":"tr_0117be9825","result":{"contacts":[{"name":"Ada"}]},"data_count":0,"effects":"none"}
   fixture saw: POST /token (form keys grant_type, scope; client auth in Basic header) · GET /contacts

$ rivet --endpoint http://127.0.0.1:18442 request contacts.list --params '{}'     # twice
{"request_id":"req_010c585155",…,"result":{"contacts":[{"name":"Ada"}]},…}
{"request_id":"req_028bdd254a",…,"result":{"contacts":[{"name":"Ada"}]},…}
   fixture saw: POST /token · GET /contacts · GET /contacts     (second call reused the cached token)
```

**authorization_code + PKCE (through `rivet serve --listen 127.0.0.1:18442`):**

```text
$ rivet --endpoint http://127.0.0.1:18442 auth status crm_user --account ada
{"request_id":"req_030b64755f","trace_id":"tr_030b64755f","result":{"profile":"crm_user","account":"ada","state":"disconnected","scopes":[],"expires_at":null,"generation":0},"data_count":0,"effects":"none"}

$ rivet --endpoint http://127.0.0.1:18442 auth begin crm_user --account ada
{"request_id":"req_0485126e04","trace_id":"tr_0485126e04","result":{"transaction_id":"auth_01bz9yg4nh6asb","expires_at":"2026-09-28T05:22:54Z","authorization_url":"http://127.0.0.1:18440/authorize?response_type=code&client_id=rivet-desktop&redirect_uri=http%3A%2F%2F127.0.0.1%3A18441%2Fcallback&scope=contacts.read&state=thQ4A-baCdiPnE9mjIlyZSLqKFH6MiSSPMi0cI0KCik&code_challenge=o1BJGstFnIUrRcRa5vzGTzEqfp-ICDL2rrVP9IuA_Mk&code_challenge_method=S256"},"data_count":0,"effects":"none"}

$ cat cb.json
{"transaction_id":"auth_01bz9yg4nh6asb","callback":{"code":"code-xyz","state":"thQ4A-…","redirect_uri":"http://127.0.0.1:18441/callback","issuer":"http://127.0.0.1:18440"}}
$ rivet --endpoint http://127.0.0.1:18442 auth complete --params-file cb.json
{"request_id":"req_0581a216a9","trace_id":"tr_0581a216a9","result":{"profile":"crm_user","account":"ada","state":"connected","scopes":["contacts.read"],"expires_at":"2026-09-28T06:13:01Z","generation":1},"data_count":0,"effects":"none"}
   fixture saw: POST /token form keys client_id, code, code_verifier, grant_type, redirect_uri

$ rivet --endpoint http://127.0.0.1:18442 auth complete --params-file cb.json      # replay, exit 4
{"request_id":"req_0604a6d5a6","trace_id":"tr_0604a6d5a6","error":{"kind":"conflict","code":"auth.callback_invalid","message":"the authorization transaction is connected and cannot be completed again","retryable":false,"effects":"none","operation_id":"rivet.auth.complete"}}
```

**Forged state, then cancel of the failed transaction:**

```text
$ rivet --endpoint http://127.0.0.1:18442 auth complete --params-file bad.json      # exit 4, nothing exchanged
{"request_id":"req_097f198145","trace_id":"tr_097f198145","error":{"kind":"conflict","code":"auth.callback_invalid","message":"the callback state does not match the transaction; nothing was exchanged","retryable":false,"effects":"none","operation_id":"rivet.auth.complete","details":{"mismatch":"state"}}}

$ rivet --endpoint http://127.0.0.1:18442 auth cancel auth_02ffdulncba42o          # terminal state reported
{"request_id":"req_10ff11cd52","trace_id":"tr_10ff11cd52","result":{"transaction_id":"auth_02ffdulncba42o","state":"failed"},"data_count":0,"effects":"none"}
```

**device_code with pending (the fixture answers `authorization_pending` twice, then issues tokens):**

```text
$ rivet --endpoint http://127.0.0.1:18442 auth begin crm_device --account ada
{"request_id":"req_1149e1fb17","trace_id":"tr_1149e1fb17","result":{"transaction_id":"auth_03fyurf7f64hft","expires_at":"2026-09-28T05:23:06Z","verification_uri":"http://127.0.0.1:18440/activate","user_code":"WDJB-MJHT","interval_seconds":1},"data_count":0,"effects":"none"}

$ rivet --endpoint … auth complete --params '{"transaction_id":"auth_03fyurf7f64hft"}'
{"request_id":"req_12c196bf6c","trace_id":"tr_12c196bf6c","result":{"state":"pending","transaction_id":"auth_03fyurf7f64hft","expires_at":"2026-09-28T05:23:06Z"},"data_count":0,"effects":"none"}

$ rivet --endpoint … auth complete --params '{"transaction_id":"auth_03fyurf7f64hft","wait":true}' --timeout 1500ms
{"request_id":"req_1333949cb9","trace_id":"tr_1333949cb9","result":{"state":"pending","transaction_id":"auth_03fyurf7f64hft","expires_at":"2026-09-28T05:23:06Z"},"data_count":0,"effects":"none"}

$ rivet --endpoint … auth complete --params '{"transaction_id":"auth_03fyurf7f64hft","wait":true}' --timeout 30s
{"request_id":"req_1474483d4e","trace_id":"tr_1474483d4e","result":{"profile":"crm_device","account":"ada","state":"connected","scopes":["contacts.read"],"expires_at":"2026-09-28T06:13:12Z","generation":1},"data_count":0,"effects":"none"}
   fixture saw: POST /device, then 3 × POST /token (form keys client_id, device_code, grant_type)

$ rivet --endpoint … auth disconnect crm_device --account ada
{"request_id":"req_15cbf68edb","trace_id":"tr_15cbf68edb","result":{"profile":"crm_device","account":"ada","local_only":true,"generation":2},"data_count":0,"effects":"none"}
$ rivet --endpoint … auth status crm_device --account ada
{"request_id":"req_164b4d37b8","trace_id":"tr_164b4d37b8","result":{"profile":"crm_device","account":"ada","state":"disconnected","scopes":[],"expires_at":null,"generation":2},"data_count":0,"effects":"none"}
```

`--timeout` takes a bare duration (`30s`). A quoted value such as `'"30s"'` is rejected with
`validation.usage` (exit 2).

### Order of checks (every built-in)

```text
 params present? ─▶ profile known? ─▶ account 1-128 chars, no '/' or control ─▶ flow/shape guard
   (validation.required)  (not_found.auth_profile)  (validation.auth_account)   (validation.auth_flow /
                                                                                  validation.auth_callback)
 ─▶ allow_auth P/A/VERB ─▶ allow_credentials P/A ─▶ driver (store / network) ─▶ sanitized JSON
     (permission.denied)    (permission.denied)
```

`complete` and `cancel` first look the transaction up for the calling principal. An unknown ID and
another principal's transaction both return `not_found.auth_transaction`, so the response does not
reveal whether the other transaction exists.

### Error codes

| Code | Kind | HTTP / exit | When |
|---|---|---|---|
| `validation.auth_profile` | validation | 422 / 2 | invalid profile (load) |
| `unsupported.auth_flow`, `unsupported.auth_method`, `unsupported.auth_kind` | unsupported | 501 / 5 | refused flow, client auth or profile kind |
| `validation.auth_flow` | validation | 422 / 2 | begin/complete on client_credentials |
| `validation.auth_account` | validation | 422 / 2 | bad account key |
| `validation.auth_callback` | validation | 422 / 2 | missing/extra callback, malformed callback |
| `validation.auth_scope` | validation | 422 / 2 | scope or audience outside the profile |
| `not_found.auth_profile`, `not_found.auth_transaction` | not_found | 404 / 4 | unknown profile / transaction (or foreign) |
| `permission.denied` | permission | 403 / 3 | missing allow_auth / allow_credentials / allow_env / allow_network |
| `auth.origin_not_bound` | permission | 403 / 3 | destination origin not in `resource_origins` |
| `auth.login_required` | conflict | 409 / 4 | no login for a user flow, or `invalid_grant` |
| `auth.callback_invalid` | conflict | 409 / 4 | state/redirect/issuer mismatch, replay, provider error |
| `auth.access_denied` | conflict | 409 / 4 | user or server denied |
| `auth.transaction_expired` | conflict | 409 / 4 | TTL passed or `expired_token` |
| `auth.insufficient_scope` | conflict | 409 / 4 | granted scopes lack a required one, or `invalid_scope` |
| `auth.refresh_uncertain` | conflict | 409 / 4 | refresh outcome unknown; tokens dropped |
| `auth.token_endpoint_failed` | application / protocol | 502 / 5 | other token/device endpoint status, or unusable 200 body |
| `auth.client_secret_missing` | application | 502 / 5 | secret env var unset or empty |
| `auth.store_failed` | application | 502 / 5 | memory/keychain read or write failed |
| `unsupported.credential_store` | unsupported | 501 / 5 | no secure store on this platform |
| `unsupported.token_type` | unsupported | 501 / 5 | token_type other than Bearer |
| `unsupported.oauth` | unsupported | 501 / 5 | HTTP adapter built without a credential provider |
| `limit.auth_transactions` | limit | 429 / 5 | more than 8 open transactions |
| `timeout.auth_token` | timeout | 504 / 6 | token endpoint or single-flight wait exceeded the deadline |

## Data and Storage

```text
 OAuthAdapter (one per Runtime)
 ├── profiles       HashMap<name, OAuthProfile>                    immutable after load
 ├── transactions   HashMap<id, Tx>                                in memory only (never persisted)
 │     Tx{principal, profile, account, state, expires_at, oauth_state*, verifier*, device_code*,
 │        interval_ms, next_poll, poll mutex}                      * SecretString, cleared on end
 ├── memory         HashMap<key, Stored>                           store memory
 ├── keychain       Arc<dyn SecretBackend> (lazy)                  store keychain "NS"
 ├── slots          HashMap<key, tokio Mutex>                      single flight per credential
 └── revoked        HashSet<sha256(access_token)>                  tokens a resource answered 401 to

 key    = "<principal>/<profile>#<config_hash[..12]>/<account>@<identity_digest[..12]>"
          identity_digest = sha256("aud=…\nres=<sorted origins>\nscope=<sorted scopes>")
 Stored = {generation, version, profile_hash, access_token?, refresh_token?, expires_at?, issued_at, scopes}
```

- **memory**: process-local and lost at exit. Each standalone `rivet` CLI invocation has its own store.
- **keychain "NS"**: one JSON blob per `(service = NS, user = key)` through `keyring-core`. The backend is
  macOS Keychain (`apple-native-keyring-store`), Windows Credential Manager or Linux Secret Service
  (`zbus-secret-service-keyring-store`). The backend is opened lazily. `begin` opens it first, so a
  missing backend fails before any authorization starts. There is no plaintext fallback: without a
  backend the call fails with `unsupported.credential_store`. Backend error text is reduced to a
  failure class. Keychain calls run on `spawn_blocking`.
- A stored entry whose `profile_hash` differs from the loaded profile is ignored (treated as absent).
- Writers are serialized per key by the slot mutex. The `version` counter detects a concurrent
  rotation between the refresh request and the commit, for example by another process sharing the
  keychain.
- Transaction IDs are `auth_<counter><random>`, for example `auth_01bz9yg4nh6asb`.

## Dependencies

| Dependency | Use |
|---|---|
| `transports.exchange_http` (`src/infra/http_adapter.rs`) | Every token/device POST: `allow_network`, private-range and DNS checks, no redirects (`redirect_limit: 0`), 1 MiB body cap |
| `ring` (`SystemRandom`) | state, verifier and transaction randomness |
| `sha2`, `base64` | PKCE S256 challenge, config hash, revoked-token digests, Basic auth |
| `url` | origin normalization, authorization URL construction, form encoding |
| `keyring-core` + platform store crates | `store keychain` |
| `serde` / `serde_json` | keychain entry encoding |
| Policy broker ([SYS-2026-0003](../components/sys-2026-0003-policy-broker-and-io-manifest.md)) | all permits and traced decisions |

Crate choices are recorded in [ADR-0002](../../decisions/adr-0002-rust-crate-selection.md).

## Deployment

- No extra process or service. The adapter is part of every `Runtime` (`src/orchestrator/runtime.rs`).
- For interactive flows, run one long-lived `rivet serve --listen 127.0.0.1:PORT` and drive it with
  `rivet --endpoint http://127.0.0.1:PORT auth …`. Standalone CLI calls cannot share `store memory` or
  transactions.
- `store keychain` on macOS may show the system Keychain access prompt the first time. Headless Linux
  hosts need a running Secret Service. Otherwise use `store memory`.
- Supply client secrets through the host environment only (`client_secret env "VAR"`).

## Security Boundaries

```text
   ┌─ script / surfaces ─────────────┐     ┌─ OAuthAdapter (trusted) ──────────────┐     ┌─ network ───────────┐
   │ sees: challenge data, sanitized │     │ holds: SecretString tokens, codes,    │     │ token_url/device_url│
   │ status, receipts, API bodies    │◀───▶│ state, verifier, device_code, secret  │────▶│ (allow_network)     │
   │ never: tokens, codes, verifier  │     │ formats every secret as "[redacted]"  │     │ resource origins    │
   └─────────────────────────────────┘     └───────────────────────────────────────┘     │ (bound, allow_net.) │
                                                                                          └─────────────────────┘
```

- `SecretString` prints `[redacted]` for both `Debug` and `Display`. Only `expose()` reveals the value, at
  the two places that must send it: the form or header, and the `Authorization: Bearer` attachment.
- **Principal binding.** Transactions and store keys include the principal name. Another principal's
  account of the same name shows as `disconnected`, and its transactions as `not_found`.
- **Destination binding.** A lease is issued only for an exact `scheme://host:port` listed in
  `resource_origins`; otherwise `auth.origin_not_bound`. The HTTP adapter authorizes the resource
  URL before it asks for a token. `exchange_http` removes the `authorization` header on any origin
  change, and profile token requests never follow redirects.
- **Callback checks before exchange.** The callback must match the transaction state (constant-time
  compare), carry the exact redirect URI and the profile issuer when `issuer`/`iss` is present, and
  arrive before the TTL. Each state is single use.
- **Error hygiene.** Only RFC 6749-shaped provider `error` tokens (lowercase letters and `_`, 64 characters
  or fewer) reach error details. Raw provider bodies never do.
- **Refresh safety.** An ambiguous refresh drops the stored tokens instead of replaying the old refresh
  token.
- **401 handling.** The rejected token's digest is added to the revoked set. The request is retried once
  only when the method is replay-safe and the request is not streaming.
- **No token in traces.** Trace attempts record capability, target and decision only (see Observability).

## Observability

Every permit is a traced decision. A served `contacts.list` call that had to acquire a token shows the
resource check, the auth and credential permits, the env read and the token endpoint in its trace, and
no token. Excerpt from `rivet --endpoint http://127.0.0.1:18442 trace show req_010c585155` against the
fixture:

```text
{"request_id":"req_010c585155","attempts":[
 {…,"attempt":1,"effect_id":"contacts.list#6","phase":"decision","capability":"allow_network","access":"connect","target":"http://127.0.0.1:18440/contacts","decision":"allowed",…},
 {…,"attempt":1,"effect_id":"contacts.list#4","phase":"decision","capability":"allow_auth","access":"use","target":"crm_service/service/use","decision":"allowed",…},
 {…,"attempt":1,"effect_id":"contacts.list#2","phase":"decision","capability":"allow_credentials","access":"read","target":"crm_service/service","decision":"allowed",…},
 {…,"attempt":1,"effect_id":"contacts.list#1","phase":"decision","capability":"allow_env","access":"read","target":"CRM_CLIENT_SECRET","decision":"allowed",…},
 {…,"attempt":1,"effect_id":"contacts.list#5","phase":"decision","capability":"allow_network","access":"connect","target":"http://127.0.0.1:18440/token","decision":"allowed",…},
 {…,"attempt":1,"effect_id":"contacts.list#3","phase":"decision","capability":"allow_credentials","access":"write","target":"crm_service/service","decision":"allowed",…},
 {…,"attempt":2,"effect_id":"contacts.list#6",…,"target":"http://127.0.0.1:18440/contacts","decision":"allowed",…}],
 "complete":true,"next_cursor":null,"gaps":0}
```

`rivet io` and `rivet policy explain` list the same five targets statically (see Configuration). Built-in
calls return standard Completions and ErrorEnvelopes with `operation_id` `rivet.auth.*`.

## Known Limitations

- Transactions live only in memory. A restart loses open transactions even with `store keychain`.
- `store memory` is per process. Multi-step flows need one served runtime.
- No issuer metadata discovery, token revocation endpoint, token introspection, DPoP, private-key JWT
  or dynamic client registration.
- Only Bearer tokens. The `audience` is sent on authorization, device and client_credentials requests.
  Transports always request the profile's full scope list; no per-call scope narrowing is exposed in
  the language.
- A refresh runs inside the caller that holds the single-flight slot. If that caller is cancelled, the
  refresh is abandoned and the next caller starts again.
- **MCP 401 invalidates the lease without retry** (a known limitation, see the
  [manual](../../manuals/man-2026-0001-rivet-manual.md#known-limitations)): the resource-401 retry applies to
  HTTP effects only; MCP connectors invalidate the lease but do not retry the call (see
  [SYS-2026-0009](sys-2026-0009-mcp-client-connectors.md)).
- `store keychain` was not exercised against the real macOS Keychain in this verification. Unit tests
  cover it with an in-memory `SecretBackend`
  (`keychain_entries_round_trip_without_plaintext_defaults`).
- The `docs/demos/07-oauth2` success path needs a live authorization and resource server. It was
  verified only with the local fixture described above.

## Last Verified Version

0.1.0-dev (commit 829ca43), on macOS (darwin 25.4.0), 2026-09-28. The token-reuse and cache-key changes (G20)
were checked at `829ca43` against `src/infra/oauth_adapter.rs` (`usable`, `key`, `identity_digest`) and
`tests/conformance_oauth.rs`; the flows below were first run at `f40d4aa`. Commands were run with
`target/debug/rivet` from `docs/demos/07-oauth2`. The end-to-end grants were run against a scratch
fixture authorization server on `127.0.0.1:18440` and `rivet serve` on `127.0.0.1:18442`. Both were
stopped afterwards.

## Related Documents

- [PROP-2026-0001 Rivet runtime proposal](../../proposals/approved/prop-2026-0001-rivet-runtime.md), Increment 9
- [PLAN-2026-0001 v0.1.0 implementation and release](../../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md)
- [ADR-0002 Rust crate selection](../../decisions/adr-0002-rust-crate-selection.md)
- [SYS-2026-0003 Policy broker and I/O manifest](../components/sys-2026-0003-policy-broker-and-io-manifest.md)
- [SYS-2026-0004 Surfaces and serve](../components/sys-2026-0004-surfaces-and-serve.md)
- [SYS-2026-0005 Protocol adapters](sys-2026-0005-protocol-adapters.md)
- [SYS-2026-0008 policy.json reference](../configuration/sys-2026-0008-policy-json-reference.md)
- [SYS-2026-0009 MCP client connectors](sys-2026-0009-mcp-client-connectors.md)
- [Language and usage reference](../../references/ref-2026-0002-language-and-usage.md)
- [Demos](../../demos/README.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial current-state document (PLAN-2026-0001 D-20). |
| 2 | 2026-09-28 | Claude | TASK-092 drift fix for G20 (829ca43): tokens without expiry are never reused in any flow (user flows refresh or need login), cache key includes the audience, resource-origin set and scope set; MCP 401 limitation linked to the manual. |
