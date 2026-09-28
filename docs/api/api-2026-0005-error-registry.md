---
document_id: API-2026-0005
title: "Rivet error registry"
document_type: api
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [execution, language, policy, serve, http, ws, mcp, poll, cli, library, transports, files, connectors, auth, grpc, quic, datagrams, sessions]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, embedded, server]
audience: [developers, integrators, operators]
scope: Every error kind with its HTTP status, CLI exit code and retryability, and every error code the 0.1.0 source emits, grouped by kind; plus the CLI-only exit codes of `io` and `policy generate`.
reason: DOCUMENTATION.md §31 API impact for PLAN-2026-0001 row D-28 (test T-24); one registry is shared by every surface and callers need the complete list.
related_documents: [PLAN-2026-0001, PROP-2026-0001, API-2026-0001, API-2026-0002, API-2026-0003, API-2026-0004]
supersedes: null
superseded_by: null
tags: [rivet, api, errors, registry, exit-codes]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.1.0-dev (commit f40d4aa)"
---

# Rivet error registry

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** every surface and feature

## Summary

Rivet has **one** error model (`rivet::domain::RivetError`, `src/domain/errors.rs`). Every error has a **kind** (22 fixed values) and a dotted **code**. The kind alone decides the CLI exit code, the HTTP status and the default `retryable` flag, so the same failure looks the same on the CLI, REST, SSE, polling, WebSocket, MCP and the library.

```text
                        RivetError { kind, code, message, retryable, effects, source?, details?, … }
                                           │
          ┌───────────────┬────────────────┼────────────────┬──────────────────┬───────────────┐
          ▼               ▼                ▼                ▼                  ▼               ▼
   CLI: exit code   HTTP: status +   SSE: event:error  WS: {type:error}  MCP: isError:true  library:
   + rendered text  ErrorEnvelope    ErrorEnvelope     frame             structuredContent  Err(RivetError)
   (error[code]:…)  {request_id,     with seq          {ref, error}      = ErrorEnvelope
                    trace_id, error}
```

## Audience and Stability

Everyone handling Rivet failures. **Kinds** and their mappings are stable for 0.1.x. **Codes** are stable identifiers: new codes may be added in any release; an existing code keeps its kind. Match on `kind` for control flow and on `code` for specific handling. Operation-declared application codes (`error CODE "…"` in a `.rivet` file, raised with `fail`) belong to the bundle author.

## Authentication

Not applicable — this is a reference of error values. Authentication failures are kind `auth` (401, exit 3).

## Endpoints or Events

### Kinds → HTTP → exit → retryable

| Kind | HTTP | Exit | Retryable | Meaning |
|---|---|---|---|---|
| `syntax` | 422 | 2 | no | Source does not parse / lower |
| `validation` | 422 | 2 | no | Bad input, options, config, frames (`validation.malformed_json` is **400**) |
| `auth` | 401 | 3 | no | Serve authentication missing/invalid |
| `permission` | 403 | 3 | no | Policy broker or principal map refused |
| `not_found` | 404 | 4 | no | Unknown/hidden operation, file, session, route … |
| `conflict` | 409 | 4 | no | State conflict (exists, version, sequence, OAuth state) |
| `limit` | 429 | 5 | **yes** | A bounded resource is exhausted |
| `timeout` | 504 | 6 | no | A deadline expired |
| `cancelled` | 409 | 130 | no | Cancelled by caller, disconnect or session end |
| `connection` | 502 | 5 | no | Dial / socket / bind failure |
| `dns` | 502 | 5 | no | Resolution failure |
| `tls` | 502 | 5 | no | TLS configuration or handshake |
| `http` | 502 | 5 | no | Non-accepted HTTP status from a remote |
| `protocol` | 502 | 5 | no | Peer broke a protocol contract |
| `process` | 502 | 5 | no | Child process failed |
| `parse` | 502 | 5 | no | Undecodable payload (JSON, UTF-8) |
| `application` | 502 | 5 | no | Operation-declared failure (`fail`), MCP tool error, auth store/endpoint |
| `output_invalid` | 500 | 5 | no | Result does not match the declared `output` |
| `consumer_failed` | 500 | 5 | no | The caller's data sink failed |
| `cleanup` | 500 | 5 | no | A scoped resource failed to close |
| `unsupported` | 501 | 5 | no | Feature/platform not available in this build |
| `internal` | 500 | 5 | no | Bug or unexpected host error |

`retryable` defaults from the kind (only `limit` is `true`); no 0.1.0 code overrides it. An error decoded from a remote server keeps the server's value.

### CLI exit codes

```text
  0   ok
  2   syntax · validation · config        (e.g. serve.auth_required, policy.invalid, validation.input)
  3   auth · permission                    also: io --check-policy with any denied/partial site,
                                                 io --check-files with not_permitted/unreadable files
  4   not_found · conflict                 also: io --check-files with missing files (nothing worse)
  5   limit · connection · dns · tls · http · protocol · process · parse · application ·
      output_invalid · consumer_failed · cleanup · unsupported · internal
  6   timeout
  7   inspection incomplete                io --strict with dynamic/opaque sites;
                                           policy generate with review items (the draft is still produced)
130   cancelled                            includes Ctrl-C
```

## Request Format

Not applicable.

## Response Format

The `error` object (inside an ErrorEnvelope, a WS `error` frame, an SSE `error` event or an MCP error result):

| Field | Always | Notes |
|---|---|---|
| `kind` | yes | one of the 22 kinds |
| `code` | yes | dotted code below |
| `message` | yes | human text; never contains secrets |
| `retryable` | yes | boolean |
| `effects` | yes | `none` · `committed` · `partial` · `unknown` — what may already have happened outside |
| `source` | no | `{file, start_line, start_col, end_line, end_col}` |
| `operation_id`, `node_id` | no | where it failed (DAG node) |
| `hint` | no | suggested fix |
| `details` | no | structured context (`field`, `capability`, `target`, `nodes` …) |
| `cause` | no | nested error |
| `suppressed` | no | errors hidden by this one (e.g. cleanup failures) |

## Error Format

### Codes by kind

The lists below were extracted from `src/` at commit `f40d4aa` (every literal passed with a kind, directly or through a kind-fixed helper). Where one code appears under two kinds, both sites are real.

**syntax (422, exit 2)** — compile time, `rivet check` and bundle load:
`syntax.unparsed`, `syntax.unknown_statement`, `syntax.indent`, `syntax.top_level`, `syntax.statement`, `syntax.trailing`, `syntax.fcall_style`, `syntax.expression`, `syntax.character`, `syntax.number`, `syntax.string`, `syntax.escape`, `syntax.interpolation`, `syntax.duration`, `syntax.duration_unquoted`, `syntax.type`, `syntax.fields`, `syntax.field_modifier`, `syntax.duplicate_field`, `syntax.param_modifier`, `syntax.duplicate_param`, `syntax.duplicate_output`, `syntax.duplicate_error`, `syntax.output_required`, `syntax.operation_id`, `syntax.reserved_id`, `syntax.header_order`, `syntax.option_expected`, `syntax.option_after_body`, `syntax.option_misplaced`, `syntax.group_option`, `syntax.http`, `syntax.file`, `syntax.with`, `syntax.secret`, `syntax.try_without_catch`, `syntax.catch_without_try`, `syntax.catch`, `syntax.map`, `syntax.map_yield`, `syntax.poll`, `syntax.until`, `syntax.iterate`, `syntax.scope`, `syntax.concurrent`, `syntax.dag`, `syntax.dag_cycle`, `syntax.break`, `syntax.yield`;
checks: `check.unknown_operation`, `check.unknown_connector`, `check.unknown_auth_profile`, `check.call_cycle`, `registry.duplicate_id`;
documentation rules: `docs.description`, `docs.param_description`, `docs.output_description`, `docs.field_description`, `docs.undeclared_error`.

**validation (422, exit 2)**
- request & params: `validation.required`, `validation.type`, `validation.unknown_field`, `validation.min`, `validation.max`, `validation.enum`, `validation.params`, `validation.usage`, `validation.query`, `validation.output`, `validation.argument`, `validation.option`, `validation.duration`, `validation.codec`, `validation.body`, `validation.malformed_json` (**HTTP 400**)
- surfaces & sessions: `stream.required`, `stream.input_required`, `stream.emits_undeclared`, `validation.frame`, `validation.subprotocol`, `validation.input`, `validation.no_input`, `validation.endpoint`, `validation.check_files_remote`, `mcp.session_required`, `mcp.protocol_version`
- config: `policy.invalid`, `serve.auth_required`
- runtime values (raised with the statement's span): `value.type`, `value.missing_key`, `value.overflow`, `value.division_by_zero`, `value.not_iterable`, `value.handle`, `file.form`, `syntax.map_yield`
- files: `file.codec`, `file.to`
- HTTP client: `validation.url`, `validation.url_segment`, `validation.http_option`, `validation.http_stream`, `validation.http_version`, `validation.http_request`, `validation.http_retry_unsafe`, `validation.tls`, `syntax.http`
- sockets & processes: `validation.socket_option`, `validation.socket_send`, `validation.socket_receive`, `validation.framing`, `validation.frame_delimiter`, `syntax.with`, `validation.command_option`, `validation.command_stream`, `validation.process_program`, `syntax.command`
- UDP / QUIC: `udp.connected`, `udp.no_peer`, `validation.udp_address`, `validation.udp_option`, `quic.alpn_required`, `quic.datagrams_disabled`, `quic.direction`, `quic.stream_finished`, `validation.quic_endpoint`, `validation.quic_option`, `framing.delimiter_in_payload`
- gRPC: `grpc.connector`, `grpc.descriptor`, `grpc.endpoint`, `grpc.form`, `grpc.invalid_argument` (status 3), `grpc.invalid_message`, `grpc.metadata`, `grpc.method`, `grpc.mode`, `grpc.option`, `grpc.send`, `grpc.unknown_connector`, `grpc.unknown_method`, `grpc.unknown_service`
- MCP connectors: `mcp.connector`, `mcp.snapshot`, `mcp.snapshot_missing`, `mcp.snapshot_unapproved`, `mcp.alias_collision`, `mcp.unknown_import`, `mcp.unknown_tool`, `mcp.unknown_resource`, `mcp.unknown_prompt`, `mcp.auth_transport`, `validation.mcp_params`
- OAuth: `validation.auth_profile`, `validation.auth_flow`, `validation.auth_callback`, `validation.auth_account`, `validation.auth_scope`

**auth (401, exit 3)**: `auth.required`, `auth.invalid`.

**permission (403, exit 3)**: `permission.denied` (policy broker, `serve.principals`, MCP Origin, symlink refused), `permission.os`, `file.hardlink_refused`, `auth.origin_not_bound`, `grpc.permission_denied` (status 7).

**not_found (404, exit 4)**: `not_found.operation`, `not_found.route`, `not_found.request`, `not_found.session`, `not_found.ref`, `not_found.mcp_session`, `not_found.trace`, `not_found.source`, `not_found.file`, `file.root`, `not_found.env`, `not_found.program`, `not_found.descriptor`, `not_found.mcp_connector`, `not_found.mcp_snapshot`, `not_found.mcp_resource`, `not_found.mcp_prompt`, `not_found.auth_profile`, `not_found.auth_transaction`, `grpc.not_found` (status 5).

**conflict (409, exit 4)**: `conflict.already_exists`, `conflict.exists`, `conflict.version`, `conflict.ref`, `conflict.cursor`, `stream.cursor_expired`, `conflict.input_sequence`, `conflict.input_closed`, `conflict.session_terminal`, `grpc.input_closed`, `auth.login_required` (also gRPC status 16), `auth.access_denied`, `auth.transaction_expired`, `auth.insufficient_scope`, `auth.callback_invalid`, `auth.refresh_uncertain`.

**limit (429, exit 5, retryable)**: `limit.concurrency`, `limit.call_depth`, `limit.sessions`, `limit.input_queue`, `limit.ws_refs`, `limit.response_body`, `limit.http_body`, `limit.http_redirects`, `limit.file_size`, `limit.frame`, `limit.stream_item`, `limit.process_output`, `limit.quic_streams`, `limit.mcp_hops`, `limit.mcp_recursion`, `limit.mcp_pages`, `limit.mcp_message`, `limit.auth_transactions`, `grpc.resource_exhausted` (status 8).

**timeout (504, exit 6)**: `timeout` (UDP/QUIC receive), `timeout.request`, `timeout.scope`, `timeout.dag`, `timeout.concurrent`, `timeout.poll`, `timeout.stream`, `timeout.http`, `timeout.connect`, `timeout.receive`, `timeout.process`, `timeout.mcp`, `timeout.auth_token`, `quic.idle_timeout`, `grpc.deadline_exceeded` (status 4).

**cancelled (409, exit 130)**: `cancelled.request`, `cancelled.runtime`, `cancelled.session`, `cancelled.idle`, `cancelled.disconnect`, `cancelled.consumer`, `cancelled.socket`, `cancelled.grpc`, `grpc.cancelled` (status 1).

**connection (502, exit 5)**: `connection.failed`, `connection.refused`, `connection.timeout`, `connection.http`, `connection.http_body`, `connection.http3`, `connection.udp`, `connection.io`, `connection.websocket`, `connection.mcp_closed`, `connection.endpoint`, `connection.bind`, `connection.serve`, `udp.socket_failed`, `udp.bind_failed`, `udp.connect_failed`, `udp.send_failed`, `udp.receive_failed`, `quic.connect`, `quic.connection`, `quic.socket`, `quic.closed`.

**dns (502, exit 5)**: `dns.resolve`, `dns.no_address`.

**tls (502, exit 5)**: `tls.config`, `tls.handshake`, `tls.pem`, `quic.tls`, `quic.tls_config`, `quic.alpn_mismatch`.

**http (502, exit 5)**: `http.status` (a response status not in `accept status`).

**protocol (502, exit 5)**: `protocol.endpoint`, `protocol.http_location`, `protocol.websocket_handshake`, `protocol.unexpected_eof`, `protocol.unsupported_capability`, `protocol.mcp_message`, `protocol.mcp_initialize`, `protocol.mcp_version`, `protocol.mcp_cancelled`, `protocol.mcp_error`, `protocol.mcp_capability`, `protocol.mcp_result`, `protocol.mcp_output_schema`, `protocol.mcp_schema_changed`, `http.version_unavailable`, `auth.token_endpoint_failed` (malformed token response), `framing.truncated`, `udp.truncated`, `udp.message_too_large`, `quic.transport`, `quic.stream`, `quic.stream_reset`, `quic.stream_finished`, `quic.stop_sending`, `quic.frame_too_large`, `quic.datagram_too_large`, `quic.datagrams_unavailable`, `grpc.decode`, `grpc.cardinality`, `grpc.missing_message`, and `grpc.<status_name>` for every other non-OK gRPC status (e.g. `grpc.unavailable`, `grpc.internal`).

**process (502, exit 5)**: `process.exit` (status outside `accept exit`), `process.spawn`, `process.not_executable`, `process.io`.

**parse (502, exit 5)**: `parse.json`, `parse.utf8`.

**application (502, exit 5)**: codes declared by the operation and raised with `fail CODE {details}`; `mcp.tool_failed`; `auth.token_endpoint_failed` (error status from the token endpoint), `auth.client_secret_missing`, `auth.store_failed`.

**output_invalid (500, exit 5)**: `output.invalid`.

**consumer_failed (500, exit 5)**: `consumer_failed`.

**cleanup (500, exit 5)**: `cleanup.failed`, `cleanup.timeout` (the 5 s close budget), `cleanup.closed` (use of a closed handle).

**unsupported (501, exit 5)**: `unsupported.serve_mtls`, `unsupported.sandbox_backend`, `unsupported.shell`, `unsupported.interactive`, `unsupported.process_duplex`, `unsupported.command`, `unsupported.adapter`, `unsupported.method`, `unsupported.property`, `unsupported.form`, `unsupported.handle`, `unsupported.child`, `unsupported.iterate`, `unsupported.transport`, `unsupported.unix`, `unsupported.tcp_tls`, `unsupported.reconnect`, `unsupported.udp_option`, `unsupported.quic_early_data`, `unsupported.quic_migration`, `unsupported.oauth`, `unsupported.auth`, `unsupported.auth_flow`, `unsupported.auth_kind`, `unsupported.auth_method`, `unsupported.token_type`, `unsupported.credential_store`, `grpc.unimplemented` (status 12).

**internal (500, exit 5)**: `internal`, `file.io` (unexpected OS error).

### gRPC status mapping

```text
  gRPC status  1 CANCELLED          → cancelled   grpc.cancelled
               3 INVALID_ARGUMENT   → validation  grpc.invalid_argument
               4 DEADLINE_EXCEEDED  → timeout     grpc.deadline_exceeded
               5 NOT_FOUND          → not_found   grpc.not_found
               7 PERMISSION_DENIED  → permission  grpc.permission_denied
               8 RESOURCE_EXHAUSTED → limit       grpc.resource_exhausted
              12 UNIMPLEMENTED      → unsupported grpc.unimplemented
              16 UNAUTHENTICATED    → conflict    auth.login_required
              any other             → protocol    grpc.<status name, lower case>
```

## Rate Limits

Not applicable; see kind `limit`.

## Versioning and Deprecation

Codes are append-only within 0.1.x. Nothing is deprecated.

## Examples

```text
$ curl -s -X POST http://127.0.0.1:18431/v1/request -d '{"id":"demo.add","params":{"a":"x"}}'      # HTTP 422
{"request_id":"req_02062fbf4a","trace_id":"tr_02062fbf4a","error":{"kind":"validation","code":"validation.type","message":"parameter `a` must be an integer, got text","retryable":false,"effects":"none","operation_id":"demo.add","details":{"field":"a"}}}

$ rivet --file app.rivet serve --listen 0.0.0.0:18434                                             # exit 2
{"request_id":"","trace_id":"","error":{"kind":"validation","code":"serve.auth_required","message":"non-loopback listener requires serve.auth in policy.json","retryable":false,"effects":"none"}}

WS  {"type":"error","ref":"h8","error":{"kind":"limit","code":"limit.ws_refs","message":"at most 8 refs may be in flight per connection","retryable":true,"effects":"none"}}

library: error validation.required kind=validation exit=2 http=422
```

Captured on commit `f40d4aa`; see [API-2026-0001](api-2026-0001-http-rest-sse-polling.md), [API-2026-0002](api-2026-0002-websocket-rivet-v1.md) and [API-2026-0004](api-2026-0004-rust-library.md) for the full sessions.

## Compatibility Notes

- An unknown kind received from a remote server decodes as `internal` (`RivetError::from_value`), so an older CLI talking to a newer server still exits non-zero.
- The CLI renders errors as `error[CODE]: message` with a `--> file:line:col` caret excerpt and `= hint:` when present; `--json` prints the ErrorEnvelope instead.

## Related Documents

- [API index](index.md) · [HTTP API](api-2026-0001-http-rest-sse-polling.md) · [WebSocket](api-2026-0002-websocket-rivet-v1.md) · [MCP](api-2026-0003-mcp-server-tools.md) · [Library](api-2026-0004-rust-library.md)
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) · [PROP-2026-0001](../proposals/approved/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial registry: 22 kinds and every code emitted by the source at commit f40d4aa. |
