---
document_id: DEMO-2026-0012
title: "Embed Rivet as a Rust library"
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

# Embed Rivet as a Rust library

**Draft usage sample.** Rivet has no runtime or CLI implementation yet. These files make the proposed interface concrete; commands below are intended usage, not executed demos.

## Purpose

Embed Rivet as a Rust library. Delivery stage: **A**. Read [app.rivet](app.rivet) alongside this walkthrough.

| Operation ID | Output | Behavior |
|---|---|---|
| `demo.greet` | `text` | Same pure operation as the catalog sample. |
| `demo.add` | `integer` | Typed library request. |
| `demo.health` | `object {ready}` | Pure readiness call. |
| `events.count` | `object {count}`, emits `integer` | Scoped stream with pull-based backpressure. |

```text
  host process (Tokio)
  +------------------------------------------------------------+
  | Policy::from_file("policy.json")  --+                      |
  | include_str!("app.rivet")  ---------+--> Runtime::builder() |
  |                                          .build()          |
  |   rt.request("demo.add", …)      -> Completion {result: 5} |
  |   rt.scope(|scope| scope.stream("events.count", …))        |
  |        s.next() -> Ok(Some(Envelope)) x4, then Ok(None)     |
  |   rt.outputs("demo.add")         -> OutputSpec              |
  +------------------------------------------------------------+
     no CLI subprocess, no listener; host code stays outside the sandbox
```

## Verified Against Version

None. Based on proposal revision 5 semantics (UQ-17, E11) and S04, S70–S72, S106 and S115. No parser, runtime or network fixture execution is claimed.

## Prerequisites

A future Rivet build implementing this stage. External services below are controlled fixtures, not public services to contact. Commands assume the repository root initially, then the setup directory. Each folder is an independent bundle; do not concatenate folders with duplicate IDs.

## Setup

```sh
cd docs/demos/12-library
```

Read [embedding.rs.txt](embedding.rs.txt). It is intentionally a proposed API sketch, not a Cargo project. The eventual host supplies the Tokio executor and the source bytes; no CLI subprocess or listening server is needed. Build the runtime once and reuse it.

[policy.json](policy.json) is included only because the canonical sketch calls `Policy::from_file("policy.json")`. It contains no grants, only `limits` (`max_concurrent_requests` 64, `max_call_depth` 16, `max_buffered_bytes` 256 MiB). A grant-free file behaves exactly like having no file, which is deny-by-default. The four operations are pure, so nothing is denied. A host ceiling passed by the embedding application intersects with this file; it can never widen it.

## Steps

### Command / Request

The canonical library sketch, as in [embedding.rs.txt](embedding.rs.txt):

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

Compare with the same operations through the CLI, which auto-discovers the same policy.json:

```sh
rivet --file app.rivet request demo.add --params '{"a":2,"b":3}'
rivet --file app.rivet request events.count --params '{}' --stream
rivet --file app.rivet outputs demo.add
rivet --file app.rivet outputs --all --json
```

Use the catalog sample's serve steps with this app.rivet to compare the REST, SSE, polling, WebSocket and MCP surfaces. Scoped duplex embedding for `chat.exchange` is specified in reference S115, using the 10-grpc bundle and its fixture prerequisites.

### Expected Output / Response

`c.result` is `5`. The `events.count` stream yields `Ok(Some(Envelope))` for data 1, 2, 3, then the terminal result `{"count":3}`, then `Ok(None)`. A terminal failure arrives as `Err(RivetError)`. `rt.outputs("demo.add")` returns the same OutputSpec that `RegistryEntry.output` holds and that the CLI prints:

```text
demo.add — Add two integers
output  integer  Sum of a and b.
emits    —
receives —
errors   —
```

`rivet outputs --all --json` (first entry shown; the library `OutputSpec` serializes to the same JSON):

```json
[
  {"id": "demo.add",
   "output": {"type": "integer", "description": "Sum of a and b."},
   "emits": null, "receives": null, "errors": []}
]
```

- Dropping a request future hands cleanup to the runtime, which joins it.
- Sinks passed to `request` must be `Send + 'static`; the sketch passes `None` for a unary call.
- Nested requests carry `parent_request_id`.
- A policy file that fails to load (`policy.invalid`) makes `Policy::from_file` return an error before `build()`. The CLI equivalent exits 2.

## Effects and policy

The four operations are pure. Host code stays outside a library's sandbox boundary, so an arbitrary host callback or sink that performs native file or network I/O must be controlled by the host. Do not describe the runtime policy as an OS sandbox.

## Inspect before invoking

```sh
rivet --file app.rivet check --strict-docs
rivet --file app.rivet io --by target
rivet --file app.rivet io --check-policy
```

`check --strict-docs` passes: all four operations describe their params, output, output fields and emitted items.

**I/O manifest.** All four operations are pure:

```text
TARGET   ACCESS   CAPABILITY   USED BY
(no application effect sites: demo.add, demo.greet, demo.health, events.count are pure)
```

`io --check-policy` evaluates against the grant-free [policy.json](policy.json) and exits 0. The library exposes the same data. `rt.io(IoQuery::default())?` returns an `IoManifest` with `sites: []` and `complete: true`. `rt.generate_policy(&["demo.add"])?` returns a `PolicyDraft` whose `policy` is `{"version": 1, "grants": [], "network": {"deny_private_ranges": true}}` and whose `review` list is empty. Neither call writes a file. The library boundary note still applies: host callbacks and sinks are outside the manifest.

`--include-bootstrap` adds the fixed runtime-internal list under a separate `bootstrap` key: the bundle and imports, policy.json, the CA bundle, resolv.conf or the system resolver, tzdata, descriptor/schema files, and stdin/stdout/stderr. It is listed for transparency, never granted to scripts. Sandbox guarantees apply to **script-initiated effects through brokered adapters**. `io` performs no I/O and evaluates no source expression. Exit codes: 3 when `--check-policy` finds a reachable site denied or partial; 7 with `--strict` when any site is dynamic or opaque (`complete: false`); otherwise 0.

## Release Updates

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-01 | UQ-17 / R23 | `Runtime::outputs` / `OutputSpec` | `rt.outputs("demo.add")` | Integer output spec | Not run — runtime does not exist |
| U-02 | UQ-17 / R24; review E11 | Canonical builder with `Policy::from_file` replaces `Context::restricted` | See sketch | Same results | Not run |
| U-03 | UQ-18 / R26 | `io` became the generated I/O manifest (targets, access verbs, capability, `--by`, `--check-policy`) | Inspect before invoking | Tables above | Not run — runtime does not exist |

## Cleanup

Drop or stop the host runtime after awaiting owned scopes. No files are generated.

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
- [Proposal](../../proposals/draft/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest: no sites; `io --check-policy` exits 0; library `rt.io` / `rt.generate_policy` equivalents. |
| 2 | 2026-09-28 | Claude | UQ-17/E11: embedding sketch replaced by the canonical builder (`Policy::from_file`, `rt.request`, `rt.scope`/`scope.stream`, `rt.outputs`); added grant-free policy.json; declared outputs; removed `--sandbox`; View outputs, strict-docs, bootstrap and exit codes. |
| 1 | 2026-09-28 | Codex | Added draft source files, prerequisites, invocation examples and expected behavior. |
