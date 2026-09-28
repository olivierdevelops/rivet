---
document_id: SYS-2026-0005
title: "Rivet protocol adapters"
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
components: [transports, datagrams, quic, grpc, http]
affected_versions:
  from: "0.1.0"
  to: null
last_verified_version: "0.1.0-dev (commit 829ca43)"
next_review_date: 2026-10-28
review_cycle: on-release
confidentiality: internal
scope: The outbound protocol adapters behind Rivet effect statements (HTTP/1.1, HTTP/2, HTTP/3, SSE/JSONL/lines/bytes streams, TCP/Unix sockets, WebSocket client, argv-only processes and their OS sandboxes, UDP, native QUIC, gRPC and shared TLS), how each is wired, authorized, bounded and which error codes it emits.
reason: Every network, socket and child-process effect a script performs goes through one of these adapters; maintainers and reviewers need one current-state map of what each adapter accepts, what it refuses, what it checks before the first byte leaves, and what the binary actually returns.
related_documents: [PROP-2026-0001, PLAN-2026-0001, SYS-2026-0002, SYS-2026-0003, SYS-2026-0004, SYS-2026-0006, SYS-2026-0008, SYS-2026-0009, ADR-0002, ADR-0003]
supersedes: null
superseded_by: null
tags: [rivet, system, transports, http, http3, quic, udp, grpc, websocket, sockets, process, sandbox, tls]
---

# Rivet protocol adapters

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** transports, datagrams, quic, grpc, http
> **Last Verified Version:** 0.1.0-dev (commit 829ca43)

## Summary

Rivet scripts reach the outside world through effect statements such as `http get …`, `with tcp …`,
`command "/bin/cat" …`, `with udp …`, `with quic …` and `grpc users.GetUser …`. The interpreter hands each
effect kind to an **adapter**, which is an infra component registered by name. Each adapter follows the
same three steps:

1. It parses the evaluated option lines into a plain domain **plan**: `HttpExchange`, `SocketPlan`,
   `ProcessPlan`, `DatagramPlan`, `QuicPlan` or `GrpcPlan`.
2. It runs a **feature use case**, injected as a closure. The use case authorizes every target through the
   `PolicyEvaluator` and resolves names once. It then keeps only addresses that pass the private-range
   re-check.
3. It hands that checked address to a **driver** (the port implementation) that does the wire work.

No driver resolves names by itself, follows redirects by itself or widens a grant.

```text
                        effect statement in app.rivet
                                   │
                  Interpreter::register_adapter(kind, adapter)
                                   │
  ┌──────────┬──────────────┬──────┴──────┬─────────────┬─────────────┬──────────────┐
  │ http     │ tcp|unix|    │ command     │ udp         │ quic        │ grpc         │
  │          │ websocket    │             │             │             │              │
  ▼          ▼              ▼             ▼             ▼             ▼              │
 HttpEffects SocketEffects  ProcessEffects UdpAdapter   QuicAdapter   GrpcEffects    │ infra adapters
  │          │              │             │             │             │              │
  ▼          ▼              ▼             ▼             ▼             ▼              │
 transports. transports.    transports.   datagrams.    quic.         grpc.          │ feature use
 exchange_   exchange_      run_process   exchange_     exchange_     invoke_rpc     │ cases: authorize
 http        socket                       datagrams     quic                         │ + checked address
  │          │              │             │             │             │              │
  ▼          ▼              ▼             ▼             ▼             ▼              │
 HyperClient Dialer         TokioRunner   UdpSession    QuicSession   GrpcTransport  │ drivers (ports)
 (+h3_client)(FramedConn,   (+Seatbelt |  (tokio +      (quinn,       (tonic +       │
             WsConn)        Landlock |    socket2)      rustls)       prost-reflect) │
             net_tls        refuse)                                                  │
```

All six adapters were exercised against the 0.1.0-dev binary for this document. The runs used local
fixtures on 127.0.0.1 and no external network. The results appear in each section. The full list of what was run is in
[Last Verified Version](#last-verified-version).

## Responsibilities

- **HTTP/1.1 and HTTP/2 client** (`src/infra/http_adapter.rs`, `src/features/transports/exchange_http.rs`).
  Rivet's own connection-level client over `hyper::client::conn`. It opens one connection per attempt and
  negotiates ALPN `h2` then `http/1.1` for https. It never follows redirects unless the script says
  `redirect follow limit N`. It retries only replay-safe methods on listed statuses.
- **HTTP/3** (`src/infra/h3_client.rs`). QUIC v1 with ALPN `h3` only and 0-RTT off, sent to the **same
  checked host:port** the https grant covers. `version 3` is strict. `version prefer [3, …]` falls back to
  TCP only when the HTTP/3 attempt provably wrote no request byte.
- **Streamed bodies** (`src/infra/codec.rs`). `with http … as NAME` + `stream sse|jsonl|lines|bytes`, and
  `with command … as p` + `stream stdout lines|jsonl|bytes`, decoded incrementally with backpressure.
- **Sockets** (`src/infra/socket_adapter.rs`, `src/features/transports/exchange_socket.rs`). Scoped
  `with tcp`, `with unix` and `with websocket` connections. TCP and Unix connections use explicit framing
  (newline, length32 big|little, delimiter, raw). WebSocket connections keep message boundaries.
- **Processes** (`src/infra/process_adapter.rs`, `src/features/transports/run_process.rs`). Argv-only
  children with no shell, no PATH lookup and an empty environment plus `PATH=/usr/bin:/bin`. Output
  capture is bounded, the child is always reaped, and it runs inside an OS sandbox whenever `policy.json`
  is present (`src/infra/sandbox_macos.rs`, `sandbox_linux.rs`, `sandbox_unsupported.rs`).
- **UDP** (`src/infra/udp_adapter.rs`, `src/features/datagrams/exchange_datagrams.rs`). Scoped connected
  unicast, explicit `bind` listeners and multicast groups. Every step is authorized as a one-step
  `DatagramPlan`.
- **Native QUIC** (`src/infra/quic_adapter.rs`, `src/features/quic/exchange_quic.rs`). Scoped QUIC v1
  connections with one required ALPN, bidi and uni child streams with framing, and optional DATAGRAM
  frames. Migration and 0-RTT are refused.
- **gRPC** (`src/infra/grpc_adapter.rs`, `src/features/grpc/invoke_rpc.rs`, `src/domain/grpc.rs`). All four
  call modes from a pinned `FileDescriptorSet`, using ProtoJSON messages, no server reflection and no
  generated code.
- **TLS and DNS plumbing** (`src/infra/net_tls.rs`). This is the one resolver entry point. It builds the
  rustls client config with the ring provider and either the platform verifier or `tls ca_file` roots.
- **Wiring** (`src/orchestrator/transports.rs`, `register_transports` and the gRPC block in
  `src/orchestrator/runtime.rs`).

## Boundaries and Non-Responsibilities

- **Policy decisions** belong to the policy broker. The adapters build `EffectIntent`s and obey the `Permit`.
  See [SYS-2026-0003](../components/sys-2026-0003-policy-broker-and-io-manifest.md) and the
  `policy.json` key reference in [SYS-2026-0008](../configuration/sys-2026-0008-policy-json-reference.md).
- **Scopes, deadlines and cleanup ordering** of `with` blocks belong to the interpreter. See
  [SYS-2026-0002](../runtime/sys-2026-0002-execution-scopes-and-dag.md). The adapters implement
  `ResourceHandle::close` and respect the deadline that the context hands them.
- **OAuth token acquisition** (`auth PROFILE account A` on `http` and `grpc`) is delegated to the
  credential provider. See [SYS-2026-0006](sys-2026-0006-oauth-and-credentials.md).
- **MCP client connectors** reuse the HTTP client and `confine_process`, but their protocol lives elsewhere.
  See [SYS-2026-0009](sys-2026-0009-mcp-client-connectors.md).
- **Inbound serving** (REST, SSE, WebSocket and MCP server surfaces) is not in scope. See
  [SYS-2026-0004](../components/sys-2026-0004-surfaces-and-serve.md).
- The 0.1.0 adapters do **not** provide:
  - connection pooling
  - Alt-Svc discovery
  - TLS on raw TCP or Unix sockets
  - socket reconnect
  - interactive processes
  - QUIC migration or 0-RTT
  - gRPC compression or reflection
  - a certified Linux or Windows process sandbox

  See [Known Limitations](#known-limitations).

## Architecture

### Adapter map

```text
 effect kind   adapter (infra)                use case (feature)                   driver / port impl          registered in
 ───────────   ────────────────────────────   ──────────────────────────────────   ─────────────────────────   ─────────────────────────────
 http          HttpEffects                    transports.exchange_http             HyperClient : HttpClient    orchestrator/transports.rs
               src/infra/http_adapter.rs      features/transports/exchange_http.rs   └ h3_client::send (H3)     register()
 tcp           SocketEffects                  transports.exchange_socket           Dialer : SocketStream       orchestrator/transports.rs
 unix          src/infra/socket_adapter.rs    features/transports/exchange_socket.rs   FramedConn | WsConn
 websocket
 command       ProcessEffects                 transports.run_process               TokioRunner : ProcessRunner orchestrator/transports.rs
               src/infra/process_adapter.rs   features/transports/run_process.rs     └ sandbox::command (cfg OS)
 udp           UdpAdapter                     datagrams.exchange_datagrams         UdpSession : DatagramDriver orchestrator/runtime.rs
               src/infra/udp_adapter.rs       features/datagrams/exchange_datagrams.rs                          register_transports()
 quic          QuicAdapter                    quic.exchange_quic                   QuicSession : QuicDriver    orchestrator/runtime.rs
               src/infra/quic_adapter.rs      features/quic/exchange_quic.rs                                   register_transports()
 grpc          GrpcEffects                    grpc.invoke_rpc                      GrpcTransport : GrpcDriver  orchestrator/runtime.rs
               src/infra/grpc_adapter.rs      features/grpc/invoke_rpc.rs                                      Runtime::assemble
```

Shared infra:

```text
 src/infra/net_tls.rs     resolve(host,port) · client_config(TlsMaterial, alpn) · server_name · connect_err / handshake_err
 src/infra/codec.rs       StdCodec (json|text|bytes|xml|form) · StreamDecoder (sse|jsonl|lines|bytes) · encode_frame/take_frame
 src/infra/wire_codec.rs  json|text|bytes encode/decode + argument helpers for udp/quic handles
 src/infra/effect_args.rs options(), budget_ms(), apply_tls() (tls files read through the policed FileAccess)
 src/domain/effect_checks.rs  authorize() + checked_addr() used by exchange_http / exchange_socket
```

### Domain types

`src/domain/transports.rs` covers HTTP, sockets and processes:

```text
 HttpExchange ─┬─ method, url, headers, query, body: CodecInput?, version: HttpVersionPolicy
               ├─ decode: CodecKind?, accept: [u16], retry: RetryPolicy?, redirect_limit (0 = never follow)
               ├─ tls: TlsMaterial{server_name, ca_pem, cert_pem, key_pem}, stream: StreamMode?, unix_socket?
               └─ max_body (default 8 MiB), origin: EffectOrigin{operation_id, span}
        │ use case authorizes + resolves
        ▼
 HttpWire { …, target: WireTarget::Tcp(checked SocketAddr) | Unix(path), version, stream: bool }
        │ HttpClient::send
        ▼
 HttpReply { status, headers, version "3"|"2"|"1.1"|"1.0", body: Complete(bytes) | Stream(ByteStream) }
        │ accept / decode
        ▼
 HttpResponse.to_value() = {status, headers, body, version}   (version: 3, 2 integers; 1.1 float)

 HttpVersionPolicy: Auto | Http1 | Http2 | Http3 | Http3OrHttp2 | Http3OrHttp1 | Http3OrAuto
 SocketPlan { scheme tcp|unix|ws|wss, endpoint, host?, port?, framing, max_frame, tls_requested, tls, timeout_ms?, reconnect }
 ProcessPlan { program, resolved, args, env, cwd?, stdin?, timeout_ms, accept_exit, decode_stdout/stderr,
               stream?, sandbox: SandboxSpec?, max_output, interactive }
 SandboxSpec { read, write, exec, deny_read, deny_write }   (absolute prefixes; never network)
```

`src/domain/transport.rs` covers UDP and QUIC:

```text
 DatagramPlan { mode Connected|Bind|Multicast, peer?, peer_addr?, bind?, multicast?, interface?,
                max_datagram (default 8192, max 65507), timeout_ms?, steps: [DatagramStep], context }
 DatagramStep  Open | Send{payload} | SendTo{peer, addr?, payload} | Receive{timeout} | ReceiveFrom{timeout} | Close
 QuicPlan     { endpoint, server_name, alpn, max_streams (default 8, max 1024), datagrams, migration,
                early_data, timeout_ms?, tls: QuicTls, peer_addr?, steps: [QuicStep], context }
 QuicStep      Connect | OpenStream | AcceptStream | Send | Receive | FinishSend | CloseStream
               | SendDatagram | ReceiveDatagram | Close
 Framing       { kind Raw|Newline|Length32{big_endian}|Delimiter(bytes), max_frame (default 8 MiB) }
```

`src/domain/grpc.rs` covers gRPC:

```text
 GrpcCatalog ─ GrpcConnectorInfo { name, endpoint, service, descriptor, descriptor_hash "sha256:…", tls, methods[] }
                 └ GrpcMethodInfo { connector, service, method, mode: RpcMode, input_type, output_type }
                    target() = "CONNECTOR/pkg.Service/Method"  (the allow_grpc target)
 RpcMode       unary | server_stream | client_stream | bidi
 GrpcPlan      { connector, method, usage one_shot|scoped, message?, metadata[], timeout_ms?, deadline_ms, auth? }
 GrpcTerminal  { status: GrpcStatus{code, name, message}, initial_metadata, trailers }
```

### Authorization pattern shared by every use case

```text
   plan ──▶ static guards (unsupported / validation, zero effects)
        ──▶ authorize(capability, verb, logical target)      ── Denied ─▶ permission.denied (exit 3)
        ──▶ name? ── literal IP ─────────────────────────────────────┐
              │                                                     │
              └─ resolve once (net_tls::resolve / driver.resolve)   │
                   for each address:                                │
                     public ─────────────────────────────▶ use it   │
                     private & deny_private_ranges ─▶ authorize     │
                        "scheme://IP:PORT" literally ─ ok ─▶ use it │
                                                   └ denied: next   │
        ──▶ driver dials exactly the checked SocketAddr ◀───────────┘
```

A hostname never inherits authority over a private address. Only a grant that names the IP literally lets a
name that resolves to 10/8, 127/8, 169.254/16, 100.64/10, fc00::/7 or fe80::/10 through. The same
predicate is implemented as `is_private_ip` in `src/domain/transports.rs` and `src/domain/transport.rs`,
and as `is_private` in `invoke_rpc.rs`. The copies exist because features cannot import each other.

## Interfaces

### Ports

| Port | Methods | Implemented by |
|---|---|---|
| `HttpClient` | `resolve(host, port)`, `send(HttpWire) -> HttpReply`, `wait(delay_ms, jitter)` | `HyperClient` (`http_adapter.rs`) |
| `Codec` | `decode(CodecInput)`, `encode(CodecInput)` | `StdCodec` (`codec.rs`) |
| `SocketStream` | `resolve`, `connect(plan, checked addr) -> SocketConnection` | `Dialer` (`socket_adapter.rs`) |
| `SocketConnection` | `send(Frame)`, `receive()`, `finish_send()`, `close()` | `FramedConn`, `WsConn` |
| `ProcessRunner` | `run`, `spawn`, `spawn_duplex` (MCP stdio) | `TokioRunner` (`process_adapter.rs`) |
| `ByteStream` | `next_chunk`, `finish` (late exit failure), `close` | `HyperStream`, `H3Body`, `ProcStream` |
| `DatagramDriver` | `resolve`, `exchange(DatagramPlan)` | `UdpSession` (`udp_adapter.rs`) |
| `QuicDriver` | `resolve`, `exchange(QuicPlan)` | `QuicSession` (`quic_adapter.rs`) |
| `GrpcDriver` | `catalog`, `validate_input`, `resolve`, `invoke(GrpcDial)` | `GrpcTransport` (`grpc_adapter.rs`) |

### Use cases

| Use case | Signature | Needs |
|---|---|---|
| `transports.exchange_http` | `HttpExchange -> HttpOutcome` | `HttpClient`, `Codec`, `PolicyEvaluator` |
| `transports.exchange_socket` | `SocketPlan -> SocketConnection` | `SocketStream`, `PolicyEvaluator` |
| `transports.run_process` | `ProcessPlan -> ProcessOutcome` | `ProcessRunner`, `Codec`, `PolicyEvaluator` |
| `datagrams.exchange_datagrams` | `DatagramPlan -> DatagramResult` | `DatagramDriver`, `PolicyEvaluator` |
| `quic.exchange_quic` | `QuicPlan -> QuicResult` | `QuicDriver`, `PolicyEvaluator` |
| `grpc.invoke_rpc` | `GrpcPlan -> GrpcCall` | `GrpcDriver`, `PolicyEvaluator`, `CredentialProvider` |

`confine_process` in `run_process.rs` is public. MCP stdio connectors use it so that they get exactly the
same shell guard, `allow_exec` check and sandbox as `command`.

### Script-visible values

| Form | Value |
|---|---|
| `r = http …` | `{status, headers (lower-case keys, repeats joined with ", "), body, version}` |
| `with http … as s` | iterate `for x in s`. Properties are `s.status`, `s.headers` and `s.version` |
| `out = command …` | `{stdout, stderr, exit, duration_ms}` |
| `with command … as p` | iterate `for line in p.stdout` |
| `with tcp/unix/websocket … as c` | `c.send KIND V`, `c.receive KIND [timeout "D"]`, `c.finish_send`, `for f in c`, and `c.close` on WebSocket only |
| `with udp … as s` | `s.send`, `s.send_to PEER`, `s.receive`, `s.receive_from` (returns `{peer, data}`), `for p in s` (returns `{peer, data: bytes}`) |
| `with quic … as c` | `with c.open bidi\|uni as st`, `with c.accept bidi\|uni as st`, `c.send_datagram`, `c.receive_datagram` and `c.alpn`. The child stream `st` has `st.send`, `st.receive`, `st.finish_send` and `for f in st` |
| `r = grpc C.M …` | `{message, initial_metadata, trailers, status}` |
| `with grpc C.M as rpc` | `rpc.send V`, `rpc.finish_send`, `for m in rpc`, `rpc.result` and `rpc.completion` |

### CLI surfaces that expose adapters without running them

`rivet check` compiles the bundle and loads gRPC descriptors. `rivet io` lists every I/O site with its
access verb. `rivet io --check-policy` evaluates each site against `policy.json`. `rivet describe` prints
the operation contract. Each command was verified against the demo folders. From `docs/demos/`:

```text
$ (cd 03-http && rivet --file app.rivet io)
OPERATION     KIND     ACCESS        TARGET                                             KNOWLEDGE        SOURCE
users.create  network  connect POST  https://api.example.com/users                      exact            app.rivet:29
users.get     network  connect GET   https://api.example.com/users/{id}                 param_dependent  app.rivet:10
users.search  network  connect GET   https://api.example.com/search?q={query}&limit=10  param_dependent  app.rivet:41

$ (cd 09-quic && rivet --file app.rivet io --check-policy)      # exit 3
OPERATION      KIND     ACCESS       TARGET                          KNOWLEDGE  SOURCE        DECISION
engine.status  network  connect      quic://engine.example.com:4433  exact      app.rivet:7   allowed
items.http3    network  connect GET  https://api.example.com/items   exact      app.rivet:26  denied
1 allowed · 1 denied
```

The shipped `09-quic/policy.json` grants only the QUIC origin. `policies/http3.json` grants
`https://api.example.com:443` for `items.http3`.

`10-grpc` does not ship the binary descriptor, so `rivet check` fails at bundle load until it is
generated:

```text
$ (cd 10-grpc && rivet --file app.rivet check)                   # exit 4
error[not_found.descriptor]: cannot read descriptor ./schemas/users.pb of connector `users`: No such file or directory (os error 2)
  --> app.rivet:1:1
   |
  1| connector users grpc
   | ^
  = hint: generate it with protoc --include_imports --descriptor_set_out=… before loading the bundle
```

The next run used a scratch copy after
`protoc --proto_path=schemas --include_imports --descriptor_set_out=schemas/users.pb schemas/users.proto`:

```text
$ rivet --file app.rivet check
ok: 4 operations, 1 connectors, 0 auth profiles
$ rivet --file app.rivet io
OPERATION       KIND     ACCESS              TARGET                         KNOWLEDGE  SOURCE
chat.exchange   network  connect POST        https://users.example.com:443  exact      app.rivet:2
chat.exchange   grpc     call bidi           users/example.Users/Chat       exact      app.rivet:68
users.grpc_get  network  connect POST        https://users.example.com:443  exact      app.rivet:2
users.grpc_get  grpc     call unary          users/example.Users/GetUser    exact      app.rivet:15
users.upload    network  connect POST        https://users.example.com:443  exact      app.rivet:2
users.upload    grpc     call client_stream  users/example.Users/Upload     exact      app.rivet:47
users.watch     network  connect POST        https://users.example.com:443  exact      app.rivet:2
users.watch     grpc     call server_stream  users/example.Users/Watch      exact      app.rivet:30
```

## Configuration

Adapters take their configuration from the option lines inside the effect block and from `policy.json`
(grants, deny, `network.deny_private_ranges`). There are no environment variables or config files. The tables
below list every option each parser accepts. Any other option returns the per-adapter
`validation.*_option` error, with exit code 2.

### HTTP (`http METHOD URL … end`, `with http METHOD URL as NAME … end`)

| Option | Values | Default | Notes / errors |
|---|---|---|---|
| `query K V` | text | — | Form-encoded and appended to the URL. |
| `header K V` | text | `user-agent: rivet/0.1.0` added if absent | — |
| `body json\|text\|form\|bytes V` | value | none | Content-Type is added unless a header sets one. `form` needs an object and `bytes` needs bytes or text (`validation.codec`). |
| `body xml V` | `(xml.element …)` or text | — | Other types return `validation.http_option`. |
| `body file PATH` | path | — | Read through the policed file port (`allow_read`) before connecting. |
| `body multipart` + `field N V` / `file N PATH [TYPE]` child lines | — | — | Boundary `rivet-<32 hex>` from the CSPRNG. File parts are authorized reads. |
| `decode json\|text\|bytes\|xml\|form` | — | text when UTF-8, else bytes | A failed decode of an accepted non-2xx body falls back to text. |
| `accept status [CODES]` | int list | any 2xx | Any other status returns `http.status`, with `details.status` and `details.method`. |
| `timeout "D"` | duration | request deadline | Capped at the remaining deadline. Returns `timeout.http` when exceeded. |
| `redirect follow limit N` | int | 0 (never follow) | Without this line, a 3xx is returned as the response. More than N hops returns `limit.http_redirects`. A bad `Location` returns `protocol.http_location`. |
| `retry N on status [..] backoff exponential\|fixed base "D" max "D" jitter B` | — | on `[429, 502, 503, 504]`, base 100 ms, max 2 s, exponential, no jitter | GET, HEAD and OPTIONS only. Other methods return `validation.http_retry_unsafe`. `Retry-After` seconds are honoured, capped at `max`. |
| `version 1.1\|2\|3` | — | auto (ALPN h2 → http/1.1; plain http = 1.1) | `version 2` is strict. It returns `http.version_unavailable` if h2 is not negotiated. |
| `version prefer [3, 2]`, `[3, 1.1]`, `[3, 2, 1.1]`, `[2, 1.1]` | list | — | HTTP/3 may only come first and may not repeat. Other lists return `validation.http_version`. |
| `tls server_name V` / `tls ca_file P` / `tls cert_file P` + `tls key_file P` | — | platform verifier | Files are read through `allow_read`. `cert_file` and `key_file` must be given together (`validation.tls`). |
| `unix "PATH"` | path | — | HTTP over a Unix socket, authorized as `allow_unix connect PATH`. It cannot be combined with `auth`. |
| `auth PROFILE account "A"` | — | — | An origin-bound bearer credential. See [SYS-2026-0006](sys-2026-0006-oauth-and-credentials.md). |
| `max_body N` | bytes | 8 MiB | A buffered body over the limit returns `limit.http_body`. |
| `stream sse\|jsonl\|ndjson\|lines\|bytes` | scoped only | — | `with http` without it returns `validation.http_stream`. |

### Sockets (`with tcp "H:P"`, `with unix "PATH"`, `with websocket "ws(s)://…"`)

| Option | Applies to | Default | Notes / errors |
|---|---|---|---|
| `framing newline\|raw\|length32 [endian big\|little]\|delimiter "S" [max_frame N]` | tcp, unix | `raw` | Bad syntax returns `validation.framing`. |
| `max_frame N` | all | 8 MiB | A bigger frame returns `limit.frame`. For WebSocket it also bounds the message and frame size. |
| `timeout "D"` | all | request deadline | The connect budget and the scope deadline. `timeout.connect` and `timeout.receive` are returned when exceeded. |
| `tls …` | websocket (`wss`) | platform verifier | ALPN is `http/1.1`. |
| `tls …` | tcp, unix | — | Refused with **`unsupported.tcp_tls`** before any file read or dial. |
| `reconnect …` | all | — | Refused with **`unsupported.reconnect`**. |
| `resume …` | all | — | Accepted and ignored. |

### Processes (`command "BIN" … end`, `with command "BIN" as p … end`)

| Option | Default | Notes / errors |
|---|---|---|
| `args [..]` | `[]` | Argv elements, never parsed by a shell. |
| `env {K: V}` | empty (+ `PATH=/usr/bin:/bin`) | The host environment is never inherited. |
| `cwd "P"` | bundle root | Authorized as `allow_read stat`. |
| `stdin json\|text\|bytes V` | stdin is `/dev/null` | Encoded through `StdCodec`. |
| `timeout "D"` | request deadline | On expiry the adapter kills the process group, reaps the child and returns `timeout.process`. |
| `accept exit [..]` | `[0]` | Any other exit status returns `process.exit`, with `details.exit` and a stderr tail of up to 2048 bytes. |
| `decode stdout\|stderr json\|text\|bytes` | text if UTF-8, else bytes | — |
| `stream stdout lines\|jsonl\|bytes` | scoped only | `with command` without it returns `validation.command_stream`. |
| `interactive true` | false | Refused with `unsupported.interactive`. |

Other process refusals:

- The program must contain `/`. A bare name returns `validation.process_program`, because PATH lookup is not allowed.
- A shell (`sh`, `bash`, `zsh`, `dash`, `ksh`, `fish`, `csh`, `tcsh`, `cmd.exe` or `powershell`) called
  with a `-…c…` flag returns `unsupported.shell`.
- Captured output is limited to 8 MiB per pipe. Output past the limit returns `limit.process_output`.

### UDP (`with udp "H:P"`, `with udp bind "IP:P"`, `with udp multicast "GROUP:P"`)

| Option | Mode | Default | Notes / errors |
|---|---|---|---|
| `max_datagram N` | all | 8192 | Must be in 1..=65507, else `validation.udp_option`. |
| `timeout "D"` | all | request deadline | The default wait for receives that set no timeout of their own. |
| `bind "IP:P"` | multicast | `0.0.0.0:PORT` / `[::]:PORT` | Must be an IP literal in the same family as the group. |
| `interface "NAME"\|"IP"` | multicast | any | An unknown interface or a failed join returns `unsupported.udp_option`. |

### QUIC (`with quic "quic://H:P"|"https://H[:P]" as c`)

| Option | Default | Notes / errors |
|---|---|---|
| `alpn "ID"` | **required** | Missing returns `quic.alpn_required` (validation). A different ALPN negotiated by the peer returns `quic.alpn_mismatch` (tls). |
| `max_streams N` | 8 | Must be in 1..=1024 (`validation.quic_option`). Opening past the limit returns `limit.quic_streams`. |
| `datagrams true` | false | Datagram calls without it return `quic.datagrams_disabled`. A peer without DATAGRAM support returns `quic.datagrams_unavailable`. |
| `migration true` | false | Refused with **`unsupported.quic_migration`** before any packet. |
| `early_data true` | false | Refused with **`unsupported.quic_early_data`**. |
| `timeout "D"` | request deadline | The handshake and step bound. |
| `tls server_name\|ca_file\|cert_file\|key_file V` | URL host, platform verifier | TLS 1.3 only. `ca_file` replaces the roots. The files are `allow_read` intents. |
| child: `framing raw\|newline\|length32 [endian …]\|delimiter "S" [max_frame N]`, `timeout "D"` | raw, 8 MiB | `max_frame 0` returns `validation.quic_option`. |

The endpoint must be `quic://HOST:PORT` with an explicit port, or `https://HOST[:PORT]` (default port 443),
with no path or query. Anything else returns `validation.quic_endpoint`.

### gRPC (connector + call form)

```text
connector users grpc
    endpoint "https://users.example.com:443"     # https = TLS + h2; http = h2c (development)
    descriptor "./schemas/users.pb"              # FileDescriptorSet (protoc --include_imports)
    service "example.Users"                      # optional when the set has exactly one service
    tls server_name|ca_file|cert_file|key_file "V"
end
```

The descriptor is read and hashed (`sha256:…`) at bundle load. The load fails before anything dials if:

- the connector is missing `endpoint` or `descriptor` (`grpc.connector`)
- the descriptor file is missing (`not_found.descriptor`, exit 4)
- the descriptor set is invalid (`grpc.descriptor`)
- the service is unknown (`grpc.unknown_service`)
- a call's mode does not match the method (`grpc.mode`). `check_grpc_program` performs this check.

| Call option | Notes / errors |
|---|---|
| `message {…}` | Unary and server-streaming methods only. On client or bidi streaming methods it returns `grpc.mode`. It is validated against the input type before dialing. |
| `metadata "k" V` / `metadata "k-bin" bytes V` | Keys must use `[0-9a-z-_.]`. `grpc-*` keys, `content-type`, `te`, `host`, `user-agent`, `connection`, `transfer-encoding` and `upgrade` are rejected. Binary values need a `-bin` key and ASCII values must be printable. `authorization` together with `auth` is rejected. All failures return `grpc.metadata`. |
| `timeout "D"` | Capped at the remaining deadline. The connect timeout returns `timeout.connect`. |
| `auth PROFILE account A` | Adds `authorization: Bearer …` metadata after the method, origin and TLS permits pass. |

## Runtime Behaviour

### HTTP exchange (`transports.exchange_http`)

```text
 build_request ─▶ retry on non-replay-safe method? ──yes──▶ validation.http_retry_unsafe (exit 2)
      │  url must be http/https (validation.url); query appended; body encoded
      ▼
 ┌─▶ checked_target: authorize allow_network connect scheme://host:port/path   (or allow_unix connect PATH)
 │        resolve once → checked SocketAddr (private needs literal grant)
 │   send_versioned(client, wire, policy)
 │        │
 │   3xx + Location + redirect_limit>0 ?
 │        ├─ hops ≥ limit → limit.http_redirects
 │        ├─ new origin → drop authorization / proxy-authorization / cookie
 │        ├─ 303, or 301/302 after POST → GET, no body, no content-type
 │        └─ loop (each hop re-authorized) ─────────────────────────────────┐
 │   status ∈ retry.on_status and attempts left?                              │
 │        └─ wait(Retry-After ≤ max | backoff) → loop ──────────────────────┤
 └───────────────────────────────────────────────────────────────────────────┘
      ▼
 accepted? (accept list, default 2xx) ── no ─▶ http.status {status, method} (kind http, exit 5)
      ▼
 decode finite body (Codec) │ hand the live stream back for `with http … stream`
```

With `auth PROFILE account A`, `HttpEffects::exchange_authed` authorizes the resource origin before
acquiring a lease. A `401` then invalidates the lease. A replay-safe, non-streaming request is retried
once with a new lease.

### HTTP/3: `version 3` strict and `version prefer [3, …]` fallback

`HttpVersionPolicy::wants_h3()` routes the first attempt to `h3_client::send`. That attempt goes to the
same checked `SocketAddr`, over an ephemeral UDP socket, with ALPN `h3` only and `enable_early_data = false`.
Each failure is tagged `details.request_sent` (true or false). The **use case** alone decides whether to
fall back, never the client.

```text
 script          exchange_http            HyperClient / h3_client               peer 127.0.0.1:18453
   │  http get … version prefer [3, 1.1]
   │────────────────▶│ authorize https://127.0.0.1:18453/items → checked addr
   │                 │ send(wire{version: Http3})
   │                 │──────────────────────▶│ quinn connect (UDP, ALPN h3, 0-RTT off)
   │                 │                       │─────────── QUIC Initial ──────────────▶│ (no QUIC listener)
   │                 │                       │     … H3_HANDSHAKE_TIMEOUT = 3 s …      │
   │                 │◀── http.version_unavailable {request_sent:false, version:3} ────│
   │                 │ h3_fallback() = Http1  AND request_unsent(e)  ⇒ fall back
   │                 │ send(wire{version: Http1}) SAME checked addr
   │                 │──────────────────────▶│ TCP + TLS (ALPN http/1.1) ─────────────▶│
   │                 │                       │◀──────────── 200 {"items":[1,2,3]} ─────│
   │◀── {items:[1,2,3], version: 1.1}
```

```text
 failure point in h3_client::send                         code                        request_sent   prefer → fallback?
 ───────────────────────────────────────────────────────  ──────────────────────────  ────────────   ──────────────────
 http:// URL, Unix target, endpoint/socket creation       http.version_unavailable /   false          yes
                                                          connection.udp
 no QUIC answer within 3 s, handshake/ALPN refusal,       http.version_unavailable     false          yes
 h3 SETTINGS setup failure
 TLS alert other than no_application_protocol             tls.handshake {tls_alert}    false          yes
 TLS config / server name                                 tls.config                   false          yes
 send_request (HEADERS) / body / finish / recv_response   connection.http3             true           NO (never re-sent)
 response body read                                       connection.http_body         —              NO
```

`version 3` has no fallback (`h3_fallback() = None`), so the first error is returned as is. Verified with
the local fixture described in [Last Verified Version](#last-verified-version). The fixture is an HTTPS/1.1
server with no QUIC listener, reached with `tls ca_file "./ca.pem"`:

```text
$ rivet --file app.rivet request items.strict3         # version 3            → exit 5, 3.04 s
{"request_id":"req_010e9e11c5","trace_id":"tr_010e9e11c5","error":{"kind":"protocol","code":"http.version_unavailable","message":"HTTP/3 is not available: no QUIC answer from 127.0.0.1:18453 within 3000 ms","retryable":false,"effects":"none","source":{"file":"app.rivet","line":5,"column":5,"end_line":9,"end_column":8},"operation_id":"items.strict3","details":{"request_sent":false,"version":3}}}

$ rivet --file app.rivet request items.prefer          # version prefer [3, 1.1] → exit 0, 3.04 s
{"request_id":"req_0158a874ed","trace_id":"tr_0158a874ed","result":{"items":[1,2,3],"version":1.1},"data_count":0,"effects":"none"}

$ rivet --file app.rivet request h.badpref             # version prefer [2, 3]   → exit 2
{…,"error":{"kind":"validation","code":"validation.http_version","message":"expected `version 1.1|2|3` or `version prefer [3, 2]` (HTTP/3 may only come first; no repeats)",…}}
```

Request and trace IDs vary between runs. The fixture log shows exactly one `GET /items` for the
`prefer` run, which confirms that the HTTP/3 attempt sent no request bytes.

### Redirects and streams (verified)

The same fixture answers `/old` with `302 Location: /items`, and `/events` with two SSE events:

```text
$ rivet --file app.rivet request items.redirect_default   # accept status [302], no redirect line
{"request_id":"req_01a1842bcd","trace_id":"tr_01a1842bcd","result":{"status":302,"location":"/items"},"data_count":0,"effects":"none"}

$ rivet --file app.rivet request items.redirect_follow    # redirect follow limit 1
{"request_id":"req_019e136195","trace_id":"tr_019e136195","result":{"status":200,"body":{"items":[1,2,3]},"version":1.1},"data_count":0,"effects":"none"}

$ rivet --file app.rivet request items.events             # with http get … as stream / stream sse
{"request_id":"req_019b3daa3d","trace_id":"tr_019b3daa3d","result":[{"event":"tick","id":null,"data":{"n":1}},{"event":"tick","id":null,"data":{"n":2}}],"data_count":0,"effects":"none"}
```

### Stream decoding (`StreamDecoder`, `src/infra/codec.rs`)

```text
 ByteStream.next_chunk ──push──▶ buffer ──pop()──▶ item
   (pulled only when the loop asks for the next item: backpressure to the socket / pipe)

 sse    lines → fields event/data/id/retry; blank line dispatches {event (default "message"), id, data[, retry]}
        data joined by "\n", parsed as JSON when possible unless `decode text|bytes`; ":" comments ignored
 jsonl  one JSON value per non-blank line ("ndjson" is an alias)         parse.json on a bad line
 lines  one text item per LF (CRLF trimmed)                               parse.utf8 on bad UTF-8
 bytes  each chunk as bytes
 every  a line longer than 8 MiB (MAX_STREAM_ITEM) → limit.stream_item
        no chunk before the scope deadline → timeout.stream
        at EOF: final unterminated line / pending event is flushed; child streams then check exit (process.exit)
```

### Sockets (`transports.exchange_socket`)

```text
 SocketPlan ─▶ tls on tcp/unix? → unsupported.tcp_tls      reconnect? → unsupported.reconnect   (zero effects)
           ─▶ unix: authorize allow_unix connect PATH
              tcp:  authorize allow_network connect tcp://host:port   → checked_addr
              ws/wss: authorize allow_network connect ws(s)://host:port/path → checked_addr
           ─▶ Dialer.connect: TCP (nodelay) | UnixStream
                 tcp/unix → FramedConn{framing, max_frame}
                 ws/wss   → [TLS alpn http/1.1] → tungstenite upgrade (protocol.websocket_handshake on failure)
 scope:   send KIND V → encode_frame   receive KIND [timeout] → take_frame   for f in conn
          finish_send → TCP FIN / WS close frame     scope exit → shutdown (WS: drain close ≤ 2 s)
 errors:  connection.refused|timeout|failed · connection.io · connection.websocket · protocol.unexpected_eof
          (peer closed mid-frame) · limit.frame · timeout.connect · timeout.receive · cleanup.closed
```

Length32 frames carry a 4-byte length prefix in the configured byte order. Newline frames that are valid
UTF-8 come back as text. Delimiter frames split on the delimiter bytes. Raw frames return each read chunk
as it arrives.

### Process launch with sandbox (`transports.run_process` + `TokioRunner`)

```text
 script            run_process / confine_process          TokioRunner              sandbox_macos           child
   │ command "/bin/cat" args ["./data/public.json"]
   │───────────────────▶│ shell with -c?  → unsupported.shell
   │                    │ interactive?    → unsupported.interactive
   │                    │ bare name?      → validation.process_program
   │                    │ authorize allow_exec exec /bin/cat
   │                    │ cwd? authorize allow_read stat
   │                    │ policy.present ⇒ sandbox_spec(policy)
   │                    │   grants  allow_read→read, allow_write/delete→write, allow_exec→exec
   │                    │   deny    →deny_read / deny_write
   │                    │   `DIR/**`→DIR, `*`→/, exact path; other glob or narrowed `access`
   │                    │       → unsupported.sandbox_backend (before spawn)
   │                    │ encode stdin
   │                    │ run(plan) ──────────────▶│ command(): sandbox::command(spec, resolved, args)
   │                    │                          │────────────────────▶│ /usr/bin/sandbox-exec present?
   │                    │                          │                     │   no → unsupported.sandbox_backend
   │                    │                          │                     │ profile(n R, n W, n X, n DR, n DW)
   │                    │                          │◀── sandbox-exec -p PROFILE -D R0=… -D X0=… -- /bin/cat …
   │                    │                          │ env_clear; PATH=/usr/bin:/bin; env pairs; cwd=root
   │                    │                          │ stdin piped|null; stdout/stderr piped; process_group(0)
   │                    │                          │ kill_on_drop ─────────────────────────────────────────▶│ spawn
   │                    │                          │ drain stdout+stderr concurrently (≤ max_output each)
   │                    │                          │ timeout → killpg(SIGKILL) + kill + wait → timeout.process
   │                    │                          │◀────────────────────────────────────── exit status ────│
   │                    │ exit ∉ accept → process.exit {exit, stderr tail}
   │◀── {stdout, stderr, exit, duration_ms}
```

The platform backend is chosen at compile time in `process_adapter.rs`:

```text
 target_os = "macos"   → sandbox_macos        BACKEND "macos-seatbelt"          ACTIVE
 target_os = "linux"   → sandbox_linux        BACKEND "linux-landlock-seccomp"  BUILT, GATED (CERTIFIED = false)
 anything else         → sandbox_unsupported  BACKEND "none"                    always refuses
```

**macOS Seatbelt profile.** The profile is static text. It names only parameters (`R0`, `W0`, `X0`,
`DR0`, `DW0`), so paths can never inject SBPL. Paths are canonicalized; for example `/tmp` becomes
`/private/tmp`.

```text
(version 1)
(deny default)                                   ← no network (incl. loopback), no process-fork
(allow process-exec (subpath (param "X0")) …)    ← the program itself is always appended to X*
(allow signal (target same-sandbox))
(allow sysctl-read)
(allow file-read-metadata)
(allow file-read* (literal "/") (subpath "/usr") (subpath "/bin") (subpath "/System")
                  (subpath "/Library/Apple") (subpath "/private/var/db/timezone") (subpath "/dev")
                  (subpath (param "R0")) … (subpath (param "X0")) …)
(allow file-write* (literal "/dev/null") (subpath (param "W0")) …)
(allow file-ioctl (literal "/dev/null") (literal "/dev/tty"))
(deny file-read* (subpath (param "DR0")) …)       ← only when deny entries exist
(deny file-write* (subpath (param "DW0")) …)
```

**Linux Landlock + seccomp** (`sandbox_linux.rs`). This backend is implemented but gated. While
`CERTIFIED = false`, every sandboxed spawn fails with `unsupported.sandbox_backend` and the detail
"gated until the T-08 conformance suite passes on Linux CI (kernel >= 6.12, Landlock ABI 6)". The
certified code path would behave as follows:

- It builds a Landlock ABI V6 ruleset at `HardRequirement`. The ruleset handles all filesystem and network
  access and all scopes. The system read set is `/usr`, `/bin`, `/lib`, `/lib64`, `/etc/ld.so.cache` and `/dev`.
- It adds a seccomp filter that returns EPERM for:
  - `socket` in the INET, INET6, UNIX, NETLINK and PACKET families
  - `ptrace`, `process_vm_*`, `bpf` and `perf_event_open`
  - `io_uring_setup`
  - `mount`, `umount2`, `pivot_root`, `unshare` and `setns`
  - the `keyctl` family, `kexec_load` and `init_module`
- It applies both between fork and exec.
- It refuses any policy that has deny entries, because Landlock cannot express them.

Verified on macOS (Darwin 25.4.0), using a scratch bundle and the policy below:

```json
{"version": 1,
 "grants": [{"capability": "allow_exec", "targets": ["/bin/cat", "/usr/bin/curl", "/bin/sh", "/usr/bin/printf"]},
            {"capability": "allow_read", "targets": ["./data/**"]}],
 "deny":   [{"capability": "allow_read", "targets": ["./data/private/**"]}]}
```

```text
$ rivet --file app.rivet request p.cat           # /bin/cat ./data/public.json, decode stdout json   → exit 0
{"request_id":"req_0118cf4d15","trace_id":"tr_0118cf4d15","result":{"message":"public"},"data_count":0,"effects":"committed"}

$ rivet --file app.rivet request p.cat_private   # /bin/cat ./data/private/secret.json (deny wins in the kernel) → exit 5
{…,"error":{"kind":"process","code":"process.exit","message":"`/bin/cat` exited with status 1",…,"details":{"exit":1,"stderr":"cat: ./data/private/secret.json: Operation not permitted\n"}}}

$ rivet --file app.rivet request p.shell         # /bin/sh args ["-c", "echo hi"]                     → exit 5
{…,"error":{"kind":"unsupported","code":"unsupported.shell","message":"`/bin/sh` with -c evaluates a shell string; pass argv to the target binary instead",…}}

$ rivet --file app.rivet request p.bare          # command "cat"                                     → exit 2
{…,"error":{"kind":"validation","code":"validation.process_program","message":"`cat` is not a path; use an absolute path or a bundle-relative ./path (no PATH lookup)",…}}

$ rivet --file app.rivet request p.lines         # with command "/usr/bin/printf" … stream stdout lines → exit 0
{"request_id":"req_010fc74815","trace_id":"tr_010fc74815","result":["a","b"],"data_count":0,"effects":"none"}

$ rivet --file app.rivet --policy narrowed.json request p.cat   # allow_read ./data/** access ["read"]  → exit 5
{…,"error":{"kind":"unsupported","code":"unsupported.sandbox_backend","message":"the process sandbox cannot represent this policy exactly: allow_read ./data/** narrowed by access",…,"details":{"grant":"allow_read ./data/** narrowed by access"}}}

$ rivet --file app.rivet --policy glob.json request p.cat       # allow_read ./data/*.json            → exit 5
{…,"error":{"kind":"unsupported","code":"unsupported.sandbox_backend","message":"the process sandbox cannot represent this policy exactly: glob selector `./data/*.json`",…}}
```

A loopback-network check was also run. A Python listener was started on `127.0.0.1:18456`.
`/usr/bin/nc -z -w 2 127.0.0.1 18456` run through `command` (with `accept exit [0, 1]`) returned
`{"exit":1,"stderr":""}`, and the listener saw no connection from it. The same `nc` run outside Rivet
connected (`Connection to 127.0.0.1 port 18456 [tcp/*] succeeded!`).

`docs/demos/11-sandbox` contains no `command` statement; it exercises file grants. On this host it
returned `{"message":"public demo data"}` for `data.read` (exit 0). For `data.private` it returned
`permission.denied` with the message "allow_read read on ./data/private/secret.json denied: deny
allow_read ./data/private/**" (exit 3).

### UDP (`datagrams.exchange_datagrams`)

Every handle method becomes a one-step `DatagramPlan`. That plan runs through the use case before
`UdpSession` touches the socket.

```text
 step           authorization (before any socket / bind / join / packet)                  driver
 ────────────   ──────────────────────────────────────────────────────────────────────   ─────────────────────────────
 Open connected allow_network connect udp://PEER → checked peer_addr                      bind 0.0.0.0:0 (or [::]:0), connect
 Open bind      IP literal required; allow_listen bind udp://BIND                          bind BIND
 Open multicast group must be a multicast IP literal; bind IP literal, same family;        socket2: reuse addr/port,
                allow_network connect udp://GROUP                                          broadcast off, bind, join
                allow_listen  bind            udp://BIND                                   (interface by IP or name)
                allow_listen  multicast_join  udp://BIND
 Send           bind mode → udp.no_peer; len > max_datagram → udp.message_too_large       send to peer / group
 SendTo         connected mode → udp.connected; size guard;                               send_to checked addr
                allow_network connect udp://PEER per call (a received peer inherits nothing)
 Receive(From)  —                                                                         wait ≤ min(step, resource, deadline)
                                                                                           buffer max+1: n > max → udp.truncated
 Close          —                                                                         leave group, drop socket
```

A sequence diagram of a multicast receive, as verified on loopback:

```text
 script                 UdpAdapter            exchange_datagrams               PolicyBroker        UdpSession / OS
   │ with udp multicast "239.255.42.99:18452" as socket
   │   bind "0.0.0.0:18452"  interface "127.0.0.1"  max_datagram 1024
   │──────────────────────▶│ base_plan(Multicast) + steps [Open]
   │                       │────────────────────▶│ max_datagram ∈ 1..=65507
   │                       │                     │ group literal & multicast; bind literal; same family
   │                       │                     │ allow_network connect udp://239.255.42.99:18452 ─▶│ allowed
   │                       │                     │ allow_listen bind udp://0.0.0.0:18452 ───────────▶│ allowed
   │                       │                     │ allow_listen multicast_join udp://0.0.0.0:18452 ─▶│ allowed
   │                       │                     │ driver.exchange(plan) ────────────────────────────────────────▶│
   │                       │                     │                                     socket2 DGRAM, reuse, bind,
   │                       │                     │                                     join_multicast_v4(group, 127.0.0.1)
   │   message = socket.receive_from text timeout "3s"
   │──────────────────────▶│ step ReceiveFrom{3000} ─▶ (no intents) ─▶ recv_from ≤ 3 s ─────────────────────────────▶│
   │◀── {peer: "127.0.0.1:52110", data: "tick"}
   │ end ─────────────────▶│ close: step Close ─────────────────────────────▶ leave group, drop socket ─────────────▶│
```

Verified with a scratch bundle on ports 18450–18452 and Python fixtures:

```text
$ rivet --file app.rivet request telemetry.status       # no fixture listening on 127.0.0.1:18450 → exit 5
{"request_id":"req_019db5d83d","trace_id":"tr_019db5d83d","error":{"kind":"connection","code":"udp.receive_failed","message":"receive: Connection refused (os error 61)","retryable":false,"effects":"committed","source":{"file":"app.rivet","line":10,"column":9,"end_line":10,"end_column":48},"operation_id":"telemetry.status"}}

$ rivet --file app.rivet request telemetry.status       # fixture replies {"state":"ready"}      → exit 0
fixture got b'{"command":"status"}'
{"request_id":"req_017d0ac17d","trace_id":"tr_017d0ac17d","result":{"state":"ready"},"data_count":0,"effects":"committed"}

$ rivet --file app.rivet request telemetry.multicast    # sender loops "tick" to 239.255.42.99:18452 via 127.0.0.1 → exit 0
{"request_id":"req_01ea6aeecd","trace_id":"tr_01ea6aeecd","result":{"peer":"127.0.0.1:52110","data":"tick"},"data_count":0,"effects":"none"}

$ rivet --file app.rivet --policy nojoin.json request telemetry.multicast   # allow_listen … access ["bind"] → exit 3
{…,"error":{"kind":"permission","code":"permission.denied","message":"allow_listen multicast_join udp://0.0.0.0:18452 denied: grant allow_listen udp://0.0.0.0:18452 access [bind] does not include `multicast_join`",…,"details":{"capability":"allow_listen","access":"multicast_join","target":"udp://0.0.0.0:18452"}}}
```

The first run fails because the connected socket received an ICMP port-unreachable, which the OS
reports as `udp.receive_failed`. A silent peer would instead produce `timeout` (kind timeout, exit 6)
with `effects: unknown` on the error. The request envelope rolls effects up as `none` or `committed`.

**Manifest alignment (B3).** Since commit `829ca43` the I/O manifest (`rivet io`, `--check-policy`,
`policy generate`) lists a multicast site exactly as the table above authorizes it: `allow_network` connect
on the **group**, and `allow_listen` `bind` + `multicast_join` on the **bind address** (the `bind` option,
or the unspecified address on the group's port by default). The static verdict now matches the runtime for
every grant combination (`tests/conformance_udp.rs`, including the `docs/demos/08-udp` bundle).

### Native QUIC (`quic.exchange_quic`)

```text
 Connect   alpn empty → quic.alpn_required · max_streams ∉ 1..=1024 → validation.quic_option
           early_data → unsupported.quic_early_data · cert/key unpaired → validation.quic_option
           migration  → unsupported.quic_migration
           authorize allow_read read  for each tls ca_file / cert_file / key_file
           authorize allow_network connect SCHEME://HOST:PORT   (the logical origin; no raw udp grant involved)
           checked_addr (private → literal SCHEME://IP:PORT grant) → peer_addr
           QuicSession.connect: quinn client endpoint (ephemeral UDP), TLS 1.3, ALPN = [alpn], 0-RTT off,
             max_concurrent_bidi/uni = max_streams, datagram buffer only if datagrams
             handshake wait ≤ min(resource timeout, deadline) → timeout
             negotiated ALPN ≠ alpn → quic.alpn_mismatch;  datagrams w/o peer support → quic.datagrams_unavailable
 Open/Accept  stream budget (limit.quic_streams) · framing max_frame ≥ 1
 Send/Receive framed; one frame per receive, Null at FIN; frame > max_frame → quic.frame_too_large
              uni-accepted stream cannot send → quic.direction; reset → quic.stream_reset;
              STOP_SENDING → quic.stop_sending
 Datagrams    needs `datagrams true` (quic.datagrams_disabled); oversize → quic.datagram_too_large
 CloseStream  FIN if still open, stop reading   Close  connection close, wait_idle ≤ 1 s
```

Verified. There is no QUIC server on `127.0.0.1:18455`, and the policy grants `quic://127.0.0.1:18455`:

```text
$ rivet --file app.rivet request q.noalpn    → exit 2
{…,"error":{"kind":"validation","code":"quic.alpn_required","message":"`with quic` needs `alpn \"ID\"`; ALPN is required and must match the peer",…}}
$ rivet --file app.rivet request q.migrate   → exit 5
{…,"error":{"kind":"unsupported","code":"unsupported.quic_migration","message":"QUIC connection migration is not available: every new path would need its own policy approval before probing; use `migration false`",…}}
$ rivet --file app.rivet request q.silent    → exit 6   (timeout "1s")
{…,"error":{"kind":"timeout","code":"timeout","message":"the QUIC handshake did not complete within 1000 ms",…}}
```

### gRPC (`grpc.invoke_rpc`)

```text
 GrpcPlan ─▶ catalog.connector / method           grpc.unknown_connector / grpc.unknown_method
          ─▶ check_usage (one-shot ⇔ unary; message only on unary/server-stream)   grpc.mode
          ─▶ check_metadata                         grpc.metadata
          ─▶ auth w/o provider                      unsupported.auth
          ─▶ validate_input(ProtoJSON vs input type) grpc.invalid_message
          ─▶ parse_endpoint: https (TLS+h2) | http (h2c) | other → unsupported.transport
          ─▶ authorize allow_grpc call CONNECTOR/pkg.Service/Method
          ─▶ authorize allow_network connect scheme://host:port
          ─▶ authorize allow_read read  each tls file (https only)
          ─▶ resolve; private behind a name needs a literal grant; none left → dns.no_address / permission.denied
          ─▶ credentials.acquire (auth) → authorization: Bearer metadata
          ─▶ driver.invoke: first checked address that accepts TCP, TLS in Rivet's connector,
                            timeout = min(timeout, deadline), queues of 16 each way, 4 MiB message cap
          ─▶ unary / server-stream: send the one message, half-close
          ─▶ RpcCall: separate send/receive locks (bidi tasks progress independently)
```

Final status mapping (`status_error`):

| gRPC status | Rivet code | kind |
|---|---|---|
| CANCELLED (1) | `grpc.cancelled` | cancelled |
| INVALID_ARGUMENT (3) | `grpc.invalid_argument` | validation |
| DEADLINE_EXCEEDED (4) | `grpc.deadline_exceeded` | timeout |
| NOT_FOUND (5) | `grpc.not_found` | not_found |
| PERMISSION_DENIED (7) | `grpc.permission_denied` | permission |
| RESOURCE_EXHAUSTED (8) | `grpc.resource_exhausted` | limit |
| UNIMPLEMENTED (12) | `grpc.unimplemented` | unsupported |
| UNAUTHENTICATED (16) | `auth.login_required` | conflict |
| any other | `grpc.<status_name lower-case>` | protocol |

The error details keep the gRPC status, code, message (at most 512 characters) and `data_count`. The call
also enforces cardinality: `rpc.result` needs exactly one message and an OK status (`grpc.cardinality`).
Sending after a half-close or after the peer finished returns `grpc.input_closed`. Leaving the scope
cancels an unfinished call.

Metadata that scripts see is bounded to 64 entries of 1024 bytes each. `grpc-*` transport keys are
omitted. Keys named `authorization`, `proxy-authorization`, `cookie` or `set-cookie`, and keys that contain
`token`, `secret`, `password` or `api-key`, are redacted.

Verified on the scratch copy of `10-grpc` with its descriptor generated:

```text
$ rivet --file app.rivet --policy nogrpc.json request users.grpc_get --params '{"id":"42"}'   → exit 3
{…,"error":{"kind":"permission","code":"permission.denied","message":"allow_grpc call users/example.Users/GetUser denied: no grant for allow_grpc users/example.Users/GetUser",…,"details":{"capability":"allow_grpc","access":"call","target":"users/example.Users/GetUser"}}}

$ rivet --file local.rivet --policy local.json request users.grpc_get --params '{"id":"42"}'  → exit 5
  (endpoint "http://127.0.0.1:18457", nothing listening)
{…,"error":{"kind":"connection","code":"connection.refused","message":"gRPC connect to http://127.0.0.1:18457 failed: transport error: Connection refused (os error 61): Connection refused (os error 61)",…}}
```

### Error codes emitted, by adapter

This table was collected from the non-test code of each module. Exit codes follow the registry: 2 for
validation and syntax; 3 for permission; 4 for not_found; 5 for connection, protocol, tls, http, process,
limit and unsupported; 6 for timeout.

| Adapter | Codes |
|---|---|
| HTTP adapter + use case | `syntax.http`, `validation.http_option`, `validation.http_version`, `validation.http_stream`, `validation.http_request`, `validation.http_retry_unsafe`, `validation.url`, `validation.tls`, `validation.codec`, `unsupported.unix`, `unsupported.oauth`, `unsupported.iterate`, `unsupported.property`, `http.status`, `http.version_unavailable`, `limit.http_body`, `limit.http_redirects`, `protocol.http_location`, `connection.http`, `connection.http_body`, `connection.refused`, `connection.timeout`, `connection.failed`, `dns.resolve`, `tls.config`, `tls.handshake`, `timeout.http`, `timeout.stream`, `permission.denied` |
| HTTP/3 (`h3_client.rs`) | `http.version_unavailable`, `tls.handshake`, `tls.config`, `connection.udp`, `connection.http3`, `connection.http_body`, `validation.http_request` |
| Codecs / streams | `parse.json`, `parse.utf8`, `limit.stream_item`, `limit.frame`, `validation.codec`, `validation.frame_delimiter` |
| Sockets | `syntax.with`, `validation.endpoint`, `validation.url`, `validation.framing`, `validation.socket_option`, `validation.socket_send`, `validation.socket_receive`, `unsupported.tcp_tls`, `unsupported.reconnect`, `unsupported.unix`, `unsupported.method`, `protocol.websocket_handshake`, `protocol.unexpected_eof`, `connection.websocket`, `connection.io`, `timeout.connect`, `timeout.receive`, `cleanup.closed`, `parse.utf8` |
| Processes | `syntax.command`, `validation.command_option`, `validation.command_stream`, `validation.process_program`, `unsupported.shell`, `unsupported.interactive`, `unsupported.sandbox_backend`, `unsupported.process_duplex`, `not_found.program`, `process.not_executable`, `process.spawn`, `process.io`, `process.exit`, `limit.process_output`, `timeout.process` |
| UDP | `validation.udp_option`, `validation.udp_address`, `udp.no_peer`, `udp.connected`, `udp.message_too_large`, `udp.truncated`, `udp.socket_failed`, `udp.bind_failed`, `udp.connect_failed`, `udp.send_failed`, `udp.receive_failed`, `unsupported.udp_option`, `unsupported.method`, `dns.resolve`, `dns.no_address`, `timeout`, `cleanup.closed` |
| QUIC | `validation.quic_option`, `validation.quic_endpoint`, `quic.alpn_required`, `quic.alpn_mismatch`, `quic.datagrams_disabled`, `quic.datagrams_unavailable`, `quic.datagram_too_large`, `quic.direction`, `quic.frame_too_large`, `quic.stream`, `quic.stream_finished`, `quic.stream_reset`, `quic.stop_sending`, `quic.tls`, `quic.tls_config`, `quic.transport`, `quic.connection`, `quic.connect`, `quic.closed`, `quic.idle_timeout`, `quic.socket`, `tls.pem`, `framing.truncated`, `framing.delimiter_in_payload`, `limit.quic_streams`, `unsupported.quic_migration`, `unsupported.quic_early_data`, `unsupported.child`, `unsupported.method`, `unsupported.property`, `dns.resolve`, `dns.no_address`, `timeout`, `cleanup.closed` |
| gRPC | `grpc.connector`, `grpc.descriptor`, `grpc.unknown_service`, `grpc.unknown_connector`, `grpc.unknown_method`, `grpc.mode`, `grpc.form`, `grpc.option`, `grpc.metadata`, `grpc.endpoint`, `grpc.method`, `grpc.invalid_message`, `grpc.decode`, `grpc.send`, `grpc.input_closed`, `grpc.cardinality`, the status-mapped codes above, `not_found.descriptor`, `not_found.file`, `unsupported.transport`, `unsupported.auth`, `unsupported.method`, `unsupported.property`, `connection.refused`, `connection.failed`, `timeout.connect`, `tls.config`, `tls.handshake`, `dns.resolve`, `dns.no_address`, `cancelled.grpc` |

## Data and Storage

The adapters persist nothing. The following state lives only in memory, for as long as its owner lives:

- One HTTP connection per attempt. Its connection task is aborted when the body closes or drops.
- One QUIC endpoint and h3 driver per HTTP/3 exchange (`Owner` drop closes both).
- Scoped socket, UDP, QUIC and gRPC handles. They live exactly as long as their `with` block.
- Child processes. Each child is reaped on exit, deadline, cancellation or scope close. The adapter
  sends SIGTERM to the process group, waits a 1.5 s grace, then sends SIGKILL and waits.
- gRPC descriptor pools and the `GrpcCatalog`. They are loaded once per `Runtime` and pinned by
  `descriptor_hash`.

The size bounds are summarised here:

| Bound | Value | Source |
|---|---|---|
| HTTP buffered body | 8 MiB (`max_body`) | `http_adapter.rs` `DEFAULT_MAX_BODY` |
| Stream item (SSE event / line) | 8 MiB | `MAX_STREAM_ITEM` |
| Socket frame (tcp/unix/ws) | 8 MiB (`max_frame`) | `socket_adapter.rs` `DEFAULT_MAX_FRAME` |
| QUIC stream frame | 8 MiB (`max_frame`) | `transport.rs` `DEFAULT_MAX_FRAME` |
| MCP client message | 8 MiB | `mcp_client.rs` `MAX_MESSAGE` |
| UDP payload | 8192 default, 65507 max | `DEFAULT_MAX_DATAGRAM`, `MAX_UDP_PAYLOAD` |
| Process stdout / stderr | 8 MiB each; streaming stderr tail 64 KiB | `DEFAULT_MAX_OUTPUT` |
| gRPC message | 4 MiB; queue 16 per direction | `grpc_adapter.rs` `MAX_MESSAGE_BYTES` |

```text
  8 MiB default for every frame/body/item/output (PROP-2026-0001 default, G11)
    HTTP body ── max_body N ──┐        socket / QUIC frame ── max_frame N ──┐
    SSE / JSONL / line item ──┼── fixed                  process output ────┼── fixed
    MCP message ──────────────┘                          file read / chunk ─┘
  above the bound ─▶ limit.http_body | limit.stream_item | limit.frame | limit.process_output | limit.mcp_message
```
| HTTP/3 handshake + h3 setup | 3 s | `H3_HANDSHAKE_TIMEOUT` |

## Dependencies

| Crate | Version (Cargo.toml) | Used by |
|---|---|---|
| `hyper` / `hyper-util` | 1.11 / 0.1.21 | HTTP/1.1 and h2 connection-level client |
| `quinn` | 0.11.12 (`rustls-ring`) | HTTP/3 and native QUIC |
| `h3` / `h3-quinn` | =0.0.8 / =0.0.10 | HTTP/3 framing |
| `rustls` / `tokio-rustls` / `rustls-platform-verifier` | 0.23.45 / 0.26.5 / 0.7.1 | All TLS (ring provider only) |
| `tokio-tungstenite` | 0.29 | WebSocket client |
| `socket2` | 0.6.5 | UDP multicast socket options |
| `tonic` / `prost` / `prost-reflect` | 0.14.6 / 0.14.4 / 0.16.5 | gRPC channel, dynamic messages from the descriptor set |
| `url` | 2.5.8 | URL parsing and query encoding |
| `ring` | 0.17.14 | Multipart boundary CSPRNG |
| `libc` (unix) | 0.2 | `killpg`, `if_nametoindex` |
| `landlock` / `seccompiler` (linux only) | 0.4.7 / 0.5.0 | Gated Linux sandbox |
| `/usr/bin/sandbox-exec` (macOS system binary) | — | Seatbelt backend |

The crate choices are recorded in [ADR-0002](../../decisions/adr-0002-rust-crate-selection.md). The
sandbox backend contract is in [ADR-0003](../../decisions/adr-0003-process-sandbox-backends.md).

## Deployment

The adapters are compiled into the single `rivet` binary and into the `rivet` library crate. They are
registered for every `Runtime`, whether it is built by the CLI, by `rivet serve` or by an embedding host.
The adapters themselves have no feature flags. Platform differences are resolved at compile time:

- Process sandbox backend: macOS gets Seatbelt, Linux gets the gated Landlock + seccomp backend, and every
  other platform refuses.
- Unix sockets are available only on `cfg(unix)`. Other platforms return `unsupported.unix`.
- Multicast joins by interface name need `if_nametoindex`, which is available on Unix only.

No external protocol tooling is needed at runtime. The only exception is the gRPC descriptor set, which
the operator generates ahead of time with
`protoc --include_imports --descriptor_set_out=…`. Rivet never runs `protoc` itself.

## Security Boundaries

```text
                  ┌──────────────────────── policy.json (the only authority) ─────────────────────────┐
 script effect ──▶│ adapter parses → use case: static guards → authorize → resolve → private re-check │──▶ driver dials
                  └──────────────────────────────────────────────────────────────────────────────────┘    checked addr only
```

- **Deny before bytes.** Every permission check, and every refusal of an unsupported feature, happens before
  a socket is created, a packet is sent, a file is read or a child is spawned. The refused features are TCP
  TLS, reconnect, QUIC migration, 0-RTT, interactive processes, shell strings and unrepresentable sandbox
  policies. The verified denials above all report `"effects":"none"`.
- **No self-resolution.** Drivers receive a `SocketAddr` that the use case has already checked. HTTP/3 reuses
  the https grant's host:port over UDP; no raw UDP grant is involved.
- **DNS-rebinding guard.** A private address that a name resolves to needs its own literal grant. For
  example, `https://127.0.0.1:18459/items` without a grant was denied with "127.0.0.1 is a
  private/loopback/link-local address; grant it literally".
- **Redirects.** Redirects are off by default. When they are on, each hop is re-authorized. Credential
  headers are dropped whenever the origin changes. A `401` refresh retries only replay-safe requests.
- **Replay safety.** `retry` is allowed only for GET, HEAD and OPTIONS. The HTTP/3 fallback happens only when
  `request_sent: false`. QUIC early data is refused. UDP sends report local acceptance only.
- **TLS.** Certificate validation is always on, through the platform verifier or `ca_file` roots. Key
  material is never echoed in errors ("tls key_file is not a PEM private key"). QUIC uses TLS 1.3 only.
- **Processes.** Processes run from argv only. The environment is cleared. The process runs in its own process
  group, and the whole group is killed on deadline. When `policy.json` is present an OS sandbox is
  mandatory, and the child gets no network and no fork. A policy the sandbox cannot represent exactly is
  refused rather than widened.
- **gRPC.** Rivet refuses reserved metadata and redacts credential-like metadata in script views. The
  descriptor set is pinned by its hash.
- **Secrets.** Before any adapter runs, the interpreter checks every part of the effect form (URL, headers,
  body, query, options, child parts) and every handle send for a `secret` value (or a derived encoding); only
  a network destination whose `scheme://host:port` is one of the secret's bound origins may receive it. File,
  process, Unix-socket and pipe sinks never do (`permission.denied`, `details.secret`; SYS-2026-0002).

## Observability

- Every `authorize` call goes through the runtime's `TracedEvaluator`. That evaluator records the decision
  against the site's `effect_id` for `rivet trace show`. See
  [SYS-2026-0003](../components/sys-2026-0003-policy-broker-and-io-manifest.md).
- Errors carry `kind`, `code`, `message`, `retryable`, `effects`, the source span and structured `details`:
  - `details.status` and `details.method` for `http.status`
  - `details.request_sent` and `details.version` for HTTP/3
  - `details.exit` and `details.stderr` for `process.exit`
  - `details.backend` and `details.reason` for sandbox refusals
  - `details.capability`, `details.access` and `details.target` for denials
  - `details.peer` for `udp.truncated`
- `response.version` reports the protocol that was actually used: 3, 2, 1.1 or 1.0.
- `rivet io` and `rivet io --check-policy` show every adapter site statically without doing any I/O.
- The adapters write no payload logs.

## Known Limitations

From the [manual's Known Limitations](../../manuals/man-2026-0001-rivet-manual.md#known-limitations):

- There is no connection pooling. Every HTTP attempt, HTTP/3 exchange and gRPC call dials a new connection.
- There is no Alt-Svc discovery. HTTP/3 is used only when the script asks for it with `version 3` or
  `version prefer [3, …]`. A server that has no QUIC listener adds the 3 s handshake bound before a
  `prefer` fallback.
- The process sandbox is active on macOS only. The Linux backend is built but gated
  (`CERTIFIED = false`) until it is verified on a kernel of 6.12 or later. Windows and other platforms always
  refuse sandboxed spawns (`unsupported.sandbox_backend`). Because `policy.json` makes the sandbox mandatory,
  `command` and MCP stdio connectors cannot run on those platforms under a policy.
- Stage C forms are refused: TLS on raw TCP or Unix sockets (`unsupported.tcp_tls`), socket `reconnect`
  (`unsupported.reconnect`), interactive processes (`unsupported.interactive`), named pipes
  (`unsupported.adapter`). `resume` is accepted and ignored.

Behaviour by design: QUIC connection migration and 0-RTT are refused (the client never rebinds its UDP
socket); the Seatbelt backend expresses only `DIR/**`, exact paths and `*` (not grants narrowed by `access`,
or other globs) and the Linux backend also cannot express deny entries; gRPC has no compression and no server
reflection, and the descriptor must be generated before the bundle loads.

## Last Verified Version

`0.1.0-dev (commit 829ca43)`. First verified at `f40d4aa` with `target/debug/rivet` on macOS (Darwin 25.4.0)
on 2026-09-28; the fix-batch changes (8 MiB defaults, multicast manifest alignment, secret sinks) were
re-checked at `829ca43` against the source constants and `tests/conformance_udp.rs`,
`tests/conformance_errors_limits_dag.rs` and the secret-taint tests; the table below is the original run.
No external network was used. All fixtures ran on 127.0.0.1 ports 18450–18459 from a scratchpad directory,
and the demo folders were not modified.

| Area | What was run | Result |
|---|---|---|
| Demos | `check` on `docs/demos/03-http`, `08-udp`, `09-quic` and `11-sandbox`; `io` and `io --check-policy` on the same four; `describe items.http3` on `09-quic` | `ok` for each. Manifests as shown. |
| gRPC demo | `check` on `10-grpc` as shipped | `not_found.descriptor`, exit 4 |
| gRPC demo | `check`, `io`, a denied call and an unreachable `http://127.0.0.1:18457` on a scratch copy with `protoc`-generated `users.pb` | as shown |
| HTTP/1.1 + HTTP/3 | HTTPS fixture with a test CA on 18453 (no QUIC listener): `version 3`, `version prefer [3, 1.1]`, `version prefer [2, 3]`, 302 without and with `redirect follow limit 1`, SSE stream, private-address denial | as shown |
| QUIC | missing ALPN, `migration true`, a silent peer on 18455 | as shown |
| UDP | connected unicast against a Python fixture on 18450; multicast receive on `239.255.42.99:18452` over loopback; `multicast_join` denied by `access ["bind"]` | as shown |
| Processes + Seatbelt | granted and denied reads, `sh -c`, a bare name, a line stream, narrowed and globbed policies, loopback `nc` blocked; `11-sandbox` `data.read` and `data.private` | as shown |

The following were not verified against a live peer: HTTP/2 negotiation, WebSocket and TCP/Unix framing,
a real HTTP/3 or QUIC server, all four gRPC modes against a server, and the Linux sandbox (gated). These
paths are covered by the conformance suites (`tests/conformance_http3.rs`, `conformance_quic.rs`,
`conformance_udp.rs`, `conformance_grpc.rs`, `conformance_streams.rs`, `conformance_resources.rs`).

## Related Documents

- [PROP-2026-0001 Rivet runtime proposal](../../proposals/implemented/prop-2026-0001-rivet-runtime.md)
- [PLAN-2026-0001 v0.1.0 implementation and release plan](../../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md)
- [ADR-0002 Rust crate selection](../../decisions/adr-0002-rust-crate-selection.md)
- [ADR-0003 Process sandbox backends](../../decisions/adr-0003-process-sandbox-backends.md)
- [REF-2026-0002 Language and usage reference](../../references/ref-2026-0002-language-and-usage.md)
- [SYS-2026-0002 Execution scopes and DAG](../runtime/sys-2026-0002-execution-scopes-and-dag.md)
- [SYS-2026-0003 Policy broker and I/O manifest](../components/sys-2026-0003-policy-broker-and-io-manifest.md)
- [SYS-2026-0004 Surfaces and serve](../components/sys-2026-0004-surfaces-and-serve.md)
- [SYS-2026-0006 OAuth and credentials](sys-2026-0006-oauth-and-credentials.md)
- [SYS-2026-0008 policy.json reference](../configuration/sys-2026-0008-policy-json-reference.md)
- [SYS-2026-0009 MCP client connectors](sys-2026-0009-mcp-client-connectors.md)
- [Demos](../../demos/README.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial current-state document (PLAN-2026-0001 D-19). |
| 2 | 2026-09-28 | Claude | TASK-092 drift fix for the fix batch (829ca43): 8 MiB defaults for HTTP body, stream items, socket and QUIC frames, process output and MCP messages; multicast manifest now mirrors the runtime (discrepancy removed); secret sink rule; limitations reduced to the current ones. |
