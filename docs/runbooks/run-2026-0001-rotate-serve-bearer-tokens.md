---
document_id: RUN-2026-0001
title: "Rotate rivet serve bearer tokens"
document_type: runbook
status: active
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 3
authors: [Claude]
owner: Project maintainer
systems: [Rivet]
components: [serve, auth, policy, cli]
affected_versions:
  from: "0.1.0"
  to: null
last_validation_date: 2026-09-28
applicable_environments: [development, server]
escalation_path: "Operator on duty → Project maintainer (owner of policy.json and the serve deployment)"
audience: [operators]
confidentiality: internal
scope: Replace a bearer token accepted by one `rivet serve` process with zero or near-zero client downtime.
reason: PLAN-2026-0001 D-31 — token rotation is a routine credential procedure for the serve surface.
related_documents: [PLAN-2026-0002, API-2026-0006, OPS-2026-0001, RUN-2026-0002, PLAN-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, runbook, serve, bearer, credentials, rotation]
---

# Rotate rivet serve bearer tokens

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** serve, auth, policy, cli

## Purpose

Replace the bearer token a client uses against `rivet serve` without ever storing the token itself on the server.
The server only knows `sha256(token)` in `policy.json` `serve.auth.tokens[]`. Rotation uses an **overlap window**:
both hashes are accepted, clients switch, then the old hash is removed.

```text
 phase:        0 before         1 overlap                    2 after
 policy.json   [old]            [old, new]                   [new]
 old token     200 ───────────▶ 200 ───────────────────────▶ 401 auth.invalid
 new token     401 ───────────▶ 200 ───────────────────────▶ 200
                     restart ▲        clients switch   restart ▲
```

## When to Use This Runbook

- Scheduled credential rotation.
- A token may have leaked (skip the overlap: go straight to phase 2 for that hash).
- A client or principal is being retired (remove its hash; also remove its `serve.principals` entry).

## Preconditions

- A `rivet` binary on the host (0.2.0: built or installed with `--features cli`, see
  [OPS-2026-0001](../operations/ops-2026-0001-operating-rivet-serve.md#installing-the-binary-020)); `openssl` and `shasum`
  (macOS) or `sha256sum` (Linux). Supported hosts: macOS and Linux (Windows is not supported in 0.2.0, INC-2026-0011).
- `serve.auth.type` is `bearer` in the `policy.json` used by the running server.
- You know the server's start command (entry file, `--listen`, optional `--policy`) and how to reach its PID.
- **Rivet (0.1.0 and 0.2.0) reads `policy.json` only at startup.** There is no reload signal; every phase change needs a restart
  (SIGINT, then start again). Plan for a brief outage per restart (about one second in validation).

## Required Access

- Write access to the server's `policy.json` and permission to signal/start the `rivet serve` process.
- A secure channel to hand the new token to the client owner.

## Safety Warnings

- Never put the raw token in `policy.json`, argv, environment variables, tickets or chat. Clients pass it with
  `--token-file PATH` (mode 600) or an `Authorization: Bearer` header.
- Hash the token **without a trailing newline** (`printf %s`). `echo` hashes `token\n` — a different digest — and the
  server returns 401 `auth.invalid` for the real token.
- A malformed hash (not 64 hex characters) makes the server refuse to start (`policy.invalid`, exit 2). Always run
  `rivet --file app.rivet check` after editing, before restarting.
- Stop with SIGINT (`kill -INT`) or SIGTERM (`kill -TERM`): since commit `829ca43` both drain (cancel in-flight
  requests and sessions, close their handles within 5 s) and exit 0. The procedure below uses SIGINT.

## Procedure

Validated example (0.2.0-rc): entry `app.rivet` (the 01-catalog demo), listener `127.0.0.1:18930`, principal `ci`
with `"operations": ["demo.*"]`. Run from the bundle directory. Hashes, `policy_hash` values and IDs below are from
that run and differ for every token.

```text
 ┌────────────┐   ┌──────────────┐   ┌──────────────┐   ┌──────────┐   ┌────────────┐   ┌──────────────┐   ┌──────────┐
 │1 generate  │──▶│2 hash + add  │──▶│3 check +     │──▶│4 verify  │──▶│5 hand over │──▶│6 remove old  │──▶│7 verify  │
 │  new token │   │  (overlap)   │   │  restart     │   │  both 200│   │  + switch  │   │  + restart   │   │ old 401  │
 └────────────┘   └──────────────┘   └──────────────┘   └──────────┘   └────────────┘   └──────────────┘   └──────────┘
```

### 1. Generate the new token

```sh
umask 077
openssl rand -hex 32 > new.token
ls -l new.token          # -rw-------  … new.token
```

### 2. Compute its hash and add it next to the old one

```sh
printf %s "$(cat new.token)" | shasum -a 256 | cut -d' ' -f1 > new.sha     # Linux: sha256sum
cat new.sha
# 1a36216a2ba82572b2fd2002f7d9cad0b65a7bc8fc07382923c623a9b67f1ba6   (differs per token)
cp policy.json policy.json.bak
```

Edit `policy.json` so the principal has **two** entries:

```json
{
  "version": 1,
  "serve": {
    "surfaces": ["http", "sse", "poll", "ws", "mcp"],
    "auth": {
      "type": "bearer",
      "tokens": [
        {"principal": "ci", "sha256": "fdc011bc798e29994a18df7f14f394961f30d1d27dd256b50bb2312f1b6170c6"},
        {"principal": "ci", "sha256": "1a36216a2ba82572b2fd2002f7d9cad0b65a7bc8fc07382923c623a9b67f1ba6"}
      ]
    },
    "principals": {"ci": {"operations": ["demo.*"]}}
  }
}
```

### 3. Check the file, then restart

```sh
rivet --file app.rivet check                       # ok: 4 operations, 0 connectors, 0 auth profiles   (exit 0)
kill -INT "$(cat serve.pid)"                        # old process exits 0
rivet --file app.rivet serve --listen 127.0.0.1:18930 2> serve.log & echo $! > serve.pid
cat serve.log
```

```text
{"listen_addr":"127.0.0.1:18930","stdio":false,"surfaces":["http","sse","poll","ws","mcp"],"auth_type":"bearer","catalog_version":"sha256:67104f0e7faaeafee253db758a668a9b24aa4263677e9d5ea2451b32019f9730","policy_hash":"sha256:a1a8c93b90f3028e6775bb4cc11d40bcc36ec29a1c6eb998b920f8c031a65fd0"}
```

The `policy_hash` must differ from the value before the edit.

### 4. Verify both tokens are accepted

```sh
for t in old new; do
  curl -s -o /dev/null -w "$t token: HTTP %{http_code}\n" http://127.0.0.1:18930/v1/operations \
       -H "Authorization: Bearer $(cat $t.token)"
done
```

```text
old token: HTTP 200
new token: HTTP 200
```

### 5. Hand over the new token and switch clients

Deliver `new.token` over the secure channel. Each client replaces its token file and confirms:

```sh
rivet --endpoint http://127.0.0.1:18930 --token-file new.token list     # table of demo.* operations, exit 0
rivet --endpoint http://127.0.0.1:18930 --token-file new.token request demo.add --data '{"a":2,"b":3}'
```

```text
{"request_id":"req_04a9ceae9c","trace_id":"tr_04a9ceae9c","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}
```

Clients that call the HTTP API directly send `{"operation": …, "data": …}` (0.2.0). A client that still sends the
0.1.0 `{"id", "params"}` body works through 0.2.x, but its responses carry `deprecation: true` and its access-log
lines carry `"deprecated":1`. Use the token switch as a chance to move it
([MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md)).

Wait until every client using the old token has switched.

### 6. Remove the old hash and restart

```sh
cp policy.json policy.json.overlap
# delete the {"principal":"ci","sha256":"<old hash>"} entry from serve.auth.tokens
rivet --file app.rivet check
kill -INT "$(cat serve.pid)"
rivet --file app.rivet serve --listen 127.0.0.1:18930 2> serve.log & echo $! > serve.pid
cat serve.log            # policy_hash changes again (sha256:56307f14… in validation)
```

### 7. Verify the old token is refused

```sh
curl -s -w ' HTTP %{http_code}\n' http://127.0.0.1:18930/v1/operations -H "Authorization: Bearer $(cat old.token)"
rivet --endpoint http://127.0.0.1:18930 --token-file old.token list; echo "exit=$?"
rivet --endpoint http://127.0.0.1:18930 --token-file new.token list; echo "exit=$?"
```

## Expected Results

```text
{"request_id":"","trace_id":"","operation":"rivet.list","type":"result","status":"error","data":null,"error":{"kind":"auth","code":"auth.invalid","message":"invalid bearer token","retryable":false},"effects":"none","data_count":0} HTTP 401
error[auth.invalid]: invalid bearer token
exit=3
ID              NAME                DESCRIPTION
demo.greet      Greet a person      Return a greeting for the supplied person.
demo.add        Add two integers    Add two signed integers and return their sum.
demo.health     Check availability  Return a constant readiness response without I/O.
demo.countdown  Count down          Emit 3, 2, 1 as data items and then return a summary.
exit=0
```

## Verification

| Check | Pass condition |
|---|---|
| Startup receipt | `auth_type` is `bearer`; `policy_hash` equals `sha256` of the deployed `policy.json` (`shasum -a 256 policy.json`) |
| Old token | HTTP 401 `auth.invalid`; CLI exit 3 |
| New token | HTTP 200 on `GET /v1/operations`; CLI `list` exit 0 |
| Authorization unchanged | a call outside the principal's listing (e.g. `request rivet.io`) still gives 403 `permission.denied`, exit 3 (a `rivet.io` envelope with `status: "error"`) |
| No token material in files | `grep -c "$(cat new.token)" policy.json serve.log` prints 0 for both |

## Rollback

Rollback is restoring the previous file and restarting — the same mechanism as the change.

```text
  after step 6 went wrong          after step 3 went wrong
  cp policy.json.overlap policy.json   cp policy.json.bak policy.json
              └──────────────┬──────────────┘
                   rivet --file app.rivet check
                   kill -INT <pid>; start serve again
                   verify with the curl loop from step 4
```

Validated: restoring `policy.json.overlap` and restarting made both tokens return 200 again.

## Failure Scenarios

| Symptom | Cause | Action |
|---|---|---|
| New token gets 401 after the edit | server not restarted (no hot reload), or hash made with `echo` (includes newline) | restart; recompute with `printf %s` |
| Server exits 2: `policy.invalid … /serve/auth/tokens/N/sha256: must be 64 hex characters` | truncated or pasted-wrong hash | fix the hash, `check`, start again |
| Server exits 2: `policy.invalid … unknown key` | typo in a key | fix the key named in the JSON pointer |
| Server exits 5: `connection.bind … Address already in use` | old process still running | `kill -INT` it and wait; confirm with `lsof -i :PORT` |
| New token: `POST /v1/request` 403 `permission.denied` ("principal `cii` may not call …"), `GET /v1/operations` returns an envelope whose `data` is `{"operations":[],"next_cursor":null}` | new hash assigned to a principal name with no `serve.principals` entry (typo) | use the same `principal` value as in `serve.principals` |
| Client CLI exit 2: `--token-file X must hold exactly one token line` or `--token-file X: entity not found` | token file has several lines, or is missing | write exactly one token line to an existing file |

## Escalation

Operator on duty → Project maintainer (owner of `policy.json` and the serve deployment). Escalate immediately if a
token is suspected leaked and the server cannot be restarted, or if an unknown hash appears in `policy.json`.

## Related Monitoring

Rivet writes the startup receipt and one JSON access-log line per request to stderr. Monitor: the receipt's
`policy_hash` after each restart; the liveness probe `GET /v1/health` with the probe's own token on a non-loopback
bind (expect 200 and a `rivet.health` envelope with `"data":{"status":"ok",…}`); the access log's `status` 401 lines (and their `principal: null`) during the
overlap window — a rise after step 6 means a client still sends the old token. See [OPS-2026-0001 Health Checks](../operations/ops-2026-0001-operating-rivet-serve.md#health-checks).

## Last Validation Date

2026-09-29: executed end to end (steps 1–7, the verification table, rollback, and the `cii` typo and token-file
failure scenarios) against the 0.2.0 release candidate. The binary was `target/release/rivet`, built with
`cargo build --release --features cli` at main `8031baa`, on macOS arm64, with listener `127.0.0.1:18930` and a
temporary copy of the 01-catalog bundle. The server was stopped afterwards. The procedure is unchanged from 0.1.0;
only the outputs are now envelopes.

History: 2026-09-28 — executed end to end (steps 1–7 and rollback) against `target/debug/rivet` 0.1.0-dev (commit
`f40d4aa`) on macOS arm64, listener `127.0.0.1:18471`, using a temporary copy of the 01-catalog bundle. Hashes and
request IDs of that run are no longer shown. The fix-batch changes that touch this runbook (SIGTERM drains with exit 0,
`/v1/health`, the access log) were verified separately at commit `829ca43` (see OPS-2026-0001); the procedure's
commands are unchanged.

## Related Documents

- [OPS-2026-0001 Operating rivet serve](../operations/ops-2026-0001-operating-rivet-serve.md)
- [RUN-2026-0002 Roll out a policy change](run-2026-0002-roll-out-policy-change.md)
- [Demo 01 catalog](../demos/01-catalog/README.md)
- [MIG-2026-0001 Migrating to 0.2.0](../migrations/mig-2026-0001-response-and-input-envelopes.md) · [API-2026-0006 Envelopes](../api/api-2026-0006-envelopes.md)
- [Runbooks index](index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial runbook, validated against 0.1.0-dev (f40d4aa). |
| 2 | 2026-09-28 | Claude | Fix batch (829ca43): SIGTERM now drains like SIGINT; monitoring uses `/v1/health` and the per-request access log. Procedure unchanged. |
| 3 | 2026-09-29 | Claude | PLAN-2026-0002 D-38 (TASK-079): re-executed on the 0.2.0-rc; outputs as envelopes, `request --data` check in step 5, note on legacy clients and `Deprecation`, `cli` feature prerequisite, macOS/Linux only. |
