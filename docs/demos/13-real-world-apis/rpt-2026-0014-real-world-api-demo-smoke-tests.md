---
document_id: RPT-2026-0014
title: "Real-world API demo smoke-test results"
document_type: report
status: draft
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Codex]
owner: Project maintainer
report_date: 2026-09-28
systems: [Rivet]
components: [http, transports, ws, datagrams, quic, grpc, mcp, policy]
affected_versions:
  from: "0.1.0"
  to: null
confidentiality: internal
scope: Record live smoke-test inputs and outputs for the real-world API demo bundle.
reason: User requested every added example be exercised and documented with inputs and outputs.
methodology: Invoke every catalog operation once with bounded subprocess timeouts where applicable; redact credentials and summarize dynamic provider fields.
evidence_sources: [docs/demos/13-real-world-apis/app.rivet, rivet check --strict-docs, scripts/check_docs.py, vhco validate, vhco sync, vhco check]
related_documents: [DEMO-2026-0014]
supersedes: null
superseded_by: null
tags: [rivet, demos, smoke-test, api, transports]
---

# Real-world API demo test results

Date: 2026-09-28  
Bundle: [`app.rivet`](app.rivet)  
Command: `rivet --file app.rivet request ...`  
Host: macOS arm64, local Rivet build

These are live smoke tests, not deterministic unit tests. Request IDs, timestamps, weather, market data and model text change between runs. Secrets are not included.

## Summary

| Operation | Input | Result |
|---|---|---|
| `free.weather` | `{"latitude":25.03,"longitude":121.56}` | PASS — JSON forecast returned; exit 0 |
| `free.post` | `{"id":1}` | PASS — JSONPlaceholder post returned; exit 0 |
| `free.github_repo` | `{"owner":"rust-lang","repo":"rust"}` | PASS — public repository metadata returned; exit 0 |
| `ai.openai` | `{"prompt":"Say hello."}` | BLOCKED — `not_found.env`, `OPENAI_API_KEY` absent; exit 4 |
| `ai.openai_stream` | `{"prompt":"Say hello."}` | BLOCKED — `not_found.env`, `OPENAI_API_KEY` absent; exit 4 |
| `ai.ollama` | `{"prompt":"Reply with exactly three words: ownership is explicit."}` | PASS — local `ministral-3:3b` returned content; exit 0 |
| `market.binance_ticker` | `{}` | PASS — one BTCUSDT ticker event returned; exit 0 |
| `transport.udp_gateway` | `{"host":"127.0.0.1","port":9000}` | EXPECTED FAIL — `udp.receive_failed`, no peer listening; exit 5 |
| `transport.quic_gateway` | `{"endpoint":"quic://127.0.0.1:4433"}` | EXPECTED FAIL — QUIC handshake timeout; exit 6 |
| `transport.grpc_health` | `{}` | EXPECTED FAIL — `users.example.com` is a placeholder/unresolvable endpoint; exit 5 |
| `transport.mcp_fetch` | `{"url":"https://www.rust-lang.org"}` | ENVIRONMENT FAIL — `uvx` exited 2 with `Operation not permitted`; exit 5 |

## Successful outputs

### `free.weather`

Input:

```json
{"latitude":25.03,"longitude":121.56}
```

Observed output shape:

```json
{
  "latitude": 25.06151,
  "longitude": 121.5194,
  "timezone": "GMT",
  "current": {
    "time": "2026-09-28T09:00",
    "temperature_2m": 32.0,
    "wind_speed_10m": 7.5
  }
}
```

### `free.post`

Input:

```json
{"id":1}
```

Observed output:

```json
{
  "userId": 1,
  "id": 1,
  "title": "sunt aut facere repellat provident occaecati excepturi optio reprehenderit",
  "body": "quia et suscipit\nsuscipit recusandae consequuntur expedita et cum\nreprehenderit molestiae ut ut quas totam\nnostrum rerum est autem sunt rem eveniet architecto"
}
```

### `free.github_repo`

Input:

```json
{"owner":"rust-lang","repo":"rust"}
```

Observed output excerpt:

```json
{
  "id": 724712,
  "name": "rust",
  "full_name": "rust-lang/rust",
  "private": false,
  "language": "Rust",
  "default_branch": "main",
  "stargazers_count": 119255
}
```

The complete provider response also contained the normal GitHub URLs, ownership and repository metadata.

### `ai.ollama`

Input:

```json
{"prompt":"Reply with exactly three words: ownership is explicit."}
```

Observed output:

```json
{
  "model": "ministral-3:3b",
  "message": {
    "role": "assistant",
    "content": "**Ownership is clear.**"
  },
  "done": true,
  "done_reason": "stop"
}
```

The operation received 6 streamed data items and correctly accumulated their content before returning the terminal result.

### `market.binance_ticker`

Input:

```json
{}
```

Observed output excerpt:

```json
{
  "e": "24hrTicker",
  "s": "BTCUSDT",
  "c": "82660.00000000",
  "P": "-2.282",
  "v": "15494.70170000"
}
```

The WebSocket operation received one event and disposed the socket after the bounded read.

## Credential and setup failures

### OpenAI

Both operations were invoked with:

```json
{"prompt":"Say hello."}
```

Both returned the expected safe failure before network I/O:

```json
{
  "kind": "not_found",
  "code": "not_found.env",
  "message": "environment variable OPENAI_API_KEY is not set",
  "effects": "none"
}
```

To run them, export a real key in the shell and never add it to this repository:

```sh
export OPENAI_API_KEY='...'
rivet --file app.rivet request ai.openai --params '{"prompt":"Say hello."}'
rivet --file app.rivet request ai.openai_stream --params '{"prompt":"Say hello."}' --stream
```

### Private UDP, QUIC and gRPC

These operations are intentionally gateway examples. Their failures confirm that Rivet reaches the transport boundary and does not silently succeed against a nonexistent peer:

- UDP: `udp.receive_failed`, connection refused, exit 5.
- QUIC: handshake did not complete within 5000 ms, exit 6.
- gRPC: DNS resolution failed for the placeholder `users.example.com`, exit 5.

Replace the endpoint and matching policy grant, then rerun the same inputs.

### MCP command connector

The MCP operation passed catalog, snapshot, and `allow_mcp` authorization. The installed absolute `uvx` command then exited with:

```text
error: Operation not permitted (os error 1)
```

This is a host/process-sandbox setup issue, not a Rivet catalog or MCP target mismatch. On another host, replace the absolute `uvx` path in both `app.rivet` and `policy.json`, and ensure the `uvx` cache/package location is permitted by the host policy.

## Verification after the run

```text
ok: 11 operations, 2 connectors, 0 auth profiles
check_docs: 122 files, 0 problem(s)
vhco validate: no violations
vhco sync: code matches vhco-contract.json
vhco check: 1 guarantee holds
```
