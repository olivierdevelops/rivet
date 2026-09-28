---
document_id: REF-2026-0002
title: "Rivet language and usage reference — 153 proposed examples"
document_type: reference
status: draft
created_date: 2026-09-27
last_updated: 2026-09-28
document_revision: 6
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, runtime, interfaces, sandbox]
affected_versions:
  from: not-applicable
  to: proposed-v0.1
applicable_environments: [development, embedded, server]
audience: [maintainers, developers, reviewers]
scope: Proposed Rivet behavior and design review; no implementation or release claim.
reason: Record the project brief and requested changes as reviewable contracts and examples.
dependencies: [PROJECT.md, DOCUMENTATION.md, AGENTS.md]
related_documents: ["PROP-2026-0001", "REF-2026-0001"]
supersedes: null
superseded_by: null
tags: [rivet, rust, capy, design]
confidentiality: internal
review_cycle: on-design-change
next_review_date: 2026-10-27
---

# Rivet language and usage reference — 153 proposed examples

**Design reference, not an installed or tested runtime.** This document specifies proposed syntax for [PROP-2026-0001](../proposals/draft/prop-2026-0001-rivet-runtime.md). Stage labels are `A`, `B` or `C`, matching the proposal's delivery matrix: Stage B (including every UDP, OAuth 2.0, QUIC/HTTP3, gRPC and session feature) is required scope; Stage C items (named pipes/FIFO, file watching, mTLS TCP, interactive processes, custom codecs, reconnect) are later, optional adapters. Shell commands are intended invocation contracts. Rust blocks follow the proposal's single canonical API sketch. DSL snippets use Capy prefix-call form and define target grammar; the Capy spike gate is that every S01–S153 block parses cleanly.

For complete files grouped by use case, see the [sample folders](../demos/README.md). The twelve bundles materialize selected examples below, with request bodies, local fixtures and per-folder READMEs; the numbered S01–S153 examples remain stable.

## Read this first

Start with S01–S06 for one operation across surfaces; S19–S32 for contextual lifetimes; S33–S43 for files; S44–S51 for workflows; S52–S61 for MCP; S62–S80 for inspection, policy, streaming and errors; S81–S102 for required UDP, OAuth 2.0, QUIC and HTTP/3 support; S103–S120 for multi-operation files, incoming MCP, gRPC and duplex sessions; S121–S128 for declared outputs and the commands that show them; S129–S132 for policy.json and serve authentication; S133–S137 for one `rivet serve` exposing every surface; S138–S140 for DAG completion, hard-link refusal and destination-bound secrets; S141–S153 (with the rewritten S62–S65) for the generated I/O manifest, access-narrowed policy and least-privilege policy drafts.

Save full operation/connector declarations in `app.rivet`. Body fragments are placed inside this explicit wrapper, choosing a suitable output/emits type:

```rivet
operation example.run
    output json
    # For fragments that emit: add `emits text`, `emits bytes`, or `emits json`.
    # Insert the sample body here, indented four spaces.
end
```

`output json` accepts any JSON-compatible value. A bytes return uses `output bytes`; emitted bytes are tagged for JSON surfaces. Samples with an early `return` are complete bodies; no extra return should be appended. Never concatenate all samples into one operation. Syntax intentionally uses four-space indentation, explicit `end`, object values, Capy prefix calls `(request "id" {...})`, quoted durations (`"10s"`) and `${dotted.path}` strings. Inside `map`/`poll` bodies `yield VALUE` produces the block value; `return` always exits the whole operation. Inline comments begin with `#`. Multiline text uses backticks; triple-quoted strings from the original brief are superseded.

Network examples use a deterministic fixture service at `api.example.com` (replace with a local fixture origin and update grants). Expected fixture behavior: GET `/users/42` returns `{"id":42,"name":"Ada"}`, GET `/search` returns a list, mutation routes accept the shown fields, `/chat` emits SSE data objects with `delta`, `/logs` returns UTF-8 lines, `/audio` returns bytes, and `/jobs/job_42` transitions pending→done. Endpoints are illustrative, not live services discovered or called during documentation work. `/v1/*` examples require a future running Rivet HTTP server. IDs `req_01`/`tr_01` are representative generated IDs, not fixed values.

Where an example shows a `policy.json`, that file sits beside the entry `.rivet` file for that run (auto-discovered); `--policy PATH` appears only when an example deliberately selects a different file. Examples without one run with no policy.json, which means deny-by-default for new application I/O. Local examples assume `./data`, `./out`, `./scratch`, `./audit`, `./schemas`, and named fixture binaries exist. These are runtime input/output directories in an example bundle, not new Rust source buckets. Create fixtures explicitly outside the script or through granted file operations. Platform-specific examples return `unsupported` where the build lacks that capability.

## Syntax summary

The proposal's [syntax table](../proposals/draft/prop-2026-0001-rivet-runtime.md#increment-1--rust-capy-syntax-and-the-compilation-boundary) and [error registry](../proposals/draft/prop-2026-0001-rivet-runtime.md#increment-2--requests-streams-and-errors) are authoritative; this section restates them with an example per form.

```text
operation users.get                         <- header: stable callable ID
    name "Get a user"                       \
    description "Read one user."             |
    private true              (optional)     |  header area, in this order,
    param id integer required ...            |  all before the first body
    output object description "..."          |  statement (leading-options
        field id integer required ...        |  rule); out-of-order or late
    end                                      |  lines -> syntax.option_after_body
    emits json                (optional)     |
    receives json             (optional)     |
    error "users.not_found" description "..."/
    ---------------------------------------- body starts here
    response = http get "https://api.example.com/users/${id}"
        decode json                         <- resource options lead ...
        timeout "10s"
    end                                     <- ... then (none here) body statements
    return response.body
end
```

### Statement shapes

| Keyword | Shape | Example |
|---|---|---|
| `operation` / `pipeline` | `operation ID` … `end` | `operation demo.add` |
| `name` | `name "LABEL"` | `name "Add two integers"` |
| `description` | `description "TEXT"` | `description "Add two signed integers."` |
| `private` | `private true` | `private true` (hidden from every surface) |
| `param` | `param NAME TYPE required\|default V [min N] [max N] [enum [...]] [description "…"]` | `param b integer default 0 description "Second operand."` |
| `output` (scalar) | `output TYPE [description "…"]` | `output integer description "Sum of a and b."` |
| `output` (structured) | `output object [description "…"]` NL `field`* [`open true`] NL `end` | S122 |
| `field` | `field NAME TYPE [required\|optional] [description "…"]`; `object` fields open a nested block closed by `end`; `list T` for lists | `field tags list text optional description "Free-form labels."` |
| `emits` / `receives` | `emits TYPE` or `emits object … end` (same field form) | `emits text`, `receives json` |
| `error` | `error "CODE" description "…"` (one per code, header only) | `error "users.not_found" description "No user has this ID."` |
| assignment | `NAME = EXPR`, `NAME += EXPR` | `text += event.data.delta` |
| `request` | `(request ID PARAMS)` | `user = (request "users.get" {id: id})` |
| `request.stream` | `with (request.stream ID PARAMS) as NAME` … `end` | `with (request.stream "chat.reply" {prompt: "Hello"}) as events` |
| `allow` | call followed by `allow [IDS]` … `end` | `value = (request turn.operation turn.params)` / `allow ["users.get"]` / `end` |
| prefix call | `(FN ARG …)`; named args become one trailing object | `(rpc.result response.body {expected_id: 1})`, `(length value)` |
| `return` | `return EXPR` — exits the whole operation | `return response.body` |
| `yield` | `yield EXPR` — value of a `map`/`poll` body | `yield (request "users.get" {id: item})` |
| `emit` | `emit EXPR` — one stream item; needs `emits` | `emit event.data.delta` |
| `fail` | `fail "CODE" {DETAILS}` | `fail "users.not_found" {id: id}` |
| `if` | `if COND` … [`else` …] `end` | `if response.status == 404` |
| `for` | `for NAME in ITER` … `end` | `for event in events` |
| `while` | `while COND` … `end`; needs a finite deadline; `while true` only inside a scope/operation with a finite `timeout` | S20 |
| `break` | `break` | exits the innermost loop and disposes its scope |
| `try` | `try` … `catch error kind K` \| `catch error code "C"` … [`finally` …] `end` | `catch error code "http.status"` |
| `iterate` | `iterate max N` … `end` | `iterate max 3` |
| `poll` | `poll every "D" timeout "D"` … `until COND` … `yield V` … `end` | S48 |
| `map` | `map NAME in LIST limit N` … `yield V` … `end` | S47 |
| `dag` / `node` | `dag [limit N] [timeout "D"] [fail fast\|fail independent]` … `node NAME [after [A, B]] = EXPR` … `end` (default `fail fast`) | `node summary after [user, orders] = (request "summary.make" {…})` |
| `concurrent` / `task` | `concurrent limit N [timeout "D"] fail fast\|independent` … `task NAME` … `end` … `end` | S21 |
| `scope` | `scope timeout "D"` … `end` | `scope timeout "20s"` |
| `with` | `with RESOURCE TARGET [mode M] as NAME` NL options* NL body* `end` | `with tcp "127.0.0.1:9000" as conn` |
| `http` | `http METHOD URL` NL options* `end` | `response = http get "https://api.example.com/search"` |
| `grpc` | `grpc CONNECTOR.Method` NL options* `end` | `response = grpc users.GetUser` |
| `command` | `command BINARY` NL options* `end` | `result = command "/usr/bin/printf"` |
| `file` | `file VERB PATH [json\|text\|bytes V] [to PATH]` [options `end`] | `file create "./out/config.json" json {enabled: true}` |
| `secret` | `secret NAME from env "VAR" for "ORIGIN"` | `secret API_KEY from env "EXAMPLE_API_KEY" for "https://api.example.com:443"` |
| `connector` | `connector NAME mcp\|grpc` NL options* `end` | `connector users grpc` |
| `auth` (profile) | `auth NAME oauth2` NL options* `end` | `auth crm_service oauth2` |
| `auth` (use) | `auth PROFILE account "ACCOUNT"` inside `http`/`grpc`/`connector` | `auth crm_service account "service"` |
| comment | `# …` to end of line | `# fixture only` |
| string | `"…"` with escapes `\n \t \" \\ \xNN \uNNNN`; `${dotted.path}` interpolation; backtick multiline | `"Hello, ${person}!"`, `"\x00"` |
| duration | quoted string matching `^[0-9]+(ms\|s\|m\|h)$` | `"100ms"`, `"5s"`, `"2m"` |

### Resource options (leading-options rule)

Option lines must precede the first body statement of their block; an option after a statement is `syntax.option_after_body` (exit 2). Header-clause options (`limit`, `timeout`, `fail …`) sit on the opening line.

| Resource / block | Valid option lines |
|---|---|
| `http METHOD URL` | `query K V`, `header K V`, `body json\|text\|xml\|form\|multipart … end\|bytes\|file`, `decode json\|text\|bytes\|xml\|form`, `accept status [...]`, `timeout "D"`, `redirect follow limit N`, `retry N on status [...] backoff … base "D" max "D" jitter B`, `version 3` / `version prefer [3, 2]`, `auth PROFILE account "A"`, `unix "PATH"` |
| `with http … as NAME` | all `http` options plus `stream sse\|jsonl\|lines\|bytes` |
| `with websocket URL as NAME` | `timeout "D"`, `reconnect N backoff … max "D"`, `resume none` |
| `with tcp HOST:PORT as NAME` | `framing newline\|length32 endian E\|delimiter "S"\|raw [max_frame N]`, `tls true`, `tls server_name\|ca_file\|cert_file\|key_file V`, `timeout "D"` |
| `with unix PATH as NAME` / `with pipe PATH mode M as NAME` | `framing …`, `timeout "D"` |
| `with udp HOST:PORT` / `udp bind` / `udp multicast` | `max_datagram N`, `timeout "D"`; multicast adds `bind "HOST:PORT"`, `interface "IF"` |
| `with quic URL as NAME` | `alpn "ID"`, `max_streams N`, `migration true\|false`, `datagrams true`, `timeout "D"` |
| `with connection.open\|accept uni\|bidi as NAME` | `framing …`, `timeout "D"` |
| `grpc CONNECTOR.Method` / `with grpc … as NAME` | `message {…}`, `metadata K V`, `auth PROFILE account A`, `timeout "D"` |
| `command BIN` / `with command BIN as NAME` | `args [...]`, `stdin json\|text\|bytes V`, `env {…}`, `timeout "D"`, `decode stdout\|stderr T`, `stream stdout T`, `interactive true` |
| `with file open PATH mode M as NAME` | `chunk_size N` |
| `with file watch PATH as NAME` | `debounce "D"` |
| `file update` / `file delete` / `file copy\|move` | `if_version V` / `missing ok` / `overwrite false` |
| `connector NAME mcp` | `transport http URL` or `transport command BIN` (+ `args`/`env` … `end`), `auth PROFILE account "A"`, `schema PATH`, `expose tools\|resources\|prompts [...]` |
| `connector NAME grpc` | `endpoint URL`, `descriptor PATH`, `service NAME` |
| `auth NAME oauth2` | `flow`, `pkce s256`, `issuer`, `authorization_url`, `device_url`, `token_url`, `client_id`, `client_secret env "VAR"`, `client_auth basic\|post\|none`, `redirect_uri`, `scopes [...]`, `resource_origins [...]`, `store memory\|keychain "NS"` |

### Error registry

One registry governs every surface ([proposal](../proposals/draft/prop-2026-0001-rivet-runtime.md#increment-2--requests-streams-and-errors)). Codes not listed follow their kind's row.

| Kind / code | Meaning | HTTP | CLI exit | Retryable |
|---|---|---|---|---|
| `syntax` (incl. `syntax.option_after_body`, `registry.duplicate_id`, `check.call_cycle`) | Source does not parse/check | 422 | 2 | no |
| `validation` (incl. `validation.required`, `stream.input_required`, `stream.required`, `policy.invalid`, `serve.auth_required`) | Bad params/usage/config, policy.json errors | 422 (`serve.auth_required`: startup only) | 2 | no |
| `auth` (caller) | Missing/bad server credentials on a serve surface | 401 | 3 | no |
| `permission` (incl. `permission.denied`, `file.hardlink_refused`) | Policy denies the effect or operation | 403 | 3 | no |
| `not_found` | Unknown ID, file, session | 404 | 4 | no |
| `conflict` (incl. `already_exists`, `conflict.exists` from `policy generate --output`, OAuth lifecycle `auth.login_required` …) | State conflict | 409 | 4 | no |
| `limit` (incl. `limit.call_depth`, session/ref caps) | Budget exceeded | 429 | 5 | yes, after backoff |
| `timeout` | Deadline exhausted | 504 | 6 | caller decides |
| `connection`, `dns`, `tls`, `protocol` (incl. `udp.truncated`, `quic.*`), `http` (`http.status` + `details.status`), `grpc.<code>`, `application` (incl. `mcp.tool_failed`) | Dependency failure | 502 | 5 | only if replay-safe |
| `unsupported` (incl. `unsupported.sandbox_backend`) | Feature/platform unavailable | 501 | 5 | no |
| `output_invalid` (`output.invalid`) | Result violates declared output; effects preserved | 500 | 5 | no |
| `internal`, `cleanup`, `consumer_failed`, `parse`, `process` | Runtime fault / cleanup / sink failure | 500 (`process`, upstream `parse`: 502) | 5 | no |
| `io --check-policy` result | a reachable site is `denied` or `partial` (not an error envelope; the manifest is still printed) | — | 3 | no |
| inspection incomplete | `io --strict` found dynamic/opaque sites; `policy generate` left review items (draft still written) | — | 7 | no |
| `cancelled` | Caller or parent cancelled | 409 before headers; terminal event after | 130 | no |

```text
exit:  0 ok   2 syntax/validation/usage/config   3 permission/auth   4 not_found/conflict
       5 dependency/runtime/unsupported/output_invalid   6 timeout   7 inspection incomplete   130 cancelled
```

## Named operation fixtures used by composition examples

These contracts are explicit sample dependencies; they are not proposed magic built-ins. Define them in the test fixture bundle before running composition examples. Their implementations must use the same protocol/file primitives and policy broker.

| Fixture ID | Input → output | Behavior/effects |
|---|---|---|
| `orders.list` | `{user_id: integer}` → JSON list | GET /orders with user_id; network |
| `summary.make` | `{user: json, orders: json}` → JSON object | Pure object assembly |
| `fixture.echo` | `{value: json}` → json | Returns value; pure |
| `fixture.fail` | `{}` → json | Raises application.fixture_failure; pure |
| `files.create_record` | `{path: text}` → json | Exclusive file create; write |
| `files.update_record` | `{path: text}` → json | Existing file update; write |
| `text.refine` | `{text: text}` → text | POST /refine; network; no automatic retry |
| `agent.next` | `{messages: json}` → json | POST /agent/turn; validates final/tool-call union; network |
| `orders.create` | `{item: text}` → `{id: text}` | POST /orders; network mutation |
| `payments.charge` | `{order_id: text}` → json | POST /payments; network mutation |
| `orders.cancel` | `{id: text}` → json | POST /orders/{id}/cancel; network mutation |
| `fixture.mcp_sampling` | `{}` → json | Test MCP peer requests unsupported sampling |
| `fixture.shell` | `{}` → json | Test unsupported shell refusal; never spawns |
| `fixture.print` | `{text: text}` → text | /usr/bin/printf with argv; process capability |

## Core syntax and semantics

| Form | Rule |
|---|---|
| `operation id` / `pipeline id` | Both register a typed callable. Pipeline emphasizes composition; no second dispatch mechanism. |
| `name "Display label"`, `description "..."` | Per-operation metadata; header remains stable callable ID. |
| `param name type required/default value description "..."` | Unknown input fields rejected; defaults applied only after field-name checks. `min`, `max`, `enum` add constraints. |
| `receives type` | Declares scoped live `incoming` stream; independent of initial params. |
| `output type [description "..."]`, `output object ... end`, `emits type` | Final result and streamed data are separate schemas; the result is validated before Completion (`output.invalid`). No implicit empty-frame filtering. |
| `name = expression`, `name += expression` | Local mutable value; no shared cross-task mutation. Lists append lists, strings append strings. |
| `(request "id" {params})` | Returns child's final value for non-streaming operations. Context inherited. Prefix call form. |
| `with (request.stream "id" {params}) as events` | Explicit stream ownership; data/result envelopes; terminal errors raise. |
| `with acquire ... as handle` | Acquisition, body, awaited disposal; lexical handle cannot escape. |
| `scope timeout "20s" ... end` | Groups child resources/tasks under one lifetime and deadline. |
| `http method url ... end` | Finite response object `{status, headers, body}`; decode changes body only. |
| `with http method url as stream ... end` | Config directives precede executable body; requires `stream sse/jsonl/lines/bytes`. |
| `command binary ... end` | Direct argv execution; finite result `{stdout,stderr,exit,duration}`. |
| `with command binary as process` | Scoped streaming/interactive process; bounded stdout/stderr. |
| `file verb ...` | One-shot operation; an optional option block ends with `end`. |
| `if condition ... else ... end` | Ordinary conditional; missing object keys and out-of-range indexes are typed errors. |
| `for value in iterable ... end` | Iterate value or owned stream; break stops iteration and exits relevant scope. |
| `try ... catch error kind/code ... finally ... end` | Match typed error; generic catch excludes cancellation; finally does not grant new effects. |
| `fail "code" {details}` | Application failure with safe structured details and causal context. |
| `poll every "D" timeout "D" ... end` | Evaluate body; `until` checks completion; `yield` supplies the block value only when terminal; `return` exits the operation. |
| `iterate max N` | Bounded local sequential loop; not a cyclic DAG edge. |
| `dag limit N timeout "D" fail fast/independent` | Explicit dependency graph; omitted policy = `fail fast`. Each node value is `{status, result, error}`; `.result` is null unless the node succeeded. |
| `map item in list limit N ... yield V ... end` | Bounded fan-out, result order matches input; `yield` produces each item's value. |
| `concurrent ... task name ... end ... end` | Structured parallel child tasks; join all before scope exit. |

Every call uses the same Capy prefix form `(name arg ...)`; named arguments become one trailing object, e.g. `(rpc.result response.body {expected_id: 1})`. Parenthesized pure helpers include `(length value)`, `(base64.decode text)`, `(base64.encode bytes)` and `(xml.element name attributes content)`. These helpers never perform I/O. Expression helpers, interpolation, response decoding and JSON object construction preserve secret taint. Raw bytes are first-class internally; JSON encoding is `{"$type":"bytes","base64":"AAEC"}`. Integers are signed 64-bit; JSON surfaces use ordinary numbers only within exact interoperable range, otherwise `{"$type":"integer","decimal":"9223372036854775807"}`. NaN/infinity cannot cross JSON surfaces.

HTTP body forms: `body json object`, `body text string`, `body xml xml_value`, `body form object`, `body multipart ... end`, `body bytes bytes_value`, `body file path`. File bodies add read effects. `decode json/text/bytes/xml/form` affects response.body; XML parsing disables external entities and network/file resolution. Unknown codecs/options fail at compile time.

Socket framing: newline, length32 with explicit endian, delimiter with bounded length, and raw (read yields chunks, not messages). WebSocket preserves frame/message boundaries; UDP preserves datagrams; neither uses TCP framing. JSON-RPC is an application layer on these transports; the runtime validates IDs and distinguishes notifications/errors rather than treating the next arbitrary frame as the requested response.

## Policy file (policy.json)

Policy comes only from a JSON file; there are no grant strings on the command line and no environment-variable policy. The `--sandbox` flag is removed (UQ-17 supersedes UQ-13's flag syntax and keeps its deny-by-default behaviour).

```text
 rivet --file ./svc/app.rivet request ...        rivet --file ./svc/app.rivet --policy ./ci.json request ...
             |                                                  |
             v                                                  v
   ./svc/policy.json exists? --no--> deny-by-default     ./ci.json (path only; never grants)
             | yes                   (pure ops still run)       |
             v                                                  v
   parse + validate schema v1 --error--> policy.invalid, exit 2 (unknown key/capability, bad selector)
             |
             v
   effective authority = host ceiling  ∩  policy.json  ∩  per-request restriction (HTTP/MCP callers narrow only)
             |
             v
   deny entries override grants;  network.deny_private_ranges (default true) applied after DNS/redirects
```

Schema v1 (every key except `version` is optional; targets resolve relative to the policy file's directory):

```json
{
  "version": 1,
  "grants":   [{"capability": "allow_read",    "targets": ["./data/**"]},
               {"capability": "allow_write",   "targets": ["./out/**"], "access": ["create"]}],
  "deny":     [{"capability": "allow_network", "targets": ["https://internal.example.com:443"]}],
  "network":  {"deny_private_ranges": true},
  "limits":   {"max_concurrent_requests": 64, "max_call_depth": 16, "max_buffered_bytes": 268435456},
  "approved": {"snapshots": ["sha256:..."], "overlaps": ["sha256:..."]},
  "serve":    {"surfaces": ["http", "sse", "poll", "ws", "mcp"],
               "auth": {"type": "none"},
               "principals": {"local": {"operations": ["*"]}}}
}
```

| Key | Rule |
|---|---|
| `grants` / `deny` | Capabilities `allow_read`, `allow_write`, `allow_delete`, `allow_network`, `allow_exec`, `allow_env`, `allow_unix`, `allow_pipe`, `allow_listen`, `allow_mcp`, `allow_grpc`, `allow_auth`, `allow_credentials`. `deny` overrides `grants`. `"*"` is an explicit broad target and `policy explain` flags it. |
| `grants[].access` / `deny[].access` | Optional list of [access verbs](#access-verbs) that narrows the entry. Absent = every verb of that capability (backwards compatible). A verb that does not belong to the entry's capability → `policy.invalid`, exit 2. `{"capability":"allow_write","targets":["./out/**"],"access":["create"]}` allows creating files in `./out` but never overwriting or appending (S146). |
| `network.deny_private_ranges` | Default true in all modes: RFC1918, loopback, link-local, 169.254.169.254, fc00::/7 and fe80::/10 are denied unless a grant names that IP/CIDR literally. |
| `limits` | Host-wide budgets across nested calls, DAGs and sessions. |
| `approved` | A snapshot/overlap is "reviewed" only when its sha256 is listed here; otherwise loading it fails. |
| `serve` | Surfaces, caller authentication and per-principal operation authorization (see [One serve, every surface](#one-serve-every-surface)). |

### Access verbs

Every effect site performs one or more **access verbs**; each verb belongs to exactly one capability. `rivet io` reports the verbs per site, `grants[].access` / `deny[].access` narrow by them, and `policy generate` emits exactly the verbs used.

```text
    kind        access verbs                                  capability
    ----------  --------------------------------------------  -----------------
    file        read, list, stat, watch                       allow_read
    file        create, update, append                        allow_write
    file        delete                                        allow_delete
    network     connect (+ protocol, + method for HTTP)       allow_network
    network     bind, listen, multicast_join                  allow_listen
    process     exec                                          allow_exec
    env         read                                          allow_env
    pipe        read, write                                   allow_pipe
    unix        connect, listen                               allow_unix
    mcp         call (tool|resource|prompt)                   allow_mcp (+ transport's allow_network/allow_exec)
    grpc        call (unary|server_stream|client_stream|bidi) allow_grpc (+ allow_network)
    auth        use, manage, status                           allow_auth
    credential  read, write                                   allow_credentials
```

| Rule | Detail |
|---|---|
| HTTP detail | HTTP sites also record `method` (GET, POST, …) and `protocol` (`http1`, `http2`, `http3`, `ws`, `sse`, `grpc`, `quic`, `udp`, `tcp`). `access` narrows verbs only; methods are reported, not granted separately. |
| Several sites per line | One source line can produce several sites, each listed: `file update` = `stat` (allow_read) + `update` (allow_write); an MCP stdio call = `exec` (allow_exec) + `call` (allow_mcp). |
| DSL → verb | `file read` → read · `file list` → list · `file stat` → stat · `with file watch` → watch · `file create` → create · `file update` → stat + update · `file write` → create or update (both listed) · `file append` → append · `file delete` → delete · `file copy/move` → read at source + create at destination (+ delete at source for move). |

```text
 policy.json entry                      verbs allowed                     S-example
 -------------------------------------  --------------------------------  ---------
 allow_write  ./out/**                  create, update, append            S67
 allow_write  ./out/**  ["create"]      create                    ──────► S146
 allow_read   ./data/** ["delete"]      (invalid: delete ∉ allow_read) ─► S147 policy.invalid, exit 2
```

Library hosts use `Policy::from_file(path)` or `Policy::from_json(bytes)` with the same schema; the host ceiling intersects it. The "Required authority" line of each example uses `capability=target` as shorthand for one `grants` entry; that shorthand is documentation only and is accepted by no command.

## I/O manifest (rivet io)

`rivet io` produces a generated **I/O manifest** (R26, UQ-18): every I/O site, the concrete target it uses (URL, path, host:port, argv, env var, connector method…), the [access verbs](#access-verbs) it performs and the capability that permits them. Nothing is executed: no I/O is performed and source expressions are not evaluated.

```text
app.rivet ──parse/lower──▶ effect sites ──normalize──▶ IoManifest ──┬─▶ io --by operation|target|capability
                               │                                    ├─▶ io --check-policy  (◀── policy.json)
                               │                                    ├─▶ policy generate ──▶ draft policy.json
                               └──effect_id──▶ runtime trace ───────┴─▶ io --trace REQ (planned vs actual)
```

```sh
rivet --file app.rivet io [ID ...] [--all] [--transitive] [--include-bootstrap]
      [--by operation|target|capability] [--kind file|network|process|env|pipe|unix|mcp|grpc|auth|credential]
      [--access VERB[,VERB]] [--format table|json|markdown|csv] [--check-policy] [--strict] [--trace REQUEST]
rivet --file app.rivet policy generate [ID ...|--all] [--output PATH]
```

| Option | Rule |
|---|---|
| entry | Default: every public operation, transitively (`--transitive` is the default and may be written explicitly). `--all` adds private helpers and operations unreachable from any public entry. |
| `--by` | `operation` (default) · `target` (what every URL/path is used for) · `capability` (rows grouped under allow_read / allow_write / …). |
| `--kind`, `--access` | Filter sites by kind and by access verb (comma list). An unknown kind or verb is a usage error, exit 2. |
| `--format` | `table` (default) · `json` (IoManifest) · `markdown` (the same tables, for reviews) · `csv` (one row per site). |
| `--check-policy` | Evaluates each site against host ceiling ∩ policy.json and adds `DECISION`: `allowed` · `denied` · `partial` (a param_dependent target only partly covered) · `unknown` (dynamic/opaque). Exit 3 when any reachable site is denied or partial. |
| `--strict` | Exit 7 when any site is dynamic/opaque (`complete:false`). |
| `--trace REQUEST` | Adds an `ATTEMPTS` column (count, last decision) joined on `effect_id` from that request's trace. |
| targets | URLs as `scheme://host:port` + path template, interpolations as `{param}`; paths relative to the bundle root with `{param}` segments and the derived glob for param_dependent paths (`./out/notes/*.json`). |
| knowledge | `exact` · `bounded` · `param_dependent` · `dynamic` · `opaque_remote` · `opaque_native`. |

`policy generate` builds a policy.json v1 draft: one grant per (capability, target) with `access` narrowed to the verbs used (omitted when the verbs used are the capability's whole verb set, e.g. `connect` for allow_network); exact targets as-is, param_dependent URLs → origin, param_dependent paths → derived glob, `network.deny_private_ranges: true`. Dynamic/opaque sites are not granted: they go to stderr as review items and the exit code is 7 (the draft is still written). Output goes to stdout unless `--output PATH`, which refuses to overwrite (`conflict.exists`, exit 4). The draft never contains `serve` auth or secrets and is a starting point for human review, not approval.

Every surface has the same inventory: built-in operations `rivet.io` and `rivet.policy.generate` ([built-in tools](#complete-cli-and-registry-mapping)), HTTP `GET /v1/io?by=target&kind=file&check_policy=true` and `POST /v1/policy/generate`, MCP tools of the same names, and library `rt.io(IoQuery) -> IoManifest` / `rt.generate_policy(&[ids]) -> PolicyDraft`. Inventories reveal internal URLs and paths, so a remote principal may call `rivet.io` / `rivet.policy.generate` only when its `serve.principals.<name>.operations` lists them explicitly; a `*` or `demo.*` pattern never matches these two IDs. The loopback principal `local` may call them.

### I/O manifest fixture bundle

S62–S65 and S141–S153 use this `app.rivet`. `SOURCE` columns (`app.rivet:6`) count lines from the first line of this block.

```rivet
operation users.get
    description "Read one user."
    param id integer required min 1 description "User ID."
    output json description "The user."
    secret api_key from env "API_KEY" for "https://api.example.com:443"
    response = http get "https://api.example.com/users/${id}"
        header "Authorization" "Bearer ${api_key}"
        decode json
    end
    return response.body
end

operation users.create
    description "Create a user."
    param name text required description "Display name."
    output json description "The created user."
    secret api_key from env "API_KEY" for "https://api.example.com:443"
    response = http post "https://api.example.com/users"
        header "Authorization" "Bearer ${api_key}"
        body json {name: name}
        decode json
    end
    return response.body
end

pipeline users.snapshot
    description "Fetch a user and save it once."
    param id integer required min 1 description "User ID."
    output json description "The saved user."
    user = (request "users.get" {id: id})
    file create "./out/user.json" json user
    return user
end

operation report.load
    description "Load the input report."
    output json description "Parsed input."
    report = file read "./data/input.json" as json
    return report
end

operation notes.create
    description "Create one note; never overwrite."
    param name text required description "Note name."
    param body text required description "Note text."
    output json description "Receipt."
    file create "./out/notes/${name}.json" json {body: body}
    return {created: name}
end

operation notes.update
    description "Replace one existing note."
    param name text required description "Note name."
    param body text required description "Note text."
    output json description "Receipt."
    file update "./out/notes/${name}.json" json {body: body}
    return {updated: name}
end

operation notes.delete
    description "Delete one note."
    param name text required description "Note name."
    output json description "Receipt."
    file delete "./out/notes/${name}.json"
    return {deleted: name}
end

operation sync.push
    description "Post a heartbeat to the endpoint named in a config file."
    output json description "Upstream reply."
    config = file read "./data/endpoint.json" as json
    response = http post config.url
        body json {ok: true}
        decode json
    end
    return response.body
end

operation archive.purge
    description "Remove the old archive file; no public operation calls it."
    private true
    output json description "Receipt."
    file delete "./out/archive/old.json"
        missing ok
    end
    return {purged: true}
end
```

```text
 line  site(s)                                                   knowledge
 ----  --------------------------------------------------------  ---------------
    5  env read API_KEY                        (secret, bound)   exact
    6  network connect GET  https://api.example.com/users/{id}   param_dependent
   17  env read API_KEY                        (secret, bound)   exact
   18  network connect POST https://api.example.com/users        exact
   30  call users.get                          (not a site)      —
   31  file create ./out/user.json                               exact
   38  file read   ./data/input.json                             exact
   47  file create ./out/notes/{name}.json                       param_dependent
   56  file stat   ./out/notes/{name}.json   ┐ one line,         param_dependent
   56  file update ./out/notes/{name}.json   ┘ two sites         param_dependent
   64  file delete ./out/notes/{name}.json                       param_dependent
   71  file read   ./data/endpoint.json                          exact
   72  network connect POST {config.url}                         dynamic
   83  file delete ./out/archive/old.json      (private, --all)  exact
```

## One serve, every surface

```text
                           rivet --file app.rivet serve --listen 127.0.0.1:8080
                                         |
      +----------------+-----------------+------------------+-----------------+
      |                |                 |                  |                 |
   REST (http)      SSE (sse)       polling (poll)     WebSocket (ws)     MCP (mcp)
 POST /v1/request  POST /v1/request POST /v1/requests   GET /v1/ws        POST|GET|DELETE /mcp
 GET /v1/operations Accept: text/   GET /v1/requests/   subprotocol       Streamable HTTP
 GET /v1/operations/{id}  event-stream   {id}/events     rivet.v1
 GET /v1/operations/{id}/outputs          ?after_seq&wait_ms
                                      POST /v1/requests/{id}/input
                                      POST /v1/requests/{id}/finish_input
                                      POST /v1/requests/{id}/cancel
      +----------------+-----------------+------------------+-----------------+
                                         |
                   one auth -> principal -> operation authorization
                                         |
                     shared dispatcher / immutable catalog / broker
```

- `rivet serve [--listen HOST:PORT]` (default `127.0.0.1:8080`) mounts all five surfaces on one listener; `rivet serve --stdio` serves MCP over stdio only. The `--transport` and `--mcp` flags are removed.
- Polling is the HTTP projection of `rivet.sessions.*`: POST `/v1/requests` returns 202 SessionReceipt with `events_url`; events = `sessions.read` (SessionBatch); input = `sessions.send`. `{id}` is the receipt's session ID; clients follow `events_url`. Unary operations can also be submitted this way; their batch has one terminal result event.
- WebSocket `/v1/ws` (subprotocol `rivet.v1`, JSON text frames) multiplexes client-chosen `ref`s: at most 8 in flight per connection, 16-frame queue per ref, exactly one terminal frame per ref; closing the socket cancels and joins its refs (polling sessions, by contrast, survive reconnect). WS adds no WS-only operations.
- MCP at `/mcp`: direct named tools are canonical (`tools/call {name:"users.get", arguments:{id:42}}`); built-in generic tools are listed below.
- `serve.auth` in policy.json: `{"type":"none"}` (loopback binds only; principal `local`), `{"type":"bearer","tokens":[{"principal":"ada","sha256":"<hex>"}]}` or `{"type":"mtls","client_ca":"./ca.pem","principals":[{"principal":"ci","subject":"CN=ci"}]}`. Missing `serve.auth` means none. A non-loopback bind with auth none refuses to start (`serve.auth_required`, exit 2). The same principal applies on every surface (`Authorization: Bearer` on HTTP, the WS upgrade request and MCP HTTP requests); library hosts may supply an `Authenticator` callback.
- `GET /v1/io` (query `by`, `kind`, `access`, `check_policy`, `ids`, `all`) and `POST /v1/policy/generate` are REST routes on the same listener. Because inventories reveal internal URLs and paths, only the loopback principal `local` or a principal whose `operations` list names `rivet.io` / `rivet.policy.generate` explicitly may call them; `*` and `demo.*` patterns never match these two IDs (S150–S151).
- `serve.principals` maps principal → allowed operation patterns; absent map → every authenticated principal may call every public operation. `serve.surfaces` narrows the mounted surfaces (default all five); a disabled surface returns 404. `serve` without a policy.json runs on loopback with auth none: pure operations work and all effects are denied.

## Complete CLI and registry mapping

Global `--file` selects a trusted bundle input. Policy comes only from a policy.json file (see [Policy file](#policy-file-policyjson)); `--policy PATH` selects a different file and takes a path only, never grants. `--timeout` sets a capped request deadline. `--params` is JSON text; `--params-file` is an explicitly authorized invocation input. `--endpoint URL` routes request/inspection/auth aliases to an existing server via `/v1/request` and is mutually exclusive with `--file`; it preserves server-owned memory transactions across CLI calls. The HTTP caller supplies no new server authority: effective authority is host ceiling ∩ policy.json ∩ per-request restriction, so callers can only narrow. No evaluation of shell snippets or environment expansion inside the runtime. The shell surrounding a CLI invocation remains the user's shell.

| CLI alias | Registry ID / params | Output and effect |
|---|---|---|
| `request ID --params JSON [--stream] [--input-jsonl -]` | ID directly | Completion or NDJSON events |
| `list --json` | `rivet.list {cursor?,limit?}` | Visible ID descriptors |
| `describe ID --json` | `rivet.describe {id}` | Input/output/emits/effects, plus an Output section (params + output + errors) |
| `outputs ID [--json]` | `rivet.outputs {id}` | Human table, or JSON Schema `{id,output,emits,receives,errors}` |
| `outputs --all [--json]` | `rivet.outputs {all:true}` | Same, for every public operation |
| `list --outputs` | `rivet.list {cursor?,limit?}` | Adds a one-line output summary column |
| `check [--strict-docs]` | `rivet.check {strict_docs?}` | Diagnostics from loaded bundle; no execution |
| `io [ID ...] [--all] [--transitive] [--include-bootstrap] [--by operation\|target\|capability] [--kind KIND] [--access VERB,…] [--format table\|json\|markdown\|csv] [--check-policy] [--strict] [--trace REQ]` | `rivet.io {ids?,all?,by?,kind?,access?,check_policy?}` | IoManifest: every site's target, access verbs and capability; exit 3 on denied/partial with `--check-policy`, 7 on unknowns with `--strict` ([I/O manifest](#io-manifest-rivet-io)) |
| `policy generate [ID ...\|--all] [--output PATH]` | `rivet.policy.generate {ids?,all?}` | Least-privilege policy.json draft on stdout (or a new file); review items on stderr, exit 7 if any; never overwrites (exit 4) |
| `graph ID --json` | `rivet.graph {id}` | DAG/call/effect graph |
| `policy explain ID --params JSON --json` | `rivet.policy.explain {id,params}` | Missing grants without issuing a permit |
| `trace show REQUEST --json` | `rivet.trace.show {request_id}` | Authorized trace from current host store |
| `trace export REQUEST --output PATH` | `rivet.trace.export {request_id,path}` | Brokered sanitized export |
| `connectors sync NAME --output PATH` | `rivet.connectors.sync {name,path}` | Authorized discovery and candidate snapshot |
| `request rivet.capabilities --params '{}'` | `rivet.capabilities {}` | Build/platform support, no I/O |
| `request rivet.cancel --params '{"request_id":"..."}'` | `rivet.cancel {request_id}` | Idempotent cancellation receipt |
| `auth begin PROFILE --account ACCOUNT` | `rivet.auth.begin {profile,account}` | Expiring code/device challenge; no token result |
| `auth complete --params JSON` or `--params-file PATH` | `rivet.auth.complete {transaction_id,callback?,wait?}` | Validated exchange/poll, then sanitized CredentialStatus |
| `auth status PROFILE --account ACCOUNT` | `rivet.auth.status {profile,account}` | Status without refresh |
| `auth disconnect PROFILE --account ACCOUNT` | `rivet.auth.disconnect {profile,account}` | Local deletion/generation receipt, not provider revocation |
| `serve [--listen HOST:PORT]` (default `127.0.0.1:8080`) | Host lifecycle bootstrap, not an application operation | One listener with REST, SSE, polling, WebSocket and MCP mounted; never callable by an unprivileged request |
| `serve --stdio` | Host lifecycle bootstrap | MCP over stdio only (cannot share a socket) |

Administrative operation IDs use the same dispatcher and can be reached over HTTP/library/MCP when the principal is authorized. A remote request cannot make `serve` open another listener: bootstrap establishes the host, rather than being an application API. `rivet.check` describes the loaded bundle; uploading/compiling arbitrary new programs over HTTP is not part of this proposal. Library hosts can compile supplied bytes directly.

Built-in generic operations are also MCP tools. Direct named tools (one per public operation) are the canonical MCP way to call an operation; these generic tools exist so every authorized registry operation stays reachable:

| Built-in tool / operation | Params → Completion.result |
|---|---|
| `rivet.request` | `{id, params}` → the called operation's Completion.result |
| `rivet.list` | `{cursor?, limit?}` → visible operation descriptors |
| `rivet.describe` | `{id}` → descriptor with Output section |
| `rivet.outputs` | `{id?, all?}` → output JSON Schema(s) with emits, receives and errors |
| `rivet.io` | `{ids?: [text], all?: boolean, by?: text, kind?: text, access?: [text], check_policy?: boolean}` → IoManifest. Remote principals need it listed explicitly in `serve.principals.<name>.operations` |
| `rivet.policy.generate` | `{ids?: [text], all?: boolean}` → `{policy: {…}, review: [site…], complete: bool}`; never writes files. Same explicit-listing rule as `rivet.io` |
| `rivet.sessions.open` / `send` / `finish_input` / `read` / `cancel` | see [gRPC, catalog and session additions](#grpc-catalog-and-session-additions) |

Every HTTP route, the WebSocket endpoint and `/mcp` are mounted by one `rivet serve`; see [One serve, every surface](#one-serve-every-surface). Exact curl requests, response shapes, statuses and the MCP initialization sequence are in the proposal's [surface contract](../proposals/draft/prop-2026-0001-rivet-runtime.md#sample-api-calls-and-cli-commands).

## OAuth, UDP, QUIC and HTTP/3 additions

These are required Stage B scope. The detailed behavioral and security contracts are [proposal increments 9–11](../proposals/draft/prop-2026-0001-rivet-runtime.md#increment-9--oauth-20-as-a-reusable-authorization-layer).

| Syntax / configuration | Contract |
|---|---|
| `auth NAME oauth2 ... end` | Immutable profile with flow, issuer, endpoints, client auth, scopes, resource origins and explicit memory/keychain store |
| `flow client_credentials/authorization_code/device_code` | Three supported flows; authorization_code requires PKCE S256; password/implicit refused |
| `auth PROFILE account ACCOUNT` | HTTP/HTTP3/MCP/gRPC credential attachment; principal/profile-hash/account scoped; no token return |
| `client_auth basic/post/none` | Explicit provider-supported client authentication; no silent method switching |
| `store memory` / `store keychain "namespace"` | Memory lost at process exit; secure durable store must pass atomicity/access-control gates |
| `rivet.auth.begin/complete/status/disconnect` | Same management operations through CLI/HTTP/MCP/library; no separate auth endpoint |
| `allow_auth=PROFILE/ACCOUNT/use` / `manage` / `status` | Independent policy actions, intersected with principal ownership and `allow_credentials=PROFILE/ACCOUNT` |
| `with udp bind "host:port" as socket` | Explicit listener grant; receive_from reports peer/data; every send_to needs outbound authority |
| `with udp multicast "group:port" as socket` | Explicit bind/interface/group grant; membership disposed in context |
| `with quic "quic://host:port" as connection` | QUIC v1, verified TLS and required ALPN; migration false and no 0-RTT by default |
| `with connection.open/accept uni/bidi as stream` | Scoped streams, bounded concurrency; type checks send/receive direction |
| `stream.finish_send` | Protocol FIN only; reads and ownership continue until scope exit |
| `datagrams true`, `send_datagram`, `receive_datagram` | Require negotiated QUIC DATAGRAM and bounded payload; unreliable delivery |
| HTTP `version 3` / `version prefer [3, 2]` | Strict H3 versus explicit safe fallback; response.version reports actual selection |

All examples use controlled fixtures. The UDP peer on 127.0.0.1:7000 returns `{state:"ready"}`. The QUIC peer engine.example.com:4433 has a trusted matching certificate and explicit fixture ALPNs: rivet-rpc/1 (length32 or newline fixture configuration), rivet-eof/1 (respond after FIN) and rivet-telemetry/1 (DATAGRAM). Configure matching framing on both peers; ALPN alone cannot determine two different encodings. HTTP3 `/items` returns `{items:[]}`. The OAuth fixture has `/authorize`, `/token`, `/device` and `/activate`, returns controllable expiry/error/rotation responses, and issues only configured scopes. These are not public live services.

`crm_service`, `crm_user`, and `crm_device` are profile declarations in S84–S86; load those definitions before their dependent samples. Scope isolation and cached-token checks still apply when a profile exists. For `--endpoint` examples the operator must load that bundle into one running server and provision its policy.json (grants and `serve.principals`); client CLI flags cannot grant the server extra rights. Auth `none` is accepted only on loopback binds; real remote access uses policy.json `serve.auth` (S131), independent of resource OAuth.

Auth profile option rules: code flow requires authorization_url, token_url, redirect_uri, issuer, client_id and `pkce s256`; device flow requires device_url/token_url/client_id; client_credentials requires confidential client auth. All flows require explicit resource_origins, scopes and store. Browser opening/callback collection is delegated to the host; no implicit exec/listen occurs. `rivet.auth.complete` for code flow requires a secret callback object with code/state/redirect_uri and issuer when supported; for device flow use transaction_id/wait:true. If the request deadline arrives before the user approves, complete returns `{"state":"pending"}` and does not consume the transaction; only an explicit `auth cancel` or expiry invalidates it. Token material, state/verifier and device codes are redacted from traces. Authorization URL/user-code is returned only as an intentional challenge to the authorized initiator.

`auth.login_required`, `auth.access_denied`, `auth.callback_invalid`, `auth.transaction_expired` and `auth.refresh_uncertain` use HTTP 409 / CLI 4; invalid input 422 / CLI 2; token/store dependency faults 502 / CLI 5; policy denies 403 / CLI 3. These errors are distinct from incoming Rivet authentication's HTTP 401. QUIC/UDP dependency faults normally use 502 / CLI 5; deadline uses 504 / CLI 6; unsupported backend/option uses 501 / CLI 5. ErrorEnvelope/Completion and stream terminal rules remain unchanged.

## gRPC, catalog and session additions

Native gRPC over HTTP/2 (unary, server-streaming, client-streaming and bidirectional) is required Stage B scope, and each public operation is published through incoming MCP as its own direct named tool. A file may contain any number of uniquely identified operation/pipeline declarations. Names and descriptions flow into one catalog used by CLI, HTTP, MCP and library. Reserved `rivet.*` IDs cannot be shadowed; private helpers are unreachable from external surfaces. `check --strict-docs` requires nonblank public operation, parameter and output descriptions, descriptions on every output/emits/receives field, and a declared `error` line for every `fail` code.

`allow_grpc=CONNECTOR/Fully.Qualified.Service/Method` grants a specific method independently of `allow_network=https://host:port`. Descriptor loading, OAuth token exchange and credential storage are separate effects. The broker inventories and checks them; with no policy.json, new application effects are denied. No gRPC-Web, inbound native gRPC server or gRPC-over-HTTP3 claim is made.

For S110–S119, the approved `./schemas/users.pb` fixture describes `example.Users`: GetUser({id:string}) → {id:string,name:string}; Watch({topic:string}) → stream {text:string}; Upload(stream {text:string}) → {count:int32}; Chat(stream {text:string}) → stream {text:string}. Descriptor cardinality determines legal send/receive operations. Methods use canonical ProtoJSON at the adapter boundary: 64-bit integers are decimal strings and bytes base64 strings, distinct from generic Rivet tagged JSON. Well-known types follow the pinned Protobuf mapping; Any types must resolve inside approved descriptors. See [gRPC concepts](https://grpc.io/docs/what-is-grpc/core-concepts/) and [ProtoJSON](https://protobuf.dev/programming-guides/json/).

`grpc CONNECTOR.Method ... end` returns a unary GrpcResult; `with grpc ... as rpc` owns a stream. `rpc.send`, `rpc.finish_send`, `rpc.result` and `rpc.completion` validate method cardinality, flow control and final status. Completion occurs only after OK trailers. No hidden retries after input submission, partial output or uncertain mutation. Metadata and status details are bounded/redacted. Sources: [gRPC status codes](https://grpc.io/docs/guides/status-codes/).

Live input uses `receives TYPE`; callers use scope.duplex in Rust or the common registry operations below. Direct MCP streaming tools return SessionReceipt, advertised with `_meta: {"rivet/delivery":"session"}`; unary tools return Completion. Generated outputSchema matches the actual envelope or error. This uses ordinary [MCP tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools), not an invented protocol stream.

| Common registry ID | Params → Completion.result |
|---|---|
| `rivet.sessions.open` | `{id,params}` → SessionReceipt |
| `rivet.sessions.send` | `{session_id,send_seq,data}` → SessionAck |
| `rivet.sessions.finish_input` | `{session_id}` → SessionAck |
| `rivet.sessions.read` | `{session_id,after_seq,max_events?,wait_ms?}` → SessionBatch |
| `rivet.sessions.cancel` | `{session_id}` → CancelReceipt `{session_id,request_id,state}` after cleanup |

Session IDs are not bearer authority. All accesses recheck the owning principal and intersect grants; catalog and schemas are pinned for a call. Default limits: eight live sessions per principal, total duration 30s (host-capped override), idle 60s capped by total deadline, sixteen frames/32 MiB aggregate queue budgets, read batches up to sixteen events and wait up to 5s. One consumer owns the read cursor. Acknowledging the last delivered sequence permits bounded eviction; replayed events require sequence deduplication. Missing retained history is a typed cursor error. Most-recent identical send retry reuses its acknowledgement without re-enqueueing; other sequence conflicts fail. Resource cleanup happens immediately at terminal state; terminal metadata may remain for 60s. An expired/abandoned client cannot retain live sockets. See proposal increments 12–14 for complete rules.

## Result and failure contract

Unary success is `Completion {request_id,trace_id,result,data_count,effects}`. Stream frames are `{request_id,trace_id,seq,type,data?/result?/error?}`; zero or more data frames, then one terminal result/error. `effects` classifies committed mutations; a read-only network call can have `effects:none` while still appearing in the I/O audit. Unknown remote mutation status is `unknown`. CLI stdout is machine output; stderr is diagnostics. No successful result is fabricated after a callback stop.

Exit codes and HTTP statuses come from the single [error registry](#error-registry): 0 ok · 2 syntax/validation/usage/config (incl. policy.json) · 3 permission · 4 not_found/conflict · 5 dependency/runtime/unsupported/output_invalid · 6 timeout · 7 inspection incomplete · 130 cancelled.

Missing or bad server credentials on a serve surface use 401; limits 429 (size 413); `output.invalid` and internal errors 500. After streaming starts, failures are terminal error events with the already-sent HTTP status. Finite requests default to 30s, cleanup 5s, body/frame 8 MiB, buffer 16 frames and 32 MiB aggregate, no automatic retry, and redirects disabled. Host-wide policy.json `limits` cap concurrency (`max_concurrent_requests`, default 64), nesting (`max_call_depth`, default 16, `limit.call_depth`) and buffered memory (`max_buffered_bytes`, default 256 MiB) across all requests and sessions. Caps are cumulative across nested scopes; fan-out cannot multiply the host's total byte/task budget.

## Sample index

- [S01 — Define one typed operation](#s01--define-one-typed-operation)
- [S02 — Call from the CLI](#s02--call-from-the-cli)
- [S03 — Call the same ID over HTTP](#s03--call-the-same-id-over-http)
- [S04 — Call from a Rust host](#s04--call-from-a-rust-host)
- [S05 — Compose by ID](#s05--compose-by-id)
- [S06 — Describe the operation everywhere](#s06--describe-the-operation-everywhere)
- [S07 — GET with encoded query parameters](#s07--get-with-encoded-query-parameters)
- [S08 — POST a typed JSON body](#s08--post-a-typed-json-body)
- [S09 — PUT and PATCH without hiding the HTTP method](#s09--put-and-patch-without-hiding-the-http-method)
- [S10 — DELETE and inspect metadata](#s10--delete-and-inspect-metadata)
- [S11 — Form and XML bodies](#s11--form-and-xml-bodies)
- [S12 — Use a secret without returning it](#s12--use-a-secret-without-returning-it)
- [S13 — Multipart upload with a file](#s13--multipart-upload-with-a-file)
- [S14 — SSE with a final result](#s14--sse-with-a-final-result)
- [S15 — NDJSON from a local model endpoint](#s15--ndjson-from-a-local-model-endpoint)
- [S16 — Download bytes into a scoped writer](#s16--download-bytes-into-a-scoped-writer)
- [S17 — Read a line stream](#s17--read-a-line-stream)
- [S18 — Make redirects explicit](#s18--make-redirects-explicit)
- [S19 — Scoped WebSocket request/response](#s19--scoped-websocket-requestresponse)
- [S20 — WebSocket text deltas and terminal marker](#s20--websocket-text-deltas-and-terminal-marker)
- [S21 — Duplex binary exchange with structured concurrency](#s21--duplex-binary-exchange-with-structured-concurrency)
- [S22 — TCP with newline framing](#s22--tcp-with-newline-framing)
- [S23 — Length-prefixed binary TCP](#s23--length-prefixed-binary-tcp)
- [S24 — Mutual TLS over TCP](#s24--mutual-tls-over-tcp)
- [S25 — Unix socket and JSON framing](#s25--unix-socket-and-json-framing)
- [S26 — HTTP over a Unix socket](#s26--http-over-a-unix-socket)
- [S27 — UDP request and timeout](#s27--udp-request-and-timeout)
- [S28 — Execute argv without a shell](#s28--execute-argv-without-a-shell)
- [S29 — Send stdin and parse command output](#s29--send-stdin-and-parse-command-output)
- [S30 — Stream a subprocess stdout](#s30--stream-a-subprocess-stdout)
- [S31 — Interactive JSON subprocess](#s31--interactive-json-subprocess)
- [S32 — Named pipe or FIFO](#s32--named-pipe-or-fifo)
- [S33 — Create a file exclusively](#s33--create-a-file-exclusively)
- [S34 — Read a JSON file](#s34--read-a-json-file)
- [S35 — Update only an existing file](#s35--update-only-an-existing-file)
- [S36 — Conditional update with a version guard](#s36--conditional-update-with-a-version-guard)
- [S37 — Explicit upsert and append](#s37--explicit-upsert-and-append)
- [S38 — Delete with an explicit missing policy](#s38--delete-with-an-explicit-missing-policy)
- [S39 — List and inspect without reading contents](#s39--list-and-inspect-without-reading-contents)
- [S40 — Copy and move with destination guards](#s40--copy-and-move-with-destination-guards)
- [S41 — Read a large file in bounded chunks](#s41--read-a-large-file-in-bounded-chunks)
- [S42 — Temporary directory with explicit publication](#s42--temporary-directory-with-explicit-publication)
- [S43 — Watch changes in a scoped directory](#s43--watch-changes-in-a-scoped-directory)
- [S44 — Diamond DAG with a typed join](#s44--diamond-dag-with-a-typed-join)
- [S45 — Enforce side-effect ordering](#s45--enforce-side-effect-ordering)
- [S46 — Keep independent DAG results after a failure](#s46--keep-independent-dag-results-after-a-failure)
- [S47 — Bounded fan-out with ordered results](#s47--bounded-fan-out-with-ordered-results)
- [S48 — Poll a remote job to completion](#s48--poll-a-remote-job-to-completion)
- [S49 — Bounded iteration for refinement](#s49--bounded-iteration-for-refinement)
- [S50 — Bounded agent loop with an allow-list](#s50--bounded-agent-loop-with-an-allow-list)
- [S51 — Compensate explicitly after a partial workflow](#s51--compensate-explicitly-after-a-partial-workflow)
- [S52 — Declare a remote MCP connector](#s52--declare-a-remote-mcp-connector)
- [S53 — Discover and freeze MCP schemas explicitly](#s53--discover-and-freeze-mcp-schemas-explicitly)
- [S54 — Call an imported MCP tool](#s54--call-an-imported-mcp-tool)
- [S55 — Read an MCP resource](#s55--read-an-mcp-resource)
- [S56 — Get a reusable MCP prompt](#s56--get-a-reusable-mcp-prompt)
- [S57 — MCP over a scoped subprocess](#s57--mcp-over-a-scoped-subprocess)
- [S58 — Expose the same registry through MCP stdio](#s58--expose-the-same-registry-through-mcp-stdio)
- [S59 — Bridge an MCP tool into a normal operation](#s59--bridge-an-mcp-tool-into-a-normal-operation)
- [S60 — Call the bridge from the ordinary CLI](#s60--call-the-bridge-from-the-ordinary-cli)
- [S61 — Decline an unsupported MCP server callback](#s61--decline-an-unsupported-mcp-server-callback)
- [S62 — Find every I/O site, including unused operations](#s62--find-every-io-site-including-unused-operations)
- [S63 — Inspect one operation and its call chain](#s63--inspect-one-operation-and-its-call-chain)
- [S64 — Filter network operations and include bootstrap](#s64--filter-network-operations-and-include-bootstrap)
- [S65 — Fail inspection when completeness is unknown](#s65--fail-inspection-when-completeness-is-unknown)
- [S66 — No policy.json denies new I/O](#s66--no-policyjson-denies-new-io)
- [S67 — Write an explicitly broad policy](#s67--write-an-explicitly-broad-policy)
- [S68 — Restrict exact paths and network origin](#s68--restrict-exact-paths-and-network-origin)
- [S69 — Select an alternate policy file](#s69--select-an-alternate-policy-file)
- [S70 — Consume emitted data with an async callback](#s70--consume-emitted-data-with-an-async-callback)
- [S71 — Stop after the first data item](#s71--stop-after-the-first-data-item)
- [S72 — Pull the stream in the host context](#s72--pull-the-stream-in-the-host-context)
- [S73 — Consume a child request stream in the DSL](#s73--consume-a-child-request-stream-in-the-dsl)
- [S74 — Read a trace by request ID](#s74--read-a-trace-by-request-id)
- [S75 — Trace the running server through the same request API](#s75--trace-the-running-server-through-the-same-request-api)
- [S76 — Export a sanitized trace](#s76--export-a-sanitized-trace)
- [S77 — Explain a denial before attempting the operation](#s77--explain-a-denial-before-attempting-the-operation)
- [S78 — Retry a transient read and recover a missing item](#s78--retry-a-transient-read-and-recover-a-missing-item)
- [S79 — Ask what this build/platform actually supports](#s79--ask-what-this-buildplatform-actually-supports)
- [S80 — Refuse sandboxed process execution without OS enforcement](#s80--refuse-sandboxed-process-execution-without-os-enforcement)

- [S81 — UDP as a registered operation](#s81--udp-as-a-registered-operation)
- [S82 — UDP listener with separately authorized replies](#s82--udp-listener-with-separately-authorized-replies)
- [S83 — UDP multicast with explicit bind and interface](#s83--udp-multicast-with-explicit-bind-and-interface)
- [S84 — OAuth client credentials profile and request](#s84--oauth-client-credentials-profile-and-request)
- [S85 — OAuth authorization code with PKCE profile](#s85--oauth-authorization-code-with-pkce-profile)
- [S86 — OAuth device authorization profile](#s86--oauth-device-authorization-profile)
- [S87 — Start user authorization through the shared interface](#s87--start-user-authorization-through-the-shared-interface)
- [S88 — Complete the callback without putting codes in argv](#s88--complete-the-callback-without-putting-codes-in-argv)
- [S89 — Complete device authorization with bounded polling](#s89--complete-device-authorization-with-bounded-polling)
- [S90 — Inspect and disconnect an OAuth account](#s90--inspect-and-disconnect-an-oauth-account)
- [S91 — Use OAuth from the Rust library](#s91--use-oauth-from-the-rust-library)
- [S92 — Use OAuth for an HTTP MCP connector](#s92--use-oauth-for-an-http-mcp-connector)
- [S93 — Audit OAuth refresh and its policy requirements](#s93--audit-oauth-refresh-and-its-policy-requirements)
- [S94 — QUIC request through a scoped reliable stream](#s94--quic-request-through-a-scoped-reliable-stream)
- [S95 — Multiplex QUIC streams without sharing receive ownership](#s95--multiplex-quic-streams-without-sharing-receive-ownership)
- [S96 — Signal QUIC end-of-input while keeping the response readable](#s96--signal-quic-end-of-input-while-keeping-the-response-readable)
- [S97 — Use negotiated QUIC datagrams](#s97--use-negotiated-quic-datagrams)
- [S98 — Reauthorize a QUIC address change before sending probes](#s98--reauthorize-a-quic-address-change-before-sending-probes)
- [S99 — HTTP/3 with the ordinary HTTP syntax](#s99--http3-with-the-ordinary-http-syntax)
- [S100 — Allow HTTP version fallback explicitly](#s100--allow-http-version-fallback-explicitly)
- [S101 — Inspect native QUIC versus HTTP/3 authority](#s101--inspect-native-quic-versus-http3-authority)
- [S102 — Inspect capabilities and explicit refusal modes](#s102--inspect-capabilities-and-explicit-refusal-modes)

- [S103 — Define several documented operations in one file](#s103--define-several-documented-operations-in-one-file)
- [S104 — Discover and invoke the catalog from CLI](#s104--discover-and-invoke-the-catalog-from-cli)
- [S105 — Call the same catalog over HTTP](#s105--call-the-same-catalog-over-http)
- [S106 — Use the whole catalog as a Rust library](#s106--use-the-whole-catalog-as-a-rust-library)
- [S107 — Publish the catalog through MCP stdio](#s107--publish-the-catalog-through-mcp-stdio)
- [S108 — Discover described operations as MCP tools](#s108--discover-described-operations-as-mcp-tools)
- [S109 — Reject duplicate IDs and keep helpers private](#s109--reject-duplicate-ids-and-keep-helpers-private)
- [S110 — Call unary gRPC with a pinned descriptor](#s110--call-unary-grpc-with-a-pinned-descriptor)
- [S111 — Forward server streaming gRPC data](#s111--forward-server-streaming-grpc-data)
- [S112 — Upload a finite list with client streaming gRPC](#s112--upload-a-finite-list-with-client-streaming-grpc)
- [S113 — Declare bidirectional gRPC with live input](#s113--declare-bidirectional-grpc-with-live-input)
- [S114 — Drive a duplex call through HTTP session operations](#s114--drive-a-duplex-call-through-http-session-operations)
- [S115 — Own a duplex request in a Rust scope](#s115--own-a-duplex-request-in-a-rust-scope)
- [S116 — Drive a streaming operation as an MCP tool](#s116--drive-a-streaming-operation-as-an-mcp-tool)
- [S117 — Preserve late gRPC failures after partial output](#s117--preserve-late-grpc-failures-after-partial-output)
- [S118 — Authorize gRPC with OAuth and inspect every effect](#s118--authorize-grpc-with-oauth-and-inspect-every-effect)
- [S119 — Validate descriptions and stream CLI input](#s119--validate-descriptions-and-stream-cli-input)
- [S120 — Serve one catalog through HTTP and MCP together](#s120--serve-one-catalog-through-http-and-mcp-together)

- [S121 — Declare a scalar output with a description](#s121--declare-a-scalar-output-with-a-description)
- [S122 — Declare a structured output with nested fields](#s122--declare-a-structured-output-with-nested-fields)
- [S123 — Declare error codes and fail strict docs on an undeclared one](#s123--declare-error-codes-and-fail-strict-docs-on-an-undeclared-one)
- [S124 — View declared outputs as a table](#s124--view-declared-outputs-as-a-table)
- [S125 — Export every declared output as JSON Schema](#s125--export-every-declared-output-as-json-schema)
- [S126 — Read declared outputs over HTTP](#s126--read-declared-outputs-over-http)
- [S127 — Read declared outputs through MCP](#s127--read-declared-outputs-through-mcp)
- [S128 — Reject a result that violates the declared output](#s128--reject-a-result-that-violates-the-declared-output)
- [S129 — Discover policy.json beside the entry file](#s129--discover-policyjson-beside-the-entry-file)
- [S130 — Block SSRF with deny rules and private ranges](#s130--block-ssrf-with-deny-rules-and-private-ranges)
- [S131 — Authenticate serve callers with bearer tokens and principals](#s131--authenticate-serve-callers-with-bearer-tokens-and-principals)
- [S132 — Refuse a non-loopback serve without authentication](#s132--refuse-a-non-loopback-serve-without-authentication)
- [S133 — Serve every surface from one listener](#s133--serve-every-surface-from-one-listener)
- [S134 — Poll a request to its terminal event](#s134--poll-a-request-to-its-terminal-event)
- [S135 — Multiplex requests over one WebSocket](#s135--multiplex-requests-over-one-websocket)
- [S136 — Call the same server through MCP at /mcp](#s136--call-the-same-server-through-mcp-at-mcp)
- [S137 — Disable surfaces in policy.json](#s137--disable-surfaces-in-policyjson)
- [S138 — Fail fast by default and report node statuses](#s138--fail-fast-by-default-and-report-node-statuses)
- [S139 — Refuse to write through a hard link](#s139--refuse-to-write-through-a-hard-link)
- [S140 — Bind a secret to its destination](#s140--bind-a-secret-to-its-destination)
- [S141 — Show every URL and path with its access verbs](#s141--show-every-url-and-path-with-its-access-verbs)
- [S142 — Group the manifest by capability](#s142--group-the-manifest-by-capability)
- [S143 — Find every deletion](#s143--find-every-deletion)
- [S144 — Check the manifest against policy.json](#s144--check-the-manifest-against-policyjson)
- [S145 — Export the manifest as Markdown and CSV](#s145--export-the-manifest-as-markdown-and-csv)
- [S146 — Allow creating notes but never overwriting them](#s146--allow-creating-notes-but-never-overwriting-them)
- [S147 — Reject an access verb that does not belong to its capability](#s147--reject-an-access-verb-that-does-not-belong-to-its-capability)
- [S148 — Generate a least-privilege policy draft](#s148--generate-a-least-privilege-policy-draft)
- [S149 — Refuse to overwrite an existing policy file](#s149--refuse-to-overwrite-an-existing-policy-file)
- [S150 — Read the manifest over HTTP](#s150--read-the-manifest-over-http)
- [S151 — Call rivet.io through MCP with an explicitly authorized principal](#s151--call-rivetio-through-mcp-with-an-explicitly-authorized-principal)
- [S152 — Build the manifest and a policy draft from a Rust host](#s152--build-the-manifest-and-a-policy-draft-from-a-rust-host)
- [S153 — Compare planned and actual I/O for one request](#s153--compare-planned-and-actual-io-for-one-request)

## Worked samples

<a id="s01--define-one-typed-operation"></a>

### S01 — Define one typed operation

Stage: **A**. Required authority: allow_network=https://api.example.com:443.

```rivet
operation users.get
    param id integer required min 1
    output json
    response = http get "https://api.example.com/users/${id}"
        decode json
        timeout "10s"
    end
    return response.body
end
```

Expected behavior: For id=42, returns {"id":42,"name":"Ada"}; invalid IDs fail before network access. `${id}` sits in a URL path segment, so it is percent-encoded as one segment and can never add `/`, `?`, `#` or `@`.

<a id="s02--call-from-the-cli"></a>

### S02 — Call from the CLI

Stage: **A**. Required authority: same effects as users.get.

```sh
rivet --file app.rivet request users.get --params '{"id":42}'
```

Expected behavior: Completion JSON on stdout with result {"id":42,"name":"Ada"}; exit 0. Unknown parameter name exits 2.

<a id="s03--call-the-same-id-over-http"></a>

### S03 — Call the same ID over HTTP

Stage: **A**. Required authority: same effects as users.get; client-to-server transport is outside the invoked operation.

```sh
curl -sS http://127.0.0.1:8080/v1/request   -H 'Content-Type: application/json'   -d '{"id":"users.get","params":{"id":42}}'
```

Expected behavior: HTTP 200 Completion JSON with the same result as S02. Input/schema failures are 422 ErrorEnvelope.

<a id="s04--call-from-a-rust-host"></a>

### S04 — Call from a Rust host

Stage: **A**. Required authority: same effects as users.get.

```rust
let rt = Runtime::builder()
    .source(include_str!("app.rivet"))
    .policy(Policy::from_file("policy.json")?)
    .build()?;
let c: Completion = rt.request("users.get", json!({"id": 42}), None).await?;
assert_eq!(c.result["id"], 42);
```

Expected behavior: Canonical proposed Rust API sketch. `Policy::from_file` reads the same policy.json schema as the CLI; the host ceiling intersects it. Host provides Tokio and credentials. No subprocess CLI and no nested async runtime. `None` means no data sink; the runtime joins cleanup even if the future is dropped.

<a id="s05--compose-by-id"></a>

### S05 — Compose by ID

Stage: **A**. Required authority: allow_network=https://api.example.com:443, allow_write=./out/**.

```rivet
pipeline users.snapshot
    param id integer required min 1
    output json
    user = (request "users.get" {id: id})
    file create "./out/user.json" json user
    return user
end
```

Expected behavior: Calls the same registry operation; exclusive creation prevents accidental overwrite. Parent policy and deadline propagate.

<a id="s06--describe-the-operation-everywhere"></a>

### S06 — Describe the operation everywhere

Stage: **A**. Required authority: none after trusted bundle load.

```sh
rivet --file app.rivet describe users.get --json
rivet --file app.rivet request rivet.describe --params '{"id":"users.get"}'
curl -sS http://127.0.0.1:8080/v1/request   -H 'Content-Type: application/json'   -d '{"id":"rivet.describe","params":{"id":"users.get"}}'
```

Expected behavior: Same descriptor as GET /v1/operations/users.get (alias unwraps the generic Completion), including the Output section (params + output + errors). Unauthorized IDs are hidden.

<a id="s07--get-with-encoded-query-parameters"></a>

### S07 — GET with encoded query parameters

Stage: **A**. Required authority: allow_network=https://api.example.com:443.

```rivet
response = http get "https://api.example.com/search"
    query q "rust & capy"
    query limit 10
    decode json
end
return response.body
```

Expected behavior: Query encoder preserves the ampersand as data; payload is returned separately from response.status.

<a id="s08--post-a-typed-json-body"></a>

### S08 — POST a typed JSON body

Stage: **A**. Required authority: allow_network=https://api.example.com:443.

```rivet
response = http post "https://api.example.com/users"
    body json {name: "Ada", tags: ["developer", "reviewer"]}
    decode json
end
return response.body
```

Expected behavior: Serializes values as JSON; quotes and newlines cannot escape the body structure. No retry by default.

<a id="s09--put-and-patch-without-hiding-the-http-method"></a>

### S09 — PUT and PATCH without hiding the HTTP method

Stage: **A**. Required authority: allow_network=https://api.example.com:443.

```rivet
replaced = http put "https://api.example.com/users/42"
    body json {name: "Ada", active: true}
    decode json
end
patched = http patch "https://api.example.com/users/42"
    body json {active: false}
    decode json
end
return patched.body
```

Expected behavior: Sequential mutations. If PATCH fails after PUT, error.effects reports committed or partial, not none.

<a id="s10--delete-and-inspect-metadata"></a>

### S10 — DELETE and inspect metadata

Stage: **A**. Required authority: allow_network=https://api.example.com:443.

```rivet
response = http delete "https://api.example.com/users/42"
    accept status [204]
    decode bytes
end
return {status: response.status, bytes: (length response.body)}
```

Expected behavior: Returns {status:204,bytes:0}; an empty body is a valid explicit result.

<a id="s11--form-and-xml-bodies"></a>

### S11 — Form and XML bodies

Stage: **A**. Required authority: allow_network=https://api.example.com:443.

```rivet
form = http post "https://api.example.com/login"
    body form {username: "ada", password: "fixture-only"}
    decode json
end
speech = http post "https://api.example.com/speech"
    body xml (xml.element "speak" {} "Hello & welcome")
    decode bytes
end
return {received: (length speech.body)}
```

Expected behavior: Form encoding and XML builder escape values. Production credentials use secret values rather than literals.

<a id="s12--use-a-secret-without-returning-it"></a>

### S12 — Use a secret without returning it

Stage: **A**. Required authority: allow_env=EXAMPLE_API_KEY, allow_network=https://api.example.com:443.

```rivet
secret API_KEY from env "EXAMPLE_API_KEY" for "https://api.example.com:443"
response = http get "https://api.example.com/account"
    header "Authorization" "Bearer ${API_KEY}"
    decode json
end
return response.body
```

Expected behavior: Environment lookup is brokered; the secret is bound to the listed origin, so using it toward any other origin is denied (S140). Taint follows explicit flows through interpolation, so the value cannot be emitted/logged; taint is best-effort and does not track implicit flows such as `if API_KEY == x`. Missing variable -> not_found.secret.

<a id="s13--multipart-upload-with-a-file"></a>

### S13 — Multipart upload with a file

Stage: **A**. Required authority: allow_read=./data/**, allow_network=https://api.example.com:443.

```rivet
response = http post "https://api.example.com/upload"
    body multipart
        field purpose "transcription"
        file audio "./data/voice.wav" "audio/wav"
    end
    decode json
end
return response.body
```

Expected behavior: Reads the named file through the broker and uploads bounded chunks. File reading is visible as an additional effect.

<a id="s14--sse-with-a-final-result"></a>

### S14 — SSE with a final result

Stage: **A**. Required authority: allow_network=https://api.example.com:443.

```rivet
operation chat.reply
    param prompt text required
    output text
    emits text
    text = ""
    with http post "https://api.example.com/chat" as events
        body json {prompt: prompt, stream: true}
        stream sse
        decode json
        for event in events
            text += event.data.delta
            emit event.data.delta
        end
    end
    return text
end
```

Expected behavior: SSE metadata is event.id/event.event/event.retry; decoded payload is event.data. Producer pauses under downstream backpressure.

<a id="s15--ndjson-from-a-local-model-endpoint"></a>

### S15 — NDJSON from a local model endpoint

Stage: **A**. Required authority: allow_network=http://127.0.0.1:11434.

```rivet
with http post "http://127.0.0.1:11434/api/generate" as chunks
    body json {model: "fixture", prompt: "Hello", stream: true}
    stream jsonl
    decode json
    for chunk in chunks
        emit chunk.response
        if chunk.done
            break
        end
    end
end
return null
```

Expected behavior: Each complete JSON line is a value. Malformed/oversized lines fail parse/limit; break disposes the response.

<a id="s16--download-bytes-into-a-scoped-writer"></a>

### S16 — Download bytes into a scoped writer

Stage: **A**. Required authority: allow_write=./out/**, allow_network=https://api.example.com:443.

```rivet
with file open "./out/audio.mp3" mode create as writer
    with http get "https://api.example.com/audio" as chunks
        stream bytes
        for chunk in chunks
            writer.write bytes chunk
        end
    end
end
return {path: "./out/audio.mp3"}
```

Expected behavior: Scope flushes/closes the writer; failure reports any partial file. This streaming create does not promise atomic all-or-nothing publication.

<a id="s17--read-a-line-stream"></a>

### S17 — Read a line stream

Stage: **A**. Required authority: allow_network=https://api.example.com:443.

```rivet
with http get "https://api.example.com/logs" as lines
    stream lines
    for line in lines
        emit line
    end
end
return null
```

Expected behavior: UTF-8 lines exclude line terminators; final unterminated line is delivered; invalid UTF-8 is parse error.

<a id="s18--make-redirects-explicit"></a>

### S18 — Make redirects explicit

Stage: **A**. Required authority: allow_network=https://api.example.com:443; any redirected origin needs a separate grant.

```rivet
response = http get "https://api.example.com/download"
    redirect follow limit 2
    decode bytes
end
return {size: (length response.body)}
```

Expected behavior: Each redirect target needs its own grant. Cross-origin credentials are stripped; denied redirect yields permission error before connection.

<a id="s19--scoped-websocket-requestresponse"></a>

### S19 — Scoped WebSocket request/response

Stage: **B**. Required authority: allow_network=wss://api.example.com:443.

```rivet
with websocket "wss://api.example.com/realtime" as socket
    timeout "20s"
    socket.send json {type: "ping"}
    event = socket.receive json
    return event
end
```

Expected behavior: Return waits for bounded scope cleanup; no socket.close call. Unexpected remote EOF is protocol/connection error.

<a id="s20--websocket-text-deltas-and-terminal-marker"></a>

### S20 — WebSocket text deltas and terminal marker

Stage: **B**. Required authority: allow_network=wss://api.example.com:443.

```rivet
text = ""
with websocket "wss://api.example.com/realtime" as socket
    timeout "30s"
    socket.send json {type: "generate", prompt: "Hello"}
    while true
        event = socket.receive json timeout "5s"
        if event.type == "done"
            break
        end
        text += event.delta
        emit event.delta
    end
end
return text
```

Expected behavior: `while true` is legal only because the enclosing scope has a finite `timeout`. A receive timeout fails and disposes the socket; no automatic replay of the generate message.

<a id="s21--duplex-binary-exchange-with-structured-concurrency"></a>

### S21 — Duplex binary exchange with structured concurrency

Stage: **B**. Required authority: allow_network=wss://api.example.com:443.

```rivet
with websocket "wss://api.example.com/voice" as socket
    concurrent limit 2 timeout "20s" fail fast
        task sender
            socket.send bytes (base64.decode "AAEC")
        end
        task receiver
            packet = socket.receive bytes
            emit packet
        end
    end
end
return null
```

Expected behavior: One send and one receive may run together; parent joins both before cleanup. Multiple simultaneous receives are invalid.

<a id="s22--tcp-with-newline-framing"></a>

### S22 — TCP with newline framing

Stage: **B**. Required authority: allow_network=tcp://127.0.0.1:9000.

```rivet
with tcp "127.0.0.1:9000" as conn
    framing newline max_frame 65536
    conn.send text "STATUS"
    response = conn.receive text timeout "2s"
    return response
end
```

Expected behavior: Framing appends/removes one newline. EOF before a complete frame is protocol.unexpected_eof.

<a id="s23--length-prefixed-binary-tcp"></a>

### S23 — Length-prefixed binary TCP

Stage: **B**. Required authority: allow_network=tcp://127.0.0.1:9000.

```rivet
with tcp "127.0.0.1:9000" as conn
    framing length32 endian big max_frame 1048576
    conn.send bytes (base64.decode "AAEC")
    return conn.receive bytes
end
```

Expected behavior: Unsigned 32-bit big-endian byte length; partial reads assembled; oversized lengths rejected before allocation.

<a id="s24--mutual-tls-over-tcp"></a>

### S24 — Mutual TLS over TCP

Stage: **C**. Required authority: allow_network=tcp://render.example.com:7443, allow_read=./certs/**.

```rivet
with tcp "render.example.com:7443" as conn
    tls true
    tls server_name "render.example.com"
    tls ca_file "./certs/ca.pem"
    tls cert_file "./certs/client.pem"
    tls key_file "./certs/client.key"
    framing newline
    conn.send json {action: "status"}
    return conn.receive json
end
```

Expected behavior: Certificate files are explicit read effects; key contents are secret. Validation stays enabled.

<a id="s25--unix-socket-and-json-framing"></a>

### S25 — Unix socket and JSON framing

Stage: **B**. Required authority: allow_unix=/tmp/render.sock.

```rivet
with unix "/tmp/render.sock" as conn
    framing newline
    conn.send json {action: "render", scene: "scene01"}
    return conn.receive json timeout "20s"
end
```

Expected behavior: Same framing semantics as TCP; ordinary allow_read does not permit connecting to the socket.

<a id="s26--http-over-a-unix-socket"></a>

### S26 — HTTP over a Unix socket

Stage: **B**. Required authority: allow_unix=/var/run/service.sock.

```rivet
response = http get "http://localhost/info"
    unix "/var/run/service.sock"
    decode json
end
return response.body
```

Expected behavior: HTTP semantics over a Unix transport; requires Unix authority, not outbound TCP authority; remote redirects need network grants.

<a id="s27--udp-request-and-timeout"></a>

### S27 — UDP request and timeout

Stage: **B**. Required authority: allow_network=udp://127.0.0.1:7000.

```rivet
with udp "127.0.0.1:7000" as socket
    max_datagram 8192
    socket.send json {command: "status"}
    return socket.receive json timeout "1s"
end
```

Expected behavior: One datagram per message; no reliability/order guarantee; oversized/truncated datagrams fail explicitly.

<a id="s28--execute-argv-without-a-shell"></a>

### S28 — Execute argv without a shell

Stage: **A**. Required authority: allow_exec=/usr/bin/printf.

```rivet
result = command "/usr/bin/printf"
    args ["%s", "hello; echo this stays data"]
    timeout "2s"
    decode stdout text
end
return result.stdout
```

Expected behavior: The semicolon is data, not shell syntax. Tool-specific flags still need argument validation; argv alone does not stop argument injection. Any policy-confined process needs a supported worker backend.

<a id="s29--send-stdin-and-parse-command-output"></a>

### S29 — Send stdin and parse command output

Stage: **A**. Required authority: allow_exec=./bin/json-worker; enforced worker resources separately granted.

```rivet
result = command "./bin/json-worker"
    args ["--once"]
    stdin json {action: "summarize", text: "Hello"}
    decode stdout json
    env {LANG: "C"}
end
return result.stdout
```

Expected behavior: Starts with empty env plus runtime-required controlled values; stdout/stderr drained concurrently; nonzero exit fails.

<a id="s30--stream-a-subprocess-stdout"></a>

### S30 — Stream a subprocess stdout

Stage: **A**. Required authority: allow_exec=./bin/log-worker; worker read grants if it reads logs.

```rivet
with command "./bin/log-worker" as process
    args ["--follow"]
    stream stdout lines
    for line in process.stdout
        emit line
    end
end
return null
```

Expected behavior: Cancellation terminates and reaps the owned process tree; bounded stderr is drained to avoid pipe deadlock.

<a id="s31--interactive-json-subprocess"></a>

### S31 — Interactive JSON subprocess

Stage: **C**. Required authority: allow_exec=./bin/interactive-worker.

```rivet
with command "./bin/interactive-worker" as process
    interactive true
    stream stdout jsonl
    decode stdout json
    process.stdin.send json {action: "echo", text: "Hello"}
    reply = process.stdout.receive
    return reply
end
```

Expected behavior: Scoped stdio request/response. Scope closes stdin and reaps child; protocol stdout excludes diagnostics.

<a id="s32--named-pipe-or-fifo"></a>

### S32 — Named pipe or FIFO

Stage: **C**. Required authority: allow_pipe=/tmp/rivet-input.fifo.

```rivet
with pipe "/tmp/rivet-input.fifo" mode read as channel
    framing newline
    return channel.receive text timeout "5s"
end
```

Expected behavior: Unix FIFO reader opens with deadline and reads one frame; request/response needs two FIFOs. Windows duplex uses an explicit `\\.\pipe\rivet` endpoint with mode duplex; unsupported platforms refuse.

<a id="s33--create-a-file-exclusively"></a>

### S33 — Create a file exclusively

Stage: **A**. Required authority: allow_write=./out/**.

```rivet
file create "./out/config.json" json {enabled: true}
return {created: true}
```

Expected behavior: Existing destination yields conflict.already_exists and remains unchanged. ./out must already exist.

<a id="s34--read-a-json-file"></a>

### S34 — Read a JSON file

Stage: **A**. Required authority: allow_read=./data/**.

```rivet
config = file read "./data/config.json" as json
return config.enabled
```

Expected behavior: Missing path -> not_found; malformed JSON -> parse; read is bounded by context limits.

<a id="s35--update-only-an-existing-file"></a>

### S35 — Update only an existing file

Stage: **A**. Required authority: allow_write=./out/**.

```rivet
file update "./out/config.json" json {enabled: false}
return {updated: true}
```

Expected behavior: Atomic replacement in the same directory; missing target fails. No implicit create on update.

<a id="s36--conditional-update-with-a-version-guard"></a>

### S36 — Conditional update with a version guard

Stage: **A**. Required authority: allow_read=./out/**, allow_write=./out/**.

```rivet
info = file stat "./out/config.json"
file update "./out/config.json" json {enabled: true}
    if_version info.version
end
return {updated: true}
```

Expected behavior: Conflict if observed version changed; unsupported.conditional_update if backend cannot guard against external writers. Never silently degrades to read-then-write.

<a id="s37--explicit-upsert-and-append"></a>

### S37 — Explicit upsert and append

Stage: **A**. Required authority: allow_write=./out/**.

```rivet
file write "./out/events.txt" text "started\n"
file append "./out/events.txt" text "finished\n"
return {written: true}
```

Expected behavior: write explicitly creates or replaces; append requires an existing file and is not a multi-writer transaction.

<a id="s38--delete-with-an-explicit-missing-policy"></a>

### S38 — Delete with an explicit missing policy

Stage: **A**. Required authority: allow_delete=./out/**.

```rivet
file delete "./out/obsolete.json"
    missing ok
end
return {absent: true}
```

Expected behavior: Deletes one file or succeeds if already absent; directories and recursive deletion require separate explicit operations.

<a id="s39--list-and-inspect-without-reading-contents"></a>

### S39 — List and inspect without reading contents

Stage: **A**. Required authority: allow_read=./data/**.

```rivet
entries = file list "./data"
info = file stat "./data/config.json"
return {entries: entries, size: info.size, version: info.version}
```

Expected behavior: Metadata/listing are observable reads and require read grants. Entries are sorted by name for stable results.

<a id="s40--copy-and-move-with-destination-guards"></a>

### S40 — Copy and move with destination guards

Stage: **A**. Required authority: allow_read=./data/**, allow_read=./out/**, allow_write=./out/**, allow_delete=./out/**.

```rivet
file copy "./data/input.txt" to "./out/copy.txt"
    overwrite false
end
file move "./out/copy.txt" to "./out/final.txt"
    overwrite false
end
return {path: "./out/final.txt"}
```

Expected behavior: No overwrite by default. Cross-filesystem atomic move fails unsupported; copy+delete must be requested separately.

<a id="s41--read-a-large-file-in-bounded-chunks"></a>

### S41 — Read a large file in bounded chunks

Stage: **A**. Required authority: allow_read=./data/**.

```rivet
with file open "./data/archive.bin" mode read as reader
    chunk_size 65536
    for chunk in reader
        emit chunk
    end
end
return null
```

Expected behavior: At most configured bytes buffered; each chunk uses the bytes envelope across JSON surfaces.

<a id="s42--temporary-directory-with-explicit-publication"></a>

### S42 — Temporary directory with explicit publication

Stage: **A**. Required authority: allow_write=./scratch/**, allow_read=./scratch/**, allow_write=./out/**.

```rivet
with file tempdir "./scratch" as temp
    file create "${temp.path}/result.json" json {ok: true}
    file publish "${temp.path}/result.json" to "./out/result.json"
end
return {path: "./out/result.json"}
```

Expected behavior: Explicit publish copies into durable caller-selected destination. Temporary contents are removed on exit; cleanup authority comes with acquisition.

<a id="s43--watch-changes-in-a-scoped-directory"></a>

### S43 — Watch changes in a scoped directory

Stage: **C**. Required authority: allow_read=./data/**.

```rivet
with file watch "./data" as changes
    debounce "100ms"
    for change in changes
        emit {kind: change.kind, path: change.path}
    end
end
return null
```

Expected behavior: Deadline/cancellation disposes watcher. Overflow emits a rescan-required event; watching does not promise every OS change is delivered.

<a id="s44--diamond-dag-with-a-typed-join"></a>

### S44 — Diamond DAG with a typed join

Stage: **A**. Required authority: transitive effects of users.get and orders.list.

```rivet
dag limit 4 timeout "20s" fail fast
    node user = (request "users.get" {id: 42})
    node orders = (request "orders.list" {user_id: 42})
    node summary after [user, orders] = (request "summary.make" {
        user: user.result, orders: orders.result
    })
end
return summary.result
```

Expected behavior: user/orders run independently; summary starts only after both validated results. Summary fixture is pure. Under `fail fast` any node failure fails the `dag` statement itself, so the `return` is reached only when summary succeeded.

<a id="s45--enforce-side-effect-ordering"></a>

### S45 — Enforce side-effect ordering

Stage: **A**. Required authority: allow_write=./out/**.

```rivet
dag limit 2 fail fast
    node create = (request "files.create_record" {path: "./out/item.json"})
    node update after [create] = (request "files.update_record" {path: "./out/item.json"})
end
return update.result
```

Expected behavior: Explicit dependency prevents write overlap. Same dynamic target without ordering is a compile-time conflict unless reviewed as disjoint.

<a id="s46--keep-independent-dag-results-after-a-failure"></a>

### S46 — Keep independent DAG results after a failure

Stage: **A**. Required authority: none.

```rivet
dag limit 2 fail independent
    node good = (request "fixture.echo" {value: "ok"})
    node bad = (request "fixture.fail" {})
    node blocked after [bad] = (request "fixture.echo" {value: bad.result})
end
return {good: good.status, bad: bad.status, blocked: blocked.status}
```

Expected behavior: Returns succeeded/failed/blocked. Each node value is `{status, result, error}`; `bad.result` is null because bad did not succeed, and `check` warns when a `return` reads `.result` of such a node without a status guard. Completed side effects remain recorded.

<a id="s47--bounded-fan-out-with-ordered-results"></a>

### S47 — Bounded fan-out with ordered results

Stage: **A**. Required authority: allow_network=https://api.example.com:443.

```rivet
results = map item in [1, 2, 3, 4] limit 2
    yield (request "users.get" {id: item})
end
return results
```

Expected behavior: At most two in-flight calls; result order matches input order. `yield` gives each item's value; a `return` here would exit the whole operation. Default failure policy is fail fast with joined cleanup.

<a id="s48--poll-a-remote-job-to-completion"></a>

### S48 — Poll a remote job to completion

Stage: **A**. Required authority: allow_network=https://api.example.com:443.

```rivet
result = poll every "1s" timeout "20s"
    response = http get "https://api.example.com/jobs/job_42"
        decode json
    end
    if response.body.state == "failed"
        fail "job_failed" {job_id: "job_42"}
    end
    until response.body.state == "done"
    yield response.body
end
return result
```

Expected behavior: `yield` inside poll supplies the terminal value only when until is true (`return` would exit the operation); each unsuccessful round waits. Timeout cancels and returns timeout.

<a id="s49--bounded-iteration-for-refinement"></a>

### S49 — Bounded iteration for refinement

Stage: **A**. Required authority: transitive effects of text.refine.

```rivet
answer = "draft"
iterate max 3
    next = (request "text.refine" {text: answer})
    if next == answer
        break
    end
    answer = next
end
return answer
```

Expected behavior: Runs at most three rounds. Reaching max stops normally here; add an explicit domain failure if convergence is mandatory.

<a id="s50--bounded-agent-loop-with-an-allow-list"></a>

### S50 — Bounded agent loop with an allow-list

Stage: **A**. Required authority: union of agent.next, orders.list and users.get effects.

```rivet
messages = [{role: "user", content: "Find order 42"}]
iterate max 5
    turn = (request "agent.next" {messages: messages})
    if turn.type == "final"
        return turn.text
    end
    value = (request turn.operation turn.params)
        allow ["orders.list", "users.get"]
    end
    messages += [{role: "tool", content: value}]
end
fail "iteration_limit" {max: 5}
```

Expected behavior: Dynamic operation must be in the explicit list; policy still checks its effects. Remote tool names cannot select arbitrary local operations.

<a id="s51--compensate-explicitly-after-a-partial-workflow"></a>

### S51 — Compensate explicitly after a partial workflow

Stage: **A**. Required authority: transitive order/payment effects; mutations are not automatically retried.

```rivet
created = (request "orders.create" {item: "book"})
try
    paid = (request "payments.charge" {order_id: created.id})
    return paid
catch error kind http
    (request "orders.cancel" {id: created.id})
    fail "payment_failed" {order_id: created.id}
end
```

Expected behavior: Compensation is separately authorized and can itself fail; original failure remains the cause. No transaction or automatic refund promise.

<a id="s52--declare-a-remote-mcp-connector"></a>

### S52 — Declare a remote MCP connector

Stage: **B**. Required authority: schema read at explicit bundle assembly; network/MCP grants only at invocation.

```rivet
connector crm mcp
    transport http "https://mcp.example.com/mcp"
    schema "./schemas/crm.json"
    expose tools ["search", "create_contact"]
    expose resources ["crm://contacts/schema"]
    expose prompts ["contact_summary"]
end
```

Expected behavior: Declaration reads a supplied schema snapshot during bundle load; no network discovery during compilation. Import IDs are crm.tools.*, crm.resources.read and crm.prompts.get.

<a id="s53--discover-and-freeze-mcp-schemas-explicitly"></a>

### S53 — Discover and freeze MCP schemas explicitly

Stage: **B**. Required authority: network + logical discovery + snapshot write.

`policy.json` (beside `app.rivet`):

```json
{
  "version": 1,
  "grants": [
    {"capability": "allow_network", "targets": ["https://mcp.example.com:443"]},
    {"capability": "allow_mcp",     "targets": ["crm/discover"]},
    {"capability": "allow_write",   "targets": ["./schemas/**"]}
  ]
}
```

```sh
rivet --file app.rivet connectors sync crm --output ./schemas/crm.next.json
```

Expected behavior: Negotiates and paginates tools/resources/prompts; writes candidate snapshot. A snapshot is reviewed only when its sha256 is listed in policy.json `"approved": {"snapshots": ["sha256:..."]}`; otherwise loading it fails. The user promotes the snapshot, approves its hash and recompiles; the running registry does not change.

<a id="s54--call-an-imported-mcp-tool"></a>

### S54 — Call an imported MCP tool

Stage: **B**. Required authority: allow_network=https://mcp.example.com:443, allow_mcp=crm/tools/search.

```rivet
result = (request "crm.tools.search" {query: "Ada"})
return result
```

Expected behavior: Preserves content and structuredContent in McpResult. A tool isError result becomes mcp.tool_failed with safe details.

<a id="s55--read-an-mcp-resource"></a>

### S55 — Read an MCP resource

Stage: **B**. Required authority: allow_network=https://mcp.example.com:443, allow_mcp=crm/resources/read.

```rivet
result = (request "crm.resources.read" {uri: "crm://contacts/schema"})
return result.contents
```

Expected behavior: Validates negotiated resources capability and exposed URI. Binary blobs stay tagged; remote filesystem effects remain opaque.

<a id="s56--get-a-reusable-mcp-prompt"></a>

### S56 — Get a reusable MCP prompt

Stage: **B**. Required authority: allow_network=https://mcp.example.com:443, allow_mcp=crm/prompts/get.

```rivet
result = (request "crm.prompts.get" {
    name: "contact_summary", arguments: {name: "Ada"}
})
return result.messages
```

Expected behavior: Prompt content is returned as data; it is never executed as policy or local code.

<a id="s57--mcp-over-a-scoped-subprocess"></a>

### S57 — MCP over a scoped subprocess

Stage: **B**. Required authority: allow_exec=/opt/rivet-fixtures/docs-mcp, allow_read=./data/**, allow_mcp=local_docs/tools/search.

```rivet
connector local_docs mcp
    transport command "/opt/rivet-fixtures/docs-mcp"
        args ["--root", "./data"]
        env {LANG: "C"}
    end
    schema "./schemas/local_docs.json"
    expose tools ["search"]
end
```

Expected behavior: stdio uses protocol-only stdout. Rivet refuses startup unless the worker backend enforces the child read/network restrictions from policy.json.

<a id="s58--expose-the-same-registry-through-mcp-stdio"></a>

### S58 — Expose the same registry through MCP stdio

Stage: **B**. Required authority: inherited protocol descriptors are bootstrap; invocation effects still checked.

`policy.json` (beside `app.rivet`):

```json
{
  "version": 1,
  "grants": [{"capability": "allow_network", "targets": ["https://api.example.com:443"]}]
}
```

```sh
rivet --file app.rivet serve --stdio
```

Expected behavior: MCP initialize/tools/list/tools/call use inherited stdin/stdout; diagnostics go to stderr. Each public operation is a direct named tool (canonical); the built-in generic tools are also listed. Operation authorization remains per exposed ID.

<a id="s59--bridge-an-mcp-tool-into-a-normal-operation"></a>

### S59 — Bridge an MCP tool into a normal operation

Stage: **B**. Required authority: transitive CRM network and MCP grants.

```rivet
operation contacts.find
    param query text required
    output json
    found = (request "crm.tools.search" {query: query})
    return found.structuredContent
end
```

Expected behavior: contacts.find is callable from CLI, HTTP, library and Rivet MCP. Missing structuredContent fails output validation; no silent empty frame.

<a id="s60--call-the-bridge-from-the-ordinary-cli"></a>

### S60 — Call the bridge from the ordinary CLI

Stage: **B**. Required authority: same as contacts.find.

```sh
rivet --file app.rivet request contacts.find --params '{"query":"Ada"}'
```

Expected behavior: Completion.result is the imported structured contact data; no MCP-specific command is required.

<a id="s61--decline-an-unsupported-mcp-server-callback"></a>

### S61 — Decline an unsupported MCP server callback

Stage: **B**. Required authority: only fixture MCP transport grant; no extra capability minted.

```sh
rivet --file app.rivet request fixture.mcp_sampling --params '{}'
```

Expected behavior: Fixture requests sampling; host has not enabled it. Result is protocol.unsupported_capability, with no implicit model call. CLI exit 5.

<a id="s62--find-every-io-site-including-unused-operations"></a>

### S62 — Find every I/O site, including unused operations

Stage: **A**. Required authority: none after explicit bundle load. Uses the [I/O manifest fixture bundle](#io-manifest-fixture-bundle); no policy.json yet.

```sh
rivet --file app.rivet io --all --transitive --format json
```

IoManifest (abridged to four of the thirteen sites and two of the target rows):

```json
{
  "bundle": {"file": "app.rivet", "sha256": "…"},
  "policy": null,
  "complete": false,
  "sites": [
    {
      "effect_id": "users.get#2",
      "operation_id": "users.get",
      "kind": "network",
      "access": ["connect"],
      "method": "GET",
      "protocol": "http1|http2",
      "capability": "allow_network",
      "target": {"template": "https://api.example.com/users/{id}", "scheme": "https", "host": "api.example.com",
                 "port": 443, "path": "/users/{id}", "glob": null, "params": ["id"]},
      "knowledge": "param_dependent",
      "condition": null,
      "call_chain": ["users.snapshot", "users.get"],
      "secrets": ["api_key"],
      "source": {"file": "app.rivet", "line": 6, "column": 5},
      "decision": null
    },
    {
      "effect_id": "notes.delete#1", "operation_id": "notes.delete", "kind": "file",
      "access": ["delete"], "method": null, "protocol": null, "capability": "allow_delete",
      "target": {"template": "./out/notes/{name}.json", "path": "./out/notes/{name}.json",
                 "glob": "./out/notes/*.json", "params": ["name"]},
      "knowledge": "param_dependent", "condition": null, "call_chain": ["notes.delete"], "secrets": [],
      "source": {"file": "app.rivet", "line": 64, "column": 5}, "decision": null
    },
    {
      "effect_id": "sync.push#2", "operation_id": "sync.push", "kind": "network",
      "access": ["connect"], "method": "POST", "protocol": "http1|http2", "capability": "allow_network",
      "target": {"template": null, "expression": "config.url", "glob": null, "params": []},
      "knowledge": "dynamic", "condition": null, "call_chain": ["sync.push"], "secrets": [],
      "source": {"file": "app.rivet", "line": 72, "column": 5}, "decision": null
    },
    {
      "effect_id": "archive.purge#1", "operation_id": "archive.purge", "kind": "file",
      "access": ["delete"], "method": null, "protocol": null, "capability": "allow_delete",
      "target": {"template": "./out/archive/old.json", "path": "./out/archive/old.json", "glob": null, "params": []},
      "knowledge": "exact", "condition": null, "call_chain": ["archive.purge"], "secrets": [],
      "source": {"file": "app.rivet", "line": 83, "column": 5}, "decision": null
    }
  ],
  "targets": [
    {"target": "https://api.example.com:443", "capability": "allow_network",
     "access": ["connect"], "methods": ["GET", "POST"], "operations": ["users.get", "users.create"]},
    {"target": "./out/notes/*.json", "capability": "allow_delete",
     "access": ["delete"], "methods": [], "operations": ["notes.delete"]}
  ]
}
```

```text
 13 sites = 12 reachable from public entries + 1 in private archive.purge (only because of --all)
   env      2   read                      allow_env
   network  3   connect GET/POST          allow_network   (1 dynamic → complete:false)
   file     8   read, stat, create,       allow_read / allow_write / allow_delete
                update, delete
```

Expected behavior: exit 0. Every site carries `effect_id`, `operation_id`, kind, access verbs, capability, the normalized target, knowledge class, source span and call chain; HTTP sites add `method` and `protocol` (`http1|http2` = the set ALPN may negotiate without a `version` option). A site reachable from several entries is listed once; `call_chain` shows its longest chain from a public entry. `${id}` becomes `{id}` with `params:["id"]`, and param_dependent paths carry their derived `glob`. The dynamic `config.url` target is reported by expression, never evaluated, and makes `complete:false` (S65). `--all` adds the private, unreachable `archive.purge`; `policy` is null because no policy.json was found.

<a id="s63--inspect-one-operation-and-its-call-chain"></a>

### S63 — Inspect one operation and its call chain

Stage: **A**. Required authority: none after explicit bundle load. Fixture bundle as S62.

```sh
rivet --file app.rivet io users.snapshot --transitive
rivet --file app.rivet graph users.snapshot --json
```

`io` table (by operation, the default):

```text
OPERATION       KIND     ACCESS       TARGET                              KNOWLEDGE        SOURCE
users.snapshot  (calls users.get — see above)                                              app.rivet:30
users.get       env      read         env API_KEY                         exact            app.rivet:5
users.get       network  connect GET  https://api.example.com/users/{id}  param_dependent  app.rivet:6
users.snapshot  file     create       ./out/user.json                     exact            app.rivet:31
```

`graph` (rendered as a tree; the JSON carries the same nodes and edges):

```text
users.snapshot                                                            app.rivet:26
├── call users.get                                                        app.rivet:30
│   ├── env      read         env API_KEY                                 app.rivet:5
│   └── network  connect GET  https://api.example.com/users/{id}          app.rivet:6
└── file         create       ./out/user.json                             app.rivet:31
```

Expected behavior: exit 0. Transitive analysis follows the literal `(request "users.get" …)` call, so the snapshot's sites are its own file create plus users.get's env read and GET. The call itself is a row but not a site. Static analysis includes conditional branches (their `condition` is recorded), not just last-run effects; S153 joins these rows to what a real request did.

<a id="s64--filter-network-operations-and-include-bootstrap"></a>

### S64 — Filter network operations and include bootstrap

Stage: **A**. Required authority: none beyond declared bootstrap. Fixture bundle as S62.

```sh
rivet --file app.rivet io --all --kind network --include-bootstrap
```

```text
OPERATION     KIND     ACCESS        TARGET                              KNOWLEDGE        SOURCE
users.get     network  connect GET   https://api.example.com/users/{id}  param_dependent  app.rivet:6
users.create  network  connect POST  https://api.example.com/users       exact            app.rivet:18
sync.push     network  connect POST  <dynamic: config.url>               dynamic          app.rivet:72

BOOTSTRAP (runtime-internal; listed, not governed by policy.json)
KIND  ACCESS       TARGET
file  read         ./app.rivet (+ imports)
file  read         ./policy.json (when present)
file  read         system CA bundle
file  read         /etc/resolv.conf / system resolver
file  read         tzdata
file  read         descriptor/schema files named by connectors (none here)
pipe  read, write  stdin, stdout, stderr
```

Expected behavior: exit 0. `--kind network` keeps the three network sites (one dynamic). `--include-bootstrap` appends the fixed runtime-internal list, printed in full whatever `--kind` says; with `--format json` it is the separate `bootstrap` array. Policy guarantees apply to script-initiated effects through brokered adapters; bootstrap I/O is listed, not hidden.

<a id="s65--fail-inspection-when-completeness-is-unknown"></a>

### S65 — Fail inspection when completeness is unknown

Stage: **A**. Required authority: none after explicit bundle load. Fixture bundle as S62.

```sh
rivet --file app.rivet io --all --transitive --strict
echo $?
```

```text
OPERATION       KIND     ACCESS        TARGET                              KNOWLEDGE        SOURCE
users.get       env      read          env API_KEY                         exact            app.rivet:5
users.get       network  connect GET   https://api.example.com/users/{id}  param_dependent  app.rivet:6
users.create    env      read          env API_KEY                         exact            app.rivet:17
users.create    network  connect POST  https://api.example.com/users       exact            app.rivet:18
users.snapshot  (calls users.get — see above)                                               app.rivet:30
users.snapshot  file     create        ./out/user.json                     exact            app.rivet:31
report.load     file     read          ./data/input.json                   exact            app.rivet:38
notes.create    file     create        ./out/notes/{name}.json             param_dependent  app.rivet:47
notes.update    file     stat          ./out/notes/{name}.json             param_dependent  app.rivet:56
notes.update    file     update        ./out/notes/{name}.json             param_dependent  app.rivet:56
notes.delete    file     delete        ./out/notes/{name}.json             param_dependent  app.rivet:64
sync.push       file     read          ./data/endpoint.json                exact            app.rivet:71
sync.push       network  connect POST  <dynamic: config.url>               dynamic          app.rivet:72
archive.purge   file     delete        ./out/archive/old.json              exact            app.rivet:83

stderr:
io: complete=false — 1 of 13 sites is dynamic/opaque
  sync.push#2  network connect POST  target from expression config.url  (app.rivet:72)
7
```

Expected behavior: the full manifest is still printed; `--strict` turns `complete:false` into exit 7 (inspection incomplete). Without `--strict` the same output exits 0. Making the URL a literal, or a param with `enum [...]` (knowledge `bounded`), removes the unknown. `policy generate` treats the same site as a review item (S148).

<a id="s66--no-policyjson-denies-new-io"></a>

### S66 — No policy.json denies new I/O

Stage: **A**. Required authority: no grants (no policy.json beside `app.rivet`).

```text
project/
├── app.rivet
└── (no policy.json)   ──► deny-by-default for new application I/O
```

```sh
rivet --file app.rivet request users.get --params '{"id":42}'
rivet --file app.rivet request fixture.echo --params '{"value":"ok"}'
```

Expected behavior: users.get fails permission.denied; exit 3; no DNS/HTTP request. The pure fixture.echo operation still succeeds with exit 0. There is no implicit trusted allow-all mode.

<a id="s67--write-an-explicitly-broad-policy"></a>

### S67 — Write an explicitly broad policy

Stage: **A**. Required authority: three intentionally broad grants.

`policy.json` (beside `app.rivet`):

```json
{
  "version": 1,
  "grants": [
    {"capability": "allow_read",    "targets": ["*"]},
    {"capability": "allow_write",   "targets": ["*"]},
    {"capability": "allow_network", "targets": ["*"]}
  ]
}
```

```sh
rivet --file app.rivet request users.snapshot --params '{"id":42}'
rivet --file app.rivet policy explain users.snapshot --params '{"id":42}' --json
```

Expected behavior: Allows host-visible reads/writes/outbound network, subject to the host ceiling; `network.deny_private_ranges` still defaults to true, so loopback/private/metadata addresses stay denied. `policy explain` flags all three grants as broad. Does not allow delete, exec, env, Unix sockets or MCP logical calls. Allow-all exists only when written in the file.

<a id="s68--restrict-exact-paths-and-network-origin"></a>

### S68 — Restrict exact paths and network origin

Stage: **A**. Required authority: specified selectors only.

`policy.json` (beside `app.rivet`):

```json
{
  "version": 1,
  "grants": [
    {"capability": "allow_read",    "targets": ["./data/**"]},
    {"capability": "allow_write",   "targets": ["./out/**"]},
    {"capability": "allow_network", "targets": ["https://api.example.com:443"]}
  ]
}
```

```sh
rivet --file app.rivet request users.snapshot --params '{"id":42}'
```

Expected behavior: Targets resolve relative to the policy file's directory. Sibling directories, symlink escapes (including Windows junctions/reparse points) and redirected hosts are denied even if their strings share a prefix. Selector matching case-folds on case-insensitive volumes and normalizes Unicode to NFC.

<a id="s69--select-an-alternate-policy-file"></a>

### S69 — Select an alternate policy file

Stage: **A**. Required authority: only the grants in the selected file.

```text
project/
├── app.rivet
├── policy.json        <- default, auto-discovered (not read when --policy is given)
├── data/
└── policies/
    └── ci.json        <- selected with --policy; its targets resolve from policies/
```

`policies/ci.json`:

```json
{
  "version": 1,
  "grants": [
    {"capability": "allow_read",  "targets": ["../data/**"]},
    {"capability": "allow_write", "targets": ["../out/**"]}
  ],
  "deny": [
    {"capability": "allow_read", "targets": ["../data/private/**"]}
  ]
}
```

```sh
rivet --file app.rivet --policy ./policies/ci.json request fixture.echo --params '{"value":"ok"}'
rivet --file app.rivet --policy "allow_read=./data/**" request fixture.echo --params '{"value":"ok"}'
```

Expected behavior: The first command uses ci.json only; `deny` overrides `grants`. `--policy` takes a path, never grants: the second command looks for a file literally named `allow_read=./data/**`, fails to load it (`policy.invalid`) and exits 2. Unknown keys or capabilities and malformed selectors are also load errors with exit 2.

<a id="s70--consume-emitted-data-with-an-async-callback"></a>

### S70 — Consume emitted data with an async callback

Stage: **A**. Required authority: chat.reply effects; host sink side effects must be separately owned/authorized.

```rust
let sink = DataSink::from_fn(async move |event: DataEvent| {
    tx.send(event.data).await?;
    Ok(Continue)
});
let c: Completion = rt.request("chat.reply", json!({"prompt": "Hello"}), Some(sink)).await?;
let final_text = c.result;
```

Expected behavior: Canonical Rust sketch: the sink (`AsyncDataSink + Send + 'static`) is awaited per item; final result arrives only after stream validation and cleanup. Sink errors become consumer_failed.

<a id="s71--stop-after-the-first-data-item"></a>

### S71 — Stop after the first data item

Stage: **A**. Required authority: chat.reply effects until cancellation.

```rust
let stop_first = DataSink::from_fn(async move |_event: DataEvent| Ok(Stop));
let outcome = rt.request("chat.reply", json!({"prompt": "Hello"}), Some(stop_first)).await;
assert_eq!(outcome.unwrap_err().kind, ErrorKind::Cancelled);
```

Expected behavior: One item is accepted, then child work cancels and cleanup is awaited. Stop is not falsely reported as a successful complete answer.

<a id="s72--pull-the-stream-in-the-host-context"></a>

### S72 — Pull the stream in the host context

Stage: **A**. Required authority: chat.reply effects; host consume routine outside DSL trust boundary.

```rust
let reply = rt.scope(|scope| async move {
    let mut events = scope.stream("chat.reply", json!({"prompt": "Hello"})).await?;
    while let Some(event) = events.next().await? {   // Result<Option<Envelope>>
        match event {
            Envelope::Data(data) => consume(data).await?,
            Envelope::Result(result) => return Ok(result),
        }
    }
    Err(RivetError::missing_terminal())
}).await?;
```

Expected behavior: Canonical Rust sketch: `next()` returns `Result<Option<Envelope>>`, so a wire terminal error surfaces as `Err`; the scope closure is `async move`; scope exit joins cleanup, including on early return.

<a id="s73--consume-a-child-request-stream-in-the-dsl"></a>

### S73 — Consume a child request stream in the DSL

Stage: **A**. Required authority: transitive chat.reply effects.

```rivet
text = ""
with (request.stream "chat.reply" {prompt: "Hello"}) as events
    for event in events
        if event.type == "data"
            text += event.data
        end
        if event.type == "result"
            return event.result
        end
    end
end
fail "missing_terminal" {}
```

Expected behavior: Typed terminal error raises RivetError in DSL iteration; breaking early cancels child scope. Data and result are distinct.

<a id="s74--read-a-trace-by-request-id"></a>

### S74 — Read a trace by request ID

Stage: **A**. Required authority: trace-store read if persistent; in-memory none.

```sh
rivet --file app.rivet trace show req_01 --json
```

Expected behavior: Reads the runtime trace store selected by the host. Separate CLI processes have no shared in-memory history; use the running server or an explicitly persisted trace source. Unknown request -> exit 4.

<a id="s75--trace-the-running-server-through-the-same-request-api"></a>

### S75 — Trace the running server through the same request API

Stage: **A**. Required authority: host trace-store read; no new operation effects.

```sh
curl -sS http://127.0.0.1:8080/v1/request   -H 'Content-Type: application/json'   -d '{"id":"rivet.trace.show","params":{"request_id":"req_01"}}'
```

Expected behavior: Authorized principal receives Completion with source-linked attempt events. Access to another principal's trace is denied/hidden.

<a id="s76--export-a-sanitized-trace"></a>

### S76 — Export a sanitized trace

Stage: **A**. Required authority: allow_write=./audit/**; persistent source may additionally need read.

`policy.json` (beside `app.rivet`):

```json
{
  "version": 1,
  "grants": [{"capability": "allow_write", "targets": ["./audit/**"]}]
}
```

```sh
rivet --file app.rivet trace export req_01 --output ./audit/req_01.json
```

Expected behavior: Writes only authorized trace data; absent local request yields not_found. Required export failure is visible. Payload/credential recording is off by default.

<a id="s77--explain-a-denial-before-attempting-the-operation"></a>

### S77 — Explain a denial before attempting the operation

Stage: **A**. Required authority: none after bootstrap.

`policy.json` (beside `app.rivet`):

```json
{
  "version": 1,
  "grants": [{"capability": "allow_read", "targets": ["./data/**"]}]
}
```

```sh
rivet --file app.rivet policy explain users.snapshot --params '{"id":42}' --json
```

Expected behavior: Returns missing network/write grants and originating effect sites. Explanation is not a transferable Permit; runtime checks again.

<a id="s78--retry-a-transient-read-and-recover-a-missing-item"></a>

### S78 — Retry a transient read and recover a missing item

Stage: **A**. Required authority: allow_network=https://api.example.com:443.

```rivet
try
    response = http get "https://api.example.com/users/42"
        retry 3 on status [429, 503] backoff exponential base "100ms" max "2s" jitter true
        decode json
    end
    return response.body
catch error code "http.status"
    if error.details.status == 404
        return {found: false}
    end
    fail "users.lookup_failed" {status: error.details.status}
end
```

Expected behavior: An upstream 404 is `http.status` with `details.status: 404` (kind `http`, a dependency failure: 502 / exit 5) unless mapped as here. At most four attempts within parent deadline; honor bounded Retry-After. Validation, permission and cancellation are not retried. POST needs explicit replay safety.

<a id="s79--ask-what-this-buildplatform-actually-supports"></a>

### S79 — Ask what this build/platform actually supports

Stage: **A**. Required authority: none for capabilities; shell refused before any process.

```sh
rivet --file app.rivet request rivet.capabilities --params '{}'
rivet --file app.rivet request fixture.shell --params '{}'
```

Expected behavior: Capabilities returns supported stages/platform backends; fixture.shell returns unsupported.shell, exit 5. Alternative is explicit argv command.

<a id="s80--refuse-sandboxed-process-execution-without-os-enforcement"></a>

### S80 — Refuse sandboxed process execution without OS enforcement

Stage: **A**. Required authority: allow_exec authorizes launch intent; it never exempts child I/O from policy.

`policy.json` (beside `app.rivet`):

```json
{
  "version": 1,
  "grants": [{"capability": "allow_exec", "targets": ["/usr/bin/printf"]}]
}
```

```sh
rivet --file app.rivet request fixture.print --params '{"text":"Hello"}'
```

Expected behavior: On a host without an enabled tested worker backend: unsupported.sandbox_backend, exit 5, zero process spawns. With backend, only explicitly granted child effects are available.

<a id="s81--udp-as-a-registered-operation"></a>

### S81 — UDP as a registered operation

Stage: **B**. Required authority: allow_network=udp://127.0.0.1:7000.

```rivet
operation telemetry.status
    output json
    with udp "127.0.0.1:7000" as socket
        max_datagram 8192
        socket.send json {command: "status"}
        return socket.receive json timeout "1s"
    end
end
```

Expected behavior: UDP fixture returns {"state":"ready"}. The operation has the same request ID/schema interface over CLI, HTTP, MCP and library. A successful send cannot prove remote execution; response loss returns timeout with uncertain effects.

<a id="s82--udp-listener-with-separately-authorized-replies"></a>

### S82 — UDP listener with separately authorized replies

Stage: **B**. Required authority: allow_listen=udp://127.0.0.1:7001; allow_network for the actual permitted reply peer, e.g. udp://127.0.0.1:7002.

```rivet
with udp bind "127.0.0.1:7001" as socket
    max_datagram 1024
    message = socket.receive_from json timeout "5s"
    socket.send_to message.peer json {received: true}
    return {peer: message.peer, value: message.data}
end
```

Expected behavior: Receive preserves sender metadata. A packet from an unauthorized reply destination does not grant permission to send back; send_to fails permission.denied. Empty bytes datagrams are valid; clipped payloads fail udp.truncated.

<a id="s83--udp-multicast-with-explicit-bind-and-interface"></a>

### S83 — UDP multicast with explicit bind and interface

Stage: **B**. Required authority: allow_network=udp://239.0.0.1:5000, allow_listen=udp://0.0.0.0:5000.

```rivet
with udp multicast "239.0.0.1:5000" as socket
    bind "0.0.0.0:5000"
    interface "eth0"
    max_datagram 4096
    message = socket.receive_from bytes timeout "5s"
    return message
end
```

Expected behavior: Joins one group on the selected interface, returns sender/data and leaves membership on exit. Unsupported interface/multicast is a typed refusal; timeout does not trigger retries. Interface eth0 is a fixture value to replace for the host.

<a id="s84--oauth-client-credentials-profile-and-request"></a>

### S84 — OAuth client credentials profile and request

Stage: **B**. Required authority: allow_auth=crm_service/service/use, allow_credentials=crm_service/service, allow_env=CRM_CLIENT_SECRET, allow_network=https://auth.example.com:443, allow_network=https://api.example.com:443.

```rivet
auth crm_service oauth2
    flow client_credentials
    issuer "https://auth.example.com"
    token_url "https://auth.example.com/token"
    client_id "rivet-service"
    client_secret env "CRM_CLIENT_SECRET"
    client_auth basic
    scopes ["contacts.read"]
    resource_origins ["https://api.example.com:443"]
    store memory
end

operation contacts.list
    output json
    response = http get "https://api.example.com/contacts"
        auth crm_service account "service"
        decode json
    end
    return response.body
end
```

Expected behavior: First authorized use obtains a token; subsequent valid cache hits reuse it. Expiry reacquires for this flow. Tokens never appear in the operation result. Missing env/grants fail before the corresponding effect.

<a id="s85--oauth-authorization-code-with-pkce-profile"></a>

### S85 — OAuth authorization code with PKCE profile

Stage: **B**. Required authority: management/use/status allow_auth selectors as needed, allow_credentials=crm_user/ada and token-endpoint network; host separately owns callback/browser I/O.

```rivet
auth crm_user oauth2
    flow authorization_code
    pkce s256
    issuer "https://auth.example.com"
    authorization_url "https://auth.example.com/authorize"
    token_url "https://auth.example.com/token"
    client_id "rivet-desktop"
    client_auth none
    redirect_uri "https://app.example.com/oauth/callback"
    scopes ["contacts.read"]
    resource_origins ["https://api.example.com:443"]
    store keychain "rivet/crm"
end
```

Expected behavior: Public-client fixture; callback belongs to the host application. Rivet creates single-use state/verifier and binds transaction to principal, account and profile. No browser/listener is opened implicitly. Secure backend must support the declared storage contract or refuse.

<a id="s86--oauth-device-authorization-profile"></a>

### S86 — OAuth device authorization profile

Stage: **B**. Required authority: allow_auth=crm_device/ada/manage, allow_credentials=crm_device/ada, allow_network=https://auth.example.com:443.

```rivet
auth crm_device oauth2
    flow device_code
    issuer "https://auth.example.com"
    device_url "https://auth.example.com/device"
    token_url "https://auth.example.com/token"
    client_id "rivet-cli"
    client_auth none
    scopes ["contacts.read"]
    resource_origins ["https://api.example.com:443"]
    store memory
end
```

Expected behavior: Suitable for a CLI without callback collection. Start and complete on the same runtime with --endpoint; two unrelated local CLI processes cannot share this memory store.

<a id="s87--start-user-authorization-through-the-shared-interface"></a>

### S87 — Start user authorization through the shared interface

Stage: **B**. Required authority: server principal has allow_auth=crm_user/ada/manage and allow_credentials=crm_user/ada; code begin itself performs no token HTTP call.

```sh
rivet --endpoint http://127.0.0.1:8080 auth begin crm_user --account ada
curl -sS http://127.0.0.1:8080/v1/request   -H 'Content-Type: application/json'   -d '{"id":"rivet.auth.begin","params":{"profile":"crm_user","account":"ada"}}'
```

Expected behavior: Each alternative starts a distinct transaction; use only one for a single login. Completion.result contains transaction_id, authorization_url and expires_at. The host presents the URL for user authorization; no token/code verifier is returned.

<a id="s88--complete-the-callback-without-putting-codes-in-argv"></a>

### S88 — Complete the callback without putting codes in argv

Stage: **B**. Required authority: allow_auth=crm_user/ada/manage, allow_credentials=crm_user/ada, allow_network=https://auth.example.com:443; explicit params-file is invocation input.

```sh
rivet --endpoint http://127.0.0.1:8080 auth complete --params-file ./callback.json
# callback.json (host supplies actual values from its callback):
# {"transaction_id":"auth_01","callback":{"code":"provider-code","state":"returned-state","redirect_uri":"https://app.example.com/oauth/callback","issuer":"https://auth.example.com"}}
```

Expected behavior: Checks state/issuer/redirect/owner/TTL before exchange. Returns connected CredentialStatus, never tokens. Replay or mismatch -> auth.callback_invalid. Treat the temporary callback input as sensitive and delete through the host-controlled workflow.

<a id="s89--complete-device-authorization-with-bounded-polling"></a>

### S89 — Complete device authorization with bounded polling

Stage: **B**. Required authority: allow_auth=crm_device/ada/manage, allow_credentials=crm_device/ada, allow_network=https://auth.example.com:443.

```sh
rivet --endpoint http://127.0.0.1:8080 auth begin crm_device --account ada
# result: {"transaction_id":"auth_device_01","verification_uri":"https://auth.example.com/activate","user_code":"ABCD-EFGH","expires_at":"2026-09-28T12:10:00Z","interval_seconds":5}
# The user visits verification_uri and approves; use the actual returned transaction ID.
rivet --endpoint http://127.0.0.1:8080 --timeout 2m auth complete   --params '{"transaction_id":"auth_device_01","wait":true}'
```

Expected behavior: Waits at least provider interval; slow_down increases interval; denial, expiry and cancellation stop polling. Returns connected status on success. If the `--timeout 2m` deadline arrives first, Completion.result is `{"state":"pending"}` and the transaction is not consumed; call complete again with the same transaction_id. Only explicit `auth cancel` or expiry invalidates it. No detached background polling after cancellation.

<a id="s90--inspect-and-disconnect-an-oauth-account"></a>

### S90 — Inspect and disconnect an OAuth account

Stage: **B**. Required authority: allow_auth=crm_user/ada/status for status; allow_auth=crm_user/ada/manage for disconnect; both require allow_credentials=crm_user/ada.

```sh
rivet --endpoint http://127.0.0.1:8080 auth status crm_user --account ada
rivet --endpoint http://127.0.0.1:8080 auth disconnect crm_user --account ada
```

Expected behavior: Status returns profile/account/state/scopes/expiry/generation without refreshing. Disconnect returns local_only:true and a new generation; it removes local credentials, invalidates local transactions and prevents new leases. Provider revocation is not claimed.

<a id="s91--use-oauth-from-the-rust-library"></a>

### S91 — Use OAuth from the Rust library

Stage: **B**. Required authority: same grants as S84; no extra authority from embedding.

```rust
let c: Completion = rt.request("contacts.list", json!({}), None).await?;
let contacts = c.result;
```

Expected behavior: Proposed API sketch using S84. Host principal/policy selects authorized profile/account. Auth refresh remains inside the brokered adapter and returns no public token lease; concurrent calls coordinate refresh by credential identity.

<a id="s92--use-oauth-for-an-http-mcp-connector"></a>

### S92 — Use OAuth for an HTTP MCP connector

Stage: **B**. Required authority: allow_auth=crm_user/ada/use, allow_credentials=crm_user/ada, allow_network for token and MCP origins, allow_mcp=crm/tools/search.

```rivet
connector crm mcp
    transport http "https://mcp.example.com/mcp"
    auth crm_user account "ada"
    schema "./schemas/crm.json"
    expose tools ["search"]
end
```

Expected behavior: The profile must additionally list https://mcp.example.com:443 as an allowed resource origin and the provider must grant relevant scopes. MCP transport requests use the bound credential; remote-requested URLs cannot borrow it. This is a preconfigured OAuth profile, not a claim of full MCP auth discovery/dynamic-registration support.

<a id="s93--audit-oauth-refresh-and-its-policy-requirements"></a>

### S93 — Audit OAuth refresh and its policy requirements

Stage: **B**. Required authority: listed grants intentionally omit token endpoint.

`policy.json` (beside `app.rivet`):

```json
{
  "version": 1,
  "grants": [
    {"capability": "allow_auth",        "targets": ["crm_service/service/use"]},
    {"capability": "allow_credentials", "targets": ["crm_service/service"]},
    {"capability": "allow_env",         "targets": ["CRM_CLIENT_SECRET"]},
    {"capability": "allow_network",     "targets": ["https://api.example.com:443"]}
  ]
}
```

```sh
rivet --file app.rivet io contacts.list --transitive --format json
rivet --file app.rivet request contacts.list --params '{}'
```

Expected behavior: Inventory includes potential token acquisition/refresh, env lookup and credential-store effects even on a cache hit. In a fresh local memory store the invocation is denied before contacting auth.example.com because its network grant is absent; no resource API call follows.

<a id="s94--quic-request-through-a-scoped-reliable-stream"></a>

### S94 — QUIC request through a scoped reliable stream

Stage: **B**. Required authority: allow_network=quic://engine.example.com:4433; custom certificate files need read grants.

```rivet
operation engine.status
    output json
    with quic "quic://engine.example.com:4433" as connection
        alpn "rivet-rpc/1"
        max_streams 8
        migration false
        with connection.open bidi as stream
            framing length32 endian big max_frame 1048576
            stream.send json {action: "status"}
            return stream.receive json timeout "5s"
        end
    end
end
```

Expected behavior: Fixture returns {"state":"ready"}. Certificate name and ALPN must match. Stream and connection are disposed by context exit, including early return. max_streams and byte caps also apply under backpressure.

<a id="s95--multiplex-quic-streams-without-sharing-receive-ownership"></a>

### S95 — Multiplex QUIC streams without sharing receive ownership

Stage: **B**. Required authority: allow_network=quic://engine.example.com:4433.

```rivet
with quic "quic://engine.example.com:4433" as connection
    alpn "rivet-rpc/1"
    max_streams 2
    concurrent limit 2 timeout "10s" fail independent
        task first
            with connection.open bidi as stream
                framing newline
                stream.send json {action: "status"}
                emit stream.receive json
            end
        end
        task second
            with connection.open bidi as stream
                framing newline
                stream.send json {action: "metrics"}
                emit stream.receive json
            end
        end
    end
end
return null
```

Expected behavior: Distinct streams may run concurrently; a stream reset fails only that task under fail independent. Both children are joined; result/error policy still applies. Concurrent emissions are serialized by the request sink; arrival order across tasks is nondeterministic. Fixture supports the configured framing.

<a id="s96--signal-quic-end-of-input-while-keeping-the-response-readable"></a>

### S96 — Signal QUIC end-of-input while keeping the response readable

Stage: **B**. Required authority: allow_network=quic://engine.example.com:4433.

```rivet
with quic "quic://engine.example.com:4433" as connection
    alpn "rivet-eof/1"
    with connection.open bidi as stream
        framing raw
        timeout "5s"
        stream.send bytes (base64.decode "AAEC")
        stream.finish_send
        for chunk in stream
            emit chunk
        end
    end
end
return null
```

Expected behavior: finish_send is a protocol half-close (FIN), not resource disposal. The response side stays readable after it: the loop emits every raw chunk (the fragment declares `emits bytes`) until the peer's own FIN ends iteration, bounded by the 5s timeout. A single `stream.receive bytes` would return only one raw chunk, not the whole response. Uni streams similarly finish at scope exit; connection.accept uni yields a read-only scoped stream.

<a id="s97--use-negotiated-quic-datagrams"></a>

### S97 — Use negotiated QUIC datagrams

Stage: **B**. Required authority: allow_network=quic://engine.example.com:4433.

```rivet
with quic "quic://engine.example.com:4433" as connection
    alpn "rivet-telemetry/1"
    datagrams true
    connection.send_datagram bytes (base64.decode "AAEC")
    return connection.receive_datagram bytes timeout "1s"
end
```

Expected behavior: Peer must negotiate DATAGRAM or acquisition fails quic.datagrams_unavailable. Oversized payload -> quic.datagram_too_large. Loss/reordering remain possible; send success means local acceptance, not delivery.

<a id="s98--reauthorize-a-quic-address-change-before-sending-probes"></a>

### S98 — Reauthorize a QUIC address change before sending probes

Stage: **B**. Required authority: allow_network=quic://engine.example.com:4433 only.

```rivet
with quic "quic://engine.example.com:4433" as connection
    alpn "rivet-rpc/1"
    migration true
    with connection.open bidi as stream
        framing newline
        stream.send json {action: "suggest_new_address"}
        return stream.receive json timeout "5s"
    end
end
```

Expected behavior: Test peer suggests port 4444; with only a 4433 grant, Rivet sends zero path probes to 4444 and returns permission.quic_path_denied if the operation requires that path. Remaining on the original authorized path is allowed only if the protocol can complete there.

<a id="s99--http3-with-the-ordinary-http-syntax"></a>

### S99 — HTTP/3 with the ordinary HTTP syntax

Stage: **B**. Required authority: S84 grants; HTTPS-origin permit authorizes this adapter's same-origin QUIC/UDP, not arbitrary raw UDP.

```rivet
operation items.h3
    output json
    response = http get "https://api.example.com/items"
        version 3
        auth crm_service account "service"
        decode json
    end
    return {items: response.body.items, version: response.version}
end
```

Expected behavior: Fixture returns {"items":[],"version":3}. Strict H3 never silently downgrades. OAuth is configured by S84; certificate, token-origin and policy rules apply unchanged.

<a id="s100--allow-http-version-fallback-explicitly"></a>

### S100 — Allow HTTP version fallback explicitly

Stage: **B**. Required authority: allow_network=https://api.example.com:443; both permitted transports appear in inventory.

```rivet
response = http get "https://api.example.com/items"
    version prefer [3, 2]
    decode json
end
return {version: response.version, items: response.body.items}
```

Expected behavior: If H3 negotiation fails before application data is sent, H2 is permitted. After any request data is sent, existing retry/idempotency rules govern replay. A POST with uncertain execution must fail rather than silently send again over H2.

<a id="s101--inspect-native-quic-versus-http3-authority"></a>

### S101 — Inspect native QUIC versus HTTP/3 authority

Stage: **B**. Required authority: exact QUIC grant in the invocation; no credentials needed for engine.status.

```sh
rivet --file app.rivet io engine.status --transitive --format json
rivet --file app.rivet io items.h3 --transitive --format json
rivet --file app.rivet request engine.status --params '{}'
```

`policy.json` (beside `app.rivet`) for the request:

```json
{
  "version": 1,
  "grants": [{"capability": "allow_network", "targets": ["quic://engine.example.com:4433"]}]
}
```

Expected behavior: Inventory distinguishes native QUIC origin/ALPN/stream effects from HTTPS+H3 and auth/store effects. A QUIC grant does not permit raw arbitrary UDP; an HTTPS grant does not permit another origin/alternate port via Alt-Svc.

<a id="s102--inspect-capabilities-and-explicit-refusal-modes"></a>

### S102 — Inspect capabilities and explicit refusal modes

Stage: **B**. Required authority: none; run with no policy.json beside `app.rivet`. Denied invocations perform no external effects.

```sh
rivet --file app.rivet request rivet.capabilities --params '{}'
rivet --file app.rivet request telemetry.status --params '{}'
rivet --file app.rivet request items.h3 --params '{}'
```

Expected behavior: Capabilities lists udp, udp_multicast, oauth2_client_credentials, oauth2_pkce, oauth2_device, quic_v1, quic_datagram and http3 with support/reason. With no policy.json, telemetry.status and items.h3 are refused (permission.denied, exit 3) before new I/O. Unsupported OAuth password/implicit flow or QUIC early_data true is rejected explicitly; invalid_grant produces auth.login_required, not an infinite refresh loop.

<a id="s103--define-several-documented-operations-in-one-file"></a>

### S103 — Define several documented operations in one file

Stage: **A**. Required authority: none for these operation bodies; file loading is explicit host bootstrap. Save as `catalog.rivet`.

```rivet
operation demo.greet
    name "Greet a person"
    description "Return a greeting for the supplied person."
    param person text required description "The person's display name."
    output text description "Greeting sentence."
    return "Hello, ${person}!"
end

operation demo.add
    name "Add two integers"
    description "Add two signed integers and return their sum."
    param a integer required description "First operand."
    param b integer default 0 description "Second operand; defaults to zero."
    output integer description "Sum of a and b."
    return a + b
end

operation demo.health
    name "Check availability"
    description "Return a constant readiness response without I/O."
    output json description "Readiness object."
    return {ready: true}
end
```

Expected: three independently callable catalog entries from one source file. The header is the stable ID; `name` is a display label. Definitions perform no application I/O when loaded. Integer overflow returns a typed error rather than wrapping.

<a id="s104--discover-and-invoke-the-catalog-from-cli"></a>

### S104 — Discover and invoke the catalog from CLI

Stage: **A**. Required authority: explicit source loading, then no application I/O; no policy.json beside `catalog.rivet` (deny-by-default is enough for pure operations).

```sh
rivet --file catalog.rivet list --json
rivet --file catalog.rivet describe demo.add --json
rivet --file catalog.rivet request demo.add --params '{"a":2,"b":3}'
rivet --file catalog.rivet request demo.greet --params '{"person":"Ada"}'
rivet --file catalog.rivet request demo.health --params '{}'
```

Expected Completion.result values: `5`, `"Hello, Ada!"`, `{"ready":true}`. Describe includes display name, description, both parameter descriptions, default `b:0`, and source location. Unknown parameter `c` is rejected before execution. The chosen source bytes and stdout result sink are invocation capabilities, not general read/write grants.

<a id="s105--call-the-same-catalog-over-http"></a>

### S105 — Call the same catalog over HTTP

Stage: **A**. Required authority: host listener/bootstrap and authorized incoming principal. Load S103 on the fixture server.

```sh
curl -sS http://127.0.0.1:8080/v1/operations/demo.add
curl -sS http://127.0.0.1:8080/v1/request -H 'Content-Type: application/json' \
  -d '{"id":"demo.add","params":{"a":2,"b":3}}'
curl -sS http://127.0.0.1:8080/v1/request -H 'Content-Type: application/json' \
  -d '{"id":"demo.greet","params":{"person":"Ada"}}'
```

Expected: GET returns the same descriptor as CLI describe. POST responses are HTTP 200 Completion, for example `{"request_id":"req_01","trace_id":"tr_01","result":5,"data_count":0,"effects":"none"}`. Omitting `a` produces HTTP 422 `validation.required`; omitting `b` applies zero.

<a id="s106--use-the-whole-catalog-as-a-rust-library"></a>

### S106 — Use the whole catalog as a Rust library

Stage: **A**. Required authority: none for operations; host supplies source bytes and policy.

```rust
let rt = Runtime::builder()
    .source(include_str!("catalog.rivet"))
    .policy(Policy::from_json(br#"{"version": 1}"#)?)   // no grants: pure operations only
    .build()?;
let sum: Completion = rt.request("demo.add", json!({"a": 2, "b": 3}), None).await?;
let greeting: Completion = rt.request("demo.greet", json!({"person": "Ada"}), None).await?;
assert_eq!(sum.result, json!(5));
assert_eq!(greeting.result, json!("Hello, Ada!"));
let spec: OutputSpec = rt.outputs("demo.add")?;   // integer, "Sum of a and b."
```

Expected: one compilation, multiple calls, no CLI subprocess or listener. Canonical proposed Rust API sketch; `Policy::from_json` accepts the policy.json schema. Concurrent requests isolate mutable locals and cancellation.

<a id="s107--publish-the-catalog-through-mcp-stdio"></a>

### S107 — Publish the catalog through MCP stdio

Stage: **B**. Required authority: explicit source loading and protocol stdin/stdout bootstrap; no application I/O for S103, so no policy.json is needed.

```sh
rivet --file catalog.rivet serve --stdio
```

Expected: the MCP client launches this process and performs initialization before tools/list. Protocol messages alone use stdout. The authorized catalog publishes `demo.greet`, `demo.add` and `demo.health` as direct named tools (the canonical way to call them), plus the built-in generic tools `rivet.request`, `rivet.list`, `rivet.describe`, `rivet.outputs` and `rivet.sessions.*`. The display label never replaces a callable ID. An outgoing `connector ... mcp` declaration is unnecessary to serve local operations.

<a id="s108--discover-described-operations-as-mcp-tools"></a>

### S108 — Discover described operations as MCP tools

Stage: **B**. Required authority: initialized authorized MCP connection to S107 or S120.

```json
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
```

Expected `result.tools` contains this entry (alongside the other authorized entries):

```json
{
  "name":"demo.add",
  "title":"Add two integers",
  "description":"Add two signed integers and return their sum.",
  "inputSchema":{
    "type":"object",
    "properties":{
      "a":{"$ref":"#/$defs/integer","description":"First operand."},
      "b":{"$ref":"#/$defs/integer","default":0,"description":"Second operand; defaults to zero."}
    },
    "required":["a"],
    "additionalProperties":false,
    "$defs":{"integer":{"oneOf":[
      {"type":"integer","minimum":-9007199254740991,"maximum":9007199254740991},
      {"type":"object","properties":{"$type":{"const":"integer"},"decimal":{"type":"string","pattern":"^-?(0|[1-9][0-9]*)$"}},"required":["$type","decimal"],"additionalProperties":false}
    ]}}
  }
}
```

The displayed entry omits `outputSchema` for brevity; the actual tool's `outputSchema` is the Completion envelope whose `result` property is the declared output schema (`integer`, "Sum of a and b."), including the tagged large-integer variant. The dispatcher additionally enforces signed 64-bit bounds and canonical decimal encoding; JSON Schema alone cannot express that string-to-integer range check. The same rules apply on every surface.

```json
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"demo.add","arguments":{"a":2,"b":3}}}
```

Expected: `result.structuredContent` is Completion with `result:5`; `result.content` includes its serialized JSON text. No second routing table or different validation rules. Invalid arguments return a safe tool error with `isError:true` and ErrorEnvelope, without running the operation.

<a id="s109--reject-duplicate-ids-and-keep-helpers-private"></a>

### S109 — Reject duplicate IDs and keep helpers private

Stage: **A**. Required authority: none after source loading.

```rivet
operation helper.normalize
    private true
    param value text required
    output text
    return value
end

operation demo.echo
    description "Return a supplied string through an internal helper."
    param value text required description "Text to return."
    output text
    return (request "helper.normalize" {value: value})
end
```

Expected: `demo.echo` is public; helper is callable only inside the compiled bundle. Direct external request, MCP tools/call and sessions.open cannot bypass visibility. Defining another `operation demo.echo` in any loaded source rejects the entire candidate catalog with `registry.duplicate_id`, showing both spans; no last-definition-wins behavior. Existing running catalog remains unchanged after a failed replacement.

<a id="s110--call-unary-grpc-with-a-pinned-descriptor"></a>

### S110 — Call unary gRPC with a pinned descriptor

Stage: **B**. Required authority: approved descriptor input, network origin and method grant. Save this connector and dependent operations as `grpc.rivet`.

```rivet
connector users grpc
    endpoint "https://users.example.com:443"
    descriptor "./schemas/users.pb"
    service "example.Users"
end

operation users.grpc_get
    name "Get a user over gRPC"
    description "Read one user from the gRPC service."
    param id text required description "User ID."
    output json
    response = grpc users.GetUser
        message {id: id}
        timeout "5s"
    end
    return response.message
end
```

`policy.json` (beside `grpc.rivet`):

```json
{
  "version": 1,
  "grants": [
    {"capability": "allow_network", "targets": ["https://users.example.com:443"]},
    {"capability": "allow_grpc",    "targets": ["users/example.Users/GetUser"]}
  ],
  "approved": {"snapshots": ["sha256:<hash of ./schemas/users.pb>"]}
}
```

```sh
rivet --file grpc.rivet request users.grpc_get --params '{"id":"42"}'
```

Expected Completion.result: `{"id":"42","name":"Ada"}` from the fixture. The ID is `users.grpc_get`, distinct from the HTTP `users.get` (integer `id`), because the gRPC method takes a string ID. `users.pb` is a reviewed (hash-approved) binary FileDescriptorSet with dependencies supplied as explicit compilation input; compilation performs no reflection, network discovery or implicit protoc execution. Ordinary HTTP 200 without gRPC OK trailers is not success.

<a id="s111--forward-server-streaming-grpc-data"></a>

### S111 — Forward server streaming gRPC data

Stage: **B**. Required authority: S110 origin and `allow_grpc=users/example.Users/Watch`.

```rivet
operation users.watch
    name "Watch user changes"
    description "Emit changes until the server completes or the deadline expires."
    param topic text required description "Subscription topic."
    output json
    emits json
    with grpc users.Watch as rpc
        message {topic: topic}
        timeout "20s"
        for message in rpc
            emit message
        end
        return rpc.completion
    end
end
```

Expected: ordered data events followed by terminal result only after OK trailers. Slow consumers apply bounded backpressure. Peer error after some messages becomes a terminal error preserving partial output. Scope cleanup runs on return, cancellation and failure.

<a id="s112--upload-a-finite-list-with-client-streaming-grpc"></a>

### S112 — Upload a finite list with client streaming gRPC

Stage: **B**. Required authority: S110 origin and `allow_grpc=users/example.Users/Upload`.

```rivet
operation users.upload
    description "Upload a finite batch and return the accepted count."
    param items json required description "Array of objects with a text field."
    output json
    with grpc users.Upload as rpc
        timeout "10s"
        for item in items
            rpc.send item
        end
        rpc.finish_send
        response = rpc.result
        return response.message
    end
end
```

Expected: fixture input `{"items":[{"text":"a"},{"text":"b"}]}` returns `{"count":2}`. `finish_send` signals end-of-input while retaining the response side; there is no `close`. A non-array input fails iteration type validation, and invalid message fields fail descriptor validation; this operation does not claim atomic upload or automatic replay. A finite batch needs no externally interactive session and is callable directly as an ordinary MCP tool.

<a id="s113--declare-bidirectional-grpc-with-live-input"></a>

### S113 — Declare bidirectional gRPC with live input

Stage: **B**. Required authority: S110 origin and `allow_grpc=users/example.Users/Chat`.

```rivet
operation chat.exchange
    name "Chat with the service"
    description "Send and receive chat messages within one bounded call."
    output json
    emits json
    receives json
    with grpc users.Chat as rpc
        timeout "30s"
        concurrent limit 2 fail fast
            task send
                for message in incoming
                    rpc.send message
                end
                rpc.finish_send
            end
            task receive
                for message in rpc
                    emit message
                end
            end
        end
        return rpc.completion
    end
end
```

Expected: send and receive progress independently without buffering the entire conversation. Descriptor validation applies to each message. Early peer completion closes the input feeder and wakes blocked senders; task joining cannot wait indefinitely on abandoned input. `incoming` exists only for an operation declaring `receives`.

<a id="s114--drive-a-duplex-call-through-http-session-operations"></a>

### S114 — Drive a duplex call through HTTP session operations

Stage: **B**. Required authority: authorized principal and chat.exchange grants on a running fixture host. Use the real returned session_id in later calls; `sess_01` is illustrative.

```sh
curl -sS http://127.0.0.1:8080/v1/request -H 'Content-Type: application/json' \
  -d '{"id":"rivet.sessions.open","params":{"id":"chat.exchange","params":{}}}'
# Completion.result: {"session_id":"sess_01","request_id":"req_chat","catalog_version":"sha256:fixture","input_schema":{},"emits_schema":{},"next_send_seq":1,"expires_at":"2026-09-28T12:00:30Z"}
curl -sS http://127.0.0.1:8080/v1/request -H 'Content-Type: application/json' \
  -d '{"id":"rivet.sessions.send","params":{"session_id":"sess_01","send_seq":1,"data":{"text":"hello"}}}'
# Completion.result: {"session_id":"sess_01","accepted_seq":1,"input_closed":false}
curl -sS http://127.0.0.1:8080/v1/request -H 'Content-Type: application/json' \
  -d '{"id":"rivet.sessions.finish_input","params":{"session_id":"sess_01"}}'
# Completion.result: {"session_id":"sess_01","accepted_seq":null,"input_closed":true}
curl -sS http://127.0.0.1:8080/v1/request -H 'Content-Type: application/json' \
  -d '{"id":"rivet.sessions.read","params":{"session_id":"sess_01","after_seq":0,"max_events":16,"wait_ms":1000}}'
# Completion.result: {"session_id":"sess_01","events":[{"request_id":"req_chat","trace_id":"tr_chat","seq":1,"type":"data","data":{"text":"hello"}}],"last_seq":1,"terminal":false}
curl -sS http://127.0.0.1:8080/v1/request -H 'Content-Type: application/json' \
  -d '{"id":"rivet.sessions.cancel","params":{"session_id":"sess_01"}}'
# Completion.result: {"session_id":"sess_01","request_id":"req_chat","state":"cancelled"} after cleanup; no rollback of messages already sent.
```

Expected: each successful control call returns HTTP 200 Completion. The empty input/emits schemas reflect `receives json`/`emits json`; individual gRPC messages undergo additional descriptor validation before adapter send or after decode. In normal consumption repeat read with the last delivered sequence until terminal; cancel is shown for early termination. Production send/read loops run concurrently. Send acknowledgment means local queue acceptance, not remote processing. Identical retry of the most recent send_seq acknowledges without re-enqueue; a conflicting payload returns 409. Cross-principal access is denied even with a valid session ID. Idle/deadline expiry cleans up abandoned sessions. The same session is reachable through the polling routes (S134).

<a id="s115--own-a-duplex-request-in-a-rust-scope"></a>

### S115 — Own a duplex request in a Rust scope

Stage: **B**. Required authority: chat.exchange grants in the host policy. Canonical proposed Rust API sketch.

```rust
rt.scope(|scope| async move {
    let duplex = scope.duplex("chat.exchange", json!({})).await?;
    let (mut input, mut output) = duplex.split();
    let send = async {
        input.send(json!({"text": "hello"})).await?;
        input.finish_send().await
    };
    let receive = async {
        while let Some(event) = output.next().await? {
            consume(event).await?;
        }
        Ok::<_, RivetError>(())
    };
    tokio::try_join!(send, receive)?;
    Ok(())
}).await?;
```

Expected: split handles remain scope-bound. Input is half-closed, outputs drained, and cleanup awaited. `next()` returns `Result<Option<Envelope>>`: terminal failure is `Err(RivetError)`, and the terminal result envelope arrives before `Ok(None)`. Host `consume` is caller code and must obey the host's own policy; embedding cannot sandbox arbitrary native host functions. Cancellation/future drop triggers supervised cleanup.

<a id="s116--drive-a-streaming-operation-as-an-mcp-tool"></a>

### S116 — Drive a streaming operation as an MCP tool

Stage: **B**. Required authority: initialized authorized MCP client plus chat.exchange grants.

```json
{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"chat.exchange","arguments":{}}}
```

Expected: calling the operation as its direct named tool (canonical) opens a bounded session and returns SessionReceipt in structuredContent and serialized content; the descriptor advertises `_meta: {"rivet/delivery":"session"}` and the receipt/error outputSchema. This is session startup, not the final chat result.

```json
{"jsonrpc":"2.0","id":11,"method":"tools/call","params":{"name":"rivet.sessions.send","arguments":{"session_id":"sess_01","send_seq":1,"data":{"text":"hello"}}}}
{"jsonrpc":"2.0","id":12,"method":"tools/call","params":{"name":"rivet.sessions.read","arguments":{"session_id":"sess_01","after_seq":0,"wait_ms":1000}}}
{"jsonrpc":"2.0","id":13,"method":"tools/call","params":{"name":"rivet.sessions.finish_input","arguments":{"session_id":"sess_01"}}}
```

Expected: control tools return Completion containing SessionAck/SessionBatch; repeat read until terminal. An operation failure is a terminal error inside the returned batch, while a failed control operation is an MCP tool error. MCP progress notifications are not used as an application data channel. Disconnected clients cannot keep sessions alive indefinitely.

<a id="s117--preserve-late-grpc-failures-after-partial-output"></a>

### S117 — Preserve late gRPC failures after partial output

Stage: **B**. Required authority: Watch fixture grants. Configure fixture to emit one item and then gRPC UNAVAILABLE.

`policy.json` (beside `grpc.rivet`):

```json
{
  "version": 1,
  "grants": [
    {"capability": "allow_network", "targets": ["https://users.example.com:443"]},
    {"capability": "allow_grpc",    "targets": ["users/example.Users/Watch"]}
  ],
  "approved": {"snapshots": ["sha256:<hash of ./schemas/users.pb>"]}
}
```

```sh
rivet --file grpc.rivet request users.watch --params '{"topic":"fail_after_one"}' --stream
```

Expected NDJSON: one data event, then one terminal error event with `error.code:"grpc.unavailable"`, `error.details.grpc_status:14`, and partial-output context. CLI exits 5. The adapter must consume final trailers; HTTP status 200 and one decoded message cannot turn this into success. No automatic replay after emission. Trace includes method, descriptor hash, safe status and cleanup; authorization metadata and sensitive status details are redacted. CANCELLED and DEADLINE_EXCEEDED remain distinguishable from UNAVAILABLE.

<a id="s118--authorize-grpc-with-oauth-and-inspect-every-effect"></a>

### S118 — Authorize gRPC with OAuth and inspect every effect

Stage: **B**. Required authority: separate profile, credentials, token/resource origins and method grants. Adapt S84's `crm_service` profile to include `https://users.example.com:443` in resource_origins; do not assume an unrelated API token is valid here.

```rivet
operation users.secure_get
    description "Read a user using the service OAuth profile."
    param id text required description "User ID."
    output json
    response = grpc users.GetUser
        message {id: id}
        auth crm_service account service
        metadata "x-request-id" "fixture-request"
        timeout "5s"
    end
    return response.message
end
```

```sh
rivet --file grpc.rivet io users.secure_get --transitive --include-bootstrap --format json
rivet --file grpc.rivet policy explain users.secure_get --params '{"id":"42"}' --json
```

`policy.json` (beside `grpc.rivet`) used by `policy explain`:

```json
{
  "version": 1,
  "grants": [
    {"capability": "allow_network",     "targets": ["https://users.example.com:443", "https://auth.example.com:443"]},
    {"capability": "allow_grpc",        "targets": ["users/example.Users/GetUser"]},
    {"capability": "allow_auth",        "targets": ["crm_service/service/use"]},
    {"capability": "allow_credentials", "targets": ["crm_service/service"]}
  ]
}
```

Expected inventory: descriptor bootstrap read, OAuth credential/secret source, possible token exchange/refresh, DNS/TLS/HTTP2 method and trace sink. Policy explain reports additional source-specific grants needed by S84's client secret backend; it never claims the shown grants authorize an unspecified secret provider. A method grant alone cannot grant network or token access. Successful authenticated execution requires all reported grants. TLS verification and origin binding precede token attachment.

<a id="s119--validate-descriptions-and-stream-cli-input"></a>

### S119 — Validate descriptions and stream CLI input

Stage: **A** for strict documentation; **B** for duplex. Required authority: declared source inputs; chat grants and explicitly selected stdin invocation capability.

```sh
rivet --file catalog.rivet check --strict-docs
rivet --endpoint http://127.0.0.1:8080 request chat.exchange --params '{}' --input-jsonl - --stream <<'JSONL'
{"text":"hello"}
{"text":"goodbye"}
JSONL
```

Expected: strict-docs rejects blank public operation/parameter/output descriptions and undeclared `fail` codes with source spans; compact older examples may intentionally fail this optional gate. CLI reads stdin concurrently with emitting NDJSON on stdout. EOF finishes input; SIGINT cancels and waits for bounded cleanup. Malformed JSONL produces a typed input error and cancels the live request; prior messages may already have reached the peer. `--input-jsonl` requires a receives schema (otherwise `stream.input_required`, 422 / exit 2); `--stream` is required with live input to avoid buffering indefinitely (otherwise `stream.required`, 422 / exit 2).

<a id="s120--serve-one-catalog-through-http-and-mcp-together"></a>

### S120 — Serve one catalog through HTTP and MCP together

Stage: **B**. Required authority: explicit source and listener; no policy.json, so auth is `none` and the bind must be loopback.

```sh
rivet --file catalog.rivet serve --listen 127.0.0.1:8080
curl -sS http://127.0.0.1:8080/v1/request -H 'Content-Type: application/json' \
  -d '{"id":"demo.add","params":{"a":2,"b":3}}'
```

After the proposal's MCP initialize/initialized exchange, use the actual returned MCP session ID (the fixture value below is illustrative):

```sh
curl -sS http://127.0.0.1:8080/mcp \
  -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' \
  -H 'MCP-Session-Id: demo-session' -H 'MCP-Protocol-Version: 2025-11-25' \
  -d '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"demo.add","arguments":{"a":2,"b":3}}}'
```

Expected: REST Completion.result and MCP structuredContent.result both equal 5, with distinct request/trace IDs but identical schema/default/error semantics. MCP includes serialized Completion in content for compatibility. HTTP transport policy may choose JSON or SSE framing per MCP negotiation; this does not introduce arbitrary tool-level duplex streaming. Principal authorization filters both catalogs consistently. When REST exposure is unwanted, narrow the mounted surfaces in policy.json (`"serve": {"surfaces": ["mcp"]}`, S137) instead of a flag.

<a id="s121--declare-a-scalar-output-with-a-description"></a>

### S121 — Declare a scalar output with a description

Stage: **A**. Required authority: none. Append to `catalog.rivet` (S103).

```rivet
operation demo.average
    name "Average two numbers"
    description "Return the arithmetic mean of two numbers."
    param a number required description "First value."
    param b number required description "Second value."
    output number description "Mean of a and b."
    total = a + b
    return total / 2
end
```

```sh
rivet --file catalog.rivet request demo.average --params '{"a":2,"b":5}'
rivet --file catalog.rivet outputs demo.average
```

Expected: Completion.result `3.5`. The scalar form `output number` stays valid without a description, but `check --strict-docs` requires one on every public operation. The description appears in `describe`, `outputs`, `GET /v1/operations/demo.average/outputs`, MCP `rivet.outputs` and the tool's `outputSchema`:

```text
demo.average — Average two numbers
output  number   Mean of a and b.
emits    —
receives —
errors   —
```

<a id="s122--declare-a-structured-output-with-nested-fields"></a>

### S122 — Declare a structured output with nested fields

Stage: **A**. Required authority: allow_network=https://api.example.com:443. Save in `users.rivet`. The fixture's GET `/users/42/profile` returns `{"id":42,"name":"Ada","tags":["developer"],"address":{"city":"London"}}`.

```rivet
operation users.profile
    name "Get a user profile"
    description "Read one user's profile including labels and postal address."
    param id integer required min 1 description "User ID."
    output object description "The user's profile."
        field id integer required description "Stable user ID."
        field name text required description "Display name."
        field tags list text optional description "Free-form labels."
        field address object optional description "Postal address."
            field city text required description "City name."
            field postcode text optional description "Postal code, when known."
        end
    end
    response = http get "https://api.example.com/users/${id}/profile"
        decode json
        timeout "10s"
    end
    return response.body
end
```

Generated output schema (`rivet outputs users.profile --json`, `output` member, `$defs` omitted):

```json
{
  "type": "object",
  "description": "The user's profile.",
  "properties": {
    "id":   {"$ref": "#/$defs/integer", "description": "Stable user ID."},
    "name": {"type": "string", "description": "Display name."},
    "tags": {"type": "array", "items": {"type": "string"}, "description": "Free-form labels."},
    "address": {
      "type": "object",
      "description": "Postal address.",
      "properties": {
        "city":     {"type": "string", "description": "City name."},
        "postcode": {"type": "string", "description": "Postal code, when known."}
      },
      "required": ["city"],
      "additionalProperties": false
    }
  },
  "required": ["id", "name"],
  "additionalProperties": false
}
```

Expected: `required` is the default per field; objects are closed unless the block contains `open true`. Types are text, integer, number, boolean, bytes, json (opaque, no structural check), object (nested fields) and `list T`. `emits object … end` and `receives object … end` use the same field form for item schemas.

<a id="s123--declare-error-codes-and-fail-strict-docs-on-an-undeclared-one"></a>

### S123 — Declare error codes and fail strict docs on an undeclared one

Stage: **A**. Required authority: allow_network=https://api.example.com:443. Save at the top of `users.rivet`; the `users.get` here is this file's HTTP operation (S01's copy lives in `app.rivet`).

```rivet
operation users.get
    name "Get a user"
    description "Read one user by numeric ID."
    param id integer required min 1 description "User ID."
    output object description "The requested user."
        field id integer required description "Stable user ID."
        field name text required description "Display name."
        field email text optional description "Primary email, when public."
    end
    error "users.not_found" description "No user has this ID."
    response = http get "https://api.example.com/users/${id}"
        accept status [200, 404]
        decode json
        timeout "10s"
    end
    if response.status == 404
        fail "users.not_found" {id: id}
    end
    return response.body
end

operation users.remove
    description "Delete one user by numeric ID."
    param id integer required min 1 description "User ID."
    output object description "Deletion receipt."
        field id integer required description "The deleted user's ID."
    end
    error "users.not_found" description "No user has this ID."
    response = http delete "https://api.example.com/users/${id}"
        accept status [204, 404, 423]
        decode bytes
    end
    if response.status == 404
        fail "users.not_found" {id: id}
    end
    if response.status == 423
        fail "users.locked" {id: id}
    end
    return {id: id}
end
```

```sh
rivet --file users.rivet check
# warning users.rivet:37:9  users.remove fails with undeclared code "users.locked"      -> exit 0
rivet --file users.rivet check --strict-docs
# error   users.rivet:37:9  users.remove fails with undeclared code "users.locked"      -> exit 2
```

Expected: `error` lines sit in the header after `output`, one per code; they document the code and add it to the operation's error schema. An undeclared `fail` code is a `check` warning and a `--strict-docs` error. Adding `error "users.locked" description "The user is locked and cannot be deleted."` clears both. Declared errors do not change runtime behaviour: `fail` still produces an `application` error.

<a id="s124--view-declared-outputs-as-a-table"></a>

### S124 — View declared outputs as a table

Stage: **A**. Required authority: none after source loading.

```sh
rivet --file users.rivet outputs users.get
```

```text
users.get — Get a user
output  object   The requested user.
  id       integer  required  Stable user ID.
  name     text     required  Display name.
  email    text     optional  Primary email, when public.
emits    —
receives —
errors
  users.not_found   No user has this ID.
```

Expected: exit 0. The table is built from the immutable RegistryEntry, the same data every surface shows. `rivet describe users.get` includes it as its Output section; `rivet list --outputs` adds a one-line summary column (`object — The requested user.`). Unknown or private ID → not_found, exit 4.

<a id="s125--export-every-declared-output-as-json-schema"></a>

### S125 — Export every declared output as JSON Schema

Stage: **A**. Required authority: none after source loading.

```sh
rivet --file users.rivet outputs users.get --json
rivet --file users.rivet outputs --all --json
```

Single-operation JSON (the `--all` form returns one such object per public operation, sorted by ID; `$defs` omitted):

```json
{
  "id": "users.get",
  "output": {
    "type": "object",
    "description": "The requested user.",
    "properties": {
      "id":    {"$ref": "#/$defs/integer", "description": "Stable user ID."},
      "name":  {"type": "string", "description": "Display name."},
      "email": {"type": "string", "description": "Primary email, when public."}
    },
    "required": ["id", "name"],
    "additionalProperties": false
  },
  "emits": null,
  "receives": null,
  "errors": [{"code": "users.not_found", "description": "No user has this ID."}]
}
```

Expected: `--all` covers `users.brief`, `users.get`, `users.profile` and `users.remove` from `users.rivet`; private operations are omitted. Output is JSON on stdout; exit 0.

<a id="s126--read-declared-outputs-over-http"></a>

### S126 — Read declared outputs over HTTP

Stage: **A**. Required authority: running `rivet --file users.rivet serve` (S133) and an authorized principal.

```sh
curl -sS http://127.0.0.1:8080/v1/operations/users.get/outputs
curl -sS -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8080/v1/operations/helper.normalize/outputs
```

Expected: the first returns HTTP 200 with the same JSON as `rivet outputs users.get --json` (S125). The second prints `404`: private and unknown IDs are not disclosed. `GET /v1/operations/users.get` also carries the output in its descriptor.

<a id="s127--read-declared-outputs-through-mcp"></a>

### S127 — Read declared outputs through MCP

Stage: **B**. Required authority: initialized authorized MCP session to `/mcp` (S136) or `serve --stdio`.

```json
{"jsonrpc":"2.0","id":20,"method":"tools/call","params":{"name":"rivet.outputs","arguments":{"id":"users.get"}}}
{"jsonrpc":"2.0","id":21,"method":"tools/call","params":{"name":"rivet.outputs","arguments":{"all":true}}}
```

Expected: `structuredContent` is a Completion whose `result` is the S125 JSON (or the list of them for `all:true`). Independently, `tools/list` gives the direct `users.get` tool an `outputSchema` equal to the Completion envelope whose `result` property is the declared output schema:

```json
{"outputSchema": {"type": "object",
  "properties": {"request_id": {"type": "string"}, "trace_id": {"type": "string"},
                 "result": {"$ref": "#/$defs/users.get.output"},
                 "data_count": {"type": "integer"}, "effects": {"type": "string"}},
  "required": ["request_id", "trace_id", "result", "data_count", "effects"]}}
```

<a id="s128--reject-a-result-that-violates-the-declared-output"></a>

### S128 — Reject a result that violates the declared output

Stage: **A**. Required authority: users.get effects and allow_write=./out/**. Append to `users.rivet`.

```rivet
operation users.brief
    description "Return a user's ID and display name, recording an audit line."
    param id integer required min 1 description "User ID."
    output object description "Brief user view."
        field id integer required description "Stable user ID."
        field name text required description "Display name."
    end
    user = (request "users.get" {id: id})
    file append "./out/audit.log" text "brief requested\n"
    return {id: user.id, display: user.name}
end
```

```sh
rivet --file users.rivet request users.brief --params '{"id":42}'
# stderr ErrorEnvelope (abridged):
# {"kind":"output_invalid","code":"output.invalid","operation_id":"users.brief",
#  "details":{"missing":["name"],"unexpected":["display"]},"effects":"committed"}
# exit 5
```

Expected: the dispatcher validates the final result against the declared output before producing Completion. `name` is missing and `display` is not allowed in a closed object, so the request fails with kind `output_invalid`, code `output.invalid`, HTTP 500 / exit 5. Effects are preserved and reported: the audit line was appended (`effects: committed`) and is not rolled back. An `output json` operation gets no structural check.

<a id="s129--discover-policyjson-beside-the-entry-file"></a>

### S129 — Discover policy.json beside the entry file

Stage: **A**. Required authority: allow_read=./fixtures/** (relative to `services/`).

```text
repo/                          <- current directory for every command below
├── policy.json                <- NOT read: discovery ignores the current directory
└── services/
    ├── app.rivet              <- entry file (--file)
    ├── policy.json            <- discovered: same directory as the entry file
    └── fixtures/config.json
```

`services/policy.json`:

```json
{
  "version": 1,
  "grants": [{"capability": "allow_read", "targets": ["./fixtures/**"]}]
}
```

```rivet
operation config.read
    description "Read the service configuration file."
    output json description "Parsed configuration."
    config = file read "./fixtures/config.json" as json
    return config
end
```

```sh
rivet --file services/app.rivet request config.read --params '{}'
rivet --file services/app.rivet io config.read --include-bootstrap --format json
mv services/policy.json services/policy.off
rivet --file services/app.rivet request config.read --params '{}'
```

Expected: the first call succeeds; `./fixtures/**` resolves from the policy file's directory (`services/`). The inventory lists `services/policy.json` as a bootstrap read. With the file renamed, no policy file is found and the same call fails `permission.denied`, exit 3, with zero file reads; pure operations would still run. A malformed file (unknown key, unknown capability, bad selector) fails to load with `policy.invalid`, exit 2; it never falls back to allow-all.

<a id="s130--block-ssrf-with-deny-rules-and-private-ranges"></a>

### S130 — Block SSRF with deny rules and private ranges

Stage: **A**. Required authority: allow_network for the two granted origins; the deny entry and private-range rule take precedence.

```rivet
operation links.fetch
    description "Fetch a caller-supplied URL within policy."
    param url text required description "Absolute https URL to fetch."
    output text description "Response body decoded as text."
    response = http get url
        decode text
        timeout "5s"
    end
    return response.body
end
```

`policy.json` (beside `app.rivet`):

```json
{
  "version": 1,
  "grants": [{"capability": "allow_network", "targets": ["https://api.example.com:443", "https://internal.example.com:443"]}],
  "deny":   [{"capability": "allow_network", "targets": ["https://internal.example.com:443"]}],
  "network": {"deny_private_ranges": true}
}
```

```sh
rivet --file app.rivet request links.fetch --params '{"url":"https://api.example.com/logs"}'
rivet --file app.rivet request links.fetch --params '{"url":"https://internal.example.com/admin"}'
rivet --file app.rivet request links.fetch --params '{"url":"http://169.254.169.254/latest/meta-data/"}'
rivet --file app.rivet io links.fetch --format json
```

```text
 url -> origin granted? --no--> permission.denied (no DNS)
            | yes
            v
        origin in deny? --yes--> permission.denied (no DNS)
            | no
            v
        resolve DNS -> address private/loopback/link-local/metadata? --yes--> permission.denied (no connect)
            | no
            v
        connect to the checked address (bound to this connection)
```

Expected: call 1 succeeds. Call 2 is denied because `deny` overrides `grants` (exit 3, no DNS). Call 3 is denied: its origin is not granted and 169.254.169.254 is a private-range literal. If the fixture DNS for api.example.com is switched to answer 10.0.0.7, call 1 is denied after resolution and before connecting. `deny_private_ranges` is already true by default; only a grant naming the IP/CIDR literally admits a private address. The inventory marks the `http get url` target `param_dependent`.

<a id="s131--authenticate-serve-callers-with-bearer-tokens-and-principals"></a>

### S131 — Authenticate serve callers with bearer tokens and principals

Stage: **B**. Required authority: listener bootstrap; operations in `catalog.rivet` are pure. Fixture tokens are `test` (ada) and `ci-token` (ci); only their SHA-256 hashes are stored.

`policy.json` (beside `catalog.rivet`):

```json
{
  "version": 1,
  "serve": {
    "auth": {
      "type": "bearer",
      "tokens": [
        {"principal": "ada", "sha256": "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"},
        {"principal": "ci",  "sha256": "948b8c2427cd29047839b8e4a27a08763f8befbafa86be5cce8e46217d75e58a"}
      ]
    },
    "principals": {
      "ada": {"operations": ["demo.*", "users.get"]},
      "ci":  {"operations": ["demo.health"]}
    }
  }
}
```

```sh
rivet --file catalog.rivet serve --listen 0.0.0.0:8080
curl -sS http://127.0.0.1:8080/v1/request -H 'Authorization: Bearer test' \
  -H 'Content-Type: application/json' -d '{"id":"demo.add","params":{"a":2,"b":3}}'
curl -sS http://127.0.0.1:8080/v1/request \
  -H 'Content-Type: application/json' -d '{"id":"demo.add","params":{"a":2,"b":3}}'
curl -sS http://127.0.0.1:8080/v1/request -H 'Authorization: Bearer ci-token' \
  -H 'Content-Type: application/json' -d '{"id":"demo.add","params":{"a":2,"b":3}}'
curl -sS http://127.0.0.1:8080/v1/operations -H 'Authorization: Bearer ci-token'
```

```text
 request --(Authorization: Bearer T)--> sha256(T) in tokens? --no--> 401 auth
                                              | yes -> principal
                                              v
                         principal's operations match ID? --no--> 403 permission.denied
                                              | yes
                                              v
                                   dispatcher (same on REST/SSE/poll/WS/MCP)
```

Expected: ada gets 200 with result 5; no header gets 401; ci gets 403 for demo.add, and ci's operation list contains only `demo.health`. The same token authenticates the WebSocket upgrade request and every `/mcp` HTTP request. A non-loopback bind is allowed because auth is configured; bearer tokens are plaintext on plain HTTP, so the fixture uses an isolated network (production uses `mtls` or a TLS-terminating proxy). A library host can supply an `Authenticator` callback instead.

<a id="s132--refuse-a-non-loopback-serve-without-authentication"></a>

### S132 — Refuse a non-loopback serve without authentication

Stage: **B**. Required authority: none; the listener never opens.

```sh
rivet --file catalog.rivet serve --listen 0.0.0.0:8080
# stderr: {"kind":"validation","code":"serve.auth_required",
#          "message":"non-loopback listener requires serve.auth in policy.json"}
# exit 2
rivet --file catalog.rivet serve --listen 127.0.0.1:8080
```

Expected: with no policy.json (or with `"serve": {"auth": {"type": "none"}}`), a non-loopback bind refuses to start with `serve.auth_required`, exit 2, and no socket is bound. The loopback bind starts with principal `local`. Missing `serve.auth` is treated as none, so it is loopback-only.

<a id="s133--serve-every-surface-from-one-listener"></a>

### S133 — Serve every surface from one listener

Stage: **B**. Required authority: explicit source and loopback listener; no policy.json (auth none, pure operations only). Append these operations to `catalog.rivet`:

```rivet
operation demo.count
    description "Emit the integers 1..n, then return n."
    param n integer default 3 min 1 max 10 description "How many integers to emit."
    output integer description "The last integer emitted."
    emits integer
    i = 0
    iterate max 10
        if i == n
            break
        end
        i = i + 1
        emit i
    end
    return n
end

operation demo.relay
    description "Emit each received text item back, then return how many were relayed."
    output integer description "Number of items relayed."
    emits text
    receives text
    count = 0
    for item in incoming
        emit item
        count = count + 1
    end
    return count
end
```

```sh
rivet --file catalog.rivet serve                       # listens on 127.0.0.1:8080
curl -sS http://127.0.0.1:8080/v1/request -H 'Content-Type: application/json' \
  -d '{"id":"demo.add","params":{"a":2,"b":3}}'
curl -sS -N http://127.0.0.1:8080/v1/request -H 'Content-Type: application/json' \
  -H 'Accept: text/event-stream' -d '{"id":"demo.count","params":{"n":2}}'
```

```text
id: 1
event: data
data: {"request_id":"req_05","trace_id":"tr_05","seq":1,"type":"data","data":1}

id: 2
event: data
data: {"request_id":"req_05","trace_id":"tr_05","seq":2,"type":"data","data":2}

id: 3
event: result
data: {"request_id":"req_05","trace_id":"tr_05","seq":3,"type":"result","result":2}
```

Expected: REST returns HTTP 200 Completion with result 5; SSE streams two data events and one terminal result on the same listener. Polling (S134), WebSocket (S135) and MCP (S136) are mounted on the same port at the same time, sharing one authenticator, catalog and dispatcher. No surface-specific operations exist.

<a id="s134--poll-a-request-to-its-terminal-event"></a>

### S134 — Poll a request to its terminal event

Stage: **B**. Required authority: the S133 server; same principal for every call.

```sh
curl -sS -X POST http://127.0.0.1:8080/v1/requests -H 'Content-Type: application/json' \
  -d '{"id":"demo.count","params":{"n":2}}'
# 202 {"session_id":"sess_07","request_id":"req_07","catalog_version":"sha256:fixture",
#      "events_url":"/v1/requests/sess_07/events","expires_at":"2026-09-28T12:00:30Z"}
curl -sS 'http://127.0.0.1:8080/v1/requests/sess_07/events?after_seq=0&wait_ms=5000'
# 200 {"session_id":"sess_07","events":[
#        {"request_id":"req_07","trace_id":"tr_07","seq":1,"type":"data","data":1},
#        {"request_id":"req_07","trace_id":"tr_07","seq":2,"type":"data","data":2}],
#      "last_seq":2,"terminal":false}
curl -sS 'http://127.0.0.1:8080/v1/requests/sess_07/events?after_seq=2&wait_ms=5000'
# 200 {"session_id":"sess_07","events":[
#        {"request_id":"req_07","trace_id":"tr_07","seq":3,"type":"result","result":2}],
#      "last_seq":3,"terminal":true}
```

```text
 POST /v1/requests --202 receipt--> GET events?after_seq=0 (long-poll up to wait_ms)
                                        | data 1, data 2
                                        v
                                    GET events?after_seq=2 --> result (terminal:true) --> stop
```

Expected: the routes are the HTTP projection of `rivet.sessions.open/read`; `after_seq` acknowledges delivered events and `wait_ms` long-polls (capped at 5s). A unary operation submitted the same way returns a batch with one terminal result event. For `demo.relay`, POST `/v1/requests/sess_ID/input` with `{"send_seq":1,"data":"hi"}`, then `/finish_input`; POST `/cancel` returns `{"session_id","request_id","state":"cancelled"}` after cleanup. Sessions survive client reconnects until idle/total expiry.

<a id="s135--multiplex-requests-over-one-websocket"></a>

### S135 — Multiplex requests over one WebSocket

Stage: **B**. Required authority: the S133 server. Any WebSocket client works; `websocat` is shown.

```sh
websocat --protocol rivet.v1 ws://127.0.0.1:8080/v1/ws
```

```text
client -> {"type":"request","ref":"c1","id":"demo.count","params":{"n":2}}
client -> {"type":"request","ref":"c2","id":"demo.add","params":{"a":2,"b":3}}
client -> {"type":"request","ref":"c3","id":"demo.relay","params":{}}
server <- {"type":"data","ref":"c1","seq":1,"data":1}
server <- {"type":"result","ref":"c2","completion":{"request_id":"req_09","trace_id":"tr_09","result":5,"data_count":0,"effects":"none"}}
client -> {"type":"input","ref":"c3","seq":1,"data":"hi"}
server <- {"type":"data","ref":"c1","seq":2,"data":2}
server <- {"type":"data","ref":"c3","seq":1,"data":"hi"}
client -> {"type":"finish_input","ref":"c3"}
server <- {"type":"result","ref":"c1","completion":{"request_id":"req_08","trace_id":"tr_08","result":2,"data_count":2,"effects":"none"}}
server <- {"type":"result","ref":"c3","completion":{"request_id":"req_10","trace_id":"tr_10","result":1,"data_count":1,"effects":"none"}}
client -> {"type":"request","ref":"c4","id":"demo.missing","params":{}}
server <- {"type":"error","ref":"c4","error":{"kind":"not_found","message":"unknown operation"}}
```

```text
 one socket
   ├── ref c1 demo.count  data,data,result   (16-frame queue)
   ├── ref c2 demo.add    result
   ├── ref c3 demo.relay  input -> data, finish_input -> result
   └── ref c4 unknown     error
 close socket -> cancel + join every open ref
```

Expected: frames for different refs interleave; each ref ends with exactly one terminal `result` or `error` frame. `{"type":"cancel","ref":"c1"}` ends that ref with an `error` frame of kind `cancelled`. A ninth concurrent ref gets an `error` frame of kind `limit` (429 semantics). Closing the socket cancels and joins every open ref; unlike polling sessions, WS refs are connection-owned.

<a id="s136--call-the-same-server-through-mcp-at-mcp"></a>

### S136 — Call the same server through MCP at /mcp

Stage: **B**. Required authority: the S133 server. After the proposal's initialize/initialized exchange, use the returned MCP session ID (`demo-session` is illustrative).

```sh
curl -sS http://127.0.0.1:8080/mcp \
  -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' \
  -H 'MCP-Session-Id: demo-session' -H 'MCP-Protocol-Version: 2025-11-25' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
curl -sS http://127.0.0.1:8080/mcp \
  -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' \
  -H 'MCP-Session-Id: demo-session' -H 'MCP-Protocol-Version: 2025-11-25' \
  -d '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"demo.count","arguments":{"n":2}}}'
```

Expected: `tools/list` returns one direct tool per public operation (`demo.add`, `demo.count`, `demo.relay`, …) plus the built-in `rivet.request`, `rivet.list`, `rivet.describe`, `rivet.outputs` and `rivet.sessions.*`; because the S133 server runs on loopback with auth none (principal `local`), `rivet.io` and `rivet.policy.generate` are listed too (a remote principal sees them only when named explicitly, S151). Calling the streaming `demo.count` directly returns a SessionReceipt (tool `_meta: {"rivet/delivery":"session"}`); the events are then read with `rivet.sessions.read` or, for the same principal, `GET /v1/requests/{session_id}/events` (S134). A unary tool such as `demo.add` returns Completion in `structuredContent`.

<a id="s137--disable-surfaces-in-policyjson"></a>

### S137 — Disable surfaces in policy.json

Stage: **B**. Required authority: listener bootstrap.

`policy.json` (beside `catalog.rivet`):

```json
{
  "version": 1,
  "serve": {"surfaces": ["http", "mcp"]}
}
```

```sh
rivet --file catalog.rivet serve
curl -sS -o /dev/null -w '%{http_code}\n' -X POST http://127.0.0.1:8080/v1/requests \
  -H 'Content-Type: application/json' -d '{"id":"demo.add","params":{"a":2,"b":3}}'
curl -sS -o /dev/null -w '%{http_code}\n' -N http://127.0.0.1:8080/v1/request \
  -H 'Content-Type: application/json' -H 'Accept: text/event-stream' -d '{"id":"demo.count","params":{}}'
```

```text
 surfaces: http ✔   sse ✘   poll ✘   ws ✘   mcp ✔
```

Expected: both curls print `404` (poll and sse are disabled); `GET /v1/ws` is also 404. REST and `/mcp` keep working. Omitting `surfaces` mounts all five. Unknown surface names fail policy loading with `policy.invalid`, exit 2.

<a id="s138--fail-fast-by-default-and-report-node-statuses"></a>

### S138 — Fail fast by default and report node statuses

Stage: **A**. Required authority: users.get effects. The fixture makes users.get slower than fixture.fail.

```rivet
operation report.build
    description "Fetch a user and orders, then assemble a summary."
    param id integer required min 1 description "User ID."
    output json description "Summary object."
    dag limit 2 timeout "10s"
        node user = (request "users.get" {id: id})
        node orders = (request "fixture.fail" {})
        node enrich after [user] = (request "fixture.echo" {value: user.result})
        node summary after [user, orders] = (request "summary.make" {user: user.result, orders: orders.result})
    end
    return summary.result
end
```

```sh
rivet --file app.rivet request report.build --params '{"id":42}'
rivet --file app.rivet trace show req_11 --json
```

DagCompletion recorded for the dag (trace view, abridged; `run_dag` returns the same shape to hosts):

```json
{"result": null,
 "nodes": [
   {"id": "user",    "status": "cancelled", "started_at": "…", "ended_at": "…"},
   {"id": "orders",  "status": "failed", "error": {"code": "application.fixture_failure"}, "started_at": "…", "ended_at": "…"},
   {"id": "enrich",  "status": "skipped"},
   {"id": "summary", "status": "blocked"}
 ]}
```

```text
pending --deps ok--> ready --> running --> succeeded
   |                             |------> failed
   |--dep failed/blocked--> blocked       |------> cancelled
   |--fail fast before ready--> skipped
```

Expected: no failure policy is written, so it is `fail fast`. orders fails; user was running and is cancelled; enrich never became ready and is skipped; summary is blocked by its failed dependency. The request fails with the first failure (`application.fixture_failure`, exit 5). Nested requests share `limits.max_concurrent_requests` (default 64) and `max_call_depth` (default 16, `limit.call_depth`); `check` rejects literal `(request "id" …)` cycles with `check.call_cycle`.

<a id="s139--refuse-to-write-through-a-hard-link"></a>

### S139 — Refuse to write through a hard link

Stage: **A**. Required authority: allow_write=./out/**; nothing grants `./data/`.

`policy.json` (beside `app.rivet`):

```json
{
  "version": 1,
  "grants": [{"capability": "allow_write", "targets": ["./out/**"]}]
}
```

```rivet
operation files.touch_report
    description "Overwrite the report file."
    output json description "Update receipt."
    file update "./out/report.json" json {ok: true}
    return {updated: true}
end
```

```sh
ln ./data/secret.json ./out/report.json      # fixture setup outside Rivet
rivet --file app.rivet request files.touch_report --params '{}'
```

```text
./out/report.json  --\
                      >--  inode 81, link count 2  (also ./data/secret.json, not granted)
./data/secret.json --/
```

Expected: `file.hardlink_refused` (kind permission, 403 / exit 3); neither path changes. Write and delete refuse any file whose link count is greater than 1, because a hard link inside `./out/**` could otherwise modify a file outside it. Windows junctions and reparse points are treated as symlinks; selector matching case-folds on case-insensitive volumes and normalizes Unicode to NFC.

<a id="s140--bind-a-secret-to-its-destination"></a>

### S140 — Bind a secret to its destination

Stage: **A**. Required authority: allow_env=EXAMPLE_API_KEY and network grants for both origins; the secret binding still refuses the second.

`policy.json` (beside `app.rivet`):

```json
{
  "version": 1,
  "grants": [
    {"capability": "allow_env",     "targets": ["EXAMPLE_API_KEY"]},
    {"capability": "allow_network", "targets": ["https://api.example.com:443", "https://mirror.example.com:443"]}
  ]
}
```

```rivet
operation account.mirror
    description "Send the API key to a second origin (refused by the secret binding)."
    output json description "Mirror response."
    secret API_KEY from env "EXAMPLE_API_KEY" for "https://api.example.com:443"
    response = http post "https://mirror.example.com/collect"
        header "Authorization" "Bearer ${API_KEY}"
        decode json
    end
    return response.body
end
```

```sh
rivet --file app.rivet request account.mirror --params '{}'
```

Expected: permission.denied, exit 3, before connecting to mirror.example.com: the secret may only reach the origins in its `for` clause, even though policy grants the network origin. S12 shows the permitted use. Taint follows explicit flows (interpolation, object construction, decoding); implicit flows such as `if API_KEY == "x"` are not tracked, so secret handling is best-effort beyond the destination binding.

<a id="s141--show-every-url-and-path-with-its-access-verbs"></a>

### S141 — Show every URL and path with its access verbs

Stage: **A**. Required authority: none after explicit bundle load. [Fixture bundle](#io-manifest-fixture-bundle), public entries.

```sh
rivet --file app.rivet io --by target
```

```text
TARGET                       ACCESS             CAPABILITY     USED BY
https://api.example.com:443  connect GET, POST  allow_network  users.get, users.create
  ├ /users/{id}              connect GET                       users.get (also via users.snapshot)
  └ /users                   connect POST                      users.create
<dynamic: config.url>        connect POST       allow_network  sync.push   (not grantable: review)
./data/endpoint.json         read               allow_read     sync.push
./data/input.json            read               allow_read     report.load
./out/notes/*.json           stat               allow_read     notes.update
                             create, update     allow_write    notes.create, notes.update
                             delete             allow_delete   notes.delete
./out/user.json              create             allow_write    users.snapshot
env API_KEY                  read               allow_env      users.get, users.create
                                                               (secret api_key, bound to https://api.example.com:443)
```

Expected: exit 0. One row per (target, capability): URLs group under their origin `scheme://host:port` with each path template beneath; param_dependent paths show their derived glob (`./out/notes/{name}.json` → `./out/notes/*.json`); env vars appear as `env NAME` with any secret binding. Access verbs are the [vocabulary](#access-verbs) (`file update` contributes `stat` + `update`). The private `archive.purge` is absent because `--all` was not given. The same rows are the `targets` array of `--format json`.

<a id="s142--group-the-manifest-by-capability"></a>

### S142 — Group the manifest by capability

Stage: **A**. Required authority: none after explicit bundle load. Fixture bundle as S141.

```sh
rivet --file app.rivet io --by capability
```

```text
CAPABILITY / TARGET                   ACCESS          USED BY                     KNOWLEDGE
allow_read
  ./data/endpoint.json                read            sync.push                   exact
  ./data/input.json                   read            report.load                 exact
  ./out/notes/*.json                  stat            notes.update                param_dependent
allow_write
  ./out/notes/*.json                  create, update  notes.create, notes.update  param_dependent
  ./out/user.json                     create          users.snapshot              exact
allow_delete
  ./out/notes/*.json                  delete          notes.delete                param_dependent
allow_network
  https://api.example.com/users/{id}  connect GET     users.get                   param_dependent
  https://api.example.com/users       connect POST    users.create                exact
  <dynamic: config.url>               connect POST    sync.push                   dynamic
allow_env
  env API_KEY                         read            users.get, users.create     exact

unused: allow_listen allow_exec allow_pipe allow_unix allow_mcp allow_grpc allow_auth allow_credentials
```

Expected: exit 0. The same sites as S141, grouped under capability headings in [vocabulary](#access-verbs) order, so a reviewer reads the bundle as the list of grants it needs; capabilities with no sites are listed as unused. Under `allow_network` full path templates are shown; the grantable unit is still the origin (S148).

<a id="s143--find-every-deletion"></a>

### S143 — Find every deletion

Stage: **A**. Required authority: none after explicit bundle load. Fixture bundle as S141.

```sh
rivet --file app.rivet io --all --kind file --access delete
rivet --file app.rivet io --all --access create,update,append,delete --by target
```

```text
OPERATION      KIND  ACCESS  TARGET                   KNOWLEDGE        SOURCE
notes.delete   file  delete  ./out/notes/{name}.json  param_dependent  app.rivet:64
archive.purge  file  delete  ./out/archive/old.json   exact            app.rivet:83
```

```text
TARGET                  ACCESS          CAPABILITY    USED BY
./out/archive/old.json  delete          allow_delete  archive.purge (private)
./out/notes/*.json      create, update  allow_write   notes.create, notes.update
                        delete          allow_delete  notes.delete
./out/user.json         create          allow_write   users.snapshot
```

Expected: exit 0 for both. The first command answers "what can delete anything?": two sites, including the private, unreachable `archive.purge` that only `--all` reveals. The second lists every mutating file verb by target. `--access` filters verbs, not operations: the `stat` half of `notes.update` is filtered out. An unknown verb (`--access remove`) is a usage error, exit 2.

<a id="s144--check-the-manifest-against-policyjson"></a>

### S144 — Check the manifest against policy.json

Stage: **A**. Required authority: none; policy.json is read, nothing runs. Fixture bundle as S141.

`policy.json` (beside `app.rivet`):

```json
{
  "version": 1,
  "grants": [
    {"capability": "allow_env",     "targets": ["API_KEY"]},
    {"capability": "allow_network", "targets": ["https://api.example.com:443"]},
    {"capability": "allow_read",    "targets": ["./data/**", "./out/notes/**"]},
    {"capability": "allow_write",   "targets": ["./out/user.json", "./out/notes/2026-*.json"]}
  ]
}
```

```sh
rivet --file app.rivet io --check-policy
echo $?
```

```text
OPERATION       KIND     ACCESS        TARGET                              KNOWLEDGE        SOURCE        DECISION
users.get       env      read          env API_KEY                         exact            app.rivet:5   allowed
users.get       network  connect GET   https://api.example.com/users/{id}  param_dependent  app.rivet:6   allowed
users.create    env      read          env API_KEY                         exact            app.rivet:17  allowed
users.create    network  connect POST  https://api.example.com/users       exact            app.rivet:18  allowed
users.snapshot  (calls users.get — see above)                                               app.rivet:30
users.snapshot  file     create        ./out/user.json                     exact            app.rivet:31  allowed
report.load     file     read          ./data/input.json                   exact            app.rivet:38  allowed
notes.create    file     create        ./out/notes/{name}.json             param_dependent  app.rivet:47  partial
notes.update    file     stat          ./out/notes/{name}.json             param_dependent  app.rivet:56  allowed
notes.update    file     update        ./out/notes/{name}.json             param_dependent  app.rivet:56  partial
notes.delete    file     delete        ./out/notes/{name}.json             param_dependent  app.rivet:64  denied
sync.push       file     read          ./data/endpoint.json                exact            app.rivet:71  allowed
sync.push       network  connect POST  <dynamic: config.url>               dynamic          app.rivet:72  unknown

stderr: 8 allowed · 2 partial · 1 denied · 1 unknown
3
```

```text
 ./out/notes/{name}.json  →  glob ./out/notes/*.json
                          ┌──────────────────────────────┐
   granted allow_write →  │ ./out/notes/2026-*.json  ✔   │  other names ✘   ⇒ partial
                          └──────────────────────────────┘
   no allow_delete grant                                   ⇒ denied
   <dynamic: config.url>  cannot be matched statically     ⇒ unknown
```

Expected: exit 3 because a reachable site is `denied` (notes.delete: no `allow_delete`) or `partial` (notes.create/notes.update: `{name}` may be any segment, but only `2026-*` names are covered). `unknown` alone does not cause exit 3; add `--strict` to fail on it (exit 7). Effective policy is host ceiling ∩ policy.json, so a host ceiling can turn an `allowed` row into `denied`. The manifest is still printed; `--format json` fills each site's `decision` and records `policy.file` and `policy.sha256`.

<a id="s145--export-the-manifest-as-markdown-and-csv"></a>

### S145 — Export the manifest as Markdown and CSV

Stage: **A**. Required authority: none; stdout is redirected by the user's shell. Fixture bundle and policy.json as S144.

```sh
rivet --file app.rivet io --by target --format markdown > io-review.md
rivet --file app.rivet io --check-policy --format csv > io.csv
```

`io-review.md` (paste into a review; the same table S141 prints):

```markdown
| TARGET | ACCESS | CAPABILITY | USED BY |
|---|---|---|---|
| https://api.example.com:443 | connect GET, POST | allow_network | users.get, users.create |
| <dynamic: config.url> | connect POST | allow_network | sync.push |
| ./data/endpoint.json | read | allow_read | sync.push |
| ./data/input.json | read | allow_read | report.load |
| ./out/notes/*.json | stat | allow_read | notes.update |
| ./out/notes/*.json | create, update | allow_write | notes.create, notes.update |
| ./out/notes/*.json | delete | allow_delete | notes.delete |
| ./out/user.json | create | allow_write | users.snapshot |
| env API_KEY | read | allow_env | users.get, users.create |
```

`io.csv` (header plus six of the twelve site rows):

```text
effect_id,operation_id,kind,access,method,protocol,capability,target,glob,knowledge,call_chain,secrets,source,decision
users.get#1,users.get,env,read,,,allow_env,env:API_KEY,,exact,users.snapshot>users.get,api_key,app.rivet:5:5,allowed
users.get#2,users.get,network,connect,GET,http1|http2,allow_network,https://api.example.com/users/{id},,param_dependent,users.snapshot>users.get,api_key,app.rivet:6:5,allowed
notes.update#1,notes.update,file,stat,,,allow_read,./out/notes/{name}.json,./out/notes/*.json,param_dependent,notes.update,,app.rivet:56:5,allowed
notes.update#2,notes.update,file,update,,,allow_write,./out/notes/{name}.json,./out/notes/*.json,param_dependent,notes.update,,app.rivet:56:5,partial
notes.delete#1,notes.delete,file,delete,,,allow_delete,./out/notes/{name}.json,./out/notes/*.json,param_dependent,notes.delete,,app.rivet:64:5,denied
sync.push#2,sync.push,network,connect,POST,http1|http2,allow_network,<dynamic: config.url>,,dynamic,sync.push,,app.rivet:72:5,unknown
```

Expected: the first command exits 0. The second exits 3 exactly like S144, because `--check-policy` decides the exit code whatever the format; the file is still complete. CSV has one row per site (never per target), multi-valued fields joined with `;`, `call_chain` joined with `>`, `source` as `file:line:column`, and RFC 4180 quoting when a value contains a comma or quote. `--by` shapes table and Markdown only; CSV and JSON always carry sites.

<a id="s146--allow-creating-notes-but-never-overwriting-them"></a>

### S146 — Allow creating notes but never overwriting them

Stage: **A**. Required authority: exactly the two narrowed grants below. Fixture bundle as S141.

`policy.json` (beside `app.rivet`):

```json
{
  "version": 1,
  "grants": [
    {"capability": "allow_read",  "targets": ["./out/notes/*.json"], "access": ["stat"]},
    {"capability": "allow_write", "targets": ["./out/notes/*.json"], "access": ["create"]}
  ]
}
```

```sh
rivet --file app.rivet request notes.create --params '{"name":"a","body":"hi"}'
rivet --file app.rivet request notes.update --params '{"name":"a","body":"bye"}'
rivet --file app.rivet io notes.create notes.update --check-policy
```

```text
OPERATION     KIND  ACCESS  TARGET                   KNOWLEDGE        SOURCE        DECISION
notes.create  file  create  ./out/notes/{name}.json  param_dependent  app.rivet:47  allowed
notes.update  file  stat    ./out/notes/{name}.json  param_dependent  app.rivet:56  allowed
notes.update  file  update  ./out/notes/{name}.json  param_dependent  app.rivet:56  denied
```

`notes.update` result (stderr ErrorEnvelope):

```json
{"kind": "permission", "code": "permission.denied",
 "message": "allow_write on ./out/notes/a.json does not permit access update",
 "details": {"effect_id": "notes.update#2", "capability": "allow_write", "access": "update",
             "target": "./out/notes/a.json", "granted_access": ["create"]}}
```

```text
 notes.create a ──create──▶ allow_write ["create"] ✔ ──▶ ./out/notes/a.json written, exit 0
 notes.update a ──stat────▶ allow_read  ["stat"]   ✔
                └─update──▶ allow_write ["create"] ✘ ──▶ permission.denied, exit 3, file unchanged
```

Expected: notes.create succeeds (`{"created":"a"}`, exit 0); notes.update passes its `stat` and is denied at `update` (permission.denied, 403 / exit 3) before any byte is written. The `io --check-policy` run shows the same decisions statically and exits 3. Without `access`, the same grants would allow every verb of allow_read and allow_write (backwards compatible). `deny` entries may also carry `access`, e.g. deny `allow_write` access `["append"]` on `./out/audit.log`.

<a id="s147--reject-an-access-verb-that-does-not-belong-to-its-capability"></a>

### S147 — Reject an access verb that does not belong to its capability

Stage: **A**. Required authority: none (policy load fails first). Fixture bundle as S141.

`policy.json` (beside `app.rivet`):

```json
{
  "version": 1,
  "grants": [{"capability": "allow_read", "targets": ["./data/**"], "access": ["read", "delete"]}]
}
```

```sh
rivet --file app.rivet request report.load --params '{}'
rivet --file app.rivet io --check-policy
```

```json
{"kind": "validation", "code": "policy.invalid",
 "message": "grants[0].access[1]: \"delete\" is not an access verb of allow_read",
 "details": {"path": "grants[0].access[1]", "verb": "delete", "capability": "allow_read",
             "valid": ["read", "list", "stat", "watch"]}}
```

```text
 allow_read  = {read, list, stat, watch}
 "delete"    ∈ allow_delete, ∉ allow_read   ──▶ policy.invalid, exit 2, nothing runs
```

Expected: both commands print the ErrorEnvelope on stderr and exit 2; the bundle's operations never start, not even pure ones, because the whole policy file is rejected. The same applies to verbs in `deny` entries and to unknown verbs (`"wirte"`). To allow deletion, add a separate `allow_delete` grant.

<a id="s148--generate-a-least-privilege-policy-draft"></a>

### S148 — Generate a least-privilege policy draft

Stage: **A**. Required authority: none; nothing runs. Fixture bundle as S141.

```sh
rivet --file app.rivet policy generate > policy.draft.json
echo $?
rivet --file app.rivet policy generate users.snapshot
```

stdout of the first command (`policy.draft.json`):

```json
{
  "version": 1,
  "grants": [
    {"capability": "allow_read",    "targets": ["./data/endpoint.json"],        "access": ["read"]},
    {"capability": "allow_read",    "targets": ["./data/input.json"],           "access": ["read"]},
    {"capability": "allow_read",    "targets": ["./out/notes/*.json"],          "access": ["stat"]},
    {"capability": "allow_write",   "targets": ["./out/notes/*.json"],          "access": ["create", "update"]},
    {"capability": "allow_write",   "targets": ["./out/user.json"],             "access": ["create"]},
    {"capability": "allow_delete",  "targets": ["./out/notes/*.json"]},
    {"capability": "allow_network", "targets": ["https://api.example.com:443"]},
    {"capability": "allow_env",     "targets": ["API_KEY"]}
  ],
  "network": {"deny_private_ranges": true}
}
```

stderr and exit code:

```text
review  sync.push#2  network connect POST  <dynamic: config.url>  app.rivet:72  not granted (dynamic target)
policy generate: 8 grants, 1 review item — draft incomplete
7
```

stdout of the second command (exit 0, nothing to review):

```json
{
  "version": 1,
  "grants": [
    {"capability": "allow_write",   "targets": ["./out/user.json"], "access": ["create"]},
    {"capability": "allow_network", "targets": ["https://api.example.com:443"]},
    {"capability": "allow_env",     "targets": ["API_KEY"]}
  ],
  "network": {"deny_private_ranges": true}
}
```

```text
 manifest site                               draft grant
 ------------------------------------------  --------------------------------------------------
 GET https://api.example.com/users/{id}      allow_network https://api.example.com:443  (origin)
 create ./out/notes/{name}.json              allow_write   ./out/notes/*.json  ["create","update"]
 update ./out/notes/{name}.json              ┘ (merged: same capability + target)
 delete ./out/notes/{name}.json              allow_delete  ./out/notes/*.json  (access omitted:
                                                                                 delete is its only verb)
 POST <dynamic: config.url>                  — not granted → stderr review item, exit 7
```

Expected: one grant per (capability, target), `access` narrowed to exactly the verbs used and omitted when those are the capability's whole verb set (allow_network `connect`, allow_env `read`, allow_delete `delete`). Exact targets are kept, the param_dependent URL becomes its origin and param_dependent paths their glob; `deny_private_ranges` is always true. The dynamic site is not granted: it is a review item on stderr and the exit is 7, but the draft is still written. The draft never contains `serve` auth or secrets. Writing it as `policy.draft.json` keeps it from being auto-discovered; it is a starting point for human review, not approval.

<a id="s149--refuse-to-overwrite-an-existing-policy-file"></a>

### S149 — Refuse to overwrite an existing policy file

Stage: **A**. Required authority: CLI host file creation for `--output` (not an application effect). Fixture bundle as S141; `policy.json` from S144 already exists.

```sh
rivet --file app.rivet policy generate --output policy.json
echo $?
rivet --file app.rivet policy generate --output policy.draft.json
echo $?
```

```text
stderr: {"kind":"conflict","code":"conflict.exists","message":"refusing to overwrite ./policy.json","details":{"path":"./policy.json"}}
4
stderr: review  sync.push#2  network connect POST  <dynamic: config.url>  app.rivet:72  not granted (dynamic target)
7
```

```text
 policy generate --output P
        │
        ├── P exists (file, symlink or directory)? ──yes──▶ conflict.exists, exit 4, nothing written
        │
        └── no ──▶ create P exclusively ──▶ review items? ──yes──▶ exit 7 (P is written)
                                                       └──no───▶ exit 0
 then: human review ──▶ diff policy.json policy.draft.json ──▶ rename by hand
```

Expected: the first run changes nothing and exits 4 (`conflict.exists`, 409 semantics). The second creates `policy.draft.json` exclusively with the S148 draft and exits 7 for the dynamic review item. There is no `--force`: replacing the live policy is a deliberate human rename after review.

<a id="s150--read-the-manifest-over-http"></a>

### S150 — Read the manifest over HTTP

Stage: **B**. Required authority: listener bootstrap. Fixture bundle as S141; `policy.json` = S144's `grants` plus this `serve` block (fixture tokens `test` → ada, `ci-token` → ci, as in S131):

```json
{
  "serve": {
    "auth": {"type": "bearer", "tokens": [
      {"principal": "ada", "sha256": "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"},
      {"principal": "ci",  "sha256": "948b8c2427cd29047839b8e4a27a08763f8befbafa86be5cce8e46217d75e58a"}]},
    "principals": {
      "ada": {"operations": ["*"]},
      "ci":  {"operations": ["users.*", "rivet.io", "rivet.policy.generate"]}
    }
  }
}
```

```sh
rivet --file app.rivet serve --listen 0.0.0.0:8080
curl -sS 'http://127.0.0.1:8080/v1/io?by=target&kind=file&check_policy=true' -H 'Authorization: Bearer ci-token'
curl -sS -o /dev/null -w '%{http_code}\n' 'http://127.0.0.1:8080/v1/io?by=target' -H 'Authorization: Bearer test'
curl -sS -X POST http://127.0.0.1:8080/v1/policy/generate -H 'Authorization: Bearer ci-token' \
  -H 'Content-Type: application/json' -d '{"ids":["users.snapshot"]}'
```

First response, HTTP 200 (abridged):

```json
{"bundle": {"file": "app.rivet", "sha256": "…"},
 "policy": {"file": "policy.json", "sha256": "…"},
 "complete": true,
 "targets": [
   {"target": "./data/endpoint.json", "capability": "allow_read",   "access": ["read"],             "operations": ["sync.push"]},
   {"target": "./data/input.json",    "capability": "allow_read",   "access": ["read"],             "operations": ["report.load"]},
   {"target": "./out/notes/*.json",   "capability": "allow_read",   "access": ["stat"],             "operations": ["notes.update"]},
   {"target": "./out/notes/*.json",   "capability": "allow_write",  "access": ["create", "update"], "operations": ["notes.create", "notes.update"]},
   {"target": "./out/notes/*.json",   "capability": "allow_delete", "access": ["delete"],           "operations": ["notes.delete"]},
   {"target": "./out/user.json",      "capability": "allow_write",  "access": ["create"],           "operations": ["users.snapshot"]}],
 "sites": ["… 7 file sites, decisions as S144: 4 allowed, 2 partial, 1 denied …"]}
```

```text
 ci   GET /v1/io              ──▶ "rivet.io" listed explicitly            ──▶ 200 IoManifest
 ada  GET /v1/io              ──▶ "*" never matches rivet.io              ──▶ 403 permission.denied
 —    GET /v1/io (no token)   ──▶ authentication fails                    ──▶ 401
 ci   POST /v1/policy/generate ─▶ "rivet.policy.generate" listed          ──▶ 200 {policy, review:[], complete:true}
```

Expected: ci gets 200 with the file-only manifest (`complete` describes the filtered sites, so the dynamic network site does not count here); denied/partial decisions are data in the body, not an HTTP error (the CLI's exit 3 has no HTTP analogue). ada's second curl prints `403` even though ada may call every public operation. The generate call returns `{"policy": {…S148 users.snapshot draft…}, "review": [], "complete": true}` and writes no file on the server. `POST /v1/request {"id":"rivet.io","params":{"by":"target"}}` is equivalent and returns a Completion whose `result` is the IoManifest.

<a id="s151--call-rivetio-through-mcp-with-an-explicitly-authorized-principal"></a>

### S151 — Call rivet.io through MCP with an explicitly authorized principal

Stage: **B**. Required authority: the S150 server and an initialized MCP session at `/mcp` per principal (session IDs illustrative).

```json
{"jsonrpc":"2.0","id":30,"method":"tools/call","params":{"name":"rivet.io","arguments":{"by":"capability","kind":"network","check_policy":true}}}
{"jsonrpc":"2.0","id":31,"method":"tools/call","params":{"name":"rivet.policy.generate","arguments":{"all":true}}}
```

ci's `id:30` result, `structuredContent` (abridged Completion):

```json
{"request_id": "req_13", "trace_id": "tr_13", "data_count": 0, "effects": "none",
 "result": {"complete": false,
   "sites": [
     {"effect_id": "users.get#2",    "access": ["connect"], "method": "GET",  "target": {"template": "https://api.example.com/users/{id}"}, "decision": "allowed"},
     {"effect_id": "users.create#2", "access": ["connect"], "method": "POST", "target": {"template": "https://api.example.com/users"},      "decision": "allowed"},
     {"effect_id": "sync.push#2",    "access": ["connect"], "method": "POST", "target": {"template": null, "expression": "config.url"},  "decision": "unknown"}]}}
```

```text
 principal  serve.principals.<name>.operations                 rivet.io / rivet.policy.generate
 ---------  -------------------------------------------------  --------------------------------
 local      (loopback, auth none; also serve --stdio)          callable
 ci         ["users.*", "rivet.io", "rivet.policy.generate"]   callable (listed explicitly)
 ada        ["*"]                                              hidden and refused
 ops        ["rivet.*"]                                        hidden and refused (pattern, not a listing)
```

Expected: for ci, `rivet.io` returns the three network sites with decisions; `rivet.policy.generate` with `all:true` returns the full draft (now including `allow_delete` on `./out/archive/old.json` for the private helper), `review: [sync.push#2]` and `complete:false` — incompleteness is data, not a tool error, and no file is written. For ada, `tools/list` omits both tools and a direct `tools/call` returns `isError:true` with a permission.denied ErrorEnvelope. Only the literal IDs authorize these inventory tools.

<a id="s152--build-the-manifest-and-a-policy-draft-from-a-rust-host"></a>

### S152 — Build the manifest and a policy draft from a Rust host

Stage: **A**. Required authority: none; neither call performs I/O. Fixture bundle and policy.json as S144.

```rust
let rt = Runtime::builder()
    .source(include_str!("app.rivet"))
    .policy(Policy::from_file("policy.json")?)
    .build()?;
let m: IoManifest = rt.io(IoQuery {
    by: IoGrouping::Target,
    kind: Some(IoKind::File),
    check_policy: true,
    ..IoQuery::default()
})?;
for t in &m.targets {
    println!("{:<22} {:<13} {:?} {:?}", t.target, t.capability, t.access, t.operations);
}
assert!(m.sites.iter().any(|s| s.decision == Some(Decision::Denied)));
let draft: PolicyDraft = rt.generate_policy(&["users.snapshot"])?;
assert!(draft.complete && draft.review.is_empty());
std::fs::write("policy.draft.json", serde_json::to_vec_pretty(&draft.policy)?)?;
```

```text
./data/endpoint.json   allow_read    ["read"] ["sync.push"]
./data/input.json      allow_read    ["read"] ["report.load"]
./out/notes/*.json     allow_read    ["stat"] ["notes.update"]
./out/notes/*.json     allow_write   ["create", "update"] ["notes.create", "notes.update"]
./out/notes/*.json     allow_delete  ["delete"] ["notes.delete"]
./out/user.json        allow_write   ["create"] ["users.snapshot"]
```

Expected: the same IoManifest as S150 and the same draft as S148's second command. `IoQuery` mirrors the `rivet.io` params (`ids`, `all`, `by`, `kind`, `access`, `check_policy`); `rt.io` and `rt.generate_policy` are synchronous like `rt.outputs`, because they only read the compiled catalog and the loaded policy. The library never writes the draft; the host chooses to, here with an ordinary `std::fs::write` outside Rivet's broker. `PolicyDraft {policy, review, complete}` matches `rivet.policy.generate`.

<a id="s153--compare-planned-and-actual-io-for-one-request"></a>

### S153 — Compare planned and actual I/O for one request

Stage: **A**. Required authority: users.snapshot's grants (S148 second draft) and trace-store read. Fixture bundle as S141; the fixture returns 404 for `/users/7`.

```sh
rivet --file app.rivet request users.snapshot --params '{"id":7}'     # fails: http.status 404, exit 5 → req_12
rivet --file app.rivet trace show req_12 --json
rivet --file app.rivet io users.snapshot --trace req_12
```

Trace attempts (abridged; every attempt carries its manifest `effect_id`):

```json
{"request_id": "req_12", "attempts": [
  {"effect_id": "users.get#1", "access": "read",    "target": "env:API_KEY",                       "decision": "allowed", "outcome": "ok"},
  {"effect_id": "users.get#2", "access": "connect", "method": "GET",
   "target": "https://api.example.com/users/7", "decision": "allowed", "outcome": "http.status", "status": 404}]}
```

```text
OPERATION       KIND     ACCESS       TARGET                              KNOWLEDGE        SOURCE        ATTEMPTS
users.snapshot  (calls users.get — see above)                                              app.rivet:30  1 call
users.get       env      read         env API_KEY                         exact            app.rivet:5   1 allowed
users.get       network  connect GET  https://api.example.com/users/{id}  param_dependent  app.rivet:6   1 allowed
users.snapshot  file     create       ./out/user.json                     exact            app.rivet:31  0 —
```

```text
 planned (manifest)                         actual (trace req_12)
 users.get#1       env read API_KEY   ───▶  1 attempt  allowed  ok
 users.get#2       GET /users/{id}    ───▶  1 attempt  allowed  /users/7 → 404 → http.status
 users.snapshot#1  create user.json   ───▶  0 attempts            never reached
```

Expected: the join is on `effect_id`, so each planned row shows how many attempts happened and the last policy decision; `0 —` marks planned I/O that did not run (the request failed first with `http.status`, exit 5). A dynamic site would show the concrete target observed at run time in the trace, never in the static manifest. As in S74, `--trace` reads the host's trace store (separate CLI processes need a persisted store); an unknown request ID is not_found, exit 4.

## Additional protocol cases covered by the same grammar

These unnumbered variants complement S01–S153 so the original brief's transport families are explicitly addressed. They follow the same prefix-call, quoted-duration and leading-options rules.

### JSON-RPC over HTTP and scoped transports

```rivet
response = http post "https://api.example.com/rpc"
    body json {jsonrpc: "2.0", id: 1, method: "render", params: {scene: "scene01"}}
    decode json
end
return (rpc.result response.body {expected_id: 1})
```

`rpc.result` is a pure validator: version/id mismatch -> protocol error; error member -> typed rpc failure; missing result and error -> protocol error. The same validator works with newline JSON over Unix/TCP, WebSocket JSON messages, and interactive stdout JSONL. Multiple outstanding requests require a protocol multiplexer with explicit bounded correlation state; do not consume whichever reply arrives first. JSON-RPC notifications have no response and use a separate `rpc.notify` operation contract.

### UDP multicast

```rivet
with udp multicast "239.0.0.1:5000" as packets
    bind "0.0.0.0:5000"
    interface "eth0"
    max_datagram 4096
    timeout "10s"
    for packet in packets
        emit packet
    end
end
return null
```

Stage B; like S83 it binds explicitly and requires `allow_network=udp://239.0.0.1:5000` and `allow_listen=udp://0.0.0.0:5000` for that bind. Interface selection and group join are audited. Delivery remains lossy; expiration cancels and leaves the group through scoped disposal.

### Explicit socket reconnect

```rivet
with websocket "wss://api.example.com/events" as socket
    reconnect 2 backoff exponential max "2s"
    resume none
    for event in socket
        emit event
    end
end
return null
```

Stage C. Reconnect is allowed only for adapters that declare how a new session is surfaced; session changes emit a typed reconnect event. `resume none` does not replay send operations or emitted messages. Authentication/permission failures are terminal. Resumable SSE may use a declared Last-Event-ID policy only when the remote contract defines it; it is not inferred from generic retry behavior.

### Safe binary and custom codecs

```rivet
with tcp "127.0.0.1:9000" as conn
    framing delimiter "\x00" max_frame 65536
    conn.send bytes (base64.decode "AQID")
    return conn.receive bytes
end
```

Stage C. Delimiter framing rejects unescaped delimiter bytes in an outgoing frame; raw framing makes no message-boundary promise. A registered Protobuf codec requires an explicit descriptor supplied in SourceBundle, preserves source/effect identity, and cannot load schemas over the network while decoding.

### Whole-file body and metadata-only HEAD

```rivet
uploaded = http put "https://api.example.com/archive"
    body file "./data/archive.bin"
    decode json
end
info = http head "https://api.example.com/archive"
    decode bytes
end
return {uploaded: uploaded.body, headers: info.headers}
```

Read and network grants required. HEAD has no body; no content auto-decoder may invent a parse error from absent bytes.

## Threat and edge-case checklist for sample implementers

- A missing policy.json must deny DNS, sockets, file metadata, env, processes and undeclared sink writes, not merely the obvious HTTP/file statements.
- `network.deny_private_ranges` (default true) is checked after every DNS resolution and redirect; hostname grants never admit a private address.
- File write/delete refuses targets with link count > 1 (`file.hardlink_refused`).
- Empty input/output objects and empty streams retain explicit completion; unknown fields do not disappear before validation.
- Cleanup on break/return/error/cancel is observable; failed flush is not silently ignored. Already-committed writes are reported on later failures.
- Never transport live resources across request/DAG boundaries. Do not return a scratch path that cleanup immediately deletes.
- Do not call a subprocess sandboxed merely because executable launch was authorized. Enforce descendants or refuse before spawning.
- Remote MCP grants authorize remote operations; they do not confine that server's internal filesystem. Static reports retain opaque boundaries.
- File path checks use held directory capabilities; redirects/DNS and credential forwarding receive fresh authorization checks.
- Timeouts cover body reads, consumer backpressure and retries, with separately bounded cleanup grace. A reconnect is not write replay.
- Pure-looking transformations cannot load schemas, expand environment variables or resolve XML entities through ambient I/O.
- Statically known branch effects remain in the inventory even if the last execution did not visit them.

## Source-to-sample coverage

| Input brief / request | Samples |
|---|---|
| Parameters, variables, typed values, errors | S01, S05, S14, S46, S49–51, S78 and core syntax |
| HTTP methods, encodings, files, streaming, redirects | S07–18 and HTTP variants above |
| WS/TCP/Unix/UDP/pipes and lifecycle | S19–27, S32 plus multicast/reconnect variants |
| CLI/process, stdin/stdout, env, shell refusal | S28–31, S57, S79–80 |
| JSON-RPC and protocol layering | JSON-RPC section and S22–26, S31 |
| File CRUD, watching and ownership | S33–43 |
| Concurrency, polling, loops, multimodal composition, DAG | S44–51; media workflow composes S08/S16/S28 under S44 |
| MCP in/out and bridge | S52–61 |
| Unified request and Rust library | S01–06, S58–60, S70–73 |
| I/O inventory, auditing and policy | S62–69, S74–80, S93, S98, S101–S102 |
| UQ-14 UDP/OAuth/QUIC follow-up | S81–S102; proposal R15–R18 |
| UQ-15 gRPC, multi-operation file and incoming MCP | S103–S120; proposal R19–R22 |
| UQ-17 declared outputs and output viewing | S121–S128, S06, S106, S108; proposal R23 |
| UQ-17 policy.json only | S53, S58, S66–S69, S76–S80, S93, S101–S102, S110, S117–S118, S129–S132, S139–S140; proposal R24 |
| UQ-17 one serve, every surface | S120, S131–S137, S114; proposal R25, R22 |
| UQ-17 review fixes (DAG, errors, secrets, files) | S44–S48, S65, S78, S89, S96, S110, S138–S140 |
| UQ-18 generated I/O manifest (targets, access verbs, policy check, exports, every surface, planned vs actual) | S62–S65, S141–S145, S150–S153; proposal R26, UC-21 |
| UQ-18 access-narrowed policy and least-privilege drafts | S146–S149, S151–S152; proposal R26, UC-22 |

## Verification status

Documentation-only review: 153 unique numbered samples, linked source requirements, and proposed effects/expected behavior per sample. **Runtime validation: not executed; Rivet is not implemented.** Revision 3 of this reference added S103–S120; revision 5 added S121–S140 and converted every block to Capy prefix calls, quoted durations and policy.json; revision 6 rewrites S62–S65 with concrete manifest output and adds S141–S153 (UQ-18), preserving all earlier sample identities. A script check for revision 6 confirmed gapless S01–S153 numbering, index/heading/anchor agreement and balanced code fences. The implementation gate is the Capy spike (every block parses cleanly), then extracting each block into positive/negative fixtures, supplying the declared dependencies and running the proposal's test matrix. Neither copied snippets nor this statement constitute passing tests.

## Related Documents

- [Proposal](../proposals/draft/prop-2026-0001-rivet-runtime.md)
- [Original request and source evidence](ref-2026-0001-request-and-evidence.md)
- [Current state](../../README.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 6 | 2026-09-28 | Claude | UQ-18: added the access-verb vocabulary, optional `access` narrowing in the policy.json schema and an I/O manifest section with a line-numbered fixture bundle; rewrote S62–S65 to show concrete manifest output; added S141–S153 (`io --by target/capability`, deletion search, `--check-policy`, Markdown/CSV export, access narrowing, `policy.invalid`, `policy generate` and its overwrite refusal, HTTP/MCP with explicit principal listing, `rt.io`/`rt.generate_policy`, `io --trace`); added `rivet.io`/`rivet.policy.generate` to the CLI and built-in tool tables; `io` examples use `--format json`. |
| 5 | 2026-09-28 | Claude | UQ-17: converted all DSL to Capy prefix calls and quoted durations; replaced `--sandbox` with policy.json and `serve --transport/--mcp` with one `rivet serve`/`serve --stdio`; added syntax summary, per-resource options, error registry, policy and serve sections; added S121–S140 (declared outputs, policy.json, serve auth and surfaces, DAG completion, hard links, bound secrets); applied review fixes E2–E16 (canonical Rust API, `users.grpc_get`, yield, exit 7, A/B stages). |
| 4 | 2026-09-28 | Codex | Added twelve draft sample folders with source files, fixtures, request bodies and usage READMEs (UQ-16). |
| 3 | 2026-09-28 | Codex | Added gRPC, documented multi-operation catalogs, incoming MCP tools and duplex sessions; expanded reference to 120 examples. |
| 2 | 2026-09-28 | Codex | Added UDP, OAuth 2.0, QUIC/HTTP3 design coverage, traceability and 22 examples; updated current-state navigation. |
| 1 | 2026-09-27 | Codex | Defined proposed usage and 80 numbered examples with effects, outcomes and delivery stages. |
