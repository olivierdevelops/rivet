---
document_id: MAN-2026-0008
title: "Rivet protocols and connectors"
document_type: manual
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [transports, http, datagrams, quic, grpc, connectors, auth, mcp, policy, files]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, server, embedded]
audience: [integrators, developers, operators]
scope: When and how to use each 0.1.0 transport and connector — HTTP/1.1 and HTTP/2, response streams, HTTP/3, WebSocket client, TCP and Unix sockets, processes and the OS sandbox, UDP, QUIC, gRPC, OAuth 2.0 and MCP client connectors — with the grants each needs, errors and recovery.
reason: PLAN-2026-0001 row D-41 — integrator guide for the implemented protocol adapters; examples run against local fixtures (HTTP, TCP, Unix, UDP, OAuth, MCP) or verified up to the policy/DNS boundary where no fixture server was available (WebSocket, QUIC, HTTP/3, gRPC).
related_documents: [MAN-2026-0001, MAN-2026-0003, MAN-2026-0005, MAN-2026-0006, SYS-2026-0006, ADR-0003, DEMO-2026-0003, DEMO-2026-0004, DEMO-2026-0006, DEMO-2026-0007, DEMO-2026-0008, DEMO-2026-0009, DEMO-2026-0010, DEMO-2026-0011]
supersedes: null
superseded_by: null
tags: [rivet, manual, http, http3, websocket, tcp, udp, quic, grpc, oauth, mcp, sandbox]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.1.0-dev (commit f40d4aa)"
---

# Rivet protocols and connectors

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** transports, http, datagrams, quic, grpc, connectors, auth, mcp, policy, files

## Purpose

Pick the right transport, write it, grant it, and recover when it fails. Part of the
[Rivet manual](man-2026-0001-rivet-manual.md). Language basics are in
[MAN-2026-0003](man-2026-0003-language-guide.md); grant syntax in
[MAN-2026-0005](man-2026-0005-policy-and-io-manifest-guide.md).

## Reading Order

```text
 choose ─► HTTP/1.1+2 ─► response streams ─► HTTP/3 ─► WebSocket ─► TCP/Unix ─► processes + sandbox
        ─► UDP ─► QUIC ─► gRPC ─► OAuth 2.0 ─► MCP connectors ─► errors
```

## Concepts

### Which transport?

```text
 need request/response to a web API ────────────────► http METHOD URL            (allow_network https://h:443)
   …and item-by-item output (SSE, NDJSON, logs) ────► with http … stream sse|jsonl|lines|bytes
   …over QUIC, never falling back ──────────────────► version 3                  (same grant)
 need a persistent bidirectional channel ───────────► with websocket "wss://…"   (allow_network wss://h:443)
 line/length-framed service on a port ──────────────► with tcp "host:port"       (allow_network tcp://h:p)
 local daemon socket ───────────────────────────────► with unix "/path.sock"     (allow_unix /path.sock)
 a local tool ──────────────────────────────────────► command "/abs/bin"         (allow_exec /abs/bin)
 datagrams (telemetry, discovery) ──────────────────► with udp / udp bind / udp multicast
 multiplexed streams with ALPN ─────────────────────► with quic "quic://h:p"     (allow_network quic://h:p)
 typed RPC from a .proto ───────────────────────────► connector X grpc + grpc X.Method  (allow_network + allow_grpc)
 a token from an OAuth provider ────────────────────► auth P oauth2 + `auth P account "A"` on http/grpc/connector
 tools of an MCP server ────────────────────────────► connector X mcp + (request "X.tools.NAME" {…})
```

Common rules for every transport:

- Every connection is authorized **before** it is opened; denial is `permission.denied` (exit 3) with
  `effects: "none"`.
- Private, loopback and link-local addresses are denied unless granted literally (MAN-2026-0005).
- `with` blocks close their handles in reverse order within 5 s on any exit.
- No connection pooling in 0.1.0: every request opens its own connections.
- Mutations are never replayed automatically; `retry` applies only where you declare it.

## Task-Oriented Workflows

Local fixture examples below ran against small Python servers on `127.0.0.1` with `rivet 0.1.0-dev` (commit
`f40d4aa`); IDs vary per run.

### HTTP/1.1 and HTTP/2

Why: call REST/JSON APIs with explicit method, headers, body, decoding, accepted statuses, retries and redirects.

```rivet
operation users.get
    name "Get a user"
    description "GET with a declared 404."
    param id integer required min 1 description "User ID."
    output object description "The user."
        field id integer required description "ID."
        field name text required description "Name."
    end
    error "users.not_found" description "404 from the service."
    response = http get "http://127.0.0.1:18480/users/${id}"
        retry 2 on status [429, 503] backoff exponential base "50ms" max "200ms" jitter true
        accept status [200, 404]
        decode json
        timeout "5s"
    end
    if response.status == 404
        fail "users.not_found" {id: id}
    end
    return response.body
end
```

Grant (`policy.json`): `{"capability": "allow_network", "targets": ["http://127.0.0.1:18480"]}` (a loopback
target must be granted literally).

| Call | Result | Exit |
|---|---|---|
| `{"id":42}` | `{"id":42,"name":"Ada"}` | 0 |
| `{"id":7}` (404 accepted, then `fail`) | `application` `users.not_found` "404 from the service." | 5 |
| same request without `accept status` | `http` `http.status` "GET http://127.0.0.1/users/7 returned 404", `details.status: 404` | 5 |
| 503 after `retry 2 on status [503]` | `http.status` with `details.status: 503` | 5 |
| no grant | `permission.denied` "allow_network connect http://127.0.0.1:18480/users/42 denied …" | 3 |

Options (all lead the block): `query K V` (URL-encoded: `a b&c` → `q=a+b%26c`), `header K V`,
`body json|text|xml|form|bytes|file V` or `body multipart … end`, `decode json|text|bytes|xml|form`,
`accept status [...]`, `timeout "D"`, `retry N on status [...] backoff … base "D" max "D" jitter B`,
`redirect follow limit N`, `version …`, `auth PROFILE account "A"`, `unix "PATH"`,
`tls server_name|ca_file|cert_file|key_file V`.

The response is `{status, headers, body, version}` (`version` was `1.0` against the HTTP/1.0 Python fixture; `2` or `3` for those protocols).

Redirects are **not** followed unless asked:

```text
http get ".../old"  accept status [200, 302]                → 302
http get ".../old"  redirect follow limit 2  decode json    → {"id":42,"name":"Ada"}
```

Secrets in headers: see MAN-2026-0003 (`secret … for "ORIGIN"`); a POST with a secret header returned `201`
with `effects: "committed"`.

`tls ca_file`, `cert_file`, `key_file` and `body file` are file reads listed by `rivet io` (`allow_read`,
phase `before_connect`) and must be granted.

### HTTP response streams

```rivet
operation chat.reply
    name "SSE"
    description "Stream SSE deltas."
    output text description "Full text."
    emits text description "Each delta."
    text = ""
    with http get "http://127.0.0.1:18480/chat" as events
        stream sse
        for event in events
            emit event.data.delta
            text += event.data.delta
        end
    end
    return text
end
```

```text
$ rivet request --file app.rivet chat.reply --stream
{"request_id":"req_01a2bcbd6d","trace_id":"tr_01a2bcbd6d","seq":1,"type":"data","data":"Hel"}
{"request_id":"req_01a2bcbd6d","trace_id":"tr_01a2bcbd6d","seq":2,"type":"data","data":"lo"}
{"request_id":"req_01a2bcbd6d","trace_id":"tr_01a2bcbd6d","result":"Hello","data_count":2,"effects":"none","type":"result"}
```

| `stream` | Item |
|---|---|
| `sse` | one SSE event; `event.data` is the decoded JSON payload (verified: `event.data.delta`) |
| `jsonl` | one JSON value per line |
| `lines` | one text line (verified: `one`, `two`, `three` → result 3) |
| `bytes` | raw chunks |

### HTTP/3

| Option | Behaviour | Failure |
|---|---|---|
| `version 3` | HTTP/3 only; never falls back | `protocol` `http.version_unavailable` (exit 5), e.g. "HTTP/3 is not available: HTTP/3 needs an https:// URL", `details.request_sent: false` |
| `version prefer [3, 2]` | try HTTP/3; fall back to HTTP/2 **only before any request bytes were sent** | 3 must be first: `version prefer [2, 3]` → `validation.http_version` (exit 2, at run time) |
| (none) | HTTP/1.1 or HTTP/2 | — |

No Alt-Svc discovery. The HTTPS grant (`https://host:443`) covers the same-origin QUIC/UDP of HTTP/3; it does
not grant raw UDP. `version prefer [3, 2]` on an `http://` URL skips HTTP/3 and speaks HTTP/2 cleartext, which
HTTP/1-only servers reject (`connection.http` "HTTP exchange failed: http2 error"). Demo:
[09-quic](../demos/09-quic/README.md) (`items.http3`; no public HTTP/3 fixture is contacted — without a grant it
is `permission.denied`).

### WebSocket client

```rivet
with websocket "wss://api.example.com/realtime" as socket
    timeout "20s"
    socket.send json {type: "ping"}
    event = socket.receive json
    return event
end
```

Grant `{"capability": "allow_network", "targets": ["wss://api.example.com:443"]}`. Messages keep their
boundaries; `socket.send json|text|bytes V`, `socket.receive json|text|bytes [timeout "D"]`. Use `concurrent`
with one sender and one receiver task for duplex traffic. Verified in [04-streaming](../demos/04-streaming/README.md)
up to the network boundary: without a grant `permission.denied`; with `policies/websocket.json` the fixture
host does not resolve (`dns.resolve`, exit 5). `rivet io` shows `wss://api.example.com:443  connect GET (ws)`.

### TCP and Unix sockets

```rivet
operation tcp.echo
    name "TCP echo"
    description "Newline-framed JSON over TCP."
    output json description "Echo reply."
    with tcp "127.0.0.1:18493" as conn
        framing newline
        timeout "5s"
        conn.send json {hello: "tcp"}
        reply = conn.receive json
        return reply
    end
end
```

Grant `{"capability": "allow_network", "targets": ["tcp://127.0.0.1:18493"]}` →
`{"result":{"echo":{"hello":"tcp"}},…,"effects":"committed"}`.

Unix sockets use `allow_unix` with the socket path (`allow_read` does not cover sockets):

```text
with unix "/tmp/rivet-doc.sock" as conn … framing newline …
  no grant  → permission.denied "allow_unix connect /tmp/rivet-doc.sock denied: no policy.json: allow_unix is denied by default"  [3]
  granted   → {"ok":true}                                                                                               [0]
```

Framings: `newline`, `length32 endian big|little [max_frame N]`, `delimiter "S"`, `raw` (reads yield chunks).
`tls true` / `tls server_name|ca_file|cert_file|key_file V` for TLS over TCP. HTTP over a Unix socket:
`http get "http://localhost/info"` with `unix "/var/run/service.sock"` (needs `allow_unix`).

### Processes and the sandbox

```rivet
operation proc.cat
    name "Cat a file"
    description "Run /bin/cat in the OS sandbox."
    param path text required description "Path for cat."
    output json description "Process result."
    result = command "/bin/cat"
        args [path]
        timeout "5s"
    end
    return result.stdout
end
```

```json
{
  "version": 1,
  "grants": [
    {"capability": "allow_exec", "targets": ["/bin/cat"]},
    {"capability": "allow_read", "targets": ["./data/**"]}
  ]
}
```

```text
 command "/bin/cat" ─► argv only? absolute or ./path? ─► allow_exec grant? ─► policy.json present?
                           │ no: unsupported.shell /          │ no: denied        │ yes: build OS sandbox from
                           │ validation.process_program       │                   │ read/write/exec grants
                           ▼                                                      ▼
                         refuse (never spawned)                  spawn confined child ─► {stdout, stderr, exit, duration_ms}
```

| Call | Result | Exit |
|---|---|---|
| `{"path":"./data/pub.txt"}` | `"public\n"`, `effects: "committed"` | 0 |
| `{"path":"./secret.txt"}` (outside the read grant) | `process.exit` "`/bin/cat` exited with status 1", `details.stderr: "cat: ./secret.txt: Operation not permitted"` | 5 |
| `command "echo"` | `validation.process_program` "`echo` is not a path; use an absolute path or a bundle-relative ./path (no PATH lookup)" | 2 |
| `command "/bin/sh"` with `args ["-c", …]` | `unsupported.shell` "pass argv to the target binary instead" | 5 |
| read grant with `"access": ["read"]` | `unsupported.sandbox_backend` "the process sandbox cannot represent this policy exactly: allow_read ./data/** narrowed by access" | 5 |

Options: `args [...]`, `stdin json|text|bytes V`, `env {…}` (the child environment starts empty), `cwd`,
`timeout "D"`, `decode stdout|stderr T` (e.g. `decode stdout json`), `accept exit [...]`, and for
`with command … as p`: `stream stdout lines` then `for line in p.stdout`. Verified: stdin JSON round trip and a
two-line stdout stream.

Sandbox backends ([ADR-0003](../decisions/adr-0003-process-sandbox-backends.md)):

| Platform | Backend | 0.1.0 state |
|---|---|---|
| macOS | Seatbelt | active: child confined to the policy's read/write/exec grants; no network; no fork for children |
| Linux | Landlock + seccomp | built but gated: refuses (`unsupported.sandbox_backend`) until verified on kernel ≥ 6.12 |
| other | none | `unsupported.sandbox_backend` |

With **no** `policy.json`, processes are denied anyway (no `allow_exec`). Grants that the sandbox cannot express
exactly (`access` lists, globs other than `DIR/**`) fail before spawning rather than being widened.

### UDP

```rivet
operation telemetry.status
    name "Read UDP status"
    description "Send one status datagram and wait for a bounded reply."
    output object description "Status reported by the UDP peer."
        field state text required description "Peer state; the fixture answers \"ready\"."
    end
    with udp "127.0.0.1:7000" as socket
        max_datagram 8192
        socket.send json {command: "status"}
        return socket.receive json timeout "1s"
    end
end
```

| Form | Grants | Verified |
|---|---|---|
| `with udp "h:p"` | `allow_network udp://h:p` | with a local responder: `{"state":"ready"}`; without: `udp.receive_failed` "Connection refused" with `effects: "committed"` (exit 5) |
| `with udp bind "h:p"` + `receive_from` / `send_to peer` | `allow_listen udp://h:p` (bind) + `allow_network` for each reply peer | no bind grant → `permission.denied` "allow_listen bind udp://127.0.0.1:7001 denied …"; the reply target is dynamic (`io --strict` exit 7) |
| `with udp multicast "group:p"` + `bind "0.0.0.0:p"` [+ `interface "IF"`] | `allow_network udp://group:p` **and** `allow_listen udp://0.0.0.0:p` with `bind` and `multicast_join` | with those grants the join succeeds and a quiet group times out (`timeout` "no datagram within 1000 ms"); a `bind`-only access list → `permission.denied` "… does not include `multicast_join`" |

Manifest discrepancy (0.1.0): `rivet io --check-policy` reports the multicast site as `multicast_join` on the
**group** target, while the runtime checks `multicast_join` on the **bind** address. To make both agree, also
grant `{"capability":"allow_listen","targets":["udp://239.0.0.1:5000"],"access":["multicast_join"]}`.

A successful send proves only local acceptance; a lost reply is a timeout with uncertain effects. Clipped
payloads fail `udp.truncated`. Demo: [08-udp](../demos/08-udp/README.md).

### QUIC

```rivet
with quic "quic://engine.example.com:4433" as connection
    alpn "rivet-rpc/1"
    max_streams 8
    migration false
    with connection.open bidi as stream
        framing length32 endian big max_frame 1048576
        stream.send json {action: "status"}
        return stream.receive json timeout "5s"
    end
end
```

Grant `{"capability": "allow_network", "targets": ["quic://engine.example.com:4433"]}`.

| Rule | Error |
|---|---|
| ALPN is required | `quic.alpn_required` "`with quic` needs `alpn \"ID\"`" (exit 2) |
| connection migration is refused | `migration true` → `unsupported.quic_migration` "use `migration false`" (exit 5) |
| 0-RTT (early data) is off | — |
| datagrams need peer support | `datagrams true` + `connection.send_datagram` / `receive_datagram`; unavailable → `quic.datagrams_unavailable` |

Streams: `connection.open bidi|uni`, `connection.accept uni`; `stream.finish_send` half-closes (FIN) while the
response stays readable. Verified in [09-quic](../demos/09-quic/README.md) up to DNS (`dns.resolve` for the
fixture host) and with local refusals above; no QUIC fixture server was run for this manual.

### gRPC

```rivet
connector users grpc
    endpoint "https://users.example.com:443"
    descriptor "./schemas/users.pb"
    service "example.Users"
end

operation users.grpc_get
    name "Get a user over gRPC"
    description "Read one user from the gRPC service."
    param id text required description "User ID."
    output object description "The example.User message."
        field id text required description "User ID as stored by the service."
        field name text required description "Display name."
    end
    response = grpc users.GetUser
        timeout "5s"
        message {id: id}
    end
    return response.message
end
```

- `descriptor` is a pinned `FileDescriptorSet` (`protoc --include_imports --descriptor_set_out=users.pb …`); it is
  a bootstrap read. Missing: `not_found.descriptor` with that hint (exit 4) — the bundle does not load.
- Modes follow the method: unary (`grpc C.M`), server streaming (`with grpc C.M as rpc` + `for message in rpc`),
  client streaming (`rpc.send`, `rpc.finish_send`, `rpc.result`), bidi (`concurrent` sender/receiver tasks,
  `for message in incoming` for live input).
- Grants: `allow_network` for the endpoint and `allow_grpc` per method (`users/example.Users/GetUser`):

```text
TARGET                         ACCESS               CAPABILITY     USED BY
https://users.example.com:443  connect POST (grpc)  allow_network  chat.exchange, users.grpc_get, users.upload, users.watch
users/example.Users/Chat       call bidi            allow_grpc     chat.exchange
users/example.Users/GetUser    call unary           allow_grpc     users.grpc_get
users/example.Users/Upload     call client_stream   allow_grpc     users.upload
users/example.Users/Watch      call server_stream   allow_grpc     users.watch
```

- Status failures are `grpc.<code>`; late failures after partial output keep the emitted items.
- Verified with [10-grpc](../demos/10-grpc/README.md) plus the test descriptor `tests/fixtures/grpc/users.pb`:
  compile, `describe`, `io` and the `dns.resolve` boundary; `chat.exchange` without input →
  `stream.input_required`. No gRPC fixture server was run for this manual.

### OAuth 2.0

| Flow | Profile keys | How a token is obtained |
|---|---|---|
| `client_credentials` | `token_url`, `client_id`, `client_secret env "VAR"`, `client_auth basic\|post` | first authorized use of `auth PROFILE account "A"` |
| `authorization_code` + `pkce s256` | `authorization_url`, `token_url`, `redirect_uri`, `client_auth none` | `rivet auth begin` → host shows `authorization_url` → `rivet auth complete --params-file callback.json` |
| `device_code` | `device_url`, `token_url` | `rivet auth begin` → user visits `verification_uri` with `user_code` → `rivet auth complete … "wait":true` |

All profiles take `issuer`, `scopes`, `resource_origins` (the only origins the token may be sent to) and
`store memory|keychain "NS"`.

Grants (verified wording):

```json
{"capability": "allow_auth",        "targets": ["crm_service/service/use", "crm_service/service/status", "crm_service/service/manage"]},
{"capability": "allow_credentials", "targets": ["crm_service/service"]},
{"capability": "allow_env",         "targets": ["CRM_CLIENT_SECRET"]},
{"capability": "allow_network",     "targets": ["http://127.0.0.1:18490"]}
```

`allow_auth` targets are `PROFILE/ACCOUNT/VERB` and accept globs (`crm_device/ada/*`).

Client credentials against a local token endpoint and resource server:

```text
$ CRM_CLIENT_SECRET=s3cret rivet request --file app.rivet contacts.list
{"request_id":"req_01287d6885","trace_id":"tr_01287d6885","result":{"contacts":[{"name":"Ada"}]},"data_count":0,"effects":"none"}
```

Device code on one server (transactions and `store memory` live in the serving process):

```text
 rivet --endpoint E auth begin crm_device --account ada
   → {"transaction_id":"auth_01inhukq474nwe","expires_at":"…","verification_uri":"http://127.0.0.1:18491/activate","user_code":"ABCD-EFGH","interval_seconds":1}
 rivet --endpoint E auth complete --timeout 2s --params '{"transaction_id":"auth_01inhukq474nwe","wait":true}'
   → {"state":"pending","transaction_id":"auth_01inhukq474nwe","expires_at":"…"}          (user has not approved yet)
 (user approves)
 rivet --endpoint E auth complete --timeout 10s --params '{"transaction_id":"auth_01inhukq474nwe","wait":true}'
   → {"profile":"crm_device","account":"ada","state":"connected","scopes":["contacts.read"],"expires_at":"…","generation":1}
 rivet --endpoint E auth disconnect crm_device --account ada
   → {"profile":"crm_device","account":"ada","local_only":true,"generation":2}
 rivet --endpoint E auth status crm_device --account ada
   → {…"state":"disconnected","scopes":[],"expires_at":null,"generation":2}
```

Authorization code: `auth begin crm_user --account ada` returned `authorization_url` with `response_type=code`,
`state`, `code_challenge` and `code_challenge_method=S256`. Complete it with a params **file** (keeps codes out
of argv): `{"transaction_id":"…","callback":{"code":"…","state":"…","redirect_uri":"…","issuer":"…"}}`.

| Error | Cause | Exit |
|---|---|---|
| `auth.client_secret_missing` | `client_secret env` variable unset | 5 |
| `validation.auth_flow` | `auth begin` on a client_credentials profile | 2 |
| `permission.denied` "allow_auth status crm_service/service/status denied" | missing `allow_auth` verb | 3 |
| `not_found.auth_transaction` / `not_found.auth_profile` | unknown transaction/profile | 4 |

Disconnect is local only (no provider revocation). Tokens are never printed. Demo:
[07-oauth2](../demos/07-oauth2/README.md).

### MCP client connectors

Why: call a remote MCP server's tools as ordinary Rivet operations, but only after a human reviewed exactly what
the server offers.

```text
 1 declare connector ─► 2 rivet connectors sync NAME --output ./schemas/NAME.json (new file, never overwrites)
 ─► 3 review the snapshot ─► 4 add its sha256 to policy.json approved.snapshots ─► 5 rivet check ─► 6 call
       │ bundle does not load until 4:  not_found.mcp_snapshot (4) / mcp.snapshot_unapproved (2)
```

Verified end to end with a second Rivet as the MCP server (stdio):

```rivet
connector peer mcp
    transport command "/path/to/rivet/target/debug/rivet"
        args ["--file", "/path/to/rivet/docs/demos/01-catalog/app.rivet", "serve", "--stdio"]
    end
    schema "./schemas/peer.json"
    expose tools ["demo.add"]
end

operation bridge.add
    name "Bridge add"
    description "Call the peer's demo.add tool."
    param a integer required description "First operand."
    output json description "The peer's sum."
    r = (request "peer.tools.demo.add" {a: a, b: 40})
    return r.structuredContent.result
end
```

```json
{
  "version": 1,
  "grants": [
    {"capability": "allow_exec",  "targets": ["/path/to/rivet/target/debug/rivet"]},
    {"capability": "allow_read",  "targets": ["/path/to/rivet/docs/demos/01-catalog/**"]},
    {"capability": "allow_write", "targets": ["./schemas/**"]},
    {"capability": "allow_mcp",   "targets": ["peer/discover", "peer/tools/demo.add"]}
  ],
  "approved": {"snapshots": ["sha256:9173a4bbfd55693b49cfd2d68c793dac5496b8bd7c461d0fcae145c9b57cc491"]}
}
```

(The stdio child runs in the OS sandbox built from these grants, so it needs `allow_read` on the catalog it
serves; the `allow_write` grant is for the snapshot file and must not be `access`-narrowed, or the sandbox
refuses.)

```text
$ rivet check --file app.rivet
error[not_found.mcp_snapshot]: cannot read snapshot ./schemas/peer.json of connector `peer` …
  = hint: create it with `rivet connectors sync peer --output ./schemas/peer.json`, review it and approve its sha256 in policy.json
$ rivet connectors --file app.rivet sync peer --output ./schemas/peer.json
{"connector":"peer","path":"./schemas/peer.json","sha256":"sha256:9173a4bb…","protocolVersion":"2025-11-25","tools":["demo.greet","demo.add",…],"resources":[],"prompts":[]}
$ rivet check --file app.rivet            # before approving                                  [exit 2]
error[mcp.snapshot_unapproved]: snapshot ./schemas/peer.json of connector `peer` is not reviewed: sha256:9173a4bb… is not listed in policy.json approved.snapshots
$ rivet check --file app.rivet            # after approving
ok: 1 operations, 1 connectors, 0 auth profiles
$ rivet request --file app.rivet bridge.add --params '{"a":2}'
{"request_id":"req_013839c10d","trace_id":"tr_013839c10d","result":42,"data_count":0,"effects":"committed"}
```

- The snapshot starts with `"format": "rivet.mcp.snapshot/1"`; re-running `sync` on an unchanged server yields
  the same sha256.
- Imported tools are listed as `CONNECTOR.tools.NAME` and return the MCP result
  `{content, structuredContent, isError}` (`effects: "unknown"` when called directly); `isError: true` becomes
  `mcp.tool_failed`.
- `transport http "https://…/mcp"` (Streamable HTTP; `allow_network`) and `transport command "/abs/bin"`
  (`allow_exec`) are supported; `auth PROFILE account "A"` adds an OAuth token for HTTP transports.
- Sampling and other server-to-client requests are declined. Legacy HTTP+SSE MCP servers are not supported.

| Error | Cause | Exit |
|---|---|---|
| `not_found.mcp_snapshot` | schema file missing | 4 |
| `mcp.snapshot_unapproved` | sha256 not in `approved.snapshots` | 2 |
| `permission.denied` "allow_mcp call peer/discover denied" | sync without a discover grant | 3 |
| `conflict.already_exists` | sync `--output` exists | 4 |
| `not_found.mcp_connector` | unknown connector name | 4 |
| `mcp.tool_failed` | remote tool returned `isError: true` | 5 |

Demo: [06-mcp-bridge](../demos/06-mcp-bridge/README.md) (its remote fixture server is not public; the
walkthrough above used a local Rivet peer).

### Expected Result and Side Effects

Every transport records broker decisions in the trace (`effect_id` per site). Connections, sockets and child
processes are closed when their block, scope or request ends. OAuth credentials persist only in the selected
store (`memory` = the running process).

### Verified Demo

[03-http](../demos/03-http/README.md), [04-streaming](../demos/04-streaming/README.md),
[06-mcp-bridge](../demos/06-mcp-bridge/README.md), [07-oauth2](../demos/07-oauth2/README.md),
[08-udp](../demos/08-udp/README.md), [09-quic](../demos/09-quic/README.md), [10-grpc](../demos/10-grpc/README.md),
[11-sandbox](../demos/11-sandbox/README.md) — each run with `rivet 0.1.0-dev` (commit `f40d4aa`) as described in
its section.

## Errors and Recovery Reference

| Error / Code / Message | Surface | Cause | User-Visible Result | Recovery | Retry Safe | Related Feature |
|---|---|---|---|---|---|---|
| `permission.denied` | all | missing grant, deny, private range | exit 3, nothing sent | add a literal grant | no | all transports |
| `dns.resolve` | network | host does not resolve | exit 5 | fix host/DNS | yes | all |
| `connection.*`, `udp.receive_failed` | network | refused/reset | exit 5; check `effects` | fix the peer; retry if idempotent | if replay-safe | all |
| `http.status` | HTTP | status not in `accept status` | exit 5, `details.status` | accept it and branch, or fix | per method | HTTP |
| `http.version_unavailable` | HTTP/3 | H3 impossible | exit 5, `request_sent:false` | use `prefer [3, 2]` or HTTPS | yes | HTTP/3 |
| `timeout` / `timeout.*` | all | deadline | exit 6 | raise timeouts | caller decides | all |
| `process.exit` | process | nonzero exit | exit 5, `details.stderr` | inspect stderr; `accept exit` | depends | processes |
| `unsupported.shell`, `validation.process_program` | process | shell string / bare name | exit 5 / 2 | pass argv to an absolute path | no | processes |
| `unsupported.sandbox_backend` | process, MCP stdio | no backend or inexpressible grants | exit 5 | simplify grants or run on macOS | no | sandbox |
| `quic.alpn_required`, `unsupported.quic_migration` | QUIC | missing ALPN / migration on | exit 2 / 5 | add `alpn`, set `migration false` | no | QUIC |
| `not_found.descriptor` | gRPC | missing descriptor | exit 4 at load | generate it with protoc | no | gRPC |
| `auth.client_secret_missing`, `validation.auth_flow` | OAuth | env unset / wrong command | exit 5 / 2 | set the env var / use first use | no | OAuth |
| `not_found.mcp_snapshot`, `mcp.snapshot_unapproved` | MCP | review not done | exit 4 / 2 at load | sync, review, approve | no | MCP |

## Limitations

- No connection pooling; no Alt-Svc discovery; no QUIC migration or 0-RTT.
- Process sandbox active on macOS only; Linux gated; others refuse.
- `with file open` / `with file watch` are unavailable at run time (MAN-2026-0003).
- MCP: no legacy HTTP+SSE transport; server-to-client requests (sampling) are declined.
- `rivet io --check-policy` names a different target for `multicast_join` than the runtime checks (workaround
  above).
- WebSocket, QUIC, HTTP/3 and gRPC were verified in this manual only up to the policy and DNS boundaries; their
  wire behaviour is covered by the conformance suites (`tests/conformance_*.rs`).

## Version Applicability

| Feature / Interface | Introduced | Changed | Deprecated / Removed | Applicable Environment |
|---|---|---|---|---|
| HTTP/1.1, HTTP/2, streams, redirects, retries | 0.1.0 | — | — | all |
| HTTP/3, QUIC v1 | 0.1.0 | — | — | all |
| WebSocket, TCP, Unix, UDP | 0.1.0 | — | — | all (Unix sockets: Unix-like OS) |
| Processes + Seatbelt sandbox | 0.1.0 | — | — | macOS (Linux gated) |
| gRPC, OAuth 2.0, MCP client connectors | 0.1.0 | — | — | all |

## Related Documents

- [Rivet manual](man-2026-0001-rivet-manual.md) · [Language guide](man-2026-0003-language-guide.md) ·
  [Policy guide](man-2026-0005-policy-and-io-manifest-guide.md) · [Serving](man-2026-0006-serving-and-surfaces.md)
- [SYS-2026-0006 OAuth and credentials](../system/integrations/sys-2026-0006-oauth-and-credentials.md)
- [ADR-0003 process sandbox backends](../decisions/adr-0003-process-sandbox-backends.md)
- [REF-2026-0002 examples S19–S30, S52–S61, S81–S102, S110–S118](../references/ref-2026-0002-language-and-usage.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial protocols and connectors guide for 0.1.0, verified against 0.1.0-dev commit f40d4aa with local fixtures. |
