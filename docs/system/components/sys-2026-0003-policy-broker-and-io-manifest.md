---
document_id: SYS-2026-0003
title: "Rivet policy broker and I/O manifest"
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
components: [policy, audit]
affected_versions:
  from: "0.1.0"
  to: null
last_verified_version: "0.1.0-dev (commit 829ca43)"
next_review_date: 2026-10-28
review_cycle: on-release
confidentiality: internal
scope: How Rivet loads policy.json, authorizes every effect attempt through the traced policy broker, and derives the static I/O manifest, policy drafts and request traces.
reason: Every application effect in Rivet passes through one decision function; operators and reviewers need an accurate description of how that decision is made, what the I/O manifest reports and where the evidence (traces, drafts) lives.
related_documents: [PROP-2026-0001, PLAN-2026-0001, SYS-2026-0001, SYS-2026-0002, SYS-2026-0004, SYS-2026-0005, SYS-2026-0006, SYS-2026-0008, SYS-2026-0009]
supersedes: null
superseded_by: null
tags: [rivet, system, policy, broker, audit, io-manifest, trace, least-privilege]
---

# Rivet policy broker and I/O manifest

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** policy, audit
> **Last Verified Version:** 0.1.0-dev (commit 829ca43)

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
- **Does not govern bootstrap reads.** The entry bundle, `policy.json` itself, CA bundles, the resolver,
  tzdata, connector descriptor/schema snapshots and stdio are listed with `--include-bootstrap` but are
  never checked against `policy.json`.
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
| path glob | `allow_read`, `allow_write`, `allow_delete`, `allow_exec`, `allow_unix`, `allow_pipe` | selector is joined to the **policy file's directory** and normalized; the target is joined to the **bundle root**; `globset` with `literal_separator(true)` (`*` stays inside one segment, `**` crosses); `dir/**` also matches `dir` itself; on macOS and Windows both sides are lower-cased |
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
| `rivet policy explain ID --file F [--params JSON]` | the above plus the `--check-policy` table for that operation; with `--params`, each site whose placeholders are all params of the call is filled with the concrete target (`fill_params`, knowledge `exact`) and evaluated | 0, 2, 4; **3** when `--params` is given and a concrete target is denied |
| `rivet io [ID …] [--all] [--by operation\|target\|capability] [--kind K] [--access V,V] [--format table\|json\|markdown\|csv] [--check-policy] [--strict] [--trace REQ] [--needs] [--check-files] [--include-bootstrap]` | the I/O manifest | 0, 2, 3, 4, 7 |
| `rivet policy generate [ID …\|--all] [--output PATH]` | least-privilege draft | 0, 4, 7 |
| `rivet trace show REQ` | one request's decisions from this host's trace store | 0, 2, 4 |
| `rivet trace export REQ --output PATH` | write that trace as JSON to a new bundle-relative file (broker: `allow_write` create) | 0, 3, 4 |
| `rivet graph ID [--all] [--json]` | static call graph (local only) | 0, 2, 4 |
| `rivet --endpoint URL [--token-file F] io …` / `trace show REQ` | the same use cases on a running server (`--endpoint` refuses `--file`/`--policy`) | as above, 3 when the principal is not allowed |

### Library (`src/orchestrator/runtime.rs`)

```text
Runtime::builder().file(p) | .source(path, text, root)
                  .policy_file(p)        -- explicit file (same rules as --policy)
                  .policy(Policy)        -- Policy::from_file(p) / Policy::from_json(bytes) / built by the host
                  .ceiling(Policy)       -- host ceiling (intersection; limits take the minimum)
                  .build()
rivet::orchestrator::runtime::policy_from_json(bytes, base_dir) -- same strict parser as policy.json
rt.policy() · rt.io(&IoQuery) · rt.generate_policy_draft(ids, all, output) · rt.trace(req) · rt.trace_store()
rt.export_trace(req, path) · rt.graph(&GraphQuery) · rt.request_restricted(id, params, restrict, sink)
```

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

$ rivet request notes.create --file app.rivet --params '{"text":"hi"}'
{"request_id":"req_017689aced","trace_id":"tr_017689aced","error":{"kind":"permission","code":"permission.denied","message":"allow_write create on ./out/note.json denied: no policy.json: allow_write is denied by default (add a grant for ./out/note.json)","retryable":false,"effects":"none","source":{"file":"app.rivet","line":9,"column":5,"end_line":9,"end_column":52},"operation_id":"notes.create","details":{"capability":"allow_write","access":"create","target":"./out/note.json"}}}
exit=3
```

(Request and trace IDs vary per run.)

### Decisions in practice

The following outputs come from a scratch bundle with six operations (`notes.save` creates
`./out/${name}.json`, `notes.touch` updates `./out/a.json`, `data.read` / `data.secret` read
`./data/in.txt` / `./data/private/k.txt`, `cloud.meta` GETs `http://169.254.169.254/latest`, `local.get`
GETs `http://127.0.0.1:18439/x`) and this policy:

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
cloud.meta   network  connect GET  http://169.254.169.254/latest  exact            app.rivet:42  denied
data.read    file     read         ./data/in.txt                  exact            app.rivet:26  allowed
data.secret  file     read         ./data/private/k.txt           exact            app.rivet:34  denied
local.get    network  connect GET  http://127.0.0.1:18439/x       exact            app.rivet:52  allowed
notes.save   file     create       ./out/{name}.json              param_dependent  app.rivet:8   allowed
notes.touch  file     stat         ./out/a.json                   exact            app.rivet:18  allowed
notes.touch  file     update       ./out/a.json                   exact            app.rivet:18  denied
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
$ rivet request notes.touch --file app.rivet --params '{}'
{"request_id":"req_015d36e1ad","trace_id":"tr_015d36e1ad","error":{"kind":"permission","code":"permission.denied","message":"allow_write update on ./out/a.json denied: grant allow_write ./out/** access [create] does not include `update`","retryable":false,"effects":"none","source":{"file":"app.rivet","line":18,"column":5,"end_line":18,"end_column":43},"operation_id":"notes.touch","details":{"capability":"allow_write","access":"update","target":"./out/a.json"}}}
exit=3
```

A deny entry narrowed with `access` blocks only those verbs:

```text
policy: grants allow_read ./out/**, allow_write ./out/**; deny allow_write ./out/** access [update]

$ rivet io notes.save notes.touch --file app.rivet --policy deny-verb.json --check-policy
OPERATION    KIND  ACCESS  TARGET             KNOWLEDGE        SOURCE        DECISION
notes.save   file  create  ./out/{name}.json  param_dependent  app.rivet:8   allowed
notes.touch  file  stat    ./out/a.json       exact            app.rivet:18  allowed
notes.touch  file  update  ./out/a.json       exact            app.rivet:18  denied
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

With `policy explain ID --params JSON`, param-dependent sites of the entry operation are first filled with the
call's values, so they are evaluated as `exact` targets (scratch bundle at `829ca43`, `demo.read` =
`return file read path as text`, policy granting `allow_read ./data/**`):

```text
$ rivet --file app.rivet policy explain demo.read --params '{"path":"app.rivet"}'           exit 3
OPERATION  KIND  ACCESS  TARGET     KNOWLEDGE  SOURCE        DECISION
demo.read  file  read    app.rivet  exact      app.rivet:73  denied
denied: demo.read#1 allow_read app.rivet (read)
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
cfg.fetch  file     read         ./data/cfg.json     exact      app.rivet:5
cfg.fetch  network  connect GET  <dynamic: cfg.url>  dynamic    app.rivet:6
io: complete=false — 1 of 2 sites is dynamic/opaque
  cfg.fetch#2  network connect GET  target from expression cfg.url  (app.rivet:6)
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
{"request_id":"","trace_id":"","error":{"kind":"conflict","code":"conflict.exists","message":"refusing to overwrite draft.json","retryable":false,"effects":"none","details":{"path":"draft.json"}}}
exit=4

$ rivet policy generate --file app.rivet                        # bundle with a dynamic URL
{
  "version": 1,
  "grants": [
    {"capability": "allow_read", "targets": ["./data/cfg.json"], "access": ["read"]}
  ],
  "network": {"deny_private_ranges": true}
}
review  cfg.fetch#2  network connect GET  <dynamic: cfg.url>  app.rivet:6  not granted (dynamic target)
policy generate: 1 grants, 1 review item — draft incomplete
exit=7
```

A draft never contains `deny`, `limits`, `serve`, `approved` or secret material, and always sets
`network.deny_private_ranges: true`. Network sites become their origin, so a draft for a literal private
address (for example `http://169.254.169.254:80`) is a literal grant: review drafts before adopting them.

### Traces

The trace store lives in the process that ran the request. A one-shot CLI process starts with an empty
store, so `rivet trace show` and `rivet io --trace` are only useful against a running server:

```text
$ rivet trace show req_01f723dfc5 --file app.rivet
{"request_id":"","trace_id":"","error":{"kind":"not_found","code":"not_found.trace","message":"no trace for request `req_01f723dfc5` in this host's trace store","retryable":false,"effects":"none"}}
exit=4

$ rivet --endpoint http://127.0.0.1:18437 --token-file ci.token trace show req_02eb0dc172
{"request_id":"req_02eb0dc172","attempts":[{"request_id":"req_02eb0dc172","trace_id":"tr_02eb0dc172","node_id":null,"attempt":1,"effect_id":"data.secret#1","operation_id":"data.secret","phase":"decision","capability":"allow_read","access":"read","target":"./data/private/k.txt","decision":"denied","policy_hash":"sha256:fd3a186d2587b4be4a44c6455578c126847f8e163da1e5a407c2d2a26a152e5b","source":null,"outcome":{"rule":"deny allow_read ./data/private/**"}}],"complete":true,"next_cursor":null,"gaps":0}
exit=0
```

(In that run principal `ci` had an exact `rivet.trace.show` entry in `serve.principals`; a principal
listed only with `*` got ``principal `ada` may not call `rivet.trace.show` ``, exit 3.)

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
| `EffectCatalog` (sites per operation, connector load sites) | built in `Runtime::assemble` | — | runtime (immutable) |
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
| Which policy is in force, with which hash? | `rivet policy explain` (`--json` prints `{present, file, sha256, grants, deny}`) |
| What will this operation touch, and is it allowed? | `rivet io ID --check-policy`, `rivet policy explain ID` |
| Which files must exist before a run? | `rivet io --needs`, `rivet io --check-files` |
| Why was an attempt denied? | the `permission.denied` message and `details {capability, access, target}`; `outcome.rule` in the trace |
| What did a request actually do? | `rivet --endpoint URL trace show REQ`, `rivet io --trace REQ` (planned vs actual, `unplanned` attempts), `Runtime::export_trace` |
| Would this concrete call be allowed? | `rivet policy explain ID --params JSON` (exit 3 when denied) |
| What does this operation call and touch? | `rivet graph ID [--json]` |

Summaries (decision counts, probe counts, incompleteness) go to stderr; stdout carries only the requested
format, so `--format json` output can be piped.

## Known Limitations

From the [manual's Known Limitations](../../manuals/man-2026-0001-rivet-manual.md#known-limitations):

- **No persistent trace store**: traces are in memory only, bounded at 10 000 events, and lost when the process
  exits; a one-shot CLI process always has an empty store.
- **`approved.overlaps` is unused**: parsed and validated, never consulted.
- The `*` principal pattern matches `rivet.auth.*` (governed by `allow_auth`; see SYS-2026-0004).

Other current behaviour: on macOS and Windows path selectors and targets are compared lower-cased. The
`rivet.trace.export` dispatch defect found at `829ca43` was fixed in `2a751ab` (INC-2026-0007); a literal call to an
operation without sites renders as `(calls X — no I/O)` in `io --check-policy` tables.

## Last Verified Version

0.1.0-dev (commit 829ca43), macOS, `target/debug/rivet`. Outputs were first produced at `f40d4aa` against
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
- [Demos](../../demos/README.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Initial current-state document (PLAN-2026-0001 D-17). |
| 2 | 2026-09-28 | Claude | TASK-092 drift fix for the fix batch (829ca43): host ceiling and per-request restriction in the decision flow, URL path segments, `policy explain --params` (exit 3), multicast manifest mirrors the runtime, file decisions carry source spans, `Runtime::export_trace`, `rivet graph`, bare `/v1/io`; limitations reduced to the current ones; export defect fixed in 2a751ab. |
