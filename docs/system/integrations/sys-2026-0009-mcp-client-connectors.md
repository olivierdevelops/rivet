---
document_id: SYS-2026-0009
title: "Rivet MCP client connectors"
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
components: [connectors, mcp, policy]
affected_versions:
  from: "0.1.0"
  to: null
last_verified_version: "0.1.0-dev (commit 829ca43)"
next_review_date: 2026-10-28
review_cycle: on-release
confidentiality: internal
scope: How Rivet calls remote MCP servers as a client. Covers `connector NAME mcp` declarations, stdio and Streamable HTTP transports, reviewed snapshots (format rivet.mcp.snapshot/1, sha256 approval in policy.json), imported operation IDs, the connectors.invoke_mcp use case, `rivet connectors sync` / rivet.connectors.sync, bridge recursion guards and the typed error codes.
reason: Outbound MCP lets Rivet run tools whose effects it cannot see. Reviewers need to know exactly which schema is trusted, which grants each call needs, how a schema refresh is proposed and approved, and how bridge loops are stopped.
related_documents: [PROP-2026-0001, PLAN-2026-0001, ADR-0003, SYS-2026-0001, SYS-2026-0003, SYS-2026-0004, SYS-2026-0005, SYS-2026-0006, SYS-2026-0008]
supersedes: null
superseded_by: null
tags: [rivet, system, mcp, connectors, snapshots, streamable-http, stdio, bridge]
---

# Rivet MCP client connectors

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** connectors, mcp, policy
> **Last Verified Version:** 0.1.0-dev (commit 829ca43)

## Summary

A `connector NAME mcp … end` declaration makes a remote MCP server's tools, resources and prompts
available as ordinary Rivet operation IDs: `crm.tools.search`, `crm.resources.read` and
`crm.prompts.get`. The declared schema is never discovered live. At bundle load Rivet reads a local
**snapshot file** (format `rivet.mcp.snapshot/1`) and hashes its bytes. The snapshot counts as
reviewed only if that `sha256:…` is listed in policy.json `approved.snapshots`. A new snapshot is
proposed only by an explicit, policed discovery, `rivet connectors sync NAME --output PATH` (or the
`rivet.connectors.sync` built-in). Discovery writes a **new candidate file** and never changes the
running catalog.

```text
            review time                                         run time
 ┌───────────────────────────────────────┐   ┌───────────────────────────────────────────────────┐
 │ rivet connectors sync crm --output F  │   │ bundle load (McpPeer::load)                         │
 │   allow_mcp crm/discover              │   │   read schema F (bootstrap) ─▶ sha256 ∈ approved?   │
 │   + transport grant                   │   │   exposed names ⊆ snapshot ─▶ imports + aliases     │
 │   + allow_write F (create only)       │   │                                                     │
 │   ─▶ F (candidate) + sha256           │   │ request crm.tools.search {…}                        │
 │ human review of F                     │   │   ─▶ connectors.invoke_mcp                          │
 │ add sha256 to approved.snapshots ─────┼──▶│      pin · recursion · params · allow_mcp · lease   │
 └───────────────────────────────────────┘   │      ─▶ McpClient.open (allow_network | allow_exec) │
                                             │      ─▶ initialize · call · close                   │
                                             └───────────────────────────────────────────────────┘
```

## Responsibilities

| Responsibility | Where |
|---|---|
| Snapshot format, parse/serialize, hash; import IDs, aliases, catalog entries; bridge `_meta`; minimal JSON Schema check | `src/domain/mcp.rs` |
| Parse `connector NAME mcp` options, read and approve snapshots, build imports, reject collisions and unknown literal calls | `src/infra/mcp_client.rs` (`McpPeer::load`, `connector_info`, `add_imports`) |
| Open stdio or Streamable HTTP sessions, run `initialize`, correlate requests, answer `ping`, decline server requests, close | `src/infra/mcp_client.rs` (`McpPeer::open`, `Session`, `StdioLink`, `HttpLink`) |
| Authorize and run one call or discovery: pin, recursion, params, `allow_mcp`, OAuth lease, result mapping | `src/features/connectors/invoke_mcp.rs` (`invoke_mcp`) |
| Dispatch imported IDs under the request deadline | `src/orchestrator/runtime.rs` (`dispatch_mcp`) |
| Write candidate snapshots | `src/orchestrator/runtime.rs` (`sync_connector`), `src/orchestrator/setup_cli.rs` (`connectors sync`), `src/orchestrator/builtins.rs` (`rivet.connectors.sync`) |
| Carry inbound bridge `_meta` into nested connector calls | `src/orchestrator/setup_mcp.rs` → `Runtime::request_bridged` |

## Boundaries and Non-Responsibilities

- **Inbound MCP is separate.** `rivet serve` exposing operations at `/mcp` or over `--stdio` is described
  in [SYS-2026-0004](../components/sys-2026-0004-surfaces-and-serve.md). Serving MCP grants no outbound
  authority.
- **No live discovery at compile or load time.** A connector without an approved snapshot fails to load.
- **No sampling, elicitation or roots.** The client declares no capabilities in `initialize`.
- **No legacy HTTP+SSE transport** and no resource templates or subscriptions.
- **Remote effects are opaque.** Rivet cannot see what a remote tool does. The I/O manifest marks the
  call `opaque_remote`, and a tool call's Completion reports `effects: "unknown"` — whether the tool is
  called directly or through an operation that wraps it (the caller's status is folded upward with
  `unknown` > `partial` > `committed` > `none`, so a wrapper never reports `committed` just because the
  remote call returned).
- **gRPC connectors** (`connector NAME grpc`) are not covered here; see
  [SYS-2026-0005](sys-2026-0005-protocol-adapters.md).

## Architecture

```text
 surfaces / scripts                                    features/connectors            infra/mcp_client.rs
 ──────────────────                                    ───────────────────            ───────────────────
 rivet request crm.tools.search ─┐
 /v1/request, MCP, WS, library ──┼─▶ Runtime::dispatch_mcp ─▶ invoke_mcp(McpRequest) ─▶ McpClient (McpPeer)
 (request "crm.tools.search" …) ─┘   (deadline, bridge ctx)     │                       │ catalog()
 rivet connectors sync crm ──────────▶ Runtime::sync_connector ─┘ method "discover"     │ open(conn, ctx)
 rivet.connectors.sync {name,output} ┘                                                  ▼
                                                                          ┌──────── Session ────────┐
                                                                          │ Link = StdioLink        │
                                                                          │      | HttpLink         │
                                                                          │ initialize → request →  │
                                                                          │ close                   │
                                                                          └─────────────────────────┘
                                          StdioLink ─ spawn (McpSpawnFn: allow_exec + OS sandbox) ─▶ child
                                          HttpLink  ─ POST/DELETE (McpHttpFn = exchange_http) ─────▶ /mcp
```

### Declaration

```text
connector crm mcp
    transport http "https://mcp.example.com/mcp"      # or: transport command "/abs/or/bundle-relative/bin"
    schema "./schemas/crm.json"                        #         args ["--flag", "…"]
    expose tools ["search"]                            #         env {KEY: "value"}
    expose resources ["crm://contacts/readme"]         # optional
    expose prompts ["summarize"]                       # optional
    auth crm_user account "ada"                        # http only (OAuth lease, see SYS-2026-0006)
    tls ca_file "./ca.pem"                             # http only: server_name | ca_file | cert_file | key_file
end
```

| Load rule | Error code |
|---|---|
| Connector named `rivet` | `mcp.connector` |
| Missing `transport`, unknown option, bad `expose`/`tls`/`args`/`env` | `mcp.connector` |
| `auth` on `transport command` | `mcp.auth_transport` |
| `tls` on `transport command` | `mcp.connector` |
| No `schema` (outside discovery) | `mcp.connector` (hint: run `connectors sync`) |
| Snapshot file missing | `not_found.mcp_snapshot` (exit 4) |
| Snapshot hash not in `approved.snapshots` | `mcp.snapshot_unapproved` (exit 2, details `{path, sha256}`) |
| Snapshot malformed (unknown key, wrong `format`, missing `protocolVersion`/`tools`/`inputSchema`, duplicate tool) | `mcp.snapshot` |
| Exposed tool / resource / prompt not in the snapshot | `mcp.unknown_tool` / `mcp.unknown_resource` / `mcp.unknown_prompt` |
| Import ID collides with an operation or another import | `mcp.alias_collision` |
| A literal `(request "crm.x")` names no exposed import | `mcp.unknown_import` |
| Connector kind other than `mcp`/`grpc` | `mcp.connector` |

### Imports

| Import ID | Kind | JSON-RPC method | `allow_mcp` target | Params | Needs server capability |
|---|---|---|---|---|---|
| `C.tools.<alias>` | tool | `tools/call` | `C/tools/<alias>` | the tool's `inputSchema`, verbatim | `tools` |
| `C.resources.read` | resource | `resources/read` | `C/resources/read` | `{uri}`, an exposed URI | `resources` |
| `C.prompts.get` | prompt | `prompts/get` | `C/prompts/get` | `{name, arguments?}`, an exposed name; text arguments | `prompts` |
| (discovery) | — | `tools/list`, `resources/list`, `prompts/list` | `C/discover` | — | per list |

`tool_alias` maps every character outside `[A-Za-z0-9_.]` to `_` and turns empty dot segments into `_`.
The remote name is still sent on the wire unchanged. Imports are real catalog rows
(`McpCatalog::import_entries`), so `list`, `describe`, `outputs`, `/v1/operations` and MCP `tools/list`
show them.

### Snapshot file (`rivet.mcp.snapshot/1`)

```text
{
  "format": "rivet.mcp.snapshot/1",           optional on read; must match when present
  "protocolVersion": "2025-11-25",            required
  "serverInfo": {…},                          optional
  "tools": [{name, title?, description?, inputSchema (object), outputSchema?}],   required
  "resources": [{uri, name?, description?, mimeType?}],                          optional
  "prompts": [{name, description?, arguments: [{name, description?, required}]}]  optional
}
 any other top-level key → mcp.snapshot
 written by sync as pretty JSON + trailing "\n"; hash = "sha256:" + hex(sha256(file bytes))
```

Approval is by exact bytes, so any edit (even whitespace) changes the hash and the bundle stops loading
until the new hash is approved. This was verified by appending a space to an approved file:

```text
error[mcp.snapshot_unapproved]: snapshot ./schemas/crm.json of connector `crm` is not reviewed: sha256:3d3b0c72fcc9d1a56ff415c04ff70c8d595335f39047cd332dde2a8991e4fa9e is not listed in policy.json approved.snapshots
```

### Connector call sequence

```text
 caller          dispatch_mcp          invoke_mcp                          McpPeer / Session              MCP server
   │ crm.tools.search {query}│                │                                   │                          │
   │────────────────────────▶│ McpRequest{connector, method "tools.search",     │                          │
   │                         │   schema_hash = loaded hash, ctx{deadline, bridge, identity, principal}}     │
   │                         │───────────────▶│ 1 connector + import in pinned catalog? (not_found.*)        │
   │                         │                │ 2 schema_hash == loaded? (protocol.mcp_schema_changed)      │
   │                         │                │ 3 auth without provider? (unsupported.auth)                 │
   │                         │                │ 4 hops < 8? (limit.mcp_hops)                                │
   │                         │                │ 5 "identity/crm.tools.search" ∉ chain? (limit.mcp_recursion)│
   │                         │                │ 6 params vs snapshot (validation.mcp_params, not_found.*)   │
   │                         │                │ 7 allow_mcp call crm/tools/search (permission.denied)       │
   │                         │                │ 8 auth P account A: allow_network URL, then lease            │
   │                         │                │──open──────────────────────────▶│ HTTP: allow_network per POST
   │                         │                │                                 │ stdio: allow_exec + sandbox
   │                         │                │                                 │──initialize {2025-11-25}──▶│
   │                         │                │                                 │◀─{protocolVersion, caps}───│
   │                         │                │                                 │  version ∈ {2025-11-25,     │
   │                         │                │                                 │   2025-06-18}? else         │
   │                         │                │                                 │   protocol.mcp_version      │
   │                         │                │                                 │──notifications/initialized─▶│
   │                         │                │ capability "tools"? (protocol.mcp_capability)                 │
   │                         │                │ first use of the session: live tools/list vs the approved     │
   │                         │                │   snapshot (name + inputSchema per exposed tool, key order    │
   │                         │                │   ignored) ─ differs? mcp.schema_drift, the call is NOT sent  │
   │                         │                │──request tools/call {name, arguments,                          │
   │                         │                │    _meta{rivet/hops: h+1, rivet/chain: chain+[entry]}}────────▶│
   │                         │                │                                 │  ping → {} ; sampling/       │
   │                         │                │                                 │  elicitation/roots → -32601  │
   │                         │                │◀────────────── result | JSON-RPC error ────────────────────────│
   │                         │                │ isError → mcp.tool_failed · error → protocol.mcp_error         │
   │                         │                │ structuredContent vs outputSchema → protocol.mcp_output_schema │
   │                         │                │──close─────────────────────────▶│ HTTP DELETE (session id) /  │
   │                         │                │                                 │ stdin EOF, 500 ms, reap      │
   │◀── Completion{result:{content, structuredContent?, isError:false}, effects:"unknown"} ────────────────────│
```

Steps 1 to 7 run before any transport I/O. Each call opens its own session and always closes it. A
dropped session with a request still pending sends `notifications/cancelled` first. The whole call
runs under the request deadline; when it expires the result is `timeout.mcp` (exit 6, effects
`unknown`).

### Transports

| | stdio (`transport command BIN`) | Streamable HTTP (`transport http URL`) |
|---|---|---|
| Start | `McpSpawnFn`: `confine_process` (`allow_exec` on the resolved program, OS sandbox) then a duplex spawn; argv only, no shell | none; the first POST starts the session |
| Program path | absolute, or resolved against the bundle root | — |
| Messages | one JSON object per line on stdin/stdout | JSON-RPC POST, `accept: application/json, text/event-stream`; reply as JSON body or SSE stream |
| Session | the process | `mcp-session-id` from the first response; `mcp-protocol-version` after initialize |
| Policy per attempt | `allow_exec` (+ sandbox file grants) | `allow_network` connect through `exchange_http` (private-range and DNS checks, no redirects) |
| TLS | — | `tls` files read through the policed `FileAccess` (`allow_read`) |
| Auth | not allowed | `Authorization: Bearer` from an origin-bound OAuth lease |
| Close | stdin EOF, drain up to 500 ms, terminate and reap | `DELETE` with the session id (200/202/204/404/405 accepted) |
| Message cap | 8 MiB (`limit.mcp_message`) | 8 MiB (`limit.mcp_message`) |

On macOS the Seatbelt sandbox confines stdio children. A child that must read files needs matching
`allow_read` grants. In the verification below, a stdio child failed with `Operation not permitted`
until the child bundle directory was granted.

### Snapshot review and approval flow

```text
  ┌────────────┐ connectors sync crm --output ./schemas/crm.json            ┌────────────────────────────┐
  │ no snapshot│─────────────────────────────────────────────────────────▶ │ candidate file written     │
  │ (load fails│   discovery-mode load (unreviewed/missing snapshot         │ (exclusive create)         │
  │  for normal│   tolerated, connector imports nothing)                    │ stdout: {connector, path,  │
  │  commands) │                                                            │  sha256, protocolVersion,  │
  └────────────┘                                                            │  tools, resources, prompts}│
        ▲                                                                   └─────────────┬──────────────┘
        │                                                                                 │ human review
        │ file edited / replaced (hash changes)                                           ▼
        │                                                                   ┌────────────────────────────┐
  ┌─────┴──────────────┐   policy.json "approved": {"snapshots": ["sha256:…"]}│ reviewed snapshot          │
  │ mcp.snapshot_      │◀───────── hash not listed ────────────────────────── │ bundle loads; imports      │
  │ unapproved (exit 2)│                                                     │ appear in list/describe    │
  └────────────────────┘                                                     └────────────────────────────┘
```

A refresh never overwrites: syncing to an existing path fails with `conflict.already_exists` (exit 4)
**before** anything contacts the server.

**Drift check.** An approved snapshot is also checked against the live server: at the first use of each
connector session, Rivet reads `tools/list` and compares every exposed tool's name and `inputSchema` with the
snapshot (JSON equality, key order ignored). A changed or missing tool fails `mcp.schema_drift` (protocol,
exit 5) before the tool call is sent, and the live schema is never used. Servers that did not negotiate
`tools` are left to the call's own capability check.

```text
 approved snapshot (sha256 in policy.json)        live server
        │                                            │ tools/list (first use of the session)
        └──────────── compare exposed tools ─────────┘
              same name + same inputSchema ─▶ proceed with tools/call
              changed / missing           ─▶ mcp.schema_drift (nothing sent) ─▶ connectors sync → review → approve
```
Write the refresh to a new path (for example `crm.next.json`), review the diff, then switch `schema` and
the approved hash together.

### Sync journey (`rivet connectors sync` / `rivet.connectors.sync`)

```text
 rivet --file app.rivet --policy ./policies/sync.json connectors sync crm --output ./schemas/crm.json
   │
   ├─ CLI: --output must resolve inside the bundle root, else validation.output (exit 2)
   ├─ --output exists? ─▶ conflict.already_exists (exit 4) BEFORE any discovery traffic
   ├─ Runtime built with connector_discovery(): McpPeer::load(discovery = true)
   ├─ invoke_mcp(method "discover")
   │    ├─ allow_mcp call crm/discover                         (permission.denied, exit 3)
   │    ├─ open session (allow_network URL | allow_exec BIN)
   │    ├─ tools/list, resources/list, prompts/list            (only negotiated capabilities;
   │    │    cursor paging, ≤ 100 pages → limit.mcp_pages)       JSON-RPC error → protocol.mcp_error)
   │    └─ McpSnapshot::parse(listing)                         (invalid listing → mcp.snapshot)
   ├─ serialize (pretty JSON + "\n"), sha256
   ├─ FileOperation create, overwrite false, bytes             (allow_write PATH; a path that appeared meanwhile
   │                                                              → conflict.already_exists)
   └─ stdout: receipt JSON · stderr: "wrote candidate snapshot … approve it in policy.json …"
```

`rivet.connectors.sync {name, output}` does the same through any surface. Its `output` is relative to
the bundle root. Network principals can call it only if they are listed exactly for
`rivet.connectors.sync` in `serve.principals`; the local principal can always call it.

## Interfaces

| Interface | Shape |
|---|---|
| Imported IDs | `C.tools.<alias>`, `C.resources.read`, `C.prompts.get`, callable wherever operations are (`rivet request`, `/v1/request`, MCP, WS, library, `(request "…" {…})` in scripts) |
| Tool result | `{content: [...], structuredContent?: …, isError: false, _meta?}`, preserved as the server returned it |
| Resource result | `{contents: [...]}` |
| Prompt result | `{description?, messages: [...]}` |
| CLI | `rivet connectors sync NAME --output PATH` |
| Built-in | `rivet.connectors.sync {name, output}` → `{connector, path, sha256, protocolVersion, tools, resources, prompts}` |
| Port | `McpClient { catalog(); open(conn, ctx) -> McpSession }`, `McpSession { peer(); request(method, params) -> McpReply; close() }` |

```text
$ rivet connectors sync --help
Discover a connector's tools/resources/prompts and write a NEW candidate snapshot (never overwrites); prints the sha256 to approve in policy.json

Usage: rivet connectors sync [OPTIONS] --output <OUTPUT> <NAME>
```

### Error codes

| Code | Kind | HTTP / exit | When |
|---|---|---|---|
| `not_found.mcp_connector` | not_found | 404 / 4 | unknown connector |
| `not_found.operation` | not_found | 404 / 4 | ID is not an exposed import |
| `not_found.mcp_resource`, `not_found.mcp_prompt` | not_found | 404 / 4 | URI / prompt not exposed |
| `not_found.mcp_snapshot` | not_found | 404 / 4 | snapshot file missing (load) |
| `mcp.snapshot_unapproved`, `mcp.snapshot`, `mcp.connector`, `mcp.unknown_*`, `mcp.alias_collision`, `mcp.unknown_import`, `mcp.auth_transport`, `mcp.snapshot_missing` | validation | 422 / 2 | load or declaration problems |
| `validation.mcp_params` | validation | 422 / 2 | params fail the snapshot (details `problems[]`) |
| `validation.output` | validation | 422 / 2 | sync output outside the bundle |
| `permission.denied` | permission | 403 / 3 | `allow_mcp`, `allow_network`, `allow_exec`, `allow_write` or auth grants |
| `conflict.already_exists` | conflict | 409 / 4 | sync output exists (checked before discovery) |
| `unsupported.auth` | unsupported | 501 / 5 | connector `auth` on a host without a credential provider |
| `limit.mcp_hops`, `limit.mcp_recursion` | limit | 429 / 5 | bridge guards (retryable flag set by kind) |
| `limit.mcp_pages`, `limit.mcp_message` | limit | 429 / 5 | more than 100 list pages; message over 8 MiB |
| `mcp.schema_drift` | protocol | 502 / 5 | the live `tools/list` no longer matches the approved snapshot (first use of a session); nothing sent |
| `mcp.tool_failed` | application | 502 / 5 | `isError: true`; details `{tool, content, structuredContent?}`, effects `unknown` |
| `protocol.mcp_error` | protocol | 502 / 5 | JSON-RPC error; details `{code, message, method, data?}` |
| `protocol.mcp_initialize`, `protocol.mcp_version` | protocol | 502 / 5 | initialize failed / incompatible version |
| `protocol.mcp_capability` | protocol | 502 / 5 | server lacks tools/resources/prompts |
| `protocol.mcp_output_schema` | protocol | 502 / 5 | structuredContent violates the reviewed outputSchema |
| `protocol.mcp_result`, `protocol.mcp_message`, `protocol.mcp_cancelled` | protocol | 502 / 5 | malformed result/message; server cancelled |
| `protocol.unsupported_capability` | protocol | 502 / 5 | server asked for sampling/elicitation/roots (declined with -32601) |
| `protocol.mcp_schema_changed` | protocol | 502 / 5 | caller pinned a different snapshot hash |
| `connection.mcp_closed` | connection | 502 / 5 | peer closed before answering |
| `timeout.mcp` | timeout | 504 / 6 | request deadline hit during the call |

## Configuration

`docs/demos/06-mcp-bridge` separates everyday use from schema refresh with two policies:

```text
  policy.json (auto-discovered)                    policies/sync.json (--policy)
  ─────────────────────────────                    ───────────────────────────────
  allow_network https://mcp.example.com:443        allow_network https://mcp.example.com:443
  allow_mcp     crm/tools/search                   allow_mcp     crm/discover
  approved.snapshots [sha256 of schemas/crm.json]  allow_write   ../schemas/crm.next.json  (relative to policies/)
```

| Capability | Target | Verb |
|---|---|---|
| `allow_mcp` | `C/tools/<alias>`, `C/resources/read`, `C/prompts/get`, `C/discover` | `call` |
| `allow_network` | the connector URL origin | `connect` (each POST/DELETE) |
| `allow_exec` | the resolved stdio program | exec (plus sandbox read/write grants the child needs) |
| `allow_read` | `tls` files | read |
| `allow_write` | sync `--output` | create |
| `approved.snapshots` | `"sha256:<hex>"` list | — |

Policy details are in [SYS-2026-0008](../configuration/sys-2026-0008-policy-json-reference.md).

## Runtime Behaviour

### `docs/demos/06-mcp-bridge` as shipped

The demo ships the MCP protocol fixtures but no `schemas/crm.json`. Every load-based command stops at
the snapshot check, and the error carries the next step as a hint:

```text
$ rivet --file app.rivet check          # same for io, io --by target, io --check-policy, describe, list
error[not_found.mcp_snapshot]: cannot read snapshot ./schemas/crm.json of connector `crm`: No such file or directory (os error 2)
  --> app.rivet:1:1
   |
  1| connector crm mcp
   | ^
  = hint: create it with `rivet connectors sync crm --output ./schemas/crm.json`, review it and approve its sha256 in policy.json
exit 4

$ rivet --file app.rivet connectors sync crm --output ./schemas/crm.next.json          # default policy
{"request_id":"","trace_id":"","error":{"kind":"permission","code":"permission.denied","message":"allow_mcp call crm/discover denied: no grant for allow_mcp crm/discover","retryable":false,"effects":"none","details":{"capability":"allow_mcp","access":"call","target":"crm/discover"}}}
exit 3

$ rivet --file app.rivet connectors sync crm --output /tmp/x.json
{"request_id":"","trace_id":"","error":{"kind":"validation","code":"validation.output","message":"--output /tmp/x.json must be inside the bundle directory .","retryable":false,"effects":"none"}}
exit 2

$ rivet --file app.rivet --policy ./policies/sync.json connectors sync crm --output ./schemas/crm.next.json
{"request_id":"","trace_id":"","error":{"kind":"dns","code":"dns.resolve","message":"cannot resolve mcp.example.com: failed to lookup address information: nodename nor servname provided, or not known","retryable":false,"effects":"none"}}
exit 5
```

The last command passes the policy and fails only because `mcp.example.com` is a placeholder. A live
MCP server is needed to go further with the demo itself.

### Full journey against a local fixture server

A scratch copy of the demo pointed `transport http` at a Python Streamable HTTP fixture on
`127.0.0.1:18443`. The fixture answers `initialize`, `tools/list` (from
`schemas/tools-list.fixture.json`) and `tools/call` (from `schemas/search-result.fixture.json`, or
`isError: true` for other queries). The sync policy granted `allow_network http://127.0.0.1:18443`,
`allow_mcp crm/discover` and `allow_write ../schemas/crm.json`. Request IDs vary per run.

**1. Sync writes the candidate.**

```text
$ rivet --file app.rivet --policy ./policies/sync.json connectors sync crm --output ./schemas/crm.json
{"connector":"crm","path":"./schemas/crm.json","sha256":"sha256:ab6b8ae3a332bf859f4d4ab1dae097e49973f0289aa9f2a00baffc5eeff3d805","protocolVersion":"2025-11-25","tools":["search"],"resources":[],"prompts":[]}
wrote candidate snapshot ./schemas/crm.json (sha256:ab6b8ae3a332bf859f4d4ab1dae097e49973f0289aa9f2a00baffc5eeff3d805); after review, approve it in policy.json: "approved": {"snapshots": ["sha256:ab6b8ae3a332bf859f4d4ab1dae097e49973f0289aa9f2a00baffc5eeff3d805"]}
exit 0
   server saw: initialize · notifications/initialized · tools/list · DELETE session=fx-session-1

$ head -8 schemas/crm.json
{
  "format": "rivet.mcp.snapshot/1",
  "protocolVersion": "2025-11-25",
  "serverInfo": {
    "name": "crm-fixture",
    "version": "1.0"
  },
  "tools": [

$ shasum -a 256 schemas/crm.json
ab6b8ae3a332bf859f4d4ab1dae097e49973f0289aa9f2a00baffc5eeff3d805  schemas/crm.json

$ rivet … connectors sync crm --output ./schemas/crm.json          # again: never overwrites (f40d4aa)
{"request_id":"","trace_id":"","error":{"kind":"conflict","code":"conflict.already_exists","message":"./schemas/crm.json already exists","retryable":false,"effects":"none"}}
exit 4

# at 829ca43 the check runs first and names the rule (docs/demos/06-mcp-bridge, no server contacted):
$ rivet --file app.rivet connectors sync crm --output ./schemas/crm.json
{"request_id":"","trace_id":"","error":{"kind":"conflict","code":"conflict.already_exists","message":"./schemas/crm.json already exists; connectors sync never overwrites a snapshot","retryable":false,"effects":"none","details":{"path":"./schemas/crm.json"}}}
exit 4
```

**2. Before and after approval** (the runtime policy grants `allow_network http://127.0.0.1:18443` and
`allow_mcp crm/tools/search`):

```text
$ rivet --file app.rivet check
error[mcp.snapshot_unapproved]: snapshot ./schemas/crm.json of connector `crm` is not reviewed: sha256:ab6b8ae3a332bf859f4d4ab1dae097e49973f0289aa9f2a00baffc5eeff3d805 is not listed in policy.json approved.snapshots
  --> app.rivet:1:1
   |
  1| connector crm mcp
   | ^
  = hint: after reviewing ./schemas/crm.json, add "sha256:ab6b8ae3a332bf859f4d4ab1dae097e49973f0289aa9f2a00baffc5eeff3d805" to policy.json "approved": {"snapshots": [...]}
exit 2

   … add "approved": {"snapshots": ["sha256:ab6b8ae3…d805"]} to policy.json …

$ rivet --file app.rivet check
ok: 1 operations, 1 connectors, 0 auth profiles

$ rivet --file app.rivet list
ID                NAME           DESCRIPTION
contacts.find     Find contacts  Call a reviewed remote MCP search tool through the shared registry.
crm.tools.search  search         Search the controlled contact fixture.

$ rivet --file app.rivet describe crm.tools.search
crm.tools.search — search
Search the controlled contact fixture.
source   app.rivet:1
delivery unary

params
  query   text     required    Name to search for.

output  object   MCP tool result: content blocks and structuredContent, unchanged.
  content            list json required  
  structuredContent  object   optional  
    contacts  list object required  
  isError            boolean  required  
  (open: extra fields allowed)
emits    —
receives —
errors   —
```

**3. Inspection.** The remote call is `opaque_remote`, so `--strict` reports the inventory as incomplete:

```text
$ rivet --file app.rivet io --by target
TARGET                  ACCESS        CAPABILITY     ORIGIN          PHASE    NEEDS FILE  USED BY
http://127.0.0.1:18443  connect POST  allow_network  transport http  connect  —           contacts.find (via crm.tools.search)
crm/tools/search        call tool     allow_mcp      request         body     —           contacts.find (via crm.tools.search)

$ rivet --file app.rivet io --check-policy
OPERATION      KIND     ACCESS        TARGET                      KNOWLEDGE      SOURCE        DECISION
contacts.find  (calls crm.tools.search — connector crm)                          app.rivet:17
contacts.find  network  connect POST  http://127.0.0.1:18443/mcp  exact          app.rivet:2   allowed
contacts.find  mcp      call tool     crm/tools/search            opaque_remote  app.rivet:17  allowed
2 allowed

$ rivet --file app.rivet io --strict
…
io: complete=false — 1 of 2 sites is dynamic/opaque
  contacts.find#2  mcp call tool  crm/tools/search (opaque_remote)  (app.rivet:17)
exit 7
```

**4. Calls.**

```text
$ rivet --file app.rivet request contacts.find --params '{"query":"Ada"}'          # captured at f40d4aa
{"request_id":"req_0174bd72c5","trace_id":"tr_0174bd72c5","result":{"content":[{"type":"text","text":"{\"contacts\":[{\"id\":\"42\",\"name\":\"Ada\"}]}"}],"structuredContent":{"contacts":[{"id":"42","name":"Ada"}]},"isError":false},"data_count":0,"effects":"committed"}
   (since 829ca43 the wrapping operation reports "effects":"unknown" — G22, tests/conformance_mcp.rs)
   server saw: initialize · notifications/initialized ·
               tools/call _meta {"rivet/hops": 1, "rivet/chain": ["rivet:232af55e413a4e42/crm.tools.search"]} · DELETE

$ rivet --file app.rivet request crm.tools.search --params '{"query":"Bob"}'        # exit 5
{"request_id":"req_01714d76f5","trace_id":"tr_01714d76f5","error":{"kind":"application","code":"mcp.tool_failed","message":"MCP tool `crm.tools.search` returned isError: true","retryable":false,"effects":"unknown","operation_id":"crm.tools.search","details":{"tool":"search","content":[{"type":"text","text":"no match"}]}}}

$ rivet --file app.rivet request crm.tools.search --params '{"query":42}'           # exit 2, no I/O
{…,"error":{"kind":"validation","code":"validation.mcp_params","message":"invalid params for `crm.tools.search`: params.query: expected string, got number","retryable":false,"effects":"none","operation_id":"crm.tools.search","details":{"problems":["params.query: expected string, got number"]}}}

$ rivet --file app.rivet request crm.tools.search --params '{"query":"Ada","limit":1}'  # exit 2
{…,"error":{"kind":"validation","code":"validation.mcp_params","message":"invalid params for `crm.tools.search`: params.limit: unknown field",…}}

$ rivet --file app.rivet --policy ./policies/nomcp.json request contacts.find --params '{"query":"Ada"}'   # exit 3, server saw nothing
{"request_id":"req_012f6009f5.1","trace_id":"tr_012f6009f5","error":{"kind":"permission","code":"permission.denied","message":"allow_mcp call crm/tools/search denied: no grant for allow_mcp crm/tools/search","retryable":false,"effects":"none","source":{"file":"app.rivet","line":17,"column":13,"end_line":17,"end_column":20},"operation_id":"crm.tools.search","details":{"capability":"allow_mcp","access":"call","target":"crm/tools/search"}}}
```

A direct call of the import reports `effects: "unknown"`. The wrapping operation `contacts.find`
reported `effects: "committed"` in this run.

**5. Built-in over serve.** The served runtime loaded the everyday policy, which has no discover grant:

```text
$ rivet --endpoint http://127.0.0.1:18444 connectors sync crm --output ./schemas/crm.next.json      # exit 3
{"request_id":"req_039292be7f","trace_id":"tr_039292be7f","error":{"kind":"permission","code":"permission.denied","message":"allow_mcp call crm/discover denied: no grant for allow_mcp crm/discover","retryable":false,"effects":"none","details":{"capability":"allow_mcp","access":"call","target":"crm/discover"}}}
```

### stdio transport (verified)

A connector with
`transport command "/…/target/debug/rivet"` and `args ["--file", "/…/child/app.rivet", "serve", "--stdio"]`
used another Rivet bundle as the MCP server. That bundle has one operation, `greet`. The grants were
`allow_exec` on the binary and `allow_read ./child/`, plus `allow_mcp peer/discover` and `allow_write`
for sync:

```text
$ rivet --file app.rivet --policy ./sync.json connectors sync peer --output ./schemas/peer.json
{"connector":"peer","path":"./schemas/peer.json","sha256":"sha256:5a44d74bffbb63d56b9cd940fcdab35dc69f7a14ae961a9a322ae2818c9d568b","protocolVersion":"2025-11-25","tools":["greet","rivet.request","rivet.list","rivet.describe","rivet.outputs","rivet.sessions.open","rivet.sessions.send","rivet.sessions.finish_input","rivet.sessions.read","rivet.sessions.cancel"],"resources":[],"prompts":[]}

$ rivet --file app.rivet request peer.tools.greet --params '{"who":"Ada"}'        # after approval, allow_mcp peer/tools/greet
{"request_id":"req_01d8c4cdcd","trace_id":"tr_01d8c4cdcd","result":{"content":[{"type":"text","text":"{\"request_id\":\"req_01d7ade2b5\",\"trace_id\":\"tr_01d7ade2b5\",\"result\":\"hello Ada\",\"data_count\":0,\"effects\":\"none\"}"}],"structuredContent":{"request_id":"req_01d7ade2b5","trace_id":"tr_01d7ade2b5","result":"hello Ada","data_count":0,"effects":"none"},"isError":false},"data_count":0,"effects":"unknown"}
```

Without `allow_read` on the child bundle, the sandboxed child could not read its source:
`process.exit … "error[not_found.source]: cannot read …/child/app.rivet: Operation not permitted (os error 1)"`.

### Recursion guards

```text
  upstream (hops h, chain K) ──tools/call contacts.find _meta{rivet/hops:h, rivet/chain:K}──▶ rivet serve /mcp
        setup_mcp.rs: BridgeHops::from_meta ─▶ Runtime::request_bridged (bridges[trace_id] = {h, K})
        contacts.find ─▶ (request "crm.tools.search") ─▶ dispatch_mcp: ctx.bridge = bridges[trace_id]
        invoke_mcp:  h ≥ 8                                  ─▶ limit.mcp_hops
                     "rivet:<bundle-hash16>/crm.tools.search" ∈ K ─▶ limit.mcp_recursion
                     else send _meta{rivet/hops: h+1, rivet/chain: K + [entry]} downstream
```

Both behaviours were verified by sending `tools/call contacts.find` to `rivet serve --listen
127.0.0.1:18444` with `_meta`:

```text
_meta {"rivet/hops":1,"rivet/chain":["rivet:upstream/x.tools.y"]}
   → success; downstream server saw
     _meta {"rivet/hops": 2, "rivet/chain": ["rivet:upstream/x.tools.y", "rivet:232af55e413a4e42/crm.tools.search"]}

_meta {"rivet/hops":8,"rivet/chain":["rivet:upstream/x.tools.y"]}
   → isError:true with {"kind":"limit","code":"limit.mcp_hops","message":"MCP bridge hop limit (8) reached calling `crm.tools.search`","retryable":true,"effects":"none",…,"details":{"hops":8,"chain":["rivet:upstream/x.tools.y"]}}
     (no downstream traffic)
```

`_meta` is added only to `tools/call`. The chain read from `_meta` is capped at 64 entries.

## Data and Storage

- Snapshots are ordinary files inside the bundle. They are read once at load (a bootstrap read, listed
  by `io --include-bootstrap`) and written only by sync, through a policed exclusive create.
- The catalog (`McpCatalog`: connectors plus imports) is immutable for the life of the `Runtime`.
  Changing a snapshot requires a reload.
- Nothing about sessions is persisted. Each call opens and closes its own session. The
  `bridges` map (trace_id → `BridgeHops`) lives only while a bridged request runs.

## Dependencies

| Dependency | Use |
|---|---|
| `transports.exchange_http` (`src/infra/http_adapter.rs`) | Streamable HTTP POST/DELETE with `allow_network`, SSE decoding (`StreamDecoder`) |
| Process runner + sandbox (`confine_process`; [ADR-0003](../../decisions/adr-0003-process-sandbox-backends.md)) | stdio children |
| Policed `FileAccess` | `tls` files, sync output |
| OAuth `CredentialProvider` ([SYS-2026-0006](sys-2026-0006-oauth-and-credentials.md)) | `auth PROFILE account "A"` bearer leases |
| `sha2`, `serde_json` | snapshot hash, JSON-RPC |

## Deployment

No extra service is needed. Operators must:

1. Run `connectors sync` with a dedicated refresh policy (discover plus write grants only).
2. Review the candidate file, then add its hash to `approved.snapshots` in the everyday policy.
3. Keep the `allow_mcp` grants in the everyday policy limited to the exposed imports.

On Linux and Windows, stdio connectors inherit the process sandbox status described in
[ADR-0003](../../decisions/adr-0003-process-sandbox-backends.md). Where no sandbox backend is active,
confined spawns are refused.

## Security Boundaries

- **Pinned schemas.** Only reviewed snapshot bytes define what can be called and with which params.
  Params are checked against the snapshot `inputSchema` (the supported keywords are type, enum,
  const, properties, required, additionalProperties, items, min/max, minLength/maxLength and
  anyOf/oneOf/allOf; other keywords are ignored). This check runs before any I/O.
- **Output checked too.** `structuredContent` must match the snapshot `outputSchema` when one is present.
- **Two grants per call.** The logical `allow_mcp` grant and the transport grant (`allow_network` or
  `allow_exec`) are both required. Neither implies the other.
- **Discovery is separately authorized** (`C/discover`) and can only create new files inside the bundle.
- **No server-initiated authority.** Sampling, elicitation and roots are declined with JSON-RPC
  `-32601`, and the call fails with `protocol.unsupported_capability`. Progress and log notifications
  are ignored and never become data.
- **Credentials.** OAuth is allowed only on HTTP transports and is bound to the endpoint origin. The
  endpoint is authorized before a token is requested, and a 401 invalidates the lease. Stdio
  connectors cannot carry `auth`.
- **Recursion.** A hop limit of 8 plus an identity chain stops Rivet → MCP → Rivet loops.

## Observability

- `rivet io`, `io --by target` and `io --check-policy` show the transport and `allow_mcp` sites. The
  remote part is `opaque_remote`, so `io --strict` exits 7.
- Trace attempts record every `allow_mcp`, `allow_network` and `allow_exec` decision (`rivet trace show REQ`).
- Errors carry JSON-RPC identity (`details.code`, `method`) or the tool's content (`mcp.tool_failed`).
- Sync prints a receipt JSON on stdout and the approval hint on stderr.

## Known Limitations

From the [manual's Known Limitations](../../manuals/man-2026-0001-rivet-manual.md#known-limitations):

- No legacy HTTP+SSE MCP transport and no resource templates.
- **MCP 401 invalidates the lease without retry**: a resource 401 drops the cached OAuth token, the call
  fails `http.status` (401), and the next call reacquires.
- No connection or session pooling: one session per call, so each call pays for `initialize` (and the drift
  check's `tools/list`).

Other current behaviour (by design): no subscriptions, sampling, elicitation or roots; the HTTP reply stream is
read only for the current request (server-to-client messages on a separate GET stream are not consumed); the
minimal JSON Schema checker ignores unsupported keywords such as `pattern`, `format` and `$ref`;
`docs/demos/06-mcp-bridge` targets a placeholder host, so its success path needs a live server.

## Last Verified Version

0.1.0-dev (commit 829ca43), on macOS (darwin 25.4.0), 2026-09-28. The drift check, sync-output check, opaque
effects and 8 MiB message cap were re-verified at `829ca43` (source, `tests/conformance_mcp.rs`, and the
`connectors sync` refusal against `docs/demos/06-mcp-bridge`). First verified at `f40d4aa`: commands were run with
`target/debug/rivet` from `docs/demos/06-mcp-bridge`. The full journey used scratch copies with a
fixture MCP server on `127.0.0.1:18443` and `rivet serve` on `127.0.0.1:18444`, both stopped
afterwards, plus a stdio child Rivet bundle.

## Related Documents

- [PROP-2026-0001 Rivet runtime proposal](../../proposals/approved/prop-2026-0001-rivet-runtime.md), Increment 6
- [PLAN-2026-0001 v0.1.0 implementation and release](../../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md)
- [ADR-0003 Process sandbox backends](../../decisions/adr-0003-process-sandbox-backends.md)
- [SYS-2026-0001 Compiler and catalog](../components/sys-2026-0001-compiler-and-catalog.md)
- [SYS-2026-0003 Policy broker and I/O manifest](../components/sys-2026-0003-policy-broker-and-io-manifest.md)
- [SYS-2026-0004 Surfaces and serve](../components/sys-2026-0004-surfaces-and-serve.md)
- [SYS-2026-0005 Protocol adapters](sys-2026-0005-protocol-adapters.md)
- [SYS-2026-0006 OAuth 2.0 and credentials](sys-2026-0006-oauth-and-credentials.md)
- [SYS-2026-0008 policy.json reference](../configuration/sys-2026-0008-policy-json-reference.md)
- [Language and usage reference](../../references/ref-2026-0002-language-and-usage.md)
- [Demos](../../demos/README.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial current-state document (PLAN-2026-0001 D-23). |
| 2 | 2026-09-28 | Claude | TASK-092 drift fix for the fix batch (829ca43): live `tools/list` drift check (`mcp.schema_drift`), sync refuses an existing `--output` before discovery, remote effects stay `unknown` when wrapped, 8 MiB message cap; limitations reduced to the current ones. |
