---
document_id: OPS-2026-0001
title: "Operating rivet serve"
document_type: operations
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
component_owner: Project maintainer
systems: [Rivet]
components: [serve, auth, policy, audit, http, ws, poll, mcp, sessions, cli]
affected_versions:
  from: "0.1.0"
  to: null
last_verified_version: "0.1.0-dev (commit f40d4aa)"
applicable_environments: [development, server]
audience: [operators, maintainers]
confidentiality: internal
review_cycle: on-release
scope: How to deploy, configure, observe, upgrade and roll back one `rivet serve` process in Rivet 0.1.0.
reason: PLAN-2026-0001 D-30 — `rivet serve` is a new network surface and needs a current-state operations guide.
related_documents: [PLAN-2026-0001, PROP-2026-0001, ADR-0002, RUN-2026-0001, RUN-2026-0002, REF-2026-0001, TRBL-2026-0003]
supersedes: null
superseded_by: null
tags: [rivet, operations, serve, auth, policy, deployment]
---

# Operating rivet serve

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** serve, auth, policy, audit, http, ws, poll, mcp, sessions, cli

## Summary

`rivet serve` exposes one Rivet bundle (an entry `.rivet` file plus its `policy.json`) over a single TCP listener.
Every surface — REST, SSE, polling, WebSocket and MCP (Streamable HTTP) — shares that listener, one catalog, one
dispatcher and one authenticator. This guide covers the deployment shapes, the configuration an operator owns
(bind address, `serve.auth`, `serve.principals`, `serve.surfaces`, `limits`), what the process writes, how to check
health, and how to upgrade and roll back. Every command and output below was run against the built binary
(`rivet 0.1.0-dev`, commit `f40d4aa`) on macOS on 2026-09-28. Request, trace and catalog IDs and hashes differ on
each run.

```text
                         one process, one listener, one policy snapshot
 ┌──────────────────────────────────────────────────────────────────────────────────┐
 │ rivet --file app.rivet serve --listen 127.0.0.1:8080                              │
 │                                                                                  │
 │   startup: parse + check bundle ─▶ load policy.json once ─▶ bind ─▶ mount surfaces│
 │                                                                                  │
 │   ┌─ http ─ POST /v1/request · GET /v1/operations[/{id}[/outputs]] · GET /v1/io   │
 │   │         POST /v1/policy/generate                                             │
 │   ├─ sse ── POST /v1/request with Accept: text/event-stream                       │
 │   ├─ poll ─ POST /v1/requests · /v1/requests/{id}/events|input|finish_input|cancel│
 │   ├─ ws ─── GET /v1/ws  (subprotocol rivet.v1)                                    │
 │   └─ mcp ── POST|GET|DELETE /mcp  (Streamable HTTP; GET answers 405)              │
 │                         │                                                        │
 │        every request: authenticate_principal ─▶ authorize_operation ─▶ dispatcher │
 │                                                      │                           │
 │                                   effects ─▶ policy broker ─▶ bounded trace store │
 └──────────────────────────────────────────────────────────────────────────────────┘
```

## Deployment Shapes

Rivet 0.1.0 ships a single binary and no service manager integration. Pick one of three shapes.

```text
 A. local / developer (default)            B. shared host behind a TLS proxy
 ┌──────────────┐                          ┌──────────┐  TLS  ┌─────────────┐ plain ┌──────────────┐
 │ client on the│ http://127.0.0.1:8080    │ clients  │──────▶│ TLS proxy   │──────▶│ rivet serve  │
 │ same machine │─────────────────────────▶│ (bearer) │       │ (nginx, …)  │       │ 127.0.0.1:N  │
 └──────────────┘  auth none → `local`     └──────────┘       └─────────────┘       │ auth bearer  │
                                                                                    └──────────────┘
 C. MCP host over stdio
 ┌──────────────┐ stdin/stdout JSON-RPC ┌───────────────────────────────┐
 │ MCP client   │◀─────────────────────▶│ rivet --file app.rivet serve  │   binds nothing; MCP only
 │ (spawns it)  │                       │       --stdio                 │
 └──────────────┘                       └───────────────────────────────┘
```

| Shape | Command | `serve.auth` | Notes |
|---|---|---|---|
| A. Local | `rivet --file app.rivet serve` | `none` (or absent) | Default listen `127.0.0.1:8080`; every caller is principal `local` |
| B. Shared host | `rivet --file app.rivet serve --listen 127.0.0.1:18080` behind a TLS-terminating proxy | `bearer` | Rivet has no TLS listener in 0.1.0; terminate TLS in the proxy and forward the `Authorization` header |
| B'. Direct non-loopback | `rivet --file app.rivet serve --listen 10.0.0.5:8080` | `bearer` | Plain HTTP on the wire; bearer tokens travel in clear text — use only on a trusted network |
| C. MCP stdio | `rivet --file app.rivet serve --stdio` | not used | MCP over stdin/stdout only; no socket is bound |

`serve.auth.type: "mtls"` is accepted by the schema but refuses to start in 0.1.0 (see
[Startup refusals](#startup-refusals-and-exit-codes)); use shape B.

## Configuration

Everything an operator configures lives in `policy.json`: beside the entry file, or the file named by
`--policy PATH`. There is no environment-variable or command-line override for grants, auth or limits.
The file is read **once at startup**; the running process never re-reads it (verified: a new token hash added to the
file was rejected with 401 until the process was restarted).

```text
 policy.json (schema version 1; unknown keys are policy.invalid, exit 2)
 ├── version: 1
 ├── grants[] / deny[]      effect permissions  → RUN-2026-0002
 ├── network                { deny_private_ranges: true }
 ├── limits                 { max_concurrent_requests, max_call_depth, max_buffered_bytes }
 ├── approved               { snapshots[], overlaps[] }
 └── serve
     ├── surfaces[]         subset of http, sse, poll, ws, mcp   (default: all five)
     ├── auth               { type: none } | { type: bearer, tokens: [{principal, sha256}] }
     │                      | { type: mtls, … }  ← refuses to start in 0.1.0
     └── principals         { "<principal>": { operations: ["demo.*", "demo.health", "*"] } }
```

### Bind address

| `--listen` value | Result |
|---|---|
| omitted | `127.0.0.1:8080` |
| `127.0.0.1:PORT`, `[::1]:PORT`, `localhost:PORT` | loopback; `auth none` allowed |
| any other IP literal | non-loopback; requires `serve.auth` bearer, else `serve.auth_required` (exit 2) before binding |
| a host name other than `localhost`, or no port | `validation.usage` (exit 2) |
| `HOST:0` | the OS picks a port; the startup receipt reports the real one |

### Authentication and principals

```text
 request ──▶ serve.auth.type?
              │
              ├─ none ──── loopback bind? ── yes ─▶ principal "local" (may call everything)
              │                          └─ no ──▶ refused at startup (serve.auth_required)
              │
              └─ bearer ── Authorization: Bearer <token> present?
                              │ no ─▶ 401 auth.required
                              ▼
                           sha256(token) (lower-case hex) equals any tokens[].sha256?  (constant time)
                              │ no ─▶ 401 auth.invalid   (no hint which tokens exist)
                              ▼
                           principal = that entry's "principal"
                              │
                              ▼
                 serve.principals present?
                   ├─ no ─▶ every public operation EXCEPT the four sensitive built-ins below
                   └─ yes ─▶ principal's operations[] must match: exact id · "*" · "prefix.*"
                             sensitive built-ins (rivet.io, rivet.policy.generate, rivet.trace.show,
                             rivet.connectors.sync) match only an EXACT entry
                             no match ─▶ 403 permission.denied
```

A token's hash is `printf %s "$TOKEN" | shasum -a 256` (no trailing newline). Several entries may share one principal;
that is how rotation overlaps old and new tokens ([RUN-2026-0001](../runbooks/run-2026-0001-rotate-serve-bearer-tokens.md)).
The raw token never appears in `policy.json`, in the startup receipt or in any response.

Observed on 2026-09-28 against a bearer listener where principal `ci` has `operations: ["demo.*"]`:

```text
$ curl -s -X POST http://127.0.0.1:18471/v1/request -H 'content-type: application/json' \
       -d '{"id":"demo.add","params":{"a":2,"b":3}}'
{"request_id":"","trace_id":"","error":{"kind":"auth","code":"auth.required","message":"missing bearer token","retryable":false,"effects":"none"}}      ← HTTP 401

$ curl -s -X POST http://127.0.0.1:18471/v1/request -H "Authorization: Bearer $(cat old.token)" \
       -H 'content-type: application/json' -d '{"id":"demo.add","params":{"a":2,"b":3}}'
{"request_id":"req_01370d2b15","trace_id":"tr_01370d2b15","result":5,"data_count":0,"effects":"none"}   ← HTTP 200

$ rivet --endpoint http://127.0.0.1:18471 --token-file old.token request rivet.io
{"request_id":"req_0336affb17","trace_id":"tr_0336affb17","error":{"kind":"permission","code":"permission.denied","message":"principal `ci` may not call `rivet.io`","retryable":false,"effects":"none"}}
$ echo $?
3
```

### Surfaces

`serve.surfaces` selects which route groups are mounted. A surface that is not listed is never mounted and its
routes answer `404 not_found.route`. Observed with `"surfaces": ["http", "mcp"]`:

| Probe | Status |
|---|---|
| `GET /v1/operations` | 200 |
| `GET /v1/requests/x/events` (poll not mounted) | 404 `not_found.route` |
| `GET /v1/ws` (ws not mounted) | 404 |
| `GET /mcp` | 405 (MCP Streamable HTTP uses POST; DELETE ends a session) |

### Limits

| Setting | Where | Default | Effect when exceeded |
|---|---|---|---|
| `max_concurrent_requests` | `policy.json` `limits` | 64 | new request fails `limit.concurrency` (HTTP 429, exit 5) |
| `max_call_depth` | `policy.json` `limits` | 16 | nested `(request …)` chain refused with a `limit.*` error |
| `max_buffered_bytes` | `policy.json` `limits` | 268435456 (256 MiB) | buffering beyond the bound fails with a `limit.*` error |
| request deadline | `deadline_ms` on `/v1/request`, `--timeout` on the CLI | 30000 ms | `timeout` (HTTP 504, exit 6); `deadline_ms` is capped at 600000 |
| sessions per principal | built in | 8 | polling/WS session refused |
| session idle lease | built in | 60 s | session cancelled with `cancelled.idle` |
| WS refs per connection | built in | 8 | further `ref`s on that connection refused |
| cleanup grace | built in | 5 s | `with`-block handles closed in reverse order within 5 s |

`rivet policy explain` prints the effective limits, for example
`limits   64 concurrent, depth 16, 268435456 buffered bytes`. Non-positive limits are `policy.invalid`.

## Trace Storage

Effect decisions (allowed/denied, rule, `policy_hash`, access verb, target) go to an **in-memory, bounded** trace
store inside the serve process.

```text
  effect attempt ──▶ policy broker decision ──▶ MemoryTraceStore (ring, 10 000 events)
                                                    │ full? evict oldest, count the gap
                                                    ▼
                    rivet --endpoint URL trace show <request_id>   (local principal or exact listing)
                                                    │
                          process exits or restarts ─▶ everything is gone
```

- Capacity: 10 000 events per process (oldest evicted first; `gaps` counts evicted events for a request).
- Persistence: none. A restart empties it; after the policy rollout restart, `trace show` for a pre-restart
  request returned `not_found.trace` (exit 4).
- Access: `rivet.trace.show` is a sensitive built-in: the loopback `local` principal, or a principal listing
  `rivet.trace.show` exactly.
- A persistent trace store is a known 0.1.0 limitation; keep the response envelopes (`request_id`, `trace_id`,
  error `code`) in your own client logs if you need history.

Example (`auth none`, loopback):

```text
$ rivet --endpoint http://127.0.0.1:18481 trace show req_03e2a6bdff
{"request_id":"req_03e2a6bdff","attempts":[{"request_id":"req_03e2a6bdff","trace_id":"tr_03e2a6bdff","node_id":null,"attempt":1,"effect_id":"notes.delete#1","operation_id":"notes.delete","phase":"decision","capability":"allow_delete","access":"delete","target":"./out/note.json","decision":"denied","policy_hash":"sha256:8cd42eb2…","source":null,"outcome":{"rule":"no grant for allow_delete ./out/note.json"}}],"complete":true,"next_cursor":null,"gaps":0}
```

## Logs and Exit Codes

`rivet serve` writes exactly one line on success — a JSON **startup receipt on stderr** — and nothing per request
(0.1.0 installs no log subscriber). Capture stderr from your supervisor.

```text
$ rivet --file app.rivet serve --listen 127.0.0.1:18471 2> serve.log &
$ cat serve.log
{"listen_addr":"127.0.0.1:18471","stdio":false,"surfaces":["http","sse","poll","ws","mcp"],"auth_type":"bearer","catalog_version":"sha256:67104f0e…","policy_hash":"sha256:ea001264…"}
```

| Receipt field | Use |
|---|---|
| `listen_addr` | the bound address (real port when `:0`) |
| `surfaces` | the mounted surfaces, canonical order |
| `auth_type` | `none` or `bearer` |
| `catalog_version` | sha256 of the bundle — changes when `.rivet` sources change |
| `policy_hash` | sha256 of the `policy.json` bytes in force (`null` when no policy file) — the value to compare after a rollout |

### Startup refusals and exit codes

Startup errors are one JSON error envelope (or an `error[code]: message` line for policy parse errors) on stderr.
All rows below were reproduced on 2026-09-28.

| Situation | Code | Exit |
|---|---|---|
| auth none on `0.0.0.0:18473` | `serve.auth_required` | 2 |
| `serve.auth.type: "mtls"` | `unsupported.serve_mtls` | 5 |
| bad token hash (`"sha256":"abc"`) | `policy.invalid` — `/serve/auth/tokens/0/sha256: must be 64 hex characters` | 2 |
| unknown key under `serve` | `policy.invalid` — `/serve/extra: unknown key` | 2 |
| port already in use | `connection.bind` — `Address already in use (os error 48)` | 5 |
| SIGINT (Ctrl-C) while serving | graceful shutdown | 0 |
| SIGTERM while serving | default signal disposition, no drain | 143 |

Stop the server with **SIGINT** (`kill -INT PID`), not SIGTERM, when you want in-flight work to be shut down
through the listener. The full CLI exit-code registry: 0 ok, 2 syntax/validation/config, 3 permission/auth,
4 not_found/conflict, 5 dependency/runtime/unsupported/output_invalid/limit, 6 timeout, 7 inspection incomplete,
130 cancelled.

## Health Checks

There is no dedicated health route (`GET /healthz` answers 404). Use these instead:

```text
 liveness   GET /v1/operations        200 ⇒ listener up, auth works, catalog loaded
            (send the probe's bearer token; give the probe principal a narrow listing such as ["demo.health"])
 readiness  POST /v1/request {"id":"<a pure operation>"}   e.g. demo.health → {"ready":true}
 config     compare the startup receipt's policy_hash / catalog_version with the files you deployed
 policy     rivet --file app.rivet io --check-policy        (offline; exit 0 = every site allowed, 3 = something denied)
```

```text
$ curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:18471/v1/operations -H "Authorization: Bearer $(cat new.token)"
200
```

## Upgrade and Rollback

The process holds no durable state (traces and sessions are in memory; OAuth tokens live in the configured
credential store, not in the process). An upgrade is therefore stop → replace → start.

```text
   ┌──────────┐  check new build  ┌─────────┐ SIGINT ┌─────────┐ start new ┌──────────┐ probe ok? ┌──────┐
   │ running  │──────────────────▶│ staged  │───────▶│ stopped │──────────▶│ starting │──────────▶│ live │
   │ v_old    │ `check`, `io      │ binary  │        │ (port   │           │ v_new    │           │v_new │
   └──────────┘  --check-policy`  └─────────┘        │  free)  │           └────┬─────┘           └──────┘
        ▲                                            └─────────┘                │ exit ≠ 0 or probe fails
        └──────────────── restore previous binary / policy.json and start ◀─────┘
```

1. Build or fetch the new binary; keep the previous one (`rivet.prev`).
2. Validate the unchanged bundle with the new binary, offline:
   `rivet-new --file app.rivet check` (expect `ok: N operations, …`, exit 0) and
   `rivet-new --file app.rivet io --check-policy` (same decisions as before).
3. `kill -INT <pid>`; wait for exit 0.
4. Start the new binary with the same `--listen` and files; confirm the startup receipt's `policy_hash` and
   `catalog_version` are unchanged.
5. Run the liveness and readiness probes.
6. Rollback: `kill -INT`, start `rivet.prev` with the same arguments, re-probe.

Expect a short outage between steps 3 and 4: two processes cannot share the port (the second one exits 5 with
`connection.bind`). Clients should retry `limit.*` and connection errors; in-flight requests, sessions and traces
of the old process are lost. Policy-only changes follow
[RUN-2026-0002](../runbooks/run-2026-0002-roll-out-policy-change.md); token changes follow
[RUN-2026-0001](../runbooks/run-2026-0001-rotate-serve-bearer-tokens.md).

## Capacity Notes

| Item | Measured / configured value |
|---|---|
| Debug binary `target/debug/rivet` | 55 611 896 bytes (≈ 53 MiB), macOS arm64 |
| Release binary `target/release/rivet` (`cargo build --release`, default release profile) | 17 043 360 bytes (≈ 16 MiB), macOS arm64; `target/release/` ≈ 523 MiB |
| Full `target/` directory after debug build + tests | ≈ 14 GiB — keep ≥ 20 GiB free when building ([TRBL-2026-0003](../troubleshooting/trbl-2026-0003-linker-fails-with-no-space-left-on-device.md)) |
| Concurrency | `max_concurrent_requests` (default 64) per process |
| Memory ceiling for buffered data | `max_buffered_bytes` (default 256 MiB) plus up to 10 000 trace events |
| Sessions | 8 per principal, 16 queued frames, 32 MiB queue, 60 s idle lease |
| Horizontal scale | run independent processes on different ports; nothing is shared between them (traces, sessions and MCP session IDs are per process), so a load balancer must keep a session's requests on one process |

## Security Considerations

- Keep the listener on loopback unless `serve.auth` is `bearer`; Rivet enforces this at startup.
- Rivet serves plain HTTP. Put TLS in front for anything leaving the host.
- Store tokens in files with mode 600 and pass them with `--token-file` (never argv or env).
- Give each client its own principal and the narrowest `operations` listing; sensitive built-ins need exact entries.
- `policy.json` is the security boundary: review every change with `io --check-policy` and `policy explain`
  ([RUN-2026-0002](../runbooks/run-2026-0002-roll-out-policy-change.md)).

## Known Limitations

mTLS serve, persistent trace store, per-request access logs, configuration reload without restart, connection
pooling for outbound HTTP, Linux/Windows process sandbox, and `--timeout` over the WebSocket duplex path are not
available in 0.1.0.

## Related Documents

- [RUN-2026-0001 Rotate serve bearer tokens](../runbooks/run-2026-0001-rotate-serve-bearer-tokens.md)
- [RUN-2026-0002 Roll out a policy change](../runbooks/run-2026-0002-roll-out-policy-change.md)
- [REF-2026-0001 Request and evidence](../references/ref-2026-0001-request-and-evidence.md)
- [Demo 01 catalog (bearer + principals example)](../demos/01-catalog/README.md)
- [ADR-0002 Rust crate selection](../decisions/adr-0002-rust-crate-selection.md)
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md)
- [Operations index](index.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial operations guide, verified against 0.1.0-dev (f40d4aa). |
