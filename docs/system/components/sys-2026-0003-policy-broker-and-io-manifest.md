---
document_id: SYS-2026-0003
title: "Rivet policy broker and I/O manifest"
document_type: system
status: active
created_date: 2026-09-28
last_updated: 2026-09-29
document_revision: 4
authors: [Claude]
owner: Project maintainer
component_owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [policy, audit]
affected_versions:
  from: "0.1.0"
  to: null
last_verified_version: "0.2.0-rc (main at 8031baa)"
next_review_date: 2026-10-29
review_cycle: on-release
confidentiality: internal
scope: How Rivet loads policy.json, authorizes every effect attempt through the traced policy broker, and derives the static I/O manifest, policy drafts and request traces — including (0.2.0) global substitution into targets, one policy across file modules, and bootstrap reads of imported files.
reason: Every application effect in Rivet passes through one decision function; operators and reviewers need an accurate description of how that decision is made, what the I/O manifest reports and where the evidence (traces, drafts) lives.
related_documents: [PROP-2026-0001, PLAN-2026-0001, PROP-2026-0002, PLAN-2026-0002, API-2026-0006, SEC-2026-0001, SYS-2026-0001, SYS-2026-0002, SYS-2026-0004, SYS-2026-0005, SYS-2026-0006, SYS-2026-0008, SYS-2026-0009]
supersedes: null
superseded_by: null
tags: [rivet, system, policy, broker, audit, io-manifest, trace, least-privilege]
---

# Rivet policy broker and I/O manifest

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-29
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** policy, audit
> **Last Verified Version:** 0.2.0-rc (main at 8031baa)

## Summary

Rivet has exactly one source of authority: a `policy.json` file. The **policy** feature loads and
validates it (`load_policy`), and decides every single effect attempt (`authorize_effect`) through a
**policy broker** that every adapter (files, HTTP, UDP, QUIC, gRPC, processes, MCP, OAuth) must call
before it touches the outside world. The **audit** feature turns the compiled program into a static
**I/O manifest** (`rivet io`) that lists every effect site, its target and its access verbs; the same
manifest can be checked against the policy, probed on disk, joined to a request trace, or turned into
a least-privilege policy draft (`rivet policy generate`). Each brokered decision is recorded in a
bounded, in-memory **trace store** readable with `rivet trace show` and exportable with
`Runtime::export_trace`. The effective authority of one attempt is **policy.json ∩ library host
ceiling ∩ every per-request `restrict`** — callers and hosts can only narrow. The audit feature also
builds the static call graph behind `rivet graph`.

In 0.2.0 the manifest also sees `global` constants and file modules. Globals are substituted into effect
targets, so a target built only from literals and globals is `exact`. A bundle of several files runs under **one**
policy, the loader's, and each imported file is listed as its own bootstrap read. The `--json` outputs of these
commands are [ResponseEnvelopes](../../api/api-2026-0006-envelopes.md) (`rivet.io`, `rivet.policy.generate`,
`rivet.trace.show`, …).

```text
                       ┌──────────────────────────── policy feature ─────────────────────────────┐
  policy.json ──read──▶│ load_policy ──▶ Policy ──▶ PolicyBroker ◀── authorize_effect (decide fn) │
  (or --policy PATH)   │   .ceiling(Policy) (library) ─┘   policy ∩ ceiling, then ∩ restrict stack │
                       └────────────────────────────────┬────────────────────────────────────────┘
                                                        │ evaluate(EffectIntent) → Permit
                         adapters / file use cases ─────┤
                                                        ▼
                                               TracedEvaluator ──record──▶ MemoryTraceStore
                                                                               │
                       ┌──────────────────────────── audit feature ───────────▼─────────────────┐
  compiled program ───▶│ effect_sites::analyze_program ──▶ EffectCatalog ──▶ inspect_effects     │
                       │         (static, nothing evaluated)                   │  ├─▶ IoManifest  │
                       │                                                       │  ├─▶ render      │
                       │ read_trace ◀────────── rivet trace show / export       │  └─▶ generate_   │
                       │ build_graph ◀───────── rivet graph (registry only)     │      policy      │
                       └───────────────────────────────────────────────────────┴──────────────────┘
```

## Responsibilities

| Responsibility | Module | Surface |
|---|---|---|
| Discover `policy.json` beside the entry file, or read the file named by `--policy PATH` | `src/infra/policy_file_reader.rs` | every command that loads a bundle |
| Validate schema v1 strictly and build the effective `Policy` (deny-by-default when absent) | `src/features/policy/load_policy.rs`, `src/domain/policy.rs` | `rivet check`, `rivet policy explain`, every run |
| Decide one effect attempt: deny entries, private ranges, grants, access verbs | `src/features/policy/authorize_effect.rs` | all adapters |
| Hold the policy, apply the decide function, keep a decision log | `src/infra/policy_broker.rs` | runtime assembly |
| Attribute each decision to request, operation and manifest `effect_id`; record a trace event | `TracedEvaluator` in `src/orchestrator/runtime.rs`, `src/infra/trace_store.rs` | all adapters |
| Static effect analysis of the compiled IR | `src/features/audit/support/effect_sites.rs` | runtime assembly |
| Select, filter, check, join and group sites into an `IoManifest` | `src/features/audit/inspect_effects.rs`, `src/domain/io_manifest.rs` | `rivet io`, `GET /v1/io`, `rivet.io` |
| Render tables, markdown, CSV and JSON | `src/features/audit/support/render.rs` | `rivet io` |
| Produce a least-privilege draft and write it exclusively | `src/features/policy/generate_policy.rs`, `src/infra/policy_draft_writer.rs` | `rivet policy generate`, `POST /v1/policy/generate`, `rivet.policy.generate` |
| Read one request's recorded decisions | `src/features/audit/read_trace.rs` | `rivet trace show`, `rivet.trace.show` |
| Export one request's trace to a new file through the broker (`allow_write` create) | `Runtime::export_trace` (`src/orchestrator/runtime.rs`) | `rivet trace export` (remote), `rivet.trace.export {request_id, path}`, `Runtime::export_trace` |
| Static call graph of one operation | `src/features/audit/build_graph.rs`, `src/domain/call_graph.rs` | `rivet graph ID [--all] [--json]`, `Runtime::graph` |
| Intersect with a library host ceiling | `Policy::with_ceiling` (`src/domain/policy.rs`), first check in `authorize_effect` | `RuntimeBuilder::ceiling` |
| Intersect with per-request restrictions | `REQUEST_RESTRICTION` task-local stack in `TracedEvaluator` (`src/orchestrator/runtime.rs`) | `restrict` on HTTP, polling, WS, MCP; `Runtime::request_restricted` |

## Boundaries and Non-Responsibilities

- **Not an authenticator.** Who may call which operation over `rivet serve` (`serve.auth`,
  `serve.principals`) is decided by the serve feature; see
  [SYS-2026-0004](sys-2026-0004-surfaces-and-serve.md). The broker decides *what an operation may touch*.
- **Not a sandbox.** Process isolation (Seatbelt, Landlock) is a separate layer; the broker only decides
  whether `allow_exec` permits the program path. See [SYS-2026-0005](../integrations/sys-2026-0005-protocol-adapters.md).
- **Does not perform I/O for the manifest.** `rivet io` never connects, never evaluates a source
  expression and never opens a file. The only exception is `--check-files`, which `stat`s paths through
  the broker (metadata only).
- **Does not govern bootstrap reads.** The entry bundle and every imported module file (0.2.0), `policy.json`
  itself, CA bundles, the resolver, tzdata, connector descriptor/schema snapshots and stdio are listed with
  `--include-bootstrap` but are never checked against `policy.json`. Module reads are confined to the runtime
  root by the loader instead (SYS-2026-0001).
- **Does not persist anything.** The trace store and decision log are memory only and die with the
  process. Drafts are only written when `--output PATH` is given.
- **The schema reference lives elsewhere.** Every key, default and error message of `policy.json` is in
  [SYS-2026-0008](../configuration/sys-2026-0008-policy-json-reference.md).

## Architecture

### Module map

```text
 src/domain/policy.rs          Capability(13) · AccessVerb(18) · Grant · Policy · EffectIntent · Permit
 src/domain/io_manifest.rs     EffectSite · SiteKind · Knowledge · Phase · IoManifest · IoQuery · TraceEvent
                               PolicyDraft · bootstrap_list() · capability_for(kind, verb)
 src/domain/ports.rs           PolicyFileReader · PolicyEvaluator · PolicyDraftWriter · TraceStore · FileProbe
 src/domain/effect_checks.rs   checked_addr(): resolve once, re-authorize private results

 src/features/policy/
   load_policy.rs              load_policy() + parse_policy()  (strict schema v1)
   authorize_effect.rs         authorize_effect() + selector matching + is_private()
   generate_policy.rs          generate_policy()  (manifest → draft)
 src/features/audit/
   inspect_effects.rs          inspect_effects()  (query → IoReport)
   read_trace.rs               read_trace()       (paging over TraceStore)
   build_graph.rs              build_graph()      (registry → CallGraph: calls, connectors, effects, DAG, if arms)
   support/effect_sites.rs     analyze_program()  (compiled IR → EffectCatalog)
   support/render.rs           table / markdown / csv renderers

 src/infra/
   policy_file_reader.rs       DiskPolicyReader       (satisfies PolicyFileReader)
   policy_broker.rs            PolicyBroker           (satisfies PolicyEvaluator)
   trace_store.rs              MemoryTraceStore + task-local EffectScope (satisfies TraceStore)
   policy_draft_writer.rs      ExclusiveDraftWriter   (satisfies PolicyDraftWriter)
   effect_args.rs              read_file(): option files (tls ca_file, body file) go through the policed file port

 src/orchestrator/runtime.rs   Runtime::assemble(): wires broker → TracedEvaluator → adapters
```

### Authorization decision flow (`authorize_effect`)

The decision function is pure: `(EffectIntent, Policy, bundle_root) → Permit`. Checks run in this fixed
order; the first one that decides wins. A library **host ceiling** (`Policy.ceiling`) is evaluated first with
the same function: if it denies, the attempt is denied with rule `host ceiling: …` whatever policy.json grants
(a ceiling never grants; limits narrow to the smaller value). After the broker allows an attempt, the
`TracedEvaluator` evaluates it again against every **per-request restriction** on the task's stack; a denial
there reads `request restriction: …`.

```text
 intent ─▶ ceiling? ── deny ─▶ "host ceiling: …"
             │ allow / none
             ▼
          policy.json checks (below) ── deny ─▶ rule
             │ allow
             ▼
          restrict stack (request + nested calls) ── any deny ─▶ "request restriction: …"
             │ all allow
             ▼
          Permit allowed  ─▶ trace event
```

URL grants with a path match **whole segments**: `/users/42` covers `/users/42` and `/users/42/…` but not
`/users/420`; a selector ending in `/` covers what is below it; an explicit trailing `*` is a raw prefix.

```text
            EffectIntent { capability, verb, target, operation_id, effect_id?, span? }
                                          │
                                          ▼
                          ┌───────────────────────────────┐   no
                          │ policy.present ?              │──────▶ DENIED  "no policy.json: <cap> is denied by
                          └───────────────┬───────────────┘                default (add a grant for <target>)"
                                          │ yes
                                          ▼
                          normalize target (paths joined to the bundle root, `.`/`..` folded;
                          URLs parsed; env / logical names kept as text)
                                          │
                                          ▼
                          ┌───────────────────────────────┐  match
                          │ any deny entry with same cap, │──────▶ DENIED  "deny <cap> <targets>"
                          │ verb in its access (or none), │
                          │ and a matching selector?      │
                          └───────────────┬───────────────┘
                                          │ none
                                          ▼
                          ┌───────────────────────────────┐  yes
                          │ cap ∈ {network, listen, grpc},│──────▶ DENIED  "<ip> is a private/loopback/link-local
                          │ target host is an IP (or      │                address; grant it literally (e.g. "…")"
                          │ localhost), deny_private_     │
                          │ ranges true, IP is private,   │
                          │ and NO grant names it         │
                          │ literally?                    │
                          └───────────────┬───────────────┘
                                          │ no
                                          ▼
                          ┌───────────────────────────────┐  verb ok
                          │ first grant with same cap and │──────▶ ALLOWED "grant <cap> <targets>"
                          │ a matching selector           │
                          └───────────────┬───────────────┘
                                          │ selector matched but verb not in `access`
                                          ├──────────────────────▶ DENIED  "grant <cap> <targets> access [..]
                                          │                                  does not include `<verb>`"
                                          │ nothing matched
                                          └──────────────────────▶ DENIED  "no grant for <cap> <target>"
```

Selector matching (`selector_matches` in `src/features/policy/authorize_effect.rs`):

| Selector kind | Applies to | Rule |
|---|---|---|
| `"*"` | every capability | matches every target (still subject to the private-range rule) |
| path glob | `allow_read`, `allow_write`, `allow_delete`, `allow_exec`, `allow_unix`, `allow_pipe` | selector is joined to the **policy file's directory** and normalized; the target is joined to the **bundle root**; `globset` with `literal_separator(true)` (`*` stays inside one segment, `**` crosses); `dir/**` also matches `dir` itself; on macOS both sides are lower-cased (the code also lower-cases on Windows, which is not a supported platform; INC-2026-0011) |
| URL | network-style targets | same scheme, same host (case-insensitive), same port after defaults (`https` → 443, `http` → 80); an empty or `/` path matches every path; otherwise the target path must equal the selector path or continue it with `/` (whole segments: `/users/42` never covers `/users/420`; a selector ending in `/` covers everything below it); a trailing `*` makes it a raw string prefix |
| IP or CIDR | URL targets whose host is an IP literal | single IP equality or CIDR containment (IPv4 and IPv6) |
| anything else | env names, logical targets (`crm/tools/search`, `users/example.Users/GetUser`, `PROFILE/ACCOUNT/use`) | plain `globset` match against the text |

Private addresses (`is_private`): RFC 1918, loopback, link-local (including `169.254.169.254`),
unspecified, broadcast, CGNAT `100.64.0.0/10`, IPv6 loopback/unspecified, `fc00::/7`, `fe80::/10` and
IPv4-mapped forms of those. The host `localhost` counts as `127.0.0.1`. Hosts of non-special schemes
(`udp://`, `quic://`, `tcp://`) are parsed as IPs too, so the rule applies on every scheme. A grant
"names it literally" when one of its targets is that IP, a CIDR containing it, or a URL whose host is that
IP and which otherwise matches the target (same scheme and port). `"*"` never counts as literal.

Hostnames are resolved later by the adapter: `checked_addr` in `src/domain/effect_checks.rs` (and the UDP /
QUIC variants) resolves once and, when a resolved address is private, issues a **second** intent for
`scheme://IP:port`, which only a literal grant can allow. A public hostname grant therefore cannot be
rebound to an internal address.

### Broker sequence (one effect attempt)

```text
 interpreter            adapter / use case          TracedEvaluator        PolicyBroker        MemoryTraceStore
 (run_effect)           (files, http, udp, …)       (runtime.rs)           (policy_broker.rs)  (trace_store.rs)
     │ with_effect_scope{request_id, trace_id,              │                     │                    │
     │   operation_id, line} ─────────▶│                    │                     │                    │
     │                                 │ evaluate(intent) ─▶│                     │                    │
     │                                 │                    │ evaluate(intent) ──▶│                    │
     │                                 │                    │                     │ decide = authorize_effect
     │                                 │                    │                     │ push Permit to decision log
     │                                 │                    │◀──────── Permit ────│  (max 10 000)      │
     │                                 │                    │ current_effect_scope()                   │
     │                                 │                    │ effect_id = intent.effect_id, else looked up
     │                                 │                    │   by (operation, line, capability, verb)  │
     │                                 │                    │ attempt += 1 per (request, effect_id)     │
     │                                 │                    │ record(TraceEvent{…, target redacted,     │
     │                                 │                    │   decision, policy_hash, outcome.rule}) ─▶│
     │                                 │◀──── Permit ───────│                     │                    │
     │                    Denied → permission.denied (exit 3 / HTTP 403), nothing touched               │
     │                    Allowed → dial / open / spawn                                                  │
```

Every attempt is decided separately: each retry, each redirect hop, each resolved address and each file
named by an option line (`tls ca_file`, `body file`, read through `read_file` in `src/infra/effect_args.rs`)
produces its own intent. A `file update` is two intents (`allow_read` `stat` then `allow_write` `update`).

### Manifest generation pipeline (`rivet io`)

```text
 app.rivet ──compile──▶ CompiledProgram
                              │  analyze_program()  (once, at Runtime::assemble; pure, deterministic IDs op#N)
                              ▼
                        EffectCatalog { operations[ {operation_id, private, sites[], calls[]} ], load_sites[] }
                              │
   IoQuery ──validate──▶ select entries (IDs | public | --all) ──▶ follow literal (request "id") edges
   (--by --format                                                     (breadth-first, longest call chain)
    --kind --access)          │
                              ▼
                        sites of reached operations ──▶ filter --kind / --access
                              │
              ┌───────────────┼──────────────────┬───────────────────┬──────────────────────┐
              ▼               ▼                  ▼                   ▼                      ▼
        --check-policy    --trace REQ        group targets        --needs             --include-bootstrap
        site_decision()   join TraceStore    (TargetSummary)      OperationNeeds        bootstrap_list()
        allowed/partial/  on effect_id →                          ──▶ --check-files     + connector
        denied/unknown    attempts{count,…}                           broker stat →      descriptor/schema
                                                                      FileProbe           reads
              └───────────────┴──────────────────┴───────────────────┴──────────────────────┘
                              ▼
                        IoManifest ──▶ render: table (by operation|target|capability) | markdown | csv | json
                              │         stderr: decision counts, probe counts, incompleteness list
                              ▼
                        exit 3 (denied/partial, not_permitted/unreadable) > 4 (missing file) > 7 (--strict incomplete) > 0
```

Knowledge classes (`Knowledge` in `src/domain/io_manifest.rs`): `exact`, `bounded` (enum params expanded
into `bound`), `param_dependent` (template with `{param}`, derived glob), `dynamic` (target from a
non-param expression; shown as `<dynamic: EXPR>`), `opaque_remote` and `opaque_native`. A manifest is
`complete` only when no site is `dynamic` or opaque.

### Policy draft pipeline (`rivet policy generate`)

```text
 IoManifest (selected IDs or --all, no bootstrap)
     │
     ├── dynamic / opaque_remote / opaque_native / param_dependent option-file sites ──▶ review list
     │
     └── grantable sites ─▶ target per site:
            network          → origin scheme://host:port
            bounded          → every bound value
            param_dependent  → glob (./out/{name}.json → ./out/*.json)
            auth             → PROFILE/ACCOUNT/<verb>
            otherwise        → template as written
         rebase path targets when --output lives in another directory (policies/x.json → ../out/…)
         merge verbs per (capability, target); drop `access` when the union is the whole verb set
            │
            ▼
      PolicyDraft.render()  ──parse_policy() (same strict schema; failure = internal error)──▶ text
            │
            ├── no --output ─▶ stdout
            └── --output PATH ─▶ ExclusiveDraftWriter (create_new; existing path/symlink → conflict.exists, exit 4)
      stderr: "policy generate: N grants, M review items"   exit 7 when M > 0, else 0
```

## Interfaces

### CLI

| Command | Behaviour | Exit codes |
|---|---|---|
| `rivet check --file F [--policy P]` | compiles and loads the policy; a schema error stops here | 0, 2 |
| `rivet policy explain --file F` | prints the effective policy (file + sha256, base dir, network, limits, every grant/deny; `"*"` flagged as broad) | 0, 2 |
| `rivet policy explain ID --file F [--params JSON]` | the above plus the `--check-policy` table for that operation; with `--params`, each site whose placeholders are all params of the call is filled with the concrete target (`fill_params`, knowledge `exact`) and evaluated. In 0.2.0 this flag is still named `--params` (it is **not** the deprecated `request --params` and prints no warning; there is no `--data` here) | 0, 2, 4; **3** when `--params` is given and a concrete target is denied |
| `rivet io [ID …] [--all] [--by operation\|target\|capability] [--kind K] [--access V,V] [--format table\|json\|markdown\|csv] [--check-policy] [--strict] [--trace REQ] [--needs] [--check-files] [--include-bootstrap]` | the I/O manifest | 0, 2, 3, 4, 7 |
| `rivet policy generate [ID …\|--all] [--output PATH]` | least-privilege draft | 0, 4, 7 |
| `rivet trace show REQ` | one request's decisions from this host's trace store | 0, 2, 4 |
| `rivet trace export REQ --output PATH` | write that trace as JSON to a new bundle-relative file (broker: `allow_write` create) | 0, 3, 4 |
| `rivet graph ID [--all] [--json]` | static call graph (local only) | 0, 2, 4 |
| `rivet --endpoint URL [--token-file F] io …` / `trace show REQ` | the same use cases on a running server (`--endpoint` refuses `--file`/`--policy`) | as above, 3 when the principal is not allowed |

With `--json` (0.2.0), `io`, `policy explain`, `policy generate`, `trace show` and `check` print one
ResponseEnvelope whose `operation` is the built-in and whose `data` is the report. Failures print an error
envelope on stderr. `policy generate` **without** `--json` still prints the bare draft on stdout, so
`> policy.json` keeps working (PLAN-2026-0002, TASK-011). With `--json` its `data` is
`{policy, review, complete}`:

```text
$ rivet --file app.rivet --json policy generate notes.touch
{"request_id":"req_011c59653d","trace_id":"tr_011c59653d","operation":"rivet.policy.generate","type":"result","status":"ok","data":{"policy":{"version":1,"grants":[{"capability":"allow_read","targets":["./out/a.json"],"access":["stat"]},{"capability":"allow_write","targets":["./out/a.json"],"access":["update"]}],"network":{"deny_private_ranges":true}},"review":[],"complete":true},"error":null,"effects":"none","data_count":0}
policy generate: 2 grants, 0 review items          ← stderr
```

### Library (`src/orchestrator/runtime.rs`)

```text
Runtime::builder().file(p) | .source(path, text, root) | .root(dir)
                  .policy_file(p)        -- explicit file (same rules as --policy)
                  .policy(Policy)        -- Policy::from_file(p) / Policy::from_json(bytes) / built by the host
                  .ceiling(Policy)       -- host ceiling (intersection; limits take the minimum)
                  .build()
rivet::orchestrator::runtime::policy_from_json(bytes, base_dir) -- same strict parser as policy.json
rt.policy() · rt.io(&IoQuery) · rt.generate_policy_draft(ids, all, output) · rt.trace(req) · rt.trace_store()
rt.export_trace(req, path) · rt.graph(&GraphQuery) · rt.request_restricted(id, params, restrict, sink)
rt.load(path) / rt.load_as(path, alias)  -- 0.2.0: the module runs under this runtime's policy and ceiling
```

In 0.2.0 the stable facade exports `rivet::Policy` (with `Policy::from_file`, `Policy::from_json`) and
`rivet::types::IoQuery`. The paths above under `rivet::orchestrator::…` moved to the hidden
`rivet::internal::…` ([SYS-2026-0010](sys-2026-0010-ffi-surface-and-packaging.md#facade)).

Without `.file(...)` and without an explicit policy, the builder uses `Policy::deny_all(root)`.

### Network surfaces

`GET /v1/io` (the bare `IoManifest`; `format=table|markdown|csv` or `report=true` for the rendered
`IoReport`), `POST /v1/policy/generate` (never writes files), and the built-ins `rivet.io`,
`rivet.policy.generate`, `rivet.trace.show`, `rivet.trace.export` are served by `rivet serve`. They reveal internal URLs and
paths, so a network principal needs an **exact** entry for them in `serve.principals` (`*` and `prefix.*`
never match them); the loopback principal `local` may always call them. Details in
[SYS-2026-0004](sys-2026-0004-surfaces-and-serve.md).

## Configuration

This component consumes the `policy.json` keys `version`, `grants`, `deny` and `network`; `limits`,
`approved` and `serve` are parsed by the same loader but consumed elsewhere (dispatcher, MCP connectors,
serve). The complete schema with every error message is in
[SYS-2026-0008](../configuration/sys-2026-0008-policy-json-reference.md).

```text
 discovery:  --policy PATH given?  ── yes ──▶ that file (missing → policy.invalid "--policy P: no such file", exit 2)
                   │ no
                   ▼
             <dir of --file>/policy.json exists? ── yes ──▶ load + validate (error → policy.invalid, exit 2)
                   │ no
                   ▼
             Policy::deny_all(entry dir)  (present=false: every new application effect denied)
```

Relative path selectors resolve against the **policy file's own directory** (`base`), not the working
directory; effect targets resolve against the **bundle root**. There is no environment variable and no
command-line grant syntax.

## Runtime Behaviour

### Loading

`rivet policy explain` shows what was loaded. From `docs/demos/02-file-crud`:

```text
$ rivet policy explain --file app.rivet
policy   ./policy.json (sha256:deccf2027323af83c9798a05d6cac1adb657a81852e54ac7b99ce3eca4b0bef3)
base     .
network  deny_private_ranges true
limits   64 concurrent, depth 16, 268435456 buffered bytes
grant    allow_read ./out, ./out/**
grant    allow_write ./out/**
grant    allow_delete ./out/**
```

With no `policy.json` beside the entry file:

```text
$ rivet policy explain --file app.rivet
policy   none — no policy.json: every new application effect is denied (pure operations still run)
base     .
network  deny_private_ranges true
limits   64 concurrent, depth 16, 268435456 buffered bytes

$ rivet request notes.create --file app.rivet --data '{"text":"hi"}'
{"request_id":"req_0161980a65","trace_id":"tr_0161980a65","operation":"notes.create","type":"result","status":"error","data":null,"error":{"kind":"permission","code":"permission.denied","message":"allow_write create on ./out/note.json denied: no policy.json: allow_write is denied by default (add a grant for ./out/note.json)","retryable":false,"source":{"file":"app.rivet","line":9,"column":5,"end_line":9,"end_column":52},"operation_id":"notes.create","details":{"capability":"allow_write","access":"create","target":"./out/note.json"}},"effects":"none","data_count":0}
exit=3
```

(Request and trace IDs vary per run.)

### Decisions in practice

The following outputs come from a scratch bundle with six operations (`notes.save` creates
`./out/${name}.json`, `notes.touch` updates `./out/a.json`, `data.read` / `data.secret` read
`./data/in.txt` / `./data/private/k.txt`, `cloud.meta` GETs `http://169.254.169.254/latest`, `local.get`
GETs `http://127.0.0.1:18439/x`) and this policy. They were re-captured on the 0.2.0-rc; the source lines
differ from the 0.1.0 captures because the scratch bundle was rewritten.

```json
{
  "version": 1,
  "grants": [
    {"capability": "allow_read", "targets": ["./out/**"], "access": ["stat"]},
    {"capability": "allow_write", "targets": ["./out/**"], "access": ["create"]},
    {"capability": "allow_read", "targets": ["./data/**"]},
    {"capability": "allow_network", "targets": ["*", "http://127.0.0.1:18439"]}
  ],
  "deny": [
    {"capability": "allow_read", "targets": ["./data/private/**"]}
  ]
}
```

```text
$ rivet policy explain --file app.rivet
policy   ./policy.json (sha256:f288f0d547622c648aca3b8e87e2eff7f2bd46eb8634eabc01b3e31507dc19d4)
base     .
network  deny_private_ranges true
limits   64 concurrent, depth 16, 268435456 buffered bytes
grant    allow_read ./out/** access [stat]
grant    allow_write ./out/** access [create]
grant    allow_read ./data/**
grant    allow_network *, http://127.0.0.1:18439   ⚠ broad: "*" allows every target
deny     allow_read ./data/private/**

$ rivet io --file app.rivet --check-policy
OPERATION    KIND     ACCESS       TARGET                         KNOWLEDGE        SOURCE        DECISION
cloud.meta   network  connect GET  http://169.254.169.254/latest  exact            app.rivet:26  denied
data.read    file     read         ./data/in.txt                  exact            app.rivet:16  allowed
data.secret  file     read         ./data/private/k.txt           exact            app.rivet:21  denied
local.get    network  connect GET  http://127.0.0.1:18439/x       exact            app.rivet:34  allowed
notes.save   file     create       ./out/{name}.json              param_dependent  app.rivet:4   allowed
notes.touch  file     stat         ./out/a.json                   exact            app.rivet:10  allowed
notes.touch  file     update       ./out/a.json                   exact            app.rivet:10  denied
4 allowed · 3 denied
exit=3
```

The runtime refuses the same attempts with the rule that decided them:

| Request | Decisive rule (from the `permission.denied` message) | Exit |
|---|---|---|
| `notes.touch` | ``grant allow_write ./out/** access [create] does not include `update` `` | 3 |
| `data.secret` | `deny allow_read ./data/private/**` (deny wins over the `./data/**` grant) | 3 |
| `cloud.meta` | `169.254.169.254 is a private/loopback/link-local address; grant it literally (e.g. "http://169.254.169.254:80/latest") to allow it` (`"*"` does not count) | 3 |
| `local.get` | allowed by the literal `http://127.0.0.1:18439` grant; the run then fails with `connection.refused` because nothing listens there | 5 |
| `local.get` with the grant changed to port 18438 | `127.0.0.1 is a private/loopback/link-local address; …` (a literal grant must also match scheme and port) | 3 |

Full envelope of one denial:

```text
$ rivet request notes.touch --file app.rivet
{"request_id":"req_013f5e87fd","trace_id":"tr_013f5e87fd","operation":"notes.touch","type":"result","status":"error","data":null,"error":{"kind":"permission","code":"permission.denied","message":"allow_write update on ./out/a.json denied: grant allow_write ./out/** access [create] does not include `update`","retryable":false,"source":{"file":"app.rivet","line":10,"column":5,"end_line":10,"end_column":56},"operation_id":"notes.touch","details":{"capability":"allow_write","access":"update","target":"./out/a.json"}},"effects":"none","data_count":0}
exit=3
$ rivet request local.get --file app.rivet          # allowed by the literal grant; nothing listens
{"request_id":"req_013dabbbfd","trace_id":"tr_013dabbbfd","operation":"local.get","type":"result","status":"error","data":null,"error":{"kind":"connection","code":"connection.refused","message":"cannot connect to 127.0.0.1 (127.0.0.1:18439): Connection refused (os error 61)","retryable":false,"source":{"file":"app.rivet","line":34,"column":5,"end_line":36,"end_column":8},"operation_id":"local.get"},"effects":"none","data_count":0}
exit=5
```

A deny entry narrowed with `access` blocks only those verbs:

```text
policy: grants allow_read ./out/**, allow_write ./out/**; deny allow_write ./out/** access [update]

$ rivet io notes.save notes.touch --file app.rivet --policy deny-verb.json --check-policy
OPERATION    KIND  ACCESS  TARGET             KNOWLEDGE        SOURCE        DECISION
notes.save   file  create  ./out/{name}.json  param_dependent  app.rivet:4   allowed
notes.touch  file  stat    ./out/a.json       exact            app.rivet:10  allowed
notes.touch  file  update  ./out/a.json       exact            app.rivet:10  denied
2 allowed · 1 denied
exit=3
```

### Static decisions (`--check-policy`)

`inspect_effects` uses a static evaluator over the same policy (no decision log, no trace). Per site and
per verb:

| Knowledge | Evaluated target | Result |
|---|---|---|
| `exact` | the template itself | `allowed` / `denied` |
| `bounded` with `bound` values | every bound value | `allowed` (all), `partial` (some), `denied` (none) |
| `param_dependent` | an arbitrary instance of the template | `allowed`; else `partial` when some grant selector instance falls inside the site's glob (files) or origin (network) and is itself allowed; else `denied` |
| `dynamic`, `opaque_native`, empty template | — | `unknown` |

The worst verb decides the site (`denied` > `partial` > `unknown` > `allowed`). Exit 3 when any site is
`denied` or `partial`. UDP multicast sites mirror exactly what the runtime authorizes when the socket opens:
`allow_network` connect on the group, then `allow_listen` `bind` and `multicast_join` on the **bind address**
(the `bind` option, or the unspecified address on the group's port), so the static verdict and the run agree.

With `policy explain ID --data JSON` (alias `--params`), param-dependent sites of the entry operation — and of the
operations it calls with statically known arguments (`(users.fetch {id: id})`, a constant or a global) — are first
filled with the call's values, so they are evaluated as `exact` targets. With `--json`, a denial prints a
`status: "error"` envelope (kind `permission`, exit 3) on stderr whose `error.details` holds the explanation (scratch bundle, `demo.read` =
`return file read path as text`, policy granting `allow_read ./data/**`; the policy header lines are omitted):

```text
$ rivet --file app.rivet policy explain demo.read --params '{"path":"app.rivet"}'           exit 3
OPERATION  KIND  ACCESS  TARGET     KNOWLEDGE  SOURCE        DECISION
demo.read  file  read    app.rivet  exact      app.rivet:13  denied
denied: demo.read#1 allow_read app.rivet (read)
$ rivet --file app.rivet policy explain demo.read --params '{"path":"./data/x.txt"}'        exit 0
OPERATION  KIND  ACCESS  TARGET        KNOWLEDGE  SOURCE        DECISION
demo.read  file  read    ./data/x.txt  exact      app.rivet:13  allowed
```

### Views, needs and file probes

```text
$ rivet io --file app.rivet --by target --check-policy
TARGET                     ACCESS       CAPABILITY     ORIGIN       PHASE    NEEDS FILE  USED BY      DECISION
http://169.254.169.254:80  connect GET  allow_network  http get     connect  —           cloud.meta   denied
./data/in.txt              read         allow_read     file read    body     yes         data.read    allowed
./data/private/k.txt       read         allow_read     file read    body     yes         data.secret  denied
http://127.0.0.1:18439     connect GET  allow_network  http get     connect  —           local.get    allowed
./out/*.json               create       allow_write    file create  body     no          notes.save   allowed
./out/a.json               stat         allow_read     file update  body     yes         notes.touch  allowed
./out/a.json               update       allow_write    file update  body     yes         notes.touch  denied
4 allowed · 3 denied
exit=3

$ rivet io --file app.rivet --check-files
cloud.meta needs no existing files.
data.read needs, before it can run:
  ./data/in.txt         (file read)     present
data.secret needs, before it can run:
  ./data/private/k.txt  (file read)     not_permitted
local.get needs no existing files.
notes.save needs no existing files.
notes.touch needs, before it can run:
  ./out/a.json          (file update)   present
3 files · 2 present · 1 not_permitted
exit=3

$ rivet io notes.touch --file app.rivet --check-files        # after deleting out/a.json
notes.touch needs, before it can run:
  ./out/a.json  (file update)   missing
1 file · 1 missing
exit=4
```

`--check-files` authorizes `allow_read` `stat` on each distinct exact path first; a denial reports
`not_permitted` and the path is never touched. Only then does `FileProbe` read metadata (the file is never
opened). Non-exact paths are `not_checkable`.

Dynamic targets and `--strict` (scratch bundle whose URL comes from a file):

```text
$ rivet io --file app.rivet --strict
OPERATION  KIND     ACCESS       TARGET              KNOWLEDGE  SOURCE
cfg.fetch  file     read         ./data/cfg.json     exact            app.rivet:3
cfg.fetch  network  connect GET  <dynamic: cfg.url>  dynamic          app.rivet:4
demo.read  file     read         {path}              param_dependent  app.rivet:13
io: complete=false — 1 of 3 sites is dynamic/opaque
  cfg.fetch#2  network connect GET  target from expression cfg.url  (app.rivet:4)
exit=7
```

Usage errors are validated before any work, for example
`rivet io --kind file --access connect` → `error[validation.usage]: --access connect: not an access verb of kind file`, exit 2.

### Policy generation

```text
$ rivet policy generate notes.save notes.touch --file app.rivet
{
  "version": 1,
  "grants": [
    {"capability": "allow_read", "targets": ["./out/a.json"], "access": ["stat"]},
    {"capability": "allow_write", "targets": ["./out/*.json"], "access": ["create"]},
    {"capability": "allow_write", "targets": ["./out/a.json"], "access": ["update"]}
  ],
  "network": {"deny_private_ranges": true}
}
policy generate: 3 grants, 0 review items
exit=0

$ rivet policy generate notes.touch --file app.rivet --output policies/draft.json
policy generate: 2 grants, 0 review items
$ cat policies/draft.json                       # targets rebased onto the draft's directory
{
  "version": 1,
  "grants": [
    {"capability": "allow_read", "targets": ["../out/a.json"], "access": ["stat"]},
    {"capability": "allow_write", "targets": ["../out/a.json"], "access": ["update"]}
  ],
  "network": {"deny_private_ranges": true}
}

$ rivet policy generate --file app.rivet --output draft.json     # second time
{"request_id":"","trace_id":"","operation":"rivet.policy.generate","type":"result","status":"error","data":null,"error":{"kind":"conflict","code":"conflict.exists","message":"refusing to overwrite draft.json","retryable":false,"details":{"path":"draft.json"}},"effects":"none","data_count":0}
exit=4

$ rivet policy generate cfg.fetch --file app.rivet              # bundle with a dynamic URL
{
  "version": 1,
  "grants": [
    {"capability": "allow_read", "targets": ["./data/cfg.json"], "access": ["read"]}
  ],
  "network": {"deny_private_ranges": true}
}
review  cfg.fetch#2  network connect GET  <dynamic: cfg.url>  app.rivet:4  not granted (dynamic target)
policy generate: 1 grants, 1 review item — draft incomplete
exit=7
```

A draft never contains `deny`, `limits`, `serve`, `approved` or secret material, and always sets
`network.deny_private_ranges: true`. Network sites become their origin, so a draft for a literal private
address (for example `http://169.254.169.254:80`) is a literal grant: review drafts before adopting them.

### Globals, file modules and bootstrap reads (0.2.0)

**Global substitution.** `effect_sites::analyze_program` reads the frozen `GlobalScope` of the site's own
file (SYS-2026-0001) and substitutes every global it finds in a target template, header or path. The knowledge
class is then decided on what remains:

```text
 http get "${users_url}/${id}"      users_url = "${api}/users", api = "https://api.example.com"
      │ substitute globals  ─▶ "https://api.example.com/users/{id}"
      ▼
 only literals and globals left?  ── yes ─▶ exact             (grantable as written)
      │ no: a param placeholder remains
      ▼
 param_dependent  (host and path known, so the origin / glob is still grantable)
```

The proposal's UC-04 sample showed `…/users/{id}` as `exact`. The build keeps the 0.1.0 rule that a param
placeholder makes a target `param_dependent`, as the plan's findings table records (TASK-023). Real output
from the globals bundle of [MAN-2026-0003](../../manuals/man-2026-0003-language-guide.md#globals):

```text
$ rivet --file app.rivet io
OPERATION  KIND     ACCESS       TARGET                              KNOWLEDGE        SOURCE
users.get  network  connect GET  https://api.example.com/users/{id}  param_dependent  app.rivet:10
```

A global used as a file path is substituted the same way. In the modules bundle below,
`global archive = "./out/last.json"` and `file write archive json w` give the `exact` site `./out/last.json`.
A global can never hold a secret or an `env` value (`check.global_not_constant`), so substitution never places
secret material in a manifest, draft or trace.

**One policy across modules.** A bundle of several files is compiled into one program and runs under **the
loader's policy** only: the entry bundle's `policy.json` (or `--policy`), intersected with the host ceiling
and every `restrict`. A `policy.json` beside a module is never loaded (`check.module_policy_ignored`, a
warning). `rivet io`, `policy generate`, `policy explain` and `rivet graph` cover every file, with each site's
own `file:line`. Connector and auth profile names stay unique across the bundle (`check.import_collision`),
because the single policy grants them by name (`allow_mcp NAME/…`, `allow_auth PROFILE/…`). A module loaded at
run time (`Runtime::load`, `rivet_load`) joins the same runtime and so the same policy and ceiling. Its sites
enter the next catalog snapshot's effect catalog.

```text
 app.rivet ─ policy.json (the ONE policy) ───────────────┐
   └─ import "./svc/weather.rivet" as weather public      │ governs every file
        svc/weather.rivet ─ svc/policy.json  ✗ ignored ───┘ (warning check.module_policy_ignored)
```

Real output (scratch bundle: `app.rivet` imports `svc/weather.rivet` as `weather public`; the entry policy
grants `allow_network https://api.weather.example` and `allow_write ./out/**`; `svc/policy.json` grants
`allow_network *` and is ignored):

```text
$ rivet --file app.rivet check
warning[check.module_policy_ignored]: the policy.json beside svc/weather.rivet is ignored: module `weather` runs under the loader's policy
  --> app.rivet:1:1
   |
  1| import "./svc/weather.rivet" as weather public
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  = hint: grant what the module needs in the entry bundle's policy.json (or the host policy)
ok: 3 operations, 0 connectors, 0 auth profiles
exit=0
$ rivet --file app.rivet io --check-policy
OPERATION         KIND     ACCESS          TARGET                                  KNOWLEDGE        SOURCE                DECISION
report.now        (calls weather.now — see above)                                                   app.rivet:9
report.now        file     create, update  ./out/last.json                         exact            app.rivet:10          allowed
weather.now       network  connect GET     https://api.weather.example/now/{city}  param_dependent  svc/weather.rivet:7   allowed
weather.stations  network  connect GET     https://api.weather.example/stations    exact            svc/weather.rivet:17  allowed
3 allowed
exit=0
$ rivet --file app.rivet policy generate --all
{
  "version": 1,
  "grants": [
    {"capability": "allow_write", "targets": ["./out/last.json"], "access": ["create", "update"]},
    {"capability": "allow_network", "targets": ["https://api.weather.example:443"]}
  ],
  "network": {"deny_private_ranges": true}
}
policy generate: 2 grants, 0 review items
exit=0
$ rivet --file app.rivet policy explain weather.now --params '{"city":"oslo"}'      (policy header omitted)
OPERATION    KIND     ACCESS       TARGET                                KNOWLEDGE  SOURCE               DECISION
weather.now  network  connect GET  https://api.weather.example/now/oslo  exact      svc/weather.rivet:7  allowed
exit=0
$ rivet --file app.rivet graph report.now
report.now                                                            app.rivet:5
├── call weather.now                                                  app.rivet:9
│   └── network  connect GET  https://api.weather.example/now/{city}  svc/weather.rivet:7
└── file     create, update ./out/last.json                           app.rivet:10
exit=0
```

**Bootstrap reads of imported files.** `inspect_effects` (`bootstrap_sites`) lists the entry file and then **each** module file
as its own `file read` row, in resolution order. In 0.1.0 (and in the INC-2026-0008 known issue) there was a
single `./app.rivet (+ imports)` placeholder. That placeholder is gone; `tests/conformance_io_manifest.rs`
asserts the per-file rows. These reads happen before the policy exists, so they are listed and never governed
by it. The loader confines them to the runtime root and refuses symlinks.

```text
$ rivet --file app.rivet io --include-bootstrap        (the BOOTSTRAP part only)
BOOTSTRAP (runtime-internal; listed, not governed by policy.json)
KIND  ACCESS       TARGET
file  read         ./app.rivet
file  read         ./svc/weather.rivet
file  read         ./policy.json
file  read         system CA bundle
file  read         /etc/resolv.conf / system resolver
file  read         tzdata
file  read         descriptor/schema files named by connectors (none here)
pipe  read, write  stdin, stdout, stderr
exit=0
```

When no `policy.json` exists beside the entry, the row reads `./policy.json (when present)`.

### Traces

The trace store lives in the process that ran the request. A one-shot CLI process starts with an empty
store, so `rivet trace show` and `rivet io --trace` are only useful against a running server:

```text
$ rivet trace show req_01f723dfc5 --file app.rivet
{"request_id":"","trace_id":"","operation":"rivet.trace.show","type":"result","status":"error","data":null,"error":{"kind":"not_found","code":"not_found.trace","message":"no trace for request `req_01f723dfc5` in this host's trace store","retryable":false},"effects":"none","data_count":0}
exit=4
```

Against a running server the answer is a `rivet.trace.show` envelope whose `data` is the `TraceResult`. This
capture is from a scratch copy of 02-file-crud on `127.0.0.1:18901`:

```text
$ rivet --endpoint http://127.0.0.1:18901 trace show req_01087ca4cd
{"request_id":"req_0282643a62","trace_id":"tr_0282643a62","operation":"rivet.trace.show","type":"result","status":"ok","data":{"request_id":"req_01087ca4cd","attempts":[{"request_id":"req_01087ca4cd","trace_id":"tr_01087ca4cd","node_id":null,"attempt":1,"effect_id":"notes.create#1","operation_id":"notes.create","phase":"decision","capability":"allow_write","access":"create","target":"./out/note.json","decision":"allowed","policy_hash":"sha256:deccf2027323af83c9798a05d6cac1adb657a81852e54ac7b99ce3eca4b0bef3","source":{"file":"app.rivet","line":9,"column":5},"outcome":{"rule":"grant allow_write ./out/**"}}],"complete":true,"next_cursor":null,"gaps":0},"error":null,"effects":"none","data_count":0}
exit=0
```

A network principal needs an exact `rivet.trace.show` entry in `serve.principals`. In a 0.1.0 run, a
principal listed only with `*` got ``principal `ada` may not call `rivet.trace.show` ``, exit 3.

A request sent with the deprecated input keys `id`/`params` has one extra trace event (0.2.0): phase `input`,
access `deprecated`, decision `deprecated` and no `effect_id`. The manifest join (`io --trace`) ignores it,
and operators can count it (see [OPS-2026-0001](../../operations/ops-2026-0001-operating-rivet-serve.md)).

File decisions carry the operation ID and the statement span (every file effect runs in its effect scope), so
their trace events have a `source`. `Runtime::export_trace(req, path)` serializes the same `TraceResult` as
pretty JSON and writes it with a `file create` through the broker; verified from a library host at `829ca43`:

```text
export -> {"request_id":"req_0195ed6d65","path":"./audit/trace.json","events":1,"bytes":649}
export again -> conflict.already_exists
(with a host ceiling that lacks allow_write ./audit/**)
export -> permission.denied allow_write create on ./audit/trace.json denied: host ceiling: no grant for allow_write ./audit/trace.json
```

## Data and Storage

| Data | Where | Bound | Lifetime |
|---|---|---|---|
| Effective `Policy` (grants, deny, network, limits, approved, serve, `sha256`, `base_dir`) | `PolicyBroker.policy` | — | runtime |
| Decision log (`Vec<Permit>`, allow and deny) | `PolicyBroker.decisions` | first 10 000 decisions | runtime |
| `EffectCatalog` (sites per operation, connector load sites) | built with each catalog snapshot (`Runtime::assemble`, and again on a module load) | — | snapshot (immutable) |
| Site index `(operation, line, capability, verb) → effect_id` | `TracedEvaluator.index` | — | runtime |
| Attempt counters per `(request, effect_id)` | `TracedEvaluator.attempts` | cleared when above 50 000 keys | runtime |
| `TraceEvent`s | `MemoryTraceStore` | 10 000 events, oldest evicted first; evictions counted per request as `gaps` | runtime |
| Policy drafts | file named by `--output` | — | on disk, created exclusively |

`TraceEvent` fields: `request_id`, `trace_id`, `node_id`, `attempt`, `effect_id`, `operation_id`, `phase`
(`"decision"`), `capability`, `access`, `target`, `decision`, `policy_hash` (the policy file's
`sha256:…`), `source`, `outcome` (`{"rule": …}`). Targets are redacted before storage: query strings,
fragments and URL userinfo are dropped (`redact_target`). `read_trace` pages with `limit` (1..10 000,
default 1000) and a numeric `cursor`; a result with evicted events reports `complete: false` and `gaps`.

## Dependencies

| Dependency | Used for |
|---|---|
| `serde_json` (with `preserve_order`) | strict parsing of `policy.json`; manifest JSON |
| `globset` | path and logical selector matching; selector validation at load |
| `url` | URL selector and target parsing, default ports |
| `sha2` | `policy.json` hash (`sha256:` prefix) and draft receipts |
| `tokio` task-locals | `EffectScope` attribution of decisions to request and source line |
| Ports: `PolicyFileReader`, `PolicyEvaluator`, `PolicyDraftWriter`, `TraceStore`, `FileProbe`, `Registry` | defined in `src/domain/ports.rs`; implemented in `src/infra/*` |

Upstream: the compiler ([SYS-2026-0001](sys-2026-0001-compiler-and-catalog.md)) provides the compiled IR.
Downstream: every adapter in [SYS-2026-0005](../integrations/sys-2026-0005-protocol-adapters.md),
OAuth in [SYS-2026-0006](../integrations/sys-2026-0006-oauth-and-credentials.md), MCP connectors in
[SYS-2026-0009](../integrations/sys-2026-0009-mcp-client-connectors.md), and execution in
[SYS-2026-0002](../runtime/sys-2026-0002-execution-scopes-and-dag.md).

## Deployment

Nothing is deployed separately; the broker and manifest are part of the `rivet` binary and the library.
Operators ship a `policy.json` next to the entry `.rivet` file (or pass `--policy PATH`). The recommended
loop is:

```text
 write operation ─▶ rivet check ─▶ rivet io --by target ─▶ rivet policy generate --output policy.json
        ▲                                                                   │ review, edit
        └──────────── rivet io --check-policy (exit 0) ◀────────────────────┘
```

## Security Boundaries

- **Deny by default.** No file means every application effect is denied; there is no implicit allow-all.
  `"*"` must be written and is flagged as broad by `policy explain`.
- **Deny wins.** A matching deny entry is checked before any grant.
- **Least privilege by verb.** `access` narrows a grant to some verbs of its capability; `create` does not
  imply `update`.
- **SSRF defaults.** Private, loopback, link-local and metadata addresses need a literal grant on every
  scheme, and resolved hostnames are re-checked before dialing.
- **Path confinement.** Targets are normalized lexically (`./data/../secret` does not match `./data/**`),
  and the file adapter confines paths under the bundle root.
- **Per-attempt evaluation.** Retries, redirects, resolved addresses and option files are each authorized.
- **No secrets in evidence.** Trace targets are redacted; drafts contain no secrets; bootstrap reads are
  listed but never granted.
- **Network exposure of evidence.** Manifest, drafts and traces reveal internal URLs and paths, so
  network principals need exact `serve.principals` entries for `rivet.io`, `rivet.policy.generate`,
  `rivet.trace.show`, `rivet.trace.export` (and `rivet.connectors.sync`).
- **Narrow-only layering.** A host ceiling and per-request restrictions can only remove authority; a
  restriction naming ungranted targets adds nothing, and a malformed one is `policy.invalid` (`/restrict/…`).
- **Secrets.** The broker authorizes targets; the interpreter additionally refuses to let a `secret` value reach
  any sink outside its bound origins (SYS-2026-0002).

## Observability

| Question | Tool |
|---|---|
| Which policy is in force, with which hash? | `rivet policy explain` (`--json`: a `rivet.policy.explain` envelope whose `data` is `{present, file, sha256, grants, deny, broad}`, with counts for `grants` and `deny`) |
| What will this operation touch, and is it allowed? | `rivet io ID --check-policy`, `rivet policy explain ID` |
| Which files must exist before a run? | `rivet io --needs`, `rivet io --check-files` |
| Why was an attempt denied? | the `permission.denied` message and `details {capability, access, target}`; `outcome.rule` in the trace |
| What did a request actually do? | `rivet --endpoint URL trace show REQ`, `rivet io --trace REQ` (planned vs actual, `unplanned` attempts), `Runtime::export_trace` |
| Would this concrete call be allowed? | `rivet policy explain ID --params JSON` (exit 3 when denied) |
| What does this operation call and touch? | `rivet graph ID [--json]` |

Summaries (decision counts, probe counts, incompleteness) go to stderr; stdout carries only the requested
format, so `--format json` output can be piped. `rivet --json io` wraps the manifest in a `rivet.io` envelope
(`data` = the manifest), while `rivet io --format json` prints the bare manifest.

## Known Limitations

From the [manual's Known Limitations](../../manuals/man-2026-0001-rivet-manual.md#known-limitations):

- **No persistent trace store**: traces are in memory only, bounded at 10 000 events, and lost when the process
  exits; a one-shot CLI process always has an empty store.
- **`approved.overlaps` is unused**: parsed and validated, never consulted.
- The `*` principal pattern matches `rivet.auth.*` (governed by `allow_auth`; see SYS-2026-0004).

Other current behaviour: on macOS path selectors and targets are compared lower-cased. Windows is not a
supported platform in 0.2.0 ([INC-2026-0011](../../incidents/active/inc-2026-0011-windows-port-failures.md)). The
`rivet.trace.export` dispatch defect found at `829ca43` was fixed in `2a751ab` (INC-2026-0007); a literal call to an
operation without sites renders as `(calls X — no I/O)` in `io --check-policy` tables.

## Last Verified Version

0.2.0-rc (main at `8031baa`), 2026-09-29, macOS, `target/release/rivet` (`cargo build --release --features cli`).
Every capture in Runtime Behaviour was re-run on scratch bundles: the six-operation decision bundle, a dynamic-URL
bundle, the MAN-2026-0003 globals bundle, a two-file module bundle with an ignored module policy, and a copy of
02-file-crud served on 127.0.0.1:18901 (stopped afterwards). Global substitution and the per-file bootstrap rows
were checked in `effect_sites.rs` and `inspect_effects.rs`. Request IDs, trace IDs and policy hashes vary per run
and per file.

History: 0.1.0-dev (commit 829ca43), macOS, `target/debug/rivet`. Outputs were first produced at `f40d4aa` against
`docs/demos/02-file-crud` and scratch bundles; the ceiling, restriction, `explain --params`, trace export and
multicast rows were verified at `829ca43` (scratch bundles, a library host and `tests/conformance_udp.rs`); request IDs, trace IDs and policy hashes
vary per run and per file.

## Related Documents

- [PROP-2026-0001 Rivet runtime](../../proposals/implemented/prop-2026-0001-rivet-runtime.md)
- [PLAN-2026-0001 v0.1.0 implementation and release](../../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md)
- [SYS-2026-0008 policy.json schema v1 reference](../configuration/sys-2026-0008-policy-json-reference.md)
- [SYS-2026-0004 Surfaces and serve](sys-2026-0004-surfaces-and-serve.md)
- [SYS-2026-0001 Compiler and catalog](sys-2026-0001-compiler-and-catalog.md)
- [SYS-2026-0002 Execution scopes and DAG](../runtime/sys-2026-0002-execution-scopes-and-dag.md)
- [SYS-2026-0005 Protocol adapters](../integrations/sys-2026-0005-protocol-adapters.md)
- [SYS-2026-0006 OAuth and credentials](../integrations/sys-2026-0006-oauth-and-credentials.md)
- [SYS-2026-0009 MCP client connectors](../integrations/sys-2026-0009-mcp-client-connectors.md)
- [REF-2026-0002 Language and usage](../../references/ref-2026-0002-language-and-usage.md)
- [API-2026-0006 Envelopes](../../api/api-2026-0006-envelopes.md)
- [SEC-2026-0001 Policy and sandbox model](../../security/sec-2026-0001-policy-and-sandbox-model.md) (module boundaries)
- [PLAN-2026-0002](../../plans/plan-2026-0002-rivet-v0-2-0-implementation-and-release.md) (D-33, D-47)
- [Demos](../../demos/README.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial current-state document (PLAN-2026-0001 D-17). |
| 2 | 2026-09-28 | Claude | TASK-092 drift fix for the fix batch (829ca43): host ceiling and per-request restriction in the decision flow, URL path segments, `policy explain --params` (exit 3), multicast manifest mirrors the runtime, file decisions carry source spans, `Runtime::export_trace`, `rivet graph`, bare `/v1/io`; limitations reduced to the current ones; export defect fixed in 2a751ab. |
| 3 | 2026-09-29 | Claude | PLAN-2026-0002 D-33/D-47 (TASK-073, TASK-070): global substitution into targets (`exact` vs `param_dependent`), one policy across file modules (module `policy.json` ignored; io/generate/explain/graph over every file; run-time loads), bootstrap rows per imported file (the `(+ imports)` placeholder is gone); `--json` outputs as envelopes; `policy explain --params` flag noted; captures re-run on the 0.2.0-rc; macOS/Linux only. |
| 4 | 2026-09-29 | Claude | INC-2026-0012: `policy explain --data` (alias `--params`), params followed along call edges, `--json` denial envelope. |
