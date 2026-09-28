---
document_id: API-2026-0005
title: "Rivet error registry"
document_type: api
status: active
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 3
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
scope: Every error kind with its HTTP status, CLI exit code and retryability, and every error code the source emits, grouped by kind — including the 0.2.0 envelope, globals, modules, Cargo-feature and C ABI codes; the `check` and deprecation warnings; the error object inside the 0.2.0 envelope; plus the CLI-only exit codes of `io`, `policy generate` and `policy explain --params`.
reason: DOCUMENTATION.md §31 API impact for PLAN-2026-0001 row D-28 (test T-24) and PLAN-2026-0002 row D-44 (new 0.2.0 codes, TASK-070); one registry is shared by every surface and callers need the complete list.
related_documents: [PLAN-2026-0001, PLAN-2026-0002, PROP-2026-0001, PROP-2026-0002, API-2026-0001, API-2026-0002, API-2026-0003, API-2026-0004, API-2026-0006, API-2026-0007, MIG-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, api, errors, registry, exit-codes]
confidentiality: internal
review_cycle: on-release
last_verified_version: "0.2.0-rc (source at 6f9943f)"
next_review_date: 2026-10-29
---

# Rivet error registry

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
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
   CLI: exit code   HTTP: status +   SSE: event:result WS: record        MCP: isError:true  Rust: call →
   + rendered text  envelope         status "error"    {ref,…,status:    structuredContent  envelope;
   (error[code]:…)  status "error"   (after start) or  "error", error}   = envelope         request → Err
   --json: envelope data null,       plain envelope                                         C ABI: envelope
                    error {…}        (before start)                                          JSON string

   every surface from 0.2.0: {request_id, trace_id, operation, type:"result", status:"error"|"cancelled",
                              data:null, error:{kind, code, message, retryable, …}, effects, data_count}
```

## Audience and Stability

Everyone handling Rivet failures. **Kinds** and their mappings are stable (unchanged from 0.1.0 through 0.2.x). **Codes** are stable identifiers: new codes may be added in any release; an existing code keeps its kind. Match on `kind` for control flow and on `code` for specific handling. Operation-declared application codes (`error CODE "…"` in a `.rivet` file, raised with `fail`) belong to the bundle author.

## Authentication

Not applicable — this is a reference of error values. Authentication failures are kind `auth` (401, exit 3).

## Endpoints or Events

### Kinds → HTTP → exit → retryable

| Kind | HTTP | Exit | Retryable | Meaning |
|---|---|---|---|---|
| `syntax` | 422 | 2 | no | Source does not parse / lower |
| `validation` | 422 | 2 | no | Bad input, options, config, frames (`validation.malformed_json` and `validation.pretty_stream` are **400**) |
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

`retryable` defaults from the kind (only `limit` is `true`); no code overrides it (0.2.0's `limit.imports` is retryable by the kind rule, although retrying an unchanged bundle cannot succeed). An error decoded from a remote server keeps the server's value.

### Warnings (never fatal)

`rivet check` and the CLI print warnings on stderr and still exit 0; they use the same rendering as errors, prefixed `warning`.

| Code | When | Becomes an error |
|---|---|---|
| `docs.undeclared_error` | a `fail "CODE"` whose operation has no `error "CODE" …` line | with `--strict-docs` (exit 2, reported once) |
| `check.unguarded_result` | `return` reads `NODE.result` of a `fail independent` DAG node with no enclosing `if NODE.status …` | never |
| `check.module_policy_ignored` (0.2.0) | a `policy.json` sits beside an imported module; the module runs under the loader's policy (registered as kind `syntax` but only ever emitted as a warning; `rivet check --json` counts it in `data.warnings`) | never |
| `deprecated.params` (0.2.0, CLI) | `rivet request --params` was used (stderr `warning[deprecated.params]: --params is deprecated; use --data (removed in 0.3.0)`) | in 0.3.0 (flag removed) |
| `deprecated.input` (0.2.0, CLI) | `rivet request --input` read an envelope with `id`/`params` | in 0.3.0 (`validation.input_envelope`) |

Missing descriptions are **not** warnings: they are reported only by `check --strict-docs` (`docs.description`, `docs.param_description`, `docs.output_description`, `docs.field_description`, exit 2).

```text
$ rivet --file app.rivet check                                  # captured on commit 829ca43
warning[docs.undeclared_error]: `demo.oops` can fail with `demo.undeclared` but declares no `error "demo.undeclared"` line
  --> app.rivet:31:5
   |
 31|     fail "demo.undeclared" {why: "demo"}
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
warning[check.unguarded_result]: `a.result` is null unless `a` succeeded; guard it with `if a.status == "succeeded"`
  --> app.rivet:41:5
   |
 41|     return a.result
   |     ^^^^^^^^^^^^^^^
ok: 6 operations, 0 connectors, 0 auth profiles                  # exit 0

$ rivet --file app.rivet check                                  # 0.2.0: a module with its own policy.json
warning[check.module_policy_ignored]: the policy.json beside sub/lib.rivet is ignored: module `lib` runs under the loader's policy
  --> app.rivet:1:1
   |
  1| import "./sub/lib.rivet" as lib public
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  = hint: grant what the module needs in the entry bundle's policy.json (or the host policy)
ok: 2 operations, 0 connectors, 0 auth profiles                  # exit 0
```

### CLI exit codes

```text
  0   ok
  2   syntax · validation · config        (e.g. serve.auth_required, policy.invalid, validation.input,
                                           validation.input_envelope, check.global_*, check.import_*)
  3   auth · permission                    (e.g. permission.import_outside_root) also: io --check-policy with any denied/partial site,
                                                 io --check-files with not_permitted/unreadable files,
                                                 policy explain ID --params … when a concrete target is denied
  4   not_found · conflict                 (e.g. not_found.import) also: io --check-files with missing files (nothing worse)
  5   limit · connection · dns · tls · http · protocol · process · parse · application ·
      output_invalid · consumer_failed · cleanup · unsupported · internal
                                           (e.g. unsupported.feature, limit.imports, internal.panic)
  6   timeout
  7   inspection incomplete                io --strict with dynamic/opaque sites;
                                           policy generate with review items (the draft is still produced)
130   cancelled                            includes Ctrl-C
```

## Request Format

Not applicable.

## Response Format

The `error` object sits at `error` inside every 0.2.0 envelope with `status` `error` or `cancelled` (HTTP body, SSE `event: result`, polling event, WebSocket record, MCP `structuredContent`, `rivet --json`, the C ABI); see [API-2026-0006](api-2026-0006-envelopes.md#error-format):

| Field | Always | Notes |
|---|---|---|
| `kind` | yes | one of the 22 kinds |
| `code` | yes | dotted code below |
| `message` | yes | human text; never contains secrets |
| `retryable` | yes | boolean |
| `effects` | moved | From 0.2.0 at the **envelope's top level**, not in `error` (`none` · `committed` · `partial` · `unknown`). Nested `suppressed[]` and `cause` objects still carry their own `effects` key. |
| `source` | no | `{file, line, column, end_line, end_column}` (1-based) |
| `operation_id`, `node_id` | no | where it failed (DAG node) |
| `hint` | no | suggested fix |
| `details` | no | structured context (`field`, `capability`, `target`, `pointer`, `secret`, `seq`, `nodes`, 0.2.0 `feature`, `key`/`alias` …) |
| `cause` | no | nested error |
| `suppressed` | no | errors hidden by this one (e.g. cleanup failures) |

## Error Format

### Codes added in 0.2.0

Registered in `src/domain/errors.rs` (`LANGUAGE_CODES` for the language, module, feature and FFI codes, with a unit
test of every row) and in `serve.parse_input` / the CLI for the envelope codes. Kind fixes the row, as for every code.

| Code | Kind | HTTP | Exit | Retryable | Raised when | Details |
|---|---|---|---|---|---|---|
| `validation.input_envelope` | validation | 422 | 2 | no | an input is not a JSON object, has an `error` key, mixes a key with its alias (`operation`+`id`, `data`+`params`) or has a wrong-typed `deadline_ms`/`stream`/`pretty` | `details {key, alias}` for a mix |
| `validation.pretty_stream` | validation | **400** | 2 | no | HTTP `?pretty=true` with `Accept: text/event-stream` | — |
| `validation.usage` (new uses) | validation | 422 | 2 | no | CLI `--pretty` with `--stream`; `--data` with `--params` | — |
| `syntax.global` | syntax | 422 | 2 | no | a malformed `global NAME = EXPR` line | — |
| `check.global_not_constant` | syntax | 422 | 2 | no | a global uses `env`, `secret`, `request`, an effect, a param or a local | `hint` |
| `check.global_forward_ref` | syntax | 422 | 2 | no | a global reads a later global | — |
| `check.global_duplicate` | syntax | 422 | 2 | no | two globals share a name | — |
| `check.global_shadow` | syntax | 422 | 2 | no | a param, loop variable, map item, `with … as`, DAG node, task or secret reuses a global's name | `hint` |
| `check.global_assign` | syntax | 422 | 2 | no | `NAME = …` / `NAME += …` on a global | — |
| `syntax.import` | syntax | 422 | 2 | no | a malformed `import … as ALIAS [public]`, or an import after the first declaration | — |
| `not_found.import` | not_found | 404 | 4 | no | the imported file cannot be read | — |
| `permission.import_outside_root` | permission | 403 | 3 | no | an import (or `rt.load`) resolves outside the runtime root | — |
| `check.import_cycle` | syntax | 422 | 2 | no | files import each other in a cycle | — |
| `check.import_duplicate` | syntax | 422 | 2 | no | an alias is imported twice; `rt.load` of an alias already loaded | — |
| `check.import_collision` | syntax | 422 | 2 | no | a namespaced ID, connector or auth profile collides across files | — |
| `limit.imports` | limit | 429 | 5 | yes (kind rule) | more than 256 files or an import depth over 16 | — |
| `check.module_policy_ignored` | syntax | — | — | — | **warning only** (see above) | `hint` |
| `unsupported.feature` | unsupported | 501 | 5 | no | the bundle uses gRPC, QUIC/HTTP/3 or OAuth, or `rivet serve` runs, in a build without that Cargo feature (every use listed: first + `suppressed`) | `details.feature` (`grpc`, `quic`, `oauth`, `serve`) |
| `validation.ffi_argument` | validation | 422 | 2 | no | C ABI: a NULL, non-UTF-8, non-JSON or freed argument ([API-2026-0007](api-2026-0007-c-abi.md)) | — |
| `internal.panic` | internal | 500 | 5 | no | C ABI: a Rust panic caught at the boundary | — |

```text
 where each new code can surface
   compile / load ── syntax.global · check.global_* · syntax.import · check.import_* · not_found.import ·
                     permission.import_outside_root · limit.imports · unsupported.feature · (warning) module_policy_ignored
   input parsing  ── validation.input_envelope · validation.pretty_stream (HTTP) · validation.usage (CLI)
   C boundary     ── validation.ffi_argument · internal.panic
```

### Codes by kind

The lists below were extracted from `src/` at commit `829ca43` (every literal passed with a kind, directly or through a kind-fixed helper) and diffed against the previous revision; codes added since `f40d4aa` are marked **(new)**. Where one code appears under two kinds, both sites are real.

**syntax (422, exit 2)** — compile time, `rivet check` and bundle load (0.2.0: `syntax.global`, `syntax.import`, `check.global_not_constant`, `check.global_forward_ref`, `check.global_duplicate`, `check.global_shadow`, `check.global_assign`, `check.import_cycle`, `check.import_duplicate`, `check.import_collision`; table above):
`syntax.unparsed`, `syntax.unknown_statement`, `syntax.indent`, `syntax.top_level`, `syntax.statement`, `syntax.trailing`, `syntax.fcall_style`, `syntax.expression`, `syntax.character`, `syntax.number`, `syntax.string`, `syntax.escape`, `syntax.interpolation`, `syntax.duration`, `syntax.duration_unquoted`, `syntax.type`, `syntax.fields`, `syntax.field_modifier`, `syntax.duplicate_field`, `syntax.param_modifier`, `syntax.duplicate_param`, `syntax.duplicate_output`, `syntax.duplicate_error`, `syntax.output_required`, `syntax.operation_id`, `syntax.reserved_id`, `syntax.header_order`, `syntax.option_expected`, `syntax.option_after_body`, `syntax.option_misplaced`, `syntax.group_option`, `syntax.http`, `syntax.file`, `syntax.with`, `syntax.secret`, `syntax.else_if` **(new)** (`else if COND`), `syntax.else_without_if` **(new)** (orphan or second `else`), `syntax.try_without_catch`, `syntax.catch_without_try`, `syntax.catch`, `syntax.map`, `syntax.map_yield`, `syntax.poll`, `syntax.until`, `syntax.iterate`, `syntax.scope`, `syntax.concurrent`, `syntax.dag`, `syntax.dag_cycle`, `syntax.break`, `syntax.yield`, and `syntax.eNNNN` (a Capy grammar diagnostic with no dedicated mapping, e.g. `syntax.e0001` "expected closer `end`"; it usually accompanies a more specific error such as `syntax.unknown_statement` for `finally`);
checks: `check.unknown_function` **(new)** (a prefix call `(NAME …)` outside the built-in function list; `hint: did you mean \`length\`?`), `check.unknown_operation`, `check.unknown_connector`, `check.unknown_auth_profile`, `check.call_cycle`, `registry.duplicate_id`;
documentation rules (`--strict-docs`): `docs.description`, `docs.param_description`, `docs.output_description`, `docs.field_description`, `docs.undeclared_error`. `check.unguarded_result` and `docs.undeclared_error` are also plain-`check` warnings (above).

**validation (422, exit 2)**
- input envelope (0.2.0): `validation.input_envelope`, `validation.pretty_stream` (**HTTP 400**), `validation.ffi_argument` (C ABI)
- request & params: `validation.required`, `validation.type`, `validation.unknown_field`, `validation.min`, `validation.max`, `validation.enum`, `validation.params`, `validation.usage`, `validation.query`, `validation.output`, `validation.argument`, `validation.option`, `validation.duration`, `validation.codec`, `validation.body`, `validation.malformed_json` (**HTTP 400**)
- surfaces & sessions: `stream.required`, `stream.input_required`, `stream.emits_undeclared`, `validation.input` (also: a received item that does not match `receives`, `details {seq, path, expected, found}`), `validation.frame`, `validation.subprotocol`, `validation.input`, `validation.no_input`, `validation.endpoint`, `validation.check_files_remote`, `mcp.session_required`, `mcp.protocol_version`
- config: `policy.invalid` (`details.pointer` is the JSON pointer, e.g. `/grants/0/access/1`; a per-request `restrict` error points at `/restrict/…`; a `limits` value wider than its field, e.g. `4294967297`, is `must be at most 4294967295`), `serve.auth_required`, `validation.usage` (also: local `--timeout` above the 600000 ms host cap)
- runtime values (raised with the statement's span): `value.type`, `value.missing_key`, `value.overflow`, `value.division_by_zero`, `value.not_iterable`, `value.handle`, `file.form`, `syntax.map_yield`
- files: `file.codec`, `file.to`, `file.not_regular` **(new)** (`with file open … mode read` on a non-regular file), `validation.file_mode` **(new)** (mode not read/write/append), `validation.file_path` **(new)**, `validation.file_scope` **(new)** (`with file VERB` other than `open`/`watch`), `validation.file_write` **(new)** (`NAME.write` shape), `validation.chunk_size` **(new)** (non-integer `chunk_size`)
- HTTP client: `validation.url`, `validation.url_segment`, `validation.http_option`, `validation.http_stream`, `validation.http_version`, `validation.http_request`, `validation.http_retry_unsafe`, `validation.tls`, `syntax.http`
- sockets & processes: `validation.socket_option`, `validation.socket_send`, `validation.socket_receive`, `validation.framing`, `validation.frame_delimiter`, `syntax.with`, `validation.command_option`, `validation.command_stream`, `validation.process_program`, `syntax.command`
- UDP / QUIC: `udp.connected`, `udp.no_peer`, `validation.udp_address`, `validation.udp_option`, `quic.alpn_required`, `quic.datagrams_disabled`, `quic.direction`, `quic.stream_finished`, `validation.quic_endpoint`, `validation.quic_option`, `framing.delimiter_in_payload`
- gRPC: `grpc.connector`, `grpc.descriptor`, `grpc.endpoint`, `grpc.form`, `grpc.invalid_argument` (status 3), `grpc.invalid_message`, `grpc.metadata`, `grpc.method`, `grpc.mode`, `grpc.option`, `grpc.send`, `grpc.unknown_connector`, `grpc.unknown_method`, `grpc.unknown_service`
- MCP connectors: `mcp.connector`, `mcp.snapshot`, `mcp.snapshot_missing`, `mcp.snapshot_unapproved`, `mcp.alias_collision`, `mcp.unknown_import`, `mcp.unknown_tool`, `mcp.unknown_resource`, `mcp.unknown_prompt`, `mcp.auth_transport`, `validation.mcp_params`
- OAuth: `validation.auth_profile`, `validation.auth_flow`, `validation.auth_callback`, `validation.auth_account`, `validation.auth_scope`

**auth (401, exit 3)**: `auth.required`, `auth.invalid`.

**permission (403, exit 3)**: `permission.import_outside_root` (0.2.0), `permission.denied` (policy broker, `serve.principals`, MCP Origin, symlink refused, host ceiling `host ceiling: no grant …`, per-request restriction `request restriction: no grant …`, and secret taint: `secret \`NAME\` cannot be returned` / `cannot be emitted` / `may not reach SINK` / `is bound to ORIGIN; it may not be sent to …` with `details {secret, origin?}`), `permission.os`, `file.hardlink_refused`, `auth.origin_not_bound`, `grpc.permission_denied` (status 7).

**not_found (404, exit 4)**: `not_found.import` (0.2.0), `not_found.operation`, `not_found.route`, `not_found.request`, `not_found.session`, `not_found.ref`, `not_found.mcp_session`, `not_found.trace`, `not_found.source`, `not_found.file`, `file.root`, `not_found.env` (includes a `secret … from env "VAR"` whose variable is unset), `not_found.program`, `not_found.descriptor`, `not_found.mcp_connector`, `not_found.mcp_snapshot`, `not_found.mcp_resource`, `not_found.mcp_prompt`, `not_found.auth_profile`, `not_found.auth_transaction`, `grpc.not_found` (status 5).

**conflict (409, exit 4)**: `conflict.already_exists` (also: `trace export` onto an existing file), `conflict.input_finished` **(new)** (library duplex `send` after `finish_send` or after the request ended), `conflict.exists`, `conflict.version`, `conflict.ref`, `conflict.cursor`, `stream.cursor_expired`, `conflict.input_sequence`, `conflict.input_closed`, `conflict.session_terminal`, `grpc.input_closed`, `auth.login_required` (also gRPC status 16), `auth.access_denied`, `auth.transaction_expired`, `auth.insufficient_scope`, `auth.callback_invalid`, `auth.refresh_uncertain`.

**limit (429, exit 5, retryable)**: `limit.imports` (0.2.0), `limit.buffered_bytes` **(new)** (host-wide `limits.max_buffered_bytes` reached by session/stream queues), `limit.chunk_size` **(new)** (`chunk_size` outside 1…8 MiB), `limit.concurrency`, `limit.call_depth`, `limit.sessions`, `limit.input_queue`, `limit.ws_refs`, `limit.response_body`, `limit.http_body`, `limit.http_redirects`, `limit.file_size`, `limit.frame`, `limit.stream_item`, `limit.process_output`, `limit.quic_streams`, `limit.mcp_hops`, `limit.mcp_recursion`, `limit.mcp_pages`, `limit.mcp_message`, `limit.auth_transactions`, `grpc.resource_exhausted` (status 8).

**timeout (504, exit 6)**: `timeout` (UDP/QUIC receive), `timeout.request`, `timeout.scope`, `timeout.dag`, `timeout.concurrent`, `timeout.poll`, `timeout.stream`, `timeout.http`, `timeout.connect`, `timeout.receive`, `timeout.process`, `timeout.mcp`, `timeout.auth_token`, `timeout.file_lock` **(new)** (another writer held the file lock for more than 10 s during `update`/`write` of an existing file), `quic.idle_timeout`, `grpc.deadline_exceeded` (status 4).

**cancelled (409, exit 130)**: `cancelled.request`, `cancelled.runtime`, `cancelled.session`, `cancelled.idle`, `cancelled.shutdown` **(new)** (`serve` drain on SIGINT/SIGTERM), `consumer.stop` **(new)** (a library `DataSink` returned `RivetError::consumer_stop()`; not a failure of the sink), `cancelled.disconnect`, `cancelled.consumer`, `cancelled.socket`, `cancelled.grpc`, `grpc.cancelled` (status 1).

**connection (502, exit 5)**: `connection.failed`, `connection.refused`, `connection.timeout`, `connection.http`, `connection.http_body`, `connection.http3`, `connection.udp`, `connection.io`, `connection.websocket`, `connection.mcp_closed`, `connection.endpoint`, `connection.bind`, `connection.serve`, `udp.socket_failed`, `udp.bind_failed`, `udp.connect_failed`, `udp.send_failed`, `udp.receive_failed`, `quic.connect`, `quic.connection`, `quic.socket`, `quic.closed`.

**dns (502, exit 5)**: `dns.resolve`, `dns.no_address`.

**tls (502, exit 5)**: `tls.config`, `tls.handshake`, `tls.pem`, `quic.tls`, `quic.tls_config`, `quic.alpn_mismatch`.

**http (502, exit 5)**: `http.status` (a response status not in `accept status`).

**protocol (502, exit 5)**: `protocol.endpoint`, `protocol.http_location`, `protocol.websocket_handshake`, `protocol.unexpected_eof`, `protocol.unsupported_capability`, `protocol.mcp_message`, `protocol.mcp_initialize`, `protocol.mcp_version`, `protocol.mcp_cancelled`, `protocol.mcp_error`, `protocol.mcp_capability`, `protocol.mcp_result`, `protocol.mcp_output_schema`, `protocol.mcp_schema_changed`, `mcp.schema_drift` **(new)** (live `tools/list` differs from the approved snapshot at first use; the call is not sent), `http.version_unavailable`, `auth.token_endpoint_failed` (malformed token response), `framing.truncated`, `udp.truncated`, `udp.message_too_large`, `quic.transport`, `quic.stream`, `quic.stream_reset`, `quic.stream_finished`, `quic.stop_sending`, `quic.frame_too_large`, `quic.datagram_too_large`, `quic.datagrams_unavailable`, `grpc.decode`, `grpc.cardinality`, `grpc.missing_message`, and `grpc.<status_name>` for every other non-OK gRPC status (e.g. `grpc.unavailable`, `grpc.internal`).

**process (502, exit 5)**: `process.exit` (status outside `accept exit`), `process.spawn`, `process.not_executable`, `process.io`.

**parse (502, exit 5)**: `parse.json`, `parse.utf8`.

**application (502, exit 5)**: codes declared by the operation and raised with `fail CODE {details}`; `mcp.tool_failed`; `auth.token_endpoint_failed` (error status from the token endpoint), `auth.client_secret_missing`, `auth.store_failed`.

**output_invalid (500, exit 5)**: `output.invalid`.

**consumer_failed (500, exit 5)**: `consumer_failed`.

**cleanup (500, exit 5)**: `cleanup.failed`, `cleanup.timeout` (the 5 s close budget), `cleanup.closed` (use of a closed handle).

**unsupported (501, exit 5)**: `unsupported.feature` (0.2.0, `details.feature`), `unsupported.stage_c` **(new)** (`with file watch`), `unsupported.conditional_update` **(new)** (`if_version` on a platform without `flock`, i.e. non-Unix), `unsupported.serve_mtls`, `unsupported.sandbox_backend`, `unsupported.shell`, `unsupported.interactive`, `unsupported.process_duplex`, `unsupported.command`, `unsupported.adapter`, `unsupported.method`, `unsupported.property`, `unsupported.form`, `unsupported.handle`, `unsupported.child`, `unsupported.iterate`, `unsupported.transport`, `unsupported.unix`, `unsupported.tcp_tls`, `unsupported.reconnect`, `unsupported.udp_option`, `unsupported.quic_early_data`, `unsupported.quic_migration`, `unsupported.oauth`, `unsupported.auth`, `unsupported.auth_flow`, `unsupported.auth_kind`, `unsupported.auth_method`, `unsupported.token_type`, `unsupported.credential_store`, `grpc.unimplemented` (status 12).

**internal (500, exit 5)**: `internal`, `internal.panic` (0.2.0, C ABI), `file.io` (unexpected OS error).

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

Codes are append-only: 0.2.0 added the codes in the table above and changed no existing code's kind. What changed is
the **container**: the error object moved into the 0.2.0 envelope and `effects` moved out of it
([MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md)). The input aliases `id`/`params` and the
CLI `--params` are deprecated in 0.2.x (warnings `deprecated.params`, `deprecated.input`; HTTP `deprecation: true`)
and become `validation.input_envelope` in 0.3.0.

## Examples

Captured on 2026-09-29 from the 0.2.0 release candidate (source at `6f9943f`); **request and trace IDs differ on
every run**.

```text
$ curl -s -X POST http://127.0.0.1:18951/v1/request -d '{"operation":"demo.add","data":{"a":"x"}}'      # HTTP 422
{"request_id":"req_101e9dfdd2","trace_id":"tr_101e9dfdd2","operation":"demo.add","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.type","message":"parameter `a` must be an integer, got text","retryable":false,"operation_id":"demo.add","details":{"field":"a"}},"effects":"none","data_count":0}

$ rivet --file app.rivet serve --listen 0.0.0.0:18956                                               # exit 2
{"request_id":"","trace_id":"","operation":"rivet.serve","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"serve.auth_required","message":"non-loopback listener requires serve.auth in policy.json","retryable":false},"effects":"none","data_count":0}

WS  {"ref":"h8","request_id":"","trace_id":"","operation":"chat.echo","type":"result","status":"error","data":null,"error":{"kind":"limit","code":"limit.ws_refs","message":"at most 8 refs may be in flight per connection","retryable":true},"effects":"none","data_count":0}

$ curl -s http://127.0.0.1:18952/v1/request \
    -d '{"operation":"demo.read","data":{"path":"data/a.txt"},"restrict":{"bogus":1}}'      # HTTP 422
{"request_id":"req_079c557633","trace_id":"tr_079c557633","operation":"demo.read","type":"result","status":"error","data":null,"error":{"kind":"validation","code":"policy.invalid","message":"restrict accepts only `grants` (got `bogus`); it can narrow, never grant","retryable":false,"operation_id":"demo.read","details":{"pointer":"/restrict/bogus"}},"effects":"none","data_count":0}

$ echo '{"operation":"demo.add","id":"demo.add"}' | rivet --file app.rivet request --input -          # exit 2
{"request_id":"","trace_id":"","operation":null,"type":"result","status":"error","data":null,"error":{"kind":"validation","code":"validation.input_envelope","message":"use `operation` or the deprecated `id`, not both","retryable":false,"details":{"key":"operation","alias":"id"}},"effects":"none","data_count":0}

$ rivet --file g1.rivet check --json                            # global BASE = env "HOME"   — exit 2
{"request_id":"","trace_id":"","operation":"rivet.check","type":"result","status":"error","data":null,"error":{"kind":"syntax","code":"check.global_not_constant","message":"global `BASE` cannot perform `env`; globals are constants fixed at load time","retryable":false,"source":{"file":"g1.rivet","line":1,"column":15,"end_line":1,"end_column":25},"hint":"move the effect into the operation that uses it"},"effects":"none","data_count":0}

$ rivet --file g2.rivet check                                   # param LIMIT reuses global LIMIT — exit 2
error[check.global_shadow]: parameter `LIMIT` reuses the name of a global; globals cannot be shadowed
  --> g2.rivet:6:11
   |
  6|     param LIMIT integer default 1 description "Shadow."
   |           ^^^^^
  = hint: rename the parameter

$ rivet --file i1.rivet check                                   # import "./missing.rivet" as missing — exit 4
error[not_found.import]: cannot read missing.rivet: No such file or directory (os error 2)
  --> i1.rivet:1:1

$ rivet --file i2.rivet check                                   # import "../outside.rivet" as out — exit 3
error[permission.import_outside_root]: import `../outside.rivet` resolves outside the runtime root .
  --> i2.rivet:1:1

$ rivet --file app.rivet check        # docs/demos/09-quic with a build without `quic` — exit 5
error[unsupported.feature]: a QUIC exchange in `engine.status` needs the `quic` feature, which this build was compiled without (rebuild rivet-runtime with `--features quic`)
  --> app.rivet:7:5
error[unsupported.feature]: HTTP/3 (`version 3` or `version prefer [3, …]`) in `items.http3` needs the `quic` feature, which this build was compiled without (rebuild rivet-runtime with `--features quic`)
  --> app.rivet:26:5
$ rivet --file app.rivet check --json   # the same: first error + suppressed, details.feature
{"request_id":"","trace_id":"","operation":"rivet.check","type":"result","status":"error","data":null,"error":{"kind":"unsupported","code":"unsupported.feature","message":"a QUIC exchange in `engine.status` needs the `quic` feature, …","retryable":false,"source":{"file":"app.rivet","line":7,"column":5,"end_line":16,"end_column":8},"details":{"feature":"quic"},"suppressed":[{"kind":"unsupported","code":"unsupported.feature","message":"HTTP/3 (`version 3` or `version prefer [3, …]`) in `items.http3` needs the `quic` feature, …","retryable":false,"effects":"none","source":{"file":"app.rivet","line":26,"column":5,"end_line":29,"end_column":8},"details":{"feature":"quic"}}]},"effects":"none","data_count":0}

$ rivet --file app.rivet request demo.add --params '{"a":2,"b":3}'                                  # exit 0, stderr warning
warning[deprecated.params]: --params is deprecated; use --data (removed in 0.3.0)
{"request_id":"req_01e01ece25","trace_id":"tr_01e01ece25","operation":"demo.add","type":"result","status":"ok","data":5,"error":null,"effects":"none","data_count":0}
```

See [API-2026-0001](api-2026-0001-http-rest-sse-polling.md), [API-2026-0002](api-2026-0002-websocket-rivet-v1.md),
[API-2026-0004](api-2026-0004-rust-library.md) and [API-2026-0007](api-2026-0007-c-abi.md) for full sessions.

## Compatibility Notes

- An unknown kind received from a remote server decodes as `internal` (`RivetError::from_value`), so an older CLI talking to a newer server still exits non-zero.
- The CLI renders errors as `error[CODE]: message` with a `--> file:line:col` caret excerpt and `= hint:` when present; `--json` (and `rivet request` always) prints the error envelope instead, on stderr. Warnings use the same rendering as `warning[CODE]`.
- Secret-taint refusals are reported as `permission.denied` (with `details.secret`) — there are no separate `secret.*` codes.
- Platforms: macOS and Linux. On Linux the process sandbox is gated: a bundle that runs a process gets `unsupported.sandbox_backend` (exit 5, HTTP 501). Windows is not supported in 0.2.0 ([INC-2026-0011](../incidents/active/inc-2026-0011-windows-port-failures.md)).

## Related Documents

- [API index](index.md) · [HTTP API](api-2026-0001-http-rest-sse-polling.md) · [WebSocket](api-2026-0002-websocket-rivet-v1.md) · [MCP](api-2026-0003-mcp-server-tools.md) · [Library](api-2026-0004-rust-library.md)
- [Envelopes](api-2026-0006-envelopes.md) · [C ABI](api-2026-0007-c-abi.md) · [MIG-2026-0001](../migrations/mig-2026-0001-response-and-input-envelopes.md)
- [PLAN-2026-0001](../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) · [PLAN-2026-0002](../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) · [PROP-2026-0001](../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial registry: 22 kinds and every code emitted by the source at commit f40d4aa. |
| 2 | 2026-09-28 | Claude | Fix batch through 829ca43: `check` warnings section; new codes `syntax.else_if`, `syntax.else_without_if`, `check.unknown_function`, `check.unguarded_result`, file-handle codes, `limit.buffered_bytes`, `limit.chunk_size`, `timeout.file_lock`, `cancelled.shutdown`, `consumer.stop`, `conflict.input_finished`, `mcp.schema_drift`, `unsupported.stage_c`, `unsupported.conditional_update`; secret-taint, ceiling and restriction refusals under `permission.denied`; `policy explain --params` exit 3; `source` span field names corrected. |
| 3 | 2026-09-29 | Claude | 0.2.0 (D-44, TASK-070): table of the 19 new codes with kind/HTTP/exit/retryable (`validation.input_envelope`, `validation.pretty_stream`, globals, modules, `limit.imports`, `unsupported.feature`, `validation.ffi_argument`, `internal.panic`) and the warnings `check.module_policy_ignored`, `deprecated.params`, `deprecated.input`; the error object inside the envelope (`effects` moved to the top level); per-surface diagram and examples re-captured on the 0.2.0-rc (source `6f9943f`); platform note. |
