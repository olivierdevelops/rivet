---
document_id: DEMO-2026-0014
title: "Real-world APIs across transports"
document_type: demo
status: draft
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 2
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [http, transports, ws, datagrams, quic, grpc, mcp, policy, registry]
affected_versions:
  from: "0.2.0"
  to: null
applicable_environments: [development]
audience: [developers, reviewers]
scope: Provide a single cookbook showing how Rivet operations map to public APIs, AI providers, local model servers and user-owned protocol gateways.
reason: The existing numbered demos explain protocol mechanics with controlled fixtures; this bundle adds recognizable real-world integration shapes without embedding credentials or claiming every remote service is permanently available. TASK-076 (PLAN-2026-0002 D-62) re-executed the locally reproducible steps and the no-key public calls against the 0.2.0 release candidate.
dependencies: [PROP-2026-0001, REF-2026-0002]
related_documents: [DEMO-2026-0013, REF-2026-0002, MAN-2026-0008, PLAN-2026-0002, DEMO-2026-0020, MIG-2026-0001, RPT-2026-0014]
supersedes: null
superseded_by: null
tags: [rivet, demo, api, openai, ollama, websocket, grpc, mcp, quic, udp]
confidentiality: internal
review_cycle: on-design-change
next_review_date: 2026-10-27
verified_against: "0.2.0"
---

# Real-world APIs across transports

> **Status:** Draft
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0 and later
> **Owner:** Project maintainer
> **Affected Components:** http, transports, ws, datagrams, quic, grpc, mcp, policy, registry

This is the practical integration cookbook. It uses free public APIs where possible, the OpenAI Responses API, a local Ollama server, a public Binance WebSocket, and clearly marked private-gateway examples for UDP, QUIC and gRPC. It is a design demo: inspect the catalog and policy before making network calls.

The 0.1.0 live smoke-test inputs and outputs are recorded in [rpt-2026-0014-real-world-api-demo-smoke-tests.md](rpt-2026-0014-real-world-api-demo-smoke-tests.md) (historical; not changed). The 0.2.0 re-verification is in [Verification record](#verification-record) below.

Since 0.2.0 every result is a ResponseEnvelope (`{request_id, trace_id, operation, type, status, data, error, effects, data_count}`) and input is given with `--data` on the CLI and `{"operation", "data"}` over HTTP ([migration guide](../../migrations/mig-2026-0001-response-and-input-envelopes.md)). What each step needs, and what could be re-run for this release:

```text
  step                              needs                         0.2.0 re-verification (2026-09-29)
  ────────────────────────────────  ────────────────────────────  ──────────────────────────────────────
  check / outputs / io / generate   10-grpc descriptor (protoc)   re-run offline, PASS
  free.* (Open-Meteo, JSONPlaceholder, GitHub)   internet       live calls PASS; not re-verifiable offline
  market.binance_ticker             internet                      live call PASS; not re-verifiable offline
  ai.openai / ai.openai_stream      OPENAI_API_KEY (billed)       only the missing-key error re-run
  ai.ollama                         local Ollama + a model        PASS against a local Ollama
  transport.udp_gateway             a peer answering {command:"health"}   denial and timeout re-run
  transport.quic_gateway            a trusted rivet-rpc/1 peer    handshake timeout re-run (no peer)
  transport.grpc_health             a real endpoint               dns.resolve re-run (placeholder host)
  transport.mcp_fetch               uvx + its cache grants        fails in the sandbox (see record)
  serve: outputs + polling job      loopback port                 PASS
```

## What is included

| Operation | Transport | Endpoint | Preparation |
|---|---|---|---|
| `free.weather` | HTTPS JSON | Open-Meteo | none; free/no key |
| `free.post` | HTTPS JSON | JSONPlaceholder | none; free test API |
| `free.github_repo` | HTTPS JSON | GitHub REST | none for low-volume public reads |
| `ai.openai` | HTTPS JSON | OpenAI Responses API | `OPENAI_API_KEY`; billed API account |
| `ai.openai_stream` | HTTPS SSE | OpenAI Responses API | `OPENAI_API_KEY`; billed API account |
| `ai.ollama` | HTTP JSONL | local Ollama | install Ollama and `ollama pull llama3.2` |
| `market.binance_ticker` | WebSocket | Binance market stream | public endpoint; rate limits apply |
| `transport.udp_gateway` | UDP datagram | local/private gateway | run a JSON UDP peer and change host/port |
| `transport.quic_gateway` | QUIC stream | local/private gateway | run a Rivet-compatible QUIC peer |
| `transport.grpc_health` | gRPC unary | reviewed descriptor + service | uses the reviewed descriptor from `10-grpc`; replace endpoint for a real service |
| `transport.mcp_fetch` | MCP over command | `uvx mcp-server-fetch` | install `uv`; network grant for fetched URLs |

The same operations can be exposed through Rivet's REST, SSE, polling, WebSocket and MCP serving surfaces. Transport here means the outbound integration; serving is still one `rivet serve` command.

## Quick start

The gRPC connector reuses 10-grpc's descriptor, which is generated, not committed. Generate it first (from `docs/demos`), or every command fails with `not_found.descriptor` (exit 4):

```sh
protoc --proto_path=10-grpc/schemas --include_imports --descriptor_set_out=10-grpc/schemas/users.pb 10-grpc/schemas/users.proto
```

```sh
cd docs/demos/13-real-world-apis
rivet --file app.rivet check --strict-docs
rivet --file app.rivet outputs --all --json
rivet --file app.rivet io --by target
rivet --file app.rivet io --check-policy
```

`check --strict-docs` prints `ok: 11 operations, 2 connectors, 0 auth profiles` (exit 0). `outputs --all --json` is a `rivet.outputs` envelope with twelve entries (the eleven operations plus the imported `fetch_mcp.tools.fetch`). `io --check-policy` ends with `13 allowed · 2 unknown` and exits 0: the UDP and QUIC targets are built from parameters and are `dynamic`.

Try the no-key APIs first:

```sh
rivet --file app.rivet request free.weather --data '{"latitude":25.03,"longitude":121.56}'
rivet --file app.rivet request free.post --data '{"id":1}'
rivet --file app.rivet request free.github_repo --data '{"owner":"rust-lang","repo":"rust"}'
```

With the internet reachable each exits 0 with `"status":"ok"`; for example (2026-09-29):

```json
{"request_id":"req_01a7c11195","trace_id":"tr_01a7c11195","operation":"free.post","type":"result","status":"ok","data":{"userId":1,"id":1,"title":"sunt aut facere repellat provident occaecati excepturi optio reprehenderit","body":"quia et suscipit\nsuscipit recusandae consequuntur expedita et cum\nreprehenderit molestiae ut ut quas totam\nnostrum rerum est autem sunt rem eveniet architecto"},"error":null,"effects":"none","data_count":0}
```

Offline, the same calls fail with `dns.resolve` (kind `dns`, exit 5), which is also what the placeholder gRPC host `users.example.com` gives.

These endpoints are documented by [Open-Meteo](https://open-meteo.com/en/docs), [JSONPlaceholder](https://jsonplaceholder.typicode.com/guide/) and [GitHub's REST API](https://docs.github.com/en/rest/repos/repos#get-a-repository).

## AI providers

OpenAI's official quickstart uses the Responses API with a model and input; the demo keeps the API key in a bound secret and returns only the provider response. [OpenAI Responses quickstart](https://platform.openai.com/docs/quickstart/make-your-first-api-request)

```sh
export OPENAI_API_KEY='...'
rivet --file app.rivet request ai.openai --data '{"prompt":"Give me three names for a tiny Rust tool."}'
rivet --file app.rivet request ai.openai_stream --data '{"prompt":"Count from one to five."}' --stream
```

Without the key the call stops before any network I/O (exit 4):

```json
{"request_id":"req_01fe1ae42d","trace_id":"tr_01fe1ae42d","operation":"ai.openai","type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"not_found.env","message":"environment variable OPENAI_API_KEY is not set","retryable":false,"source":{"file":"app.rivet","line":49,"column":5,"end_line":49,"end_column":78},"operation_id":"ai.openai"},"effects":"none","data_count":0}
```

The stream operation uses SSE and emits decoded events. Never put the key in `app.rivet`, a policy file, params, or a checked-in shell script. Rivet's secret binding prevents it from being returned as an output.

For Ollama, start the local server and pull a model:

```sh
ollama serve
ollama pull llama3.2
rivet --file app.rivet request ai.ollama --data '{"prompt":"Explain Rust ownership in one sentence."}' --stream
```

Ollama's `/api/chat` endpoint returns newline-delimited JSON when `stream` is true. This example stops on the `done` record and demonstrates local-model traffic without a cloud key. Each chunk is a `type: data` record (`{"role":"assistant","content":"…"}`) and the `done` record is the terminal `type: result` record's `data`. [Ollama API documentation](https://docs.ollama.com/api)

## WebSocket and serving surfaces

```sh
rivet --file app.rivet request market.binance_ticker
rivet --file app.rivet serve --listen 127.0.0.1:8080
curl -sS http://127.0.0.1:8080/v1/operations/free.post/outputs
curl -sS -X POST http://127.0.0.1:8080/v1/requests \
  -H 'content-type: application/json' \
  -d '{"operation":"free.weather","data":{"latitude":25.03,"longitude":121.56}}'
```

`/outputs` answers a `rivet.outputs` envelope (`"data":{"id":"free.post","output":{"description":"The JSONPlaceholder post."},…}`). The polling POST answers HTTP 202 with `"status":"accepted"` and the session receipt in `data`; read `data.events_url` until `terminal` is true (a live Open-Meteo call took up to 12 s during verification). The 0.1.0 body key `params` is still accepted in 0.2.x and marks the response `deprecation: true`.

The `serve` command publishes the same catalog to REST, SSE, polling, WebSocket and MCP. The public WebSocket operation is deliberately one-message bounded so a CLI invocation has deterministic cleanup.

## Private transports: UDP, QUIC, gRPC and MCP

Public UDP and QUIC APIs are uncommon because they require an owned peer, firewall rules and a protocol contract. The two operations show the real lifecycle and policy shape with loopback defaults; replace the endpoint and grant together:

```sh
rivet --file app.rivet request transport.udp_gateway --data '{"host":"127.0.0.1","port":9000}'
rivet --file app.rivet request transport.quic_gateway --data '{"endpoint":"quic://127.0.0.1:4433"}'
```

Without a peer, the UDP call times out after 2 s with `effects: "committed"` (the datagram left) and the QUIC call with `the QUIC handshake did not complete within 5000 ms` (both exit 6). A port that is not granted literally is refused before any I/O (`permission.denied`, exit 3), for example `--data '{"port":9001}'`.

The gRPC example intentionally requires a reviewed descriptor rather than reflection. It reuses the checked-in descriptor from the gRPC fixture bundle so the catalog can be inspected immediately; replace both the endpoint and descriptor for a production service:

```sh
rivet --file app.rivet request transport.grpc_health
```

For MCP, the connector uses an absolute `uvx` path because Rivet does not perform PATH lookup for spawned commands. Replace `/Users/oliverlaleau/.local/bin/uvx` in `app.rivet` and `policy.json` with the result of `command -v uvx` on another host, then run:

```sh
rivet --file app.rivet request transport.mcp_fetch --data '{"url":"https://www.rust-lang.org"}'
```

Review the `allow_exec`, `allow_mcp` and network grants before enabling this operation. On macOS the spawned `uvx` runs inside Rivet's Seatbelt process sandbox; with only these grants it cannot write its cache and exits with status 2 (`process.exit`, `stderr: "error: Operation not permitted (os error 1)"`). Granting the uv cache and tool directories is a host decision and is left out of this draft policy. The fetched URL is a caller-controlled remote effect and should be narrowed in a production policy.

## Policy and safety

`policy.json` is intentionally broad enough to make the examples legible, not a production policy. In particular:

- `allow_network` names each origin; private-range blocking remains enabled.
- `OPENAI_API_KEY` is the only environment grant.
- gRPC requires both the endpoint and the exact method grant.
- MCP command execution is explicitly granted to `uvx`.
- The example `serve.auth` is loopback-only. Add bearer or mTLS authentication before binding beyond loopback.

Generate a narrower starting point and review it before use:

```sh
rivet --file app.rivet policy generate --output policy.draft.json
```

It writes 10 grants and lists 3 review items (the opaque MCP tool call and the two dynamic UDP/QUIC targets), then exits 7 because the draft is incomplete. Remove `policy.draft.json` after review.

## Verification record

| Step | Verified By | Verified At | Result |
|---|---|---|---|
| Descriptor generated; `check --strict-docs`, `outputs --all --json`, `io --by target`, `io --check-policy` (13 allowed · 2 unknown) | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| `policy generate --output` (10 grants, 3 review items, exit 7) | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |
| `free.weather`, `free.post`, `free.github_repo` (live public APIs) | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS on the day; public services, not re-verifiable offline |
| `market.binance_ticker` (live public WebSocket) | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS on the day; not re-verifiable offline |
| `ai.openai` without a key → `not_found.env` | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS; calls with a real key NOT RUN (billed credential) |
| `ai.ollama --stream` against a local Ollama | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS (model named by the bundle's default; records as described) |
| `transport.udp_gateway` / `transport.quic_gateway` without a matching peer; ungranted port | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS (timeouts exit 6, denial exit 3); a real gateway NOT RUN |
| `transport.grpc_health` against the placeholder host | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | `dns.resolve` as expected; a real service NOT RUN |
| `transport.mcp_fetch` via `uvx` | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | FAIL in the sandbox: `process.exit` 2, `Operation not permitted` (needs uv cache grants; documented above) |
| `serve`: `/outputs` envelope, polling job (202 `accepted`, events), legacy `params` with `deprecation: true` | Claude (TASK-076) | 2026-09-29, commit 8031baa, macOS 26.4.1 arm64 | PASS |

Verified on 0.2.0-dev at commit `8031baa`, the release candidate (`cargo build --release --workspace --all-features`). Public-API results depend on third-party availability on the day and cannot be re-verified offline; the historical 0.1.0 smoke report RPT-2026-0014 is unchanged.

## Related documents

- [All demos](../README.md) · [v0.2.0 release verification guide](../demo-2026-0020-v0-2-0-release-verification.md) · [envelope reference](../../api/api-2026-0006-envelopes.md)
- [Language and usage reference](../../references/ref-2026-0002-language-and-usage.md)
- [Protocols and connectors guide](../../manuals/man-2026-0008-protocols-and-connectors.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 2 | 2026-09-29 | Claude | TASK-076 (PLAN-2026-0002 D-62): §7 header; `--params` → `--data`, HTTP body `{operation, data}`; descriptor prerequisite; 0.2.0 envelope examples and per-step re-verification map; real results for inspection, public no-key APIs (live on the day), missing OpenAI key, local Ollama, UDP/QUIC without peers, gRPC placeholder, the sandboxed `uvx` failure and serve; verification record; verified_against 0.2.0. Status stays draft (separately owned cookbook). |
| 1 | 2026-09-28 | Codex | Added public API, OpenAI, Ollama and cross-transport integration cookbook. |
