---
document_id: SEC-2026-0001
title: "Rivet policy and sandbox security model"
document_type: security
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 2
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [policy, files, transports, datagrams, quic, grpc, connectors, auth, serve, mcp, audit]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, embedded, server]
audience: [maintainers, operators, security-reviewers]
scope: Threat model of Rivet 0.1.0 and what the policy broker, file confinement, network checks, process sandbox, secrets handling, OAuth, MCP connectors and serve authentication do and do not guarantee, with the bootstrap I/O list and the per-OS sandbox matrix.
reason: DOCUMENTATION.md §31 security-boundary impact for PLAN-2026-0001 row D-29; 0.1.0 introduces the effect broker, deny-by-default policy.json and network serving, and operators need an honest statement of guarantees and non-guarantees.
related_documents: [PLAN-2026-0001, PROP-2026-0001, ADR-0003, INC-2026-0001, ARCH-2026-0001, API-2026-0001, API-2026-0003]
supersedes: null
superseded_by: null
tags: [rivet, security, policy, sandbox, ssrf, oauth, mcp, threat-model]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.1.0-dev (commit 829ca43)"
next_review_date: 2026-10-28
---

# Rivet policy and sandbox security model

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** policy, files, transports, datagrams, quic, grpc, connectors, auth, serve, mcp, audit

## Summary

Rivet's trust boundary is the **effect broker**: every file, network, process, environment, credential, MCP and gRPC effect that a `.rivet` script attempts is authorized against `policy.json` **per actual attempt** before any I/O. Authority comes only from that file; when it is absent, every application effect is denied and pure operations still run. A library host may add a **ceiling** and any caller may send a per-request **`restrict`**; both can only narrow. Values declared with `secret … for ORIGIN` are **tainted** and may reach only their bound network origins. Network serving authenticates every request and applies one principal map to every surface. This document states what that buys, what it does not, and where 0.1.0 has known gaps.

```text
                          ┌───────────── trusted ─────────────┐
   policy.json author ───▶│ policy.json  (grants, deny, network, limits, serve, approved)
   operator / host    ───▶│ rivet binary, OS, bootstrap files │
                          └────────────────┬──────────────────┘
                                           │ authority
   ┌──────────── semi-trusted ─────────────▼──────────────────────────────┐
   │ .rivet bundle author: may ask for any effect; gets only what policy grants │
   └──────────────────────────────────────┬───────────────────────────────┘
                                          │ EffectIntent per attempt
                                   ┌──────▼───────┐  denied ─▶ permission.denied (exit 3 / 403)
                                   │ PolicyBroker │  policy.json ∩ host ceiling ∩ restrict stack
                                   └──────┬───────┘  allowed (+ secret sink check in the interpreter)
   ┌──────────── untrusted ───────────────▼──────────────────────────────┐
   │ network callers of `rivet serve` · remote HTTP/gRPC/QUIC/MCP peers   │
   │ DNS answers · redirect targets · child processes · file contents     │
   └──────────────────────────────────────────────────────────────────────┘
```

## Threat model

| Actor | Can | Rivet's control |
|---|---|---|
| Network caller of `serve` | Send any request to any mounted route | `serve.auth` on every request; `serve.principals` per operation ID; loopback-only without auth; `Origin` check on `/mcp`; a caller's `restrict` can only remove authority |
| Bundle author (or a compromised bundle) | Write scripts that request arbitrary effects | Deny-by-default broker; grants narrowed by capability, target selector and access verb; deny entries win |
| Remote peer (HTTP, gRPC, QUIC, MCP server, OAuth server) | Return hostile data, redirect, stall, oversize | No redirects unless opted in (each hop re-authorized); size and time limits; MCP snapshot pinning and output-schema checks; errors bounded |
| DNS | Rebind a granted name to an internal address | Resolve once, check every resolved address against the private-range rule, dial the checked address |
| Child process | Anything its OS permissions allow | argv only, no shell, no PATH lookup, empty env; OS sandbox required when policy.json exists (macOS active; others refuse) |
| Local user with file access | Edit policy.json, the bundle, token stores | **Out of scope**: they are trusted inputs |

Out of scope for 0.1.0: a malicious local operator, side channels, resource exhaustion beyond the listed limits, compromise of the Rust toolchain or dependencies, and the confidentiality of data an allowed effect legitimately returns.

## What is guaranteed

```text
  G1  No policy.json  ⇒ every application effect is denied (pure operations run).
  G2  An effect runs only if a grant matches capability + target + access verb
      and no deny entry matches.
  G3  Authorization happens per attempt: each redirect hop, each resolved IP,
      each path (source and destination), each credential use.
  G4  Private, loopback, link-local, CGNAT, unspecified and metadata addresses are
      denied unless a grant names that IP literally — for every scheme
      (http, https, ws, wss, tcp, udp, quic, grpc).
  G5  File effects are confined under the bundle root and refuse symlinks
      (no-follow) and hard links.
  G6  Processes get argv only: a shell with -c is refused, bare names are refused,
      env starts empty.
  G7  `serve` refuses to listen beyond loopback without authentication,
      and every surface uses the same principal model.
  G8  OAuth tokens never appear in results, errors, traces or CLI output.
  G9  Traces record decisions only (capability, verb, redacted target, rule),
      never payloads; query strings, fragments and URL userinfo are stripped.
  G10 A host ceiling and per-request restrictions only narrow: an attempt must be
      allowed by policy.json AND the ceiling AND every restriction in force.
  G11 A `secret … for ORIGIN` value — and forms derived from it by explicit flows
      (assignment, interpolation, list/object construction, encodings) — reaches
      only a network sink whose scheme://host:port is a bound origin; returning,
      emitting, writing it to a file, a process, a Unix socket, a pipe or a nested
      request is refused (permission.denied, details.secret) before any I/O.
  G12 URL grants with a path match whole segments (/users/42 never covers /users/420).
```

## What is not guaranteed

- **Bootstrap I/O is outside the broker** (see the list below): the runtime reads its own inputs before any policy applies.
- **Secret taint covers explicit flows only.** Branching on a secret, its length or timing (implicit flows) is not tracked, and derived forms shorter than 4 bytes are not recorded. Treat an operation that reads a secret as able to leak a few bits through its control flow. Explicit leaks are refused (captured at `829ca43`):

  ```text
  $ cat secpol.json
  {"version":1,"grants":[{"capability":"allow_write","targets":["./out/**"],"access":["create"]},{"capability":"allow_env","targets":["RIVET_DEMO_TOKEN"]}]}
  $ RIVET_DEMO_TOKEN=s3cr3t-value rivet --file sec.rivet --policy secpol.json --json request s.leak     # return "Bearer ${token}"
  {"request_id":"req_01ba5acfbd","trace_id":"tr_01ba5acfbd","error":{"kind":"permission","code":"permission.denied","message":"secret `token` cannot be returned; secrets may only reach their bound origins","retryable":false,"effects":"none","operation_id":"s.leak","details":{"secret":"token"}}}
  $ … request s.emit --stream                                      # emit (base64.encode token)
  {…"code":"permission.denied","message":"secret `token` cannot be emitted; secrets may only reach their bound origins",…,"details":{"secret":"token"}}}
  $ … request s.file                                               # file create "./out/t.txt" text token
  {…"code":"permission.denied","message":"secret `token` may not reach `file`; secrets travel only to their bound network origins (https://api.example.com)",…,"details":{"secret":"token","origin":"`file`"}}}
  ```

- **Allowed effects are fully trusted in their content.** Rivet does not inspect what a granted HTTP endpoint returns or what a granted file contains, beyond size limits and declared output checks.
- **Argument injection** into a granted binary is not prevented: argv stops shell injection, not tool-specific flags.
- **Sandbox coverage is platform-dependent** (matrix below); on Linux (gated until verified on kernel ≥ 6.12) and Windows/other OSes (unsupported) 0.1.0 refuses to run processes when policy.json is present rather than run them unconfined.
- **No TLS on the listener**: bearer tokens cross the network in clear unless a TLS-terminating proxy is in front.
- **In-memory state only**: traces, sessions and `store memory` credentials vanish with the process; nothing is tamper-evident (no persistent trace store; `Runtime::export_trace` writes a copy through the broker on request).
- **`*` principals reach `rivet.auth.*`**: a `serve.principals` entry `"*"` matches the OAuth management built-ins; their effect is then governed only by `allow_auth` grants.

## Policy model

```text
  policy.json (schema v1, strict: unknown keys → policy.invalid, exit 2)
  {
    "version": 1,
    "grants":  [ {"capability": "allow_read",    "targets": ["./data/**"], "access": ["read","list"]} ],
    "deny":    [ {"capability": "allow_read",    "targets": ["./data/private/**"]} ],
    "network": { "deny_private_ranges": true },
    "limits":  { "max_concurrent_requests": 64, "max_call_depth": 16, "max_buffered_bytes": … },
    "serve":   { "surfaces": [...], "auth": {...}, "principals": {...} },
    "approved":{ "snapshots": ["sha256:…"], "overlaps": [...] }
  }
```

- **Discovery:** `policy.json` beside the entry `.rivet` file, or exactly the file named by `--policy PATH` (a path, never grant text). No environment variable is consulted. Relative file targets resolve against the policy file's directory.
- **Capabilities (13):** `allow_read`, `allow_write`, `allow_delete`, `allow_network`, `allow_exec`, `allow_env`, `allow_unix`, `allow_pipe`, `allow_listen`, `allow_mcp`, `allow_grpc`, `allow_auth`, `allow_credentials`.
- **Access verbs** narrow a grant (e.g. create-only writes); an absent `access` means every verb of that capability. A verb outside its capability is `policy.invalid`.
- **Decision order:** absent file → deny; any deny entry → deny; private literal without a literal grant → deny; first matching grant → allow; else deny.
- **Callers can only narrow.** No surface, parameter or environment variable adds authority. A library host's `.ceiling(Policy)` is evaluated first (rule `host ceiling: …`); a caller's `restrict {grants}` (HTTP, polling, WebSocket, MCP, library) is checked after the broker allows (rule `request restriction: …`) and applies to nested calls too; a malformed restriction is `policy.invalid` (`/restrict/…`).
- **Limits are exact.** A `limits` value wider than its field is rejected (`must be at most 4294967295`), never truncated; `max_buffered_bytes` bounds the bytes all session queues may hold (`limit.buffered_bytes`). A generated policy draft (`rivet policy generate`) is never an approval: it never contains `serve` auth or secrets and reports opaque sites for human review (exit 7).

## Bootstrap I/O

Reads the runtime performs to start, listed (not governed) by `rivet io --include-bootstrap` — captured from [demos/02-file-crud](../demos/02-file-crud/app.rivet):

```text
BOOTSTRAP (runtime-internal; listed, not governed by policy.json)
KIND  ACCESS       TARGET
file  read         ./app.rivet (+ imports)
file  read         ./policy.json
file  read         system CA bundle
file  read         /etc/resolv.conf / system resolver
file  read         tzdata
file  read         descriptor/schema files named by connectors (none here)
pipe  read, write  stdin, stdout, stderr
```

Also outside the broker: the `serve` listener bind, `--token-file` (remote CLI), and the platform keychain when an OAuth profile selects `store keychain` (access to it still requires `allow_auth` + `allow_credentials` grants).

## SSRF and DNS rebinding

```text
  http get "https://api.example.com/x"
     │
     ├─ authorize allow_network connect https://api.example.com:443/x      (origin grant)
     ├─ host is an IP literal?  ── yes ─▶ private & not literally granted? ─▶ denied
     │                                    (applies to opaque schemes too: INC-2026-0001)
     ├─ resolve once ─▶ [ip1, ip2 …]
     │     for each ip: private?  ── yes ─▶ authorize scheme://ip:port/ (needs literal grant)
     │                              no  ─▶ dial THIS ip (never re-resolve)
     └─ redirect?  only with `redirect follow limit N`; every hop repeats the whole check
```

Private ranges: RFC 1918, loopback, link-local (incl. `169.254.169.254`), CGNAT `100.64.0.0/10`, unspecified, broadcast, IPv6 `fc00::/7`, `fe80::/10`, loopback and IPv4-mapped private addresses. `localhost` is treated as the loopback literal. The same `checked_addr` path is used by HTTP/1.1/2, HTTP/3, WebSocket, TCP, gRPC, QUIC, UDP and OAuth token/device requests. Captured with a grant of `"*"`:

```text
$ cat policy.json
{"version":1,"grants":[{"capability":"allow_network","targets":["*"]}]}
$ rivet --file app.rivet request probe.metadata --params '{}'          # http get "http://169.254.169.254/latest/meta-data/"
{"request_id":"req_01c4030d3d","trace_id":"tr_01c4030d3d","error":{"kind":"permission","code":"permission.denied","message":"allow_network connect http://169.254.169.254:80/latest/meta-data/ denied: 169.254.169.254 is a private/loopback/link-local address; grant it literally (e.g. \"http://169.254.169.254:80/latest/meta-data/\") to allow it",…}}
exit 3
$ rivet --file app.rivet request probe.udp --params '{}'               # with udp "10.0.0.5:53"
{…"code":"permission.denied","message":"allow_network connect udp://10.0.0.5:53 denied: 10.0.0.5 is a private/loopback/link-local address; grant it literally (e.g. \"udp://10.0.0.5:53\") to allow it",…}
exit 3
```

## File confinement

File effects run through `files.apply_file_operation` (authorize every path the verb touches, source and destination) and then `ConfinedFiles` (cap-std): paths resolve under the bundle root, symlinks are not followed (`permission.denied`), hard-linked files are refused (`file.hardlink_refused`), OS permission errors are `permission.os`. An `update` with an expected version fails on mismatch (`conflict.version`) and `create` never overwrites (`conflict.already_exists`).

## Process sandbox matrix

A process effect always requires `allow_exec` on the exact binary path. **Whenever policy.json is present** the child must also run inside an OS sandbox built from the policy's read/write/delete/exec grants; if the grants cannot be represented or no backend exists, the spawn fails **before** any process starts.

| Platform | Backend | 0.1.0 status | Child restrictions |
|---|---|---|---|
| macOS | Seatbelt via `/usr/bin/sandbox-exec`, static deny-default SBPL profile; paths passed as parameters (no profile injection) | **Active** | reads: granted paths + fixed system list (`/usr`, `/bin`, `/System`, `/Library/Apple`, `/private/var/db/timezone`, `/dev`); writes: granted paths; **no network (incl. loopback)**; no fork; signals only inside the sandbox |
| Linux | Landlock ABI V6 (hard requirement) + seccomp deny-list, applied between fork and exec | **Built, gated** — refuses with `unsupported.sandbox_backend` until the conformance suite passes on kernel ≥ 6.12 | when enabled: Landlock fs/net/scope rules; seccomp denies AF_UNIX/INET/INET6/NETLINK/PACKET sockets, ptrace, bpf, io_uring, mount/unshare/setns, keyctl, module/kexec |
| Windows, others | none | **Refuses** every sandboxed spawn: `unsupported.sandbox_backend` | — |

```text
  command "/usr/bin/curl" …
     ├ shell with -c?            ─▶ unsupported.shell
     ├ bare name (PATH lookup)?  ─▶ validation.process_program
     ├ allow_exec on the path?   ─▶ else permission.denied
     ├ policy.json present?      ─▶ SandboxSpec from grants ─▶ backend available? ─▶ else unsupported.sandbox_backend
     └ spawn (env empty + explicit env {…}); deadline kills the process group; always reaped
```

Captured on macOS (policy grants `allow_exec` on `/usr/bin/printf`, `/usr/bin/curl`, `/bin/sh` only):

```text
proc.printf  args ["%s", "hello; echo this stays data"]
{"request_id":"req_01698869d5","trace_id":"tr_01698869d5","result":"hello; echo this stays data","data_count":0,"effects":"committed"}

proc.curl    args ["-sS","--max-time","2","http://127.0.0.1:9/"]     # exit 5
{…"kind":"process","code":"process.exit","message":"`/usr/bin/curl` exited with status 1",…,
  "details":{"exit":1,"stderr":"Auto configuration failed\n…Operation not permitted…fopen('/private/etc/ssl/openssl.cnf', 'rb')…"}}
  (the child could not even read a system file outside the sandbox read list)

proc.shell   command "/bin/sh" args ["-c","echo hi"]                  # exit 5
{…"kind":"unsupported","code":"unsupported.shell","message":"`/bin/sh` with -c evaluates a shell string; pass argv to the target binary instead",…}
```

## Secrets and taint

| Mechanism | 0.1.0 behaviour |
|---|---|
| `secret NAME from env "VAR" for "ORIGIN"` | Needs `allow_env read VAR` (unset → `not_found.env`, exit 4); origins required at compile time (`syntax.secret`); **tainted at run time**: every sink (URL, headers, body, query of any effect, `with` opens, socket/UDP sends, process argv/stdin/env, file writes, nested request / MCP / gRPC params, `return`, `emit`) is checked; only a bound network origin passes (explicit flows, including `base64.encode`/`text` outputs) |
| OAuth tokens, codes, state, PKCE verifiers, device codes | Held only in `SecretString` (formats as `[redacted]`); never returned by `rivet.auth.*`, never in traces |
| gRPC metadata | Sensitive keys shown as `[redacted]` in the script view |
| Trace targets | Query string, fragment and URL userinfo removed |
| Serve bearer tokens | Only SHA-256 hashes in policy.json; compared in constant time against every entry; raw token never logged |
| Errors | Messages never include secret values; OAuth errors expose safe codes only |

## OAuth token handling

```text
  rivet.auth.begin   ── allow_auth manage + allow_credentials ──▶ state + PKCE S256 verifier (secret)
        │                                                        returns only authorization_url / user_code
  rivet.auth.complete ── state, redirect_uri, issuer, TTL, single use checked ──▶ token_url via brokered HTTP
        │                                                        returns sanitized CredentialStatus
  transport "auth P account A" ── auth.acquire_credential:
        exact resource origin bound by the profile? (else auth.origin_not_bound)
        allow_auth use + allow_credentials ──▶ cached lease | one coordinated refresh (single flight)
        cache key: principal · profile + config hash · account · audience · {origins} · {scopes}
        a token without expiry is never reused (exchange again / refresh / login_required)
  store memory (process) | keychain "NS" (platform store via keyring-core); generation advances on disconnect
```

Flows: `client_credentials`, `authorization_code` + PKCE S256, `device_code` (a deadline returns `{"state":"pending"}` without consuming the transaction). Token and device endpoints are reached through the same brokered HTTP client, so `allow_network`, private-range and redirect rules apply to the authorization server. Refresh uncertainty invalidates the cache and requires re-authorization (`auth.refresh_uncertain`); `rivet.auth.disconnect` removes local tokens and reports `local_only: true` (no provider revocation is claimed). Rivet never opens a browser.

## MCP remote trust boundary

Rivet is both an MCP **server** ([API-2026-0003](../api/api-2026-0003-mcp-server-tools.md)) and an MCP **client** (connectors).

```text
  rivet connectors sync crm --output ./schemas/crm.json   (allow_mcp crm/discover + transport grants + allow_write)
        └▶ candidate snapshot, format "rivet.mcp.snapshot/1", exclusive create (never overwrites)
  human review ─▶ add its sha256 to policy.json approved.snapshots
  bundle load  ─▶ snapshot read (bootstrap) ─▶ hash approved? else mcp.snapshot_unapproved
  call crm.tools.search
        ├ bridge _meta: hops < 8, not already in chain (limit.mcp_hops / limit.mcp_recursion)
        ├ allow_mcp on the logical target, then transport grants (HTTP brokered | stdio child sandboxed)
        ├ call not pinned to the snapshot hash loaded at start ─▶ protocol.mcp_schema_changed
        ├ params checked against the snapshot's inputSchema ─▶ validation.mcp_params
        ├ result checked against the tool's outputSchema ─▶ protocol.mcp_output_schema
        ├ first use of the session: live tools/list ≠ snapshot ─▶ mcp.schema_drift (nothing sent)
        └ tool isError ─▶ application mcp.tool_failed; effects reported as "unknown"
          (a wrapping operation also reports "unknown", never "committed")
  HTTP transport 401 ─▶ lease invalidated, call fails http.status (no retry; the next call reacquires)
```

What the boundary gives: the tool surface a bundle can call, and the schemas its arguments and results are checked against, are exactly those of the reviewed, hash-approved snapshot (imports not in it fail at load with `mcp.unknown_tool` / `mcp.unknown_resource` / `mcp.unknown_prompt`); message size (8 MiB), pagination and hop counts are bounded. At the first use of each connector session Rivet also compares the live `tools/list` with the snapshot (name and `inputSchema` of every exposed tool): a changed server fails `mcp.schema_drift` before the call is sent, so a live schema is never trusted. `connectors sync` refuses an existing `--output` before contacting the server. What it does not give: the *content* a remote tool returns is untrusted data — Rivet does not sanitize it for downstream consumers (for example an LLM reading tool output), and a tool call's side effects on the remote system are unknown to Rivet (`effects: "unknown"`).

As a server, `/mcp` rejects cross-origin browser requests (`Origin` must be localhost or the same host), binds MCP sessions to the authenticating principal, and exposes only operations that principal may call.

## Serve authentication

```text
  --listen ADDR
    ├ auth none   + loopback     ─▶ principal local (may call everything)
    ├ auth none   + non-loopback ─▶ refuse to start: serve.auth_required (exit 2)
    ├ auth bearer                ─▶ sha256(token) ∈ tokens[] (constant time) ─▶ principal
    └ auth mtls                  ─▶ refuse to start: unsupported.serve_mtls (exit 5) in 0.1.0
  per request: serve.principals — exact | "*" | "prefix.*"
     sensitive IDs (rivet.io, rivet.policy.generate, rivet.trace.show, rivet.trace.export,
     rivet.connectors.sync) match only an exact entry for network principals (they reveal
     URLs/paths or write files); "*" DOES match rivet.auth.* (governed by allow_auth)
  every response: one access-log line (route pattern, principal, operation, status; never
     params, bodies, query strings or tokens) · GET /v1/health unauthenticated only on loopback
```

Hidden operations are indistinguishable from unknown ones on catalog routes and in `tools/list`. Deploy bearer auth behind a TLS-terminating proxy; rotate a token by replacing its hash and restarting (SIGTERM or SIGINT drains in-flight work and exits 0).

Related incident: [INC-2026-0005](../incidents/resolved/inc-2026-0005-url-grant-path-prefix-match.md) (URL grant paths matched by raw prefix, `/users/42` covering `/users/420`) was fixed in commit `2d581b8`; see G12.

## Example of a fixed defect: INC-2026-0001

[INC-2026-0001](../incidents/resolved/inc-2026-0001-private-range-bypass-opaque-url-hosts.md) (S2, resolved before any release): with a `"*"` network grant, `udp://10.0.0.5:53`, `quic://[::1]:4433` and `tcp://…` were **allowed**, because the URL parser keeps hosts of non-special schemes as opaque text and the private-range predicate only recognized parsed IP hosts.

```text
  before:  udp://10.0.0.5:53 ─▶ Host::Domain("10.0.0.5") ─▶ "a name" ─▶ grant "*" ─▶ ALLOWED  ✗
  after:   udp://10.0.0.5:53 ─▶ Host::Domain text parsed as IP ─▶ private ─▶ DENIED unless literal ✓
```

Fixed in commit `2c3d09b` with a regression test (`opaque_scheme_ip_literals_get_the_private_range_rule`) and conformance tests that assert denied UDP/QUIC targets send zero packets. The capture in [SSRF and DNS rebinding](#ssrf-and-dns-rebinding) shows the fixed behaviour. Lesson kept in this model: security predicates must not assume a parsed host type; every scheme an adapter uses is tested.

## Related Documents

- [Security index](index.md) · [ARCH-2026-0001](../architecture/arch-2026-0001-rivet-runtime-architecture.md) · [API-2026-0001](../api/api-2026-0001-http-rest-sse-polling.md) · [API-2026-0003](../api/api-2026-0003-mcp-server-tools.md) · [Error registry](../api/api-2026-0005-error-registry.md)
- [ADR-0003 process sandbox backends](../decisions/adr-0003-process-sandbox-backends.md) · [RES-2026-0003](../research/res-2026-0003-process-sandbox-backends.md)
- [INC-2026-0001](../incidents/resolved/inc-2026-0001-private-range-bypass-opaque-url-hosts.md) · [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) · [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial security model for 0.1.0, verified against commit f40d4aa. |
| 2 | 2026-09-28 | Claude | Fix batch through 829ca43: secret taint enforced on every sink (G11, captured refusals replace the leak capture), host ceiling and per-request restriction (G10), URL path segments (G12, INC-2026-0005), exact limit widths and buffered-bytes budget, OAuth cache key and no-expiry rule, MCP drift check / sync order / opaque effects / 401 behaviour, sensitive `rivet.trace.export`, `*` matching `rivet.auth.*`, access log and health. |
