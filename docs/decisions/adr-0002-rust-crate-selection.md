---
document_id: ADR-0002
title: "Rust crate selection for Rivet v0.1.0"
document_type: decision
status: approved
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
approval:
  decision_date: 2026-09-28
  approvers: [Project maintainer]
  basis: "ADR-0001 blanket approval — maintainer, 2026-09-28: \"everything else is approved\" (covers the P1 research-derived ADRs of PLAN-2026-0001)"
systems: [Rivet]
components: [language, execution, files, connectors, policy, auth, datagrams, quic, grpc, serve, transports, cli, http, ws, mcp, library]
affected_versions:
  from: not-applicable
  to: "0.1.0"
scope: The third-party Rust crates Rivet v0.1.0 depends on and why.
reason: PLAN-2026-0001 TASK-012 — fix the third-party crates, versions and feature sets that TASK-014 writes into Cargo.toml, based on the RES-2026-0002 prototypes.
related_documents: [RES-2026-0002, RES-2026-0001, PROP-2026-0001, PLAN-2026-0001, ADR-0001, ADR-0003]
supersedes: null
superseded_by: null
tags: [rivet, rust, crates, dependencies, decision]
confidentiality: internal
review_cycle: on-change
next_review_date: 2026-12-28
---

# Rust crate selection for Rivet v0.1.0

> **Status:** Approved
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** not-applicable → 0.1.0
> **Owner:** Project maintainer
> **Affected Components:** language, execution, files, connectors, policy, auth, datagrams, quic, grpc, serve, transports, cli, http, ws, mcp, library

## Status

Approved on 2026-09-28. The approver is the project maintainer, under the blanket approval recorded in
[ADR-0001](adr-0001-approve-rivet-runtime-design.md). The maintainer approved the design and PLAN-2026-0001
with *"everything else is approved"*, and PLAN-2026-0001 names this ADR as a P1 output (TASK-012). A change to
the crate set after this date needs a new ADR that supersedes this one.

## Context

PROP-2026-0001 needs the following:
- an async host;
- one HTTP listener for REST, SSE, polling, WebSocket and MCP;
- an HTTP client that binds a broker-checked IP and never follows redirects on its own;
- WebSocket, UDP multicast, QUIC with DATAGRAM, and HTTP/3;
- dynamic gRPC from a pinned `FileDescriptorSet`;
- OAuth 2.0 (client credentials, code+PKCE, device, refresh);
- TLS and mTLS;
- JSON Schema;
- confined no-follow file handles;
- argv-only processes;
- MCP (`2025-11-25`, stdio and Streamable HTTP, both directions);
- CLI parsing, tracing, hashing, glob matching, URL/IDNA handling and a secure credential store.

The proposal's Environment and Feasibility table left most of these as "experiment needed".
[RES-2026-0002](../research/res-2026-0002-rust-crate-feasibility.md) ran 20 live probes on macOS with Rust
1.90.0, and all 20 passed. It also built the full dependency block for Linux and Windows and audited all 393
packages in the graph for licences.

## Decision

Rivet v0.1.0 uses the dependency block in
[RES-2026-0002 § Proposed dependency block](../research/res-2026-0002-rust-crate-feasibility.md#proposed-dependency-block-for-task-014)
verbatim. It is summarised here:

```text
 LAYER            CHOICE                                                     VERSION (pin style)
 ───────────────  ─────────────────────────────────────────────────────────  ──────────────────────────
 language         capy-core (git rev 84f984c64e0811ef2bfff7835167d6630422ecaa) 0.22.0 (exact rev)
 runtime          tokio · tokio-util · tokio-stream · futures-util · bytes  1.53 · 0.7.19 · 0.1.19 · 0.3.34 · 1.12
 data             serde · serde_json · jsonschema (no default features)      1.0.229 · 1.0.151 · 0.58.1
 small utils      sha2 · base64 · globset · url · idna                       0.11 · 0.22 · 0.4.20 · 2.5.8 · 1.1
 cli / logs       clap (derive) · tracing · tracing-subscriber               4.6.7 · 0.1.44 · 0.3.23
 http server      axum (ws, http1, http2) on hyper 1 + hyper-util server-auto 0.8.9 · 1.11 · 0.1.21
 http client      hyper::client::conn (h1/h2) — Rivet-owned pool, broker-dialed 1.11
 websocket        tokio-tungstenite (client) · axum ws (server)              0.29 (matches axum)
 tls              rustls (ring) · tokio-rustls · rustls-platform-verifier    0.23.45 · 0.26.5 · 0.7.1
 quic / http3     quinn (rustls-ring) · h3 · h3-quinn                        0.11.12 · =0.0.8 · =0.0.10
 grpc             tonic (channel, codegen) · prost · prost-reflect (serde)   0.14.6 · 0.14.4 · 0.16.5
 oauth 2.0        HAND-ROLLED over the broker HTTP client (serde/url/sha2/base64)   —
 files / sockets  cap-std · cap-fs-ext · socket2                             4.0.3 · 4.0.3 · 0.6.5
 processes        tokio::process (argv only)                                 (tokio)
 mcp              rmcp (server, client, stdio, child-process, streamable-http) ~3.5.0
 credentials      keyring-core + apple-native / windows-native / zbus-secret-service stores  1.0 · 1.0.2 · 1.1.0 · 1.0.1
 sandbox (os)     landlock · seccompiler · libc (Linux) · windows-sys (Windows)  0.4.7 · 0.5.0 · 0.2 · 0.61.2
```

Binding rules that come with the choice:

1. **One TLS crypto provider: `ring`.** Install it once at startup. No crate may enable `aws-lc-rs` or OpenSSL.
   `cargo tree -i aws-lc-rs` and `cargo tree -i openssl-sys` must stay empty (CI check).
2. **The network client is broker-dialed.** hyper, tokio-tungstenite, the tonic connector and quinn always
   receive a `SocketAddr` that the broker has already checked, and the original host is kept as the TLS
   `ServerName`. Redirects are returned to the adapter, never followed by a library.
3. **MCP version pinning.** The rmcp server overrides `supported_protocol_versions` to `[2025-11-25]`. The rmcp
   client sends `ClientInfo::with_protocol_version(V_2025_11_25)`. `allowed_hosts` is set from the serve
   listen address. Principal authentication sits in an axum layer in front of `/mcp`.
4. **`jsonschema` never enables `resolve-http` or `resolve-file`.** A remote `$ref` is a validation error, not
   an effect.
5. **Pre-1.0 pins.** `h3`, `h3-quinn` and `rmcp` are pinned (`=` or `~`). The versions in use are reported by
   `rivet.capabilities`.
6. **Credential versioning is Rivet's.** Entries hold `{generation, token_set}`, and writers are serialized per
   entry, because keyring-core has no compare-and-swap.
7. **Test-only dependencies.** `tonic-health`, `rcgen`, tonic's `server`/`router` features and optionally
   `oauth2` as a PKCE oracle are dev-dependencies only.

## Rationale

- **Evidence over preference.** Every selected runtime crate passed a live probe of the Rivet-specific
  behaviour, not only a compile check:
  - checked-IP dialing;
  - unfollowed redirects;
  - one listener serving five surfaces;
  - MCP `2025-11-25` over HTTP and stdio;
  - ProtoJSON strictness;
  - QUIC ALPN, FIN and DATAGRAM;
  - HTTP/3;
  - no-follow escapes refused;
  - multicast;
  - argv literalness.

  The two exceptions are the sandbox crates (ADR-0003) and the keychain write path, which had build and mock
  evidence only.
- **Broker control of every attempt.** The Increment 5 sandbox guarantees depend on the broker seeing DNS,
  every redirect hop and every reconnect. Connection-level hyper and a hand-rolled OAuth client keep that
  control inside Rivet's code.
- **Licence and toolchain hygiene.** Every package in the graph is permissive. MSRV ceiling 1.88 ≤ 1.90. `ring`
  avoids the cmake/NASM toolchain that aws-lc needs on Windows.
- **Official SDK for MCP.** rmcp is the protocol's reference Rust SDK. It already implements Host validation,
  sessions and version negotiation, and passed both transports in the probe.

## Alternatives Considered

| Alternative | Why rejected |
|---|---|
| `reqwest` 0.13.5 as the HTTP client | DNS overrides and pooling are per-`Client`, not per attempt. Defaults bring aws-lc and system-proxy discovery. HTTP/3 is behind an unstable cfg |
| `oauth2` 5.0.0 | Device-flow polling is an internal loop, so it cannot return `pending` at the caller's deadline or carry `slow_down` across `complete` calls. Defaults bring `reqwest` |
| `keyring` 4.2.0 umbrella crate | Its own documentation directs applications to `keyring-core` plus chosen stores |
| Hand-rolled MCP JSON-RPC | Not needed: rmcp passed. Kept as the fallback if rmcp churn breaks a contract requirement |
| `openat2` crate | Stale (2021). `cap-std` already uses `openat2` internally on Linux |
| `webpki-roots` bundled trust | The proposal names the system trust store. `rustls-platform-verifier` provides it; bundled roots would add a second trust source |
| `aws-lc-rs` provider (the rustls/quinn default) | Heavier native build on Windows. It gives no benefit Rivet needs in v0.1.0 (no FIPS requirement) |

## Consequences

### Positive Consequences

- TASK-014 can write `Cargo.toml` mechanically from RES-2026-0002, and the block is already known to build on
  all three targets.
- Most "experiment needed" items in the proposal's feasibility table are now proven. That includes shared-listener
  WS and MCP routing, which was the Increment 17 open item.
- There is no C toolchain dependency beyond `ring`'s bundled assembly and the cc build.

### Negative Consequences

- Rivet owns more code than a batteries-included client would need:
  - a per-origin connection pool;
  - redirect handling;
  - an OAuth token client (three grants plus refresh);
  - credential versioning.
- Two generations of `sha2`/`digest`/`rand` coexist in the graph, which costs compile time. It is harmless.

### Risks

- **`h3`/`h3-quinn` are 0.0.x and last released 2025-05.** Mitigation: exact pins, isolation behind the HTTP
  port, and `http.version_unavailable` if HTTP/3 must be disabled.
- **rmcp moves fast** (3.x, a new `2026-07-28` protocol without `initialize`). Mitigation: a `~3.5` pin, pinned
  protocol version, conformance tests T-06/T-12, and the hand-rolled fallback.
- **Linux and Windows are build-verified only for this crate set.** Mitigation: the T-series conformance suites
  run in CI on each OS before release (plan P3).
- **`capy-core` is a git dependency.** Mitigation: exact rev, recorded in RES-2026-0001. The licence was cleared
  by the owner (ADR-0001).

## Implementation Impact

- **TASK-014** creates `Cargo.toml` and `Cargo.lock` with this block. `rust-version = "1.90"` (TASK-008 records
  the toolchain).
- **TASK-008 / CI** adds `cargo deny check licenses bans advisories`, with an allow-list covering:
  - MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception;
  - BSD-2-Clause, BSD-3-Clause, ISC, Zlib, MIT-0, BSL-1.0, Unicode-3.0, Unlicense;
  - CDLA-Permissive-2.0 if it is ever pulled transitively.

  It also bans `aws-lc-rs` and `openssl-sys`.
- **Adapter tasks** that follow the binding rules above:
  - HTTP client: `src/infra/` HTTP adapter;
  - OAuth: auth feature;
  - MCP: `src/io/mcp`, `src/infra/mcp_client.rs`;
  - gRPC: dynamic codec;
  - credentials: store adapter.
- **Sandbox crates** are used by ADR-0003's backends.

## Superseded Decisions

None.

## Related Documents

- [RES-2026-0002](../research/res-2026-0002-rust-crate-feasibility.md) — evidence
- [RES-2026-0001](../research/res-2026-0001-capy-grammar-spike.md) — Capy pinned commit
- [ADR-0001](adr-0001-approve-rivet-runtime-design.md) — approval basis
- [ADR-0003](adr-0003-process-sandbox-backends.md) — sandbox backends
- [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md), [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Recorded the crate selection from RES-2026-0002 under ADR-0001's blanket approval. |
