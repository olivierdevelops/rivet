---
document_id: RES-2026-0002
title: "Rust crate feasibility for the Rivet runtime"
document_type: research
status: completed
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, execution, files, connectors, policy, auth, datagrams, quic, grpc, serve, transports, cli, http, ws, mcp, library]
affected_versions:
  from: not-applicable
  to: "0.1.0"
applicable_environments: [development]
audience: [maintainers, implementers, reviewers]
scope: Select, version-pin and prototype one crate (or a hand-rolled design) for every third-party need of PROP-2026-0001, with licence and MSRV checks against Rust 1.90.
reason: PLAN-2026-0001 TASK-009 (R12, R15–R19 / C-08, C-10–C-14) — the proposal's Environment and Feasibility table marks the HTTP listener, MCP, OAuth, UDP/QUIC/HTTP3 and dynamic gRPC items as "experiment needed".
related_documents: [PROP-2026-0001, PLAN-2026-0001, ADR-0001, ADR-0002, RES-2026-0001, RES-2026-0003]
supersedes: null
superseded_by: null
tags: [rivet, rust, crates, feasibility, licence, msrv, prototype]
confidentiality: internal
review_cycle: on-change
next_review_date: 2026-10-28
---

# Rust crate feasibility for the Rivet runtime

> **Status:** Completed — all 20 runtime probes PASS; the proposed dependency block builds on macOS, Linux and Windows
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** not-applicable → 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** language, execution, files, connectors, policy, auth, datagrams, quic, grpc, serve, transports, cli, http, ws, mcp, library

## Summary

Every third-party need of [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md) has a crate
that is permissively licensed and builds on Rust 1.90. The one exception is OAuth, which is hand-rolled on
purpose (see the note under the table). A single scratch prototype ran 20 live probes against local fixtures,
and all 20 passed:

- one axum listener serving REST, HTTP/2, SSE, WebSocket and MCP Streamable HTTP;
- a hyper client that dials only a broker-checked IP and never follows redirects;
- dynamic gRPC from a `FileDescriptorSet` with ProtoJSON;
- QUIC streams and DATAGRAM, and HTTP/3;
- MCP over stdio in both directions;
- cap-std no-follow confinement;
- UDP multicast;
- argv-only processes;
- Capy parsing.

The exact dependency block proposed for `Cargo.toml` (TASK-014) was compile-checked on its own:

- `cargo check` on aarch64-apple-darwin;
- full `cargo zigbuild` for x86_64-unknown-linux-gnu and x86_64-pc-windows-gnu.

The graph resolves to 327 crates, with no OpenSSL and no aws-lc.

```text
  20 live probes ─────────────────────────────── 20 PASS / 0 FAIL        (macOS 26.4.1, rustc 1.90.0)
  proposed Rivet dependency block ── macOS check ✔   Linux build ✔   Windows build ✔
  licences (393 packages, full graph) ─ all permissive (MIT / Apache-2.0 / BSD / ISC / Zlib / MIT-0 / BSL-1.0 /
                                        Unicode-3.0 / CDLA-Permissive-2.0); no copyleft-only crate
  MSRV ceiling of the chosen set ──────── 1.88 (tonic, rmcp, globset, keyring stores) ≤ 1.90 ✔
```

The decision is recorded in [ADR-0002](../decisions/adr-0002-rust-crate-selection.md).

## Question

For each need, which crate at which version:
- has a licence compatible with shipping Rivet under MIT;
- builds on MSRV ≤ 1.90;
- demonstrably supports the Rivet-specific constraints of the proposal:
  - bind a *pre-resolved, checked* IP (defeats DNS rebinding);
  - never follow redirects implicitly;
  - one listener for every serve surface;
  - descriptor-driven gRPC with ProtoJSON;
  - MCP protocol `2025-11-25`;
  - no-follow confined file handles;
  - argv-only processes?

## Method

```text
 1. crates.io API (curl) ──► latest stable version, licence, rust-version, last release, downloads
 2. cargo info          ──► feature flags (to avoid aws-lc, openssl, remote $ref fetching, default TLS)
 3. scratch workspace   ──► every candidate as a dependency, edition 2024, rust-version 1.90
    (…/scratchpad/research/crates-proto — never in the repository)
 4. cargo check + cargo run ──► 20 probes, each prints PASS/FAIL with evidence (below)
 5. cargo zigbuild --target x86_64-unknown-linux-gnu | x86_64-pc-windows-gnu
 6. cargo deny list -l license ──► licence of every package in the graph
 7. separate crate with ONLY the proposed Rivet block (…/scratchpad/research/rivet-deps) ──► 3 targets
```

Environment:
- macOS 26.4.1 (Darwin 25.4.0, arm64);
- `rustc 1.90.0 (1159e78c4 2025-09-14)` and `cargo 1.90.0`;
- `libprotoc 29.3`;
- cargo-zigbuild with zig for the cross builds;
- cargo-deny 0.20.2.

The crates.io data was retrieved on 2026-09-28.

### Probe topology

```text
                         ┌───────────────── scratch process (tokio multi-thread) ─────────────────┐
                         │                                                                          │
   broker stand-in       │   lookup_host("localhost") ─► check IP (private? granted?) ─► SocketAddr │
   resolve_checked() ────┼──────────────────────────────────────────────┐                           │
                         │                                              ▼                           │
                         │   hyper::client::conn::http1/http2 ── TcpStream::connect(checked addr)   │
                         │   tokio-tungstenite client_async   ── TcpStream::connect(checked addr)   │
                         │   tonic Channel connect_with_connector(checked addr)                     │
                         │                                              │                           │
                         │   ┌──────────── ONE axum listener 127.0.0.1:0 ▼ ────────────┐            │
                         │   │ POST /v1/request (h1 + h2)  GET /v1/sse  GET /v1/ws      │            │
                         │   │ GET /redirect (307 → 169.254.169.254)  /mcp (rmcp tower) │            │
                         │   └──────────────────────────────────────────────────────────┘            │
                         │   tonic-health fixture ◄── dynamic gRPC (prost-reflect codec)            │
                         │   quinn server (rcgen cert) ◄── quinn client: bidi + DATAGRAM            │
                         │   h3 server over h3-quinn   ◄── h3 client                                 │
                         │   child: crates-proto --mcp-stdio ◄── rmcp client (TokioChildProcess)    │
                         └──────────────────────────────────────────────────────────────────────────┘
```

## Results

### Need → crate table

MSRV is the crate's declared `rust-version` (no declaration means none is published, and the crate built on
1.90). "Probe" is the live prototype result, and each entry is explained under [Probe evidence](#probe-evidence).

| # | Need (proposal link) | Crate(s) chosen | Version | Licence | MSRV | Probe | Risks / notes |
|---|---|---|---|---|---|---|---|
| 1 | Capy parser (R1, Increment 1) | `capy-core` git rev `84f984c6…` | 0.22.0 | MIT in `Cargo.toml`; upstream `LICENSE` text is source-available. Cleared by the owner (ADR-0001, G-LIC) | 1.74 | PASS | Git dependency (not crates.io); only dependency is `regex`. Spike results in [RES-2026-0001](res-2026-0001-capy-grammar-spike.md) |
| 2 | Async runtime | `tokio` (+ `tokio-util`, `tokio-stream`, `futures-util`, `bytes`) | 1.53.1 | MIT | 1.71 | PASS (all probes) | None |
| 3 | HTTP server: REST, SSE, polling, WS, MCP on one socket (R25, Increment 17) | `axum` on `hyper` 1 + `hyper-util` (`server-auto`) | 0.8.9 / 1.11.1 / 0.1.21 | MIT | 1.80 / 1.63 / 1.85 | PASS | TLS/mTLS listener uses a hand-written `tokio-rustls` accept loop around the Router (axum::serve is plaintext). Desk-verified, not probed |
| 4 | HTTP/1.1 + HTTP/2 client with checked-IP binding and no implicit redirects (R11, R13, Increment 5) | `hyper` 1 **connection-level API** (`client::conn::http1/http2`) + `hyper-util::rt` | 1.11.1 / 0.1.21 | MIT | 1.63 / 1.85 | PASS | Rivet owns the connection pool (one per origin+checked IP). `reqwest` 0.13.5 evaluated and rejected, see [Alternatives](#alternatives-evaluated) |
| 5 | WebSocket client + server | client: `tokio-tungstenite`; server: `axum` `ws` | 0.29.0 (matches axum's) | MIT | 1.85 | PASS | 0.30.0 exists. Stay on 0.29 until axum moves, to avoid two `tungstenite` copies |
| 6 | QUIC v1 streams + DATAGRAM (R17, Increment 11) | `quinn` (`rustls-ring`, `runtime-tokio`, no default features) | 0.11.12 | MIT OR Apache-2.0 | 1.85 | PASS | Server migration disabled via `ServerConfig::migration(false)`. The client never calls `rebind`, and `enable_early_data = false` keeps 0-RTT off. A per-path policy hook for `migration true` needs design (TASK for R17) |
| 7 | HTTP/3 (R18) | `h3` + `h3-quinn` (`datagram`) | =0.0.8 / =0.0.10 | MIT | 1.70 | PASS | **Pre-1.0 (0.0.x); last release 2025-05-06.** Pin exact versions. Isolated behind the HTTP port so it can be swapped |
| 8 | Dynamic gRPC from a `FileDescriptorSet`, ProtoJSON, four modes (R19, Increment 13) | `tonic` (`channel`,`codegen` only) + `prost` + `prost-reflect` (`serde`) with a ~40-line custom `Codec` over `DynamicMessage` | 0.14.6 / 0.14.4 / 0.16.5 | MIT / Apache-2.0 / MIT OR Apache-2.0 | 1.88 / 1.85 / 1.82 | PASS | `tonic-prost` and generated code are not needed. TLS happens inside Rivet's connector (ALPN `h2`), not in tonic's `tls-*` features |
| 9 | OAuth 2.0: client credentials, code+PKCE, device, refresh (R16, Increment 9) | **Hand-rolled** over the broker HTTP client, using `serde`, `url`, `sha2`, `base64` | — | — | — | PASS (PKCE oracle) | `oauth2` 5.0.0 evaluated: PKCE output matches the hand-rolled S256. Rejected because device polling is an internal loop (see note) |
| 10 | TLS client/server, mTLS, trust | `rustls` (`ring`) + `tokio-rustls` + `rustls-platform-verifier` + `rustls-pki-types` | 0.23.45 / 0.26.5 / 0.7.1 / 1.15.1 | Apache-2.0 OR ISC OR MIT; MIT OR Apache-2.0 | 1.71 / 1.71 / 1.85 / — | PASS (live, example.com) | One crypto provider (`ring`) across rustls, quinn and tokio-rustls avoids aws-lc's cmake/NASM needs on Windows. `webpki-roots` (CDLA-Permissive-2.0) is not selected, only noted as a container fallback |
| 11 | JSON Schema validation (R23, outputs; MCP schemas) | `jsonschema` (`default-features = false`) | 0.58.1 | MIT | 1.85 | PASS | Defaults pull `reqwest` + aws-lc and fetch remote `$ref`. With defaults off, remote `$ref` is refused (probe) |
| 12 | JSON | `serde` + `serde_json` | 1.0.229 / 1.0.151 | MIT OR Apache-2.0 | 1.56 / 1.71 | PASS | None |
| 13 | Confined file handles, no-follow (R5, R11, Increment 4) | `cap-std` + `cap-fs-ext` | 4.0.3 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | none declared | PASS | Uses `openat2(RESOLVE_BENEATH)` on Linux internally. `openat2` crate rejected (last release 2021). Conditional-update guard remains `unsupported.conditional_update` per proposal |
| 14 | UDP unicast/bind/multicast (R15, Increment 10) | `tokio::net::UdpSocket` + `socket2` (`all`) | 1.53.1 / 0.6.5 | MIT / MIT OR Apache-2.0 | 1.70 | PASS | Broadcast explicitly off (`set_broadcast(false)`) |
| 15 | Processes, argv only (R12) | `tokio::process` | 1.53.1 | MIT | 1.71 | PASS | Sandboxing is a separate question, answered in [RES-2026-0003](res-2026-0003-process-sandbox-backends.md) |
| 16 | MCP server + client, protocol `2025-11-25`, stdio + Streamable HTTP (R7, R20, Increments 6, 12, 17) | `rmcp` (official SDK) | ~3.5.0 | Apache-2.0 | 1.88 | PASS (HTTP + stdio) | Fast-moving (3.x; its `LATEST` is `2026-07-28`, which has no `initialize`). Rivet **must** override `supported_protocol_versions` to `[2025-11-25]` and set `allowed_hosts` for non-loopback binds. Pin `~3.5` |
| 17 | CLI parsing | `clap` (`derive`) | 4.6.7 | MIT OR Apache-2.0 | 1.85 | PASS | None |
| 18 | Diagnostics | `tracing` + `tracing-subscriber` (stderr writer) | 0.1.44 / 0.3.23 | MIT | 1.65 | PASS | stdout is reserved for results and MCP stdio |
| 19 | Token hashing, PKCE | `sha2` | 0.11.0 | MIT OR Apache-2.0 | 1.85 | PASS | Two `sha2`/`digest` generations exist in the graph (0.10 via deps). Harmless |
| 20 | Grant/operation patterns | `globset` | 0.4.20 | Unlicense OR MIT | 1.88 | PASS | None |
| 21 | URLs, IDNA, segment-safe interpolation | `url` + `idna` | 2.5.8 / 1.1.0 | MIT OR Apache-2.0 | 1.63 / 1.57 | PASS | `path_segments_mut().push()` percent-encodes `../` and `?` in one segment |
| 22 | Secure credential storage (`store keychain`, Increment 9) | `keyring-core` + per-OS store: `apple-native-keyring-store` (keychain), `windows-native-keyring-store`, `zbus-secret-service-keyring-store` (Linux) | 1.0.0 / 1.0.2 / 1.1.0 / 1.0.1 | MIT OR Apache-2.0 | 1.85 / 1.85 / 1.88 / 1.88 | PASS (mock round trip; macOS store constructed) | **No compare-and-swap.** Rivet adds versioning and locking itself (generation in payload plus a lock). The umbrella `keyring` 4.2 crate's own docs tell applications to use `keyring-core`. Real keychain writes were not exercised (they can prompt) |
| 23 | Linux sandbox primitives | `landlock` + `seccompiler` + `libc` | 0.4.7 / 0.5.0 / 0.2 | MIT OR Apache-2.0 / Apache-2.0 OR BSD-3-Clause | 1.71 / none | build-only | See RES-2026-0003 |
| 24 | Windows sandbox primitives | `windows-sys` | 0.61.2 | MIT OR Apache-2.0 | 1.71 | build-only | See RES-2026-0003 |

> **Why OAuth is hand-rolled.** `oauth2` 5.0.0's device flow (`DeviceAccessTokenRequest::request_async(client,
> sleep_fn, timeout)`) runs its own polling loop and keeps the interval internally. PROP-2026-0001 needs two
> things it cannot express:
>
> - `rivet.auth.complete {wait:true}` returns `{"state":"pending"}` at the caller's deadline;
> - the `slow_down` interval increase must persist across later `complete` calls in the transaction record.
>
> Code+PKCE and client credentials would fit, but a split implementation (crate for two grants, custom code for
> the third) gives no net saving. The token endpoint wire format (RFC 6749 §4.1.3/§4.4/§6, RFC 7636, RFC 8628
> §3.4–3.5) is a few form posts and one JSON response type. Every request also has to go through the broker's
> checked-IP client anyway.

### Probe evidence

The output below is verbatim from `crates-proto --online`. The scratch temp paths are shortened.

```text
PASS capy-core parse              Library::parse clean=1 bad_diagnostics=1 (recovering parse with spans)
PASS serde_json+jsonschema        draft 2020-12 ok, 2 errors reported, remote $ref refused=true
PASS sha2/globset/url/idna        sha256=930bbdc51b6a.. idna=xn--bcher-kva.example segment-escape=/users/..%2Fadmin%3Fx
PASS oauth2 PKCE (evaluated)      authorize URL + PKCE S256 match hand-rolled sha2+base64; device URL settable
PASS keyring-core store           apple keychain store constructed; mock Entry round trip ok (no CAS primitive in API)
PASS tokio::process argv          metacharacters passed literally as one argv element; env_clear + kill_on_drop
PASS udp multicast socket2        joined 239.255.42.99 on lo, looped 5 bytes from 127.0.0.1:54398; leave on drop
PASS cap-std no-follow            create_new twice -> true, '../' escape refused=true, symlink-outside refused=true,
                                  symlink-dir-hop refused=true, follow(No) refused=true, nlink=1
PASS hyper h1 + axum REST         200 OK result=5; private-range check without grant -> Some("127.0.0.1 denied: private range")
PASS redirect not followed        307 Temporary Redirect Location=Some("http://169.254.169.254/latest/meta-data")
                                  returned to broker for re-check
PASS hyper h2 (same listener)     HTTP/2.0 body={"result":42}
PASS axum SSE                     200 OK content-type=Some("text/event-stream") events=3
PASS axum ws + tokio-tungstenite  proto=Some("rivet.v1") reply=echo:{"type":"request","ref":"c1"}
PASS rmcp Streamable HTTP /mcp    init 2025-11-25 ok, session id issued, initialized=202 Accepted, tools/list has
                                  demo.add, unknown version: server counter-offers 2025-11-25, foreign Host -> 403 Forbidden
PASS reqwest (evaluated)          307 Temporary Redirect (redirect not followed; but pool/resolver are per-Client,
                                  not per-attempt)
PASS rmcp stdio client+server     negotiated 2025-11-25, tools=["demo.add"], child reaped on cancel
PASS grpc dynamic unary           Check client_stream=false server_stream=false; Watch server_stream=true; ProtoJSON
                                  reply {"status":"SERVING"}; unknown JSON field rejected=true; unknown service -> Some(NotFound)
PASS quic streams+datagram        alpn=Some("rivet-rpc/1") bidi echo 19 bytes, datagram max=Some(1288) echo=b"dg",
                                  migration off, 0-RTT off
PASS http3 h3+h3-quinn            HTTP/3.0 200 OK body={"path":"/items"}
PASS rustls platform-verifier     verified example.com via checked IP 172.66.147.243:443, proto=Some(TLSv1_3);
                                  wrong SNI name rejected=true
```

The proposal's open feasibility items and what the probes settle:

| Proposal item (Environment and Feasibility) | Before | Evidence | After |
|---|---|---|---|
| Unified serve listener: WS upgrade + MCP Streamable HTTP routing on the chosen HTTP crate | experiment needed | REST (h1+h2), SSE, WS (`rivet.v1`) and `/mcp` (rmcp `nest_service`) all on one axum listener | **proven** |
| Remote MCP: negotiated protocol | experiment needed | rmcp client ↔ server over stdio negotiated `2025-11-25`, and an unknown version got a counter-offer. Session cancellation and schema-change tests remain for T-06 | **partly proven** |
| OAuth provider and credential backend | experiment needed | PKCE verified; keyring-core has no CAS. Provider fixtures and cross-process rotation still owed | experiment needed (narrowed) |
| UDP/QUIC/HTTP3 in selected crates | experiment needed | multicast join/loop; QUIC ALPN, bidi FIN, DATAGRAM; migration and 0-RTT off; HTTP/3 GET. Other OSes: build only | **proven on macOS** |
| gRPC dynamic descriptors | experiment needed | unary via `FileDescriptorSet` + `DynamicMessage` + custom codec; ProtoJSON strict; status mapping. Streaming modes use the same `Grpc::{server,client}_streaming/streaming` API with the same codec, not probed | **proven (unary)** |
| Rust embedding on Linux/macOS/Windows | experiment needed | Full dependency block builds for all three targets | **proven (build)** |

### DNS-rebinding and redirect design this enables

```text
 script effect ─► broker: resolve name ─► every A/AAAA checked (deny_private_ranges, grants) ─► ONE SocketAddr
                                                                                                    │
      hyper conn / tungstenite / tonic connector / quinn.connect(addr, server_name) ◄───────────────┘
                     │                       (TLS ServerName = the original host, verified by rustls)
                     ▼
               response 3xx ──► returned to Rivet's HTTP adapter (hyper never follows) ──► new effect
                                                    attempt: same broker path, credentials stripped on
                                                    origin change (Increment 5)
```

## Alternatives evaluated

| Candidate | Result | Why not selected |
|---|---|---|
| `reqwest` 0.13.5 (`rustls-no-provider`, `http2`) | Built; `.resolve()` + `redirect::Policy::none()` worked | DNS overrides and the connection pool belong to the `Client`, not to each attempt. The default TLS is aws-lc and the defaults include `system-proxy`, where proxy destinations bypass the broker's view. HTTP/3 needs the `reqwest_unstable` cfg. hyper's connection API gives the broker full control at the same cost |
| `oauth2` 5.0.0 (no default features) | Built; PKCE matched | Internal device-poll loop (see the note under the table). Defaults pull `reqwest` |
| `keyring` 4.2.0 umbrella (`v1`) | Resolved | Its own docs direct applications to `keyring-core` plus chosen stores; `v1` links every store |
| `openat2` 0.1.2 | Not built | Last release 2021-06; `cap-std` already uses `openat2` on Linux; `rustix` 1.1.5 exposes it if ever needed |
| Hand-rolled MCP JSON-RPC | Not built | rmcp passed stdio + Streamable HTTP + version pinning + Host validation. Hand-rolling stays the fallback if rmcp churn blocks a contract requirement |
| `jsonschema` defaults | Resolved | Remote `$ref` fetching (network I/O outside the broker) and aws-lc |
| `quinn` defaults | Resolved | `platform-verifier` feature duplicates Rivet's own verifier wiring; defaults are harmless but not minimal |
| `tonic` `transport` + `tls-*` features | Resolved | Server side and tonic's TLS are unnecessary; the connector does rustls with the checked IP |

## Proposed dependency block (for TASK-014)

This exact block compiled on its own in a separate scratch crate (`rivet-deps`):
- `cargo check` passes on aarch64-apple-darwin;
- `cargo zigbuild` passes for x86_64-unknown-linux-gnu and x86_64-pc-windows-gnu.

`cargo tree -i aws-lc-rs` and `cargo tree -i openssl-sys` match nothing.

```toml
[dependencies]
# language
capy-core = { git = "https://github.com/olivierdevelops/capy", rev = "84f984c64e0811ef2bfff7835167d6630422ecaa" }
# async runtime and plumbing
tokio = { version = "1.53", features = ["rt-multi-thread", "macros", "net", "io-util", "io-std", "process", "fs", "time", "sync", "signal"] }
tokio-util = { version = "0.7.19", features = ["codec", "rt"] }
tokio-stream = "0.1.19"
futures-util = { version = "0.3.34", default-features = false, features = ["std", "sink"] }
bytes = "1.12"
# data, schema, hashing, matching, URLs
serde = { version = "1.0.229", features = ["derive"] }
serde_json = "1.0.151"
jsonschema = { version = "0.58.1", default-features = false }
sha2 = "0.11"
base64 = "0.22"
globset = "0.4.20"
url = "2.5.8"
idna = "1.1"
# CLI and diagnostics
clap = { version = "4.6.7", features = ["derive"] }
tracing = "0.1.44"
tracing-subscriber = { version = "0.3.23", features = ["env-filter", "json"] }
# HTTP server (serve: REST, SSE, poll, WS, MCP) and HTTP/1.1+2 client
http = "1.5"
http-body-util = "0.1.5"
hyper = { version = "1.11", features = ["client", "server", "http1", "http2"] }
hyper-util = { version = "0.1.21", features = ["tokio", "server-auto", "service"] }
axum = { version = "0.8.9", default-features = false, features = ["http1", "http2", "json", "query", "tokio", "ws", "tracing"] }
tower = { version = "0.5.3", features = ["util"] }
tokio-tungstenite = { version = "0.29", default-features = false, features = ["handshake", "stream"] }
# TLS (one provider everywhere: ring)
rustls = { version = "0.23.45", default-features = false, features = ["ring", "std", "logging", "tls12"] }
tokio-rustls = { version = "0.26.5", default-features = false, features = ["ring", "tls12"] }
rustls-platform-verifier = "0.7.1"
rustls-pki-types = "1.15"
# QUIC and HTTP/3
quinn = { version = "0.11.12", default-features = false, features = ["runtime-tokio", "rustls-ring", "log"] }
h3 = "=0.0.8"
h3-quinn = { version = "=0.0.10", features = ["datagram"] }
# gRPC (dynamic, descriptor-driven)
tonic = { version = "0.14.6", default-features = false, features = ["channel", "codegen"] }
prost = "0.14.4"
prost-reflect = { version = "0.16.5", features = ["serde"] }
# files, sockets
cap-std = "4.0.3"
cap-fs-ext = "4.0.3"
socket2 = { version = "0.6.5", features = ["all"] }
# MCP (both directions)
rmcp = { version = "~3.5.0", default-features = false, features = ["server", "client", "macros", "transport-io", "transport-child-process", "transport-streamable-http-server", "transport-streamable-http-client"] }
# credential store core
keyring-core = "1.0"

[target.'cfg(target_os = "linux")'.dependencies]
landlock = "0.4.7"
seccompiler = "0.5.0"
libc = "0.2"
zbus-secret-service-keyring-store = { version = "1.0.1", features = ["rt-tokio-crypto-rust"] }

[target.'cfg(target_os = "macos")'.dependencies]
libc = "0.2"
apple-native-keyring-store = { version = "1.0.2", features = ["keychain"] }

[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.61.2", features = ["Win32_Foundation", "Win32_Security", "Win32_Security_Isolation", "Win32_Security_Authorization", "Win32_System_JobObjects", "Win32_System_Threading", "Win32_Storage_FileSystem"] }
windows-native-keyring-store = "1.1.0"
```

Dev-dependencies are for test fixtures only. They were used in the prototype and are **not** part of the runtime:
- `tonic-health = "0.14.6"` (gRPC fixture server);
- `tonic` features `server` and `router`;
- `rcgen = "0.14.10"` (self-signed certificates);
- `oauth2 = { version = "5.0.0", default-features = false }` (optional PKCE oracle).

The superset prototype block that ran the 20 probes is the block above, plus the following:
- `oauth2`, `reqwest` (`rustls-no-provider`, `http2`), `webpki-roots` and `rustix` (Linux);
- `tonic` features `server` and `router`;
- `tonic-health`, `rcgen`;
- `rmcp` without `transport-streamable-http-client`.

## Findings that shape the implementation

1. **The HTTP client is Rivet's own code over `hyper::client::conn`.** The broker resolves and checks the
   address. The adapter dials that `SocketAddr`, does the rustls handshake with the original host as
   `ServerName`, and keeps a small pool keyed by `(origin, checked addr)`. Redirects surface as responses, and
   the adapter re-enters the broker for each hop.
2. **One crypto provider.** Install `rustls::crypto::ring::default_provider()` once at startup. Every
   `ClientConfig`/`ServerConfig` (hyper, quinn, tonic connector, serve TLS) is built from it.
3. **MCP pinning is mandatory with rmcp 3.5.** Override `ServerHandler::supported_protocol_versions` to
   `[V_2025_11_25]`, and construct the client's `ClientInfo` with `.with_protocol_version(V_2025_11_25)`. Set
   `StreamableHttpServerConfig::allowed_hosts` from the serve listen address. Its default is loopback only, and
   the probe showed a foreign `Host` gets 403. Put principal authentication in an axum layer in front of
   `nest_service("/mcp", …)`.
4. **The rmcp child-process transport accepts a caller-built `tokio::process::Command`.** That is the seam
   where the platform sandbox (ADR-0003) wraps stdio MCP servers.
5. **gRPC is dynamic-only.** A `Codec` over `prost_reflect::DynamicMessage`:
   - the method's input/output `MessageDescriptor` comes from the pinned descriptor set;
   - `DynamicMessage::deserialize` rejects unknown JSON fields by default;
   - `tonic::client::Grpc::{unary, server_streaming, client_streaming, streaming}` cover the four modes.
6. **`jsonschema` must never enable `resolve-http`.** Schemas are local data. Remote `$ref` is an error, not an
   I/O effect.
7. **Credential store versioning is Rivet's job.** keyring-core stores opaque bytes. Store
   `{generation, token_set}` JSON and serialize writers through a per-entry lock, verifying the generation on
   read-back. A cross-process lock design remains open (OAuth tasks).
8. **`h3`/`h3-quinn` are the least mature choice.** Pin exact versions, keep them behind the HTTP adapter port,
   and record the version in `rivet.capabilities`.

## Limitations

- Every probe ran on macOS only. Linux and Windows are **build** evidence, not runtime evidence. The platform
  conformance suites (T-series) must run in CI on each OS.
- Streaming gRPC modes, the TLS/mTLS serve listener, OAuth provider flows and real keychain writes were not
  exercised. They were desk-checked against the crate APIs.
- Versions are "latest stable on 2026-09-28". `Cargo.lock` (TASK-014) is the binding record, and `cargo-deny`
  in CI (TASK-008 toolchain) enforces licences and advisories from then on.

## Conclusion

The crate set is feasible, permissively licensed and MSRV-compatible with Rust 1.90. Eighteen of the proposal's
external needs map to maintained crates. OAuth is hand-rolled over the broker HTTP client, and the HTTP client
is a thin Rivet adapter over hyper's connection API. Both choices keep every network attempt under broker
control. Recommendation: adopt the block above as written in [ADR-0002](../decisions/adr-0002-rust-crate-selection.md).

## Related Documents

- [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md) — Increments 5, 8–14, 17; Environment and Feasibility
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) — TASK-009, TASK-014
- [ADR-0001](../decisions/adr-0001-approve-rivet-runtime-design.md), [ADR-0002](../decisions/adr-0002-rust-crate-selection.md)
- [RES-2026-0001](res-2026-0001-capy-grammar-spike.md) (Capy), [RES-2026-0003](res-2026-0003-process-sandbox-backends.md) (sandbox)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Crate survey, 20-probe prototype, cross-target builds, licence audit and proposed dependency block. |
