---
document_id: OPS-2026-0001
title: "Operating rivet serve"
document_type: operations
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 2
authors: [Claude]
owner: Project maintainer
component_owner: Project maintainer
systems: [Rivet]
components: [serve, auth, policy, audit, http, ws, poll, mcp, sessions, cli]
affected_versions:
  from: "0.1.0"
  to: null
last_verified_version: "0.1.0-dev (commit 829ca43)"
applicable_environments: [development, server]
audience: [operators, maintainers]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-28
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
(`rivet 0.1.0-dev`, commit `f40d4aa`; health, access log, drain and limits rows re-run at commit `829ca43`) on macOS
on 2026-09-28. Request, trace and catalog IDs and hashes differ on
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
| `max_buffered_bytes` | `policy.json` `limits` | 268435456 (256 MiB) | bytes retained by all session queues of the process; a new event beyond it fails `limit.buffered_bytes` (HTTP 429, exit 5) |
| request deadline | `deadline_ms` on `/v1/request` and `POST /v1/requests`, `--timeout` on the CLI | 30000 ms | `timeout` (HTTP 504, exit 6); capped at 600000 (the CLI refuses a larger `--timeout`, exit 2) |
| outbound frame/body/item | built in (`max_body`, `max_frame` options raise per call) | 8 MiB | `limit.*` (exit 5) |
| sessions per principal | built in | 8 | polling/WS session refused |
| session idle lease | built in | 60 s | session cancelled with `cancelled.idle` by a background sweeper |
| WS refs per connection | built in | 8 | further `ref`s on that connection refused |
| cleanup grace | built in | 5 s | `with`-block handles closed in reverse order within 5 s |

`rivet policy explain` prints the effective limits, for example
`limits   64 concurrent, depth 16, 268435456 buffered bytes`. Non-positive limits and values wider than their
field (`must be at most 4294967295`) are `policy.invalid`.

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
  error `code`) and the access log in your own logs if you need history. To keep one trace, export it from the
  serving process before it restarts: `rivet --endpoint URL trace export REQ --output ./audit/REQ.json` (the
  file is written on the server, inside the bundle, and needs an `allow_write` `create` grant and an exact
  `rivet.trace.export` listing for network principals); a library host uses `Runtime::export_trace`.
- Correlate with your own tracing: send a W3C `traceparent` header and its trace-id becomes the request's
  `trace_id`; responses carry `traceparent` back.

Example (`auth none`, loopback):

```text
$ rivet --endpoint http://127.0.0.1:18481 trace show req_03e2a6bdff
{"request_id":"req_03e2a6bdff","attempts":[{"request_id":"req_03e2a6bdff","trace_id":"tr_03e2a6bdff","node_id":null,"attempt":1,"effect_id":"notes.delete#1","operation_id":"notes.delete","phase":"decision","capability":"allow_delete","access":"delete","target":"./out/note.json","decision":"denied","policy_hash":"sha256:8cd42eb2…","source":null,"outcome":{"rule":"no grant for allow_delete ./out/note.json"}}],"complete":true,"next_cursor":null,"gaps":0}
```

## Logs and Exit Codes

`rivet serve` writes a JSON **startup receipt on stderr**, then **one JSON access-log line per request** on stderr:
`{time, surface, method, route, principal, operation, status, duration_ms}` — the route is the matched pattern
(never the query string), and params, bodies and tokens are never logged. Capture stderr from your supervisor.

```text
$ rivet --file app.rivet serve --listen 127.0.0.1:18901 2> serve.log &          # commit 829ca43
$ cat serve.log
{"listen_addr":"127.0.0.1:18901","stdio":false,"surfaces":["http","sse","poll","ws","mcp"],"auth_type":"none","catalog_version":"sha256:d222025d…0627","policy_hash":"sha256:de3e7b37…1398"}
{"time":"2026-09-28T09:47:49.689Z","surface":"http","method":"GET","route":"/v1/health","principal":null,"operation":"health","status":200,"duration_ms":1}
{"time":"2026-09-28T09:47:49.715Z","surface":"http","method":"POST","route":"/v1/request","principal":"local","operation":"demo.read","status":200,"duration_ms":8}
{"time":"2026-09-28T09:47:49.731Z","surface":"http","method":"POST","route":"/v1/request","principal":"local","operation":"demo.read","status":403,"duration_ms":1}
```

```text
 request ──▶ surface router ──▶ handler ──▶ response
                   └── access_log middleware: after the response ──▶ stderr (or a library access_log sink)
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
| SIGINT (Ctrl-C) while serving | drain | 0 |
| SIGTERM while serving | drain (same as SIGINT) | 0 |

**Drain.** SIGINT and SIGTERM behave the same: stop accepting, cancel every in-flight request and session
(sessions end `cancelled.shutdown`; each run closes its handles in reverse order within the 5 s grace), wait up to
6 s for responses to flush, exit 0. Verified at `829ca43` with a live polling session:

```text
$ rivet --file app.rivet serve --listen 127.0.0.1:18902 2> s2.err & P=$!
$ curl -s -X POST http://127.0.0.1:18902/v1/requests -d '{"id":"chat.echo","params":{}}' >/dev/null
$ kill -TERM $P; wait $P; echo "exit=$?"
exit=0
```

Supervisors that stop services with SIGTERM (systemd, Kubernetes, launchd) therefore get a clean drain. The full CLI exit-code registry: 0 ok, 2 syntax/validation/config, 3 permission/auth,
4 not_found/conflict, 5 dependency/runtime/unsupported/output_invalid/limit, 6 timeout, 7 inspection incomplete,
130 cancelled.

## Health Checks

`GET /v1/health` answers `{"status":"ok","catalog_version":"sha256:…"}`. It is mounted whatever
`serve.surfaces` says, needs no credentials on a loopback bind, and is authenticated like every route on any
other bind (any valid bearer token; no `serve.principals` entry is needed). `GET /healthz` answers 404.

```text
 liveness   GET /v1/health            200 {"status":"ok","catalog_version":…} ⇒ listener up, catalog loaded
            (non-loopback: send the probe's bearer token)
 readiness  POST /v1/request {"id":"<a pure operation>"}   e.g. demo.health → {"ready":true}
 config     compare the startup receipt's policy_hash / catalog_version with the files you deployed
 policy     rivet --file app.rivet io --check-policy        (offline; exit 0 = every site allowed, 3 = something denied)
```

```text
$ rivet --file app.rivet --policy policies/team.json serve --listen 0.0.0.0:18905 &      # 01-catalog, commit 829ca43
$ curl -s -i http://127.0.0.1:18905/v1/health                                           # no token
HTTP/1.1 401 Unauthorized
{"request_id":"","trace_id":"","error":{"kind":"auth","code":"auth.required","message":"missing bearer token","retryable":false,"effects":"none"}}
$ curl -s -H 'Authorization: Bearer dev-token-ci' http://127.0.0.1:18905/v1/health
{"status":"ok","catalog_version":"sha256:67104f0e7faaeafee253db758a668a9b24aa4263677e9d5ea2451b32019f9730"}
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
3. `kill -TERM <pid>` (or `-INT`); wait for exit 0 (the drain takes at most ~6 s).
4. Start the new binary with the same `--listen` and files; confirm the startup receipt's `policy_hash` and
   `catalog_version` are unchanged.
5. Run the liveness and readiness probes.
6. Rollback: `kill -TERM`, start `rivet.prev` with the same arguments, re-probe.

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
| Memory ceiling for buffered data | `max_buffered_bytes` (default 256 MiB, enforced across all session queues) plus up to 10 000 trace events |
| Sessions | 8 per principal, 16 queued frames, 32 MiB queue, 60 s idle lease |
| Horizontal scale | run independent processes on different ports; nothing is shared between them (traces, sessions and MCP session IDs are per process), so a load balancer must keep a session's requests on one process |

## Security Considerations

- Keep the listener on loopback unless `serve.auth` is `bearer`; Rivet enforces this at startup.
- Rivet serves plain HTTP. Put TLS in front for anything leaving the host.
- Store tokens in files with mode 600 and pass them with `--token-file` (never argv or env).
- Give each client its own principal and the narrowest `operations` listing; sensitive built-ins need exact entries,
  and a `"*"` listing also reaches `rivet.auth.*` (keep `allow_auth` narrow).
- Clients may narrow individual requests with `restrict {grants}`; it can never widen `policy.json`.
- `policy.json` is the security boundary: review every change with `io --check-policy` and `policy explain`
  ([RUN-2026-0002](../runbooks/run-2026-0002-roll-out-policy-change.md)).

## Known Limitations

The operational rows of the [manual's Known Limitations](../manuals/man-2026-0001-rivet-manual.md#known-limitations):
mTLS serve (refuses to start, exit 5); no persistent trace store; no connection pooling for outbound HTTP; the Linux
sandbox is gated until verified on kernel ≥ 6.12 and Windows/other OSes are unsupported (process-spawning
operations under a policy cannot run there); `--timeout` is not applied over the WebSocket duplex path; the `"*"`
principal pattern matches `rivet.auth.*`. Configuration is not reloaded without a restart (by design).

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
| 2 | 2026-09-28 | Claude | Fix batch through 829ca43: `/v1/health` probes, per-request access log, SIGTERM drains (exit 0) and upgrade steps use it, enforced `max_buffered_bytes`, `--timeout` cap, 8 MiB outbound bounds, `traceparent`, `restrict`; limitations aligned with the manual. |
