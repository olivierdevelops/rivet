---
document_id: DEMO-2026-0007
title: "OAuth 2.0 without returning tokens"
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

# OAuth 2.0 without returning tokens

**Draft usage sample.** Rivet has no runtime or CLI implementation yet. These files make the proposed interface concrete; commands below are intended usage, not executed demos.

## Purpose

OAuth 2.0 without returning tokens. Delivery stage: **B**. Read [app.rivet](app.rivet) alongside this walkthrough.

| Operation ID | Output | Behavior |
|---|---|---|
| `contacts.list` | `object {contacts}`, open | Use client credentials and call the protected contacts endpoint. |

```text
  contacts.list
     |  auth crm_service account "service"
     v
  broker ──allow_auth crm_service/service/use──> profile crm_service
     |        ──allow_credentials crm_service/service──> memory store (cache hit?)
     |        ──allow_env CRM_CLIENT_SECRET──> client secret      (only on miss/refresh)
     |        ──allow_network https://auth.example.com:443──> POST /token
     v
  GET https://api.example.com/contacts  (allow_network https://api.example.com:443,
                                         token attached only for bound resource_origins)
     v
  output {contacts: [...]}   -- the token never appears in results, traces or outputs
```

## Verified Against Version

None. Based on proposal revision 8 semantics (UQ-17) and S84 and S93. No parser, runtime or network fixture execution is claimed.

## Prerequisites

A future Rivet build implementing this stage. External services below are controlled fixtures, not public services to contact. Commands assume the repository root initially, then the setup directory. Each folder is an independent bundle; do not concatenate folders with duplicate IDs.

## Setup

```sh
cd docs/demos/07-oauth2
```

Provision a controlled OAuth provider and resource server that match the profile:

- The provider accepts `client_auth basic`, client ID `rivet-service` and scope `contacts.read`.
- `GET /contacts` returns `{"contacts":[...]}`.
- The resource origin is explicitly bound in the profile.

Supply `CRM_CLIENT_SECRET` in the host environment through your fixture secret mechanism. No real credential belongs in this folder or in shell history.

[policy.json](policy.json) is auto-discovered. It holds the four grants plus a `serve` block that mounts only the `http` and `mcp` surfaces with loopback auth `none`.

## Steps

### Command / Request

```sh
rivet --file app.rivet policy explain contacts.list --params '{}' --json
rivet --file app.rivet request contacts.list --params '{}'
rivet --file app.rivet serve --listen 127.0.0.1:8080
```

To make repeated calls against the same in-memory credential cache, use another terminal:

```sh
rivet --endpoint http://127.0.0.1:8080 request contacts.list --params '{}'
rivet --endpoint http://127.0.0.1:8080 request contacts.list --params '{}'
curl -sS http://127.0.0.1:8080/v1/operations/contacts.list/outputs
```

View the declared output locally:

```sh
rivet --file app.rivet outputs contacts.list
rivet --file app.rivet outputs --all --json
```

### Expected Output / Response

`policy explain` reports every required grant as satisfied. Each successful call returns the contacts API payload (exit 0). Tokens never appear in results or traces. A long-lived host reuses an unexpired credential; separate standalone CLI processes have independent memory stores.

| Failure | Code / kind | HTTP | Exit |
|---|---|---|---|
| Missing grant (for example remove the auth origin) | `permission.denied` | 403 | 3 |
| Missing `CRM_CLIENT_SECRET` or rejected client | auth/dependency error | 502 | 5 |
| Token endpoint or resource times out | `timeout` | 504 | 6 |
| Malformed policy.json | `policy.invalid` | — | 2 |

`rivet outputs contacts.list`:

```text
contacts.list — List protected contacts
output  object   The contacts API payload; never contains the access token.   (open)
  contacts  list json  required  Contact objects returned by the resource server.
emits    —
receives —
errors   —
```

`rivet outputs --all --json`:

```json
[
  {"id": "contacts.list",
   "output": {"type": "object", "description": "The contacts API payload; never contains the access token.",
              "properties": {"contacts": {"type": "array", "items": {}, "description": "Contact objects returned by the resource server."}},
              "required": ["contacts"], "additionalProperties": true},
   "emits": null, "receives": null, "errors": []}
]
```

## Effects and policy

[policy.json](policy.json) separates environment access, credential-store access, profile use, token network and resource network. Allowing only the resource origin is not enough. Source inspection includes a potential acquisition or refresh even on a cache hit. Secret taint is best-effort: explicit flows are tracked, implicit flows (such as branching on a secret) are not.

## Inspect before invoking

```sh
rivet --file app.rivet check --strict-docs
rivet --file app.rivet io --by target
rivet --file app.rivet io --check-policy
```

`check --strict-docs` passes: the public operation describes its output and output field, and its body has no `fail`.

**`io --by target`**. One `http get … auth crm_service` line expands into every effect a token acquisition or refresh may need. They are listed even when a cached token would be reused:

```text
TARGET                         ACCESS         CAPABILITY          ORIGIN              PHASE     NEEDS FILE   USED BY
env CRM_CLIENT_SECRET          read           allow_env           client_secret env   body      —            contacts.list (secret, bound to https://auth.example.com:443)
crm_service/service            read, write    allow_credentials   auth                body      —            contacts.list
crm_service/service            use            allow_auth          auth                body      —            contacts.list
https://auth.example.com:443   connect POST   allow_network       token_url           connect   —            contacts.list (token endpoint)
https://api.example.com:443    connect GET    allow_network       http get            connect   —            contacts.list
```

**`io --check-policy`** with the auto-discovered [policy.json](policy.json). Every site is allowed, so it exits 0:

```text
OPERATION       KIND         ACCESS         TARGET                             KNOWLEDGE   SOURCE         DECISION
contacts.list   env          read           env CRM_CLIENT_SECRET              exact       app.rivet:6    allowed
contacts.list   credential   read           crm_service/service                exact       app.rivet:21   allowed
contacts.list   credential   write          crm_service/service                exact       app.rivet:21   allowed
contacts.list   auth         use            crm_service/service                exact       app.rivet:21   allowed
contacts.list   network      connect POST   https://auth.example.com/token     exact       app.rivet:4    allowed
contacts.list   network      connect GET    https://api.example.com/contacts   exact       app.rivet:20   allowed
```

The policy selector `crm_service/service/use` already names the auth action, so it is equivalent to target `crm_service/service` with `"access": ["use"]`. Removing the `allow_env` grant turns the env row `denied`, and `io --check-policy` exits 3. The secret column shows the origin binding from the profile. The manifest never contains the secret value or a token.

`--include-bootstrap` adds the fixed runtime-internal list under a separate `bootstrap` key: the bundle and imports, policy.json, the CA bundle, resolv.conf or the system resolver, tzdata, descriptor/schema files, and stdin/stdout/stderr. It is listed for transparency, never granted to scripts. Sandbox guarantees apply to **script-initiated effects through brokered adapters**. `io` performs no I/O and evaluates no source expression. Exit codes: 3 when `--check-policy` finds a reachable site denied or partial; 7 with `--strict` when any site is dynamic or opaque (`complete: false`); otherwise 0.

## Release Updates

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-01 | UQ-17 / R23 | Declared open output | `rivet outputs contacts.list` | Table above | Not run — runtime does not exist |
| U-02 | UQ-17 / R24–R25 | `--policy policy.json` dropped (auto-discovered); serve surfaces set in policy.json | Commands above | Same results | Not run |
| U-03 | UQ-18 / R26 | `io` became the generated I/O manifest (targets, access verbs, capability, `--by`, `--check-policy`) | Inspect before invoking | Tables above | Not run — runtime does not exist |

## Cleanup

Stop the server to dispose of the memory credential store. Clear `CRM_CLIENT_SECRET` from the demo shell if you set it there. Stopping a client does not revoke tokens at the provider.

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
| 4 | 2026-09-28 | Claude | TASK-005/R26 (ADR-0001, proposal revision 8): `io --by target` gains ORIGIN (`client_secret env`, `auth`, `token_url`, `http get`), PHASE and NEEDS FILE. |
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest: env/credential/auth/network sites by target and `io --check-policy` (all allowed). |
| 2 | 2026-09-28 | Claude | UQ-17: declared open output; policy.json auto-discovered with `serve` surfaces/auth; `serve --listen` replaces `--transport http … --mcp`; outputs route; View outputs, strict-docs, bootstrap and exit codes. |
| 1 | 2026-09-28 | Codex | Added draft source files, prerequisites, invocation examples and expected behavior. |
