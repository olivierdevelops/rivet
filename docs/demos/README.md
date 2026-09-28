---
document_id: DEMO-2026-0013
title: "Rivet sample folders"
document_type: demo
status: draft
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 3
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [examples]
affected_versions:
  from: not-applicable
  to: proposed-v0.1
applicable_environments: [development]
audience: [developers, reviewers]
scope: Illustrate the existing proposed interface with sample files; no runtime implementation or release claim.
reason: User requested sample files in folders with READMEs showing usage; UQ-17 (2026-09-28) adds declared outputs, policy.json-only policy and one serve for every surface; UQ-18 (2026-09-28) adds the generated I/O manifest and policy generate.
dependencies: [PROP-2026-0001, REF-2026-0002]
related_documents: [PROP-2026-0001, REF-2026-0002]
supersedes: null
superseded_by: null
tags: [rivet, examples, design]
confidentiality: internal
review_cycle: on-design-change
next_review_date: 2026-10-27
verified_against: not-implemented
---

# Rivet sample folders

These folders contain actual `.rivet` source files, policy files, local JSON/JSONL/text fixtures, request bodies and READMEs for the proposed Rivet interface. **They are design samples, not runnable demos yet.** This repository has no parser/runtime implementation or installed Rivet CLI. No protocol servers are included or contacted.

Start with [01-catalog](01-catalog/README.md). Its single [app.rivet](01-catalog/app.rivet) defines four pure operations, each with a name, description, parameters and a **declared, described output**. One `rivet serve` then exposes them through REST, SSE, polling, WebSocket and MCP at once. [12-library](12-library/README.md) shows the corresponding Rust library interface. Together they demonstrate one operation catalog across every access point.

```text
                       app.rivet  (+ policy.json when effects are needed)
                                  |
        +-------------+-----------+------------+------------------------------+
        |             |                        |                              |
   rivet CLI     rivet outputs ID       rivet serve --listen 127.0.0.1:8080   Rust library
   request/list  (declared outputs,       REST · SSE · poll · WS · MCP        Runtime::builder()
   describe      emits, receives,         one listener, one auth model        rt.request / rt.outputs
                 errors)
        +-------------+-----------+------------+------------------------------+
                                  |
              shared dispatcher · immutable catalog · policy broker
```

## Choose a walkthrough

| Folder | Shows | Policy file(s) | Preparation beyond the future runtime |
|---|---|---|---|
| [01-catalog](01-catalog/README.md) | One file, four operations, every surface from one serve | none (pure) · alt `policies/team.json` (bearer auth) | None; pure operations. websocat for the WS step |
| [02-file-crud](02-file-crud/README.md) | Create, read, update and delete a file | `policy.json` | Create an empty out directory |
| [03-http](03-http/README.md) | HTTP requests, declared errors and typed recovery | `policy.json` | Controlled HTTPS service |
| [04-streaming](04-streaming/README.md) | Streaming data and contextual cleanup | `policy.json` · alt `policies/websocket.json` | Included file; WSS service only for socket.ping |
| [05-dag](05-dag/README.md) | DAG dependencies and partial failures | none (pure) | None; all helpers included |
| [06-mcp-bridge](06-mcp-bridge/README.md) | Bridge a remote MCP connector into local operations | `policy.json` · alt `policies/sync.json` | Controlled MCP service + reviewed schema snapshot |
| [07-oauth2](07-oauth2/README.md) | OAuth 2.0 without returning tokens | `policy.json` | OAuth/resource fixtures + host secret |
| [08-udp](08-udp/README.md) | UDP request and separately authorized reply | `policy.json` · alt `policies/receive.json` | Controlled local datagram peers |
| [09-quic](09-quic/README.md) | QUIC streams and HTTP3 | `policy.json` · alt `policies/http3.json` | QUIC and HTTP3 fixtures with trusted TLS |
| [10-grpc](10-grpc/README.md) | All four gRPC call modes; live input over polling, WS and MCP | `policy.json` | Generate descriptor + provide gRPC fixture |
| [11-sandbox](11-sandbox/README.md) | Deny-by-default policy, the I/O manifest and `policy generate` | `policy.json` · alt `policies/empty.json`, `policies/read-only.json`, `policies/create-only.json` | Included JSON inputs; create out directory |
| [12-library](12-library/README.md) | Embed Rivet as a Rust library | `policy.json` (grant-free; loaded by the sketch) | Proposed embedding API; no Rust package yet |

## Folder conventions

Each folder is independent. `app.rivet` is the full proposed bundle. `README.md` gives setup, exact commands, expected results (including a **View outputs** step), effects, exit codes, inspection and cleanup. Supporting `policy.json`, `policies/`, `requests/`, `data/` and `schemas/` files are linked where used. [manifest.json](manifest.json) lists the public entry operations with their policy files. It is a documentation inventory, not a runtime loading format. Connector imports and built-in operations (`rivet.list`, `rivet.describe`, `rivet.outputs`, `rivet.request`, `rivet.sessions.*`, `rivet.io`, `rivet.policy.generate`) are additional catalog entries where authorized.

**Policy discovery.** Policy is configured only by a JSON file, never by grant strings on the command line.

```text
  --policy PATH given?            -> load PATH (a path only; never grants)
  else ./policy.json beside the   -> load it automatically
       entry .rivet file?
  else                            -> deny-by-default: pure operations run,
                                     every new application effect is denied
```

- Targets in a policy file are relative to **that file's directory**, so `policies/*.json` files use `../data/**` style paths.
- Unknown keys, unknown capabilities or malformed selectors fail loading with `policy.invalid` (exit 2).
- `deny` overrides `grants`.
- `network.deny_private_ranges` defaults to true, so loopback and private addresses need a grant that names the IP literally.
- Folders whose operations are all pure (01-catalog, 05-dag) have **no** policy.json on purpose. 12-library ships a grant-free one only because its Rust sketch loads it.

Run each README's `cd` command once, from the repository root. Do not load all folders together; a few deliberately repeat pure IDs (`demo.*` in 01 and 12) to compare interfaces. The former HTTP/gRPC `users.get` collision is resolved: the gRPC operation is now `users.grpc_get`.

`.example.com` hosts represent controlled fixture services, not reachable demo endpoints. Replace an endpoint and its matching policy selector together. Real HTTP/MCP hosts need `serve.auth` configured in policy.json. A non-loopback `--listen` with auth `none` refuses to start (`serve.auth_required`). Resource OAuth and incoming server authentication are separate.

## The shortest proposed usage

```sh
cd docs/demos/01-catalog
rivet --file app.rivet check --strict-docs
rivet --file app.rivet request demo.add --params '{"a":2,"b":3}'
rivet --file app.rivet outputs demo.add
```

The expected `Completion.result` is `5`. No policy file is needed, because the operation is pure and absence means deny-by-default. `outputs` prints the declared output `integer  Sum of a and b.` The same ID is discoverable through `list` and `describe` and is published as a direct MCP tool.

To expose the same catalog on every surface at once:

```sh
rivet --file app.rivet serve --listen 127.0.0.1:8080     # REST, SSE, polling, WebSocket and MCP
rivet --file app.rivet serve --stdio                     # MCP over stdio only
```

The catalog README walks through every surface: REST, SSE, polling (open → events → terminal), WebSocket frames and MCP (initialize, a direct tool call and `rivet.outputs`). It also covers bearer authentication through an alternate policy file.

## Inspect before invoking

Every README includes these inspection commands:

- `check --strict-docs`. Beyond param descriptions, it requires an output description on every public operation, descriptions on every output/emits/receives field, and an `error` line for every code a body can `fail` with.
- `io --by target`: the **I/O manifest** for that folder's app.rivet, one row per concrete target.
- `io --check-policy`: the same sites evaluated against the folder's policy.json, with a `DECISION` column.
- `outputs ID` and `outputs --all --json` to view declared outputs.

The manifest is generated from the compiled source without performing I/O or evaluating expressions:

```text
  app.rivet ──parse/lower──▶ effect sites ──normalize──▶ IoManifest ──┬─▶ io --by operation|target|capability
                                 │                                    ├─▶ io --check-policy  (◀── policy.json)
                                 │                                    ├─▶ policy generate ──▶ draft policy.json
                                 └──effect_id──▶ runtime trace ───────┴─▶ io --trace REQ (planned vs actual)
```

Each site records a **kind**, one or more **access verbs**, the **capability** that permits them, a normalized **target** and a **knowledge** class:

```text
  kind        access verbs                                   capability
  ----------  ---------------------------------------------  -----------------
  file        read, list, stat, watch                        allow_read
  file        create, update, append                         allow_write
  file        delete                                         allow_delete
  network     connect (+ protocol, + method for HTTP)        allow_network
  network     bind, listen, multicast_join                   allow_listen
  process     exec                                           allow_exec
  env         read                                           allow_env
  pipe        read, write                                    allow_pipe
  unix        connect, listen                                allow_unix
  mcp         call (tool | resource | prompt)                allow_mcp
  grpc        call (unary | server_stream | client_stream | bidi)  allow_grpc
  auth        use, manage, status                            allow_auth
  credential  read, write                                    allow_credentials

  knowledge:  exact | bounded | param_dependent | dynamic | opaque_remote | opaque_native
  decision:   allowed | denied | partial | unknown           (only with --check-policy)
```

| Folder | Sites | `io --check-policy` with the shipped policy.json |
|---|---|---|
| 01-catalog, 05-dag, 12-library | none (pure) | exit 0 |
| 02-file-crud | 6 file sites on `./out/note.json` and `./out` | all allowed, exit 0 |
| 03-http | 3 `connect` sites on `https://api.example.com:443` | all allowed, exit 0 |
| 04-streaming | file read + `wss` connect | `socket.ping` denied, exit 3 (allowed under `policies/websocket.json`) |
| 06-mcp-bridge | transport connect + `call tool crm/tools/search` | allowed, exit 0; opaque_remote so `--strict` exits 7 |
| 07-oauth2 | env, credential, auth and two network sites | all allowed, exit 0 |
| 08-udp | connect, bind, dynamic reply | bind denied, reply unknown, exit 3 |
| 09-quic | quic connect + http3 connect | `items.http3` denied, exit 3 |
| 10-grpc | 4 endpoint connects + 4 calls (one per mode) | all allowed, exit 0 |
| 11-sandbox | 3 file sites | `data.private` denied by design, exit 3 |

[11-sandbox](11-sandbox/README.md) walks through every view (`--by capability`, `--kind file --access delete`, `--format json`), compares `policy generate` output with its hand-written policy.json, and narrows a grant with `"access": ["create"]` in `policies/create-only.json`. policy.json grants and deny entries may carry an optional `access` list; without it, a grant covers every verb of its capability. A verb from another capability is `policy.invalid` (exit 2).

`--include-bootstrap` adds the fixed runtime-internal list under a separate `bootstrap` key: bundle and imports, policy.json, the CA bundle, resolv.conf or the system resolver, tzdata, descriptor/schema files, and stdin/stdout/stderr. Sandbox guarantees apply to script-initiated effects through brokered adapters. `--strict` exits 7 when any site is dynamic or opaque. `--check-policy` exits 3 when any reachable site is denied or partial. Use `policy explain ID --params JSON --json` to see missing grants for one call. Runtime permits are still checked at each actual effect. Runtime traces must be fetched from the same live host or an explicitly configured persistent store.

| Exit | Meaning |
|---|---|
| 0 | ok |
| 2 | syntax / validation / usage / config, including `policy.invalid` |
| 3 | permission (including `file.hardlink_refused`; `io --check-policy` with a denied or partial site) |
| 4 | not_found / conflict (including `policy generate --output` onto an existing file) |
| 5 | dependency / runtime / unsupported / `output.invalid` / application `fail` |
| 6 | timeout |
| 7 | inspection incomplete (`io --strict` with dynamic/opaque sites; `policy generate` with review items) |
| 130 | cancelled |

## Preparation gaps kept explicit

- `06-mcp-bridge` contains authentic-shaped MCP protocol fixtures, not a made-up Rivet snapshot envelope. A reviewed `schemas/crm.json` must be supplied by the future bundle-assembly implementation, and its sha256 must be listed under policy.json `approved.snapshots`. Its serialized format is not yet specified.
- `10-grpc` includes `schemas/users.proto`. The README gives the explicit protoc command to generate `users.pb`; no hidden build process or reflection is permitted during execution.
- `12-library/embedding.rs.txt` is an API sketch. It is intentionally not presented as an available crate or compilable Cargo project.

## Ownership, lifecycle and verification

Owner: project maintainer. All DEMO-2026-0001–0013 documents are **draft**, with no verified runtime version. They stay draft until the proposed parser, adapters and fixture tests validate them. Numbered folders give a reading order, and `README.md` is each folder's entry point. The [demo template](../../DOCUMENTATION.md#1214-demo--demo-templatemd) governs metadata, expected results and verification records. No demos are deprecated or archived.

These are sample assets under docs, not new Rust architecture buckets or runtime implementations. A static script recounted and checked this folder on 2026-09-28 (after the UQ-18 update): 67 files (the 12 sample folders plus this README, index.md and manifest.json), including 12 `.rivet` files with 38 operation/pipeline declarations (35 public, 3 private), 2 connectors and 1 OAuth profile. It checked JSON/JSONL syntax, policy-file keys and `access` verbs against their capability, local links, manifest IDs against declared public IDs, and the absence of removed flags, f-call syntax and bare durations. These checks are not a Capy parse or a Rust/runtime test. No tests or examples were executed against a real server.

## Related documents

- [Current project state](../../README.md)
- [153-example language reference](../references/ref-2026-0002-language-and-usage.md)
- [Runtime proposal](../proposals/draft/prop-2026-0001-rivet-runtime.md)
- [Request and evidence](../references/ref-2026-0001-request-and-evidence.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 3 | 2026-09-28 | Claude | UQ-18/R26: "Inspect before invoking" now describes the generated I/O manifest (`io --by target`, `io --check-policy`), the access-verb table, a per-folder result summary and the 11-sandbox `policy generate`/`access` walkthrough; added policies/create-only.json (67 files); exit-code table notes for io and policy generate. |
| 2 | 2026-09-28 | Claude | UQ-17: recounted files/operations by script (replacing the stale 49/37 claim); walkthrough table lists policy files; policy.json discovery convention replaces `--sandbox`/intersection text; shortest usage uses `outputs` and one `serve`; strict-docs, bootstrap list and exit-code table updated; users.get collision resolved. |
| 1 | 2026-09-28 | Codex | Added twelve independent sample bundles with per-folder usage READMEs and explicit verification limits. |
