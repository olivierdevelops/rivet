---
document_id: SYS-2026-0004
title: "Rivet surfaces and the serve listener"
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
components: [cli, library, serve, http, ws, poll, mcp]
affected_versions:
  from: "0.1.0"
  to: null
last_verified_version: "0.1.0-dev (commit 829ca43)"
next_review_date: 2026-10-28
review_cycle: on-release
confidentiality: internal
scope: The external access points of Rivet 0.1.0 (CLI, --endpoint remote client, Rust library, and the single `rivet serve` listener carrying REST, SSE, polling, WebSocket and MCP) plus the principal authentication and operation authorization they share.
reason: Every caller reaches the same dispatcher through one of these surfaces; operators need one current-state reference for routes, flags, exit codes, auth modes and the per-principal operation rules, verified against the built binary.
related_documents: [PROP-2026-0001, PLAN-2026-0001, SYS-2026-0001, SYS-2026-0002, SYS-2026-0003, SYS-2026-0006, SYS-2026-0007, SYS-2026-0008, SYS-2026-0009, ADR-0002]
supersedes: null
superseded_by: null
tags: [rivet, system, surfaces, cli, serve, http, sse, polling, websocket, mcp, library, auth]
---

# Rivet surfaces and the serve listener

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** cli, library, serve, http, ws, poll, mcp
> **Last Verified Version:** 0.1.0-dev (commit 829ca43)

## Summary

Rivet exposes one compiled catalog of operations through several **surfaces**. Every surface is a thin
adapter over the same dispatcher (`Runtime::dispatch_request` in `src/orchestrator/runtime.rs`), so an
operation behaves identically whether it is called from the command line, from Rust, or over the network.

```text
                        ┌────────────────────────────── callers ──────────────────────────────┐
                        │                                                                      │
   shell user        remote shell user          Rust host            HTTP / WS / MCP clients     MCP host (stdio)
       │                    │                       │                          │                      │
 rivet --file app.rivet  rivet --endpoint URL   rivet::Runtime        rivet serve --listen A:P   rivet serve --stdio
   request/list/…        [--token-file F] …     ::builder()…build()          │                      │
       │                    │  (HTTP client)        │             ┌────────────┴────────────┐         │
       │                    └──────────────────────────────────▶ │ one TCP listener (axum)  │         │
       │                                            │             │ REST SSE poll WS MCP     │         │
       │                                            │             └────────────┬────────────┘         │
       │                                            │     authenticate_principal (none|bearer|mtls)   │
       │                                            │     authorize_operation (serve.principals)      │
       ▼                                            ▼                          ▼                      ▼
  ┌──────────────────────────────────────────────────────────────────────────────────────────────────────┐
  │           Runtime  (src/orchestrator/runtime.rs) — one compiled program, one policy, one broker      │
  │  dispatch_request ─▶ depth 0: require_operation ─▶ rivet.* built-ins │ request_operation │ MCP import │
  │  sessions() ─▶ SessionHost (polling, WS, MCP streaming tools, rivet.sessions.*, library)              │
  └──────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

Key facts, all verified against the 0.1.0-dev build:

- **CLI** `rivet` (`src/io/cli/mod.rs`, `src/orchestrator/setup_cli.rs`) loads a bundle with `--file` and runs one
  command. The CLI principal is always `local`.
- **Remote CLI** `rivet --endpoint URL [--token-file PATH] …` (`src/orchestrator/remote_cli.rs`,
  `src/infra/remote_client.rs`) sends the same commands to a running `rivet serve` and reproduces the local output
  and exit code. `check`, `graph`, `policy` and `serve` are refused remotely.
- **Library** `rivet::Runtime` (`src/lib.rs`, `src/orchestrator/runtime.rs`, `src/orchestrator/setup_library.rs`).
- **`rivet serve --listen HOST:PORT`** binds one socket and mounts every enabled surface: REST, SSE, polling,
  WebSocket and MCP Streamable HTTP (`src/features/serve/start_serve.rs`, `src/infra/serve_listener.rs`,
  `src/orchestrator/setup_serve.rs`). `rivet serve --stdio` serves MCP over stdio only.
- **Auth** comes only from policy.json `serve.auth`: `none` (loopback only), `bearer` (SHA-256 token hashes), or
  `mtls` (parsed, but the listener refuses to start in 0.1.0).
- **Authorization** comes from policy.json `serve.principals`; an unlisted operation is `403 permission.denied`
  (exit 3 on the remote CLI).
- **Every request** may carry a W3C `traceparent` (its trace-id becomes the request's `trace_id`; responses echo a
  `traceparent`) and a narrow-only `restrict {grants}`; every response adds one JSON **access-log** line on
  stderr; `GET /v1/health` is always mounted; SIGINT and SIGTERM **drain** (cancel in-flight work within the 5 s
  grace, exit 0).

## Responsibilities

| Responsibility | Owner (source) |
|---|---|
| Parse CLI arguments, render tables | `src/io/cli/mod.rs` |
| Map each CLI command onto runtime calls; exit codes; Ctrl-C cancel; `--input-jsonl -` duplex feeder | `src/orchestrator/setup_cli.rs`, `src/orchestrator/remote_cli.rs` |
| `--endpoint` HTTP/SSE/WS client, ErrorEnvelope → RivetError decoding | `src/infra/remote_client.rs` |
| Library API (`Runtime`, `RuntimeBuilder`, sessions methods, `policy_from_json`) | `src/lib.rs`, `src/orchestrator/runtime.rs`, `src/orchestrator/setup_library.rs` |
| Refuse unsafe binds, bind once, mount enabled surfaces | `src/features/serve/start_serve.rs` |
| Axum listener adapter; unmounted-route fallback; WS outbox | `src/infra/serve_listener.rs` |
| Shared serve state, one authentication path, error responses, `ServeHandle`, access-log middleware, `/v1/health`, `traceparent` in/out, SIGINT/SIGTERM drain | `src/orchestrator/setup_serve.rs` |
| REST + SSE handlers | `src/orchestrator/setup_http.rs`, encoding in `src/io/http/mod.rs` |
| Polling routes (HTTP projection of sessions) | `src/orchestrator/setup_poll.rs`, `src/features/serve/project_polling.rs`, `src/io/http/poll.rs` |
| WebSocket `/v1/ws` (`rivet.v1`), ref multiplexing and per-ref pumps | `src/orchestrator/setup_ws.rs`, `src/features/serve/multiplex_ws.rs`, `src/io/ws/mod.rs` |
| MCP server (Streamable HTTP `/mcp`, stdio) | `src/orchestrator/setup_mcp.rs`, `src/io/mcp/mod.rs` |
| Principal authentication | `src/features/serve/authenticate_principal.rs` |
| Per-principal operation authorization | `src/features/serve/authorize_operation.rs` |
| Serve contracts (receipts, frames, poll routes, constants) | `src/domain/serve.rs` |
| Reserved `rivet.*` built-in operations | `src/orchestrator/builtins.rs` |

## Boundaries and Non-Responsibilities

- **Not the language or catalog.** Compilation, operation IDs and descriptors belong to
  [SYS-2026-0001](sys-2026-0001-compiler-and-catalog.md).
- **Not execution.** Scopes, deadlines, DAGs and cancellation internals belong to
  [SYS-2026-0002](../runtime/sys-2026-0002-execution-scopes-and-dag.md). Surfaces only build a `Request`
  (principal, deadline, IDs) and hand it over.
- **Not effect authorization.** Whether an operation may touch a file, host or process is decided by the policy
  broker ([SYS-2026-0003](sys-2026-0003-policy-broker-and-io-manifest.md)). `serve.principals` only decides
  *who may call which operation ID*; it never grants effects.
- **Not session semantics.** Sequence numbers, retention, queue limits and ownership rules live in the session
  driver ([SYS-2026-0007](../runtime/sys-2026-0007-sessions.md)); polling, WebSocket and MCP streaming tools are
  projections of it.
- **Not OAuth.** `rivet auth …` and `rivet.auth.*` are routed here but implemented in
  [SYS-2026-0006](../integrations/sys-2026-0006-oauth-and-credentials.md).
- **Not outbound MCP.** Rivet as an MCP *client* (connectors, `connectors sync`) is
  [SYS-2026-0009](../integrations/sys-2026-0009-mcp-client-connectors.md). This document covers Rivet as an MCP
  *server*.
- **No TLS termination.** The listener is plain TCP/HTTP. Put a TLS-terminating proxy in front for remote use.
- The `--endpoint` client is host bootstrap I/O. It is not brokered by policy.json; the *server* authorizes every
  call it makes.

## Architecture

### Surface topology

```text
 rivet --file app.rivet [--policy P] serve --listen 127.0.0.1:8080
   │
   │ setup_serve::start()                                         (src/orchestrator/setup_serve.rs)
   │   ServeState { runtime, serve: policy.serve, loopback, authenticator?, mcp_sessions }
   │   prebuild routers: http, sse, poll, ws, mcp
   ▼
 start_serve(ServeStartInput)                                     (src/features/serve/start_serve.rs)
   ├─ stdio?            ─▶ receipt {stdio:true, surfaces:[mcp]}  (no bind)
   ├─ parse --listen    ─▶ IP:PORT or localhost:PORT             else validation.usage (exit 2)
   ├─ auth none + non-loopback ─▶ serve.auth_required (exit 2)   before bind
   ├─ auth mtls         ─▶ unsupported.serve_mtls (exit 5)       before bind
   ├─ ServeListener.bind ─▶ AxumListener: tokio TcpListener      (src/infra/serve_listener.rs)
   └─ for s in [http, sse, poll, ws, mcp] ∩ serve.surfaces: ServeListener.mount(s) ─▶ Router.merge
                                                   │
                     fallback for anything unmounted: 404 not_found.route
                                                   ▼
 ┌──────────────────────────── one axum app on one socket ─────────────────────────────┐
 │ http : GET /v1/operations[/{id}[/outputs]]  GET /v1/io  POST /v1/policy/generate      │
 │        POST /v1/request (Accept ≠ text/event-stream)                                  │
 │ sse  : POST /v1/request (Accept: text/event-stream)                                   │
 │ poll : POST /v1/requests  GET …/{id}/events  POST …/{id}/input|finish_input|cancel    │
 │ ws   : GET /v1/ws  (Sec-WebSocket-Protocol: rivet.v1)                                  │
 │ mcp  : POST /mcp   GET /mcp (405)   DELETE /mcp                                        │
 │ base : GET /v1/health (always)   fallback 404 not_found.route                           │
 │ every router wrapped by access_log ─▶ one JSON line per request on stderr               │
 └─────────────────────────────────────────────────────────────────────────────────────────┘
```

The `sse` surface shares the `/v1/request` path with `http`. When `http` is enabled the http router owns the
route and the handler checks the `Accept` header and whether the chosen surface (`http` or `sse`) is enabled;
when only `sse` is enabled a router with just `/v1/request` is mounted (`setup_http::request_route`).

### Layering (VHCO buckets)

```text
  io/cli, io/http, io/ws, io/mcp        decode / encode only (no business logic)
        │
  orchestrator/setup_*                  axum handlers, wiring, ServeState
        │
  features/serve/*                      pure use cases: start_serve, authenticate_principal,
        │                               authorize_operation, project_polling, multiplex_ws
        │   ports: ServeListener, Authenticator, WsConnection, SessionDriver
  infra/serve_listener, infra/remote_client, infra/session_driver
        │
  domain/serve.rs                       ServeReceipt, AuthnInput, OperationAccess, WsFrame, PollRoute, constants
```

### Principal authentication and authorization flow

```text
   incoming HTTP request / WS upgrade / MCP POST|DELETE
                     │
                     ▼
   ServeState::authenticate(surface, headers, peer)
                     │   AuthnInput{surface, remote_addr, bind_is_loopback,
                     │              authorization header, client_cert_subject=None, serve.auth}
                     ▼
   authenticate_principal(input, host_authenticator?)
     ├─ library Authenticator supplied ──────────────▶ host decides (replaces serve.auth)
     ├─ none   ── loopback? ─ yes ─▶ Principal{local, none}
     │                      └ no  ─▶ 401 auth.required
     ├─ bearer ── "Authorization: Bearer T" (scheme case-insensitive)?
     │             no ─▶ 401 auth.required ("missing bearer token")
     │             sha256(T) hex == tokens[i].sha256 (constant-time, every entry checked)?
     │             no ─▶ 401 auth.invalid ; yes ─▶ Principal{tokens[i].principal, bearer}
     └─ mtls   ── (never reached: start_serve refuses mtls)
                     │  401 responses carry "WWW-Authenticate: Bearer"
                     ▼
   authorize_operation(OperationAccess{principal, operation_id, serve})
     ├─ principal local/none ────────────────────────▶ allow ("local")
     ├─ generic built-in (rivet.request, list, describe, outputs, sessions.*) ─▶ allow ("builtin")
     │     (the operation they target is authorized separately)
     ├─ serve.principals absent ─▶ allow, unless sensitive ID ─▶ deny
     └─ serve.principals present:
           no entry for principal ─▶ deny
           pattern == id                     ─▶ allow
           sensitive id and pattern != id    ─▶ no match
           pattern "*"                       ─▶ allow
           pattern "prefix.*" and id = prefix.<more> ─▶ allow
           nothing matched                   ─▶ deny ─▶ 403 permission.denied (exit 3)

   sensitive IDs (SENSITIVE_IDS): rivet.io, rivet.policy.generate, rivet.trace.show, rivet.connectors.sync
```

Where the decision is applied:

| Place | Effect of a denial |
|---|---|
| `Runtime::dispatch_request` at depth 0 (every surface and the library) | `403 permission.denied` with request/trace IDs |
| `GET /v1/operations`, `rivet.list`, MCP `tools/list`, `rivet.outputs --all` | Operation silently filtered out |
| `GET /v1/operations/{id}[/outputs]`, `rivet.describe`, `rivet.io ids`, `rivet.policy.generate ids` | `404 not_found.operation` (hidden looks like unknown) |
| MCP `tools/call NAME` for a non-built-in | JSON-RPC `-32602 Unknown tool: NAME` |
| `POST /v1/requests`, WS `request` frame, `rivet.request`, `rivet.sessions.open`, MCP streaming tool | `403 permission.denied` (WS: error frame) |

## Interfaces

### CLI commands

```text
 rivet [--file F] [--policy P] [--json] [--endpoint URL] [--token-file T] <COMMAND>

   request ID [--params JSON] [--stream] [--timeout D] [--input-jsonl - (needs --stream)]
   list [--outputs]                    describe [ID…]            outputs [ID | --all]
   check [--strict-docs]               io [ID…] [flags]          policy explain [ID] [--params JSON] | policy generate [ID…|--all] [--output P]
   graph ID [--all]                    serve [--listen HOST:PORT | --stdio]
   trace show REQ | trace export REQ --output PATH               connectors sync NAME --output PATH
   auth begin PROFILE --account A | complete (--params JSON | --params-file F) [--timeout D]
        | status PROFILE --account A | disconnect PROFILE --account A | cancel TXN
```

Verified `rivet --help`:

```text
$ rivet --help
Run described .rivet operations over the CLI, HTTP, WebSocket, MCP and the Rust library

Usage: rivet [OPTIONS] <COMMAND>

Commands:
  request     Invoke one operation
  list        List public operations
  describe    Describe operations: params, output, errors, source
  outputs     Show declared outputs, emits, receives and errors
  check       Compile and check the bundle without running anything
  io          Generate the I/O manifest (every I/O site, target and access verb)
  policy      Policy tools
  serve       Serve every surface (REST, SSE, polling, WebSocket, MCP) on one listener
  trace       Request traces recorded by this host
  connectors  Outbound MCP connectors
  auth        OAuth account management (rivet.auth.* built-ins); tokens are never printed
  help        Print this message or the help of the given subcommand(s)

Options:
      --file <FILE>              Entry .rivet file (policy.json beside it is discovered automatically)
      --policy <POLICY>          Use this policy file instead of the discovered policy.json (a path, never grants)
      --json                     Print JSON instead of tables
      --endpoint <ENDPOINT>      Send request/list/describe/outputs/io/trace/auth to a running `rivet serve` at this URL instead of loading a bundle (cannot be combined with --file)
      --token-file <TOKEN_FILE>  With --endpoint: read the server bearer token from this file (never argv or env)
  -h, --help                     Print help
  -V, --version                  Print version
```

Global flags:

| Flag | Meaning | Rules |
|---|---|---|
| `--file PATH` | Entry `.rivet` file; policy.json beside it is discovered | Required for every local command (`validation.usage`, exit 2, when missing) |
| `--policy PATH` | Use this policy file instead of the discovered one | A path only; never inline grants |
| `--json` | JSON instead of tables (list, describe, outputs, policy explain, io) | `request`, `trace`, `auth`, `connectors` always print JSON |
| `--endpoint URL` | Talk to a running `rivet serve` | `http://` or `https://`, no userinfo, query or fragment; excludes `--file`/`--policy` |
| `--token-file PATH` | Bearer token for `--endpoint` | Exactly one non-empty line; requires `--endpoint` |

Local examples (run in `docs/demos/01-catalog`; request/trace IDs vary per run):

```text
$ rivet --file app.rivet list --outputs
ID              NAME                OUTPUT      DESCRIPTION
demo.greet      Greet a person      text        Return a greeting for the supplied person.
demo.add        Add two integers    integer     Add two signed integers and return their sum.
demo.health     Check availability  object      Return a constant readiness response without I/O.
demo.countdown  Count down          object      Emit 3, 2, 1 as data items and then return a summary.

$ rivet --file app.rivet request demo.greet --params '{"person":"Ada"}'
{"request_id":"req_01c9d59aa5","trace_id":"tr_01c9d59aa5","result":"Hello, Ada!","data_count":0,"effects":"none"}

$ rivet --file app.rivet request demo.add --params '{}'; echo "rc=$?"
{"request_id":"req_01c8e4cac5","trace_id":"tr_01c8e4cac5","error":{"kind":"validation","code":"validation.required","message":"missing required parameter `a`","retryable":false,"effects":"none","operation_id":"demo.add","details":{"field":"a"}}}
rc=2

$ rivet --file app.rivet check
ok: 4 operations, 0 connectors, 0 auth profiles

$ rivet list; echo "rc=$?"
error[validation.usage]: --file PATH is required (the entry .rivet file)
rc=2

$ rivet --file app.rivet request demo.add --input-jsonl -; echo "rc=$?"
{"request_id":"","trace_id":"","error":{"kind":"validation","code":"validation.usage","message":"--input-jsonl - needs --stream (output is NDJSON envelopes)","retryable":false,"effects":"none"}}
rc=2
```

Output conventions: a successful `request` prints one Completion JSON line on stdout; with `--stream` each data
item is one NDJSON `{"type":"data",…}` envelope line followed by one `{"type":"result",…}` line. Failures print
an ErrorEnvelope on stderr (`request`, `serve`, `trace`, `auth`, `connectors` always as JSON; other commands as
`error[code]: message` unless `--json`).

`request --input-jsonl - --stream` (operations that declare `receives`) feeds stdin JSON Lines into the live
input while data drains to stdout: blank lines are skipped, EOF finishes input, a line that is not JSON or does
not match `receives` is `validation.input` (exit 2, line contents never echoed) and cancels the request. The
queue ahead of the consumer is 16 items (`INPUT_QUEUE`). An operation without `receives` is `validation.no_input`.

Ctrl-C during a local `request` cancels it through `Runtime::cancel`; the request ends with one `cancelled`
error (exit 130).

### Exit codes

Exit codes come from the error kind (`ErrorKind::exit_code` in `src/domain/errors.rs`) plus a few
command-specific codes:

| Exit | Meaning | Error kinds / source |
|---|---|---|
| 0 | Success | — |
| 2 | Syntax, validation, usage or configuration | `syntax`, `validation` (incl. `validation.usage`, `serve.auth_required`, `stream.required`) |
| 3 | Permission or authentication | `permission`, `auth`; also `io --check-policy` denied/partial, `io --check-files` not permitted/unreadable, `policy explain ID --params` denied |
| 4 | Not found or conflict | `not_found`, `conflict`; also `io --check-files` missing file, `policy generate --output` exists |
| 5 | Dependency / runtime failure | `connection`, `dns`, `tls`, `http`, `protocol`, `application`, `process`, `parse`, `limit`, `unsupported` (incl. `unsupported.serve_mtls`), `output_invalid`, `internal`, `cleanup`, `consumer_failed` |
| 6 | Timeout | `timeout` |
| 7 | Inspection incomplete | `io --strict` with dynamic/opaque sites; `policy generate` with review items |
| 130 | Cancelled | `cancelled` (Ctrl-C) |

### `--endpoint` remote client

```text
 rivet --endpoint http://127.0.0.1:8080 [--token-file F] COMMAND
   request ID                 ─▶ POST /v1/request {id, params, deadline_ms?}   (--timeout ≤ 10m, else exit 2)
   request ID --stream        ─▶ POST /v1/request   Accept: text/event-stream  ─▶ NDJSON envelopes
   request ID --input-jsonl - --stream
                              ─▶ GET /v1/ws (rivet.v1): request, input*, finish_input | cancel
   list [--outputs]           ─▶ GET /v1/operations        (--outputs: rivet.list {outputs:true})
   describe [ID…]             ─▶ GET /v1/operations, then GET /v1/operations/{id} per ID
   outputs ID | --all         ─▶ GET /v1/operations/{id}/outputs | rivet.outputs {all:true}
   io [flags]                 ─▶ GET /v1/io?…&format=F  (server returns the rendered report + exit code)
   trace show REQ             ─▶ rivet.trace.show {request_id}
   trace export REQ --output P ─▶ rivet.trace.export {request_id, path}   (writes on the server; 2a751ab)
   connectors sync N --output P ─▶ rivet.connectors.sync {name, output}
   auth …                     ─▶ rivet.auth.* via POST /v1/request
   check | graph | policy … | serve ─▶ refused: validation.usage (exit 2)

   2xx ─▶ Completion/JSON      non-2xx ─▶ ErrorEnvelope decoded back into the same RivetError (same exit code)
```

- `--timeout D` becomes `deadline_ms` in the body; the server caps it at 600000 ms (`MAX_REQUEST_DEADLINE_MS`).
- Responses larger than 32 MiB (body, SSE event or WS frame; the per-request collection budget) are refused
  (`limit.response_body`).
- A non-2xx body that is not an ErrorEnvelope maps by status: 400/422 validation, 401 auth, 403 permission,
  404 not_found, 409 conflict, 429 limit, 501 unsupported, 504 timeout, other connection.
- The token is sent as `Authorization: Bearer …` on every HTTP request and on the WebSocket upgrade.
- Ctrl-C drops the HTTP exchange; the server cancels an SSE request whose client disconnected; the CLI exits 130.

Verified against `rivet --file app.rivet serve --listen 127.0.0.1:18410` (IDs vary):

```text
$ rivet --endpoint http://127.0.0.1:18410 list
ID              NAME                DESCRIPTION
demo.greet      Greet a person      Return a greeting for the supplied person.
demo.add        Add two integers    Add two signed integers and return their sum.
demo.health     Check availability  Return a constant readiness response without I/O.
demo.countdown  Count down          Emit 3, 2, 1 as data items and then return a summary.

$ rivet --endpoint http://127.0.0.1:18410 request demo.add --params '{"a":2,"b":3}'
{"request_id":"req_1044058af2","trace_id":"tr_1044058af2","result":5,"data_count":0,"effects":"none"}

$ rivet --endpoint http://127.0.0.1:18410 request demo.countdown --stream
{"request_id":"req_11c5462157","trace_id":"tr_11c5462157","seq":1,"type":"data","data":3}
{"request_id":"req_11c5462157","trace_id":"tr_11c5462157","seq":2,"type":"data","data":2}
{"request_id":"req_11c5462157","trace_id":"tr_11c5462157","seq":3,"type":"data","data":1}
{"request_id":"req_11c5462157","trace_id":"tr_11c5462157","result":{"count":3},"data_count":3,"effects":"none","type":"result"}

$ rivet --endpoint http://127.0.0.1:18410 request demo.nope; echo "rc=$?"
{"request_id":"req_13c93913b9","trace_id":"tr_13c93913b9","error":{"kind":"not_found","code":"not_found.operation","message":"no operation `demo.nope`","retryable":false,"effects":"none","operation_id":"demo.nope"}}
rc=4

$ rivet --endpoint http://127.0.0.1:18410 check; echo "rc=$?"
error[validation.usage]: check, policy and serve work on a local bundle (--file); they are not available with --endpoint
rc=2

$ rivet --endpoint http://127.0.0.1:18410 --file app.rivet list; echo "rc=$?"
error[validation.usage]: --endpoint cannot be combined with --file or --policy: the server owns the bundle and its policy
rc=2

$ rivet --token-file /dev/null --file app.rivet list; echo "rc=$?"
error[validation.usage]: --token-file authenticates to a server and needs --endpoint URL
rc=2

$ rivet --endpoint ftp://x list; echo "rc=$?"
error[validation.endpoint]: --endpoint must be http:// or https://, not ftp://
rc=2
```

With a bearer server (`--policy policies/team.json`, token files hold `dev-token-ci` / `dev-token-ada`):

```text
$ rivet --endpoint http://127.0.0.1:18411 --token-file ci.token request demo.add --params '{"a":2,"b":3}'; echo "rc=$?"
{"request_id":"req_0407ce2924","trace_id":"tr_0407ce2924","error":{"kind":"permission","code":"permission.denied","message":"principal `ci` may not call `demo.add`","retryable":false,"effects":"none"}}
rc=3

$ rivet --endpoint http://127.0.0.1:18411 --token-file ci.token list
ID           NAME                DESCRIPTION
demo.health  Check availability  Return a constant readiness response without I/O.

$ rivet --endpoint http://127.0.0.1:18411 list; echo "rc=$?"
error[auth.required]: missing bearer token
rc=3

$ rivet --endpoint http://127.0.0.1:18411 --token-file ada.token request demo.greet --params '{"person":"Ada"}'
{"request_id":"req_0581bfc841","trace_id":"tr_0581bfc841","result":"Hello, Ada!","data_count":0,"effects":"none"}
```

### Library API

`src/lib.rs` re-exports `rivet::Runtime` and `rivet::RuntimeBuilder`.

```text
 Runtime::builder()
    .file("app.rivet")                    load entry from disk (policy.json beside it is discovered)
  | .source(path, text, root)             compile in-memory source (no discovery: deny-all policy unless given)
    .policy_file("policy.json")           explicit policy path           ┐ optional; last one wins
  | .policy(Policy)                       already-parsed policy          ┘ (Policy::from_file / Policy::from_json)
    .ceiling(Policy)                      host ceiling (intersection)
    .connector_discovery()                load for connectors sync (unreviewed MCP snapshots allowed)
    .build()?  ─▶ Runtime (Clone; shares one Inner)
```

| Method | Purpose |
|---|---|
| `request(id, params, sink)` | Invoke as principal `local`, default deadline 30 s |
| `new_request(id, params, principal)` + `dispatch_request(req, sink)` | Surfaces set principal, deadline and IDs themselves; depth-0 requests are authorized |
| `request_as(principal, id, params, sink)` | Request as an authenticated principal (used by serve handlers) |
| `request_restricted(id, params, restrict, sink)` | Request narrowed by `restrict {grants}` |
| `scope(\|scope\| …)` → `scope.stream(id, params)` / `scope.duplex(id, params)` | Owned stream/duplex handles, cancelled and joined when the body returns |
| `shutdown(drain)` | Cancel every request and session (the serve drain) |
| `cancel(request_id, principal)` | Cancel one of the caller's running top-level requests |
| `list()`, `describe(ids)`, `outputs(id, all)` | Catalog inspection |
| `io(&IoQuery)`, `generate_policy(&[ids])`, `generate_policy_draft(ids, all, output)` | I/O manifest and policy drafts |
| `trace(request_id)`, `trace_store()`, `export_trace(request_id, path)` | Recorded broker decisions; export to a new file |
| `graph(&GraphQuery)` | Static call graph |
| `open_session`, `send_input`, `finish_input`, `read_events`, `cancel_session` | Live sessions (`src/orchestrator/setup_library.rs`) |
| `sessions()`, `policy()`, `bundle()`, `program()`, `catalog_version()` | Accessors |
| `sync_connector(name, output)` | Programmatic `connectors sync` |

A library host serves the network surfaces with `rivet::orchestrator::setup_serve::start(runtime,
ServeOptions{listen, stdio, authenticator, access_log})`, which returns a `ServeHandle` (`receipt`, bound
`addr`, `shutdown()` = the drain). An `authenticator` (`Authenticator` port) replaces `serve.auth` for every
surface; `access_log` receives each access-log line instead of stderr.

```rust
let rt = rivet::Runtime::builder().file("app.rivet").build()?;
let c = rt.request("demo.add", rivet::domain::Value::from_json(&serde_json::json!({"a": 2, "b": 3})), None).await?;
```

### Serve route table

| Method | Path | Surface | Request | Success | Notable errors |
|---|---|---|---|---|---|
| POST | `/v1/request` | http | `{id, params, deadline_ms?, restrict?}` + optional `traceparent` | 200 Completion (+ `traceparent`) | 400 `validation.malformed_json`; 422 `validation.required` (no `id`); 422 `stream.required` (streaming op without SSE); 422 `policy.invalid` (`/restrict/…`); 401; 403; 404 |
| POST | `/v1/request` + `Accept: text/event-stream` | sse | same | 200 `text/event-stream`: `data`* then one `result` or `error` event | errors before the first event keep their HTTP status |
| GET | `/v1/operations` | http | — | 200 `{operations:[{id,name,description,streaming}], next_cursor:null}` filtered per principal | 401 |
| GET | `/v1/operations/{id}` | http | — | 200 descriptor (input/output/emits/receives JSON Schema, errors, delivery, source) | 404 (unknown or hidden) |
| GET | `/v1/operations/{id}/outputs` | http | — | 200 `{id, output, emits, receives, errors}` | 404 |
| GET | `/v1/io?by=&kind=&access=&ids=&all=&check_policy=&needs=&strict=&include_bootstrap=&trace=&format=&report=` | http | query → `rivet.io` | 200 bare IoManifest (`format=table\|markdown\|csv` or `report=true`: rendered IoReport) | 403 unless local or exact `rivet.io` listing; 422 `validation.check_files_remote` |
| POST | `/v1/policy/generate` | http | `{ids?, all?}` → `rivet.policy.generate` | 200 `{policy, review, complete}` (never writes files) | 403 unless local or exact listing |
| POST | `/v1/requests` | poll | `{id, params, deadline_ms?, restrict?}` + optional `traceparent` | 202 SessionReceipt + `events_url` (+ `traceparent`) | 403; 404; 429 `limit.sessions` |
| GET | `/v1/requests/{id}/events?after_seq=N&wait_ms=M&max_events=K` | poll | query | 200 SessionBatch `{session_id, events, last_seq, terminal}` | 404 `not_found.session` |
| POST | `/v1/requests/{id}/input` | poll | `{send_seq, data}` | 200 SessionAck | 404; 409; 422 |
| POST | `/v1/requests/{id}/finish_input` | poll | — | 200 SessionAck (`input_closed:true`) | 404 |
| POST | `/v1/requests/{id}/cancel` | poll | — | 200 CancelReceipt (`cancelled`, or the terminal state of a finished session) | 404 |
| GET | `/v1/ws` | ws | upgrade, subprotocol `rivet.v1` | 101, JSON text frames | 422 `validation.subprotocol`; 401 |
| POST | `/mcp` | mcp | one JSON-RPC 2.0 message | 200 JSON-RPC response (+ `MCP-Session-Id` on initialize); 202 for notifications | 403 bad Origin; 422 `mcp.session_required`, `mcp.protocol_version`; 404 `not_found.mcp_session`; 400 JSON-RPC parse error |
| GET | `/mcp` | mcp | — | 405, `Allow: POST, DELETE` | — |
| DELETE | `/mcp` | mcp | `MCP-Session-Id` | 204 | 404 `not_found.mcp_session` |
| GET | `/v1/health` | always mounted | — | 200 `{status:"ok", catalog_version}` | 401 on a non-loopback bind without valid credentials |
| any | anything else / disabled surface | — | — | — | 404 `not_found.route` |

All error bodies are the ErrorEnvelope `{request_id, trace_id, error:{kind, code, message, retryable, effects,
…}}` with the registry HTTP status (`ErrorKind::http_status`): validation 422 (malformed JSON 400), auth 401,
permission 403, not_found 404, conflict/cancelled 409, limit 429, unsupported 501, timeout 504, dependency
kinds 502, output_invalid/internal 500.

### REST request sequence

```text
 client                       /v1/request handler (setup_http)            Runtime
   │ POST {id,params}              │                                          │
   │──────────────────────────────▶│ wants_sse(Accept)? no → surface "http"   │
   │                               │ enabled("http")? else 404                │
   │                               │ authenticate ─▶ Principal (401 on fail)  │
   │                               │ parse_request_body (400/422)             │
   │                               │ streaming op? ─▶ 422 stream.required     │
   │                               │ new_request(id, params, principal)       │
   │                               │ deadline_ms (≤ 600000) if given          │
   │                               │ dispatch_request ───────────────────────▶│ require_operation (403)
   │                               │                                          │ rivet.* → builtins │ request_operation
   │◀────────── 200 Completion ────│◀─────────────────────────────────────────│
```

Verified (IDs vary):

```text
$ curl -s -X POST 127.0.0.1:18410/v1/request -H 'content-type: application/json' -d @requests/add.http.json
{"request_id":"req_011835d2bd","trace_id":"tr_011835d2bd","result":5,"data_count":0,"effects":"none"}

$ curl -s -w ' [%{http_code}]' -X POST 127.0.0.1:18410/v1/request -d @requests/countdown.http.json
{"request_id":"","trace_id":"","error":{"kind":"validation","code":"stream.required","message":"`demo.countdown` streams; use Accept: text/event-stream, POST /v1/requests, /v1/ws or rivet.sessions.open","retryable":false,"effects":"none"}} [422]

$ curl -s -w ' [%{http_code}]' -X POST 127.0.0.1:18410/v1/request -d '{nope'
{"request_id":"","trace_id":"","error":{"kind":"validation","code":"validation.malformed_json","message":"request body is not valid JSON: key must be a string at line 1 column 2","retryable":false,"effects":"none"}} [400]

$ curl -s -w ' [%{http_code}]' 127.0.0.1:18410/v1/nope
{"request_id":"","trace_id":"","error":{"kind":"not_found","code":"not_found.route","message":"no such route on this listener","retryable":false,"effects":"none"}} [404]

$ curl -s 127.0.0.1:18410/v1/operations
{"operations":[{"id":"demo.greet","name":"Greet a person","description":"Return a greeting for the supplied person.","streaming":false},{"id":"demo.add","name":"Add two integers","description":"Add two signed integers and return their sum.","streaming":false},{"id":"demo.health","name":"Check availability","description":"Return a constant readiness response without I/O.","streaming":false},{"id":"demo.countdown","name":"Count down","description":"Emit 3, 2, 1 as data items and then return a summary.","streaming":true}],"next_cursor":null}

$ curl -s 127.0.0.1:18410/v1/operations/demo.countdown/outputs
{"id":"demo.countdown","output":{"type":"object","properties":{"count":{"type":"integer","description":"Number of items emitted."}},"required":["count"],"additionalProperties":false,"description":"Summary returned after the last item."},"emits":{"type":"integer"},"receives":null,"errors":[]}
```

### SSE request sequence

```text
 client                         sse_response (setup_http)                 dispatcher task
   │ POST Accept: text/event-stream │                                          │
   │───────────────────────────────▶│ spawn dispatch_request(req, ChannelSink) ─▶│
   │                                │ mpsc(16) ◀── Data(ev) ─────────────────────│ emit
   │                                │ first msg = Done(Err)? ─▶ plain ErrorEnvelope with HTTP status
   │◀─ 200 text/event-stream ───────│                                          │
   │◀─ id: n / event: data ─────────│ ◀── Data(ev) ──────────────────────────────│
   │◀─ id: n+1 / event: result|error│ ◀── Done(result) ──────────────────────────│
   │  (client disconnect) ─────────▶│ body dropped ─▶ AbortOnDrop aborts the dispatcher task
```

Verified:

```text
$ curl -sN -X POST 127.0.0.1:18410/v1/request -H 'accept: text/event-stream' -d @requests/countdown.http.json
id: 1
event: data
data: {"request_id":"req_029710a41a","trace_id":"tr_029710a41a","seq":1,"type":"data","data":3}

id: 2
event: data
data: {"request_id":"req_029710a41a","trace_id":"tr_029710a41a","seq":2,"type":"data","data":2}

id: 3
event: data
data: {"request_id":"req_029710a41a","trace_id":"tr_029710a41a","seq":3,"type":"data","data":1}

id: 4
event: result
data: {"request_id":"req_029710a41a","trace_id":"tr_029710a41a","result":{"count":3},"data_count":3,"effects":"none","type":"result","seq":4}
```

### Polling sequence

```text
 client                          project_polling (features/serve)           SessionDriver
   │ POST /v1/requests {id,params}  │ authenticate ─ require_operation (403)     │
   │───────────────────────────────▶│ open(connection_owned=false) ─────────────▶│ session (principal-owned)
   │◀── 202 SessionReceipt+events_url│                                           │
   │ GET …/events?after_seq=0&wait_ms=2000                                        │
   │───────────────────────────────▶│ read(after_seq, wait clamp ≤5000 (default 1000), max_events ≤16)
   │◀── 200 SessionBatch {events,last_seq,terminal}                               │
   │ POST …/input {send_seq,data} │ …/finish_input │ …/cancel ──────────────────▶│ send / finish / cancel
   │  another principal's or unknown session ─▶ 404 not_found.session             │
```

Verified (IDs vary):

```text
$ curl -s -w ' [%{http_code}]' -X POST 127.0.0.1:18410/v1/requests -d @requests/countdown.http.json
{"session_id":"ses_01d79f976d","request_id":"req_0455fc13fc","trace_id":"tr_0455fc13fc","catalog_version":"sha256:67104f0e7faaeafee253db758a668a9b24aa4263677e9d5ea2451b32019f9730","input_schema":null,"emits_schema":{"type":"integer"},"next_send_seq":1,"expires_at":"2026-09-28T05:12:46Z","events_url":"/v1/requests/ses_01d79f976d/events"} [202]

$ curl -s "127.0.0.1:18410/v1/requests/ses_01d79f976d/events?after_seq=0&wait_ms=2000"
{"session_id":"ses_01d79f976d","events":[{"request_id":"req_0455fc13fc","trace_id":"tr_0455fc13fc","seq":1,"type":"data","data":3},{"request_id":"req_0455fc13fc","trace_id":"tr_0455fc13fc","seq":2,"type":"data","data":2},{"request_id":"req_0455fc13fc","trace_id":"tr_0455fc13fc","seq":3,"type":"data","data":1},{"request_id":"req_0455fc13fc","trace_id":"tr_0455fc13fc","result":{"count":3},"data_count":3,"effects":"none","type":"result","seq":4}],"last_seq":4,"terminal":true}

$ curl -s -w ' [%{http_code}]' -X POST 127.0.0.1:18410/v1/requests/ses_nope/cancel
{"request_id":"","trace_id":"","error":{"kind":"not_found","code":"not_found.session","message":"no session `ses_nope`","retryable":false,"effects":"none"}} [404]
```

A unary operation submitted to `/v1/requests` behaves the same way; its batch holds one terminal `result`
event (`"result":5,…,"seq":1`, `"terminal":true` in the verification run).

### WebSocket sequence

```text
 client                          setup_ws::connection                 multiplex_ws           SessionDriver
   │ GET /v1/ws  Sec-WebSocket-Protocol: rivet.v1                                                     │
   │──────────────────────────────▶ authenticate once (401) ; subprotocol offered? else 422           │
   │◀── 101 Switching Protocols, sec-websocket-protocol: rivet.v1                                     │
   │ {"type":"request","ref":"c1","id":…,"params":…} ─▶ parse_client_frame ─▶ ref in flight? 409 conflict.ref
   │                                                                  ≥ 8 refs? 429 limit.ws_refs
   │                                                                  require_operation (403)
   │                                                                  open(connection_owned=true) ─▶│
   │                                  spawn pump(ref): read(wait 5000) loop ◀─────────────────────────│
   │◀── {"type":"data","ref":"c1","request_id","trace_id","seq","data"} …                            │
   │◀── exactly one {"type":"result","ref":"c1","completion":{…}} | {"type":"error","ref":"c1",…}   │
   │ {"type":"input","ref","seq","data"} │ {"type":"finish_input","ref"} │ {"type":"cancel","ref"} ─▶ send/finish/cancel
   │   refused input/finish ─▶ that ref's terminal error frame with the SPECIFIC code (conflict.input_sequence,
   │                           validation.input, conflict.input_closed), sent first; then its session is cancelled
   │   per-ref outbound lanes of 16 frames (WsOutbox) merged into one writer: a slow ref stalls only itself
   │   request frames may carry restrict {grants}; traceparent on the upgrade sets every ref's trace
   │   unknown ref ─▶ error frame not_found.ref ; malformed frame ─▶ error frame validation.frame (socket stays open)
   │ close ─────────────────────────▶ cancel every in-flight ref, join pumps
```

Verified with a minimal raw WebSocket client sending `requests/ws-frames.jsonl` plus two bad frames (frame
arrival order interleaves across refs; IDs vary):

```text
HTTP/1.1 101 Switching Protocols
sec-websocket-protocol: rivet.v1
>> {"type":"request","ref":"c1","id":"demo.add","params":{"a":2,"b":3}}
>> {"type":"request","ref":"c2","id":"demo.countdown","params":{}}
>> {"type":"request","ref":"c3","id":"demo.add","params":{"b":3}}
>> {"type":"input","ref":"zz","seq":1,"data":1}
>> {"type":"bogus","ref":"c9"}
<< {"type":"error","ref":"c3","error":{"kind":"validation","code":"validation.required","message":"missing required parameter `a`","retryable":false,"effects":"none","operation_id":"demo.add","details":{"field":"a"}}}
<< {"type":"error","ref":"zz","error":{"kind":"not_found","code":"not_found.ref","message":"ref `zz` is not in flight","retryable":false,"effects":"none"}}
<< {"type":"data","ref":"c2","request_id":"req_0923c6d7fd","trace_id":"tr_0923c6d7fd","seq":1,"data":3}
<< {"type":"result","ref":"c1","completion":{"request_id":"req_08a0072228","trace_id":"tr_08a0072228","result":5,"data_count":0,"effects":"none"}}
<< {"type":"data","ref":"c2","request_id":"req_0923c6d7fd","trace_id":"tr_0923c6d7fd","seq":2,"data":2}
<< {"type":"data","ref":"c2","request_id":"req_0923c6d7fd","trace_id":"tr_0923c6d7fd","seq":3,"data":1}
<< {"type":"error","ref":"c9","error":{"kind":"validation","code":"validation.frame","message":"type must be request, input, finish_input or cancel","retryable":false,"effects":"none"}}
<< {"type":"result","ref":"c2","completion":{"request_id":"req_0923c6d7fd","trace_id":"tr_0923c6d7fd","result":{"count":3},"data_count":3,"effects":"none"}}
```

Upgrade without the subprotocol:

```text
HTTP/1.1 422 Unprocessable Entity
{"request_id":"","trace_id":"","error":{"kind":"validation","code":"validation.subprotocol","message":"the WebSocket client must offer subprotocol rivet.v1","retryable":false,"effects":"none"}}
```

### MCP server

Protocol revision `2025-11-25` (`MCP_PROTOCOL_VERSION`), Streamable HTTP at `/mcp` and newline-delimited
JSON-RPC over stdio. Methods: `initialize`, `ping`, `tools/list`, `tools/call`; anything else is `-32601`.
Batches are not supported (`-32600`).

```text
 client                                 post_mcp (setup_mcp)
   │ POST /mcp initialize ─────────────▶ Origin check (403) ─ authenticate (401) ─ MCP-Protocol-Version == 2025-11-25? (422)
   │◀─ 200 result + MCP-Session-Id ──── mcp_sessions[sid] = principal.name
   │ POST notifications/initialized ───▶ session header required (422) and owned by this principal (404)
   │◀─ 202 Accepted
   │ POST tools/list ──────────────────▶ direct tools for visible public operations + every built-in tool the
   │                                     principal may call (18 for local; sensitive ones only with an exact listing)
   │ POST tools/call {name, arguments, restrict?} ▶ built-in ID  ─▶ request_as(principal, name)
   │                                     unary op     ─▶ request_bridged ─▶ Completion
   │                                     streaming op ─▶ require_operation + open_session ─▶ SessionReceipt
   │                                     hidden/unknown ─▶ JSON-RPC -32602 "Unknown tool: NAME"
   │◀─ {content:[{type:text,text}], structuredContent, isError}   (failures: isError:true + ErrorEnvelope)
   │ DELETE /mcp (MCP-Session-Id) ─────▶ 204 ; again ─▶ 404 not_found.mcp_session
   │ GET /mcp ─────────────────────────▶ 405 Allow: POST, DELETE
```

- **Origin**: absent, or `localhost` / `127.0.0.1` / `::1`, or the same host as `Host`; anything else is 403.
- **Sessions**: an MCP session ID is bound to the principal name that initialized it; another principal presenting
  it gets 404 `not_found.mcp_session`.
- **Direct tools**: `name` = operation ID, `title` = operation name, `inputSchema` = params schema; unary tools
  advertise `outputSchema` = Completion whose `result` is the declared output; streaming tools advertise the
  SessionReceipt schema and `_meta: {"rivet/delivery":"session"}`.
- **Built-in tools listed** (`builtin_tools()` in `src/io/mcp/mod.rs`, filtered by `authorize_operation`):
  `rivet.request`, `rivet.list`, `rivet.describe`, `rivet.outputs`, `rivet.sessions.open|send|finish_input|read|cancel`,
  `rivet.io`, `rivet.policy.generate`, `rivet.trace.show`, `rivet.trace.export`, `rivet.capabilities`,
  `rivet.connectors.sync`, `rivet.auth.begin|complete|status|disconnect|cancel` — all 20 `BUILTIN_IDS` (since
  commit `2a751ab`).
- **`--stdio`**: principal `local`; stdout carries protocol messages only; the startup receipt goes to stderr;
  no MCP session header handling.

Verified (session IDs vary):

```text
$ curl -s -i 127.0.0.1:18410/mcp
HTTP/1.1 405 Method Not Allowed
allow: POST, DELETE
content-length: 0

$ curl -s -i -X POST 127.0.0.1:18410/mcp -H 'content-type: application/json' -d @requests/initialize.mcp.json
HTTP/1.1 200 OK
content-type: application/json
mcp-session-id: mcp_16b53335089e1368d
content-length: 146

{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"rivet","version":"0.1.0-dev"}}}

# with -H 'mcp-session-id: mcp_16b53335089e1368d' -H 'mcp-protocol-version: 2025-11-25'
notifications/initialized            -> [202]
tools/list names                     -> 13 ['demo.greet', 'demo.add', 'demo.health', 'demo.countdown', 'rivet.request', 'rivet.list', 'rivet.describe', 'rivet.outputs', 'rivet.sessions.open', 'rivet.sessions.send', 'rivet.sessions.finish_input', 'rivet.sessions.read', 'rivet.sessions.cancel']

$ … -d @requests/add.mcp.json
{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"{\"request_id\":\"req_062901d44e\",\"trace_id\":\"tr_062901d44e\",\"result\":5,\"data_count\":0,\"effects\":\"none\"}"}],"structuredContent":{"request_id":"req_062901d44e","trace_id":"tr_062901d44e","result":5,"data_count":0,"effects":"none"},"isError":false}}

$ … -d '{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"demo.nope","arguments":{}}}'
{"jsonrpc":"2.0","id":6,"error":{"code":-32602,"message":"Unknown tool: demo.nope"}}

$ curl -s -w ' [%{http_code}]' -X POST 127.0.0.1:18410/mcp -d @requests/list.mcp.json      # no session header
{"request_id":"","trace_id":"","error":{"kind":"validation","code":"mcp.session_required","message":"MCP-Session-Id header is required after initialize","retryable":false,"effects":"none"}} [422]

$ curl -s -w ' [%{http_code}]' -X POST 127.0.0.1:18410/mcp -H 'mcp-protocol-version: 2024-11-05' -d @requests/list.mcp.json
{"request_id":"","trace_id":"","error":{"kind":"validation","code":"mcp.protocol_version","message":"unsupported MCP-Protocol-Version 2024-11-05; this server speaks 2025-11-25","retryable":false,"effects":"none"}} [422]

$ curl -s -w ' [%{http_code}]' -X POST 127.0.0.1:18410/mcp -H 'origin: https://evil.example' -d @requests/initialize.mcp.json
{"request_id":"","trace_id":"","error":{"kind":"permission","code":"permission.denied","message":"Origin is not allowed for /mcp","retryable":false,"effects":"none"}} [403]

$ curl -s -w '[%{http_code}]' -X DELETE 127.0.0.1:18410/mcp -H 'mcp-session-id: mcp_16b53335089e1368d'
[204]
```

A streaming tool call (`demo.countdown`) returns a SessionReceipt in `structuredContent`
(`{"session_id":"ses_03ae43c357",…,"emits_schema":{"type":"integer"},"next_send_seq":1,…}`); the client then
uses `rivet.sessions.read`.

`--stdio`, verified by piping `initialize`, `notifications/initialized` and `tools/call demo.add` lines:

```text
$ (…three JSON-RPC lines…) | rivet --file app.rivet serve --stdio
{"listen_addr":null,"stdio":true,"surfaces":["mcp"],"auth_type":"none","catalog_version":"sha256:67104f0e7faaeafee253db758a668a9b24aa4263677e9d5ea2451b32019f9730","policy_hash":null}      <- stderr
{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"rivet","version":"0.1.0-dev"}}}
{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"{\"request_id\":\"req_0100cee3ad\",\"trace_id\":\"tr_0100cee3ad\",\"result\":5,\"data_count\":0,\"effects\":\"none\"}"}],"structuredContent":{"request_id":"req_0100cee3ad","trace_id":"tr_0100cee3ad","result":5,"data_count":0,"effects":"none"},"isError":false}}
```

### Built-in `rivet.*` operations

`BUILTIN_IDS` in `src/orchestrator/builtins.rs` (reachable from every surface through the dispatcher):

| ID | Params | Result | Authorization class |
|---|---|---|---|
| `rivet.capabilities` | `{}` | build facts: version, platform, stages A/B/C, features (with Stage C refusals), sandbox backend/status, serve surfaces/auth | open to every authenticated principal (in `GENERIC_BUILTINS`) |
| `rivet.request` | `{id, params?}` | Completion (unary) or SessionReceipt (streaming) | generic; target ID authorized |
| `rivet.list` | `{outputs?}` | `{operations, next_cursor:null}` filtered | generic |
| `rivet.describe` | `{id}` | descriptor | generic; hidden → 404 |
| `rivet.outputs` | `{id?, all?}` | one report or an array, filtered | generic |
| `rivet.sessions.open` | `{id, params?, deadline_ms?}` | SessionReceipt | generic; target ID authorized |
| `rivet.sessions.send` / `finish_input` / `read` / `cancel` | `{session_id, …}` | SessionAck / SessionBatch / CancelReceipt | generic; own sessions only |
| `rivet.io` | IoQuery fields; `format?` | IoManifest, or rendered IoReport when `format` given | **sensitive** |
| `rivet.policy.generate` | `{ids?, all?}` | `{policy, review, complete}` | **sensitive** |
| `rivet.trace.show` | `{request_id}` | trace | **sensitive** |
| `rivet.trace.export` | `{request_id, path}` (`output` alias) | `{request_id, path, events, bytes}` (new file, never overwrites) | **sensitive** |
| `rivet.connectors.sync` | `{name, output}` | snapshot receipt | **sensitive** |
| `rivet.auth.begin` / `complete` / `status` / `disconnect` / `cancel` | see SYS-2026-0006 | challenge / status / receipt | ordinary pattern matching |

Sensitive IDs (`SENSITIVE_IDS`: `rivet.io`, `rivet.policy.generate`, `rivet.trace.show`, `rivet.trace.export`,
`rivet.connectors.sync`) require the `local` principal or an **exact** entry in the principal's `operations`
list; `*` and `prefix.*` never match them. `*` **does** match `rivet.auth.*` (a known limitation; their use is
governed by `allow_auth` grants). `rivet.io` refuses `check_files` for every caller that reaches it through the
dispatcher (`validation.check_files_remote`); the CLI and library call `Runtime::io` directly.

## Configuration

All serve configuration is in policy.json (`serve` key; full schema in
[SYS-2026-0008](../configuration/sys-2026-0008-policy-json-reference.md)). There are no serve flags other than
`--listen` and `--stdio`.

```json
{
  "version": 1,
  "serve": {
    "surfaces": ["http", "sse", "poll", "ws", "mcp"],
    "auth": {"type": "bearer", "tokens": [{"principal": "ada", "sha256": "<64 hex chars: sha256 of the token>"}]},
    "principals": {"ada": {"operations": ["demo.*"]}, "ci": {"operations": ["demo.health", "rivet.io"]}}
  }
}
```

| Key | Default | Notes |
|---|---|---|
| `serve.surfaces` | all five | Subset of `http`, `sse`, `poll`, `ws`, `mcp`; unknown names fail policy load. A disabled surface answers 404. |
| `serve.auth.type` | `none` | `none` (keys: `type`), `bearer` (`type`, `tokens`), `mtls` (`type`, `client_ca`, `principals[{principal, subject}]`) |
| `serve.auth.tokens[].sha256` | — | Exactly 64 hex characters, compared lowercase |
| `serve.principals` | absent | Absent: every authenticated principal may call every public non-sensitive ID |
| `--listen` | `127.0.0.1:8080` | `IP:PORT`, `[v6]:PORT` or `localhost:PORT` (→ 127.0.0.1) |
| `--stdio` | off | MCP over stdio only; nothing is bound |

Fixed limits (code constants):

| Limit | Value | Source |
|---|---|---|
| Default request deadline | 30 s | `DEFAULT_DEADLINE_MS`, `src/domain/contracts.rs` |
| Max `deadline_ms` on `/v1/request` | 600000 ms | `MAX_REQUEST_DEADLINE_MS`, `src/io/http/mod.rs` |
| WS refs in flight per connection | 8 | `MAX_WS_REFS`, `src/domain/sessions.rs` |
| Poll `wait_ms` default / max | 1000 / 5000 ms | `SessionLimits`, `src/domain/sessions.rs` |
| Poll `max_events` cap | 16 | `SessionLimits` |
| SSE internal channel | 16 messages | `setup_http::sse_response` |
| WS outbound lane | 16 frames per ref (merged into one writer) | `WsOutbox`, `src/infra/serve_listener.rs` |
| Drain after SIGINT/SIGTERM | cancel, then up to 6 s | `DRAIN`, `src/orchestrator/setup_serve.rs` |
| `--input-jsonl` queue | 16 items | `INPUT_QUEUE`, `src/orchestrator/remote_cli.rs` |
| Remote client max body/event/frame | 32 MiB | `MAX_BODY`, `src/infra/remote_client.rs` |

## Runtime Behaviour

### Startup and shutdown

```text
 rivet serve
   │ load bundle + policy (policy errors exit 2 before anything starts)
   ▼
 start_serve ──refuse?──▶ ErrorEnvelope on stderr, exit 2 (serve.auth_required / validation.usage) or 5 (unsupported.serve_mtls)
   │ ok
   ▼
 stderr: ServeReceipt JSON {listen_addr, stdio, surfaces, auth_type, catalog_version, policy_hash}
   │
   ├─ network: serve until SIGINT or SIGTERM ─▶ drain: stop accepting, Runtime::shutdown cancels every
   │           request and session (sessions end cancelled.shutdown; handles close within 5 s),
   │           wait ≤ 6 s for responses, exit 0
   └─ stdio:   read stdin lines until EOF ─▶ exit 0
```

Verified startup receipts and refusals:

```text
$ rivet --file app.rivet serve --listen 127.0.0.1:18410          # no policy.json in 01-catalog
{"listen_addr":"127.0.0.1:18410","stdio":false,"surfaces":["http","sse","poll","ws","mcp"],"auth_type":"none","catalog_version":"sha256:67104f0e7faaeafee253db758a668a9b24aa4263677e9d5ea2451b32019f9730","policy_hash":null}

$ rivet --file app.rivet --policy policies/team.json serve --listen 127.0.0.1:18411
{"listen_addr":"127.0.0.1:18411","stdio":false,"surfaces":["http","sse","poll","mcp"],"auth_type":"bearer","catalog_version":"sha256:67104f0e7faaeafee253db758a668a9b24aa4263677e9d5ea2451b32019f9730","policy_hash":"sha256:461d2215049fbb93f51c91c0fcfe35e3247f2f0509c8d8a104b310599841c5a1"}

$ rivet --file app.rivet serve --listen 0.0.0.0:18412; echo "rc=$?"
{"request_id":"","trace_id":"","error":{"kind":"validation","code":"serve.auth_required","message":"non-loopback listener requires serve.auth in policy.json","retryable":false,"effects":"none"}}
rc=2

$ rivet --file mtls/app.rivet serve --listen 127.0.0.1:18413; echo "rc=$?"     # policy.json: serve.auth.type mtls
{"request_id":"","trace_id":"","error":{"kind":"unsupported","code":"unsupported.serve_mtls","message":"serve.auth type mtls needs a TLS listener, which this build does not provide yet; use bearer behind a TLS-terminating proxy","retryable":false,"effects":"none"}}
rc=5

$ rivet --file app.rivet serve --listen nope; echo "rc=$?"
{"request_id":"","trace_id":"","error":{"kind":"validation","code":"validation.usage","message":"--listen nope: use HOST:PORT with an IP address or localhost","retryable":false,"effects":"none"}}
rc=2
```

Drain on SIGTERM with a live polling session (commit `829ca43`, scratch bundle with the duplex `chat.echo`):

```text
$ rivet --file app.rivet serve --listen 127.0.0.1:18902 2> s2.err & P=$!
$ curl -s -X POST http://127.0.0.1:18902/v1/requests -d '{"id":"chat.echo","params":{}}' >/dev/null
$ kill -TERM $P; wait $P; echo "exit=$?"
exit=0
```

### Auth mode matrix

```text
                 bind loopback (127.0.0.0/8, ::1, localhost)     bind non-loopback (0.0.0.0, LAN IP, …)
               ┌──────────────────────────────────────────────┬─────────────────────────────────────────┐
 auth none     │ starts; every caller is principal `local`     │ refuses: serve.auth_required, exit 2    │
               │ (may call everything, incl. sensitive IDs)    │ (nothing bound)                         │
 auth bearer   │ starts; token → named principal; no token 401 │ starts; same (plain HTTP: use a TLS     │
               │                                               │ proxy in front)                          │
 auth mtls     │ refuses: unsupported.serve_mtls, exit 5       │ refuses: unsupported.serve_mtls, exit 5 │
               └──────────────────────────────────────────────┴─────────────────────────────────────────┘
```

### Operation authorization, verified

`--policy policies/team.json` maps token `dev-token-ada` → `ada` (`demo.*`) and `dev-token-ci` → `ci`
(`demo.health`), with `ws` disabled:

```text
$ curl -s -i 127.0.0.1:18411/v1/operations
HTTP/1.1 401 Unauthorized
www-authenticate: Bearer
{"request_id":"","trace_id":"","error":{"kind":"auth","code":"auth.required","message":"missing bearer token","retryable":false,"effects":"none"}}

$ curl -s -w ' [%{http_code}]' -H 'authorization: Bearer nope' 127.0.0.1:18411/v1/operations
{"request_id":"","trace_id":"","error":{"kind":"auth","code":"auth.invalid","message":"invalid bearer token","retryable":false,"effects":"none"}} [401]

$ curl -s -H 'authorization: Bearer dev-token-ci' 127.0.0.1:18411/v1/operations
{"operations":[{"id":"demo.health","name":"Check availability","description":"Return a constant readiness response without I/O.","streaming":false}],"next_cursor":null}

$ curl -s -w ' [%{http_code}]' -H 'authorization: Bearer dev-token-ci' -X POST 127.0.0.1:18411/v1/request -d @requests/add.http.json
{"request_id":"req_0192763ac5","trace_id":"tr_0192763ac5","error":{"kind":"permission","code":"permission.denied","message":"principal `ci` may not call `demo.add`","retryable":false,"effects":"none"}} [403]

$ curl -s -w ' [%{http_code}]' -H 'authorization: Bearer dev-token-ci' 127.0.0.1:18411/v1/operations/demo.add
{"request_id":"","trace_id":"","error":{"kind":"not_found","code":"not_found.operation","message":"no operation `demo.add`","retryable":false,"effects":"none"}} [404]

$ curl -s -w ' [%{http_code}]' -H 'authorization: Bearer dev-token-ci' -X POST 127.0.0.1:18411/v1/requests -d @requests/add.http.json
{"request_id":"","trace_id":"","error":{"kind":"permission","code":"permission.denied","message":"principal `ci` may not call `demo.add`","retryable":false,"effects":"none"}} [403]

$ curl -s -H 'authorization: Bearer dev-token-ada' -X POST 127.0.0.1:18411/v1/request -d @requests/add.http.json
{"request_id":"req_020a2f7eda","trace_id":"tr_020a2f7eda","result":5,"data_count":0,"effects":"none"}

$ curl -s -w ' [%{http_code}]' -H 'authorization: Bearer dev-token-ada' 127.0.0.1:18411/v1/io     # demo.* never matches rivet.io
{"request_id":"req_038ace7ff7","trace_id":"tr_038ace7ff7","error":{"kind":"permission","code":"permission.denied","message":"principal `ada` may not call `rivet.io`","retryable":false,"effects":"none"}} [403]

$ curl -s -w ' [%{http_code}]' -H 'authorization: Bearer dev-token-ada' 127.0.0.1:18411/v1/ws     # ws not in serve.surfaces
{"request_id":"","trace_id":"","error":{"kind":"not_found","code":"not_found.route","message":"no such route on this listener","retryable":false,"effects":"none"}} [404]

# MCP as ci (captured at f40d4aa): tools/list shows ['demo.health', 'rivet.request', 'rivet.list', 'rivet.describe', 'rivet.outputs',
#   'rivet.sessions.open', 'rivet.sessions.send', 'rivet.sessions.finish_input', 'rivet.sessions.read', 'rivet.sessions.cancel']
#   (since 829ca43 the built-ins ci may call are appended the same way; sensitive ones need an exact listing)
# tools/call demo.add  -> {"jsonrpc":"2.0","id":3,"error":{"code":-32602,"message":"Unknown tool: demo.add"}}
# ada presenting ci's MCP-Session-Id -> 404 not_found.mcp_session
```

### Lifecycles by surface

```text
 REST        request ────────────────────────────────▶ Completion | ErrorEnvelope            (one exchange)
 SSE         request ─▶ data* ─▶ result | error        client disconnect aborts the request
 polling     open(202) ─▶ events… ─▶ terminal batch   session is principal-owned; survives reconnects until it expires
 WebSocket   per ref: request ─▶ data* ─▶ result|error socket close cancels every open ref (connection-owned)
 MCP         initialize ─▶ tools/* … ─▶ DELETE          streaming tools return a receipt; events via rivet.sessions.read
 CLI         one command ─▶ exit code                  Ctrl-C ─▶ cancel ─▶ 130
```

## Data and Storage

The surfaces hold only in-memory, process-local state:

| State | Where | Lifetime |
|---|---|---|
| MCP session map (session ID → principal name) | `ServeState.mcp_sessions` | Until DELETE or process exit (no expiry in 0.1.0) |
| Host byte budget for session queues | `BufferBudget` (`limits.max_buffered_bytes`) | runtime |
| Open WS refs (ref → session ID) | per connection in `setup_ws::connection` | Until the ref's terminal frame or socket close |
| Live sessions (polling, WS, MCP streaming tools) | `SessionHost` via `Runtime::sessions()` | Session limits; see SYS-2026-0007 |
| Running top-level requests (for cancel) | runtime request registry | Until the request ends |
| Traces | in-memory trace store | Process lifetime |

Nothing is written to disk by the surfaces themselves. `POST /v1/policy/generate` and `rivet.policy.generate`
never write files; `rivet.connectors.sync` writes one new snapshot through the broker (allow_write).

Request and trace IDs have the form `req_NN…` / `tr_NN…`; session IDs `ses_…`; MCP session IDs `mcp_…`. They are
unique per process and are not persistent identifiers.

## Dependencies

| Dependency | Used for |
|---|---|
| `clap` | CLI parsing (`src/io/cli/mod.rs`) |
| `tokio` | Runtime, signals (Ctrl-C), channels, TCP |
| `axum` (on `hyper`) | Serve listener, routing, SSE bodies, WebSocket upgrade (ADR-0002) |
| `hyper`, `hyper-util`, `http-body-util` | `--endpoint` HTTP/1.1 client |
| `tokio-tungstenite` | `--endpoint` WebSocket client |
| `tokio-rustls` | `https://` / `wss://` for `--endpoint` |
| `sha2` | Bearer token hashing |
| `serde_json`, `url` | Bodies, query strings, endpoint parsing |

Internal dependencies: the dispatcher and registry (SYS-2026-0001/0002), the session driver (SYS-2026-0007),
the policy loader and `serve` schema (SYS-2026-0008), OAuth built-ins (SYS-2026-0006). Crate selection is
recorded in [ADR-0002](../../decisions/adr-0002-rust-crate-selection.md).

## Deployment

```text
  developer laptop                         shared host
  ─────────────────                        ─────────────────────────────────────────────
  rivet --file app.rivet serve             client ──TLS──▶ reverse proxy (TLS termination)
    --listen 127.0.0.1:8080                                   │ plain HTTP, Authorization header passed through
    (auth none, principal local)                              ▼
                                           rivet --file app.rivet serve --listen 10.0.0.5:8080
                                             policy.json: serve.auth bearer + serve.principals
```

- One process serves one bundle. There is no hot reload; restart to pick up a changed bundle or policy.
- `serve --stdio` is for MCP hosts that spawn Rivet as a child process; it cannot share the network socket.
- Bearer token hashes: `printf %s "$TOKEN" | shasum -a 256` (the stored value is the lowercase hex digest; the
  token itself never appears in policy.json).
- Pick `--listen HOST:0` in tests; the receipt's `listen_addr` reports the bound port.

## Security Boundaries

- **Loopback rule.** `auth none` never serves a non-loopback address: `start_serve` refuses before binding, and
  `authenticate_principal` would return 401 if the bound address were not loopback.
- **mTLS is not silently downgraded.** A policy with `serve.auth.type: "mtls"` refuses to start
  (`unsupported.serve_mtls`) instead of serving plaintext.
- **Tokens.** Only SHA-256 hashes are configured; comparison is constant-time over every entry; raw tokens are
  never logged (`AuthnInput`'s `Debug` redacts the header). `--token-file` keeps the token out of argv and the
  environment, and an unparsable file is reported without echoing its contents. 401 answers do not reveal which
  tokens exist.
- **One principal model.** The same authenticator runs on REST, SSE, polling, the WebSocket upgrade and every MCP
  HTTP request, and the same `authorize_operation` decision gates every depth-0 dispatch.
- **Hidden looks unknown.** Operations a principal may not see are filtered from listings and answer 404 on
  describe, so a denial never reveals whether a hidden ID exists. Private operations are never listed or callable
  externally (registry rule).
- **Sensitive built-ins.** Manifests, policy drafts, traces and snapshot writes reveal internal URLs/paths or
  write host files; network principals need an exact listing.
- **Session ownership.** Polling sessions, `rivet.sessions.*` and MCP session IDs are bound to the principal;
  another principal gets 404.
- **MCP Origin validation** blocks browser cross-origin requests to `/mcp` (DNS-rebinding defence).
- **No effect authority.** A principal allowed to call an operation still runs under policy.json grants; serve
  auth never widens effect permissions.
- **Remote CLI.** `--endpoint` refuses URLs with credentials, query or fragment; `https://` uses verified TLS.
- **Narrow-only callers.** `restrict {grants}` on any surface is intersected with policy.json; it can never add
  authority.
- **Access log hygiene.** The log records the matched route pattern, never query strings, params, bodies or
  tokens.
- **Health.** `/v1/health` is unauthenticated only on a loopback bind; it reveals the catalog version, nothing
  else.

## Observability

- **Startup receipt** (stderr JSON) states the bound address, mounted surfaces, auth type, catalog version and
  policy hash.
- **Every response carries `request_id` and `trace_id`** (Completion, ErrorEnvelope, SSE envelopes, WS data/error
  frames, polling events). Errors raised before a request exists (auth, routing, parsing) carry empty IDs.
- **Traces**: `rivet trace show REQ` / `rivet.trace.show` return the broker decisions for a request on this host
  (in memory; see SYS-2026-0003). A caller's W3C `traceparent` sets the `trace_id`, and responses carry
  `traceparent` back, so external traces can be joined.
- **Access log**: one JSON line per request on stderr (or the library `access_log` sink):
  `{time, surface, method, route, principal, operation, status, duration_ms}`.
- **Health**: `GET /v1/health` → `{"status":"ok","catalog_version":"sha256:…"}`. There is no metrics endpoint;
  the serve process writes nothing to stdout in network mode.

```text
{"time":"2026-09-28T09:47:49.689Z","surface":"http","method":"GET","route":"/v1/health","principal":null,"operation":"health","status":200,"duration_ms":1}
{"time":"2026-09-28T09:47:49.715Z","surface":"http","method":"POST","route":"/v1/request","principal":"local","operation":"demo.read","status":200,"duration_ms":8}
```

## Known Limitations

From the [manual's Known Limitations](../../manuals/man-2026-0001-rivet-manual.md#known-limitations):

- mTLS serve authentication is parsed but refused at startup (`unsupported.serve_mtls`, exit 5); the listener has
  no TLS.
- The `*` principal pattern matches `rivet.auth.*` (governed by `allow_auth`).
- `--timeout` is not applied on the WebSocket duplex path (`--endpoint … --input-jsonl - --stream`): a `request`
  frame carries no deadline.
- MCP server: no resources, resource templates or prompts, and no legacy HTTP+SSE transport.
- No persistent trace store; all serve state is in memory and lost on restart.

Other current behaviour (by design in 0.1.0): only `initialize`, `ping`, `tools/list`, `tools/call`; `GET /mcp` is
405; JSON-RPC batches are rejected; MCP session IDs do not expire; `--input-jsonl` accepts only `-`; `rivet.io`
`check_files` is CLI/library only; `GET /v1/operations` returns one page.

Drift found by TASK-092 at `829ca43` — `rivet.trace.export` had no dispatcher arm and neither it nor
`rivet.capabilities` had an MCP tool descriptor — was fixed in commit `2a751ab` (INC-2026-0007) and re-verified
(remote `trace export` writes the file; `tools/list` shows 20 built-ins).

## Last Verified Version

0.1.0-dev (commit 829ca43), `target/debug/rivet`, macOS, 2026-09-28. The fix-batch behaviour (health, access
log, SIGTERM drain, `traceparent`, `restrict` on HTTP/WS, bare `/v1/io`, polling cancel after finish, WebSocket
refusal frames, MCP `tools/list` built-ins, `rivet.capabilities`) was verified on `127.0.0.1:18901`–`18904` from
a scratch bundle; the `2a751ab` fixes (trace export dispatch, 20 listed built-ins) on `127.0.0.1:18908`–`18909`. First verified at `f40d4aa` from `docs/demos/01-catalog`
with `rivet serve --listen 127.0.0.1:18410` (no policy.json, auth none) and
`--policy policies/team.json serve --listen 127.0.0.1:18411` (bearer, `ws` disabled), plus refusal checks on
`0.0.0.0:18412` and a scratch mTLS policy, and `serve --stdio`. Both servers were stopped afterwards. Request,
trace, session and MCP session IDs differ on every run.

## Related Documents

- [PROP-2026-0001 Rivet runtime proposal](../../proposals/approved/prop-2026-0001-rivet-runtime.md)
- [PLAN-2026-0001 implementation and release plan](../../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md)
- [SYS-2026-0001 Compiler and catalog](sys-2026-0001-compiler-and-catalog.md)
- [SYS-2026-0002 Execution scopes and DAG](../runtime/sys-2026-0002-execution-scopes-and-dag.md)
- [SYS-2026-0003 Policy broker and I/O manifest](sys-2026-0003-policy-broker-and-io-manifest.md)
- [SYS-2026-0006 OAuth and credentials](../integrations/sys-2026-0006-oauth-and-credentials.md)
- [SYS-2026-0007 Sessions](../runtime/sys-2026-0007-sessions.md)
- [SYS-2026-0008 policy.json reference](../configuration/sys-2026-0008-policy-json-reference.md)
- [SYS-2026-0009 MCP client connectors](../integrations/sys-2026-0009-mcp-client-connectors.md)
- [ADR-0002 Rust crate selection](../../decisions/adr-0002-rust-crate-selection.md)
- [REF-2026-0002 Language and usage](../../references/ref-2026-0002-language-and-usage.md)
- [Demos](../../demos/README.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial current-state document (PLAN-2026-0001 D-18). |
| 2 | 2026-09-28 | Claude | TASK-092 drift fix for the fix batch (829ca43): `/v1/health`, access log, SIGTERM drain, `traceparent`, `restrict` on every surface, bare `/v1/io`, polling `deadline_ms` and cancel-after-finish, WS per-ref lanes and specific refusal frames, MCP `tools/list` built-ins, `rivet.capabilities`, `rivet.trace.export` (dispatched since 2a751ab), library scope/ceiling/shutdown/access_log, `graph` and `trace export` CLI, 32 MiB remote client budget; limitations reduced to the current ones plus the recorded drift. |
