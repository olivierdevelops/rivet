---
document_id: PROP-2026-0001
title: "Rivet scoped connection runtime and unified request interface"
document_type: proposal
status: implemented
created_date: 2026-09-27
last_updated: 2026-09-28
document_revision: 9
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [language, execution, cli, http, mcp, library, policy, serve, audit]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development, embedded, server]
audience: [maintainers, developers, reviewers]
scope: Proposed Rivet behavior and design review; no implementation or release claim.
reason: Record the project brief and requested changes as reviewable contracts and examples.
dependencies: [PROJECT.md, DOCUMENTATION.md, AGENTS.md]
related_documents: ["REF-2026-0001", "REF-2026-0002"]
supersedes: null
superseded_by: null
tags: [rivet, rust, capy, design]
confidentiality: internal
review_cycle: on-design-change
next_review_date: 2026-10-27
---

# Rivet scoped connection runtime and unified request interface

## Summary

> **Status banner.** This design was approved on 2026-09-28 (ADR-0001) and **implemented** in v0.1.0
> ([REL-0.1.0](../../releases/rel-0.1.0-release-notes.md)). It stays the design record. The current behaviour is
> described by the [manuals](../../manuals/index.md), [system](../../system/index.md) and [API](../../api/index.md)
> documents; where 0.1.0 differs from a design example, the example's status line in
> [REF-2026-0002](../../references/ref-2026-0002-language-and-usage.md) says what 0.1.0 does.

Rivet is a Rust library and CLI that runs `.rivet` files. A file declares many described operations
(`operation users.get … end`); each operation declares its params, its **output** and the I/O it may do.
Capy parses the files (prefix calls: `(request "users.get" {id: 42})`). One immutable catalog and one
dispatcher serve every access point, and one `rivet serve` listener mounts all network surfaces at once.
Authority comes only from `policy.json`; without one, every new application effect is denied. `rivet io`
generates an **I/O manifest** — every effect site, the URL/path/host it uses and the access verbs it performs
(read, write, delete, connect, exec, …) — and `rivet policy generate` turns it into a least-privilege
`policy.json` draft for review.

```text
             app.rivet  +  policy.json (optional; absent = deny all effects)
                  |              |
          Capy parse -> typed IR -> immutable catalog {id, params, output, emits, receives, errors, effects}
                  |
     +------------+-------------+--------------+-------------+-------------+
     |            |             |              |             |             |
    CLI        REST/SSE      polling       WebSocket        MCP       Rust library
  rivet ...   /v1/request   /v1/requests    /v1/ws         /mcp       Runtime::request
     +------------+-------------+--------------+-------------+-------------+
                  |
     one dispatcher -> scoped execution -> brokered effects (policy check per attempt) -> Completion
```

Scope: HTTP/SSE, files, processes, WebSocket/TCP/Unix, UDP, OAuth 2.0, QUIC/HTTP/3, native gRPC (four
modes), MCP in both directions, live duplex sessions and bounded DAGs are all **required**. Revision 5 adds
declared outputs (R23), policy.json-only configuration (R24), one serve for every surface (R25), and the
review fixes (error registry, DAG semantics, sandbox claim scoping, SSRF defaults, Capy prefix syntax).
Revision 6 adds the generated I/O manifest with per-site targets and access verbs, per-access policy narrowing
and least-privilege policy drafts (R26, [Increment 18](#increment-18--generated-io-manifest-and-policy-drafts)).
Revision 8 (TASK-005, approved in ADR-0001) makes every file-valued option line (`tls ca_file`, `tls cert_file`,
`tls key_file`, `body file`, …) its own manifest site, gives every site `origin`, `phase`, `requires_existing` and
`secret`, and adds `rivet io --needs` (files that must exist before an operation can run) and `--check-files`
([option-derived file sites](#option-derived-file-sites-and-io---needs)).

## Decision Requested

Approve the language and runtime contracts below for implementation in Rust, using Capy for parsing: a
library-first interpreter with an immutable operation registry, declared outputs, explicit protocol
syntax, structured resource scopes, one request protocol projected onto every surface, file-based policy
(with optional per-access narrowing), a generated I/O manifest (targets + access verbs + capability per site),
least-privilege policy drafts for human review, and bounded DAG execution.

Approval of this document does not establish sandbox-platform support, performance results or a shipped
release. Implementation approval additionally requires the gates in [Approval](#approval), including
**G-LIC** (closed 2026-09-28: the maintainer owns Capy and authorized its use, ADR-0001) and human review of the
[hand-authored contract](../../../vhco-contract.json) with `vhco live`, as required by
[AGENTS.md](../../../AGENTS.md).

## Original User Request

The dated, faithful request is preserved as UQ-01–UQ-18 in [REF-2026-0001](../../references/ref-2026-0001-request-and-evidence.md#original-user-request). It asks for scoped closing, good errors, simple syntax, file CRUD, complete I/O visibility, MCP bridging, CLI/HTTP/library parity, callback streaming, Rust/Capy, DAGs, 50+ examples and deny-by-default sandboxing (UQ-01–UQ-13, 2026-09-27). On 2026-09-28 the user added UDP, OAuth 2.0 and QUIC (UQ-14); gRPC, multi-operation files and incoming MCP (UQ-15); sample folders (UQ-16); and (UQ-17):

> operations must be able to define their outputs as well such that we can have commands to view them
> use a po;icy.json file to set policy instead of args
> rivet serve should serve with http, mcp, polling, ws, ... access at at once
> also, fix the gaps you found

UQ-17 supersedes UQ-13's `--sandbox "..."` flag *syntax* while keeping its deny-by-default *behaviour*. The
interpretation is recorded in [REF-2026-0001](../../references/ref-2026-0001-request-and-evidence.md#policy-outputs-and-unified-serve-request).

Later the same day the user added (UQ-18):

> we should have a way to generate all io and what they use
> like url, path
>
> file permissions like read, write, delete, et...

UQ-18 refines UQ-05's audit request: `rivet io` produces a generated I/O manifest (target and access per site,
mapped to capabilities), checkable against `policy.json` and convertible into a least-privilege policy draft.
Interpretation: [REF-2026-0001](../../references/ref-2026-0001-request-and-evidence.md#io-manifest-request).

## Problem and Evidence

### Current User Journey

```text
PROJECT.md protocol examples
    -> manually combine transport, cleanup, output and error conventions
    -> no runnable parser/runtime or common operation registry
    -> cannot verify lifecycle, API parity or actual I/O authority
```

| Problem | Affected users | Evidence | Consequence | Requests |
|---|---|---|---|---|
| P1: explicit cleanup is easy to miss | Integrators | PROJECT §§19, 23, 42, 62, 73 | Early returns and cancellation can leak resources | UQ-02 |
| P2: each protocol exposes a different contract | CLI/API/library developers | PROJECT §§7–70; Go `Function.Execute` | Duplicate validation and inconsistent stream completion | UQ-03, UQ-07–09 |
| P3: authority is disconnected from execution | Operators, library hosts | PROJECT §79 and UQ-13 | Hidden effects in nested calls and processes | UQ-05, UQ-13 |
| P4: composition is informal | Workflow authors | PROJECT §§48–55, 65–70 | Concurrency ordering, ownership and retries remain ambiguous | UQ-06, UQ-11 |
| P5: attractive examples are not a language contract | Reviewers | PROJECT uses implicit resources, triple-quoted bodies and overloaded response fields | Cannot judge implementation feasibility or compatibility | UQ-01, UQ-10, UQ-12 |
| P6: results are opaque, authority lives in argv, surfaces are served separately | Clients, operators | Revision 4 `output json` without schema; `--sandbox "allow_read=…"` strings; `serve --transport …` one surface per flag | Callers cannot discover result shapes; policy is unreviewable shell text; polling/WS clients need a second server | UQ-17 |
| P7: the inventory lists sites but not what they touch or how | Operators, reviewers | Revision 5 `io` rows carry capability and knowledge class only; no access verb, no per-target view, no policy check | Reviewers cannot see which URLs/paths are read, written or deleted, and hand-write policy.json by guesswork | UQ-18 |

## Goals and Non-Goals

| ID | Goal and observable outcome | Problems | Success signal |
|---|---|---|---|
| G-01 | Author one operation and invoke the same schema and behavior everywhere | P2, P5 | Surface parity tests return equivalent values/errors |
| G-02 | Resource scope determines lifetime and errors preserve causes | P1, P2 | Early-exit, cancellation and cleanup fault tests have no live owned children |
| G-03 | Enumerate effects before execution and authorize every actual effect | P3, P7 | Static inventory explains sites with target and access verbs; deny tests observe zero prohibited effects |
| G-04 | Compose streams, connectors and bounded DAGs predictably | P4 | Dependencies, backpressure and partial failures are testable |
| G-05 | Keep the design practical and reviewable | P5 | Capy feasibility gate, source traceability and 159 worked samples |
| G-06 | Make results, authority and access self-describing | P6, P7 | `rivet outputs` shows every declared output; one reviewed `policy.json` explains all grants; one `serve` answers on every surface; `rivet io --by target` shows every URL/path and its access |

Non-goals: a general-purpose language, distributed workflow durability, exactly-once side effects, arbitrary native-plugin isolation inside the embedding process, automatic rollback of external effects, a GUI, transparent replay of remote mutations, unrestricted shell execution, cross-process FFI bindings and a package marketplace. Named pipes, file watching and advanced TCP TLS options are Stage C (see [Increment 8](#increment-8--protocol-coverage-and-availability)); every protocol named in UQ-14/UQ-15 is required.

## Proposed User Journey

```text
BEFORE (revision 4)                                   AFTER (revision 5)
write operation (output json)                         write operation (output object ... end, described)
  -> check -> io                                        -> check --strict-docs -> io -> rivet outputs ID
  -> --sandbox "allow_read=./d/**,..." on argv          -> edit policy.json next to app.rivet
  -> serve --transport http --mcp                        -> policy explain ID
     (+ separate process for stdio / other surfaces)    -> serve  (REST+SSE+poll+WS+MCP on one listener)

write operation -> check -> io --by target -> policy generate -> review/rename policy.json -> io --check-policy
                                                                                               |
write operation -> check -> inspect effects/outputs -> write policy.json -> request ID <-------+
                                              |                |
                                              |                +-> validate -> run scoped work
                                              |                                  |
                                              +-> deny before I/O                +-> data* -> output check -> result/error
                                                                                 +-> cleanup -> trace
CLI / REST / SSE / poll / WS / MCP / Rust --> one registry + dispatcher -----------+
```

## Requirements

Every source below is outside this proposal. `UQ` references resolve to [the request record](../../references/ref-2026-0001-request-and-evidence.md). `PROJECT` refers to [PROJECT.md](../../../PROJECT.md).

| ID | Requirement | Type | Source and relevance | Acceptance criteria | Goals |
|---|---|---|---|---|---|
| R1 | Parse Rivet with Capy in a Rust library | Implementation | UQ-09, UQ-10 | No external Capy executable; clean parse required; span-preserving IR | G-01, G-05 |
| R2 | Simple protocol-visible syntax and explicit schemas | UX | UQ-03; PROJECT §§1, 5, 82 | Common HTTP/file cases remain short; errors point to exact source | G-01, G-05 |
| R3 | Close all owned resources through context exit | Lifecycle | UQ-02; PROJECT §§73–75 | No public close call needed; success/error/break/cancel await cleanup | G-02 |
| R4 | Structured, consistent error, timeout and retry semantics | Reliability | UQ-03; PROJECT §§57–62, 74–78 | Typed causes; one terminal event; unsafe retries rejected | G-02 |
| R5 | Create, read, update and delete files | Functional | UQ-04; PROJECT §§63–64 | Explicit create/update/delete semantics, size/version guards, confined paths | G-03 |
| R6 | List every potential I/O site and trace actual attempts | Operations | UQ-05; UQ-18 refines the output | Static direct/transitive inventory, unknowns visible, runtime source-linked events; inventory output is the IoManifest contract of [Increment 18](#increment-18--generated-io-manifest-and-policy-drafts) (R26) and trace attempts carry `effect_id` | G-03 |
| R7 | Connect to MCP and expose/bridge through MCP | Integration | UQ-06 | stdio and Streamable HTTP; tools/resources/prompts; no implicit permission escalation | G-01, G-04 |
| R8 | Same registered operations over CLI, HTTP and library | API | UQ-07, UQ-09 | Same IDs, schemas, authorization and outcomes; no surface-only business logic | G-01 |
| R9 | Request by ID with params and an optional data callback (UQ-08's `request(ID, PARAMS, on_data)`), plus pull-stream consumption | API | UQ-08; Go `Function.Execute` | Awaited callback/pull demand; callback stop cancels; explicit final result | G-01, G-02 |
| R10 | Bounded DAG composition | Functional | UQ-11; PROJECT §§52–55 | Cycles rejected; explicit edges; bounded concurrency; stable node status | G-04 |
| R11 | Deny-by-default sandbox denies all ungranted I/O | Security | UQ-13 (behaviour); UQ-17 (file syntax); PROJECT §79 | Absent or empty policy denies every new application effect; nested/caller policy cannot widen grants; unsupported process enforcement fails closed | G-03 |
| R12 | Cover original protocol families and advanced flows | Compatibility | UQ-01, UQ-12; PROJECT §§7–70, 83 | Explicit availability matrix and samples for every family; unsupported stages refuse | G-04, G-05 |
| R13 | Protect secrets and avoid implicit shell | Security | PROJECT §§6, 79–80 | Redacted errors/events, argv-only commands, no ambient env inheritance | G-03 |
| R14 | Proposal, contract, examples and docs trace to evidence | Documentation | UQ-12; AGENTS golden rules 1, 9, 10; DOCUMENTATION §§4.2, 12.1, 21 | 50+ numbered samples, matrices, future tests and honest proposed status | G-05 |
| R15 | UDP unicast, explicit bind and multicast are required scope | Functional | [UQ-14](../../references/ref-2026-0001-request-and-evidence.md#follow-up-request); PROJECT §§28–30 | Preserve datagram boundaries, bound sizes, report peer/timeout/truncation and authorize binds; scoped cleanup | G-02, G-03, G-04 |
| R16 | OAuth 2.0 authorization and token lifecycle | Authentication | [UQ-14](../../references/ref-2026-0001-request-and-evidence.md#follow-up-request) | Client credentials, code+PKCE and device flow; isolated secure storage, coordinated refresh and typed failures; no public token result | G-01, G-02, G-03 |
| R17 | Native QUIC with scoped streams and optional datagrams | Functional | [UQ-14](../../references/ref-2026-0001-request-and-evidence.md#follow-up-request) | QUIC v1, TLS/ALPN checks, uni/bidi streams, cancellation isolation, explicit migration and datagram negotiation | G-02, G-03, G-04 |
| R18 | HTTP/3 through the existing HTTP and request interfaces | API | [UQ-14 context](../../references/ref-2026-0001-request-and-evidence.md#follow-up-request), which followed the QUIC/HTTP/3 discussion | `version 3` enforces HTTP/3; explicit fallback policy; credentials/policy/trace preserved; no unsafe replay | G-01, G-03 |
| R19 | Full native gRPC client support | Functional | [UQ-15](../../references/ref-2026-0001-request-and-evidence.md#grpc-and-operation-catalog-request) | Unary, server/client/bidirectional streaming; pinned descriptors, metadata/trailers/status, auth and scoped cleanup | G-01, G-02, G-04 |
| R20 | Many operations in one file with name, params and descriptions | Language / UX | [UQ-15](../../references/ref-2026-0001-request-and-evidence.md#grpc-and-operation-catalog-request) | One load registers every public operation; stable ID, display name, operation/parameter descriptions and schemas survive all catalog projections; duplicate IDs fail atomically | G-01, G-05 |
| R21 | MCP is an incoming access point for the same catalog | API | [UQ-15](../../references/ref-2026-0001-request-and-evidence.md#grpc-and-operation-catalog-request) | MCP tools/list and direct tools/call for authorized public operations; same IDs/validation as CLI, HTTP and library; stdio and Streamable HTTP | G-01, G-03 |
| R22 | All access points can drive live streaming input/output | API / lifecycle | UQ-15 same-operation access; UQ-17 ("serve with … polling, ws"); gRPC modes require this extension to UQ-08 | Bounded principal-owned session operations or scoped library duplex; ordered input, half-close, cancellation and idle cleanup; no fake generic MCP streaming | G-01, G-02, G-04 |
| R23 | Declared, described outputs viewable from every surface | API / Documentation | [UQ-17](../../references/ref-2026-0001-request-and-evidence.md#policy-outputs-and-unified-serve-request) line 1 | `output` scalar or `output object … end` with typed, described fields; `emits`/`receives` use the same field form; optional declared `error` codes; result validated before Completion (`output.invalid`); `rivet outputs`, `describe`, `GET /v1/operations/{id}/outputs`, MCP `rivet.outputs` and `outputSchema`, `Runtime::outputs` all show the same schema | G-01, G-06 |
| R24 | Policy comes only from a policy.json file | Security / CLI | [UQ-17](../../references/ref-2026-0001-request-and-evidence.md#policy-outputs-and-unified-serve-request) line 2; UQ-13 behaviour | `policy.json` beside the entry file is auto-discovered; `--policy PATH` selects another file (path only); no grant strings or env-var policy; absent file = deny-by-default for new application I/O; schema v1 validated, errors exit 2 | G-03, G-06 |
| R25 | `rivet serve` exposes HTTP, SSE, polling, WebSocket and MCP simultaneously on one listener | API / Operations | [UQ-17](../../references/ref-2026-0001-request-and-evidence.md#policy-outputs-and-unified-serve-request) line 3 | One `--listen` address mounts every surface; `--stdio` for MCP stdio; surfaces narrowed only in policy.json; one authenticator/principal model on all surfaces; non-loopback bind without auth refuses to start | G-01, G-06 |
| R26 | Generated I/O manifest with targets and access per site; policy draft from it | Operations / Security | [UQ-18](../../references/ref-2026-0001-request-and-evidence.md#io-manifest-request) | `rivet io` emits an IoManifest: every site's normalized target (URL, path/glob, host:port, argv, env var, connector method), access verbs, HTTP method/protocol, capability, knowledge class and source; views `--by operation\|target\|capability`; formats table/json/markdown/csv; `--check-policy` adds a decision (exit 3 on denied/partial); `--strict` exit 7 on dynamic/opaque; policy.json grants/deny accept optional `access`; `rivet policy generate` writes a least-privilege draft (dynamic/opaque sites listed for review, exit 7); same data from `rivet.io`/`rivet.policy.generate` on every surface; no I/O performed. Revision 8: every file-valued option line (`tls ca_file`/`cert_file`/`key_file` in tcp/http/websocket/quic/grpc/mcp blocks, `body file` upload sources, operation-level `descriptor`) is its own site (file, `read`, `allow_read`); every site carries `origin` (statement or option), `phase` (`load`\|`before_connect`\|`connect`\|`body`\|`cleanup`), `requires_existing` and `secret` (key files: contents secret, path shown); `--by target` shows ORIGIN, PHASE, NEEDS FILE; `io --needs` lists per operation the files that must already exist (excluding files it creates earlier), static, also `rivet.io {needs}` / `GET /v1/io?needs=true`; `io --check-files` stats them through the broker (needs `allow_read` + `stat`), reports present/missing/unreadable/not_permitted, exit 4 on a missing file, exit 3 when stat is not permitted; `policy generate` grants option-derived paths exactly with `access: ["read"]`, never a directory glob | G-03, G-06 |

## Use Cases

All user-facing operations use the unified request contract described below. The reference gives exact DSL inputs, effects and expected behavior. A registered operation is the unit of exposure; raw handles never become public endpoints. UI contract: not applicable; no UI is proposed.

| ID | Outcome / actor | Trigger and preconditions | Normal journey and output | Error paths | Goals | Requirements | Tests / samples |
|---|---|---|---|---|---|---|---|
| UC-01 | Compile and inspect / author | Source bundle, pinned grammar; offline | Parse → type/effect check → immutable catalog | Syntax, unknown option, escaping handle, duplicate ID | G-01, G-05 | R1, R2, R14 | T-01; S01, S62–65 |
| UC-02 | Invoke once / client | Authorized ID and schema-valid params | Resolve → validate → scoped execution → typed result | Unknown ID/input, denied effect, invalid output | G-01, G-02 | R4, R8, R9 | T-02; S02–06 |
| UC-03 | Stream and stop / consumer | Streaming operation, request deadline | Receive ordered data under demand → final result; stop cancels | Callback error, stream decode, output limit, disconnect | G-01, G-02 | R3, R4, R9 | T-03; S14–18, S70–73 |
| UC-04 | File CRUD / application | Authorized confined path | Create exclusive → read → update existing → delete explicit entry | Exists, missing, conflict, symlink escape, unsupported atomicity | G-03 | R5, R11 | T-04; S33–43 |
| UC-05 | Sockets and processes / integrator | Transport options, grants, host support | Enter scope → exchange framed data → join → cleanup | EOF, protocol, deadline, nonzero exit, cleanup | G-02, G-04 | R3, R4, R12, R13 | T-05; S19–32 |
| UC-06 | MCP bridge / connector author | Explicit connector, pinned schema snapshot | Initialize → list/import → map ID → invoke → preserve result | Tool error, protocol error, schema drift, recursive bridge | G-01, G-04 | R7, R11 | T-06; S52–61 |
| UC-07 | DAG / workflow author | Acyclic declared nodes and typed edges | Schedule ready nodes → join → return named outputs | Cycle, failure, blocked dependency, partial side effects | G-04 | R4, R10 | T-07; S44–51 |
| UC-08 | Load policy and enforce it / operator or host | `policy.json` beside the entry file, `--policy PATH`, or none (deny-by-default); optional host ceiling | Load + validate file → host ceiling ∩ policy.json ∩ per-request restriction → check resolved target → allow/deny attempt | Malformed JSON/unknown key/bad selector (`policy.invalid`, exit 2), missing grant, private-range target, unsupported isolation | G-03, G-06 | R11, R13, R24 | T-08, T-21; S66–69, S77–80 |
| UC-09 | Audit / operator | Compiled bundle or authorized trace source | Enumerate all sites as an IoManifest (target, access, capability, knowledge; [Increment 18](#increment-18--generated-io-manifest-and-policy-drafts)), explain transitive chain, inspect attempts joined by `effect_id` | Unknown native/remote effect, missing trace, unauthorized principal | G-03 | R6, R11, R13, R26 | T-09, T-25; S62–65, S74–76 |
| UC-10 | Embed / Rust developer | In-memory source, host runtime and broker | Compile once → restrict per request → await completion | Abandoned future, slow sink, denied native adapter | G-01, G-02 | R1, R3, R8, R9 | T-10; S04, S70–73 |
| UC-11 | Authorize an account / operator or host | Configured profile, caller identity, allowed auth/store/endpoints | Begin code/device challenge → complete → use/refresh bound credentials → status/disconnect | State/issuer mismatch, user denial, expiry, invalid grant, storage race, refresh uncertainty | G-01, G-02, G-03 | R16, R6, R8, R11, R13 | T-11, T-15; S84–S93 |
| UC-12 | Exchange UDP / integrator | Allowed peer or explicit bound receiver and platform multicast support | Scope socket → send/receive whole datagrams → close/leave group | Loss, truncation, unwanted sender, timeout, denied bind | G-02, G-03, G-04 | R15, R3, R11 | T-12, T-15; S27, S81–S83 |
| UC-13 | Use QUIC / integrator | Peer certificate and ALPN, grants, stream budget | Handshake → scoped uni/bidi streams or negotiated datagrams → join/cleanup | TLS/ALPN, reset, flow-control deadline, datagram limit, forbidden migration | G-02, G-03, G-04 | R17, R3, R4, R11 | T-13, T-15; S94–S98 |
| UC-14 | Call HTTP/3 / API client | HTTP operation with explicit version and supported server | Shared auth → QUIC/H3 request → normal response/events | H3 unavailable, unsafe fallback, replay-risk, denied origin | G-01, G-03 | R18, R8, R11 | T-14, T-15; S99–S102 |
| UC-15 | Publish a file of described operations / author | One bundle with multiple operation/pipeline declarations | Parse all → validate IDs/metadata/schema → atomic catalog → inspect per operation | Duplicate IDs, invalid params, reserved name, malformed second operation | G-01, G-05 | R20, R2, R14 | T-16; S103–S106, S109, S119 |
| UC-16 | Invoke gRPC / integrator | Pinned descriptor set, authorized connector/method and mode | Validate protobuf input → scoped RPC → message(s) → final status/trailers | Unknown method, invalid ProtoJSON, wrong mode, late non-OK status, cancelled input | G-01, G-02, G-04 | R19, R3, R4, R11, R13 | T-17; S110–S113, S117–S118 |
| UC-17 | Use operations as MCP tools / MCP client | Initialized authenticated stdio or HTTP MCP session | tools/list → select described tool → tools/call → Completion or bounded SessionReceipt | Hidden/private ID, invalid arguments, schema mismatch, denied operation | G-01, G-03 | R21, R6, R8, R20 | T-18; S107–S108, S116, S120 |
| UC-18 | Send/read live messages from any surface / application | Authorized streaming operation and bounded host session | open → send/read concurrently → finish_input → terminal result; cancel/expire joins cleanup | Sequence conflict, stale cursor, backpressure, cross-principal access, abandoned consumer | G-01, G-02, G-04 | R22, R3, R4, R8, R9 | T-19; S113–S116 |
| UC-19 | Inspect declared outputs / client author or MCP client | Loaded catalog; public operation | `rivet outputs users.get` (table) or `--json` (JSON Schema) → same data from `describe`, `GET /v1/operations/users.get/outputs`, MCP `rivet.outputs`, `Runtime::outputs` | Unknown/private ID 404/exit 4; runtime result mismatches schema → `output.invalid` (500/exit 5); undeclared `fail` code → check warning (error with `--strict-docs`) | G-01, G-06 | R23, R8, R20 | T-20; S103–S106 |
| UC-20 | Serve every surface at once / operator | `rivet --file app.rivet serve [--listen ADDR]`; optional `serve` block in policy.json | One listener → REST, SSE, polling, WebSocket and MCP mounted → one authenticator → principal → operation authorization → shared dispatcher | Non-loopback bind without auth (`serve.auth_required`, exit 2); disabled surface 404; bad token 401; unauthorized operation 403; >8 WS refs/sessions 429 | G-01, G-06 | R25, R8, R21, R22 | T-22; S114, S116 |
| UC-21 | Generate the I/O manifest / operator or reviewer | Loaded bundle; optional policy.json; optional trace | `rivet io [ID …] --by operation\|target\|capability [--kind …] [--access …] [--format …]` → IoManifest (every site: target, access, capability, knowledge, source); `--check-policy` adds `decision`; `--trace REQ` adds `ATTEMPTS`; every site also carries `origin`, `phase`, `requires_existing`, `secret`; `--needs` lists per operation the files that must already exist (static); `--check-files` stats them through the broker; same data from `rivet.io` (`needs?`), `GET /v1/io` (`needs=true`), MCP `rivet.io`, `rt.io` | Unknown ID (404/exit 4); denied/partial reachable site under `--check-policy` (exit 3); dynamic/opaque site under `--strict` (exit 7); invalid `--kind`/`--access` (exit 2); network principal without explicit `rivet.io` authorization (403); `--check-files`: a needed file missing (exit 4), stat not granted by policy.json or file unreadable (exit 3) | G-03, G-06 | R26, R6, R8, R11 | T-25; S62–65, S154–S159 |
| UC-22 | Generate a least-privilege policy draft / operator | Loaded bundle; IoManifest | `rivet policy generate [ID …\|--all] [--output policy.json]` → policy.json v1 with one grant per (capability, target), `access` narrowed to used verbs, `deny_private_ranges: true` → human review → rename into place | dynamic/opaque sites not granted, listed on stderr, exit 7 (draft still written); `--output` onto an existing file → `conflict.exists` exit 4; never emits `serve` auth or secrets | G-03, G-06 | R26, R24, R11 | T-26; S66–69 |

## Project Standards Baseline

| Standards index | Revision | Validated at |
|---|---|---|
| [Standards index](../../standards/index.md) | 2; indexes supplied rules, introduces no new approved policy | 2026-09-27 |
| AGENTS.md | Unversioned supplied file; workflow, golden rules and document-writing sections | 2026-09-27 |
| DOCUMENTATION.md | Document revision 4 | 2026-09-27 |

## Project Validation

| Rule | Applicability | Evidence | Initial result | Follow-up |
|---|---|---|---|---|
| Contract before implementation | Applies | Authored JSON; no runtime code written | PASS | Human review before code |
| Rust vs inherited Go project facts | Explicit user request governs | R1; source record | PASS | Update project-facts block only with implementation baseline |
| Pure use cases and five code buckets | Applies to future code | Architecture and file inventory | PASS | Compiler and I/O adapters injected through ports |
| Requirements/change/file/test traceability | Applies | R/C/F/T tables and alignment | PASS | Convert planned test IDs into executable tests |
| Canonical proposal template | Applies | All authoritative sections present | PASS | CUDA sibling absent; use DOCUMENTATION template |
| Runtime validation and zero drift | Implementation gate | No `src/` or Cargo project yet | NOT APPLICABLE | Cannot claim runtime gates pass in design phase |
| 50+ examples and current-state docs | Applies | Reference S01–S159; README | PASS | Examples are design contracts, not tested demos |
| Design approval | Applies | Approval section | NEEDS HUMAN REVIEW | No approval inferred from drafting request |

## What the Reference Engines Do

### Capy

The inspected public API exposes `Library::new` and a recovering `Library::parse` with diagnostics and source spans. Use that AST as parser output, then lower it into Rivet-owned typed IR; do not execute generated Rust or shell. The grammar belongs in a bundled `.capy` library. Applicable lesson: reuse parsing and diagnostic locations, while retaining runtime authority in Rivet. Evidence: local source (`.ignore/references/capy/rust/src/capy.rs`), embedding guide (`.ignore/references/capy/docs/embedding.md`), [upstream commit](https://github.com/olivierdevelops/capy/tree/84f984c64e0811ef2bfff7835167d6630422ecaa). Capy's native call form is prefix (`(f x y)`), so Rivet adopts it rather than extending the grammar with `f(x, y)` (see [Increment 1](#increment-1--rust-capy-syntax-and-the-compilation-boundary)). Feasibility: **experiment needed** — the spike gate in Increment 1 must parse every sample cleanly. Licence: Capy's LICENSE files are source-available (no bundling, commercial use or derivatives) while its Cargo.toml says MIT; the owner (the project user) will relicense to MIT, tracked as approval gate **G-LIC**.

### AI Manager function dispatcher

`Function.Execute` yields data/error pairs and stops when the consumer declines another frame. Its input filtering can erase unknown fields before validation; its output validator currently returns nil. Applicable lesson: preserve the small dispatch/stream interface, validate before filtering, and make an empty result explicit. Evidence: local reference (`.ignore/references/ai_manager/function.go`). Rivet is an independent Rust design, not a port of that implementation.

### MCP

The referenced transport specification defines stdio and Streamable HTTP. MCP distinguishes transport/protocol errors from unsuccessful tool results and provides separate tools, resources and prompts capabilities. Applicable lesson: build a capability-aware MCP adapter rather than equating MCP with arbitrary JSON-RPC. Evidence: [transport specification](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports), [tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools), [local evidence index](../../references/ref-2026-0001-request-and-evidence.md).

### Tokio structured task ownership

Task sets can collect child completion, but dropping a task owner does not define graceful socket shutdown or subprocess reaping. Applicable lesson: the scope supervisor owns asynchronous cleanup and bounded joins; `Drop` only signals fallback cancellation. Evidence: [JoinSet documentation](https://docs.rs/tokio/latest/tokio/task/struct.JoinSet.html) and [local evidence index](../../references/ref-2026-0001-request-and-evidence.md).

## Proposed Interface and Scope

An `operation` is a public named action. A `pipeline` is an operation with multi-step orchestration. Both have parameters, a final `output`, optional `emits` and `receives`, optional display `name` and `description`, and the same registry identity. A file may contain any number of these declarations; each declaration has its own params, metadata, body, source span and registry entry. This file-level grouping does not change VHCO's rule that each internal Rust use case lives in its own source file. Dotted IDs such as `users.get` are case-sensitive; segments are snake_case identifiers. The header is the callable ID/name; optional `name "Get a user"` is its display label, not a second callable alias. IDs are 1–128 ASCII characters, start each segment with a letter/underscore and reject reserved `rivet.*` application declarations. `rivet.*` is reserved for built-in inspection/administration operations. Helpers and connector declarations are private unless exposed deliberately.

```rivet
operation users.get
    description "Fetch one user by numeric ID."
    param id integer required min 1 description "Stable user ID."
    output object description "The requested user."
        field id integer required description "Stable user ID."
        field name text required description "Display name."
    end
    error "users.not_found" description "No user has this ID."

    response = http get "https://api.example.com/users/${id}"
        decode json
        timeout "10s"
    end
    return response.body
end
```

```rivet
pipeline users.snapshot
    param id integer required min 1
    output json

    user = (request "users.get" {id: id})
    file create "./out/user.json" json user
    return user
end
```

The DSL call is `(request "ID" {params})`; the Rust call is `rt.request(id, params, sink)`; CLI, HTTP, WebSocket and MCP carry the same `{id, params}` pair. Context carries policy, identity, deadline and correlation. The data sink is optional and never confused with the final output. Calling an ID preserves protocol visibility through its declared effect manifest and source navigation.

Default values: request deadline 30s, cleanup grace 5s, stream channel 16 frames, frame/body limit 8 MiB, total buffered collection 32 MiB per request, DAG concurrency 4, retries zero, redirects zero. Host-wide limits (policy.json `limits`): `max_concurrent_requests` 64 across nested calls and DAG nodes, `max_call_depth` 16, `max_buffered_bytes` 256 MiB across all requests and sessions. Larger limits require explicit configuration capped by the host. These are safety defaults, not performance claims. `emit` suspends at downstream demand; a finite operation that emits nothing still returns a final result.

## Complexity: 4/5

| Dimension | Rating and reason |
|---|---|
| Engine reuse | Moderate: Capy parser and established Rust transport crates; Rivet semantics are new |
| New kernels/math | Low: no numerical kernels; scheduler, ownership and policy logic are the hard parts |
| Blast radius | Broad within a new runtime: dispatcher/policy mistakes affect every surface; no shipped Rivet paths exist |
| Existing quality methodology | Partial: repository defines documentation/VHCO gates; fault and sandbox harnesses must be built |
| Unknowns | Capy full grammar fit, platform process confinement, MCP schema churn and cancellation correctness |

Verdict: start with grammar, lifecycle and effect enforcement before adding transport breadth. **Difficulty is not effort**: many adapters are lengthy but routine; a small cancellation or path-resolution mistake can be difficult and affect every call.

## Proposed Changes

| ID | Change | Requirements |
|---|---|---|
| C-01 | Rust library with Capy grammar, typed IR, source maps and immutable registry | R1, R2 |
| C-02 | One dispatcher, input/output schemas and bounded data/completion protocol | R4, R8, R9 |
| C-03 | Context-owned resources, hierarchical cancellation/deadlines and typed errors | R3, R4 |
| C-04 | Brokered file CRUD and stream/file ownership | R5, R11 |
| C-05 | Effect analysis, policy intersection, authorization broker and trace correlation | R6, R11, R13 |
| C-06 | MCP client/server bridge with explicit imported operation IDs | R7, R8 |
| C-07 | DAG scheduling, bounded loops, joins and explicit failure policies | R10, R4 |
| C-08 | Protocol adapters and capability/platform availability matrix | R12, R3, R13 |
| C-09 | Proposal/reference/contract, tests, documentation and promotion gates | R14 |
| C-10 | OAuth profiles, authorization transactions, opaque credential leases and secure refresh/storage | R16, R3, R4, R6, R8, R11, R13 |
| C-11 | Required UDP unicast/bind/multicast driver and datagram result/error contracts | R15, R3, R4, R6, R8, R11 |
| C-12 | QUIC v1 adapter, scoped uni/bidi streams, DATAGRAM negotiation and path policy | R17, R3, R4, R6, R8, R11 |
| C-13 | HTTP/3 version selection and explicit safe fallback through the common HTTP adapter | R18, R4, R6, R8, R11, R13 |
| C-14 | Native HTTP/2 gRPC adapter with descriptor-driven validation and four RPC modes | R19, R3, R4, R6, R8, R11, R13 |
| C-15 | Multi-operation compilation and complete metadata/schema catalog | R20, R2, R8, R14 |
| C-16 | Incoming MCP server publishes direct named tools from the same authorized catalog | R21, R7, R8, R20 |
| C-17 | Scoped duplex and bounded session bridge across CLI/HTTP/MCP/library | R22, R3, R4, R6, R8, R9, R11 |
| C-18 | Declared, described outputs (`output … end`, `field`, `error`), runtime output validation and `rivet outputs` / `/outputs` / `rivet.outputs` / `Runtime::outputs` projections | R23, R2, R8, R20 |
| C-19 | `policy.json` loader (auto-discovery, `--policy PATH`, schema v1, deny-by-default when absent, SSRF defaults, `approved` hashes); `--sandbox` removed | R24, R11, R13 |
| C-20 | Unified `serve`: one listener mounting REST, SSE, polling, WebSocket (`rivet.v1`) and MCP; `--stdio`; policy.json `serve.surfaces/auth/principals`; `--transport`/`--mcp` removed | R25, R8, R21, R22 |
| C-21 | Capy prefix-call syntax, quoted durations, restricted interpolation and component-aware URL encoding; `infra/capy_parser.rs` → Rivet-owned `SyntaxTree`; syntax table | R1, R2 |
| C-22 | Error registry (code → kind → HTTP → exit → retryable), host-wide limits, DAG node semantics and `DagCompletion` | R4, R8, R10 |
| C-23 | Generated IoManifest: access vocabulary per site, target normalization, `rivet io` views/formats/`--check-policy`/`--strict`/`--trace`, `effect_id` on trace attempts, `rivet.io` / `GET /v1/io` / MCP `rivet.io` / `rt.io` projections, explicit-only network exposure; optional `access` narrowing in policy.json grants/deny; revision 8: option-derived file sites, per-site `origin`/`phase`/`requires_existing`/`secret`, `--by target` ORIGIN/PHASE/NEEDS FILE columns, `io --needs` (static) and `io --check-files` (brokered stat; exit 4 missing, exit 3 not permitted), exact-path grants for option-derived files in `policy generate` | R26, R6, R8, R11, R24 |
| C-24 | `rivet policy generate` least-privilege draft (one grant per capability+target, narrowed `access`, origin/glob widening rules, review list and exit 7, no-overwrite `--output`); `rivet.policy.generate` / `POST /v1/policy/generate` / MCP / `rt.generate_policy` | R26, R24, R11 |

## Design

### Increment 1 — Rust, Capy, syntax and the compilation boundary

```text
SourceBundle (caller-provided bytes; compiler does no filesystem I/O)
  -> Capy Library::parse (bundled grammar; no user host callbacks)      <- only Capy API Rivet depends on
  -> ParseResult {tree, diagnostics, line/column spans}
  -> reject non-clean parse (is_clean() == false)
  -> infra/capy_parser.rs: convert Capy tree -> Rivet-owned SyntaxTree   <- the only file touching domain::ast
  -> Rivet typed IR + type checks + ownership + effect graph + output schemas
  -> immutable CompiledProgram {source_hash, grammar_revision, registry, DAGs}
```

**Capy boundary.** Rivet depends only on `Library::parse` and its `ParseResult`. Any access to Capy's
`domain::ast` types is isolated in `src/infra/capy_parser.rs`, which converts to a Rivet-owned `SyntaxTree`;
nothing else imports Capy, so a Capy upgrade touches one file. Spans are line/column (Capy exposes no byte
offsets). Pin Capy to the inspected commit for the spike. Parse and compile are CPU-bound with
source-size/nesting budgets and worker isolation where the host needs cancellation. Loading imports is a
separate brokered read that assembles a SourceBundle; compilation never performs ambient file, network,
environment or process access. Cache only immutable compiled artifacts keyed by source, grammar and connector
schema snapshot hashes. Lowering must not silently accept raw text captures it cannot interpret; do not
introduce a hidden second top-level parser.

**Licence gate G-LIC.** Capy's `LICENSE` is source-available (forbids bundling, commercial use and
derivatives) while its `Cargo.toml` declares MIT. **Closed 2026-09-28:** the owner (the project maintainer)
authorized Rivet to depend on and ship Capy ([ADR-0001](../../decisions/adr-0001-approve-rivet-runtime-design.md));
aligning the upstream `LICENSE` text is the owner's housekeeping.

**Syntax decision: Capy prefix calls.** Revision 4 examples used `f(x, y)` calls, which Capy's value grammar
does not provide. Revision 5 adopts Capy's native prefix form everywhere:

```text
 revision 4 (removed)                                 revision 5 (canonical)
 -------------------------------------------------    ------------------------------------------------------
 request("math.double", {value: a})                   (request "math.double" {value: a})
 request.stream("events.count", {})                   (request.stream "events.count" {})
 rpc.result(response.body, expected_id: 1)            (rpc.result response.body {expected_id: 1})
 f(x, y)                                              (f x y)
 user = request("users.get", {id: id})                user = (request "users.get" {id: id})
 node t after [a, b] = request("math.sum", {...})     node t after [a, b] = (request "math.sum" {...})
 timeout 10s / dag ... timeout 5s                     timeout "10s" / dag limit 2 timeout "5s" fail fast
```

- Named arguments become a trailing object. A call followed by an `allow [...] ... end` block keeps the block
  after the closing paren.
- Durations are quoted strings matching `^[0-9]+(ms|s|m|h)$` (`"100ms"`, `"5s"`, `"1m"`). Bare `10s` lexes as
  NUMBER + IDENT in Capy and is not relied on.
- Arithmetic and comparison stay infix (`a + b`, `value * 2`, `==`) — Capy's native value grammar.
- `${...}` interpolation holds dotted paths only (`${person}`, `${user.name}`); anything else becomes a prior
  assignment. String escapes: `\n \t \" \\ \xNN \uNNNN` only (`"\0"` is written `"\x00"`).
- URL interpolation is component-aware: `${x}` in a path segment is percent-encoded as one segment (`/` →
  `%2F`), in a query value as a query component; it can never add path segments, `?`, `#` or `@` (SSRF defence,
  see Increment 5).
- Four spaces per indentation level, explicit `end`, `#` comments, double-quoted single-line strings and
  backtick multiline text. JSON bodies use object values, not string templating. Field access is dotted,
  indexes use `[n]`; bounds errors are typed. `response.body.status` is payload data; `response.status` is the
  HTTP status. Bytes use tagged base64 envelopes; file results use published artifacts, never leaked paths or
  live handles.
- **Leading-options rule.** Inside a resource block (`with …`, `http …`, `grpc …`) option lines (`timeout`,
  `alpn`, `max_datagram`, `max_streams`, `tls`, `decode`, `framing`, …) precede the first body statement; an
  option after a statement is `syntax.option_after_body`. `tls server_name|ca_file|cert_file|key_file V` is valid
  in `http`, `with http`, `with websocket`, `with quic`, `with tcp` (Stage C) and in `grpc`/`mcp` (http transport)
  connectors; each file-valued option is its own I/O site (Increment 18). The same rule orders an operation header: `name`,
  `description`, `private`, `param*`, `output` (+ block), `emits`, `receives`, `error*`, then the body.

Syntax table (every construct used in the proposal, reference and demos):

| Construct | Form | Notes |
|---|---|---|
| Operation / pipeline | `operation ID … end`, `pipeline ID … end` | Many per file; dotted snake_case IDs |
| Metadata | `name "…"`, `description "…"`, `private true` | Header only; `private true` hides from every surface |
| Parameter | `param NAME TYPE required\|default V [min/max/enum] description "…"` | Closed input schema |
| Output | `output TYPE [description "…"]` or `output object [description "…"] … end` | See [Increment 15](#increment-15--declared-outputs) |
| Field | `field NAME TYPE required\|optional description "…"` (nested `object … end`, `list T`) | In `output`, `emits`, `receives` blocks |
| Declared error | `error "code" description "…"` | Header; undeclared `fail` code warns |
| Stream item types | `emits TYPE`, `receives TYPE` (scalar or field block) | Item schemas |
| Call | `(request "ID" {…})`, `(request.stream "ID" {…})`, `(f x y)` | Prefix form only |
| Assignment | `x = EXPR` | Immutable binding per scope |
| Resource scope | `with KIND … as NAME … end`, `scope … end` | Options lead the body |
| Protocol statement | `http get URL … end`, `grpc CONNECTOR.Method … end`, `file create …` | Options lead the body |
| Emit / yield / return | `emit V` (stream item), `yield V` (value of a `map`/`poll` body), `return V` (exits the whole operation) | `return` inside `map`/`poll` exits the operation, not the block |
| Conditional | `if COND … [else …] end` | Infix comparison |
| Loops | `for x in ITER … end`, `map … end`, `poll … end`, `iterate max N … end`, `while COND … end` | `while` requires a finite deadline; `while true` only in an operation with a finite `timeout` |
| DAG | `dag [limit N] [timeout "D"] [fail fast\|fail independent] … end`, `node N [after [..]] = EXPR` | Default failure policy `fail fast` |
| Concurrency | `concurrent limit N fail fast … task NAME … end … end` | Joined at block exit |
| Errors | `try … catch error kind K … finally … end`, `fail "code" {…}`, `accept status [...]` | Cancellation is not catchable as a generic error |
| Secret | `secret NAME from env "VAR" for "https://host:443"` | Usable only toward the listed origins |
| Connector | `connector NAME mcp\|grpc … end` | Pinned snapshot / descriptor |
| Auth profile | `auth NAME oauth2 … end`; `auth PROFILE account "A"` in a request | See Increment 9 |

**Spike pass gate (feasibility: experiment needed).** Every S01–S159 example in
[REF-2026-0002](../../references/ref-2026-0002-language-and-usage.md) and every `.rivet` file under
[docs/demos](../../demos/README.md) parses cleanly with the pinned Capy commit, and each invalid fixture yields
a diagnostic with a correct line/column. Hard shapes first: `with … as …`, prefix calls with trailing objects
and `allow` blocks, quoted durations, nested `output object` blocks, `try/catch`, DAG references and recovery
from malformed input. Failing the gate revises syntax before any runtime code.

### Increment 2 — Requests, streams and errors

Request lifecycle (one state machine for every surface):

```text
 created --lookup/authz fail--> refused (typed error, no side effects)
    |
    v
 validating --unknown field / schema fail--> refused
    |
    v
 queued --global limit (max_concurrent_requests) wait--> queued
    |
    v
 running --data(1..n)--> running            (emits validated per item; demand-bounded)
    |   \--cancel / deadline--> cancelling
    v                                 |
 cleaning <---------------------------+     (children joined, resources closed, grace 5s)
    |
    v
 output check --mismatch--> error(output.invalid)
    |
    v
 terminal: exactly one of  result(Completion) | error(ErrorEnvelope) | cancelled
```

Reject unknown fields **before** defaults and projection; declared `json` fields can intentionally contain arbitrary JSON. Validate `emits` on every data item and `output` once on final return, before Completion is produced. No implicit dropping of empty objects or unmapped values. Parameters cannot carry callbacks over JSON: Rust supplies a sink; CLI/HTTP/WS consume the same event stream; DSL uses `with (request.stream "ID" {…}) as events`.

Canonical Rust API sketch (the single sketch this proposal and its reference use; proposed, not an existing API):

```rust
let rt = Runtime::builder().source(src).policy(Policy::from_file("policy.json")?).build()?;
let c: Completion = rt.request("demo.add", json!({"a":2,"b":3}), None).await?;
rt.scope(|scope| async move {
    let mut s = scope.stream("events.count", json!({})).await?;   // or scope.duplex(...)
    while let Some(item) = s.next().await? { /* Result<Option<Envelope>> */ }
    Ok(())
}).await?;
let spec: OutputSpec = rt.outputs("demo.add")?;
```

`next()` returns `Result<Option<Envelope>>`; scope closures are `async move` (Rust 2024 async closures). A sink passed as the third `request` argument is an `AsyncDataSink + Send + 'static`; it receives ordered `DataEvent` values one at a time and returns `Continue`, `Stop` or an error. `Stop` cancels and `request` returns `cancelled` after cleanup; sink errors become `consumer_failed`. The runtime owns and joins cleanup when a request future is dropped; `rt.scope` joins everything it started before returning. `Policy::from_json(bytes)` accepts the same schema; a host ceiling intersects it. Nested requests carry `parent_request_id`; W3C `traceparent` is accepted and emitted on HTTP surfaces. `Completion` = `{request_id, trace_id, result, data_count, effects}`. Abrupt process termination cannot promise graceful cleanup.

Event envelope: `{request_id, trace_id, seq, type, data?, result?, error?}`. `seq` starts at 1. Exactly one terminal result/error is generated internally; broken transports cannot guarantee the client receives it. HTTP, CLI and WebSocket use these envelopes in stream mode; unary mode returns a Completion or ErrorEnvelope.

`RivetError`: `kind`, stable `code`, sanitized `message`, `retryable`, `source{file,line,column,end_line,end_column}`, `operation_id`, `request_id`, `trace_id`, `node_id?`, `attempt`, `cause?`, `suppressed[]`, `effects` (`none|committed|partial|unknown`), and bounded redacted `details`. Omit unavailable fields instead of inventing a source location. Raw bodies are opt-in diagnostics subject to redaction and size bounds.

**Error registry.** One table governs every surface. Codes are `kind.detail`; protocol namespaces are codes under existing kinds.

| Kind / code | Meaning | HTTP | CLI exit | Retryable |
|---|---|---|---|---|
| `syntax` (incl. `syntax.option_after_body`, `registry.duplicate_id`, `check.call_cycle`) | Source does not parse/check | 422 | 2 | no |
| `validation` (incl. `validation.required`, `stream.input_required`, `stream.required`, `policy.invalid`, `serve.auth_required`) | Bad params/usage/config, policy.json errors (incl. an `access` verb that does not belong to its capability) | 422 (`serve.auth_required`: startup only) | 2 | no |
| `auth` (caller) | Missing/bad server credentials on a serve surface | 401 | 3 | no |
| `permission` (incl. `permission.denied`, `file.hardlink_refused`) | Policy denies the effect or operation | 403 | 3 | no |
| `not_found` | Unknown ID, file, session | 404 | 4 | no |
| `conflict` (incl. `conflict.input_sequence`, `conflict.exists`, `already_exists`, OAuth lifecycle `auth.login_required` etc.) | State conflict (`conflict.exists`: `policy generate --output` target already exists) | 409 | 4 | no |
| `limit` (incl. `limit.call_depth`, session/ref caps) | Budget exceeded | 429 | 5 | yes, after backoff |
| `timeout` | Deadline exhausted | 504 | 6 | caller decides |
| `connection`, `dns`, `tls`, `protocol` (incl. `udp.truncated`, `quic.*`), `http` (`http.status`, `details.status`), `grpc.<code>`, `application` (incl. `mcp.tool_failed`) | Dependency failure | 502 | 5 | only if replay-safe |
| `unsupported` (incl. `unsupported.sandbox_backend`) | Feature/platform unavailable | 501 | 5 | no |
| `output_invalid` (`output.invalid`) | Result violates declared output; effects preserved | 500 | 5 | no |
| `internal`, `cleanup`, `consumer_failed`, `parse`, `process` | Runtime fault / cleanup failure / sink failure | 500 (`process`, `parse` of upstream data: 502) | 5 | no |
| inspection incomplete | `io --strict` found dynamic/opaque sites; `policy generate` left sites ungranted for review | — | 7 | no |
| `cancelled` | Caller or parent cancelled | 409 before headers; terminal SSE/WS/poll event after | 130 | no |

An upstream HTTP 404 is `http.status` with `details.status: 404` (dependency failure, 502/exit 5) unless the operation maps it with `catch` or `accept status`. gRPC codes map as in Increment 13. Exit 7 replaces the earlier exit 2 for incomplete strict inspection. `io --check-policy` exits 3 (permission) when any reachable site is `denied` or `partial`; it reports, it does not raise an ErrorEnvelope.

Non-2xx HTTP and nonzero process exits fail by default. `accept status [...]` or `accept exit [...]` explicitly broadens success. `catch error kind permission` may recover; it cannot widen authority. `finally` is for business cleanup, never needed to close handles. A primary failure wins; shutdown failures are suppressed on it. If work succeeded but shutdown failed, return `cleanup` with committed-effect status. Cancellation is control flow: generic catches do not swallow it; explicit cancellation handling may observe it but cannot revive the cancelled scope.

Timeout covers DNS, connection, TLS, send, body/decode, downstream backpressure and retries. Children inherit the minimum deadline. Exhaustion triggers cancellation; cleanup uses its separate capped grace period, so caller completion can occur after the operation deadline. Unsafe mutations are never automatically retried. `retry 3` means at most three additional attempts. Retrying a mutation requires a remote-supported idempotency key or an explicit reviewed replay-safe operation declaration. An author assertion alone cannot create exactly-once semantics. Once data has been emitted, automatic retry is forbidden unless a separate resume protocol has been declared and tested. Reconnect creates a new session; it does not replay writes.

### Increment 3 — Context ownership

```rivet
with websocket "wss://api.example.com/events" as socket
    socket.send json {topic: "orders"}
    event = socket.receive json
    return event
end
```

`with` acquires a resource, binds a lexical handle and awaits disposal at the block exit, including `return`, `break`, errors and cancellation. `scope` groups child tasks/resources without acquiring a transport. Handles cannot escape through returns, fields, closures, DAG edges or emitted data. Sending and receiving can run concurrently through split capabilities, but two concurrent receives on one stream are rejected. Parent-owned handles can be borrowed by child tasks that are joined before the parent exits. A request/DAG child inherits scope, identity, policy and deadline.

Resource types: HTTP streaming body, WebSocket/TCP/UDP/Unix/pipe endpoint, file reader/writer, temporary directory, interactive process, MCP session, watcher and request stream. Resource order is reverse acquisition after cancelling and joining children. WebSocket attempts a close handshake, processes close stdin then terminate/kill and reap within grace, file writers finalize/flush with errors visible, and temporary resources are removed unless explicitly published. `Drop` signals supervisor cancellation and releases local ownership; asynchronous cleanup is awaited by the scope, not falsely implemented by `Drop` alone. Already-open resource disposal is a non-transferable cleanup capability: revoked grants block new effects but cannot prevent releasing an acquired resource.

A context is mandatory internally even when examples omit boilerplate. No public `socket.close`, `conn.close`, `process.close`, or reusable handle returned from an operation. There is no hidden detached-task feature. Host application shutdown must await its top-level runtime scope; abort/crash cleanup is outside the graceful guarantee.

### Increment 4 — File CRUD

| Primitive | Semantics | Required grants |
|---|---|---|
| `file create path text/json/bytes value` | Exclusive create; existing target → `already_exists` | write destination |
| `file read path as text/json/bytes` | Bounded read; no implicit MIME transform | read source |
| `file update path …` | Replace existing file; missing → `not_found`; same-filesystem atomic replacement or `unsupported` | write target and staging entry |
| `file write path …` | Explicit upsert; overwrite behavior obvious at call site | write target and staging entry |
| `file append path text value` | Append to existing file; not a multi-writer transaction | write target |
| `file delete path` | Delete one file; missing fails unless `missing ok` | delete target; write alone is insufficient |
| `file stat/list` | Metadata/directory entries are reads | read target |
| `file copy/move` | Explicit source/destination; cross-volume move refuses atomic promise | read source + write dest; move also delete source |
| `with file open … as reader/writer` | Incremental data with scope-owned handle | matching read/write |
| `with file tempdir …` | Scoped scratch storage; no automatic publication | write scratch + delete cleanup authority acquired with creation |

Version guards use a returned opaque `version`, not mtime alone. Conditional update requires an adapter that can enforce the guard against external concurrent writers; otherwise return `unsupported.conditional_update` before writing. An advisory lock is insufficient for an unconditional guarantee. Parent directory creation is explicit. `file update/write/delete` refuse a target whose link count is greater than 1 (`file.hardlink_refused`), so a hard link cannot smuggle a write outside a granted tree. Windows junctions and reparse points are treated as symlinks. Selector matching case-folds on case-insensitive volumes (macOS default APFS, Windows) and normalizes Unicode to NFC before comparison. JSON parse failures report byte/source offsets without dumping secrets. A successful atomic rename is not a promise of crash durability; `durability full` requests data and parent-directory synchronization and fails if unsupported.

`file publish source to destination` is an explicit brokered copy/move out of temporary ownership. Returning a file path does not exempt it from cleanup. Remote consumers receive a bounded bytes envelope or explicit artifact ID with a host-provided authorized artifact store. No auto-created storage service is part of v0.1.

### Increment 5 — Effects, sandbox and audit

```text
compiled effect site -> evaluated target -> host ceiling ∩ policy.json ∩ request restriction -> broker permit
          |                    |                              |                                  |
      source span          real path/IP                 decision event                     adapter attempt
          +--------------------+------------------------------+----------------------------------+
                                  request/trace/node/attempt IDs
```

Static inventories answer **where I/O can happen** — and, since revision 6, **what each site touches and how**: the inventory output format (access verbs, normalized targets, views, JSON `IoManifest`, `--check-policy`, policy drafts) is specified in [Increment 18](#increment-18--generated-io-manifest-and-policy-drafts). Unreachable-from-selected-entry operations are included when `--all` is used. Runtime traces answer **what actually happened**. Every lowered I/O node has an effect ID, operation ID, source span, capability, target expression, branch condition, direct/transitive call chain and knowledge class (`exact|bounded|param_dependent|dynamic|opaque_remote|opaque_native`). `param_dependent` marks targets whose value depends on caller params (for example `https://api.example.com/users/${id}`), so reviewers see where callers steer I/O. Computed IDs require an explicit allow-list or remain dynamic; `io --strict` exits 7 for unknowns. Inspection never performs discovery or evaluates source expressions with side effects.

Capabilities: `allow_read`, `allow_write`, `allow_delete`, `allow_network`, `allow_exec`, `allow_env`, `allow_unix`, `allow_pipe`, `allow_listen`, `allow_mcp`, `allow_grpc`, `allow_auth`, `allow_credentials`. `allow_read` covers metadata/list/watch; `allow_write` covers create/update/append; deletion is separate. `allow_network` covers outbound HTTP/WebSocket/TCP/UDP/QUIC/gRPC with explicit scheme/host/port selectors. `allow_auth` controls profile/account actions; `allow_credentials` controls access to a named credential-store entry. OAuth endpoints, persistence backends and QUIC paths retain their own declared effects. `allow_mcp` authorizes a logical remote operation in addition to its transport. Named pipes and Unix sockets need their own grant, not an ordinary file read. Each capability is made of **access verbs** (for example `allow_write` = `create`, `update`, `append`); a policy.json grant or deny entry may carry an optional `access` list that narrows it to some of those verbs, so "may create files in `./out` but never overwrite" is expressible. The full verb table is in [Increment 18](#increment-18--generated-io-manifest-and-policy-drafts).

Grants are written only in `policy.json` ([Increment 16](#increment-16--policyjson-only-configuration)). There is no command-line grant syntax. Without a policy file every new application effect is denied; pure operations still run.

**What the sandbox guarantees, exactly.** The guarantees apply to **script-initiated effects through brokered adapters**. Every actual attempt rechecks targets: redirects, DNS resolution, proxy destinations, reconnects, imported files, TLS credential reads, process env, temporary files and audit export. HTTP grants do not authorize a redirect to another origin; credential headers are stripped on origin changes. Boundaries use directory-relative handles/no-follow resolution rather than `canonicalize`-then-open checks; unsupported confinement fails closed. Network authorization binds the checked address to the connection and validates redirects and DNS changes; hostname allow-lists alone are not a defense against rebinding.

**SSRF defaults.** `network.deny_private_ranges` is `true` in every mode: RFC 1918, loopback, link-local, `169.254.169.254`, `fc00::/7` and `fe80::/10` are denied — after DNS resolution and on every redirect — unless a grant names that IP or CIDR literally. Combined with component-aware URL interpolation (Increment 1), a caller-supplied param cannot add path segments, change host, or reach metadata endpoints.

**Secrets.** `secret api_key from env "API_KEY" for "https://api.example.com:443"` binds a secret to destination origins; using it toward any other origin is denied. Taint tracking is best-effort: explicit flows (assignment, interpolation, encoding, emit, return) are tracked; implicit flows such as `if secret == x` are not. Returning or emitting a tainted value is an error.

**Runtime-internal (bootstrap) I/O.** These reads/writes happen outside script authority and are listed by `io --include-bootstrap`; they are the fixed, published list:

```text
 bootstrap I/O (not script effects; never grants anything to the script)
 +-----------------------------------+------------------------------------------------+
 | entry bundle + declared imports   | read at load                                    |
 | policy.json (or --policy PATH)    | read at load                                    |
 | CA bundle / system trust store    | read by TLS adapters                            |
 | resolv.conf / system resolver     | read by DNS                                     |
 | tzdata                            | read for timestamps                             |
 | descriptor / schema snapshot files| read at load (gRPC descriptors, MCP snapshots)  |
 | stdin / stdout / stderr           | invocation channels only                        |
 +-----------------------------------+------------------------------------------------+
```

Everything else — log files, token stores, caches, certificate files named by the script, discovery and trace export — is application I/O and needs a grant. Inherited server listener authority is owned by the host; script outbound authority is separate. Listing, `outputs`, `io`, `policy explain` and `policy generate` (to stdout) operate on loaded memory and need no extra grants.

**Enforcement limit.** Arbitrary in-process native Rust code can bypass a broker; untrusted native adapters are rejected under a policy. A child process can perform arbitrary syscalls after launch: sandboxed process/stdio-MCP execution requires a tested OS worker boundary; otherwise fail `unsupported.sandbox_backend` before spawning. Linux/macOS/Windows backends each need their own evidence (feasibility: **blocked** until a backend passes conformance). Remote MCP servers execute elsewhere: `allow_mcp` authorizes the remote operation but does not enforce the server's filesystem; inventories label that boundary opaque.

Audit events include intent, allow/deny, start/end, retry, cancellation and cleanup, with source/program/policy/schema hashes. Redact secret-marked fields, authorization headers, URL userinfo/query secrets, payloads and process env. The trace sink itself is a declared capability; default trace storage is bounded in-memory per runtime. Export needs a write grant. If required auditing fails before an effect, deny it; failures after a committed effect return an explicit audit failure with effect status, never pretend rollback. Ordered/hash-linked records can reveal some changes, but are not tamper-proof without an external trusted sink.

### Increment 6 — MCP in both directions

```text
MCP client -> Rivet MCP server -> shared request dispatcher
                                      |
CLI / HTTP / Rust --------------------+
                                      +-> pinned connector ID -> MCP client adapter -> remote MCP server
```

Connector declarations specify transport, endpoint or absolute argv, schema snapshot and an explicit exposure list. Initially support MCP version `2025-11-25` with negotiation and a declared compatibility range, stdio and Streamable HTTP. Legacy HTTP+SSE is an optional later adapter and fails explicitly when disabled. Initialization, capability negotiation, request correlation, pagination and session management belong to the MCP adapter. No discovery occurs during static compilation. `connectors sync` is an explicit I/O operation; its output is an immutable snapshot, and refreshed schemas never change a running registry silently. A snapshot is **reviewed** when its `sha256` is listed in policy.json `"approved": {"snapshots": ["sha256:…"]}`; loading an unlisted snapshot fails. The same list form (`"overlaps"`) records reviewed DAG write-target overlaps (Increment 7).

Tools map to IDs such as `crm.tools.search`; resources to `crm.resources.read` with URI params; prompts to `crm.prompts.get` with name/arguments. Keep original names/schemas and deterministic alias mappings; reject collisions. Preserve MCP content blocks and `structuredContent`; do not flatten binary content into invented strings. `isError` maps to a typed `mcp.tool_failed` operation failure; protocol faults retain JSON-RPC error identity. Progress/log notifications are distinguished from application data. Generic streaming tool deltas are not assumed to be a standard MCP feature. Explicit tool adapters may convert negotiated content notifications into typed Rivet data.

Incoming Rivet MCP tools use the same operation IDs, input/output checks and policy as other surfaces. **Direct named tools are canonical**: every authorized public operation is its own tool, called as `tools/call {name:"users.get", arguments:{id:42}}`. Built-in generic tools exist alongside them for catalog inspection and session control; private declarations are omitted from both.

Built-in registry (reserved `rivet.*` operations, callable from every surface):

| ID | Params | Result | Purpose |
|---|---|---|---|
| `rivet.list` | `{cursor?, outputs?}` | Paginated summaries | Catalog listing |
| `rivet.describe` | `{id}` | Full descriptor (params, output, emits, receives, errors, effects) | One operation |
| `rivet.outputs` | `{id?: text, all?: boolean}` | OutputSpec JSON Schema(s) | Declared outputs (R23) |
| `rivet.request` | `{id, params}` | Completion or SessionReceipt | Generic dispatch for clients that cannot use named tools |
| `rivet.sessions.open/send/finish_input/read/cancel` | see Increment 14 | Session values | Live input/output |
| `rivet.auth.begin/complete/status/disconnect/cancel` | see Increment 9 | Auth values | OAuth management |
| `rivet.capabilities` | `{}` | Feature/platform matrix | Availability |
| `rivet.io` | `{ids?: [text], all?: boolean, by?: text, kind?: text, access?: [text], check_policy?: boolean, needs?: boolean}` | IoManifest | Generated I/O manifest (R26, Increment 18); network principals only if explicitly listed |
| `rivet.policy.generate` | `{ids?: [text], all?: boolean}` | `{policy: {…}, review: [site…], complete: bool}` — never writes files | Least-privilege policy draft (R26, Increment 18); network principals only if explicitly listed |

Host authorization can narrow the catalog but cannot change the underlying operation contract. Tool results include structured Completion or a safe error result. Remote resource/prompt access is reachable through registry operations even when presented through native MCP resources/prompts as convenience views.

Authentication is configured per connector, never inherited wholesale from an upstream caller. Credential forwarding is opt-in and audience-scoped. Incoming HTTP/MCP servers bind loopback by default, validate Origin for MCP, require configured auth for remote binds and apply per-principal operation authorization. Sampling/elicitation/roots requests are denied unless the host explicitly implements and authorizes them. Detect bridge recursion with bounded hop counts and server/operation identity chains. Cancellation is explicit; an MCP HTTP disconnection alone is not treated as a cancellation notification.

### Increment 7 — DAGs and complex flows

```rivet
pipeline dashboard
    param id integer required
    output json
    dag limit 4 timeout "20s" fail fast
        node user = (request "users.get" {id: id})
        node orders = (request "orders.list" {user_id: id})
        node summary after [user, orders] = (request "summary.make" {user: user.result, orders: orders.result})
    end
    return summary.result
end
```

A node starts only after its declared dependencies succeed. References to another node require a matching `after` dependency; forward ambiguity and cycles are rejected before execution. When the failure policy is omitted it is `fail fast`. Nodes return values, not resource handles. Output order is declaration order; execution timing is nondeterministic.

Node state machine:

```text
  pending --deps ok--> ready --> running --> succeeded
     |                              |------> failed
     |--dep failed/blocked--> blocked       |------> cancelled
     |--fail fast before ready--> skipped
```

- `blocked`: a dependency failed or was blocked; the node never runs with null input.
- `skipped`: `fail fast` cancelled the run before the node became ready.
- `cancelled`: the node was running when the run was cancelled (fail fast, deadline or caller).
- `fail independent` lets unaffected branches finish.

A node reference is an envelope `{status, result, error}`. `.result` of a non-succeeded node is `null`; `check` warns when a `return` reads `.result` without a status guard (`if summary.status == "succeeded"`). `run_dag` returns `DagCompletion {result, nodes:[{id, status, error?, started_at?, ended_at?}]}`, so partial outcomes are visible on every surface.

```text
 DagCompletion
 +-- result            (the pipeline's return value, validated against its output)
 +-- nodes[]
      +-- {id:"user",    status:"succeeded", started_at, ended_at}
      +-- {id:"orders",  status:"failed",    error:{kind:"http", code:"http.status", ...}}
      +-- {id:"summary", status:"blocked"}
```

**Limits.** DAG nodes and nested `(request …)` calls share the host-wide budget `limits.max_concurrent_requests` (default 64); `dag limit N` further caps one DAG. Call depth is limited by `limits.max_call_depth` (default 16, error `limit.call_depth`). `check` detects static cycles over literal `(request "ID" …)` targets (`check.call_cycle`); computed targets are bounded by the depth limit at run time.

Node retry uses the ordinary operation policy; no whole-DAG retry after side effects. No automatic cache of I/O operations, persistent resume or exactly-once claim. Optional compensations are explicit operations, separately authorized, run in reverse completed-dependency order and report their own errors; they are best-effort business repair, not transactions. File write conflicts between concurrently runnable nodes need declared `after` ordering or disjoint targets; an unresolved overlap requires an acceptance whose `sha256` is listed in policy.json `approved.overlaps` (the definition of *reviewed*, Increment 6).

Bounded `map` is finite fan-out with ordered results. `poll` and `iterate max N` are sequential composite nodes; they do not introduce graph cycles. Inside `map`/`poll` bodies, `yield VALUE` produces the block's value; `return` always exits the whole operation. `while` requires a finite request deadline. Transport streams remain inside a node; stream-through edges between nodes are a later design extension. Realtime duplex flows use a scoped concurrent group with split send/receive handles and a common deadline.

### Increment 8 — Protocol coverage and availability

| Family / original scope | Proposed stage | Semantics |
|---|---|---|
| HTTP GET/POST/PUT/PATCH/DELETE/HEAD; query/headers; JSON/text/form/XML/multipart/bytes/file | A | Typed bodies, status separate from payload, bounded decoding |
| HTTP SSE/JSONL/lines/bytes; JSON-RPC HTTP | A | Scoped pull stream, explicit framing/decoding and final result |
| Files, CLI, bounded polling/iteration, DAG, errors, audit/sandbox | A | Common lifecycle and effect checks |
| WebSocket, TCP, Unix, JSON-RPC over those and stdio; MCP client/server | B | Scoped duplex; transport/protocol layered |
| UDP unicast, explicit bind and multicast | B | Whole datagrams, source metadata, explicit listen/network grants and scope cleanup |
| OAuth 2.0 client credentials, code+PKCE, device grant and refresh | B | Principal-bound profiles and transactions, authorized token stores, opaque leases |
| QUIC v1 reliable streams and negotiated DATAGRAM | B | TLS/ALPN, uni/bidi scopes, reset isolation, bounded streams and explicit path policy |
| HTTP/3 | B | Normal HTTP syntax; strict version selection or explicit safe fallback |
| Named pipes/FIFO, file watching, advanced TCP TLS/mTLS options, interactive binary processes | C | Platform capability checks and bounded frames |
| Native gRPC over HTTP/2; four RPC modes; descriptor-driven Protobuf | B | Scoped calls, pinned schemas, metadata/trailers/status, shared OAuth and stream sessions |
| Multi-operation files, descriptions, declared outputs and CLI/HTTP/library catalog | A | Common descriptors and `rivet outputs` |
| Incoming MCP catalog (direct named tools), polling, WebSocket, unified `serve` | B | One listener, every surface |
| Generic non-gRPC protobuf/custom codecs | C | Registered typed codec with no hidden I/O; schema must be supplied |
| Shell escape hatch | Declined initially | `unsupported.shell`; argv-only execution is the alternative |
| Reconnect/resumable delivery | C | Session-specific opt-in; never implicit mutation replay |

Stage labels are exactly `A` (first usable slice) or `B` (the rest of the required scope); Stage B is entirely required, and the proposal is fully implemented only when A and B pass on the declared platform matrix. `C` rows are outside UQ-14/UQ-15/UQ-17 protocol scope and remain later work. Stage A cannot be marketed as full PROJECT coverage. Feature availability is inspectable in `rivet.capabilities`, and unsupported calls fail before acquisition. No platform stub may silently report success.

### Increment 9 — OAuth 2.0 as a reusable authorization layer

Rivet-specific design decisions in this section implement R16. The normative baselines are [OAuth 2.0](https://www.rfc-editor.org/rfc/rfc6749.html), [PKCE](https://www.rfc-editor.org/rfc/rfc7636.html), [OAuth security guidance](https://www.rfc-editor.org/rfc/rfc9700.html), and [device authorization](https://www.rfc-editor.org/rfc/rfc8628.html). Support client credentials, authorization code with PKCE S256, and device authorization. Implicit and resource-owner-password flows are rejected. OAuth supplies access authorization; identity/login assertions from OpenID Connect are not implied.

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

`auth PROFILE account ACCOUNT` is valid on HTTP operations and HTTP MCP connector declarations. An account is an opaque key within the authenticated principal/tenant; supplying its name never grants another user's credentials. Profile names and account-use authority are validated before secret lookup. Profiles are immutable in the program snapshot. Client ID is public configuration; client secret, authorization codes, verifier, device code and token material are secret-marked. `client_auth basic`, `post` and `none` are explicit profile choices (`none` for public code/device clients). Unsupported private-key JWT, DPoP, dynamic registration and provider extensions fail `unsupported.auth_method` until separately implemented; never claim universal OAuth-extension support.

The profile also defines exact issuer/endpoints, registered redirect URI for code flow, optional provider-specific `audience`, allowed scopes and exact resource origins. `audience` is sent only where the provider contract supports it. Discovery is not automatic: configured endpoints are checked and pinned. Separate auth-server and resource-server grants are required. Bearer attachment is restricted to the profile's resource origins and requested scopes; redirects do not forward the credential to a different origin. A malicious source file cannot mint a new authorized profile by copying an existing profile name: policy binds profile identity to its configuration hash. OAuth client authentication proves the Rivet client's identity to the provider; incoming Rivet HTTP authentication remains a separate host responsibility.

```text
client_credentials: no token -> permitted exchange -> cache -> expiry -> new exchange
code+PKCE: begin -> expiring state/verifier -> user authorizes -> complete -> account connected
                               |                              |
                               +-> cancel/expiry              +-> mismatch/denial -> typed failure
device: begin -> user code/verification URI -> bounded token polling -> connected/denied/expired
use: origin+scope+principal check -> cached token OR one coordinated refresh -> opaque adapter lease
```

`rivet.auth.begin {profile,account}` creates a bounded transaction for code/device flow and returns only challenge data. Authorization-code output is `{transaction_id,authorization_url,expires_at}`; device output is `{transaction_id,verification_uri,user_code,expires_at,interval_seconds}`. User-code/authorization URL display is a deliberate authorized response; audit logs omit them. Client credentials need no begin/complete: first authorized use acquires a token. Calling begin on a client-credentials profile returns `validation.auth_flow`.

`rivet.auth.complete {transaction_id,callback? ,wait?}` consumes the initiating principal's transaction. A code callback is `{code,state,redirect_uri,issuer?}`; validate ownership, TTL, exact redirect binding, issuer defense and one-time state **before** exchanging the code with its PKCE verifier. Providers without an issuer response require a dedicated redirect binding per issuer; do not accept an ambiguous multi-issuer callback. Callback codes do not belong in CLI argv/logs; use `--params-file` or an authenticated host call. Browser navigation and redirect collection are host responsibilities; Rivet opens neither a browser nor a callback listener implicitly. A host-owned web callback forwards validated data to the same complete operation under the initiating principal. No new Rivet HTTP route is introduced.

For device flow, complete with `wait:true` polls under the caller deadline. Respect provider `interval`, `authorization_pending`, `slow_down` (increase interval by at least five seconds), denial and expiration; transport failures back off rather than busy-loop. If the request deadline arrives (or the request is cancelled) first, polling stops and complete returns `{"state":"pending","transaction_id":…,"expires_at":…}` **without consuming the transaction**; the caller calls complete again. Only `rivet.auth.cancel {transaction_id}` (CLI `auth cancel`), transaction expiry, a terminal provider answer or `disconnect` invalidates it. No detached poll continues between calls.

OAuth transaction state machine (code and device flows):

```text
                 begin
                   |
                   v
   +-------------> open ----complete(callback ok / token issued)----> consumed -> account connected
   |               |  \
   |  complete     |   \--state/issuer mismatch, access_denied----> failed (typed auth error)
   |  wait:true    |
   |  hit deadline |--auth cancel / disconnect--------------------> cancelled
   +--(pending)----|
                   +--TTL (min(provider expiry, 10 min))----------> expired (auth.transaction_expired)
``` Authorization transaction TTL is the smaller of provider expiry and host maximum (10 minutes default), max 8 outstanding transactions per principal. Transaction state is deliberately stored data under the runtime/store owner, not a live socket leaked beyond a request scope.

A successful complete returns `CredentialStatus {profile,account,state,scopes,expires_at,generation}`. `rivet.auth.status` reads this sanitized state without triggering refresh or network discovery. `rivet.auth.cancel {transaction_id}` invalidates one outstanding transaction and returns `{transaction_id,state:"cancelled"}`. `rivet.auth.disconnect` invalidates outstanding transactions, blocks new leases for that generation and removes local credentials; returns `{profile,account,local_only:true,generation}`. It does not imply provider-side token revocation or cancellation of already-committed remote work. Missing local credentials yield an idempotent disconnected receipt. Remote revocation can be a later explicit operation.

`store memory` is process-local and lost on restart. Multi-step CLI examples use `--endpoint URL` to address one running runtime. Direct CLI invocations need an explicitly configured durable store to share state. `store keychain "rivet/crm"` uses a host-selected secure credential provider with tested access control, versioned updates and locking; no plaintext token-file default. Backend unsupported -> fail before acquisition rather than silently persist insecurely. Store lookup is authorized by an `allow_credentials` target `PROFILE/ACCOUNT` and a manifest identifying the actual backend; a file-based provider additionally declares file effects and key source. Keychain mediation is a trusted named capability, not arbitrary filesystem access granted to the script.

Token cache identity includes principal/tenant, profile hash, account, audience/resource and granted scope set. Every use rechecks authority even if the token is cached. Expiry uses a monotonic deadline derived from provider lifetime with capped skew (at most 30s and never more than half the lifetime). No expiry -> do not reuse across requests without an explicit provider rule. Client credentials reacquire at expiry; user flows refresh only if a refresh token exists. One refresh runs per cache key, with a cross-process lock/versioned commit if the store is shared. Waiters can cancel independently; the refresh belongs to a bounded supervisor task authorized under the same credential identity and remains joined to its owner. One caller's cancellation cannot cancel another caller's independently authorized use.

Refresh-token rotation commits the replacement atomically before use is reported successful. Crash/network uncertainty after token exchange or store failure is `auth.refresh_uncertain`; invalidate unsafe cache state and require reauthorization rather than repeatedly submitting the old refresh token. `invalid_grant` -> `auth.login_required`; insufficient scopes -> `auth.insufficient_scope`; issuer/state mismatch -> `auth.callback_invalid`; denied authorization -> `auth.access_denied`; expired challenge -> `auth.transaction_expired`; token/store service faults -> bounded `auth.token_endpoint_failed` / `auth.store_failed`. Safe token endpoint error codes remain available, raw responses do not. A resource 401 does not trigger unbounded refresh/replay: invalidate the relevant lease; at most one retry for an explicit invalid-token response is allowed only when the original operation's existing replay-safety rules permit it. Auth errors never bypass mutation replay restrictions.

`allow_auth` targets are `PROFILE/ACCOUNT/use|manage|status`, written in policy.json, for example `{"capability":"allow_auth","targets":["crm_user/ada/use"]}`. `manage` permits begin/complete/cancel/disconnect; `status` only status; `use` permits attachment/acquisition/refresh. `{"capability":"allow_credentials","targets":["crm_user/ada"]}` separately delegates named-store access. Wildcards may broaden a principal's own accounts but never cross principal boundaries. Every request also needs relevant endpoint/env/store permits. Audit records show profile/account identifiers (redacted or hashed by host policy), auth phase, endpoint origin, expiry class and credential generation, not tokens/state/codes. `io --transitive` includes possible refresh even when this run hits cache. Static completeness can be exact only when the credential backend and endpoints are known.

### Increment 10 — UDP as required transport support

R15 promotes UDP unicast, explicit receiving sockets and multicast from a loosely staged primitive to required Stage B contracts. Datagram semantics follow [UDP](https://www.rfc-editor.org/rfc/rfc768.html); the API and policy choices here are Rivet's.

```rivet
with udp "127.0.0.1:7000" as socket
    max_datagram 8192
    socket.send json {command: "status"}
    return socket.receive json timeout "1s"
end
```

Connected client form accepts replies only from the authorized peer. The OS-selected ephemeral local bind is part of that peer-scoped outbound capability and cannot become a general listener. `with udp bind "127.0.0.1:7001" as socket` requires an `allow_listen` grant for `udp://127.0.0.1:7001`; `receive_from bytes/json` yields `{peer,data}`. Every `send_to peer ...` separately requires an outbound `allow_network` grant for `udp://HOST:PORT`, even when peer came from a received datagram. Binding an unspecified address grants no arbitrary outbound access. Preserve source address metadata and normalize IPv6 bracket notation.

Datagrams can be lost, duplicated or reordered; no automatic delivery acknowledgment, deduplication or retry is invented. Sending reports local acceptance only (`effects:unknown` for potential remote mutations), and no response can prove non-execution. Empty datagrams are valid. `max_datagram` limits incoming/outgoing application payload; the effective maximum is also constrained by OS/path limits. Detect receive truncation and return `udp.truncated`, never silently parse a clipped message. Exceeding negotiated/path limits -> `udp.message_too_large`; deadline -> `timeout`; unsupported multicast/interface -> `unsupported.udp_option`. A configured 8192-byte cap does not guarantee fragmentation-free delivery.

Multicast declares group, port and interface; require outbound group authority for join/traffic plus explicit listener bind authority. Membership expires with the scope. Sender peer and interface remain auditable; no promise of reliable discovery. Broadcast is disabled by default and requires a separate explicit reviewed option/target grant. Capabilities list build/platform multicast support so unsupported options refuse before join. Normal return/error/cancel closes the socket and drops memberships automatically.

### Increment 11 — Native QUIC and HTTP/3

R17 uses QUIC v1 with TLS identity verification and ALPN selection. Streams are reliable ordered byte sequences within each stream; concurrent streams do not impose a cross-stream order. The optional DATAGRAM extension carries unreliable application messages. HTTP/3 is a separate application protocol over QUIC. Sources: [QUIC transport](https://www.rfc-editor.org/rfc/rfc9000.html), [QUIC TLS](https://www.rfc-editor.org/rfc/rfc9001.html), [DATAGRAM](https://www.rfc-editor.org/rfc/rfc9221.html), [HTTP/3](https://www.rfc-editor.org/rfc/rfc9114.html).

```rivet
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
```

The custom ALPN is fixture-specific and must match the peer. `connection.open uni` is locally send-only; `connection.accept uni` receives a peer-opened stream; bidi permits both. The named stream handle remains lexical and cannot cross operation/DAG boundaries. At most 8 active child streams by default, bounded additionally by peer limits and the host-wide task/byte budget. A peer-opened stream does not bypass operation authority. Flow-control waits count toward deadlines. Reliable stream framing uses the common codec layer, now available in Stage B for QUIC and TCP; datagrams never use length-prefix framing.

For finite requests the child scope finishes its send direction (FIN) and waits within the deadline for required response reads; errors/cancellation reset its send direction and stop its receive direction without automatically cancelling unrelated streams. If a protocol requires EOF before replying, `stream.finish_send` explicitly half-closes only the write direction while preserving reads. This is protocol signaling, not manual resource disposal; no `stream.close` or `connection.close` is exposed. Finishing writes does not claim the peer application processed them. The connection owner joins/cancels children and performs bounded connection cleanup on exit.

`datagrams true` requires peer negotiation or returns `quic.datagrams_unavailable`; `send_datagram bytes value` and `receive_datagram bytes` preserve one message each. Size is bounded by negotiated and path limits. No silent truncation, conversion into streams, retransmission or delivery guarantee. Stream errors (`quic.stream_reset`, `quic.stop_sending`) retain safe application codes and stream identity; connection-level failures cancel all owned streams. Handshake/certificate/ALPN/idle-timeout failures remain distinguishable. Audit events carry source, logical endpoint, resolved peer, connection and stream IDs.

`migration false` is the default. For an explicit `migration true`, every candidate peer address needs policy approval **before** path probes or application packets are sent; an authenticated peer suggesting a new address is not an authorization grant. A peer-source change observed on receive cannot cause an automatic policy-bypassing response. Disallow automatic socket/library migration that cannot be intercepted. Pooling/resumption is partitioned by principal, policy and credential identity; default connections are scoped, not shared across callers. Early data (0-RTT) is disabled; configuring it returns `unsupported.quic_early_data`, avoiding hidden replay of operations. Session tickets stay in bounded memory; persistent ticket stores would be additional declared I/O.

An `allow_network` grant for `quic://engine.example.com:4433` permits only that logical QUIC endpoint and its broker-checked UDP transport; it grants neither raw arbitrary UDP nor an unrelated migrated endpoint. The effect inventory reports both logical QUIC and underlying UDP traffic under one capability-bound adapter. Raw UDP grants do not implicitly authorize a QUIC identity/ALPN. DNS, credential/certificate reads and any explicit local bind retain their own checks.

HTTP/3 keeps the HTTP surface:

```rivet
response = http get "https://api.example.com/items"
    version 3
    auth crm_service account "service"
    decode json
end
return response.body
```

`version 3` means HTTP/3 required; no fallback occurs. `version prefer [3, 2]` explicitly permits protocol negotiation fallback before sending application request data. If any request data was sent, switch/retry only under the normal method/idempotency/body-replay rules; an ambiguous POST response never triggers a second mutation over HTTP/2. Strict H3 unavailable -> `http.version_unavailable`; unsupported build -> `unsupported.http3`. Default HTTP behavior remains the existing adapter default (HTTP/1.1 or HTTP/2); H3 is opt-in. `response.version` reports the actual selected HTTP version.

An HTTPS-origin grant authorizes H3 to the same hostname/port using broker-checked UDP; no raw UDP grant is exposed to the script. Effect inventory marks H3 UDP and all explicitly allowed fallback transports. Alt-Svc redirects, alternate ports and QUIC server preferred addresses require explicit additional target authority; preserving HTTP origin identity is necessary but insufficient. Credential attachment still checks the original resource origin and authenticated peer. Arbitrary native QUIC does not understand OAuth headers: an application protocol must explicitly define token carriage and binding before auth profiles can be used there. OAuth integration covers HTTP, HTTP/3, HTTP-based MCP and authenticated gRPC metadata over TLS with the same destination binding.

Required Stage B release coverage is UDP + OAuth core flows + QUIC streams/DATAGRAM + HTTP/3 + native gRPC and the MCP/session access contract, tested through the common registry, CLI, HTTP, MCP and Rust library. Server-side native QUIC listeners and a QUIC-based Rivet public RPC protocol are not introduced: native QUIC support here is outbound client connections with accepted peer-initiated streams; UDP has an explicit listener form. This bounds "support" to reviewable behavior rather than promising every application protocol over QUIC.

### Increment 12 — One file, many described operations, one catalog

R20/R21 make the shared operation model explicit. A `.rivet` source is a module of declarations, not a single executable operation. Loading it does not run any operation or start a connector. File basenames do not prefix or rename IDs. All operation IDs must be unique across the entire loaded bundle, including imported names; collision fails the whole candidate load with both source spans. Existing running requests keep their pinned catalog; an invalid replacement never leaves a half-registered catalog.

Save this complete example as `catalog.rivet`:

```rivet
operation demo.greet
    name "Greet a person"
    description "Return a greeting for the supplied person."
    param person text required description "The person's display name."
    output text description "Greeting text."
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
    output object description "Readiness report."
        field ready boolean required description "Always true when the runtime answers."
    end
    return {ready: true}
end
```

`name` is a display label, defaulting to the callable ID; `description`, parameter `description` and output/field `description` are optional literal text. Descriptions are documentation, never an authorization rule or executable template. The compiler retains original source positions and publishes type, required/default, bounds/enums and descriptions in one input JSON Schema. Output, emitted-item and received-item schemas remain distinct. `private true` makes a helper available only to in-bundle calls; it is neither listed nor externally callable, including through generic `rivet.request` or sessions.open. No parameter-driven private-ID bypass is allowed.

`rivet check --strict-docs` (with the global file option) requires nonblank descriptions for every public operation, parameter and output, and for every `output`/`emits`/`receives` field, and turns undeclared `fail` codes into errors; without that option, missing descriptions are diagnostics/warnings rather than invented prose. Earlier compact examples remain valid but need descriptions to pass this documentation release gate. Integer overflow is a typed validation/runtime error; metadata does not change computation. Server-provided/user-authored descriptions are untrusted text when shown to MCP clients and are never appended to runtime policy.

```text
catalog.rivet: demo.greet + demo.add + demo.health
      -> Capy parse/typecheck (no effects)
      -> immutable RegistryEntry[] {id,name,description,params,output,emits,receives,errors,effects,source}
          +-> CLI list/describe/outputs/request
          +-> HTTP /v1/operations, /v1/operations/{id}/outputs, /v1/request, /v1/requests (poll), /v1/ws
          +-> Rust Runtime registry/request/outputs/duplex
          +-> MCP tools/list (inputSchema + outputSchema) and tools/call
```

The callable ID and parameter names stay identical on every access point. `GET /v1/operations` and `rivet.list` may return paginated summaries; `describe` returns the full schema/metadata. `RegistryEntry.name` is display text, while `RegistryEntry.id` is dispatch identity. MCP maps ID to tool `name`, display name to `title`, description to `description`, params to `inputSchema`, and the actual result envelope to `outputSchema`. Input schemas are closed objects by default, including no-parameter `{type:"object",additionalProperties:false}`. Constraints/defaults are interpreted by the shared dispatcher, not independently by each surface.

MCP is a first-class **incoming** access point, distinct from Rivet's outgoing MCP connector; direct tool publication is required. Streamable HTTP is mounted at `/mcp` by every `rivet serve` alongside the other surfaces ([Increment 17](#increment-17--one-serve-every-surface)); `rivet serve --stdio` runs MCP over stdio with protocol-only stdout. Protocol framing/session rules follow the [MCP transport baseline](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports).

For authorized callers, `tools/list` contains every public operation, including pure helpers deliberately declared public; it filters private/unauthorized operations. Native imported connector names may use explicit validated aliases; no silent lossy name conversion. `tools/call {name:"demo.add",arguments:{a:2,b:3}}` resolves the same registry entry as CLI/HTTP/library and is the canonical MCP call. The built-in generic tools (Increment 6 table) remain available under normal host authorization. There is no separate MCP-only implementation of demo.add. MCP tool names/schemas/results follow the [tool baseline](https://modelcontextprotocol.io/specification/2025-11-25/server/tools).

Unary direct tool results use `{content:[{type:"text",text:SERIALIZED_COMPLETION}],structuredContent:Completion,isError:false}`. Scalar/array application results remain inside Completion.result so the structured result is an object. A tool execution error uses `isError:true` with ErrorEnvelope; malformed JSON-RPC/tool invocation uses a protocol error as appropriate. The advertised `outputSchema` is the Completion envelope whose `result` property is the operation's declared output schema (Increment 15), rather than a bare scalar. Streaming tools advertise session delivery and return SessionReceipt as described in increment 14. Optional list-changed notifications never authorize new tools; catalog reloads are atomic and existing calls retain their original snapshot.

### Increment 13 — Full gRPC client adapter

R19 adds native gRPC over HTTP/2 as required Stage B scope. The four method modes, terminal status and metadata are defined by [gRPC core concepts](https://grpc.io/docs/what-is-grpc/core-concepts/). Rivet-specific choices below define the language and adapter boundary. gRPC-Web and native gRPC over HTTP/3 are not implied; unsupported transports return a typed error. Exposing arbitrary Rivet operations as a native gRPC server is a separate future surface; Rivet calls gRPC services and exposes those operations through the requested CLI/HTTP/MCP/library surfaces.

```rivet
connector users grpc
    endpoint "https://users.example.com:443"
    descriptor "./schemas/users.pb"
    service "example.Users"
end

operation users.grpc_get
    name "Get a user over gRPC"
    description "Fetch a user through the example.Users gRPC service."
    param id text required description "Stable user identifier (ProtoJSON string)."
    output json description "The example.User message as ProtoJSON."
    response = grpc users.GetUser
        timeout "5s"
        message {id: id}
    end
    return response.message
end
```

The descriptor is a pinned binary `FileDescriptorSet`, including dependencies, supplied as part of explicit bundle assembly. Capy parses Rivet source; the adapter loads descriptor data, not a second Rivet parser. `.proto` source can be compiled offline by the developer; runtime invocation never implicitly executes protoc, reads transitive files or performs server reflection. Missing service/method/imported type fails load before dialing. Descriptor hashes bind method/cardinality/input/output schemas to the program. Optional future reflection must be an explicit separately authorized discovery operation producing a reviewed snapshot; it is not part of the grammar.

Use descriptor-driven dynamic messages or generated Rust bindings behind the same port; the implementation plan selects crates and pins versions after a feasibility test. Required supported schema surface includes scalar/repeated/map fields, enums, presence, oneof and explicitly bundled well-known types. Unknown JSON fields, conflicting oneof members, invalid enum/range/bytes values and unresolved Any type URLs fail validation. No schema fetching from Any URLs. Encode/decode at the gRPC boundary follows [ProtoJSON](https://protobuf.dev/programming-guides/json/): 64-bit integer decimal strings and bytes base64 strings remain lossless in its JSON view. General Rivet bytes/integer tagged envelopes are distinct; convert explicitly using the descriptor rather than mixing representations. Non-finite protobuf floats use the documented ProtoJSON string representation within the protobuf view, never raw non-finite JSON numbers.

| RPC mode | Canonical operation | Result contract |
|---|---|---|
| Unary | `response = grpc users.GetUser ... end` with one message | GrpcResponse `{message,initial_metadata,trailers,status}` only after terminal OK |
| Server stream | `with grpc users.Watch as rpc` with one message; `for message in rpc` | Typed messages; iteration reaches normal EOF only after OK status; `rpc.completion` exposes bounded metadata/trailers/status |
| Client stream | `with grpc users.Upload as rpc`; `rpc.send value` repeatedly; `rpc.finish_send`; `response = rpc.result` | One response plus final OK; result awaits completion, not just first bytes |
| Bidirectional | `with grpc users.Chat as rpc`; independent send task and `for message in rpc` receive task | Ordered messages within each direction, bounded queues and final status; half-close does not stop reads |

Stream mode is derived from the descriptor. Supplying `message` to a client-streaming method or sending multiple messages to a unary input is rejected; no guessing from runtime traffic. Stream iteration yields message values only; EOF waits for trailing status. `rpc.finish_send` is idempotent input half-close, not resource disposal. A peer can finish early; further sends fail `grpc.input_closed` and the receive side still reports the actual final status. Request cancellation cancels this RPC and joins owned tasks; it must not kill unrelated calls using a safe shared channel. Default channels are scoped; any pooling is partitioned by identity, policy and endpoint.

Metadata syntax `metadata "x-request-id" "example"` and `metadata "trace-bin" bytes value` preserves ASCII/binary distinctions and duplicate entries internally. Reserved `grpc-*` keys and conflicting authorization entries are rejected. TLS verifies certificate identity; optional connector `tls ca_file`/`cert_file`/`key_file` lines are option-derived file sites (phase `before_connect`, key file `secret`) that need `allow_read` grants and appear in `rivet io --needs` for every operation calling the connector. `auth PROFILE account ACCOUNT` attaches an authorized bearer credential as request metadata, using the existing profile's exact HTTPS resource-origin and audience/scope checks. A midstream token expiry does not cause transparent stream restart or token replacement within an already sent RPC; any reconnect is explicit and replay-safe. Source effect inventory includes credential refresh/store and descriptor assembly reads.

Logical permission is an `allow_grpc` grant for `CONNECTOR/Fully.Qualified.Service/Method`, in addition to an `allow_network` grant for `https://HOST:PORT`. HTTPS permission is scoped by the adapter to native HTTP/2; plaintext `http://` is development opt-in and may not carry secret auth. Authority override, service-discovered alternate peers and custom resolvers cannot bypass destination grants or TLS identity; no automatic resolver discovery is enabled. Server-side load balancing remains a declared future adapter option. Auth profile names do not confer method authority.

The common deadline maps to the RPC timeout, including channel readiness and flow-control waits. Size limits apply to compressed and decompressed messages plus buffered bytes, preventing decompression expansion from bypassing limits. Message compression defaults off; unsupported compression fails explicitly. Built-in library retries/hedging are disabled unless they can obey Rivet's declared replay rules and appear as separate attempts in traces. No retry after an emitted response item, sent stream input or uncertain mutation. Method names alone do not prove idempotency.

Map gRPC codes into stable errors while preserving original numeric/name status and redacted trailers: `CANCELLED`→cancelled, `DEADLINE_EXCEEDED`→timeout, `NOT_FOUND`→not_found, `INVALID_ARGUMENT`→validation, `PERMISSION_DENIED`→permission, `UNAUTHENTICATED`→`auth.login_required`, `RESOURCE_EXHAUSTED`→limit, `UNIMPLEMENTED`→unsupported; retain other codes under `grpc.<code>` and safe dependency categories. Reference: [gRPC status codes](https://grpc.io/docs/guides/status-codes/). HTTP 200 on the underlying gRPC connection is not success; only final gRPC OK establishes success. After emitted data, non-OK trailers produce one terminal Rivet error with data_count/partial effects; never discard them or fabricate a final result. Error details in Any payloads are decoded only with bundled schemas and redacted before exposure.

### Increment 14 — Live streaming input across every access point

The ordinary request (DSL `(request "ID" {…})`, Rust `rt.request(id, params, sink)`) remains the simplest unary/server-streaming API. An operation can additionally declare `receives TYPE`; the scoped read-only `incoming` iterable then supplies input messages separately from ordinary params. `emits TYPE` defines outgoing items. Declaring receives requires duplex/session invocation or an explicit finite input feeder; a plain unary request fails `stream.input_required` before starting effects. A batch list in params is sufficient for finite client-streaming wrappers but does not pretend to be interactive duplex.

```rivet
operation chat.exchange
    name "Exchange live chat messages"
    description "Forward live messages to a gRPC chat service."
    receives json
    emits json
    output json
    with grpc users.Chat as rpc
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

A peer early-completion signal closes the incoming feeder and wakes blocked reads/writes; the sender task must exit rather than keeping the concurrent group alive forever. Body failure/cancellation stops both directions, joins tasks and disposes the RPC. The input/output schemas are checked item by item. `Runtime::scope` supplies `scope.duplex(ID,params)` with asynchronous `send`, `finish_send`, `next`, and split send/receive capabilities; its owning scope performs cancellation and cleanup. No stream handle escapes through an operation return or a DAG edge.

For HTTP, MCP and multi-command CLI interactions, use the same five registry operations:

| ID | Input params | Application result |
|---|---|---|
| `rivet.sessions.open` | `{id,params}` | SessionReceipt `{session_id,request_id,catalog_version,input_schema,emits_schema,next_send_seq,expires_at}` |
| `rivet.sessions.send` | `{session_id,send_seq,data}` | SessionAck `{session_id,accepted_seq,input_closed:false}` after bounded local queue acceptance |
| `rivet.sessions.finish_input` | `{session_id}` | SessionAck `{session_id,accepted_seq:null,input_closed:true}`; accepted messages drain before protocol half-close |
| `rivet.sessions.read` | `{session_id,after_seq,max_events?,wait_ms?}` | SessionBatch `{session_id,events,last_seq,terminal}` with normal data/result/error envelopes |
| `rivet.sessions.cancel` | `{session_id}` | CancelReceipt `{session_id,request_id,state:"cancelled"}` after bounded owned cleanup; terminal error remains readable during retention |

These are ordinary `/v1/request` calls, CLI `request` IDs and MCP named tools. `rivet serve` also projects them as the polling routes and WebSocket frames of [Increment 17](#increment-17--one-serve-every-surface); neither adds an operation. Library hosts may use these IDs too. CLI `request ID --params JSON --input-jsonl - --stream` is a convenience duplex feeder: stdin JSONL validates/enqueues input while NDJSON stdout drains output concurrently, stdin EOF finishes input, and SIGINT or malformed input cancels. File input paths are explicit reads; inherited stdin/stdout are invocation channels and grant no arbitrary script descriptor access. This convenience must not buffer all stdin before reading output.

One streaming operation appears as a normal named MCP tool with its ordinary params inputSchema. Its descriptor declares session delivery via `_meta: {"rivet/delivery":"session"}` and a SessionReceipt success schema. `tools/call chat.exchange` opens a bounded session and returns that receipt; follow with `rivet.sessions.*` calls. Server-streaming operations use the same delivery form, with input_schema null. No generic MCP tool-result streaming or arbitrary JSON-RPC notifications are assumed. Unary named tools continue returning Completion. `rivet.request` is a built-in generic tool selecting Completion or SessionReceipt from the registry mode; the direct HTTP unary route returns `stream.required` unless the caller asks for SSE, uses polling/WebSocket, or calls sessions.open. Different wire delivery does not change the operation, schemas or authorization.

Session lifecycle:

```text
 open ---> active (input open) --finish_input--> active (input closed) --terminal event--> retained (<=60s)
            |        |                                   |                                    |
            |        +--send(seq) ok / retry same seq    +--read(after_seq)                   +--TTL--> expired
            |
            +--cancel / idle 60s / total deadline / policy revoked / runtime shutdown--> cancelling
                                                                                           |
                                                             children joined, cleanup -> retained (terminal error)
```

```text
client -> sessions.open -> bounded host child context -> gRPC call
client -> sessions.send --[input sequence / queue]-------> sender
client <- sessions.read <-[retained event sequence]------- receiver
client -> finish_input ---------------------------------> FIN / half-close
                     final status -> scope cleanup -> terminal event
abandon / cancel / idle expiry -> cancel child tree -> cleanup -> expire record
```

Session IDs are opaque routing IDs, **not bearer permissions**. Every action authenticates the same principal/tenant and rechecks operation visibility and host policy. The open request pins program/catalog hash, input/output schemas and effective policy; later calls can never widen that authority. Policy revocation prevents new effects and triggers cancellation. A session record contains only IDs/values; resources belong to the host runtime's explicit child scope. This extends the earlier no-detached-work rule: open intentionally creates a bounded service-owned request lifetime, not an unowned task. Individual transport disconnection need not destroy it; runtime shutdown always cancels and joins children. Stdio host process/session termination owns its sessions; HTTP reconnect requires the same authenticated principal.

Limits: 8 live sessions per principal, default 30s total operation deadline unless explicitly raised within host caps, 60s idle lease capped by total deadline, 16 buffered frames and 32 MiB total across input/output queues. Read wait defaults 1000ms, max 5000ms; max_events defaults 16 and is capped by the host. Active data production alone does not renew the consumer's idle lease; otherwise an abandoned consumer could retain resources forever. On terminal completion, resources are closed immediately and event records remain for at most 60s for final delivery. No persistence or resume after process restart is promised.

Input `send_seq` starts at 1. The most recent accepted sequence can be retried with the same payload hash to get the same acknowledgment without a second enqueue; changed payload or older/future sequence is `conflict.input_sequence`. Finish_input is idempotent and disallows subsequent sends. A send acknowledgment proves only queue acceptance, not server execution. Streaming backpressure cannot be bypassed by opening more sessions beyond principal/global limits.

Output seq starts at 1. `read(after_seq)` acknowledges only previously delivered events up to that sequence and returns following retained events. One logical consumer is allowed; concurrent reads are serialized and conflicting cursor progress is rejected. Re-reading the same cursor may replay retained events; clients deduplicate by seq. Acknowledging a future/not-delivered sequence fails; reading data evicted after acknowledgment/TTL returns `stream.cursor_expired` rather than silently skipping. Stop producing when retention is full; idle/total deadline then cancels stalled work. There is one terminal result/error in the retained sequence, though delivery may repeat until acknowledged. Session management errors use the ordinary ErrorEnvelope (404 missing/expired, 403 unauthorized, 409 sequence/state conflict, 422 message schema, 429 session limit); operation terminal failures live inside SessionBatch.events.

A DAG node can own a scoped gRPC stream, and bounded operation composition remains available. Live stream edges between DAG nodes are still a separate design extension; session support does not introduce cycles or leak streaming handles into graph results.

### Increment 15 — Declared outputs

R23. Every operation declares what it returns, with descriptions, and every surface can show it.

Scalar form stays valid (`output text`, `output integer`, `output json`) and takes an optional description:

```rivet
operation demo.add
    param a integer required description "First operand."
    param b integer default 0 description "Second operand."
    output integer description "Sum of a and b."
    return a + b
end
```

Structured form opens a block closed by `end`; fields reuse the parameter shape:

```rivet
operation users.get
    name "Get a user"
    description "Fetch one user by numeric ID."
    param id integer required min 1 description "Stable user ID."
    output object description "The requested user."
        field id integer required description "Stable user ID."
        field name text required description "Display name."
        field email text optional description "Primary email, when public."
        field tags list text optional description "Free-form labels."
        field address object optional description "Postal address."
            field city text required description "City name."
        end
    end
    error "users.not_found" description "No user has this ID."

    response = http get "https://api.example.com/users/${id}"
        timeout "10s"
        decode json
    end
    if response.status == 404
        fail "users.not_found" {id: id}
    end
    return response.body
end
```

Rules:

- Types: `text`, `integer`, `number`, `boolean`, `bytes`, `json` (opaque), `object` (nested fields), `list T`.
- Each field is `required` (default) or `optional`. Objects are closed (`additionalProperties:false`) unless
  the output block contains `open true`.
- `emits` and `receives` accept the same field block for item schemas.
- `error "code" description "…"` lines declare failure codes (documentation + schema). A `fail "code" {…}`
  with an undeclared code is a `check` warning; `check --strict-docs` makes it an error.
- Header order (leading-options rule): `name`, `description`, `private`, `param*`, `output` (+ block),
  `emits`, `receives`, `error*` — all before the first body statement.
- Runtime: the dispatcher validates the final result against the declared output **before** producing
  Completion. Mismatch → kind `output_invalid`, code `output.invalid`, HTTP 500, exit 5, effects preserved
  (committed effects are reported, not rolled back). `json` performs no structural check.

```text
 body returns V --> validate V against RegistryEntry.output --ok--> Completion{result: V}
                                    |
                                    +--mismatch--> ErrorEnvelope{kind: output_invalid, code: output.invalid,
                                                                 effects: committed|partial|none}
```

Viewing (same data on every surface, built from the immutable `RegistryEntry`):

| Surface | Form | Shows |
|---|---|---|
| CLI | `rivet outputs demo.add` | Human table: output type/description, fields, emits, receives, errors |
| CLI | `rivet outputs demo.add --json` | `{"id","output":{schema},"emits":{…}\|null,"receives":{…}\|null,"errors":[{code,description}]}` |
| CLI | `rivet outputs --all [--json]` | Every public operation |
| CLI | `rivet describe demo.add` | Now includes an "Output" section (params + output + errors) |
| CLI | `rivet list --outputs` | Adds a one-line output summary column |
| HTTP | `GET /v1/operations/{id}/outputs` | The `--json` document |
| MCP | tool `rivet.outputs {id?, all?}`; each tool's `outputSchema` | Completion envelope whose `result` is the declared schema |
| Rust | `RegistryEntry.output: OutputSpec`, `Runtime::outputs(id) -> OutputSpec` | Same schema |

Example human output of `rivet outputs users.get`:

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

Feasibility: **proven in design** (schema projection reuses the parameter JSON Schema builder); the nested
`output object … end` block is part of the Capy spike gate (**experiment needed**).

### Increment 16 — policy.json only configuration

R24. Policy is data in a file, never text on a command line.

- **Removed:** the revision 1–4 `--sandbox` flag and all inline grant strings. There is no env-var policy.
- **Source:** `policy.json` in the same directory as the entry `.rivet` file is auto-discovered.
  `--policy PATH` selects a different file; it takes a path only, never grants.
- **Absent file = deny-by-default** for every new application effect (what `--sandbox ""` used to mean).
  Pure operations still run. There is no implicit allow-all mode: allow-all must be written explicitly (for
  example `targets: ["*"]`), and `policy explain` flags it as broad.
- **Library hosts:** `Policy::from_file(path)` / `Policy::from_json(bytes)`, same schema; the host ceiling
  intersects it.
- **Intersection:** host ceiling ∩ policy.json ∩ per-request restriction. HTTP, WebSocket and MCP callers can
  only narrow, never grant.

```text
 entry dir/
 ├── app.rivet
 └── policy.json   <-- auto-discovered          --policy ./policies/ci.json  <-- explicit path instead
        |
        v
  parse + validate (schema v1) --error--> policy.invalid, exit 2, nothing runs
        |
        v
  effective = host ceiling ∩ file grants − file deny ∩ request restriction
        |
        v
  every brokered attempt checked against `effective`
  (no file found: effective = {} -> every application effect denied)
```

Schema v1 (every key except `version` optional):

```json
{
  "version": 1,
  "grants":   [{"capability": "allow_read",    "targets": ["./data/**"]},
               {"capability": "allow_write",   "targets": ["./out/**"], "access": ["create"]}],
  "deny":     [{"capability": "allow_network", "targets": ["https://internal.example.com:443"]}],
  "network":  {"deny_private_ranges": true},
  "limits":   {"max_concurrent_requests": 64, "max_call_depth": 16, "max_buffered_bytes": 268435456},
  "approved": {"snapshots": ["sha256:…"], "overlaps": ["sha256:…"]},
  "serve":    {"surfaces": ["http", "sse", "poll", "ws", "mcp"],
               "auth": {"type": "none"},
               "principals": {"ada": {"operations": ["demo.*", "users.get"]}}}
}
```

- Capabilities are those of Increment 5. Targets resolve relative to the policy file's directory; `/**` is a
  descendant boundary, not a string prefix. Network targets normalize IDNA, case, scheme and explicit/default
  port; paths are never hostname grants.
- Unknown keys, unknown capabilities or malformed selectors → `policy.invalid`, exit 2. `deny` overrides
  `grants`.
- `access` (optional, on `grants` and `deny` entries) narrows a capability to some of its verbs
  ([Increment 18](#increment-18--generated-io-manifest-and-policy-drafts) vocabulary). Absent = every verb of
  that capability (backwards compatible). A verb that does not belong to the capability → `policy.invalid`,
  exit 2. The `allow_write` + `["create"]` grant above allows creating files in `./out` but never overwriting.
- `network.deny_private_ranges` defaults to `true` (Increment 5 SSRF defaults).
- `approved` lists reviewed snapshot and overlap hashes (definition of *reviewed*, Increment 6).
- `serve` configures surfaces, authentication and per-principal authorization (Increment 17).
- Commands: `rivet policy explain ID --params JSON --json` and `rivet io --check-policy` show the effective
  policy; `rivet policy generate` drafts one from the I/O manifest (Increment 18).

Canonical CLI forms:

```sh
rivet --file app.rivet request demo.add --params '{"a":2,"b":3}'          # uses ./policy.json if present
rivet --file app.rivet --policy ./policies/ci.json request users.get --params '{"id":42}'
```

UQ-13 is retained in behaviour (deny unless specified); UQ-17 supersedes its flag syntax. Feasibility:
**proven in design** (a JSON loader plus the existing intersection logic).

### Increment 17 — One serve, every surface

R25. One command, one listener, every network surface:

```sh
rivet --file app.rivet serve [--listen 127.0.0.1:8080]      # default listen 127.0.0.1:8080
rivet --file app.rivet serve --stdio                         # MCP over stdio only (cannot share a socket)
```

The revision 3–4 `--transport` and `--mcp` serve flags are removed.

```text
                           rivet serve --listen 127.0.0.1:8080
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

**Polling routes** are the HTTP projection of `rivet.sessions.*`:

| Route | Maps to | Request | Response |
|---|---|---|---|
| `POST /v1/requests` | `sessions.open` | `{"id","params"}` | 202 SessionReceipt + `events_url` |
| `GET /v1/requests/{id}/events?after_seq=N&wait_ms=M` | `sessions.read` | query | 200 SessionBatch `{session_id,events,last_seq,terminal}` |
| `POST /v1/requests/{id}/input` | `sessions.send` | `{"send_seq","data"}` | 200 SessionAck |
| `POST /v1/requests/{id}/finish_input` | `sessions.finish_input` | `{}` | 200 SessionAck `input_closed:true` |
| `POST /v1/requests/{id}/cancel` | `sessions.cancel` | `{}` | 200 CancelReceipt `{session_id,request_id,state}` after bounded cleanup |

Unary operations can be submitted the same way (async job); their batch has one terminal `result` event.
Polling sessions survive reconnect (principal-owned; limits of Increment 14).

**WebSocket** `GET /v1/ws`, subprotocol `rivet.v1`, JSON text frames multiplexed by a client-chosen `ref`:

```text
 client -> server                                          server -> client
 {"type":"request","ref":"c1","id":"demo.add",             {"type":"data","ref":"c1","seq":1,"data":…}
  "params":{"a":2,"b":3}}                                  {"type":"result","ref":"c1","completion":{Completion}}
 {"type":"input","ref":"c1","seq":1,"data":{…}}            {"type":"error","ref":"c1","error":{ErrorEnvelope}}
 {"type":"finish_input","ref":"c1"}
 {"type":"cancel","ref":"c1"}

 per ref:  request --> data* --> exactly one of result | error        (input*/finish_input for receives)
 socket close --> cancel + join every open ref (connection-owned)
```

- At most 8 in-flight refs per connection (same limit as sessions per principal; excess → `limit` error frame);
  per-ref bounded queues of 16 frames.
- A ref gets exactly one terminal frame. Closing the socket cancels and joins all its refs — unlike polling
  sessions, which survive reconnect.
- WebSocket is a transport projection only: no WS-only operations.

**MCP** at `/mcp` as in Increment 12 (direct named tools canonical, Increment 6).

**Surfaces** are narrowed in policy.json: `"serve": {"surfaces": ["http","sse","poll","ws","mcp"]}` (default: all
five). A disabled surface answers 404.

**Authentication and principals** (policy.json `serve.auth`):

```json
{"type": "none"}
{"type": "bearer", "tokens": [{"principal": "ada", "sha256": "<hex of token>"}]}
{"type": "mtls", "client_ca": "./ca.pem", "principals": [{"principal": "ci", "subject": "CN=ci"}]}
```

- `none` is allowed only on loopback binds; the principal is `local`. Missing `serve.auth` = `none`.
- A non-loopback bind with auth `none` refuses to start: `serve.auth_required`, exit 2.
- Library hosts may supply an `Authenticator` callback instead.
- Per-principal authorization: `"serve": {"principals": {"ada": {"operations": ["demo.*","users.get"]}}}`.
  Without the map every authenticated principal may call every public operation.
- Exception: `rivet.io` and `rivet.policy.generate` reveal internal URLs/paths, so a network principal may call
  them only when its `operations` list names them explicitly; `*`/`demo.*` patterns never match `rivet.*`.
  Loopback principal `local` may call them ([Increment 18](#increment-18--generated-io-manifest-and-policy-drafts)).
- One principal model on every surface: `Authorization: Bearer` on HTTP/SSE/poll, on the WebSocket upgrade
  request, and on MCP HTTP requests.
- `serve` with no policy.json is allowed (loopback, auth none): pure operations work, all effects are denied.

Feasibility: **proven** for the surface shapes (each is an established protocol projection of the existing
dispatcher); **experiment needed** for shared-listener routing of MCP Streamable HTTP and WebSocket upgrade in
the selected HTTP crate.

### Increment 18 — Generated I/O manifest and policy drafts

R26 ([UQ-18](../../references/ref-2026-0001-request-and-evidence.md#io-manifest-request)). `rivet io` produces a
generated **I/O manifest** (`IoManifest`): every effect site, the concrete target it uses (URL, path, host:port,
argv, env var, connector method…) and the fine-grained **access** it performs (read, write, delete, …), mapped to
the capability that permits it. The manifest is viewable by operation, target or capability, exportable, checkable
against `policy.json`, and convertible into a least-privilege policy draft. This refines the output contract of R6
/ UC-09 ([Increment 5](#increment-5--effects-sandbox-and-audit)); nothing is removed.

```text
app.rivet ──parse/lower──▶ effect sites ──normalize──▶ IoManifest ──┬─▶ io --by operation|target|capability
                               │                                    ├─▶ io --check-policy  (◀── policy.json)
                               │                                    ├─▶ policy generate ──▶ draft policy.json
                               └──effect_id──▶ runtime trace ───────┴─▶ io --trace REQ (planned vs actual)
```

**Access vocabulary (per effect site).**

| Kind | Access verbs | Capability |
|---|---|---|
| `file` | `read`, `list`, `stat`, `watch` | `allow_read` |
| `file` | `create`, `update`, `append` | `allow_write` |
| `file` | `delete` | `allow_delete` |
| `network` | `connect` (+ protocol, + method for HTTP) | `allow_network` |
| `network` | `bind`, `listen`, `multicast_join` | `allow_listen` |
| `process` | `exec` | `allow_exec` |
| `env` | `read` | `allow_env` |
| `pipe` | `read`, `write` | `allow_pipe` |
| `unix` | `connect`, `listen` | `allow_unix` |
| `mcp` | `call` (`tool`\|`resource`\|`prompt`) | `allow_mcp` (+ the transport's `allow_network`/`allow_exec`) |
| `grpc` | `call` (`unary`\|`server_stream`\|`client_stream`\|`bidi`) | `allow_grpc` (+ `allow_network`) |
| `auth` | `use`, `manage`, `status` | `allow_auth` |
| `credential` | `read`, `write` | `allow_credentials` |

HTTP sites also record `method` (`GET`, `POST`, …) and `protocol` (`http1|http2|http3|ws|sse|grpc|quic|udp|tcp`).
A single source line can produce several sites, and each is listed:

```text
 source line                                 sites in the manifest
 ------------------------------------------  -------------------------------------------------------
 file update "./out/a.json" json v       ->  file stat   ./out/a.json  (allow_read)
                                             file update ./out/a.json  (allow_write)
 MCP stdio connector tool call           ->  process exec  /usr/bin/crm-mcp       (allow_exec)
                                             mcp call tool crm.tools.search       (allow_mcp)
 grpc users.GetUser                      ->  network connect https://users.example.com:443 (allow_network)
                                             grpc call unary users/example.Users/GetUser    (allow_grpc)
 http post … with tls ca_file/cert_file/  ->  file read ./certs/ca.pem      origin tls ca_file   (allow_read)
   key_file lines (revision 8)               file read ./certs/client.pem  origin tls cert_file (allow_read)
                                             file read ./certs/client.key  origin tls key_file  (allow_read, secret)
                                             network connect POST https://render.example.com:443 (allow_network)
```

**Per-access narrowing in policy.json.** Grant and deny entries accept an optional `access` list:

```json
{"capability": "allow_write", "targets": ["./out/**"], "access": ["create"]}
```

- `access` absent = every verb of that capability (backwards compatible with revision 5 files).
- A verb that does not belong to the capability (for example `allow_write` + `["delete"]`) → `policy.invalid`,
  exit 2.
- `deny` entries may carry `access` too. The example allows creating files in `./out` but never overwriting.

```text
 site: file update ./out/user.json   (needs allow_write/update)
   grant allow_write ./out/** access [create]          -> verb not granted  -> denied
   grant allow_write ./out/**                          -> all write verbs   -> allowed
   grant allow_write ./out/** + deny ... access [update] -> deny overrides  -> denied
```

**`rivet io` — the manifest command.**

```sh
rivet --file app.rivet io [ID ...] [--all] [--transitive] [--include-bootstrap]
      [--by operation|target|capability] [--kind file|network|process|env|pipe|unix|mcp|grpc|auth|credential]
      [--access VERB[,VERB]] [--format table|json|markdown|csv] [--check-policy] [--strict] [--trace REQ]
      [--needs] [--check-files]
```

| Flag | Default | Meaning |
|---|---|---|
| `ID ...` | all public operations | Entry operations to inventory |
| `--all` | off | Also private helpers and operations unreachable from any public entry |
| `--transitive` | on | Follow literal `(request "id" …)` calls into callee sites |
| `--include-bootstrap` | off | Add the fixed runtime-internal list of Increment 5 under `bootstrap` |
| `--by` | `operation` | Group rows by operation, target or capability |
| `--kind` | every kind | Keep only sites of one kind |
| `--access` | every verb | Keep only sites performing the listed verbs |
| `--format` | `table` | `table`, `json` (IoManifest), `markdown` (same tables, for reviews), `csv` (one row per site). The global `--json` flag used by `list`, `describe` and `outputs` is an alias for `--format json` |
| `--check-policy` | off | Evaluate each site against the effective policy (host ceiling ∩ policy.json); add `decision` |
| `--strict` | off | Exit 7 when any site is `dynamic`/`opaque_*` (`complete: false`) |
| `--trace REQ` | off | Add an `ATTEMPTS` column (count, last decision) from that request's trace |
| `--needs` | off | Revision 8. Print, per operation, the files that must already exist before it can run (static; [below](#option-derived-file-sites-and-io---needs)) |
| `--check-files` | off | Revision 8. Implies `--needs`; stat each needed exact path through the policy broker and report `present`/`missing`/`unreadable`/`not_permitted`/`not_checkable`. **Performs I/O** |

`decision` values: `allowed` | `denied` | `partial` (a `param_dependent` target only partly covered by grants) |
`unknown`. Exit codes: `0`; `3` when `--check-policy` finds any reachable site `denied` or `partial`, or
`--check-files` finds a file `not_permitted` or `unreadable`; `4` when `--check-files` finds a needed file
`missing`; `7` when `--strict` and any site is dynamic/opaque. **No I/O is performed** (except the metadata probes
of `--check-files`) and source expressions are not evaluated.

Table, `--by operation` (default), for the docs/demos-style `users.snapshot` bundle with `--check-policy`:

```text
OPERATION        KIND     ACCESS         TARGET                                   KNOWLEDGE        SOURCE        DECISION
users.get        network  connect GET    https://api.example.com/users/{id}       param_dependent  app.rivet:6   allowed
users.snapshot   (calls users.get — see above)                                                    app.rivet:15
users.snapshot   file     create         ./out/user.json                          exact            app.rivet:16  allowed
notes.delete     file     delete         ./out/notes/{name}.json                  param_dependent  app.rivet:40  denied
```

`--by target` (what every URL / path is used for):

```text
TARGET                       ACCESS                  CAPABILITY     ORIGIN                     PHASE    NEEDS FILE     USED BY
https://api.example.com:443  connect GET, POST       allow_network  http get, http post        connect  —              users.get, users.create
./out/notes/*.json           create, update, delete  allow_write,   file create, file update,  body     yes: update,   notes.create, notes.update,
                                                     allow_delete   file delete                         delete         notes.delete
./data/input.json            read                    allow_read     file read                  body     yes            report.load
env API_KEY                  read                    allow_env      secret                     body     —              users.get (secret, bound to https://api.example.com:443)
```

Revision 8 adds the ORIGIN, PHASE and NEEDS FILE columns to `--by target` (the `--by operation` and
`--by capability` columns are unchanged; JSON and CSV carry the fields for every site). NEEDS FILE is `yes`,
`no`, `yes: <verbs>` for a row that merges sites, or `—` for a non-file row.

`--by capability` groups the same rows under `allow_read` / `allow_write` / … headings:

```text
allow_network
  https://api.example.com:443     connect GET, POST        users.get, users.create
allow_write
  ./out/notes/*.json              create, update           notes.create, notes.update
allow_delete
  ./out/notes/*.json              delete                   notes.delete
allow_read
  ./data/input.json               read                     report.load
allow_env
  env API_KEY                     read                     users.get
```

**Target normalization.**

- URLs: `scheme://host:port` + path template; interpolations shown as `{param}`.
- Paths: relative to the bundle root, `{param}` for interpolated segments, and the derived glob for
  `param_dependent` paths (`./out/notes/{name}.json` → glob `./out/notes/*.json`).
- Knowledge classes are unchanged: `exact | bounded | param_dependent | dynamic | opaque_remote | opaque_native`.

```text
 source expression                                  template                              glob / origin
 -------------------------------------------------  ------------------------------------  ---------------------------
 "https://api.example.com/users/${id}"          ->  https://api.example.com/users/{id}    https://api.example.com:443
 "./out/notes/${name}.json"                     ->  ./out/notes/{name}.json               ./out/notes/*.json
 "./out/user.json"                              ->  ./out/user.json                       (exact; no glob)
```

**JSON (`--format json`) = IoManifest:**

```text
{
  "bundle": {"file": "app.rivet", "sha256": "…"},
  "policy": {"file": "policy.json", "sha256": "…"} | null,
  "complete": true,
  "sites": [
    {
      "effect_id": "users.get#1",
      "operation_id": "users.get",
      "kind": "network",
      "access": ["connect"],
      "method": "GET",
      "protocol": "http1|http2|http3",
      "capability": "allow_network",
      "target": {"template": "https://api.example.com/users/{id}", "scheme":"https","host":"api.example.com",
                 "port":443, "path":"/users/{id}", "glob": null, "params": ["id"]},
      "knowledge": "param_dependent",
      "condition": null,
      "call_chain": ["users.snapshot", "users.get"],
      "secrets": ["api_key"],
      "source": {"file":"app.rivet","line":6,"column":5},
      "origin": {"statement": "http get"} | {"option": "tls key_file"},
      "phase": "load|before_connect|connect|body|cleanup",
      "requires_existing": false,
      "secret": false,
      "decision": "allowed" | null
    }
  ],
  "targets": [ {"target":"https://api.example.com:443","capability":"allow_network",
                "access":["connect"],"methods":["GET","POST"],"origins":[{"statement":"http get"},{"statement":"http post"}],
                "phases":["connect"],"needs_file":null,"operations":["users.get","users.create"]} ],
  "needs": [ … only with --needs / --check-files: {"operation_id", "files": [NeededFile…]} … ],
  "bootstrap": [ … only with --include-bootstrap … ]
}
```

(Schema sketch; `|` marks alternatives.) `decision` is `null` unless `--check-policy`. `origin`, `phase`,
`requires_existing` and `secret` are present on every site (revision 8); CSV adds them as columns
(`origin` written `statement:file read` / `option:tls key_file`). `complete` is `false` when
any site is dynamic or opaque. `--format markdown` = the same tables as Markdown (for pasting into reviews);
`--format csv` = one row per site.

**`rivet policy generate` — least-privilege draft.**

```sh
rivet --file app.rivet policy generate [ID ...|--all] [--output policy.json]
```

- Builds policy.json v1 from the manifest: one grant per (capability, target), `access` narrowed to exactly the
  verbs used.
- `exact` targets as-is; `param_dependent` URLs → origin `scheme://host:port`; `param_dependent` paths → their
  derived glob; `network.deny_private_ranges: true`.
- `dynamic` / `opaque_*` sites are **not** granted: they are listed on stderr as review items and the command
  exits 7 (the draft is still written).
- Writes stdout unless `--output`; `--output` refuses to overwrite an existing file (`conflict.exists`, exit 4) —
  review, then rename.
- Never includes `serve` auth or secrets. The draft is a starting point for human review, not approval.
- Revision 8: option-derived file sites (`tls ca_file`, `tls cert_file`, `tls key_file`, `body file`,
  operation-level `descriptor`) are granted as their **exact path** with `access: ["read"]`, never widened to a
  directory glob such as `./certs/**`. A `param_dependent` option path is not granted; it is a review item
  (exit 7). Bootstrap `descriptor`/`schema` files are never granted.

```text
 IoManifest site                                 knowledge         draft grant
 ----------------------------------------------  ----------------  ---------------------------------------------
 connect GET https://api.example.com/users/{id}  param_dependent   allow_network https://api.example.com:443
 create ./out/user.json                          exact             allow_write ./out/user.json access [create]
 delete ./out/notes/{name}.json                  param_dependent   allow_delete ./out/notes/*.json access [delete]
 connect ${base_url}                             dynamic           (none) -> stderr review item, exit 7
 read ./certs/client.key (tls key_file)          exact             allow_read ./certs/client.key access [read]
 read ./certs/{tenant}.key (tls key_file)        param_dependent   (none) -> review item: no glob for option files
```

Example output for the snapshot bundle (`users.get` + `users.snapshot`):

```json
{
  "version": 1,
  "grants": [
    {"capability": "allow_network", "targets": ["https://api.example.com:443"]},
    {"capability": "allow_write",   "targets": ["./out/user.json"], "access": ["create"]}
  ],
  "network": {"deny_private_ranges": true}
}
```

**Every surface.** The manifest and draft are built from the immutable catalog; every surface returns the same
data under the usual catalog/authorization rules.

| Surface | Manifest | Policy draft |
|---|---|---|
| CLI | `rivet io …` | `rivet policy generate …` |
| Built-in operation | `rivet.io` `{ids?: [text], all?: boolean, by?: text, kind?: text, access?: [text], check_policy?: boolean, needs?: boolean}` → IoManifest | `rivet.policy.generate` `{ids?: [text], all?: boolean}` → `{policy: {…}, review: [site…], complete: bool}` (never writes files) |
| HTTP | `GET /v1/io?by=target&kind=file&check_policy=true`, `GET /v1/io?needs=true` | `POST /v1/policy/generate` |
| MCP | tool `rivet.io` | tool `rivet.policy.generate` |
| Library | `rt.io(IoQuery) -> IoManifest` (`IoQuery.needs`, `IoQuery.check_files`) | `rt.generate_policy(&[ids]) -> PolicyDraft` |

**Serve exposure rule.** Inventories reveal internal URLs and paths, so `rivet.io` and `rivet.policy.generate`
are **not** callable over the network unless the principal's `serve.principals.<name>.operations` explicitly
lists them. A `*` or `demo.*` pattern does not match `rivet.*`. The loopback principal `local` may call them.

```text
 principal            operations list                          rivet.io / rivet.policy.generate
 -------------------  ---------------------------------------  ---------------------------------
 local (loopback)     (any / none)                             allowed
 ada (bearer)         ["demo.*", "users.get"]                  403 permission
 ada (bearer)         ["*"]                                    403 permission (no rivet.* match)
 ci  (mtls)           ["users.get", "rivet.io"]                rivet.io allowed; policy.generate 403
```

**Runtime trace link (planned vs actual).** Every trace attempt record carries its `effect_id`, so
`rivet trace show REQ --json` rows join to manifest rows. `rivet io --trace REQ` prints the manifest with an
`ATTEMPTS` column (count, last decision).

```text
 manifest site (planned)                 trace attempts (actual, joined on effect_id)
 --------------------------------------  -------------------------------------------
 users.get#1       connect GET ...{id}   ATTEMPTS 1  last: allowed
 users.snapshot#1  create ./out/user.json ATTEMPTS 1  last: allowed
 notes.delete#1    delete ./out/notes/.. ATTEMPTS 0  (not reached by REQ)
```

#### Option-derived file sites and `io --needs`

Revision 8 (PLAN-2026-0001 TASK-005, approved in
[ADR-0001](../../decisions/adr-0001-approve-rivet-runtime-design.md)). Files named by **option lines**, not only
by `file …` statements, are I/O the operation performs, and a missing certificate or key is the most common
reason a connection fails before it starts. The manifest therefore lists them, and `rivet io --needs` answers
"which files must exist before this operation can run?".

**Every file-valued option line is its own site:** kind `file`, access `["read"]`, capability `allow_read`.

| Option line | Blocks | `phase` | `secret` |
|---|---|---|---|
| `tls ca_file PATH` | `http`, `with http`, `with websocket`, `with quic`, `grpc`/`mcp` (http transport) connectors, `with tcp` (Stage C) | `before_connect` | false |
| `tls cert_file PATH` (client certificate) | same | `before_connect` | false |
| `tls key_file PATH` (client key) | same | `before_connect` | **true** |
| `body file PATH`, or a `file NAME PATH` part of `body multipart` (upload source) | `http`, `with http` | `body` | false |
| `descriptor PATH` inside an operation-level block | operation blocks (e.g. a Stage C codec) | `load` | false |

`tls …` lines are valid in their block's own stage: HTTP is Stage A; WebSocket, QUIC and gRPC are Stage B; TCP TLS
and mTLS stay Stage C (S24). A connector's option sites appear under every operation that calls the connector.
Connector-level `descriptor` (gRPC) and `schema` (MCP) files remain **bootstrap** reads. They are listed only
with `--include-bootstrap` and are never granted by policy.json, but they carry the same new fields
(`origin: {"option": "descriptor"}`, `phase: "load"`, `requires_existing: true`). A missing one fails the bundle
load itself (not_found, exit 4).

**New fields on every site.**

| Field | Values | Rule |
|---|---|---|
| `origin` | `{"statement": "file read"}` or `{"option": "tls key_file"}` | The statement (`file read`, `http get`, `with tcp`, `grpc`, `secret`, `command`, …) or option line (`tls ca_file`, `body file`, `endpoint`, `transport command`, …) that produced the site |
| `phase` | `load` \| `before_connect` \| `connect` \| `body` \| `cleanup` | Bundle load; files read to build a connection; opening the connection or spawning the process; statements and body options; `finally` blocks and scope disposal |
| `requires_existing` | bool | File sites only (false otherwise). `true` when the file must exist when the operation starts. A missing file fails with not_found (exit 4) at that site. For `load`/`before_connect` files this happens before the block dials, so the block causes no effect. `false` for files the site creates (create, write, append), for `missing ok`, and for files the **same operation creates earlier** |
| `secret` | bool | `true` when the content read is secret material (`tls key_file`, env reads by `secret`). The **path is still shown**; the content is never read by `io` and is redacted in traces |

```text
 operation render.status                          sites (in phase order)
 ─────────────────────────────────────────────    ───────────────────────────────────────────────────────────
 response = http post "https://render…/status"    ┌ before_connect  file read ./certs/ca.pem      tls ca_file
     tls ca_file   "./certs/ca.pem"          ───▶ │ before_connect  file read ./certs/client.pem  tls cert_file
     tls cert_file "./certs/client.pem"           │ before_connect  file read ./certs/client.key  tls key_file  secret
     tls key_file  "./certs/client.key"           │ connect         network connect POST https://render.example.com:443
     body json {action: "status"}                 └ body            (request/response on the open connection)
 end                                                 missing key ──▶ not_found, exit 4, no connection attempted
```

**`rivet io --needs`** (static; no I/O) lists, per operation, the sites with `requires_existing: true`, grouped
with their origin. Files the same operation creates earlier are excluded; a callee's needs appear under the caller
marked `(via callee)`; `param_dependent` paths are shown as their template.

```text
render.status needs, before it can run:
  ./certs/ca.pem        (tls ca_file)
  ./certs/client.pem    (tls cert_file)
  ./certs/client.key    (tls key_file, secret)
report.upload needs, before it can run:
  ./data/template.json  (file read)
demo.echo needs no existing files.
```

(`report.upload` also uploads `./out/report.json` with `body file`, but creates it first, so it is not a need.)
JSON adds `IoManifest.needs: [{operation_id, files: [NeededFile{path, effect_id, origin, phase, secret, knowledge,
via, source, status}]}]`. The same view is `rivet.io {needs: true}`, `GET /v1/io?needs=true`, MCP `rivet.io` and
`rt.io(IoQuery { needs: true, .. })`.

**`rivet io --check-files`** implies `--needs` and **performs real I/O**: a metadata probe (stat plus a
readability check; contents are never opened, so key files stay unread) for each needed exact path, **through the
policy broker**. Each probe needs `allow_read` with access `stat` (or a grant without an `access` list) in the
effective policy (host ceiling ∩ policy.json). It is a CLI flag and a library option only, never a network
parameter of `rivet.io`, because it touches the server host's files.

```text
 needed file ──▶ broker: allow_read + stat granted? ──no──▶ not_permitted (not touched)  ─┐
                        │ yes                                                              │ exit 3
                        ▼                                                                  │
                 stat + readability ──▶ present │ missing │ unreadable ────────────────────┤
                                                    │           └──────────────────────────┘
                 param_dependent / dynamic ──▶ not_checkable (not probed, no exit effect)
                                                    └──▶ exit 4 (not_found) unless exit 3 applies
```

```text
render.status needs, before it can run:
  ./certs/ca.pem        (tls ca_file)            present
  ./certs/client.pem    (tls cert_file)          present
  ./certs/client.key    (tls key_file, secret)   missing
stderr: 2 present · 1 missing
exit 4
```

Exit codes: `3` when any file is `not_permitted` or `unreadable` (checked first); else `4` when any needed file is
`missing`; else the ordinary `io` exit code. `policy generate` grants the option-derived paths exactly with
`access: ["read"]`; add `"stat"` by hand (or a separate grant) to allow the `--check-files` pre-flight. Examples:
[REF-2026-0002 S154–S159](../../references/ref-2026-0002-language-and-usage.md#s154--trust-a-private-ca-for-one-https-call).

Feasibility: **proven in design** (a projection of the effect graph already built for Increment 5 plus the
policy evaluator of Increment 16; the only new runtime I/O is the brokered metadata probe of `--check-files`).

## Implementation Design

### Environment and Feasibility

Status per risky item: **proven** (evidence in hand), **experiment needed** (spike defined, not run), **blocked** (cannot proceed until an external condition is met).

| Environment / item | Capability | Evidence | Status | Unknown and resolution |
|---|---|---|---|---|
| Capy licence | Legal right to depend on and redistribute Capy | LICENSE source-available vs Cargo.toml MIT | **resolved** (G-LIC closed 2026-09-28, ADR-0001) | Owner authorized use in Rivet; upstream `LICENSE` cleanup is owner housekeeping |
| Capy pinned commit | Recovering parse, line/column spans, prefix calls | `Library::parse` public API inspected | **experiment needed** | Spike gate: S01–S159 and all docs/demos `.rivet` files parse cleanly |
| Rust embedding on Linux/macOS/Windows | Tokio async host; in-memory compile | Capy public Rust API inspected | experiment needed | Lock MSRV/dependency set after grammar spike |
| policy.json loader, declared outputs, error registry | Pure data validation and projection | Design only; standard JSON Schema | proven (design) | None beyond implementation |
| I/O manifest, `--check-policy`, policy drafts | Projection of the compiled effect graph plus the policy evaluator | Design only; no runtime I/O | proven (design) | None beyond implementation |
| Unified serve listener | REST/SSE/poll/WS/MCP on one socket | Established protocols | experiment needed | Prove WS upgrade + MCP Streamable HTTP routing on the chosen HTTP crate |
| CLI/HTTP/MCP process | Broker and principal context | Shared dispatcher design | experiment needed | Parity and isolation tests before exposure |
| Sandboxed subprocess worker | OS-enforced child capabilities | No backend implemented | **blocked** per platform | Fail closed (`unsupported.sandbox_backend`) until conformance passes |
| Remote MCP | Negotiated protocol and opaque remote effects | Pinned MCP specification | experiment needed | Test session cancellation, capability mismatch, schema changes |
| OAuth provider and credential backend | Code+PKCE/device/client credentials and versioned secret store | RFC baselines above | experiment needed | Provider fixtures and secure-store adapter; cross-process rotation test |
| UDP/QUIC/HTTP3 on each supported OS | Truncation visibility, QUIC v1 TLS/ALPN, HTTP/3 | Standards above | experiment needed | Prove migration hooks, early-data disable and cleanup in selected crates |
| gRPC dynamic descriptors | Four modes from a pinned FileDescriptorSet | gRPC/ProtoJSON specs | experiment needed | Dynamic-message crate spike |
| Conditional file update | Version guard against external writers | Platform APIs vary | experiment needed | Else `unsupported.conditional_update` |

### Methods by Use Case

| Change | Use cases | Execution method | Positive path | Negative path | Feasibility |
|---|---|---|---|---|---|
| C-01 | UC-01, UC-10 | Parse → check diagnostics → lower → validate → freeze | Program/catalog | Source diagnostic, zero I/O | Grammar spike required |
| C-02 | UC-02, UC-03, UC-10 | Resolve/auth → validate → drive → validate completion | Same result everywhere | Structured error/terminal stream | New dispatcher required |
| C-03 | UC-03, UC-05, UC-10 | Scope owns resources and cancellation tree | Await cleanup | Preserve primary and suppressed errors | Fault harness required |
| C-04 | UC-04 | Broker permit → confined handle → mode operation | Explicit CRUD result | Guard/path/durability refusal | Conditional update backend constrained |
| C-05 | UC-08, UC-09 | Effect graph → intersect policy → per-attempt permit | Traceable allowed effect | Deny before effect | Process backend unknown |
| C-06 | UC-06 | Snapshot → negotiate → correlate → adapt result | Tools/resources/prompts | Typed remote/schema fault | Protocol fixtures required |
| C-07 | UC-07 | Validate dependencies → ready queue → join | Typed named outputs | Cancel/block/partial report | New scheduler required |
| C-08 | UC-05 | Acquire transport → frame/decode → return/cleanup | Protocol-visible exchange | EOF/limit/platform failure | Crate selection during plan |
| C-09 | UC-01–22 | Trace docs → executable fixtures → gates | Reviewable release evidence | No release with missing evidence | Documentation available now |
| C-10 | UC-11 | Bind principal/profile → authorize → begin/complete or refresh → atomic credential commit → opaque lease | Typed status and authenticated request | Invalid state, refresh uncertainty, denied token endpoint | Provider/store fixtures required |
| C-11 | UC-12 | Permit peer/bind → scoped socket → bounded datagram exchange → cleanup | Whole message and peer metadata | Truncation, loss timeout, denied reply target | OS loopback/multicast fixtures required |
| C-12 | UC-13 | Permit origin → verified TLS/ALPN → scoped streams/datagrams → checked path changes → join | Independent bounded streams | Reset, mismatch, unnegotiated DATAGRAM, migration deny | QUIC fixture and migration hooks required |
| C-13 | UC-14 | Resolve auth → select explicit HTTP version → permitted transport → shared response contract | Normal HTTP response.version=3 | Strict H3 refusal or unsafe fallback refusal | HTTP/3 and ambiguous-send fixtures required |
| C-14 | UC-16 | Descriptor/mode/input validation → permit → scoped native RPC → checked messages/trailers → cleanup | Four modes preserve final status | Schema, late status, metadata and deadline errors | Dynamic descriptor/Rust adapter spike required |
| C-15 | UC-15 | Parse every declaration → build documented schemas → reject collisions → freeze catalog | Same IDs/metadata everywhere | Atomic rejection with source spans | Capy grammar extensions required |
| C-16 | UC-17 | Authorize catalog → MCP projection → named tools/call → shared dispatcher/session driver | Direct named tools with faithful schemas | Private/unauthorized ID and protocol faults | MCP server conformance fixtures required |
| C-17 | UC-18 | Open principal-bound child scope → ordered send/read → half-close → terminal/cleanup | Interactive duplex across every access point | Full queues, idle expiry, cursor/sequence conflict | Fault and concurrency fixtures required |
| C-18 | UC-19, UC-02 | Lower `output`/`field`/`error` → OutputSpec in RegistryEntry → validate result before Completion → project to CLI/HTTP/MCP/Rust | Same schema on every surface; valid result returned | `output.invalid` (500/exit 5) with effects preserved; undeclared code warning | Proven in design; nested block in Capy spike |
| C-19 | UC-08 | Discover `policy.json` beside entry (or `--policy PATH`) → validate v1 → intersect with host ceiling and request restriction | Effective policy explained by `policy explain` | No file → deny all effects; malformed → `policy.invalid` exit 2 | Proven in design |
| C-20 | UC-20, UC-17, UC-18 | Bind one listener → mount enabled surfaces → authenticate → principal → operation authorization → shared dispatcher/session driver | Same Completion on REST/SSE/poll/WS/MCP | `serve.auth_required`, 401/403/404/429 | Shared-listener spike required |
| C-21 | UC-01, UC-15 | Capy parse → `capy_parser.rs` converts to `SyntaxTree` → lower prefix calls, quoted durations, component-aware URL templates | Clean parse of all samples | Diagnostic with line/column; `syntax.option_after_body` | Capy spike gate |
| C-22 | UC-02, UC-07 | Map every error to registry row; enforce global limits; run DAG node state machine → DagCompletion | Consistent status/exit; bounded concurrency | `limit.call_depth`, `check.call_cycle`, blocked/skipped nodes | New scheduler required |
| C-23 | UC-21, UC-09, UC-08 | Effect graph → per-site access verbs + capability → normalize targets → IoManifest → group/filter/format; optional evaluate against effective policy; join trace attempts on `effect_id`; classify `origin`/`phase`/`requires_existing`/`secret` per site; `--needs` groups requires-existing file sites per operation; `--check-files` probes them through the broker (`FileProbe.stat`) | Same IoManifest on CLI/HTTP/MCP/library; decision per site; needed files per operation | Exit 3 (denied/partial, or `--check-files` not permitted/unreadable), exit 4 (`--check-files` missing file), exit 7 (`--strict` unknowns), `policy.invalid` for a verb outside its capability, 403 for a network principal without explicit `rivet.io` | Proven in design |
| C-24 | UC-22 | IoManifest → group by (capability, target) → narrow `access` → widen `param_dependent` to origin/glob → policy.json v1 draft; unknown sites → review list | Least-privilege draft on stdout or a new file | Exit 7 with review list; `conflict.exists` exit 4 on existing `--output` | Proven in design |

### Added, Changed and Removed Contracts

| CRUD | Kind | Contract | Scope / consumers | Requirements |
|---|---|---|---|---|
| CREATE | Language | operation, pipeline, param, output (+ `field` block), emits, receives, error, prefix-call `(request …)`, with, scope, DAG, yield and protocol statements (see syntax table, Increment 1) | Per compiled bundle; reference defines forms | R1–R5, R7, R9, R10, R12, R23 |
| CREATE | Types | SourceBundle, SyntaxTree, CompiledProgram, RegistryEntry, OutputSpec, IoManifest, IoSite, IoQuery, PolicyDraft, Request, Context, Completion, DataEvent, RivetError, EffectIntent, Permit, Policy, TraceEvent, DagCompletion | Immutable data in domain; schemas above | R1, R4, R6, R8–R11 |
| CREATE | Runtime state | request pending/running/cleaning/terminal; DAG node statuses | Per request, supervisor-owned | R3, R4, R10 |
| CREATE | CLI | request, list, describe, outputs, check, io, graph, policy explain, policy generate, trace show/export, connectors sync, serve | Maps to `rivet.*` operations or host bootstrap | R6–R9, R14, R26 |
| CREATE | Flags/config | --file, --params, --params-file, --stream, --input-jsonl, --policy PATH, --timeout, --json, --all, --outputs, --transitive, --strict, --strict-docs, --include-bootstrap, --kind, --listen, --stdio, --endpoint; `io` flags --by, --access, --format, --check-policy, --trace, --needs, --check-files; `policy generate --output`; `policy.json` (auto-discovered beside the entry file) | Full command examples in reference; no ambient RIVET_* vars; no grant strings on argv | R6, R8, R11, R23–R26 |
| DELETE | Flags (draft only) | `--sandbox "…"` (replaced by policy.json), `serve --transport …` and `serve --mcp` (replaced by one serve mounting every surface) | Removed from revisions 1–4 drafts; never shipped | R24, R25 |
| CREATE | Config file | `policy.json` v1: `version`, `grants` / `deny` (each `{capability, targets, access?}`), `network.deny_private_ranges`, `limits.{max_concurrent_requests,max_call_depth,max_buffered_bytes}`, `approved.{snapshots,overlaps}`, `serve.{surfaces,auth,principals}` | Unknown keys and verbs outside their capability rejected (`policy.invalid`, exit 2) | R11, R24, R25, R26 |
| CREATE | HTTP | POST /v1/request (unary or SSE); GET /v1/operations; GET /v1/operations/{id}; GET /v1/operations/{id}/outputs; GET /v1/io (incl. `needs=true`); POST /v1/policy/generate; polling POST /v1/requests, GET /v1/requests/{id}/events, POST /v1/requests/{id}/input, /finish_input, /cancel | Authenticated principal; one listener (Increment 17); `/v1/io` and `/v1/policy/generate` only for principals listing them explicitly (Increment 18) | R8, R9, R22, R23, R25, R26 |
| CREATE | WebSocket | GET /v1/ws, subprotocol `rivet.v1`; frames request/input/finish_input/cancel and data/result/error keyed by `ref` | Connection-owned refs, max 8, 16-frame queues | R22, R25 |
| CREATE | MCP | /mcp POST/GET/DELETE and `serve --stdio`; direct named operation tools (canonical); built-in `rivet.list/describe/outputs/request/sessions.*/auth.*/io/policy.generate` | Standard MCP transport/session contract | R7, R8, R21, R23, R26 |
| CREATE | Storage | Optional schema snapshot and explicit trace export; trace attempt records carry `effect_id` | Brokered paths; no hidden database/cache | R6, R7, R11, R13, R26 |
| UPDATE | Design behavior | PROJECT explicit close → lexical with; ambiguous transform → response.body | Proposed supersession only; original brief preserved | R2–R4 |
| DELETE | Public language capability | Manual resource close and implicit shell from proposed language | No shipped API to remove | R3, R13 |
| CREATE | Auth declarations/operations | `auth NAME oauth2`; `auth PROFILE account ACCOUNT`; `rivet.auth.begin/complete/status/disconnect/cancel`; CLI `auth begin/complete/status/disconnect/cancel` aliases; complete may return `{state:"pending"}` | Profile snapshot; principal-bound transaction/store; no raw token API | R16 |
| CREATE | CLI flag | `--endpoint URL` routes requests/aliases to an existing runtime; mutually exclusive with local `--file` execution | Host memory store survives between calls; caller auth from configured host credential source | R16, R8 |
| CREATE | Policy targets | `allow_auth` targets `PROFILE/ACCOUNT/use\|manage\|status`; `allow_credentials` targets `PROFILE/ACCOUNT`; `quic://` network targets (all in policy.json) | Intersects host/caller policy; no principal escalation | R15–R18, R11 |
| CREATE | Transport options | `udp bind`, `receive_from/send_to`; `quic`, `alpn`, `max_streams`, `migration`, `datagrams`, `open/accept uni/bidi`, `finish_send`; HTTP `version 3/prefer [...]` | Lexical resources and request deadline | R15, R17, R18 |
| CREATE | Data/storage/errors | AuthChallenge/CompleteInput/CredentialStatus/Lease; DatagramPlan/Result; QuicPlan/Result; auth transaction/store state and typed auth/UDP/QUIC/version errors | Expiring transactions, secret storage and per-request transport state as defined in increments 9–11 | R15–R18 |
| CREATE | Operation metadata | `name`, `description`, parameter `description`, `private true`, `receives TYPE`; RegistryEntry source/name/schemas | Per declaration in a multi-operation file; all surfaces | R20–R22 |
| CREATE | MCP catalog | Required direct named tools and generic request/session tools; `_meta` delivery marker | Authorized principal and catalog snapshot; standard /mcp transport | R21 |
| CREATE | gRPC syntax/policy | `connector ... grpc`, descriptor/service, `grpc connector.Method`, message/metadata/auth, scoped send/result/completion; `allow_grpc` | Pinned method descriptor, TLS origin and scope | R19 |
| CREATE | Streaming APIs | `rivet.sessions.open/send/finish_input/read/cancel`, `scope.stream`, `scope.duplex` | Host-owned bounded request scope; projected as /v1/request, polling routes and WS frames | R20–R22 |
| CREATE | Errors | Error registry (Increment 2): new codes `output.invalid`, `policy.invalid`, `serve.auth_required`, `limit.call_depth`, `check.call_cycle`, `file.hardlink_refused`, `syntax.option_after_body`, `stream.required`, `stream.input_required` (renamed from `stream_required`/`stream_input_required`); exit 7 for incomplete strict inspection and `policy generate` review items; `conflict.exists` (`policy generate --output` onto an existing file); `io --check-policy` exit 3; `io --check-files` exit 4 (a needed file is missing) and exit 3 (stat not permitted or file unreadable) | All surfaces | R4, R8, R23–R26 |
| CREATE | Domain data | ParameterSpec, GrpcPlan/Result, SessionOpenInput/Receipt/SendInput/Ref/Ack/ReadInput/Batch | JSON catalog/session values; secret/raw handles excluded | R19–R22 |

### Architecture, Data, State and Interaction Visuals

```text
orchestrator/                     composition; setup_cli/http/library/mcp
   | wires ports                 only bucket importing all others
   +---- io/                     surface parsing/encoding; imports domain only
   +---- features/               pure plans/decisions; imports domain only
   +---- infra/                  Capy, broker, scheduler driver, transports
   +---- domain/                 values/plans/effects/errors; no internal imports

features -> EffectIntent -> injected driver/broker -> Permit -> adapter -> external I/O
                            ^                                  |
                            +-------- TraceEvent/result --------+
```

State machines in this proposal: request lifecycle (Increment 2), DAG node (Increment 7), OAuth transaction (Increment 9), session lifecycle (Increment 14), WebSocket ref (Increment 17).

A pure use case constructs/advances a plan from data and injected port outcomes. It does not create sockets, access Tokio globals, spawn tasks or construct adapters. Runtime scheduling and effect execution live in infra; orchestration provides ports. The Rust source root uses these five buckets under `src/`, as expected by VHCO's Rust layout. Crate entry points in `src/orchestrator/lib.rs` and `src/orchestrator/main.rs` are configured in Cargo to avoid extra source buckets. The `io/` bucket is explicit and contains no business workflows.

## Sample API Calls and CLI Commands

Save the `users.get` operation from [Increment 15](#increment-15--declared-outputs) in `app.rivet`, with this `policy.json` beside it. `api.example.com` is a documentation endpoint; replace it with the fixture API before running network examples.

```json
{"version": 1,
 "grants": [{"capability": "allow_network", "targets": ["https://api.example.com:443"]}]}
```

```sh
rivet --file app.rivet request users.get --params '{"id":42}'
# stdout: {"request_id":"req_01","trace_id":"tr_01","result":{"id":42,"name":"Ada"},"data_count":0,"effects":"none"}
# IDs vary; effects describes committed mutations, not whether network reads occurred.
rivet --file app.rivet list --json
rivet --file app.rivet describe users.get --json
rivet --file app.rivet outputs users.get
rivet --file app.rivet io --all --transitive --format json
rivet --file app.rivet io --by target
# TARGET                        ACCESS        CAPABILITY     ORIGIN    PHASE    NEEDS FILE  USED BY
# https://api.example.com:443   connect GET   allow_network  http get  connect  —           users.get
rivet --file app.rivet io --needs
# users.get needs no existing files.   (a `tls ca_file` line would be listed here with its origin)
rivet --file app.rivet io --check-files
# stats each needed file through the broker; exit 4 if one is missing, 3 if stat is not granted
rivet --file app.rivet io --check-policy
# adds DECISION (allowed|denied|partial|unknown); exit 3 if any reachable site is denied or partial
rivet --file app.rivet policy generate --output policy.draft.json
# least-privilege draft: allow_network https://api.example.com:443; review, then rename to policy.json
# existing policy.draft.json -> conflict.exists, exit 4; dynamic/opaque sites -> stderr review list, exit 7
rivet --file app.rivet --policy ./policies/empty.json request users.get --params '{"id":42}'
# policies/empty.json = {"version":1}; stderr ErrorEnvelope: permission.denied; exit 3; zero HTTP attempts
# (the same happens with no policy.json at all: deny-by-default)
rivet --file app.rivet serve --listen 127.0.0.1:8080
```

All HTTP JSON uses `Content-Type: application/json`. The examples below use the loopback bind with `serve.auth` absent (principal `local`); a non-loopback bind requires `bearer` or `mtls` auth. `Accept: text/event-stream` selects ordered SSE envelopes on `/v1/request`; otherwise unary JSON is used. A streaming operation requested as unary is rejected with `stream.required` (422), unless declared bounded collection is explicitly requested.

```sh
curl -sS http://127.0.0.1:8080/v1/request   -H 'Content-Type: application/json'   -d '{"id":"users.get","params":{"id":42}}'
# 200: {"request_id":"req_01","trace_id":"tr_01","result":{"id":42,"name":"Ada"},"data_count":0,"effects":"none"}

curl -sS http://127.0.0.1:8080/v1/operations
# 200: {"operations":[{"id":"users.get","streaming":false}],"next_cursor":null}

curl -sS http://127.0.0.1:8080/v1/operations/users.get
# 200: {"id":"users.get","input":{"type":"object","required":["id"],"additionalProperties":false,"properties":{"id":{"type":"integer","minimum":1}}},"output":{"type":"object","description":"The requested user.","required":["id","name"],"additionalProperties":false,"properties":{...}},"emits":null,"errors":[{"code":"users.not_found","description":"No user has this ID."}],"effects":[{"capability":"allow_network","target":"https://api.example.com:443","knowledge":"param_dependent"}]}

curl -sS http://127.0.0.1:8080/v1/operations/users.get/outputs
# 200: {"id":"users.get","output":{...schema...},"emits":null,"receives":null,"errors":[{"code":"users.not_found","description":"No user has this ID."}]}

curl -sS 'http://127.0.0.1:8080/v1/io?by=target&check_policy=true'
# 200: IoManifest with "decision" per site (principal "local" on loopback; a network principal needs
# "rivet.io" listed explicitly in serve.principals.<name>.operations, otherwise 403)

curl -sS -X POST http://127.0.0.1:8080/v1/requests/ses_01/cancel   -H 'Content-Type: application/json' -d '{}'
# 200 after bounded cleanup: {"session_id":"ses_01","request_id":"req_01","state":"cancelled"}; already terminal -> 200 with its terminal state; unknown -> 404

curl -N http://127.0.0.1:8080/v1/request   -H 'Content-Type: application/json' -H 'Accept: text/event-stream'   -d '{"id":"chat.reply","params":{"prompt":"Hello"}}'
# 200 text/event-stream; each event has id, event and JSON data:
# id: 1
# event: data
# data: {"request_id":"req_02","trace_id":"tr_02","seq":1,"type":"data","data":"Hello"}
#
# id: 2
# event: result
# data: {"request_id":"req_02","trace_id":"tr_02","seq":2,"type":"result","result":"Hello"}
```

Polling and WebSocket on the same listener:

```sh
curl -sS http://127.0.0.1:8080/v1/requests -H 'Content-Type: application/json' \
  -d '{"id":"chat.reply","params":{"prompt":"Hello"}}'
# 202: {"session_id":"ses_02","request_id":"req_05",...,"events_url":"/v1/requests/ses_02/events"}
curl -sS 'http://127.0.0.1:8080/v1/requests/ses_02/events?after_seq=0&wait_ms=1000'
# 200: {"session_id":"ses_02","events":[{"seq":1,"type":"data","data":"Hello"},{"seq":2,"type":"result","result":"Hello"}],"last_seq":2,"terminal":true}
```

```text
 WebSocket GET /v1/ws  (Sec-WebSocket-Protocol: rivet.v1)
 -> {"type":"request","ref":"c1","id":"demo.add","params":{"a":2,"b":3}}
 <- {"type":"result","ref":"c1","completion":{"request_id":"req_06","trace_id":"tr_06","result":5,"data_count":0,"effects":"none"}}
```

Malformed request JSON is 400; every other HTTP status follows the [error registry](#increment-2--requests-streams-and-errors). Explicit client cancellation before a unary response uses 409 with `cancelled`; once stream headers are sent, a terminal error is an SSE event (or poll/WS terminal event), not a second HTTP status. Rivet HTTP disconnect cancels a unary/SSE request; polling sessions survive reconnect; WebSocket close cancels its refs.

Every error body is `{"request_id":"req_03","trace_id":"tr_03","error":{"kind":"permission","code":"permission.denied","message":"allow_network is required","retryable":false,"effects":"none"}}`, augmented by source/cause fields when available. CLI exit codes: 0 ok, 2 syntax/validation/usage/config (including policy.json errors), 3 permission, 4 not found/conflict, 5 dependency/runtime/unsupported/output invalid, 6 timeout, 7 incomplete strict inspection, 130 cancelled. Unary stdout contains only Completion JSON; errors and logs use stderr. Stream stdout is NDJSON envelopes, with nonzero exit on a terminal error.

MCP transport route examples (session IDs are illustrative; substitute the returned header; this fixture supports session deletion but not independent GET event listening). The canonical tool call names the operation directly:

```sh
curl -i http://127.0.0.1:8080/mcp   -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream'   -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"example","version":"1"}}}'
# 200 + MCP-Session-Id: demo-session
# {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"rivet","version":"0.1.0"}}}

curl -i http://127.0.0.1:8080/mcp   -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream'   -H 'MCP-Session-Id: demo-session' -H 'MCP-Protocol-Version: 2025-11-25'   -d '{"jsonrpc":"2.0","method":"notifications/initialized"}'
# 202, empty body

curl -sS http://127.0.0.1:8080/mcp   -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream'   -H 'MCP-Session-Id: demo-session' -H 'MCP-Protocol-Version: 2025-11-25'   -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"users.get","arguments":{"id":42}}}'
# 200: {"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"<serialized Completion>"}],"structuredContent":{"request_id":"req_04","trace_id":"tr_04","result":{"id":42,"name":"Ada"},"data_count":0,"effects":"none"},"isError":false}}

curl -i http://127.0.0.1:8080/mcp -H 'Accept: text/event-stream'   -H 'MCP-Session-Id: demo-session' -H 'MCP-Protocol-Version: 2025-11-25'
# 405, no independent event stream supported by this fixture

curl -i -X DELETE http://127.0.0.1:8080/mcp   -H 'MCP-Session-Id: demo-session' -H 'MCP-Protocol-Version: 2025-11-25'
# 204, empty body; unknown/expired session -> 404
```

The library form is the canonical sketch in Increment 2. Further negative examples appear in [the sample reference](../../references/ref-2026-0002-language-and-usage.md). The executable is `rivet` (AGENTS' `xllm` examples name another project).

### Auth and transport operation calls

The auth management IDs are available through every authorized surface. `--endpoint URL` routes a CLI call to an existing runtime and cannot be combined with `--file`; it uses the host's configured client authentication, never OAuth resource tokens as Rivet server credentials. Authorization URLs/codes are sensitive challenge responses; keep them out of persistent logs.

```sh
rivet --endpoint http://127.0.0.1:8080 auth begin crm_user --account ada
# Completion.result: {"transaction_id":"auth_01","authorization_url":"https://auth.example.com/authorize?client_id=rivet-desktop&response_type=code&state=...&code_challenge=...&code_challenge_method=S256","expires_at":"2026-09-28T12:10:00Z"}
rivet --endpoint http://127.0.0.1:8080 auth complete --params-file ./callback.json
# callback.json: {"transaction_id":"auth_01","callback":{"code":"provider-code","state":"returned-state","redirect_uri":"https://app.example.com/oauth/callback","issuer":"https://auth.example.com"}}
# Completion.result: {"profile":"crm_user","account":"ada","state":"connected","scopes":["contacts.read"],"expires_at":"2026-09-28T13:00:00Z","generation":1}
rivet --endpoint http://127.0.0.1:8080 auth status crm_user --account ada
rivet --endpoint http://127.0.0.1:8080 auth cancel auth_02
# {"transaction_id":"auth_02","state":"cancelled"}
rivet --endpoint http://127.0.0.1:8080 auth disconnect crm_user --account ada
# disconnect result: {"profile":"crm_user","account":"ada","local_only":true,"generation":2}
```

```sh
curl -sS http://127.0.0.1:8080/v1/request -H 'Content-Type: application/json' \
  -d '{"id":"rivet.auth.begin","params":{"profile":"crm_user","account":"ada"}}'
# 200 Completion with AuthChallenge above; unauthorized profile -> 403
curl -sS http://127.0.0.1:8080/v1/request -H 'Content-Type: application/json' \
  -d '{"id":"rivet.auth.complete","params":{"transaction_id":"auth_device_01","wait":true}}'
# 200 Completion with connected CredentialStatus, or {"state":"pending",...} if the request deadline came first
# (transaction still open; call again); denied/expired grant -> 409 typed auth error
curl -sS http://127.0.0.1:8080/v1/request -H 'Content-Type: application/json' \
  -d '{"id":"rivet.auth.status","params":{"profile":"crm_user","account":"ada"}}'
# 200 Completion with sanitized CredentialStatus; never access/refresh tokens
curl -sS http://127.0.0.1:8080/v1/request -H 'Content-Type: application/json' \
  -d '{"id":"rivet.auth.disconnect","params":{"profile":"crm_user","account":"ada"}}'
# 200 Completion with DisconnectReceipt; storage failure -> 502 with auth.store_failed
```

Complete must reference a transaction returned by begin on the **same runtime** (the device example assumes a device-profile begin). OAuth lifecycle codes `auth.login_required`, `auth.access_denied`, `auth.callback_invalid`, `auth.transaction_expired` and `auth.refresh_uncertain` are `conflict` (409, exit 4), not caller auth 401; malformed callback input is 422/exit 2; token/store dependency failure is 502/exit 5; capability denial remains 403/exit 3.

```sh
rivet --file app.rivet request telemetry.status --params '{}'
# S81: result {"state":"ready"} from UDP fixture; timeout -> exit 6
rivet --file app.rivet request engine.status --params '{}'
# S94: result {"state":"ready"} from QUIC fixture; wrong ALPN -> exit 5
rivet --file app.rivet request items.h3 --params '{}'
# S99: result {"items":[],"version":3}; H3 unavailable -> exit 5
curl -sS http://127.0.0.1:8080/v1/request -H 'Content-Type: application/json' \
  -d '{"id":"engine.status","params":{}}'
# 200 Completion with {"state":"ready"}; QUIC dependency failure -> 502
```

Library: `rt.request("engine.status", json!({}), None).await?`; auth uses the same method and IDs. MCP: `tools/call {name:"engine.status", arguments:{}}`. No QUIC connection, UDP socket or OAuth token crosses this boundary.

### Catalog, every access point and gRPC

Save the three-operation catalog from Increment 12 as `catalog.rivet` (no policy.json is needed: the operations are pure). Each operation keeps its ID, parameter defaults, output and descriptions across surfaces.

```sh
rivet --file catalog.rivet check --strict-docs
rivet --file catalog.rivet list --outputs --json
rivet --file catalog.rivet describe demo.add --json
rivet --file catalog.rivet outputs --all
rivet --file catalog.rivet request demo.add --params '{"a":2,"b":3}'
# Completion: {"request_id":"req_01","trace_id":"tr_01","result":5,"data_count":0,"effects":"none"}
rivet --file catalog.rivet serve
# REST, SSE, polling, WebSocket and MCP on 127.0.0.1:8080
```

With the server running, call REST or the initialized MCP endpoint (replace demo-session with the returned session).

```sh
curl -sS http://127.0.0.1:8080/v1/operations/demo.add
# Descriptor includes id:"demo.add", name:"Add two integers", descriptions, parameter schemas and output {"type":"integer","description":"Sum of a and b."}.
curl -sS http://127.0.0.1:8080/v1/request -H 'Content-Type: application/json' \
  -d '{"id":"demo.add","params":{"a":2,"b":3}}'
# HTTP 200 Completion with result:5; missing a -> 422 validation.required.
curl -sS http://127.0.0.1:8080/mcp \
  -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' \
  -H 'MCP-Session-Id: demo-session' -H 'MCP-Protocol-Version: 2025-11-25' \
  -d '{"jsonrpc":"2.0","id":20,"method":"tools/list","params":{}}'
# result.tools includes demo.greet, demo.add and demo.health with title, descriptions, inputSchema and outputSchema.
curl -sS http://127.0.0.1:8080/mcp \
  -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' \
  -H 'MCP-Session-Id: demo-session' -H 'MCP-Protocol-Version: 2025-11-25' \
  -d '{"jsonrpc":"2.0","id":21,"method":"tools/call","params":{"name":"demo.add","arguments":{"a":2,"b":3}}}'
# result: {"content":[{"type":"text","text":"<serialized Completion>"}],"structuredContent":{"request_id":"req_02","trace_id":"tr_02","result":5,"data_count":0,"effects":"none"},"isError":false}
```

Full metadata examples are [S103–S108](../../references/ref-2026-0002-language-and-usage.md#s103--define-several-documented-operations-in-one-file). For stdio MCP, use `rivet --file catalog.rivet serve --stdio`.

For the gRPC fixture bundle in S110–S113, `grpc/policy.json`:

```json
{"version": 1,
 "grants": [{"capability": "allow_network", "targets": ["https://users.example.com:443"]},
            {"capability": "allow_grpc",    "targets": ["users/example.Users/GetUser"]}]}
```

```sh
rivet --file grpc/grpc.rivet request users.grpc_get --params '{"id":"42"}'
# Completion.result: {"id":"42","name":"Ada"}; non-OK trailers -> typed gRPC error, never success.
rivet --endpoint http://127.0.0.1:8080 request chat.exchange --params '{}' --input-jsonl - --stream <<'JSONL'
{"text":"hello"}
JSONL
# Concurrent input/output; stdin EOF half-closes; NDJSON ends in one terminal result/error.
```

Live HTTP and MCP clients drive sessions through `/v1/request` or tools/call with the five `rivet.sessions.*` IDs, or through the polling routes and WebSocket frames of Increment 17 — all projections of the same operations. [S114](../../references/ref-2026-0002-language-and-usage.md#s114--drive-a-duplex-call-through-http-session-operations) specifies curl requests for open/send/finish_input/read/cancel; [S116](../../references/ref-2026-0002-language-and-usage.md#s116--drive-a-streaming-operation-as-an-mcp-tool) the direct streaming tool receipt; [S115](../../references/ref-2026-0002-language-and-usage.md#s115--own-a-duplex-request-in-a-rust-scope) the library's scoped duplex interface.

## Expected Code and Documentation Changes

All source paths below are **planned**, not created by this proposal. Tests are colocated Rust modules to preserve the five-bucket layout. Port signatures are conceptual async interfaces carrying domain values; concrete trait details are resolved by the implementation plan without changing behavior.

| ID | Path | CRUD | Symbol / precise edit | Reason / changes | Requirements | Test / documentation |
|---|---|---|---|---|---|---|
| F-01 | `PROJECT.md`; supplied Go reference; pinned Capy sources | READ | Source brief and inspected APIs; preserve originals | C-01, C-02, C-09 | R1, R2, R9, R14 | Source record |
| F-02 | `Cargo.toml`; `src/orchestrator/lib.rs`; `src/orchestrator/main.rs` | CREATE later | Library/binary targets; `Program`, `Runtime` reexports; composition | C-01, C-02 | R1, R8, R9 | T-01, T-10 |
| F-03 | `src/domain/mod.rs`; `src/domain/contracts.rs` | CREATE later | Domain request/plan/error/effect data as specified above | C-01–C-07 | R1–R11, R13 | All typed tests |
| F-04 | `src/features/language/compile_program.rs`; `src/features/language/ports.rs`; `src/infra/rivet.capy`; `src/infra/capy_parser.rs` | CREATE later | `compile_program(SourceBundle, Parser) -> CompiledProgram`; `Parser.parse` | C-01 | R1, R2 | T-01 |
| F-05 | `src/features/registry/describe_operations.rs`; `src/features/registry/ports.rs`; `src/infra/registry.rs` | CREATE later | `Registry.describe(CatalogQuery) -> Catalog`; freeze IDs/schemas | C-01, C-02 | R2, R8 | T-02 |
| F-06 | `src/features/execution/request_operation.rs`; `cancel_request.rs`; `run_dag.rs`; `ports.rs` in same directory | CREATE later | Pure request validation, cancellation decision and DAG plan; `ExecutionDriver.drive` and `RequestControl.cancel` | C-02, C-03, C-07 | R3, R4, R9, R10 | T-02, T-03, T-07 |
| F-07 | `src/infra/execution_driver.rs`; `src/infra/scope_supervisor.rs` | CREATE later | Async scheduling, bounded queues, cleanup and error precedence | C-02, C-03, C-07 | R3, R4, R9, R10 | T-03, T-05, T-07, T-10 |
| F-08 | `src/features/files/apply_file_operation.rs`; `src/features/files/ports.rs`; `src/infra/file_access.rs` | CREATE later | `FileAccess.apply(FileOperation) -> FileResult`; confined CRUD | C-04 | R5, R11 | T-04 |
| F-09 | `src/features/policy/authorize_effect.rs`; `src/features/policy/ports.rs`; `src/infra/policy_broker.rs` | CREATE later | `PolicyEvaluator.evaluate(EffectIntent) -> Permit`; runtime target gate | C-05 | R6, R11, R13 | T-08 |
| F-10 | `src/features/audit/inspect_effects.rs`; `read_trace.rs`; `ports.rs` in same directory; `src/infra/trace_store.rs` | CREATE later | Inventory traversal; `TraceStore.read(TraceQuery) -> TraceResult`; sanitized sink | C-05 | R6, R11, R13 | T-09 |
| F-11 | `src/features/connectors/invoke_mcp.rs`; `src/features/connectors/ports.rs`; `src/infra/mcp_client.rs` | CREATE later | `McpClient.invoke(McpRequest) -> McpResult`; explicit snapshots | C-06 | R7, R8, R11 | T-06 |
| F-12 | `src/io/cli/mod.rs`; `src/io/http/mod.rs`; `src/io/library/mod.rs`; `src/io/mcp/mod.rs`; `src/orchestrator/setup_cli.rs`; `setup_http.rs`; `setup_library.rs`; `setup_mcp.rs` in same directory | CREATE later | Encode/decode the shared operations; register surfaces per setup file | C-02, C-06 | R7–R9 | T-02, T-06, T-10 |
| F-13 | `transports` feature: `src/features/transports/{exchange_http,exchange_socket,run_process,ports}.rs`; adapters `src/infra/http_adapter.rs`; `socket_adapter.rs`; `process_adapter.rs`; `codec.rs` in same directory | CREATE later | `transports.exchange_http`/`exchange_socket`/`run_process` use cases over the `HttpClient`, `SocketStream`, `ProcessRunner` and `Codec` ports; acquire/send/receive/decode through permits; annotate every edge | C-08 | R3, R12, R13 | T-05 |
| F-14 | Platform sandbox backend paths | DISCOVER before code | Inspect supported OS primitives; choose `src/infra/sandbox_<platform>.rs`; record exact chosen path and capability matrix in plan | C-05, C-08 | R11, R12 | T-08; platform evidence |
| F-15 | `vhco-contract.json`; `.vhco.json` | CREATE now, UPDATE with approved changes | Hand-authored design + Rust config; detailed todos, no claims implemented | C-09 | R14 | Human live review |
| F-16 | `README.md`; `docs/README.md`; `docs/index.md`; `docs/proposals/index.md`; `docs/proposals/draft/index.md`; `docs/references/index.md`; `docs/standards/index.md`; this proposal; both reference docs | CREATE now | Current design status, navigation, evidence and S01–S159 | C-09 | R14 | Documentation checks |
| F-17 | `vhco.json`; `vhco.html` | GENERATE after code | Model/explorer generated from annotations; never contract overwrite | C-09 | R14 | validate/sync/check/spec |
| F-18 | Colocated `<use_case>_test.rs`; `docs/testing/test-2026-0001-runtime-conformance.md`; `docs/reports/rpt-2026-0001-runtime-validation.md` | CREATE later | Tests T-01–T-26 and results; exact module registration in owning feature | C-09 | R1–R26 | Future test/evidence |
| F-19 | `src/features/auth/begin_authorization.rs`; `complete_authorization.rs`; `credential_status.rs`; `disconnect_account.rs`; `cancel_authorization.rs`; `acquire_credential.rs`; `ports.rs` in same directory; `src/infra/oauth_adapter.rs`; `src/infra/credential_store.rs` | CREATE later | `OAuthSessionDriver.begin/complete/status/disconnect/cancel` (`rivet.auth.cancel` → `cancel_authorization`: principal-bound, idempotent); `CredentialProvider.acquire -> CredentialLease`; detailed JSON todos and increment 9 | C-10 | R16, R3, R4, R6, R8, R11, R13 | T-11, T-15; S84–S93 |
| F-20 | `src/features/datagrams/exchange_datagrams.rs`; `src/features/datagrams/ports.rs`; `src/infra/udp_adapter.rs` | CREATE later | `DatagramDriver.exchange(DatagramPlan) -> DatagramResult`; whole-message/bind/multicast rules | C-11 | R15, R3, R4, R6, R8, R11 | T-12, T-15; S81–S83 |
| F-21 | `src/features/quic/exchange_quic.rs`; `src/features/quic/ports.rs`; `src/infra/quic_adapter.rs` | CREATE later | `QuicDriver.exchange(QuicPlan) -> QuicResult`; TLS/ALPN, stream scopes, DATAGRAM and path authorization | C-12 | R17, R3, R4, R6, R8, R11 | T-13, T-15; S94–S98 |
| F-22 | Planned `src/infra/http_adapter.rs`; `src/infra/codec.rs`; `src/infra/rivet.capy`; surface/setup files in F-12; domain data in F-03 | EXTEND planned CREATE | Add HTTP version/fallback and response.version; OAuth attachment, new syntax, endpoint CLI routing and auth operation registration | C-10–C-13 | R15–R18, R8 | T-11–T-15; S99–S102 |
| F-23 | Existing contract, proposal, reference, source record, README and indexes listed in F-15–F-16 | UPDATE now | Revision 2 source request, R15–R18, C-10–C-13, T-11–T-15 and S81–S102 | C-09–C-13 | R14–R18 | Link/metadata/traceability checks; no runtime test claim |
| F-24 | `src/features/grpc/invoke_rpc.rs`; `src/features/grpc/ports.rs`; `src/infra/grpc_adapter.rs` | CREATE later | `GrpcDriver.invoke(GrpcPlan) -> GrpcResult`; descriptor/message/stream/status handling per increment 13 | C-14 | R19, R3, R4, R6, R11, R13 | T-17; S110–S113, S117–S118 |
| F-25 | Planned grammar/compiler/registry/domain files F-03–F-05 | EXTEND planned CREATE | Multiple declarations, complete metadata and schema projection, atomic duplicate detection; no filename-derived IDs | C-15 | R20, R2, R8 | T-16; S103–S106, S109, S119 |
| F-26 | Planned `src/io/mcp/mod.rs`; `src/orchestrator/setup_mcp.rs`; CLI/HTTP setup files F-12 | EXTEND planned CREATE | Direct tools/list/call from shared authorized catalog; `/mcp` mounted by unified serve (F-31) and session delivery schemas | C-16 | R21, R7, R8, R20 | T-18; S107–S108, S116, S120 |
| F-27 | `src/features/sessions/open_session.rs`; `send_input.rs`; `finish_input.rs`; `read_events.rs`; `cancel_session.rs`; `ports.rs` in same directory; `src/infra/session_driver.rs`; F-12 surface files | CREATE later / EXTEND planned CREATE | SessionDriver open/send/finish_input/read/cancel; principal-bound cursors/queues, scope.duplex and concurrent CLI feeder | C-17 | R22, R3, R4, R6, R8, R9, R11 | T-19; S113–S116 |
| F-28 | Existing design/docs files in F-15–F-16; `docs/demos/README.md`, `docs/demos/manifest.json`, and twelve sample directories | UPDATE existing / CREATE samples now | Revision 3 request, contract and proposal R19–R22, C14–C17, T16–T19, 18 new samples S103–S120 and navigation; revision 4 materializes selected source/fixture/request files and per-folder READMEs (UQ-16, R14) | C-09, C-14–C-17 | R14, R19–R22 | Link/metadata/traceability checks only |
| F-29 | `src/features/registry/describe_outputs.rs`; planned F-03 domain (`OutputSpec`, `FieldSpec`, `DeclaredError`); F-04 grammar/lowering; `src/features/execution/request_operation.rs` (output check); F-12 surface files | CREATE later / EXTEND planned CREATE | `describe_outputs(OutputQuery) -> OutputSpec[]`; lower `output … end`/`field`/`error`; validate result before Completion; CLI `outputs`, `list --outputs`, HTTP `/outputs`, MCP `rivet.outputs` + `outputSchema`, `Runtime::outputs` | C-18 | R23, R2, R8, R20 | T-20; S103–S106 |
| F-30 | `src/features/policy/load_policy.rs`; `src/features/policy/ports.rs`; `src/infra/policy_file.rs`; F-12 CLI setup (`--policy PATH`; remove `--sandbox`) | CREATE later | `load_policy(PolicySource) -> Policy` (discover beside entry, validate v1, deny-by-default when absent); `Policy::from_file/from_json`; SSRF private-range defaults in `src/infra/policy_broker.rs` | C-19 | R24, R11, R13 | T-21; S66–S69 |
| F-31 | `src/io/ws/mod.rs`; `src/io/http/poll.rs`; `src/orchestrator/setup_serve.rs`; `src/features/serve/authenticate_principal.rs`; `src/features/serve/ports.rs`; `src/infra/serve_listener.rs` | CREATE later | One listener mounting REST/SSE/poll/WS/MCP; `Authenticator.authenticate(Request) -> Principal`; `serve.surfaces/auth/principals`; `--listen`, `--stdio`; remove `--transport`/`--mcp` | C-20 | R25, R8, R21, R22 | T-22; S114, S116 |
| F-32 | Planned `src/infra/capy_parser.rs`, `src/infra/rivet.capy`, `src/domain/syntax_tree.rs` | EXTEND planned CREATE | Only file importing Capy; converts `ParseResult` to Rivet `SyntaxTree`; prefix calls, quoted durations, interpolation/escape rules, component-aware URL templates, `syntax.option_after_body` | C-21 | R1, R2 | T-01, T-23 |
| F-33 | Planned `src/domain/errors.rs`; F-06/F-07 DAG and execution files; F-12 surface encoders | EXTEND planned CREATE | Error registry table (code→kind→HTTP→exit→retryable); global limits; DAG node state machine and `DagCompletion`; `check.call_cycle` | C-22 | R4, R8, R10 | T-07, T-24 |
| F-34 | This proposal; `docs/references/ref-2026-0001-request-and-evidence.md` | UPDATE now | Revision 5: UQ-17, R23–R25, UC-19–UC-20, C-18–C-22, Increments 15–17, review fixes; reference, demos, contract and indexes are updated by their owners in the same revision | C-09, C-18–C-22 | R14, R23–R25 | Link/metadata/traceability checks only |
| F-35 | Extend F-10 `src/features/audit/inspect_effects.rs` (the contract's `audit.inspect_effects`; no separate manifest file) and `src/features/audit/ports.rs` (`Registry`, `PolicyEvaluator`, `TraceStore`, `FileProbe`); planned F-03 domain (`IoManifest`, `EffectSite` with `origin`/`phase`/`requires_existing`/`secret`, `SiteOrigin`, `OperationNeeds`, `NeededFile`, `FileProbeInput`/`FileProbeResult`, `EffectQuery`, access-verb enum); `src/infra/file_access.rs` (satisfies `FileProbe`, brokered stat); `src/infra/trace_store.rs` (`effect_id` on attempts); F-12 surface files (`io` flags incl. `--needs`/`--check-files`, `GET /v1/io`, MCP `rivet.io`, `rt.io`); F-31 serve authorization | EXTEND planned CREATE | `inspect_effects(EffectQuery) -> IoReport`; access vocabulary, option-derived file sites, target normalization, views/formats, `--check-policy` decisions, `--trace` join, `--needs` view and `--check-files` probes; explicit-only network exposure of `rivet.io` | C-23 | R26, R6, R8, R11 | T-25; S62–S65, S154–S159 |
| F-36 | `src/features/policy/generate_policy.rs`; F-30 `load_policy.rs` (optional `access` on grants/deny, verb/capability check); F-12 CLI (`policy generate --output`), HTTP `POST /v1/policy/generate`, MCP `rivet.policy.generate`, `rt.generate_policy` | CREATE later / EXTEND planned CREATE | `generate_policy(IoManifest) -> PolicyDraft {policy, review, complete}`; no-overwrite output (`conflict.exists`) | C-24, C-23 | R26, R24, R11 | T-26, T-21; S66–S69 |
| F-37 | This proposal; `docs/references/ref-2026-0001-request-and-evidence.md` | UPDATE now | Revision 6: UQ-18, R26, UC-21–UC-22, C-23–C-24, Increment 18, access narrowing in Increments 5 and 16. Revision 8: option-derived file sites, `io --needs`/`--check-files` (TASK-005) and the TASK-006 contract reconciliation (F-13, F-19, F-35). Reference, demos, contract and indexes are updated by their owners in the same revision | C-09, C-23–C-24 | R14, R26 | Link/metadata/traceability checks only |

Known code create groups F-02–F-13, F-18–F-27, F-29–F-33 and F-35–F-36 contain explicit paths; F-14 is bounded discovery, not invented implementation. F-15–F-16 and the revision updates in F-23/F-28/F-34/F-37 are the only design artifacts written now, plus supporting `.ignore/references/` snapshots. No existing runtime symbol or shipped route is deleted. Source skeleton signatures and schemas are specified in the relevant Design increment and the hand-authored contract; adapter pseudocode is `authorize(intent) → perform via held permit → record outcome → release in scope`. Every new adapter will declare its `file/net/env` edges; every feature todo will describe guards, failure and execution order.

## Alternatives Considered

| Alternative | Benefit | Cost | Decision / requirements |
|---|---|---|---|
| Generic `call(transport, options)` everywhere | One syntax | Hides protocol behavior | Rejected; R2 and PROJECT principle |
| Separate CLI/HTTP/MCP implementations | Easy isolated prototypes | Schema/auth drift | Rejected; R8 |
| Compile scripts to arbitrary Rust/shell | Familiar toolchain | I/O escapes and difficult auditing | Rejected; R1, R6, R11 |
| Manual close / finally boilerplate | Visible lifetime control | Error-prone early exits | Replaced by `with`; R3 |
| YAML/JSON-only DAG | Easy static structure | Verbose imperative exchanges | Can expose IR later; primary DSL meets R2/R10 |
| Implicit parallel execution of statements | Short syntax | Side-effect order surprises | Rejected; explicit DAG/concurrent scope |
| Full distributed workflow engine now | Durable resume | Adds persistence/consensus semantics beyond request | Deferred; R10 needs an in-process DAG |
| Keep `--sandbox "allow_read=…"` grant strings | Short one-liners | Unreviewable shell text, quoting bugs, selectors with `,`/`=` need a file anyway | Rejected by the user (UQ-17); R24 |
| Extend Capy grammar with `f(x, y)` calls | Familiar call syntax | Grammar fork, spike risk, divergence from Capy | Rejected; Capy prefix calls (R1, R2) |
| One `serve` process per surface (`--transport`) | Simple routing | Duplicate auth/config, clients need several ports | Rejected; R25 one listener |

## Risks and Rollback

| Risk | Trigger / detection | Impact | Mitigation | Rollback | Owner |
|---|---|---|---|---|---|
| Grammar shapes do not fit Capy | Golden corpus parse spike | Syntax revision | Prototype hard shapes first | Revise draft before runtime code | Maintainer |
| Sandbox escape through process/native adapter | Adversarial effect test | Unauthorized I/O | Fail-closed backend gate | Disable affected capability/platform | Runtime owner |
| Cleanup stalls or hides primary error | Cancellation/fault injection | Resource leak or misleading result | Bounded supervisor and suppressed errors | Disable adapter, retain trace | Runtime owner |
| Retry duplicates writes | Fault after remote commit | Duplicate external effect | Explicit idempotency/no replay after emit | Remove retry policy | Connector owner |
| MCP schema changes | Snapshot mismatch | Wrong dispatch/output | Pin snapshot; review refresh | Restore prior program/schema snapshot | Connector owner |
| Conditional filesystem guarantees unavailable | Concurrent-writer/platform tests | Data loss | Reject unsupported atomic/version mode | Use unconditional explicit operation only if caller elects it | Runtime owner |
| Secret appears in trace/error | Canary scan | Disclosure | Tainted values and bounded redaction | Disable payload recording/export | Security owner |
| OAuth account/token mix-up | Cross-principal, redirect/issuer and rotation fault tests | Wrong account access or lost credentials | Profile hash/identity binding, PKCE, atomic store, uncertain-refresh refusal | Disable affected profile and require reauthorization | Auth adapter owner |
| QUIC library bypasses migration policy or enables early data | Packet capture under denied path and 0-RTT fixtures | Forbidden traffic or replayed mutation | Reject adapter/platform if pre-send hook or 0-RTT disable cannot be enforced | Disable QUIC/H3 capability | Runtime owner |
| UDP delivery assumed reliable | Drop/reorder/duplicate fixture tests | Ambiguous remote execution | Explicit datagram/loss semantics and no implicit retry | Use application acknowledgment protocol or reliable stream | Connector owner |
| Capy licence conflict (gate G-LIC) | LICENSE is source-available (no bundling/commercial/derivatives); Cargo.toml says MIT | Rivet could not legally depend on Capy | Closed 2026-09-28: the owner authorized Rivet's use of Capy (ADR-0001) | None needed; the owner aligns the upstream `LICENSE` text | Project maintainer |
| SSRF through caller params | `param_dependent` targets; metadata-IP/rebinding fixtures | Internal network reached | `deny_private_ranges` default true; component-aware URL encoding; post-DNS checks | Tighten grants to literal origins | Security owner |
| Unauthenticated remote serve | Non-loopback bind with auth none | Anyone can invoke operations | Serve refuses to start (`serve.auth_required`) | Bind loopback only | Operator |

## Security Impact

The effect broker is the trust boundary for **script-initiated effects through brokered adapters**; the language alone is not, and runtime bootstrap I/O (Increment 5 list) is outside it. Authority comes only from `policy.json` (absent = deny all application effects); callers on any surface can only narrow it. Private and metadata address ranges are denied by default. No implicit environment, token storage, native extensions, shell or remote credential forwarding. Secrets are bound to destination origins; taint tracking covers explicit flows only (implicit flows such as `if secret == x` are not tracked). Returning or emitting a secret is an error. Remote-controlled output remains untrusted and payload logging is off. `serve` refuses non-loopback binds without authentication and applies one principal model to every surface. File and network grants must survive symlink, hard-link, junction, case-folding, path traversal, redirect, DNS and process-child adversarial tests. Grants can be narrowed to individual access verbs (for example create-only). The I/O manifest and policy drafts reveal internal URLs and paths, so `rivet.io` / `rivet.policy.generate` are reachable over the network only by principals that list them explicitly; a generated draft is never an approval.

## Operational Impact

Compile once, serve many requests with separate principal contexts and limits. Bounded in-memory traces and no default disk cache. Host shutdown drains scopes; missing required audit sink refuses effects. Service listeners require independent operator authority. A server cannot promote a caller's per-request restriction into new grants. One `serve` process exposes every surface on one port; operators configure it through `policy.json` only.

## Compatibility Impact

No runtime compatibility exists yet. The proposal intentionally revises the brief: no explicit close, `response.body` retains metadata separation, typed JSON bodies replace interpolation, and every stream is scoped. Keep PROJECT.md unchanged as historical input and link this proposed contract from README. Registry IDs and schemas become versioned compatibility commitments only at release.

## Migration Requirements

Convert brief examples to operation definitions with declared outputs, write calls in Capy prefix form with quoted durations, move any revision 1–4 `--sandbox` strings into `policy.json`, replace `serve --transport …/--mcp` with plain `serve`, replace allocated handles with `with`, change transformed response accesses to `.body`, mark streams with `emits`, make retry safety explicit, declare file modes and grants, and assign DAG dependency edges. No migration of live data or existing deployments is required.

## Verification and Promotion Gates

### Correctness first

1. Approval gates pass (G-DESIGN, G-CONTRACT, G-LIC, G-SPIKE; see [Approval](#approval)). The grammar spike parses every S01–S159 example and every docs/demos `.rivet` file cleanly and reports correct line/column diagnostics for invalid fixtures.
2. Each implemented use case has annotated tests; validate continuously; no missing effect authorization branch.
3. Surface parity, resource cleanup, broker denial, secrets, framing and MCP/DAG fault tests pass.
4. `vhco validate .`, `vhco sync .` (zero drift), `vhco check .`, build/tests pass; regenerate model and update current-state docs.
5. Platform-specific sandbox capability is advertised only with conformance evidence. Feature stage status remains explicit.

### Test and Validation Design

Each `T-NN` names a test module/filter to implement, with `// vhco:test feature.use_case -- behavior` annotations. Evidence goes to `docs/reports/rpt-2026-0001-runtime-validation.md`.

| ID | Type / use cases | Scenario and controlled environment | Procedure | Expected result |
|---|---|---|---|---|
| T-01 | Parser/type / UC-01 | Pinned Capy; all S01–S159 code blocks, every docs/demos `.rivet` file and invalid variants | `cargo test conformance_language` | Valid grammar parses; invalid sources execute zero effects; spans exact |
| T-02 | Parity / UC-02 | Same fixture registry over CLI/HTTP/MCP/library | `cargo test conformance_surfaces` | Same params/result/error; auth filters IDs; stdout clean |
| T-03 | Streaming/fault / UC-03 | Fake slow consumer; emit then failure; invalid frame; disconnect | `cargo test conformance_streams` | Bounded queues, no duplicate terminal, cancellation observed |
| T-04 | Files/security / UC-04 | Temp root; adversarial symlinks, version races, missing/full disk | `cargo test conformance_files` | Correct CRUD guards; no outside-root mutation; unsupported claims refuse |
| T-05 | Lifecycle/protocol / UC-05 | Local deterministic HTTP/socket/process fixtures; early returns, EOF, forced cleanup faults | `cargo test conformance_resources` | No owned live handles/tasks/processes after grace; primary errors preserved |
| T-06 | MCP / UC-06 | Version-pinned stdio/HTTP fixtures; tools/resources/prompts, session expiry, schema drift | `cargo test conformance_mcp` | Negotiation, correlation, cancellation, typed tool faults and recursion limits |
| T-07 | DAG / UC-07 | Deterministic fake driver; diamond graph, cycle, fan-out, sibling failure | `cargo test conformance_dag` | Dependencies obeyed, cap respected, partial states complete |
| T-08 | Sandbox / UC-08 | Each supported OS; syscall/effect counters; no policy.json and empty grants; hard links, junctions, case-variant paths; redirect/DNS, process descendants | `cargo test conformance_sandbox` | Zero prohibited effects; unsupported backend refuses before spawn |
| T-09 | Audit / UC-09 | All adapters; dynamic/opaque sites; secret canaries; failed sink | `cargo test conformance_audit` | Every site reported; every attempt correlated; no canary leaks; strict unknown fails |
| T-10 | Embedding / UC-10 | Existing Tokio runtime; borrowed sink; abandoned future and scope exit | `cargo test conformance_library` | No nested runtime requirement, scope joins, callback stop/error defined |
| T-11 | OAuth / UC-11 | Fake provider and versioned store; all three flows, state/issuer mismatch, expired challenge, slow_down, rotation race/crash, invalid_grant, 401 on mutation | `cargo test conformance_oauth` | Tokens never escape; one refresh per key; codes single-use; polling respects interval; no unsafe replay; scope/principal isolation |
| T-12 | UDP / UC-12 | Loopback IPv4/IPv6 and multicast fixtures; drop/reorder/duplicate, empty/oversized/truncated datagram, denied bind/reply peer | `cargo test conformance_udp` | Boundaries/peer retained, errors typed, no implicit retries, no forbidden send/bind, membership cleanup |
| T-13 | QUIC / UC-13 | Local QUIC v1 peer; wrong cert/ALPN, blocked window, uni/bidi, FIN/reset, DATAGRAM absent/oversized, candidate migration | `cargo test conformance_quic` | TLS checked; bounded independent streams; no 0-RTT; denied path sends zero probes; scoped teardown |
| T-14 | H3 / UC-14 | H3/H2 fixtures, negotiation failure, alternate port, POST accepted then response lost, OAuth profile | `cargo test conformance_http3` | Strict H3 does not downgrade; preference fallback only when safe; one mutation; response.version and auth preserved |
| T-15 | Cross-surface/security / UC-11–14 | Same registered fixtures over CLI/HTTP/MCP/library; absent/empty/restricted policy.json; secret canaries; cache hits; different tenants | `cargo test conformance_auth_transport_policy` | Same values/errors; all potential effects inventoried; denied effects produce zero prohibited external operations, including cached-credential use; no secret logging |
| T-16 | Catalog / UC-15 | Multi-operation file with names/param descriptions/defaults; duplicate in second declaration or import; private helper | `cargo test conformance_operation_catalog` | All metadata preserved; atomic load; duplicates show both spans; private IDs inaccessible via every entry point |
| T-17 | gRPC / UC-16 | Fixture descriptor/server supports all four modes; map/oneof/int64/bytes/Any, deadline, partial stream then non-OK trailers, early server finish | `cargo test conformance_grpc` | Lossless schema mapping; status governs success; no retry after emit/send; half-close/cleanup and metadata redaction |
| T-18 | MCP / UC-17 | stdio and Streamable HTTP initialized clients list/call same catalog; no-param, scalar, unauthorized, input-stream and output-stream tools | `cargo test conformance_mcp_catalog` | IDs/descriptions/schemas match CLI/HTTP/library; correct Completion/SessionReceipt/error; no invented MCP data-stream feature |
| T-19 | Duplex/session / UC-18 | Interleave open/send/read/finish/cancel across CLI, HTTP session operations, polling routes, WebSocket refs, MCP and library; duplicate send, stale cursor, full queue, early peer EOF, abandoned host, tenant mismatch | `cargo test conformance_duplex_sessions` | One enqueue per accepted retry; ordered bounded events; no unauthorized access; no deadlock; expiry/cancel joins resources; terminal event never silently skipped |
| T-20 | Outputs / UC-19 | Operations with scalar, nested object, `list T`, open/closed objects, emits/receives blocks, declared errors; result that violates schema after a committed write | `cargo test conformance_outputs` | Same schema from CLI table/JSON, describe, HTTP `/outputs`, MCP `rivet.outputs`/`outputSchema`, `Runtime::outputs`; mismatch → `output.invalid` 500/exit 5 with `effects:committed`; undeclared `fail` code warns, errors under `--strict-docs` |
| T-21 | Policy file / UC-08 | No policy.json; empty `{"version":1}`; `--policy PATH`; unknown key/capability; `deny` over `grants`; private-range, metadata-IP and rebinding targets; `["*"]` grant; per-request narrowing | `cargo test conformance_policy_file` | Absent/empty → zero application effects, pure ops run; malformed → `policy.invalid` exit 2; private ranges denied unless literal grant; `policy explain` flags broad grants; callers never widen |
| T-22 | Unified serve / UC-20 | One `serve` process; same operation over REST, SSE, polling, WS and MCP; bearer/mTLS/none; non-loopback + none; disabled surface; 9th WS ref; socket close mid-stream | `cargo test conformance_serve` | Identical Completion/errors on all five; `serve.auth_required` exit 2; 401/403/404/429 as registry; WS close cancels and joins refs; polling session survives reconnect |
| T-23 | Syntax / UC-01 | Prefix calls with trailing objects and `allow` blocks, quoted/unquoted durations, `${a.b}` vs `${a + b}`, escapes, URL path/query interpolation with `/ ? # @`, option after body, `yield` vs `return` in `map`/`poll` | `cargo test conformance_syntax` | Valid forms lower; bare `10s` and non-path interpolation rejected with line/column; URL cannot gain segments/host; `syntax.option_after_body`; `return` exits the operation |
| T-24 | Errors/limits/DAG / UC-02, UC-07 | Every registry code on every surface; 65 concurrent nested calls; depth 17; literal call cycle; fail fast with running, ready and pending nodes; unguarded `.result` | `cargo test conformance_errors_limits_dag` | HTTP/exit/retryable match the registry; global cap holds; `limit.call_depth`; `check.call_cycle`; node statuses cancelled/skipped/blocked exactly as defined; DagCompletion lists every node; check warning |
| T-25 | I/O manifest / UC-21, UC-09 | Bundle with exact, bounded, param_dependent, dynamic and opaque sites across every kind (file CRUD, HTTP methods, env secret, process, MCP stdio, gRPC, auth); private helper; policy.json with/without `access`; completed trace | `cargo test conformance_io_manifest` | Every site listed with correct access verbs, capability, method/protocol, normalized template/glob and source; one line → several sites where defined; `--by`/`--kind`/`--access`/`--format` views agree; `--check-policy` allowed/denied/partial/unknown and exit 3; `--strict` exit 7; zero I/O performed; `--trace` ATTEMPTS joined on `effect_id`; CLI/HTTP/MCP/library identical; network principal without explicit `rivet.io` → 403, `*` does not match; wrong verb for capability → `policy.invalid` exit 2. Revision 8 cases: every `tls ca_file`/`cert_file`/`key_file` line in http, websocket, quic, grpc/mcp connector and tcp blocks, `body file` and an operation-level `descriptor` → one `allow_read` read site each, with correct `origin`, `phase` (`before_connect`/`body`/`load`) and `secret` (key file true, path shown, content never read); connector option sites appear under every calling operation; connector `descriptor`/`schema` only under `--include-bootstrap`; `requires_existing` false for a file the same operation creates earlier (`file create` then `body file`) and for `missing ok`; `--by target` ORIGIN/PHASE/NEEDS FILE; `--needs` per-operation grouping incl. `(via callee)` and exclusion of created files, identical via `rivet.io {needs}` and `GET /v1/io?needs=true`; `--check-files` against a temp dir: all present → 0, one missing → 4, stat not granted (`access: ["read"]` only) → `not_permitted` exit 3 with zero probes of that path, unreadable (mode 000) → 3, param_dependent → `not_checkable`; key file contents never opened; `policy generate` emits exact option paths with `access: ["read"]` and a review item (exit 7) for a param_dependent option path |
| T-26 | Policy draft / UC-22 | Same bundle; `--output` to new and existing file; dynamic URL site | `cargo test conformance_policy_generate` | One grant per (capability, target) with exact `access`; param_dependent URL → origin, path → glob; `deny_private_ranges: true`; no `serve`/secrets; dynamic/opaque → stderr review list, exit 7, draft still written; existing file → `conflict.exists` exit 4; generated draft loads and `io --check-policy` reports every granted site allowed |

### Quality/performance and measurement

Benchmark only after correctness. Freeze source fixture, compiler flags, OS, hardware, dependency lockfile and program/policy hashes. Compare brokered invocation against the same adapter invoked directly using deterministic local fixtures; record warm/cold compilation separately. Test procedure: `cargo test conformance_resources -- --nocapture` for leak/fault counts; proposed `cargo bench --bench runtime_cost` for latency, memory and dispatch overhead. Output/evidence destination is the validation report above. No throughput or speedup claim is made before measurement.

### Expected Result Before Measurement

| Metric | Forecast / acceptance criterion | Baseline |
|---|---|---|
| Unknown-field, unsafe-retry, ownership violations | 100% rejected in the declared fixture suite before prohibited effect | Explicit negative fixtures |
| Unpermitted brokered effects | Zero observed in suite | Instrumented denied target |
| Cleanup | Zero owned live handles/children after 5s configured grace in fixtures | Handles/children before acquisition |
| Stream buffering | <= configured frame and byte caps; producer stalls at demand boundary | Bounded queue instrumentation |
| Dispatch latency / throughput | Some validation/policy overhead expected; report distribution, no numeric gain claim | Direct same-adapter call |
| Parser time / memory | Finite under input/nesting budgets; discover practical limits | Fixed corpus and pinned Capy |

A too-good result must be investigated: zero overhead or perfect resource counts can mean the benchmark bypassed policy or failed to observe child work.

## Contract Delta

The [vhco-contract.json](../../../vhco-contract.json) is hand-authored for Rust; never overwrite it with `vhco spec` output. As of revision 4 it has twelve features (`language`, `registry`, `execution`, `files`, `connectors`, `audit`, `policy`, `auth`, `datagrams`, `quic`, `grpc`, `sessions`), four surfaces (CLI, HTTP, library, MCP), 53 domain records, twelve adapter contracts and twenty-three source-to-port flows. `policy.authorize_effect` and `auth.acquire_credential` are internal; neither permits public grant or token extraction. Revision 5 requires these contract deltas (made by the contract owner in the same revision): `registry.describe_outputs`; `policy.load_policy` replacing the sandbox-flag input; a `serve` feature with `authenticate_principal`; WebSocket and polling surfaces; `OutputSpec`, `SyntaxTree`, `DagCompletion` and error-registry records. Revision 6 adds: `audit.build_io_manifest` and `policy.generate_policy` use cases; `IoManifest`, `IoSite`, `IoQuery`, `PolicyDraft` records; an optional `access` list on policy grant/deny records; `effect_id` on trace attempt records.

Validation limit: VHCO 1.6.0 requires a Rust `src/` tree, so `validate`, `sync` and `check` report `open src: no such file or directory`. This is not a passing implementation gate; no dummy source is added to manufacture green checks. `vhco live vhco-contract.json` renders the contract for review. Documentation checks are filesystem/link/metadata checks, not parser or runtime execution; `vhco docs check .` reports link errors for `.ignore/references/` files omitted by its discovery, which direct filesystem checks confirm exist.

## Documentation, Demo and Release Impact

| Artifact | Path / destination | Action and gate |
|---|---|---|
| Design proposal | This document | Review, then record explicit human decision |
| Reference | `docs/references/ref-2026-0002-language-and-usage.md` | S01–S159 are proposed examples; convert to executable fixtures before release |
| Sample folders | [docs/demos/README.md](../../demos/README.md) | Proposed bundles with `policy.json` files, prefix-call sources, fixtures and walkthroughs; part of the G-SPIKE parse gate; no runtime verification |
| Source record | `docs/references/ref-2026-0001-request-and-evidence.md` | Preserve original requirements and inspected commits |
| Current-state docs | `README.md`, `docs/README.md`, directory indexes | State design-only status now; update on each implemented slice |
| Future tests/report | F-18 destinations | Match then-current templates; record real executions |
| Version/release | Cargo.toml and future `docs/releases/rel-0.1.0-release-notes.md` | Release only completed advertised stage; never imply B shipped with A; no release before G-LIC |

## Requirements Alignment

| Requirement | User/source | Goal | Use cases | Changes | Files | Tests | Reference |
|---|---|---|---|---|---|---|---|
| R1 | UQ-09/10 | G-01, G-05 | UC-01, UC-10 | C-01, C-21 | F-01–04, F-32 | T-01, T-10, T-23 | S01, S04 |
| R2 | UQ-03, PROJECT | G-01, G-05 | UC-01 | C-01, C-21 | F-03–05, F-32 | T-01, T-23 | S01, S07–18 |
| R3 | UQ-02 | G-02 | UC-03, UC-05, UC-10 | C-03, C-08 | F-06–07, F-13 | T-03, T-05, T-10 | S19–32, S70–73 |
| R4 | UQ-03, PROJECT | G-02 | UC-02, UC-03, UC-05, UC-07 | C-02, C-03, C-07, C-22 | F-03, F-06–07, F-33 | T-02, T-03, T-05, T-07, T-24 | S44–51, S78 |
| R5 | UQ-04 | G-03 | UC-04 | C-04 | F-03, F-08 | T-04 | S33–43 |
| R6 | UQ-05, UQ-18 | G-03 | UC-09, UC-21 | C-05, C-23 | F-09–10, F-35 | T-09, T-25 | S62–65, S74–76 |
| R7 | UQ-06 | G-01, G-04 | UC-06 | C-06 | F-03, F-11–12 | T-06 | S52–61 |
| R8 | UQ-07/09 | G-01 | UC-02, UC-10, UC-21 | C-02, C-06, C-23 | F-02, F-05, F-12, F-35 | T-02, T-10, T-25 | S02–06 |
| R9 | UQ-08 | G-01, G-02 | UC-02, UC-03, UC-10 | C-02 | F-02, F-06–07, F-12 | T-02, T-03, T-10 | S04, S70–73 |
| R10 | UQ-11 | G-04 | UC-07 | C-07, C-22 | F-03, F-06–07, F-33 | T-07, T-24 | S44–51 |
| R11 | UQ-13, UQ-17 | G-03 | UC-04, UC-06, UC-08, UC-09, UC-21, UC-22 | C-04–06, C-19, C-23, C-24 | F-08–11, F-14, F-30, F-35–F-36 | T-04, T-06, T-08, T-09, T-21, T-25, T-26 | S66–69, S77–80 |
| R12 | UQ-01/12, PROJECT | G-04, G-05 | UC-05 | C-08 | F-13–14 | T-05, T-08 | S07–32, S79 |
| R13 | PROJECT §§6,79–80 | G-03 | UC-05, UC-08, UC-09 | C-05, C-08 | F-09–10, F-13 | T-05, T-08, T-09 | S12, S28, S76–80 |
| R14 | UQ-12, UQ-16, project rules | G-05 | UC-01–22 | C-09 | F-01, F-15–18, F-28, F-34, F-37 | T-01–26 plus doc review | All samples |
| R15 | UQ-14; PROJECT §§28–30 | G-02, G-03, G-04 | UC-12 | C-11 | F-20, F-22, F-23 | T-12, T-15 | S27, S81–S83 |
| R16 | UQ-14 | G-01, G-02, G-03 | UC-11 | C-10 | F-19, F-22, F-23 | T-11, T-15 | S84–S93 |
| R17 | UQ-14 | G-02, G-03, G-04 | UC-13 | C-12 | F-21–F-23 | T-13, T-15 | S94–S98 |
| R18 | UQ-14 context | G-01, G-03 | UC-14 | C-13 | F-22, F-23 | T-14, T-15 | S99–S102 |
| R19 | UQ-15 | G-01, G-02, G-04 | UC-16 | C-14 | F-24, F-28 | T-17 | S110–S113, S117–S118 |
| R20 | UQ-15 | G-01, G-05 | UC-15 | C-15 | F-25, F-28 | T-16 | S103–S106, S109, S119 |
| R21 | UQ-15 | G-01, G-03 | UC-17 | C-16 | F-26, F-28 | T-18 | S107–S108, S116, S120 |
| R22 | UQ-15; UQ-08; UQ-17 | G-01, G-02, G-04 | UC-18, UC-20 | C-17, C-20 | F-27–F-28, F-31 | T-19, T-22 | S113–S116 |
| R23 | UQ-17 | G-01, G-06 | UC-19 | C-18 | F-29, F-34 | T-20 | S103–S106 |
| R24 | UQ-17; UQ-13 behaviour | G-03, G-06 | UC-08, UC-22 | C-19, C-23, C-24 | F-30, F-34, F-36 | T-21, T-26 | S66–S69 |
| R25 | UQ-17 | G-01, G-06 | UC-20 | C-20 | F-31, F-34 | T-22 | S114, S116 |
| R26 | UQ-18; TASK-005 (ADR-0001) | G-03, G-06 | UC-21, UC-22, UC-09 | C-23, C-24 | F-35–F-37 | T-25, T-26 | S62–S69, S141–S159 |

Reverse check: C-01→R1/R2; C-02→R4/R8/R9; C-03→R3/R4; C-04→R5/R11; C-05→R6/R11/R13; C-06→R7/R8; C-07→R4/R10; C-08→R3/R12/R13; C-09→R14; C-10→R16; C-11→R15; C-12→R17; C-13→R18; C-14→R19; C-15→R20; C-16→R21; C-17→R22; C-18→R23; C-19→R24 (and R11/R13); C-20→R25 (and R8/R21/R22); C-21→R1/R2; C-22→R4/R8/R10; C-23→R26 (and R6/R8/R11/R24); C-24→R26 (and R24/R11). C-14–C-17 also enforce shared R2/R3/R4/R6/R7/R8/R9/R11/R13 through T16–T19. C-10–C-13 also enforce shared R3/R4/R6/R8/R11/R13 via T-15 (and F-19–F-22), extending those original alignment rows without replacing them. Every F row names these changes and requirements. The deferred platform file discovery has a resolution rule and does not authorize an unreviewed capability.

## Plan Strategy and Estimated Work

One master implementation plan, with sequential gated increments, is sufficient. Increment 1 owns R1/R2; 2–3 own R3/R4/R8/R9; 4–5 own R5/R6/R11/R13; 6 owns R7; 7 owns R10; 8 owns R12; 9 owns R16; 10 owns R15; 11 owns R17/R18; 12 owns R20/R21; 13 owns R19; 14 owns R22; 15 owns R23; 16 owns R24; 17 owns R25; 18 owns R26 (building on the effect graph of 5 and the policy loader of 16); documentation gates own R14. Increments 1, 2, 15 and 16 land in Stage A; increments 9–14 and 17 are mandatory before claiming Stage B complete. The Capy spike (G-SPIKE) and G-LIC precede any runtime code. The owning increment establishes the contract; later adapters must obey it. This is a multi-increment project, not a small wrapper. Calendar estimates would be speculative before grammar and sandbox spikes; estimate engineering effort after those two results, separately from the complexity rating.

## Open Questions

Resolved in revision 5 and removed from this list: call syntax (Capy prefix calls), Capy licence (gate G-LIC, closed by the owner in ADR-0001), protocol phasing (all required), policy source (policy.json only), serve composition (one listener).

Still open — implementation facts, none blocking design review:

| ID | Question | Resolve by | Blocks |
|---|---|---|---|
| OQ-1 | Which OS subprocess sandbox backends pass conformance on Linux/macOS/Windows? | Platform spike (F-14) | Sandboxed `allow_exec`/stdio MCP per platform |
| OQ-2 | Which conditional file-update backends can enforce version guards? | File adapter spike | `file update` with version guard |
| OQ-3 | Which Rust crates/MSRV (HTTP server with WS upgrade, QUIC/H3, gRPC dynamic messages)? | Dependency spike after G-SPIKE | Adapter implementation |
| OQ-4 | Does Capy's recovering parser need any library-defined shape for nested `output object … end` inside an operation header? | G-SPIKE | Increment 15 grammar |

## Approval

Status: **approved 2026-09-28** by the project maintainer ([ADR-0001](../../decisions/adr-0001-approve-rivet-runtime-design.md)).
G-SPIKE is an implementation-entry gate owned by the implementer and is tracked in
[PLAN-2026-0001](../../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md) (TASK-010).

| Gate | Condition | Owner | State |
|---|---|---|---|
| G-DESIGN | Maintainer accepts syntax, request contract, outputs, policy file, serve surfaces, I/O manifest and scope | Project maintainer | **Closed 2026-09-28** — approved as written, including the option-derived file sites / `io --needs` extension (PLAN-2026-0001 TASK-005) |
| G-CONTRACT | Human reviews the hand-authored contract (AGENTS.md) | Project maintainer | **Closed 2026-09-28** — approved |
| G-LIC | Capy licence permits Rivet to depend on and ship Capy | Project maintainer (Capy owner) | **Closed 2026-09-28** — the maintainer owns Capy and confirmed its use in Rivet is authorized; the upstream `LICENSE` text is the owner's to update |
| G-SPIKE | Pinned Capy parses every S01–S159 example and every docs/demos `.rivet` file cleanly | Implementer | Open — PLAN-2026-0001 TASK-010 |

```text
 design review ---> G-DESIGN (closed) ---> G-CONTRACT (closed) --+
                                                                  +--> implementation --> Stage A --> Stage B
 Capy owner ------> G-LIC (closed) ------------------------------+
 Capy spike ------> G-SPIKE (open, PLAN-2026-0001 TASK-010) ------+
```

## Related Documents

- [Project brief](../../../PROJECT.md)
- [Request and evidence](../../references/ref-2026-0001-request-and-evidence.md)
- [Language and usage reference](../../references/ref-2026-0002-language-and-usage.md)
- [Sample folders](../../demos/README.md)
- [Standards index](../../standards/index.md)
- [Current project state](../../../README.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 9 | 2026-09-28 | Claude | Implemented in v0.1.0 (PLAN-2026-0001, REL-0.1.0): status implemented, moved to `implemented/`, status banner updated. |
| 8 | 2026-09-28 | Claude | TASK-005 (approved in ADR-0001): option-derived file sites (`tls ca_file`/`cert_file`/`key_file`, `body file`, operation-level `descriptor`), per-site `origin`/`phase`/`requires_existing`/`secret`, `--by target` ORIGIN/PHASE/NEEDS FILE, `rivet io --needs` (`rivet.io {needs}`, `GET /v1/io?needs=true`) and `--check-files` (exit 4/3), exact-path grants in `policy generate`; R26, UC-21, C-23, T-25, Increment 18 and samples S154–S159. TASK-006: F-13 names the `transports` feature, F-19 adds `cancel_authorization.rs`, F-35 extends `inspect_effects.rs` as the contract does. |
| 7 | 2026-09-28 | Claude | Approved by the project maintainer (ADR-0001): G-DESIGN, G-CONTRACT and G-LIC closed; moved to `proposals/approved/`; G-SPIKE tracked by PLAN-2026-0001. |
| 6 | 2026-09-28 | Claude | UQ-18: generated I/O manifest (R26, UC-21, UC-22, Increment 18) — access vocabulary per site, target normalization, `rivet io` views/formats/`--check-policy`/`--strict`/`--trace`, JSON IoManifest, `rivet policy generate` least-privilege drafts, `rivet.io`/`rivet.policy.generate` on every surface with explicit-only network exposure, `effect_id` trace link; optional `access` narrowing in policy.json (Increments 5, 16); R6/UC-09 refined; P7; error registry (`conflict.exists`, wrong-verb `policy.invalid`, exit 3/7 for `io`/`policy generate`); C-23–C-24, F-35–F-37, T-25–T-26; samples. |
| 5 | 2026-09-28 | Claude | UQ-17: declared outputs (R23, UC-19, Increment 15), policy.json-only configuration replacing `--sandbox` (R24, Increment 16), one serve for REST/SSE/polling/WebSocket/MCP replacing `--transport`/`--mcp` (R25, UC-20, Increment 17); Capy prefix-call syntax and syntax table; review fixes (licence gate G-LIC, direct MCP tools canonical, scoped sandbox claims and bootstrap list, SSRF defaults, device-flow pending and `auth cancel`, DAG node semantics and DagCompletion, error registry, `reviewed` definition, leading-options and yield rules, one Rust API sketch, global limits, `users.grpc_get`, stage labels); added Summary, feasibility status, state machines, approval gates; C-18–C-22, F-29–F-34, T-20–T-24. |
| 4 | 2026-09-28 | Codex | Added twelve draft sample folders with source files, fixtures, request bodies and usage READMEs (UQ-16). |
| 3 | 2026-09-28 | Codex | Added gRPC, documented multi-operation catalogs, incoming MCP tools and duplex sessions; expanded reference to 120 examples. |
| 2 | 2026-09-28 | Codex | Added UDP, OAuth 2.0, QUIC/HTTP3 design coverage, traceability and 22 examples; updated current-state navigation. |
| 1 | 2026-09-27 | Codex | Initial proposal with scoped resources, unified interfaces, effect enforcement, MCP bridge and DAG semantics. |
