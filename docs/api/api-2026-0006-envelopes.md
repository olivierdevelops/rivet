---
document_id: API-2026-0006
title: "Rivet response and input envelopes"
document_type: api
status: active
created_date: 2026-09-29
last_updated: 2026-09-29
document_revision: 2
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [serve, cli, http, poll, ws, mcp, library, ffi]
affected_versions:
  from: "0.2.0"
  to: null
applicable_environments: [development, embedded, server]
audience: [developers, integrators, operators]
scope: The one output shape (ResponseEnvelope and stream records) and the one input shape (InputEnvelope) of every Rivet 0.2.0 surface — key order, status and type values, the error object, pretty output, per-surface wrapping (SSE events, polling batches, WebSocket `ref`, MCP `structuredContent`, library and C ABI), deprecated 0.1.0 input aliases and the JSON Schemas in docs/api/schemas/.
reason: PLAN-2026-0002 row D-10 (TASK-071) — the canonical envelope reference that every other API document, manual and demo links to; examples are real output of the 0.2.0 release candidate.
related_documents: [PROP-2026-0002, PLAN-2026-0002, ADR-0004, API-2026-0001, API-2026-0002, API-2026-0003, API-2026-0004, API-2026-0005, API-2026-0007, MIG-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, api, envelope, json, schema, pretty, deprecation]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.2.0-rc (main at e7ed8ed)"
next_review_date: 2026-10-29
---

# Rivet response and input envelopes

> **Status:** Active
> **Created:** 2026-09-29
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.2.0 and later
> **Owner:** Project maintainer
> **Affected Components:** envelope, serve, cli, http, sse, poll, ws, mcp, library, ffi

## Summary

From 0.2.0, every Rivet surface answers with **one JSON shape** and accepts **one JSON input shape**:

```text
                 InputEnvelope {operation, data, deadline_ms?, restrict?, stream?, pretty?}
                                          │
   CLI --data/--input · POST /v1/request · POST /v1/requests · WS request frame
   MCP rivet.request · Runtime::call / call_json · rivet_request / rivet_call_start
                                          │
                                          ▼
                     serve.parse_input ──▶ dispatcher ──▶ outcome
                                          │
                                          ▼
   ResponseEnvelope {request_id, trace_id, operation, type, status, data, error, effects, data_count}
        + stream records {…, type:"data", seq, data, error:null} … then one {type:"result", seq, …}
                                          │
   CLI stdout/stderr · HTTP body · SSE `data:` · polling `events[]` · WS frame (+ref)
   MCP structuredContent (+ content[0].text) · Rust ResponseEnvelope · C char*
```

The 0.1.0 shapes (`{"result": …}` on success, `{request_id, trace_id, error}` on failure, `{id, params}` input,
`event: error` on SSE) are gone. The 0.1.0 input keys `id` and `params` still work through 0.2.x as deprecated
aliases; see [MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md) for the migration.

All captures below come from the 0.2.0 release candidate (`cargo build --release --features cli` at `main`
`e7ed8ed`, macOS 26.4, 2026-09-29). **Request and trace IDs differ on every run.** The workspace version was
still `0.1.0` at capture time, so version fields in captures read `0.1.0`; the release build reads `0.2.0`.

## Audience and Stability

Client authors on any surface, embedders (Rust and C) and operators reading logs. The envelope keys, their order,
the `status` and `type` values and the error object are **stable for 0.2.x**. New keys may be added to the error
object and to `data` payloads of built-ins; the top-level key set changes only with a major ABI/API revision.

## Authentication

Unchanged from 0.1.0: authentication belongs to the surface (bearer tokens on `rivet serve`, the host process for
the library and the C ABI). See [API-2026-0001](api-2026-0001-http-rest-sse-polling.md#authentication).

## Endpoints or Events

The envelopes are not endpoints; they are the body of every endpoint and event:

| Surface | Input | Output |
|---|---|---|
| CLI `rivet request` | `ID --data JSON`, or `--input FILE\|-` (a whole InputEnvelope) | ResponseEnvelope on stdout (status `ok`) or stderr (status `error`/`cancelled`); `--stream` prints NDJSON records |
| CLI `--json` commands | — | ResponseEnvelope whose `operation` is the built-in (`rivet.list`, `rivet.describe`, `rivet.outputs`, `rivet.io`, `rivet.check`, `rivet.policy.generate`, …) |
| `POST /v1/request` | InputEnvelope body | ResponseEnvelope body; with `Accept: text/event-stream`, SSE events carrying records |
| `POST /v1/requests` (polling) | InputEnvelope body | `202` ResponseEnvelope with `status: "accepted"` and the session receipt in `data` |
| `GET /v1/requests/{id}/events` | — | `{session_id, events: [records], last_seq, terminal}` (a session batch, not an envelope) |
| `GET /v1/operations…`, `/v1/io`, `/v1/health` | — | ResponseEnvelope of the matching built-in |
| WebSocket `/v1/ws` (`rivet.v1`) | `{type:"request", ref, operation, data}` | records with `ref` first |
| MCP `tools/call` | tool arguments = operation data; `rivet.request` arguments = InputEnvelope | `result.structuredContent` = ResponseEnvelope; `result.content[0].text` = the same JSON as text |
| Rust `Runtime::call(InputEnvelope)` / `call_json(&str)` | `rivet::InputEnvelope` | `rivet::ResponseEnvelope` |
| C `rivet_request` / `rivet_call_start` | InputEnvelope JSON (+ `pretty`) | ResponseEnvelope JSON / records |

## Request Format

### InputEnvelope

Schema: [`schemas/input.schema.json`](schemas/input.schema.json) (JSON Schema 2020-12).

| Key | Type | Required | Meaning |
|---|---|---|---|
| `operation` | string (non-empty) | yes | Operation ID (`demo.add`, `users.get`, `billing.invoice`, a built-in `rivet.*`) |
| `data` | any (normally an object) | no, default `{}` | The operation's input; `null` means `{}` |
| `deadline_ms` | integer ≥ 0 | no | Requested deadline; clamped to `1..=600000` |
| `restrict` | object `{grants:[…]}` | no | Narrows the policy for this request only (policy.json grant format) |
| `stream` | boolean | no | Ask for streamed records (`rivet request --input`, FFI) |
| `pretty` | boolean | no | C ABI only: indent the returned JSON |
| `id` | string | deprecated | 0.1.0 alias of `operation` (removed in 0.3.0) |
| `params` | any | deprecated | 0.1.0 alias of `data` (removed in 0.3.0) |

Rules enforced by `serve.parse_input` (`src/features/serve/parse_input.rs`):

```text
 body ─┬─ not a JSON object ─────────────────────────────▶ validation.input_envelope  (422 / exit 2)
       ├─ has an `error` key ────────────────────────────▶ validation.input_envelope
       ├─ `operation` + `id`, or `data` + `params` ───────▶ validation.input_envelope  details {key, alias}
       ├─ neither `operation` nor `id` ───────────────────▶ validation.required       details {field:"operation"}
       ├─ wrong type for deadline_ms / stream / pretty ───▶ validation.input_envelope  naming the key
       ├─ `id` or `params` used (alone) ──────────────────▶ accepted + deprecation signal (see below)
       └─ ok ─────────────────────────────────────────────▶ InputEnvelope (unknown keys ignored, as in 0.1.0)
```

Mixing `operation` with `params` (or `id` with `data`) is accepted: each key is checked against its own alias
only. It still counts as legacy input and raises the deprecation signal.

### Deprecation signals (0.2.x)

| Surface | Signal when `id`/`params` (or `--params`) is used |
|---|---|
| HTTP (`/v1/request`, `/v1/requests`, `/mcp` with `rivet.request`) | response header `deprecation: true`; access-log line gains `"deprecated":1` |
| CLI `--params` | stderr `warning[deprecated.params]: --params is deprecated; use --data (removed in 0.3.0)` |
| CLI `--input` with legacy keys | stderr ``warning[deprecated.input]: input keys `id` and `params` are deprecated; use `operation` and `data` (removed in 0.3.0)`` |
| WebSocket request frame | accepted; no per-frame header exists |
| Library / C ABI | accepted (`InputEnvelope::is_legacy()` is true) |

## Response Format

### ResponseEnvelope (unary answer, or the terminal record of a stream)

Schema: [`schemas/response.schema.json`](schemas/response.schema.json).

```text
 ┌ ref          WebSocket frames only, always first
 ├ request_id   "req_…"  ("" when the input never became a request)
 ├ trace_id     "tr_…"   ("" likewise)
 ├ operation    the operation or built-in ID; null only when the input named none (a refused
 │              envelope that names an operation string echoes it)
 ├ type         "result"
 ├ seq          stream terminal records only (the sequence after the last data record) — on every
 │              surface, library `record()` included; every WS ref is a session, so WS terminals have it
 ├ status       "ok" | "error" | "cancelled" | "accepted"
 ├ data         the result (ok), the receipt (accepted) or null
 ├ error        null (ok/accepted) or the error object (error/cancelled)
 ├ effects      "none" | "committed" | "partial" | "unknown"
 └ data_count   number of data records emitted before this one (0 for unary)
```

`data` and `error` are **always present**; exactly one is non-null, except `status: "accepted"` where `data`
is the receipt and `error` is null. Key order is fixed (`serde_json` with `preserve_order`).

```json
{"request_id":"req_011f655065","trace_id":"tr_011f655065","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}
```

### Status and type values

| `status` | Meaning | `data` | `error` | CLI exit | HTTP |
|---|---|---|---|---|---|
| `ok` | The operation returned | result | `null` | 0 | 200 |
| `error` | The operation (or the input) failed | `null` | object | registry (2–6) | registry (400–504) |
| `cancelled` | The request was cancelled (kind `cancelled`) | `null` | object | 130 | 409 |
| `accepted` | A session was opened (polling `POST /v1/requests`, MCP call of a streaming tool) | receipt | `null` | — | 202 |

| `type` | Where | Carries |
|---|---|---|
| `result` | unary answers and the last record of every stream | `status`, `effects`, `data_count` |
| `data` | each emitted item of a stream | `seq`, `data`, `error: null` (no `status`, `effects`, `data_count`) |

### Stream records

Schema: [`schemas/stream-record.schema.json`](schemas/stream-record.schema.json). A stream is zero or more
`type: "data"` records followed by exactly one `type: "result"` record with `seq`:

```text
$ rivet --file docs/demos/01-catalog/app.rivet request demo.countdown --stream
{"request_id":"req_011bdc1285","trace_id":"tr_011bdc1285","operation":"demo.countdown","type":"data","seq":1,"data":3,"error":null}
{"request_id":"req_011bdc1285","trace_id":"tr_011bdc1285","operation":"demo.countdown","type":"data","seq":2,"data":2,"error":null}
{"request_id":"req_011bdc1285","trace_id":"tr_011bdc1285","operation":"demo.countdown","type":"data","seq":3,"data":1,"error":null}
{"request_id":"req_011bdc1285","trace_id":"tr_011bdc1285","operation":"demo.countdown","type":"result","seq":4,"status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}
```

```text
 stream state machine (every surface)
   open ──emit──▶ data(seq=1) ──emit──▶ data(seq=n) ──return──▶ result(seq=n+1, status ok, data_count=n)
     │                    │
     │                    ├── failure ──▶ result(status error,     data null, error{…}, data_count=n)
     │                    └── cancel  ──▶ result(status cancelled, data null, error{kind cancelled}, data_count=n)
     └── rejected before start ──▶ one result record (status error), no data records
```

Without `--stream`, the CLI prints only the terminal record of a streaming operation (with `data_count`).

## Error Format

The error object sits at `error` and keeps the 0.1.0 fields; `effects` moved from the error to the top level.

| Key | Always | Meaning |
|---|---|---|
| `kind` | yes | One of the 22 kinds of [API-2026-0005](api-2026-0005-error-registry.md); fixes the exit code and HTTP status |
| `code` | yes | Dotted code (`validation.type`, `check.global_shadow`, `unsupported.feature`, …) |
| `message` | yes | Human text; not for matching |
| `retryable` | yes | `true` only for kind `limit` unless a code overrides it |
| `operation_id` | when known | The operation that raised it |
| `node_id` | DAG failures | The DAG node |
| `details` | per code | Structured context (`field`, `feature`, `key`/`alias`, …) |
| `source` | compile/check errors | `{file, line, column, end_line, end_column}` |
| `hint` | some codes | A suggested fix |
| `cause` | wrapped errors | The underlying error object |
| `suppressed` | multi-error checks | Further diagnostics, source order |

`effects` never appears inside the error object, nor inside a nested `cause` or `suppressed[]` error: it is reported
once, at the envelope's top level.

```text
$ rivet --file docs/demos/01-catalog/app.rivet request demo.add --data '{"a":"two"}' --pretty
{
  "request_id": "req_011e33d5fd",
  "trace_id": "tr_011e33d5fd",
  "operation": "demo.add",
  "type": "result",
  "status": "error",
  "data": null,
  "error": {
    "kind": "validation",
    "code": "validation.type",
    "message": "parameter `a` must be an integer, got text",
    "retryable": false,
    "operation_id": "demo.add",
    "details": {
      "field": "a"
    }
  },
  "effects": "none",
  "data_count": 0
}
$ echo $?
2
```

An input that never became a request has empty IDs and, when it named no operation, `operation: null`. When the
refused envelope names an operation string, `operation` echoes it (e.g. a wrong-typed `deadline_ms`:
`{"request_id":"","trace_id":"","operation":"demo.add",…,"error":{…,"code":"validation.input_envelope","message":"\`deadline_ms\` must be a non-negative integer",…,"operation_id":"demo.add"},…}`):

```json
{"request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.input_envelope","message":"use `operation` or the deprecated `id`, not both","retryable":false,"details":{"key":"operation","alias":"id"}},"effects":"none","data_count":0}
```

### Pretty output

| Surface | Switch | Refused with |
|---|---|---|
| CLI | `--pretty` | `--stream` → `validation.usage` (exit 2) |
| HTTP | `?pretty=true` | `Accept: text/event-stream` → `400 validation.pretty_stream` |
| Rust | `envelope.to_json_pretty()` (compact: `to_json_string()`) | — |
| C ABI | `"pretty": true` in the input, or in the runtime options | — (each FFI record is its own string) |

Pretty output is 2-space indented with the same key order. Real refusals:

```text
$ rivet --file docs/demos/01-catalog/app.rivet request demo.countdown --stream --pretty
{ … "status": "error", … "code": "validation.usage",
    "message": "--pretty cannot be used with --stream: NDJSON records must stay one per line", … }     (exit 2)

$ curl -s -i 'http://127.0.0.1:18951/v1/request?pretty=true' -H 'content-type: application/json' \
    -H 'accept: text/event-stream' -d '{"operation":"demo.countdown","data":{}}'
HTTP/1.1 400 Bad Request
{ … "operation": "demo.countdown", … "code": "validation.pretty_stream",
    "message": "pretty JSON cannot be used with an event stream (Accept: text/event-stream); drop ?pretty=true", … }
```

## Rate Limits

Unchanged from 0.1.0 (policy `limits`, session limits). Limit errors are envelopes with kind `limit`,
`retryable: true`, HTTP 429 and exit 5.

## Versioning and Deprecation

```text
 0.1.x ─────────────── 0.2.0 (this) ──────────────────────── 0.2.x ─────────────── 0.3.0
 {id, params} input     {operation, data} canonical           same                   id/params refused
 {"result":…} output    envelopes everywhere                  same                   (validation.input_envelope
 event: error (SSE)     id/params accepted + Deprecation      same                    with a hint)
                        --params accepted + warning           same                   --params removed
```

## Examples

### Per-surface wrapping

```text
 HTTP  200 {"request_id":"req_01b1f0b3fd","trace_id":"tr_01b1f0b3fd","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}

 SSE   id: 4
       event: result                       (event name = record type: `data` or `result`)
       data: {"request_id":"req_042cd2bd4c",…,"type":"result","seq":4,"status":"ok","data":{"count":3},"error":null,"effects":"none","data_count":3}

 WS    {"ref":"c1","request_id":"req_13f45ac261","trace_id":"tr_13f45ac261","operation":"demo.countdown","type":"data","seq":1,"data":3,"error":null}

 MCP   {"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"{\"request_id\":\"req_03c07007e7\",…}"}],
        "structuredContent":{"request_id":"req_03c07007e7","trace_id":"tr_03c07007e7","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0},
        "isError":false}}          (isError is true for status error AND cancelled)

 Poll  202 {"request_id":"req_1165fcfbc7",…,"operation":"demo.countdown","type":"result","status":"accepted",
            "data":{"session_id":"ses_016285bebd",…,"events_url":"/v1/requests/ses_016285bebd/events"},"error":null,"effects":"none","data_count":0}

 C     rivet_request(rt, "{\"operation\":\"demo.add\",\"data\":{\"a\":2,\"b\":3}}")
       → {"request_id":"req_01aaa1efc5","trace_id":"tr_01aaa1efc5","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}
```

### Reading an envelope (any language)

```text
 env = parse(json)
 switch env.status:
   "ok"        → use env.data
   "accepted"  → env.data is a session receipt (poll events_url / rivet.sessions.read)
   "error"     → env.error.kind / env.error.code ; retry only if env.error.retryable
   "cancelled" → env.error.code (cancelled.*) ; env.data_count items arrived before
 streams: loop over records until type == "result"; items are the records with type == "data"
```

```sh
# jq: the result, or a one-line error
rivet --file app.rivet request demo.add --data '{"a":2,"b":3}' 2>&1 \
  | jq -r 'if .status == "ok" then .data else "\(.error.code): \(.error.message)" end'
```

## Compatibility Notes

- Output is a **breaking change** from 0.1.0 on every surface; input is backwards compatible through 0.2.x.
  [MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md) maps each old shape.
- Polling sub-routes (`…/events`, `…/input`, `…/finish_input`, `…/cancel`) keep their session bodies
  (`{session_id, events, last_seq, terminal}`, acks, cancel receipts); their records are envelopes.
- The script-level `completion` object of a request stream inside `.rivet` keeps its 0.1.0 shape (language API).
- The Rust `Scope::stream` terminal record rendered with `record()` carries `seq` (= `data_count + 1`), like the
  CLI, SSE, WebSocket, polling and C ABI terminal records (fixed in 0.2.0, INC-2026-0012).
- Protocol-level refusals that request no operation (MCP `mcp.session_required`, `not_found.mcp_session`,
  `mcp.protocol_version`, a refused Origin) have `operation: null`; MCP `rivet.request` errors name the requested
  operation, like its successes.

## Related Documents

- [PROP-2026-0002](../proposals/implemented/prop-2026-0002-envelopes-globals-library-ffi-highlighting.md) R1–R6
- [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) D-10
- [MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md) — migrating 0.1.0 clients
- [API-2026-0001](api-2026-0001-http-rest-sse-polling.md), [API-2026-0002](api-2026-0002-websocket-rivet-v1.md),
  [API-2026-0003](api-2026-0003-mcp-server-tools.md), [API-2026-0004](api-2026-0004-rust-library.md),
  [API-2026-0005](api-2026-0005-error-registry.md), [API-2026-0007](api-2026-0007-c-abi.md)
- Schemas: [response](schemas/response.schema.json), [input](schemas/input.schema.json),
  [stream record](schemas/stream-record.schema.json)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-29 | Claude | Initial envelope reference (TASK-071, D-10) with real 0.2.0-rc captures on every surface |
| 2 | 2026-09-29 | Claude | INC-2026-0012: `seq` on library terminal records and WS refused-input terminals; refused envelopes echo a named operation; no `effects` in nested errors; MCP protocol refusals `operation: null`, `rivet.request` errors name the target. |
