---
document_id: DEMO-2026-0014
title: "Real-world APIs across transports"
document_type: demo
status: draft
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Codex]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [http, transports, ws, datagrams, quic, grpc, mcp, policy, registry]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development]
audience: [developers, reviewers]
scope: Provide a single cookbook showing how Rivet operations map to public APIs, AI providers, local model servers and user-owned protocol gateways.
reason: The existing numbered demos explain protocol mechanics with controlled fixtures; this bundle adds recognizable real-world integration shapes without embedding credentials or claiming every remote service is permanently available.
dependencies: [PROP-2026-0001, REF-2026-0002]
related_documents: [DEMO-2026-0013, REF-2026-0002, MAN-2026-0008]
supersedes: null
superseded_by: null
tags: [rivet, demo, api, openai, ollama, websocket, grpc, mcp, quic, udp]
confidentiality: internal
review_cycle: on-design-change
next_review_date: 2026-10-27
verified_against: not-verified
---

# Real-world APIs across transports

This is the practical integration cookbook. It uses free public APIs where possible, the OpenAI Responses API, a local Ollama server, a public Binance WebSocket, and clearly marked private-gateway examples for UDP, QUIC and gRPC. It is a design demo: inspect the catalog and policy before making network calls.

The latest live smoke-test inputs and outputs are recorded in [rpt-2026-0014-real-world-api-demo-smoke-tests.md](rpt-2026-0014-real-world-api-demo-smoke-tests.md).

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

```sh
cd docs/demos/13-real-world-apis
rivet --file app.rivet check --strict-docs
rivet --file app.rivet outputs --all --json
rivet --file app.rivet io --by target
rivet --file app.rivet io --check-policy
```

Try the no-key APIs first:

```sh
rivet --file app.rivet request free.weather --params '{"latitude":25.03,"longitude":121.56}'
rivet --file app.rivet request free.post --params '{"id":1}'
rivet --file app.rivet request free.github_repo --params '{"owner":"rust-lang","repo":"rust"}'
```

These endpoints are documented by [Open-Meteo](https://open-meteo.com/en/docs), [JSONPlaceholder](https://jsonplaceholder.typicode.com/guide/) and [GitHub's REST API](https://docs.github.com/en/rest/repos/repos#get-a-repository).

## AI providers

OpenAI's official quickstart uses the Responses API with a model and input; the demo keeps the API key in a bound secret and returns only the provider response. [OpenAI Responses quickstart](https://platform.openai.com/docs/quickstart/make-your-first-api-request)

```sh
export OPENAI_API_KEY='...'
rivet --file app.rivet request ai.openai --params '{"prompt":"Give me three names for a tiny Rust tool."}'
rivet --file app.rivet request ai.openai_stream --params '{"prompt":"Count from one to five."}' --stream
```

The stream operation uses SSE and emits decoded events. Never put the key in `app.rivet`, a policy file, params, or a checked-in shell script. Rivet's secret binding prevents it from being returned as an output.

For Ollama, start the local server and pull a model:

```sh
ollama serve
ollama pull llama3.2
rivet --file app.rivet request ai.ollama --params '{"prompt":"Explain Rust ownership in one sentence."}' --stream
```

Ollama's `/api/chat` endpoint returns newline-delimited JSON when `stream` is true. This example stops on the `done` record and demonstrates local-model traffic without a cloud key. [Ollama API documentation](https://docs.ollama.com/api)

## WebSocket and serving surfaces

```sh
rivet --file app.rivet request market.binance_ticker --params '{}'
rivet --file app.rivet serve --listen 127.0.0.1:8080
curl -sS http://127.0.0.1:8080/v1/operations/free.post/outputs
curl -sS -X POST http://127.0.0.1:8080/v1/requests \
  -H 'content-type: application/json' \
  -d '{"operation":"free.weather","params":{"latitude":25.03,"longitude":121.56}}'
```

The `serve` command publishes the same catalog to REST, SSE, polling, WebSocket and MCP. The public WebSocket operation is deliberately one-message bounded so a CLI invocation has deterministic cleanup.

## Private transports: UDP, QUIC, gRPC and MCP

Public UDP and QUIC APIs are uncommon because they require an owned peer, firewall rules and a protocol contract. The two operations show the real lifecycle and policy shape with loopback defaults; replace the endpoint and grant together:

```sh
rivet --file app.rivet request transport.udp_gateway --params '{"host":"127.0.0.1","port":9000}'
rivet --file app.rivet request transport.quic_gateway --params '{"endpoint":"quic://127.0.0.1:4433"}'
```

The gRPC example intentionally requires a reviewed descriptor rather than reflection. It reuses the checked-in descriptor from the gRPC fixture bundle so the catalog can be inspected immediately; replace both the endpoint and descriptor for a production service:

```sh
rivet --file app.rivet request transport.grpc_health --params '{}'
```

For MCP, the connector uses an absolute `uvx` path because Rivet does not perform PATH lookup for spawned commands. Replace `/Users/oliverlaleau/.local/bin/uvx` in `app.rivet` and `policy.json` with the result of `command -v uvx` on another host, then run:

```sh
rivet --file app.rivet request transport.mcp_fetch --params '{"url":"https://www.rust-lang.org"}'
```

Review the `allow_exec`, `allow_mcp` and network grants before enabling this operation. The fetched URL is a caller-controlled remote effect and should be narrowed in a production policy.

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

## Verification record

| Check | Result |
|---|---|
| JSON policy syntax and local links | Checked when added |
| Source syntax / strict documentation | Pending runtime verification |
| Public API calls | Not run automatically; availability and quotas vary |
| Credentialed, local and private transports | Not run; require user-owned setup |

## Related documents

- [All demos](../README.md)
- [Language and usage reference](../../references/ref-2026-0002-language-and-usage.md)
- [Protocols and connectors guide](../../manuals/man-2026-0008-protocols-and-connectors.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Codex | Added public API, OpenAI, Ollama and cross-transport integration cookbook. |
