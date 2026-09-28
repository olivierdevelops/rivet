---
document_id: DEMO-2026-0009
title: "QUIC streams and HTTP3"
document_type: demo
status: draft
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 4
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

# QUIC streams and HTTP3

**Draft usage sample.** Rivet has no runtime or CLI implementation yet. These files make the proposed interface concrete; commands below are intended usage, not executed demos.

## Purpose

QUIC streams and HTTP3. Delivery stage: **B**. Read [app.rivet](app.rivet) alongside this walkthrough.

| Operation ID | Output | Behavior |
|---|---|---|
| `engine.status` | `object {state}` | Reliable bidirectional QUIC stream. |
| `items.http3` | `object {items, version}` | Explicit HTTP3 request. |

```text
 engine.status                                   items.http3
 with quic "quic://engine.example.com:4433"      http get "https://api.example.com/items"
   alpn / max_streams / migration  (options)       version 3   (no silent fallback)
   with connection.open bidi as stream              |
     framing length32 endian big (option)           v
     send {action:"status"} --> <-- {"state":"ready"}   {"items":[], "version":3}
   scope exit: stream reset/finish, connection closed  -- even on the early return
```

## Verified Against Version

None. Based on proposal revision 8 semantics (UQ-17) and S94 and S99–S101. No parser, runtime or network fixture execution is claimed.

## Prerequisites

A future Rivet build implementing this stage. External services below are controlled fixtures, not public services to contact. Commands assume the repository root initially, then the setup directory. Each folder is an independent bundle; do not concatenate folders with duplicate IDs.

## Setup

```sh
cd docs/demos/09-quic
```

The QUIC fixture engine.example.com:4433 needs a trusted matching certificate, ALPN `rivet-rpc/1` and 32-bit big-endian length-prefixed JSON. A status frame returns `{"state":"ready"}`. The HTTPS fixture supports H3 and answers `GET /items` with `{"items":[]}`. If you use another controlled fixture, replace endpoints and policy grants together.

```text
  policy.json            (auto-discovered)  allow_network quic://engine.example.com:4433   -> engine.status
  policies/http3.json    (--policy)         allow_network https://api.example.com:443      -> items.http3
```

## Steps

### Command / Request

```sh
rivet --file app.rivet request engine.status --params '{}'
rivet --file app.rivet --policy ./policies/http3.json request items.http3 --params '{}'
```

View the declared outputs:

```sh
rivet --file app.rivet outputs items.http3
rivet --file app.rivet outputs --all --json
```

### Expected Output / Response

`engine.status` returns `{"state":"ready"}` and `items.http3` returns `{"items":[],"version":3}`, each exit 0. Stream and connection disposal follow scope exit, even on the early return.

| Failure | Code / kind | HTTP | Exit |
|---|---|---|---|
| Wrong ALPN, certificate or QUIC handshake | `quic.*` (protocol/connection) | 502 | 5 |
| Server lacks H3 | typed `unsupported`/protocol error, no fallback | 501/502 | 5 |
| Stream reply exceeds `timeout "5s"` | `timeout` | 504 | 6 |
| Operation run under the other policy file | `permission.denied` | 403 | 3 |

`rivet outputs items.http3`:

```text
items.http3 — Fetch items over HTTP3
output  object   Items plus the negotiated HTTP version.
  items    list json  required  Items returned by GET /items.
  version  integer    required  Negotiated HTTP version; always 3.
emits    —
receives —
errors   —
```

`rivet outputs --all --json` (first entry shown):

```json
[
  {"id": "engine.status",
   "output": {"type": "object", "description": "Engine status frame returned on the QUIC stream.",
              "properties": {"state": {"type": "string", "description": "Engine state; the fixture answers \"ready\"."}},
              "required": ["state"], "additionalProperties": false},
   "emits": null, "receives": null, "errors": []}
]
```

## Effects and policy

The HTTPS-origin grant allows this HTTP adapter's same-origin QUIC/UDP transport, not raw UDP access. Native QUIC has a separate scheme-specific grant. There is no 0-RTT, and native migration is false here.

## Inspect before invoking

```sh
rivet --file app.rivet check --strict-docs
rivet --file app.rivet io --by target
rivet --file app.rivet io --check-policy
rivet --file app.rivet io engine.status --check-policy
rivet --file app.rivet --policy ./policies/http3.json io items.http3 --check-policy
```

`check --strict-docs` passes. Both public operations describe their output and fields. All resource options (`alpn`, `max_streams`, `migration`, `framing`) precede the first body statement; an option after a statement would be `syntax.option_after_body` (exit 2).

**`io --by target`**. The protocol is recorded per site, so HTTP3 over QUIC is not confused with raw QUIC:

```text
TARGET                           ACCESS                 CAPABILITY      ORIGIN     PHASE     NEEDS FILE   USED BY
quic://engine.example.com:4433   connect (quic)         allow_network   with quic  connect   —            engine.status
https://api.example.com:443      connect GET (http3)    allow_network   http get   connect   —            items.http3
```

**`io --check-policy`** with the default [policy.json](policy.json), which grants only the QUIC endpoint:

```text
OPERATION       KIND      ACCESS        TARGET                           KNOWLEDGE   SOURCE         DECISION
engine.status   network   connect       quic://engine.example.com:4433   exact       app.rivet:7    allowed
items.http3     network   connect GET   https://api.example.com/items    exact       app.rivet:26   denied
```

Checking both operations exits 3 under either policy file, because each file covers exactly one operation. The per-operation checks above exit 0. In JSON, `engine.status` has `"protocol": "quic"` and `items.http3` has `"protocol": "http3", "method": "GET"`.

`--include-bootstrap` adds the fixed runtime-internal list under a separate `bootstrap` key: the bundle and imports, policy.json, the CA bundle, resolv.conf or the system resolver, tzdata, descriptor/schema files, and stdin/stdout/stderr. It is listed for transparency, never granted to scripts. Sandbox guarantees apply to **script-initiated effects through brokered adapters**. `io` performs no I/O and evaluates no source expression. Exit codes: 3 when `--check-policy` finds a reachable site denied or partial; 7 with `--strict` when any site is dynamic or opaque (`complete: false`); otherwise 0.

## Release Updates

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-01 | UQ-17 / R23 | Declared outputs | `rivet outputs --all --json` | Two entries | Not run — runtime does not exist |
| U-02 | UQ-17 / R24 | Two `--sandbox` variants became policy.json + policies/http3.json | Commands above | Same results | Not run |
| U-03 | UQ-18 / R26 | `io` became the generated I/O manifest (targets, access verbs, capability, `--by`, `--check-policy`) | Inspect before invoking | Tables above | Not run — runtime does not exist |

## Cleanup

Stop the QUIC and HTTPS fixtures. Scope exit owns application resources; do not add close calls.

## Verification Record

| Step | Verified by | Date | Result |
|---|---|---|---|
| JSON/JSONL parse, relative links, manifest IDs vs declared public IDs, no removed flags or bare durations | Claude static script | 2026-09-28 | Passed as documentation checks; not language conformance |
| Source IDs, JSON fixtures, links and documentation structure | Codex static review | 2026-09-28 | Checked as documentation; not language conformance |
| Parse/execute and validate expected output | Future implementation fixture suite | Not run | BLOCKED — runtime does not exist |

## Known Caveats

Expected values assume the declared fixture behavior. Request, trace and session IDs are generated; compare application results rather than literal IDs. Runtime errors remain typed and retain partial-effect information.

## Related Documents

- [All sample folders](../README.md)
- [Usage reference](../../references/ref-2026-0002-language-and-usage.md)
- [Proposal](../../proposals/approved/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 4 | 2026-09-28 | Claude | TASK-005/R26 (ADR-0001, proposal revision 8): `io --by target` gains ORIGIN (`with quic`, `http get`), PHASE and NEEDS FILE. |
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest: protocol-tagged sites by target and `io --check-policy` against policy.json and policies/http3.json. |
| 2 | 2026-09-28 | Claude | UQ-17: declared outputs; quoted `timeout "5s"`; `--sandbox` variants became policy.json and policies/http3.json; leading-options note; View outputs, strict-docs, bootstrap and exit codes. |
| 1 | 2026-09-28 | Codex | Added draft source files, prerequisites, invocation examples and expected behavior. |
